use crate::player::Player;
use perlica_proto::{CsHead, NetMessage, prost::Message};
use tokio::sync::mpsc;

pub struct NetContext<'a> {
    pub player: &'a mut Player,
    pub client_seq_id: u64,
    outbound: &'a mpsc::Sender<Vec<u8>>,
    server_seq_id: &'a mut u64,
}

impl<'a> NetContext<'a> {
    pub fn new(
        player: &'a mut Player,
        outbound: &'a mpsc::Sender<Vec<u8>>,
        client_seq_id: u64,
        server_seq_id: &'a mut u64,
    ) -> Self {
        Self {
            player,
            outbound,
            client_seq_id,
            server_seq_id,
        }
    }

    pub async fn send<T: NetMessage>(&mut self, message: T) -> std::io::Result<()> {
        self.write_frame(message, true).await
    }

    pub async fn notify<T: NetMessage>(&mut self, message: T) -> std::io::Result<()> {
        self.write_frame(message, false).await
    }

    async fn write_frame<T: NetMessage>(
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

        let mut frame = Vec::with_capacity(3 + head_bytes.len() + body.len());
        frame.push(head_bytes.len() as u8);
        frame.extend_from_slice(&(body.len() as u16).to_le_bytes());
        frame.extend_from_slice(&head_bytes);
        frame.extend_from_slice(&body);

        if is_response {
            *self.server_seq_id += 1;
        }

        self.outbound
            .send(frame)
            .await
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "write task closed"))
    }
}
