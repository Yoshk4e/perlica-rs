use crate::net::notify::{Notification, PlayerHandle};
use std::collections::HashMap;
use std::sync::RwLock;

pub struct SessionRegistry {
    // This is like the bouncer at the club, keeping track of who's in and who's out. All the active player sessions live here.
    sessions: RwLock<HashMap<String, PlayerHandle>>, // A fancy thread-safe map to store all our active sessions. RwLock means multiple readers, but only one writer at a time. Smart, right?
}

impl SessionRegistry {
    pub fn new() -> Self {
        // Fresh new registry, ready to roll!
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(&self, uid: String, handle: PlayerHandle) {
        // When a player logs in, we register their session here. Welcome to the server, fam!
        self.sessions.write().unwrap().insert(uid, handle);
    }

    pub fn unregister(&self, uid: &str) {
        // Player logged out or disconnected. Time to remove them from the active list. Bye bye!
        self.sessions.write().unwrap().remove(uid);
    }

    #[allow(dead_code)]
    pub fn get(&self, uid: &str) -> Option<PlayerHandle> {
        // Need to find a specific player's session? This is how you do it. Like looking up a friend in your contacts.
        self.sessions.read().unwrap().get(uid).cloned()
    }

    pub fn online(&self) -> usize {
        // How many players are online right now? Let's get that count!
        self.sessions.read().unwrap().len()
    }

    // Delivers n to every connected player; silently skips sessions with a full or closed channel.
    #[allow(dead_code, unreachable_code)]
    pub fn broadcast(&self, n: Notification) {
        // Sending a message to EVERYONE! Like a server-wide announcement. Hope it's good news! XD
        let sessions = self.sessions.read().unwrap();
        for handle in sessions.values() {
            handle.try_notify(n.clone());
        }
    }
}
