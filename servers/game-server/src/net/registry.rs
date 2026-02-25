use crate::net::notify::{Notification, PlayerHandle};
use std::collections::HashMap;
use std::sync::RwLock;

pub struct SessionRegistry {
    sessions: RwLock<HashMap<String, PlayerHandle>>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(&self, uid: String, handle: PlayerHandle) {
        self.sessions.write().unwrap().insert(uid, handle);
    }

    pub fn unregister(&self, uid: &str) {
        self.sessions.write().unwrap().remove(uid);
    }

    #[allow(dead_code)]
    pub fn get(&self, uid: &str) -> Option<PlayerHandle> {
        self.sessions.read().unwrap().get(uid).cloned()
    }

    pub fn online(&self) -> usize {
        self.sessions.read().unwrap().len()
    }

    // Delivers n to every connected player; silently skips sessions with a full or closed channel.
    #[allow(dead_code, unreachable_code)]
    pub fn broadcast(&self, n: Notification) {
        let sessions = self.sessions.read().unwrap();
        for handle in sessions.values() {
            handle.try_notify(n.clone());
        }
    }
}
