use perlica_logic::character::char_bag::CharBag;
use perlica_logic::player::WorldState;
use std::collections::{HashMap, HashSet};

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum LoadingState {
    Pending,
    Complete,
}

pub struct Player {
    pub uid: String,
    pub loading_state: LoadingState,
    pub char_bag: CharBag,
    pub world: WorldState,
    pub bitsets: HashMap<u32, HashSet<u32>>,
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
    pub fn on_login(&mut self, uid: String) {
        self.uid = uid;
    }
}
