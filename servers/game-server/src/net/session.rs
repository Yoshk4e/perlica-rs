use crate::error::ServerError;
use crate::handlers::gm;
use crate::net::crypto::CipherState;
use crate::net::{
    context::{NetContext, OutboundFrame, OutboundTx},
    notify::{MuipResult, Notification, PlayerHandle},
    registry::SessionRegistry,
    router::handle_command,
};
use crate::player::Player;
use config::BeyondAssets;
use perlica_db::{Persistable, PlayerDb};
use perlica_logic::traits::SyncWriteBack;
use perlica_muip::GmResponse;
use perlica_proto::{CsHead, prost::Message};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc,
    time::{Instant, MissedTickBehavior, interval_at},
};
use tracing::{debug, error, info, warn};

const PERSIST_INTERVAL: Duration = Duration::from_secs(30);

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

    let (outbound_tx, outbound_rx) = mpsc::channel::<OutboundFrame>(64);
    let (notify_tx, notify_rx) = mpsc::channel::<Notification>(32);

    let handle = PlayerHandle::new(notify_tx);

    let write_task = tokio::spawn(write_loop(writer, outbound_rx));

    let ctx = SessionContext {
        assets,
        registry,
        db,
    };

    let outbound = OutboundTx::new(outbound_tx);
    let result = logic_loop(reader, outbound, notify_rx, handle, ctx).await;

    let _ = write_task.await;

    result
}

/// Drains the outbound channel and writes each frame to the socket.
///
/// The channel carries `OutboundFrame`s that already contain a fully-serialized
/// packet (`[u8 head_size][u16 body_size][head][body]`) plus optional cipher
/// activation state. The write loop owns the outbound `CipherState`: it applies
/// the keystream to `head‖body` in place (leaving the 3-byte size prefix
/// untouched) before writing, and — critically — activates the pending cipher
/// AFTER the SC_LOGIN frame is on the wire so SC_LOGIN itself stays plaintext.
async fn write_loop(
    mut writer: tokio::net::tcp::OwnedWriteHalf,
    mut rx: mpsc::Receiver<OutboundFrame>,
) -> Result<(), ServerError> {
    let mut cipher = CipherState::new();

    while let Some(frame) = rx.recv().await {
        let OutboundFrame {
            mut bytes,
            arm_cipher,
        } = frame;

        // Encrypt head‖body in place; the 3-byte prefix stays plaintext.
        if bytes.len() > 3 {
            cipher.apply(&mut bytes[3..]);
        }

        writer.write_all(&bytes).await?;

        // If this frame is SC_LOGIN, `arm_cipher` carries the fresh outbound
        // ChaCha20 instance. Arm and immediately promote so the NEXT frame is
        // encrypted while SC_LOGIN itself went out plaintext.
        if let Some(c) = arm_cipher {
            cipher.arm(c);
            cipher.activate_pending();
        }
    }
    Ok(())
}

