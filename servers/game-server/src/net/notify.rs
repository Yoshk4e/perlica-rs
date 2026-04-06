use perlica_muip::GmResponse;
use tokio::sync::{mpsc, oneshot};

/// Events the server can push into a player's logic loop from outside the session.
///
/// Any system that needs to reach a connected player without going through a
/// request/response cycle (scene broadcasts, world events, GM actions, etc.)
/// sends a [`Notification`] through the player's [`PlayerHandle`].
///
/// MUIP uses this path to execute live GM mutations against the in-memory
/// player state
/// New variants are added here as world systems come online.
#[derive(Debug)]
pub enum Notification {
    MuipCommand {
        command: String,
        respond_to: oneshot::Sender<MuipResult>,
    },
}

#[derive(Debug)]
pub struct MuipResult {
    pub response: GmResponse,
    pub disconnect: bool,
}

/// A cheap, cloneable handle for pushing [`Notification`]s into one player's
/// logic loop.
///
/// Hand a `PlayerHandle` out to every system that needs to reach the player
/// asynchronously. It wraps the sending half of the player's notification
/// channel and is safe to clone and send across threads.
#[derive(Clone, Debug)]
pub struct PlayerHandle {
    tx: mpsc::Sender<Notification>,
}

#[allow(dead_code)]
impl PlayerHandle {
    /// Creates a new handle wrapping `tx`.
    pub fn new(tx: mpsc::Sender<Notification>) -> Self {
        Self { tx }
    }

    /// Sends a notification to the player's logic loop.
    ///
    /// Returns `false` if the player's session has already ended and the
    /// channel is closed.
    pub async fn notify(&self, n: Notification) -> bool {
        self.tx.send(n).await.is_ok()
    }

    /// Executes a live MUIP GM command against this player's session.
    pub async fn exec_muip(&self, command: String) -> Option<MuipResult> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Notification::MuipCommand {
                command,
                respond_to: tx,
            })
            .await
            .ok()?;
        rx.await.ok()
    }

    /// Non-blocking variant of [`notify`](Self::notify).
    pub fn try_notify(&self, n: Notification) -> bool {
        self.tx.try_send(n).is_ok()
    }

    /// Returns `true` if the player's session has ended and the channel is closed.
    pub fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }
}
