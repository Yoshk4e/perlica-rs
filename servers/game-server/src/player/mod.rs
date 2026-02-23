pub mod char_bag;

use config::BeyondAssets;
use perlica_logic::character::char_bag::CharBag;
use tracing::debug;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy)]
pub enum LoadingState {
    Login,
    ScLogin,
    CharBagSync,
    EnterScene,
    Complete,
}

pub struct Player {
    pub uid: String,
    pub loading_state: LoadingState,
    pub char_bag: CharBag,
    pub resources: &'static BeyondAssets,
}

impl Player {
    pub fn new(resources: &'static BeyondAssets, uid: String) -> Self {
        Self {
            uid: uid.to_string(),
            loading_state: LoadingState::Login,
            char_bag: CharBag::new_with_starter(resources, &uid).unwrap_or_else(|_| CharBag::new()),
            resources,
        }
    }

    pub fn on_login(&mut self, uid: String) {
        self.uid = uid;
        self.loading_state = LoadingState::ScLogin;
        debug!("Player logged in, state now ScLogin");
    }

    pub fn advance_state(&mut self) {
        let old = self.loading_state;
        self.loading_state = match self.loading_state {
            LoadingState::Login => LoadingState::ScLogin,
            LoadingState::ScLogin => LoadingState::CharBagSync,
            LoadingState::CharBagSync => LoadingState::EnterScene,
            LoadingState::EnterScene => LoadingState::Complete,
            LoadingState::Complete => LoadingState::Complete,
        };
        debug!("Loading state: {:?} -> {:?}", old, self.loading_state);
    }
}