/// Per-connection event loop: routes incoming commands and server notifications,
/// saves player data to DB on clean exit.
async fn logic_loop(
    mut reader: tokio::net::tcp::OwnedReadHalf,
    outbound_tx: OutboundTx,
    mut notify_rx: mpsc::Receiver<Notification>,
    handle: PlayerHandle,
    ctx: SessionContext,
) -> Result<(), ServerError> {
    let mut player = Player::default();
    let mut server_seq_id = 0u64;
    let mut registered = false;
    let mut in_cipher = CipherState::new();
    // Set when a newer connection for this UID asks us to step aside. Held
    // until after this session has fully flushed and unregistered, so the
    // reply only goes out once it's actually safe for the new connection to
    // register the same UID.
    let mut takeover_ack: Option<tokio::sync::oneshot::Sender<()>> = None;

    info!("Session started");

    let mut persist_timer = interval_at(Instant::now() + PERSIST_INTERVAL, PERSIST_INTERVAL);
    persist_timer.set_missed_tick_behavior(MissedTickBehavior::Delay);

    let result = loop {
        tokio::select! {
            result = read_packet(&mut reader, &mut in_cipher) => {
                match result {
                    Ok((cmd_id, body, client_seq_id)) => {
                        let mut net_ctx = NetContext::new(
                            &mut player,
                            ctx.db,
                            ctx.assets,
                            &outbound_tx,
                            client_seq_id,
                            &mut server_seq_id,
                            ctx.registry,
                        );
                        if let Err(e) = handle_command(&mut net_ctx, cmd_id, body).await {
                            warn!("Command error: CmdId={cmd_id}, Error={e}");
                        }

                        // If the login handler armed the inbound cipher on this
                        // context, promote it now so the NEXT inbound frame is
                        // decrypted while CS_LOGIN itself stayed plaintext.
                        if let Some(c) = net_ctx.take_inbound_cipher() {
                            in_cipher.arm(c);
                            in_cipher.activate_pending();
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
                match notification {
                    Notification::TakeOver { respond_to } => {
                        info!(
                            "Session evicted by reconnect: UID={}",
                            player.uid
                        );
                        takeover_ack = Some(respond_to);
                        break Ok(());
                    }
                    other => {
                        let mut net_ctx = NetContext::new(
                            &mut player,
                            ctx.db,
                            ctx.assets,
                            &outbound_tx,
                            0,
                            &mut server_seq_id,
                            ctx.registry,
                        );
                        if handle_notification(&mut net_ctx, other).await {
                            break Ok(());
                        }
                    }
                }
            }

            _ = persist_timer.tick() => {
                if registered && player.char_bag.has_pending_changes()
                    && let Err(e) = ctx
                        .db
                        .persist_char_bag_incremental(&player.uid, &mut player.char_bag)
                        .await
                    {
                        warn!(
                            "Periodic char_bag flush failed: UID={}, Error={e}",
                            player.uid
                        );
                    }
            }
        }
    };

    if registered {
        player.movement.write_back_into(&mut player.world);

        if let Err(e) = player.world.persist(&player.uid, ctx.db).await {
            error!(
                "World persist failed on disconnect: UID={}, Error={e}",
                player.uid
            );
        }

        if let Err(e) = player.wallet.persist(&player.uid, ctx.db).await {
            error!(
                "Wallet persist failed on disconnect: UID={}, Error={e}",
                player.uid
            );
        }

        if let Err(e) = ctx
            .db
            .persist_char_bag_incremental(&player.uid, &mut player.char_bag)
            .await
        {
            error!(
                "CharBag final flush failed on disconnect: UID={}, Error={e}",
                player.uid
            );
        }

        ctx.registry.unregister(&player.uid);
        info!(
            "Player offline: UID={}, count={}",
            player.uid,
            ctx.registry.online()
        );
    }

    // Only signal the new connection once we've fully unregistered above —
    // otherwise it could register its own session for this UID and have this
    // (stale) unregister call remove it out from under it.
    if let Some(ack) = takeover_ack {
        let _ = ack.send(());
    }

    result
}

/// Reads one framed packet from `reader`, decrypting `head‖body` in place if the
/// inbound cipher is active. The 3-byte size prefix is always read plaintext.
async fn read_packet(
    reader: &mut tokio::net::tcp::OwnedReadHalf,
    in_cipher: &mut CipherState,
) -> std::io::Result<(i32, Vec<u8>, u64)> {
    let head_size = reader.read_u8().await?;
    let body_size = reader.read_u16_le().await?;

    let mut head_buf = vec![0u8; head_size as usize];
    reader.read_exact(&mut head_buf).await?;

    let mut body_buf = vec![0u8; body_size as usize];
    reader.read_exact(&mut body_buf).await?;

    // Once encryption is active, the peer XORed head then body over a single
    // continuous keystream. Decrypt in the same order so counters line up.
    in_cipher.apply(&mut head_buf);
    in_cipher.apply(&mut body_buf);

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
//
// `Notification::TakeOver` is intercepted in `logic_loop` before it ever
// reaches here (it needs to hold onto `respond_to` past the DB flush below),
// so it's unreachable at this point.
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
        Notification::TakeOver { respond_to } => {
            debug_assert!(false, "TakeOver should be handled in logic_loop");
            let _ = respond_to.send(());
            true
        }
    }
}
