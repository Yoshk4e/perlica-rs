use crate::error::ServerError;
use crate::handlers::gm;
use crate::net::{
    context::NetContext,
    notify::{MuipResult, Notification, PlayerHandle},
    registry::SessionRegistry,
    router::handle_command,
};
use crate::player::Player;
use config::BeyondAssets;
use perlica_db::{PlayerDb, PlayerRecordRef};
use perlica_muip::GmResponse;
use perlica_proto::{CsHead, prost::Message};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
};
use tracing::{debug, error, info, warn};

pub struct SessionContext {
    pub assets: &'static BeyondAssets,
    pub registry: &'static SessionRegistry,
    pub db: &'static PlayerDb,
}

/// Accepts a TCP connection, spawns the write loop, and runs the session to completion.
pub async fn handle_connection(
    socket: TcpStream,
    assets: &'static BeyondAssets,
    registry: &'static SessionRegistry,
    db: &'static PlayerDb,
) -> Result<(), ServerError> {
    let (reader, writer) = socket.into_split();

    let (outbound_tx, outbound_rx) = mpsc::channel::<Vec<u8>>(64);
    let (notify_tx, notify_rx) = mpsc::channel::<Notification>(32);

    let handle = PlayerHandle::new(notify_tx);

    let write_task = tokio::spawn(write_loop(writer, outbound_rx));

    let ctx = SessionContext {
        assets,
        registry,
        db,
    };

    let result = logic_loop(reader, outbound_tx, notify_rx, handle, ctx).await;

    let _ = write_task.await;

    result
}

/// Drains the outbound channel and writes each frame to the socket.
async fn write_loop(
    mut writer: tokio::net::tcp::OwnedWriteHalf,
    mut rx: mpsc::Receiver<Vec<u8>>,
) -> Result<(), ServerError> {
    while let Some(frame) = rx.recv().await {
        writer.write_all(&frame).await?;
    }
    Ok(())
}

/// Per-connection event loop: routes incoming commands and server notifications,
/// saves player data to DB on clean exit.
async fn logic_loop(
    mut reader: tokio::net::tcp::OwnedReadHalf,
    outbound_tx: mpsc::Sender<Vec<u8>>,
    mut notify_rx: mpsc::Receiver<Notification>,
    handle: PlayerHandle,
    ctx: SessionContext,
) -> Result<(), ServerError> {
    let mut player = Player::default();
    let mut server_seq_id = 0u64;
    let mut registered = false;

    info!("Session started");

    let result = loop {
        tokio::select! {
            result = read_packet(&mut reader) => {
                match result {
                    Ok((cmd_id, body, client_seq_id)) => {
                        let mut net_ctx = NetContext::new(
                            &mut player,
                            ctx.db,
                            ctx.assets,
                            &outbound_tx,
                            client_seq_id,
                            &mut server_seq_id,
                        );
                        if let Err(e) = handle_command(&mut net_ctx, cmd_id, body).await {
                            warn!("Command error: CmdId={cmd_id}, Error={e}");
                        }

                        if !registered && !player.uid.is_empty() {
                            ctx.registry.register(player.uid.clone(), handle.clone());
                            info!("Player online: UID={}, count={}", player.uid, ctx.registry.online());
                            registered = true;
                        }
                    }
                    Err(e) if is_clean_disconnect(&e) => {
                        debug!("Client disconnected cleanly");
                        break Ok(());
                    }
                    Err(e) => break Err(e.into()),
                }
            }

            Some(notification) = notify_rx.recv() => {
                let mut net_ctx = NetContext::new(
                    &mut player,
                    ctx.db,
                    ctx.assets,
                    &outbound_tx,
                    0,
                    &mut server_seq_id,
                );
                if handle_notification(&mut net_ctx, notification).await {
                    break Ok(());
                }
            }
        }
    };

    if registered {
        player.movement.sync_to_world(&mut player.world);

        let record_ref = PlayerRecordRef {
            char_bag: &player.char_bag,
            world: &player.world,
            bitsets: &player.bitsets,
            checkpoint: player.scene.get_checkpoint(),
            revival_mode: player.scene.current_revival_mode,
            missions: &player.missions,
            guides: &player.guides,
            mail: &player.mail,
        };

        if let Err(e) = ctx.db.save(&player.uid, record_ref).await {
            error!("Save failed: UID={}, Error={e}", player.uid);
        }

        ctx.registry.unregister(&player.uid);
        info!(
            "Player offline: UID={}, count={}",
            player.uid,
            ctx.registry.online()
        );
    }

    result
}

async fn read_packet(
    reader: &mut tokio::net::tcp::OwnedReadHalf,
) -> std::io::Result<(i32, Vec<u8>, u64)> {
    let head_size = reader.read_u8().await?;
    let body_size = reader.read_u16_le().await?;

    let mut head_buf = vec![0u8; head_size as usize];
    reader.read_exact(&mut head_buf).await?;

    let mut body_buf = vec![0u8; body_size as usize];
    reader.read_exact(&mut body_buf).await?;

    let head = CsHead::decode(&head_buf[..])
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    Ok((head.msgid, body_buf, head.up_seqid))
}

fn is_clean_disconnect(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::UnexpectedEof
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::BrokenPipe
    )
}

// Returns true if the session should terminate after this notification.
async fn handle_notification(ctx: &mut NetContext<'_>, notification: Notification) -> bool {
    match notification {
        Notification::MuipCommand {
            command,
            respond_to,
        } => {
            let result = match gm::execute(ctx, &command).await {
                Ok(outcome) => MuipResult {
                    response: GmResponse::ok(outcome.message),
                    disconnect: outcome.disconnect,
                },
                Err(message) => MuipResult {
                    response: GmResponse::err(400, message),
                    disconnect: false,
                },
            };

            let disconnect = result.disconnect;
            let _ = respond_to.send(result);
            disconnect
        }
    }
}
