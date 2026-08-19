//! Per-request context threaded through every handler.

use crate::net::crypto::CipherState;
use crate::net::registry::SessionRegistry;
use crate::player::Player;
use chacha20::ChaCha20;
use config::BeyondAssets;
use perlica_db::PlayerDb;
use perlica_proto::{Code, CsHead, NetMessage, ScLogin, ScNtfErrorCode, prost::Message};
use tokio::sync::mpsc;
use tracing::error;

/// A frame handed off from the logic loop to the write loop. `bytes` is the fully
/// serialized frame (`[u8 head_size][u16 body_size][head][body]`) with the
/// head‖body region STILL in plaintext — the write loop applies the outbound
/// keystream (if any) in place before writing.
///
/// `arm_cipher` is `Some(cipher)` on exactly one frame per session: the SC_LOGIN
/// frame. The write loop writes SC_LOGIN plaintext, then activates the supplied
/// cipher so the next outbound frame is encrypted.
pub struct OutboundFrame {
    pub bytes: Vec<u8>,
    pub arm_cipher: Option<ChaCha20>,
}

/// Thin wrapper over the outbound `mpsc::Sender` so handlers can't accidentally
/// bypass the framing/encryption plumbing.
#[derive(Clone)]
pub struct OutboundTx {
    inner: mpsc::Sender<OutboundFrame>,
}

impl OutboundTx {
    pub fn new(inner: mpsc::Sender<OutboundFrame>) -> Self {
        Self { inner }
    }

    pub async fn send(&self, frame: OutboundFrame) -> std::io::Result<()> {
        self.inner
            .send(frame)
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::BrokenPipe, e))
    }
}

/// Everything a handler needs for a single request, player state, assets, DB, and the
/// outbound channel. Created fresh per command and dropped when the handler returns.
pub struct NetContext<'a> {
    pub player: &'a mut Player,
    pub db: &'static PlayerDb,
    pub client_seq_id: u64,
    pub assets: &'static BeyondAssets,
    pub registry: &'static SessionRegistry,
    outbound: &'a OutboundTx,
    pub server_seq_id: &'a mut u64,
    /// Set by the login handler after it generates fresh session keys.
    /// - `outbound_arm`: consumed by the next `send`/`notify` that ships an
    ///   `ScLogin`; passed to the write loop to activate outbound encryption
    ///   after that plaintext frame.
    /// - `inbound_arm`: drained by the session read loop after the current
    ///   handler returns; installed on the inbound cipher so the next inbound
    ///   frame is decrypted.
    outbound_arm: Option<ChaCha20>,
    inbound_arm: Option<ChaCha20>,
}

impl<'a> NetContext<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        player: &'a mut Player,
        db: &'static PlayerDb,
        assets: &'static BeyondAssets,
        outbound: &'a OutboundTx,
        client_seq_id: u64,
        server_seq_id: &'a mut u64,
        registry: &'static SessionRegistry,
    ) -> Self {
        Self {
            player,
            db,
            client_seq_id,
            assets,
            registry,
            outbound,
            server_seq_id,
            outbound_arm: None,
            inbound_arm: None,
        }
    }

    /// Called by the login handler to stage a fresh ChaCha20 cipher for each
    /// direction. Both instances are keyed from the same `SessionKeys` (see
    /// `crypto::SessionKeys::cipher`) but stay independent so their block
    /// counters never collide.
    pub fn arm_session_ciphers(&mut self, outbound: ChaCha20, inbound: ChaCha20) {
        self.outbound_arm = Some(outbound);
        self.inbound_arm = Some(inbound);
    }

    /// Drained by the session read loop after the current handler returns.
    pub fn take_inbound_cipher(&mut self) -> Option<ChaCha20> {
        self.inbound_arm.take()
    }

    /// Sends a direct response to the client, echoing the client's sequence ID.
    pub async fn send<T: NetMessage>(&mut self, message: T) -> std::io::Result<()> {
        self.write_frame(message, true)
            .await
            .inspect_err(|e| error!("Failed to send response {:?}", e))
    }

    /// Sends a server-initiated notification (no matching client request).
    pub async fn notify<T: NetMessage>(&mut self, message: T) -> std::io::Result<()> {
        self.write_frame(message, false)
            .await
            .inspect_err(|e| error!("Failed to send notification {:?}", e))
    }

    /// Sends an error notification to the client using `SC_NTF_ERROR_CODE`.
    ///
    /// This should be called when a handler rejects a request due to
    /// validation failure (bad objid, unowned character, invalid input, etc.)
    pub async fn send_error(&mut self, code: Code, details: impl Into<String>) {
        let _ = self
            .notify(ScNtfErrorCode {
                error_code: code as i32,
                details: details.into(),
            })
            .await;
    }

    /// Frames and sends a message over the outbound channel.
    ///
    /// Wire format: `[head_size: u8][body_size: u16][head][body]`
    /// Responses echo `client_seq_id`; notifications consume the next `server_seq_id`.
    ///
    /// If this message is `ScLogin`, the outbound cipher armed via
    /// `arm_session_ciphers` is attached to the frame so the write loop
    /// activates encryption AFTER shipping this (last plaintext) frame.
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

        // The only message that arms outbound encryption is SC_LOGIN — matched
        // by CMD_ID at compile time, no string compare, no allocation.
        let arm_cipher = if T::CMD_ID == <ScLogin as NetMessage>::CMD_ID {
            self.outbound_arm.take()
        } else {
            None
        };

        self.outbound
            .send(OutboundFrame {
                bytes: frame,
                arm_cipher,
            })
            .await
    }
}
