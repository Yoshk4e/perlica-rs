use crate::net::notify::{Notification, PlayerHandle};
use std::collections::HashMap;
use std::sync::RwLock;

/// Registry of all currently connected player sessions.
///
/// Acts as the server's authoritative source for which players are online and
/// provides a way to push notifications to any of them by UID. The underlying
/// map is wrapped in an [`RwLock`] so that multiple threads can look up sessions
/// concurrently while writes (login/logout) remain exclusive.
pub struct SessionRegistry {
    sessions: RwLock<HashMap<String, PlayerHandle>>,
}

impl SessionRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    /// Registers a newly authenticated player.
    pub fn register(&self, uid: String, handle: PlayerHandle) {
        self.sessions.write().unwrap().insert(uid, handle);
    }

    /// Removes a player from the registry on disconnect or logout.
    pub fn unregister(&self, uid: &str) {
        self.sessions.write().unwrap().remove(uid);
    }

    /// Returns a cloned [`PlayerHandle`] for the given UID, or `None` if the
    /// player is not online.
    #[allow(dead_code)]
    pub fn get(&self, uid: &str) -> Option<PlayerHandle> {
        self.sessions.read().unwrap().get(uid).cloned()
    }

    /// Returns the number of currently registered (online) players.
    pub fn online(&self) -> usize {
        self.sessions.read().unwrap().len()
    }

    /// Returns a sorted list of all online player UIDs.
    pub fn list_uids(&self) -> Vec<String> {
        let mut players: Vec<String> = self.sessions.read().unwrap().keys().cloned().collect();
        players.sort();
        players
    }

    /// Broadcasts a notification to every connected player.
    #[allow(dead_code)]
    pub fn broadcast<F>(&self, mut build: F)
    where
        F: FnMut() -> Notification,
    {
        let sessions = self.sessions.read().unwrap();
        for handle in sessions.values() {
            handle.try_notify(build());
        }
    }
}
