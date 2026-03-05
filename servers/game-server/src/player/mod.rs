use perlica_logic::character::char_bag::CharBag;
use perlica_logic::player::WorldState;
use std::collections::{HashMap, HashSet};
use tracing::debug;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
pub enum LoadingState {
    Login,
    ScLogin,
    CharBagSync,
    UnlockSync,
    FactorySync,
    EnterScene,
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
            loading_state: LoadingState::Login,
            char_bag: CharBag::default(),
            world: WorldState::default(),
            bitsets: HashMap::new(),
        }
    }
}

impl Player {
    pub fn on_login(&mut self, uid: String) {
        self.uid = uid;
        self.loading_state = LoadingState::ScLogin;
        debug!(uid = %self.uid, "player login");
    }

    pub fn advance_state(&mut self) {
        let prev = self.loading_state;
        self.loading_state = match self.loading_state {
            LoadingState::Login => LoadingState::ScLogin,
            LoadingState::ScLogin => LoadingState::CharBagSync,
            LoadingState::CharBagSync => LoadingState::UnlockSync,
            LoadingState::UnlockSync => LoadingState::FactorySync,
            LoadingState::FactorySync => LoadingState::EnterScene,
            LoadingState::EnterScene => LoadingState::Complete,
            LoadingState::Complete => LoadingState::Complete,
        };
        debug!(prev = ?prev, next = ?self.loading_state, "state");
    }
}
