use crate::net::{
    context::NetContext,
    notify::{Notification, PlayerHandle},
    registry::SessionRegistry,
    router::handle_command,
};
use crate::player::Player;
use config::BeyondAssets;
use perlica_db::PlayerDb;
use perlica_proto::{CsHead, prost::Message};
use std::net::SocketAddr;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
};
use tracing::{debug, error, info, instrument, warn};

pub struct SessionContext {
    pub assets: &'static BeyondAssets,
    pub registry: &'static SessionRegistry,
    pub db: &'static PlayerDb,
    pub addr: SocketAddr,
}

pub async fn handle_connection(
    socket: TcpStream,
    addr: SocketAddr,
    assets: &'static BeyondAssets,
    registry: &'static SessionRegistry,
    db: &'static PlayerDb,
) -> anyhow::Result<()> {
    let (reader, writer) = socket.into_split();

    let (outbound_tx, outbound_rx) = mpsc::channel::<Vec<u8>>(64);
    let (notify_tx, notify_rx) = mpsc::channel::<Notification>(32);

    let handle = PlayerHandle::new(notify_tx);

    let write_task = tokio::spawn(write_loop(writer, outbound_rx));

    let ctx = SessionContext {
        assets,
        registry,
        db,
        addr,
    };

    let result = logic_loop(reader, outbound_tx, notify_rx, handle, ctx).await;

    // outbound_tx dropped here, write_loop drains remaining frames then exits.
    let _ = write_task.await;

    result
}

// Drains the outbound channel and writes each pre-encoded frame to the socket.
// Exits when the sender side (logic loop) is dropped.
async fn write_loop(
    mut writer: tokio::net::tcp::OwnedWriteHalf,
    mut rx: mpsc::Receiver<Vec<u8>>,
) -> anyhow::Result<()> {
    while let Some(frame) = rx.recv().await {
        writer.write_all(&frame).await?;
    }
    Ok(())
}

#[instrument(skip(reader, outbound_tx, notify_rx, handle, ctx), fields(addr = %ctx.addr))]
async fn logic_loop(
    mut reader: tokio::net::tcp::OwnedReadHalf,
    outbound_tx: mpsc::Sender<Vec<u8>>,
    mut notify_rx: mpsc::Receiver<Notification>,
    handle: PlayerHandle,
    ctx: SessionContext,
) -> anyhow::Result<()> {
    let mut player = Player::default();
    let mut server_seq_id = 0u64;
    let mut registered = false;

    info!("session started");

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
                            warn!(error = %e, cmd_id, "command error");
                        }

                        // uid is empty until on_login sets it.
                        if !registered && !player.uid.is_empty() {
                            ctx.registry.register(player.uid.clone(), handle.clone());
                            info!(uid = %player.uid, online = ctx.registry.online(), "player online");
                            registered = true;
                        }
                    }
                    Err(e) if is_clean_disconnect(&e) => {
                        debug!("disconnected");
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
                handle_notification(&mut net_ctx, notification).await;
            }
        }
    };

    // Runs on every exit path, clean disconnect, error, or future break.
    if registered {
        if let Err(e) = ctx
            .db
            .save(&player.uid, &player.char_bag, &player.world)
            .await
        {
            error!(uid = %player.uid, error = %e, "Save failed");
        }
        ctx.registry.unregister(&player.uid);
        info!(uid = %player.uid, online = ctx.registry.online(), "Player offline");
    }

    result
}

// Reads one framed packet and returns (cmd_id, body, client_seq_id).
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

// Dispatches an inbound server notification into the player's session.
// Each Notification variant maps to the logic that produces outbound frames.
async fn handle_notification(ctx: &mut NetContext<'_>, notification: Notification) {
    match notification {
        #[allow(unreachable_patterns)]
        _ => {}
    }
}
