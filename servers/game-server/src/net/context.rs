//! Network context for handling client requests.
//!
//! This module defines `NetContext`, the primary interface for request handlers
//! to interact with the player state, database, and network.
//!
//! # Purpose
//!
//! `NetContext` bundles together everything a handler needs:
//! - **Player state**: Mutable reference to modify game state
//! - **Database**: For loading/saving player data
//! - **Assets**: Read-only game configuration data
//! - **Network**: Channels to send responses and notifications
//! - **Sequence tracking**: Client/server sequence IDs
//!
//! # Usage
//!
//! Handlers receive a `NetContext` as their first argument:
//!
//! ```rust
//! pub async fn on_cs_some_command(
//!     ctx: &mut NetContext<'_>,
//!     req: CsSomeCommand,
//! ) -> ScSomeCommand {
//!     // Access player data
//!     let uid = &ctx.player.uid;
//!
//!     // Access game assets
//!     let character = ctx.assets.characters.get("char_001");
//!
//!     // Modify player state
//!     ctx.player.world.pos_x = 100.0;
//!
//!     // Send notification (fire-and-forget)
//!     ctx.notify(ScSomeNotification { ... }).await?;
//!
//!     // Return response (will be sent automatically)
//!     ScSomeCommand { ... }
//! }
//! ```
//!
//! # Lifetime Notes
//!
//! The `NetContext<'a>` lifetime is tied to:
//! - `'a` - The duration of handling a single request
//!
//! This ensures handlers can't hold references to player state
//! across multiple requests, preventing stale data issues.

use crate::player::Player;
use config::BeyondAssets;
use perlica_db::PlayerDb;
use perlica_proto::{CsHead, NetMessage, prost::Message};
use tokio::sync::mpsc;

/// Network context for handling a single client request.
///
/// This struct is created for each incoming command and provides access
/// to all the resources a handler might need. It's designed to be the
/// single "context" parameter passed to all handlers.
///
/// # Fields
///
/// - `player`: Mutable reference to the player's game state. Handlers
///   can freely modify this during request processing.
/// - `db`: Reference to the database for loading/saving player data.
/// - `assets`: Read-only game configuration (characters, weapons, etc.).
/// - `client_seq_id`: The sequence ID from the client's request header.
/// - `outbound`: Channel to send responses back to the client.
/// - `server_seq_id`: Counter for assigning sequence IDs to responses.
///
/// # Thread Safety
///
/// `NetContext` is not thread-safe. It must be used within a single
/// async task. The mutable player reference ensures exclusive access
/// during request handling.
pub struct NetContext<'a> {
    /// Mutable reference to the player's game state.
    /// Handlers can read and write this freely during request processing.
    /// Changes are persisted to the database when the session ends.
    pub player: &'a mut Player,

    /// Reference to the database for loading/saving player data.
    /// Used during login to load existing players and during
    /// disconnect to save state.
    pub db: &'static PlayerDb,

    /// The sequence ID from the client's request header.
    /// Used for ordering and acknowledgment. Responses echo this
    /// ID so the client can match requests to responses.
    pub client_seq_id: u64,

    /// Reference to game configuration assets.
    /// Contains all static game data like character stats, weapon configs,
    /// enemy spawns, etc. This is read-only and shared across all sessions.
    pub assets: &'static BeyondAssets,

    /// Channel sender for outbound messages.
    /// Used internally by `send()` and `notify()` methods.
    /// Messages are framed and written to the socket by a background task.
    outbound: &'a mpsc::Sender<Vec<u8>>,

    /// Counter for server-initiated messages.
    /// Each response or notification increments this counter.
    /// The client uses this to detect missing or out-of-order messages.
    pub server_seq_id: &'a mut u64,
}

impl<'a> NetContext<'a> {
    /// Creates a new network context for handling a request.
    ///
    /// This is called by the session handler for each incoming command.
    /// The context bundles together all the resources the handler might need.
    ///
    /// # Arguments
    /// * `player` - Mutable reference to the player's state
    /// * `db` - Static reference to the database
    /// * `assets` - Static reference to game configuration
    /// * `outbound` - Channel sender for responses
    /// * `client_seq_id` - Sequence ID from the client's request
    /// * `server_seq_id` - Mutable reference to the server's sequence counter
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

    /// Sends a response message to the client.
    ///
    /// Response messages are sent as replies to client requests and
    /// include the client's sequence ID for acknowledgment tracking.
    /// Use this for handlers that return data directly to the client.
    ///
    /// # Arguments
    /// * `message` - The message to send (must implement `NetMessage`)
    ///
    /// # Returns
    /// * `std::io::Result<()>` - Ok on success, Err on send failure
    ///
    /// # Example
    /// ```rust
    /// let response = ScSomeCommand { ... };
    /// ctx.send(response).await?;
    /// ```
    pub async fn send<T: NetMessage>(&mut self, message: T) -> std::io::Result<()> {
        self.write_frame(message, true).await
    }

    /// Sends a notification message to the client.
    ///
    /// Notifications are server-initiated messages that don't directly
    /// respond to a request. They're used for:
    /// - Game state updates (character spawned, item dropped)
    /// - Social notifications (friend came online)
    /// - System messages (server maintenance warning)
    ///
    /// # Arguments
    /// * `message` - The notification to send (must implement `NetMessage`)
    ///
    /// # Returns
    /// * `std::io::Result<()>` - Ok on success, Err on send failure
    ///
    /// # Example
    /// ```rust
    /// ctx.notify(ScObjectEnterView { ... }).await?;
    /// ctx.notify(ScCharSyncStatus { ... }).await?;
    /// ```
    pub async fn notify<T: NetMessage>(&mut self, message: T) -> std::io::Result<()> {
        self.write_frame(message, false).await
    }

    /// Internal method to frame and write a message to the outbound channel.
    ///
    /// This handles the low-level framing protocol:
    /// ```text
    /// [head_size: u8][body_size: u16][head: bytes][body: bytes]
    /// ```
    ///
    /// # Arguments
    /// * `message` - The message to encode and send
    /// * `is_response` - Whether this is a response (true) or notification (false)
    ///    - Responses echo the client's sequence ID
    ///    - Notifications use the server's next sequence ID
    ///
    /// # Returns
    /// * `std::io::Result<()>` - Ok on success, Err on failure
    async fn write_frame<T: NetMessage>(
        &mut self,
        message: T,
        is_response: bool,
    ) -> std::io::Result<()> {
        // Encode the message body
        let body = message.encode_to_vec();

        // Build the header
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

        // Build the frame: [head_size: u8][body_size: u16][head][body]
        let mut frame = Vec::with_capacity(3 + head_bytes.len() + body.len());
        frame.push(head_bytes.len() as u8);
        frame.extend_from_slice(&(body.len() as u16).to_le_bytes());
        frame.extend_from_slice(&head_bytes);
        frame.extend_from_slice(&body);

        // Send to the write loop
        self.outbound
            .send(frame)
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::BrokenPipe, e))
    }
}
