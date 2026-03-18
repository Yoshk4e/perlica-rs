use tokio::sync::mpsc;

/// Events the server can push into a player's logic loop from outside the session.
///
/// Any system that needs to reach a connected player without going through a
/// request/response cycle (scene broadcasts, world events, GM actions, etc.)
/// sends a [`Notification`] through the player's [`PlayerHandle`].
///
/// New variants are added here as world systems come online.
#[derive(Debug, Clone)]
pub enum Notification {
    // Placeholder — variants added as systems are implemented, e.g.:
    //   BroadcastMove(Vec<MoveInfo>)
    //   SceneEvent(SceneEventPayload)
    //   Kick(String)
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

    /// Non-blocking variant of [`notify`](Self::notify).
    ///
    /// Returns `false` if the channel is full or closed. Prefer this when
    /// holding a lock (e.g. during a registry broadcast) to avoid blocking the
    /// lock holder.
    pub fn try_notify(&self, n: Notification) -> bool {
        self.tx.try_send(n).is_ok()
    }

    /// Returns `true` if the player's session has ended and the channel is closed.
    pub fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }
}
