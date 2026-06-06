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
        self.sessions
            .write()
            .expect("SessionRegistry lock poisoned")
            .insert(uid, handle);
    }

    pub fn unregister(&self, uid: &str) {
        self.sessions
            .write()
            .expect("SessionRegistry lock poisoned")
            .remove(uid);
    }

    #[allow(dead_code)]
    pub fn get(&self, uid: &str) -> Option<PlayerHandle> {
        self.sessions
            .read()
            .expect("SessionRegistry lock poisoned")
            .get(uid)
            .cloned()
    }

    pub fn online(&self) -> usize {
        self.sessions
            .read()
            .expect("SessionRegistry lock poisoned")
            .len()
    }

    pub fn list_uids(&self) -> Vec<String> {
        let mut players: Vec<String> = self
            .sessions
            .read()
            .expect("SessionRegistry lock poisoned")
            .keys()
            .cloned()
            .collect();
        players.sort();
        players
    }

    #[allow(dead_code)]
    pub fn broadcast<F>(&self, mut build: F)
    where
        F: FnMut() -> Notification,
    {
        // Collect handles while holding the lock
        let handles: Vec<PlayerHandle> = self
            .sessions
            .read()
            .expect("SessionRegistry lock poisoned")
            .values()
            .cloned()
            .collect();
        for handle in handles {
            handle.try_notify(build());
        }
    }
}
