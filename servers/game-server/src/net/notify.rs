use perlica_muip::GmResponse;
use tokio::sync::{mpsc, oneshot};

/// Out-of-band events pushed into a player's logic loop (GM commands, future world events).
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

    pub fn try_notify(&self, n: Notification) -> bool {
        self.tx.try_send(n).is_ok()
    }

    pub fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }
}
