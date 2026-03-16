use perlica_logic::character::char_bag::CharBag;
use perlica_logic::player::WorldState;
use std::collections::{HashMap, HashSet};

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum LoadingState {
    Pending,
    Complete,
}

pub struct Player {
    // This is our main player struct, kinda like the central hub for all player-related stuff.
    pub uid: String,                         // Unique ID for the player, duh.
    pub loading_state: LoadingState, // Tracks if the player's data is still loading or if we're good to go.
    pub char_bag: CharBag,           // This holds all the player's characters.
    pub world: WorldState, // Where the player is in the world, their level, exp, all that jazz.
    pub bitsets: HashMap<u32, HashSet<u32>>, // Bitsets for various game states or flags. Super useful for tracking stuff, but kinda cryptic if you don't know what's what. LOL
}

impl Default for Player {
    fn default() -> Self {
        Self {
            uid: String::new(),
            loading_state: LoadingState::Pending,
            char_bag: CharBag::default(),
            world: WorldState::default(),
            bitsets: HashMap::new(),
        }
    }
}

impl Player {
    // Player methods, where the magic happens (or at least, where we try to make it happen).
    pub fn on_login(&mut self, uid: String) {
        // What happens when a player logs in. Set their UID and get ready to rumble!
        self.uid = uid;
    }
}
