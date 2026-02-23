use crate::player::Player;
use config::BeyondAssets;
use perlica_proto::{CsHead, NetMessage, prost::Message};
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;

pub struct NetContext<'logic> {
    pub player: &'logic mut Player,
    pub resources: &'static BeyondAssets,
    writer: &'logic mut (dyn AsyncWrite + Unpin + Send),
    pub client_seq_id: u64,
    server_seq_id: &'logic mut u64,
}

impl<'logic> NetContext<'logic> {
    pub async fn send<T: NetMessage>(&mut self, message: T) -> std::io::Result<()> {
        self.send_packet(message, true).await
    }

    pub async fn notify<T: NetMessage>(&mut self, message: T) -> std::io::Result<()> {
        self.send_packet(message, false).await
    }

    async fn send_packet<T: NetMessage>(
        &mut self,
        message: T,
        is_response: bool,
    ) -> std::io::Result<()> {
        let body = message.encode_to_vec();

        let head = CsHead {
            msgid: T::CMD_ID,
            up_seqid: self.client_seq_id,
            down_seqid: *self.server_seq_id,
        };

        let head_bytes = head.encode_to_vec();

        self.writer.write_u8(head_bytes.len() as u8).await?;
        self.writer.write_u16_le(body.len() as u16).await?;
        self.writer.write_all(&head_bytes).await?;
        self.writer.write_all(&body).await?;
        self.writer.flush().await?;

        if is_response {
            *self.server_seq_id += 1;
        }

        Ok(())
    }
}

pub async fn handle_connection(
    mut socket: TcpStream,
    assets: &'static BeyondAssets,
) -> anyhow::Result<()> {
    let mut player = Player::new(assets, "0".to_string());
    let mut server_seq_id = 0u64;

    let (mut reader, mut writer) = socket.split();

    loop {
        let head_size = reader.read_u8().await?;
        let body_size = reader.read_u16_le().await?;

        let mut head_buf = vec![0u8; head_size as usize];
        reader.read_exact(&mut head_buf).await?;

        let mut body_buf = vec![0u8; body_size as usize];
        reader.read_exact(&mut body_buf).await?;

        let head = CsHead::decode(&head_buf[..])?;

        let mut ctx = NetContext {
            player: &mut player,
            resources: assets,
            writer: &mut writer,
            client_seq_id: head.up_seqid,
            server_seq_id: &mut server_seq_id,
        };

        crate::handler::handle_command(&mut ctx, head.msgid, body_buf).await?;
    }
}
