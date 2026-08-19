use perlica_muip::GmResponse;
use tokio::sync::{mpsc, oneshot};

/// Out-of-band events pushed into a player's logic loop (GM commands, future world events).
#[derive(Debug)]
pub enum Notification {
    MuipCommand {
        command: String,
        respond_to: oneshot::Sender<MuipResult>,
    },
    /// Sent to a live session when a new connection logs in with the same UID
    /// (i.e. the client reconnected before the server noticed the old TCP
    /// connection was dead). The recipient flushes its state to the DB,
    /// unregisters, and terminates, then signals `respond_to` so the new
    /// connection knows it's safe to proceed without racing the old one.
    TakeOver { respond_to: oneshot::Sender<()> },
}

#[derive(Debug)]
pub struct MuipResult {
    pub response: GmResponse,
    pub disconnect: bool,
}

/// Cloneable handle for pushing notifications into a player's logic loop.
#[derive(Clone, Debug)]
pub struct PlayerHandle {
    tx: mpsc::Sender<Notification>,
}

#[allow(dead_code)]
impl PlayerHandle {
    pub fn new(tx: mpsc::Sender<Notification>) -> Self {
        Self { tx }
    }

    /// Returns `false` if the session has ended.
    pub async fn notify(&self, n: Notification) -> bool {
        self.tx.send(n).await.is_ok()
    }

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

    /// Asks the session behind this handle to flush, unregister, and close,
    /// then waits for it to confirm. Returns once it's safe for the caller to
    /// register a replacement session for the same UID.
    ///
    /// Returns `false` if the old session was already gone (channel closed
    /// before or during the request) — in that case there's nothing to wait
    /// for and the caller can proceed immediately.
    pub async fn request_takeover(&self) -> bool {
        let (tx, rx) = oneshot::channel();
        if self
            .tx
            .send(Notification::TakeOver { respond_to: tx })
            .await
            .is_err()
        {
            return false;
        }
        rx.await.is_ok()
    }

    pub fn try_notify(&self, n: Notification) -> bool {
        self.tx.try_send(n).is_ok()
    }

    pub fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }
}
