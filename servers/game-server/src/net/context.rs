use crate::player::Player;
use config::BeyondAssets;
use perlica_proto::{CsHead, NetMessage, prost::Message};
use tokio::io::{AsyncWrite, AsyncWriteExt};

pub struct NetContext<'logic> {
    pub player: &'logic mut Player,
    pub resources: &'static BeyondAssets,
    writer: &'logic mut (dyn AsyncWrite + Unpin + Send),
    pub client_seq_id: u64,
    server_seq_id: &'logic mut u64,
}

impl<'logic> NetContext<'logic> {
    pub fn new(
        player: &'logic mut Player,
        resources: &'static BeyondAssets,
        writer: &'logic mut (dyn AsyncWrite + Unpin + Send),
        client_seq_id: u64,
        server_seq_id: &'logic mut u64,
    ) -> Self {
        Self {
            player,
            resources,
            writer,
            client_seq_id,
            server_seq_id,
        }
    }

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
