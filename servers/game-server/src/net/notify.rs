use tokio::sync::mpsc;

// Events the server can push into a player's logic loop from outside the session.
// Anything that needs to reach a connected player without going through a request
// (scene broadcasts, world events, GM actions, etc.) goes through here.
#[derive(Debug, Clone)]
pub enum Notification {
    // Placeholder, variants are added here as world systems come online.
    //   BroadcastMove(Vec<MoveInfo>)
    //   SceneEvent(SceneEventPayload)
    //   Kick(String)
}

// A cheap, cloneable handle for pushing notifications into one player's logic loop.
// Hand one out to every system that needs to reach the player asynchronously.
#[derive(Clone, Debug)]
pub struct PlayerHandle {
    tx: mpsc::Sender<Notification>,
}

#[allow(dead_code)]
impl PlayerHandle {
    pub fn new(tx: mpsc::Sender<Notification>) -> Self {
        Self { tx }
    }

    // Returns false if the player's session has already ended.
    pub async fn notify(&self, n: Notification) -> bool {
        self.tx.send(n).await.is_ok()
    }

    // Non-blocking variant; used when holding a lock (e.g. broadcast from the registry).
    // Returns false if the channel is full or closed.
    pub fn try_notify(&self, n: Notification) -> bool {
        self.tx.try_send(n).is_ok()
    }

    pub fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }
}
