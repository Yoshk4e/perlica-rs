//! Per-request context threaded through every handler.

use crate::player::Player;
use config::BeyondAssets;
use perlica_db::PlayerDb;
use perlica_proto::{CsHead, NetMessage, prost::Message};
use tokio::sync::mpsc;

/// Everything a handler needs for a single request, player state, assets, DB, and the
/// outbound channel. Created fresh per command and dropped when the handler returns.
pub struct NetContext<'a> {
    pub player: &'a mut Player,
    pub db: &'static PlayerDb,
    pub client_seq_id: u64,
    pub assets: &'static BeyondAssets,
    outbound: &'a mpsc::Sender<Vec<u8>>,
    pub server_seq_id: &'a mut u64,
}

impl<'a> NetContext<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        player: &'a mut Player,
        db: &'static PlayerDb,
        assets: &'static BeyondAssets,
        outbound: &'a mpsc::Sender<Vec<u8>>,
        client_seq_id: u64,
        server_seq_id: &'a mut u64,
    ) -> Self {
        Self {
            player,
            db,
            assets,
            outbound,
            client_seq_id,
            server_seq_id,
        }
    }

    /// Sends a direct response to the client, echoing the client's sequence ID.
    pub async fn send<T: NetMessage>(&mut self, message: T) -> std::io::Result<()> {
        self.write_frame(message, true).await
    }

    /// Sends a server-initiated notification (no matching client request).
    pub async fn notify<T: NetMessage>(&mut self, message: T) -> std::io::Result<()> {
        self.write_frame(message, false).await
    }

    /// Frames and sends a message over the outbound channel.
    ///
    /// Wire format: `[head_size: u8][body_size: u16][head][body]`
    /// Responses echo `client_seq_id`; notifications consume the next `server_seq_id`.
    async fn write_frame<T: NetMessage>(
        &mut self,
        message: T,
        is_response: bool,
    ) -> std::io::Result<()> {
        let body = message.encode_to_vec();

        let head = CsHead {
            msgid: T::CMD_ID,
            up_seqid: if is_response {
                self.client_seq_id
            } else {
                let seq = *self.server_seq_id;
                *self.server_seq_id += 1;
                seq
            },
            ..Default::default()
        };
        let head_bytes = head.encode_to_vec();

        // [head_size: u8][body_size: u16][head][body]
        let mut frame = Vec::with_capacity(3 + head_bytes.len() + body.len());
        frame.push(head_bytes.len() as u8);
        frame.extend_from_slice(&(body.len() as u16).to_le_bytes());
        frame.extend_from_slice(&head_bytes);
        frame.extend_from_slice(&body);

        self.outbound
            .send(frame)
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::BrokenPipe, e))
    }
}
