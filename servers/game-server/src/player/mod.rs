use config::BeyondAssets;
use perlica_logic::character::char_bag::CharBag;
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
    pub resources: &'static BeyondAssets,
}

impl Player {
    pub fn new(resources: &'static BeyondAssets, uid: String) -> Self {
        Self {
            uid: uid.clone(),
            loading_state: LoadingState::Login,
            char_bag: CharBag::new_with_starter(resources, &uid).unwrap_or_else(|_| CharBag::new()),
            resources,
        }
    }

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
