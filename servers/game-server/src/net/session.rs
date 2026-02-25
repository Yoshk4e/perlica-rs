use crate::net::context::NetContext;
use crate::player::Player;
use config::BeyondAssets;
use perlica_proto::{CsHead, prost::Message};
use std::net::SocketAddr;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tracing::{debug, instrument, warn};

#[instrument(skip(socket, assets), fields(addr = %addr))]
pub async fn handle_connection(
    mut socket: TcpStream,
    addr: SocketAddr,
    assets: &'static BeyondAssets,
) -> anyhow::Result<()> {
    let mut player = Player::new(assets, "0".to_string());
    let mut server_seq_id = 0u64;

    let (mut reader, mut writer) = socket.split();

    loop {
        let head_size = match reader.read_u8().await {
            Ok(b) => b,
            // client closed the connection cleanly
            Err(e) if matches!(
                e.kind(),
                std::io::ErrorKind::UnexpectedEof
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::BrokenPipe
            ) => {
                debug!("disconnected");
                return Ok(());
            }
            Err(e) => return Err(e.into()),
        };

        let body_size = reader.read_u16_le().await?;

        let mut head_buf = vec![0u8; head_size as usize];
        reader.read_exact(&mut head_buf).await?;

        let mut body_buf = vec![0u8; body_size as usize];
        reader.read_exact(&mut body_buf).await?;

        let head = CsHead::decode(&head_buf[..])?;

        let mut ctx = NetContext::new(
            &mut player,
            assets,
            &mut writer,
            head.up_seqid,
            &mut server_seq_id,
        );

        if let Err(e) = crate::net::router::handle_command(&mut ctx, head.msgid, body_buf).await {
            warn!(error = %e, cmd_id = head.msgid, "command error");
        }
    }
}
