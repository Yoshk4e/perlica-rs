use crate::net::context::NetContext;
use crate::net::notify::{Notification, PlayerHandle};
use crate::net::router::handle_command;
use crate::player::Player;
use config::BeyondAssets;
use perlica_proto::{CsHead, prost::Message};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tracing::{debug, info, instrument, warn};

pub async fn handle_connection(
    socket: TcpStream,
    addr: SocketAddr,
    assets: &'static BeyondAssets,
) -> anyhow::Result<()> {
    let (reader, writer) = socket.into_split();

    let (outbound_tx, outbound_rx) = mpsc::channel::<Vec<u8>>(64);
    let (notify_tx, notify_rx) = mpsc::channel::<Notification>(32);

    // Hand this handle out to any system that needs to reach this player.
    let _handle = PlayerHandle::new(notify_tx);
    // TODO: register _handle in a global session registry once that exists.

    let write_task = tokio::spawn(write_loop(writer, outbound_rx));

    let result = logic_loop(reader, outbound_tx, notify_rx, assets, addr).await;

    // outbound_tx dropped above, write_loop drains remaining frames then exits.
    let _ = write_task.await;

    result
}

// Drains the outbound channel and writes each pre-encoded frame to the socket.
// Exits cleanly when the sender side (logic loop) is dropped.
async fn write_loop(
    mut writer: tokio::net::tcp::OwnedWriteHalf,
    mut rx: mpsc::Receiver<Vec<u8>>,
) -> anyhow::Result<()> {
    while let Some(frame) = rx.recv().await {
        writer.write_all(&frame).await?;
    }
    Ok(())
}

#[instrument(skip(reader, outbound_tx, notify_rx, assets), fields(addr = %addr))]
async fn logic_loop(
    mut reader: tokio::net::tcp::OwnedReadHalf,
    outbound_tx: mpsc::Sender<Vec<u8>>,
    mut notify_rx: mpsc::Receiver<Notification>,
    assets: &'static BeyondAssets,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let mut player = Player::new(assets, "0".to_string());
    let mut server_seq_id = 0u64;

    info!("session started");

    loop {
        tokio::select! {
            result = read_packet(&mut reader) => {
                match result {
                    Ok((cmd_id, body, client_seq_id)) => {
                        let mut ctx = NetContext::new(
                            &mut player,
                            &outbound_tx,
                            client_seq_id,
                            &mut server_seq_id,
                        );
                        if let Err(e) = handle_command(&mut ctx, cmd_id, body).await {
                            warn!(error = %e, cmd_id, "command error");
                        }
                    }
                    Err(e) if is_clean_disconnect(&e) => {
                        debug!("disconnected");
                        return Ok(());
                    }
                    Err(e) => return Err(e.into()),
                }
            }

            Some(notification) = notify_rx.recv() => {
                handle_notification(&mut player, &outbound_tx, &mut server_seq_id, notification).await;
            }
        }
    }
}

// Reads one framed packet from the socket and returns (cmd_id, body, client_seq_id).
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
async fn handle_notification(
    _player: &mut Player,
    _outbound: &mpsc::Sender<Vec<u8>>,
    _server_seq_id: &mut u64,
    notification: Notification,
) {
    match notification {
        // Variants handled here as world systems are added.
        #[allow(unreachable_patterns)]
        _ => {}
    }
}
