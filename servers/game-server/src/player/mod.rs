//! Central player state. Handlers receive a `&mut Player` through `NetContext`.
//!
//! Persisted to DB on disconnect: `char_bag`, `world`, `bitsets`,
//! `scene.checkpoint`, `scene.current_revival_mode`, `missions`, `guides`.
//! Everything else (`movement`, `entities`, scene loading state) is runtime-only.

use perlica_logic::{
    bitset::BitsetManager,
    character::char_bag::CharBag,
    entity::EntityManager,
    mission::{GuideManager, MissionManager},
    movement::MovementManager,
    player::WorldState,
    scene::SceneManager,
};

/// Whether the login sequence has finished pushing initial state to the client.
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
    pub bitsets: BitsetManager,
    pub movement: MovementManager,
    pub scene: SceneManager,
    pub entities: EntityManager,
    pub missions: MissionManager,
    pub guides: GuideManager,
}

impl Player {
    pub fn on_login(&mut self, uid: String) {
        self.uid = uid;
        self.loading_state = LoadingState::Pending;
        self.movement = MovementManager::from_world(&self.world);
        // scene_id will be resolved properly during the login sequence
        self.scene.current_scene = self.world.last_scene.clone();
    }
}

impl Default for Player {
    fn default() -> Self {
        let world = WorldState::default();
        let movement = MovementManager::from_world(&world);
        let scene = SceneManager::new();

        Self {
            uid: String::new(),
            loading_state: LoadingState::Pending,
            char_bag: CharBag::default(),
            world,
            bitsets: BitsetManager::new(),
            movement,
            scene,
            entities: EntityManager::new(),
            missions: MissionManager::default(),
            guides: GuideManager::default(),
        }
    }
}

impl Player {
    #[allow(dead_code)]
    pub fn get_char_by_objid(
        &self,
        objid: u64,
    ) -> Option<&perlica_logic::character::char_bag::Char> {
        self.char_bag.get_char_by_objid(objid)
    }

    #[allow(dead_code)]
    pub fn get_char_by_objid_mut(
        &mut self,
        objid: u64,
    ) -> Option<&mut perlica_logic::character::char_bag::Char> {
        self.char_bag.get_char_by_objid_mut(objid)
    }

    pub fn get_leader_objid(&self) -> u64 {
        let team_idx = self.char_bag.meta.curr_team_index as usize;
        if let Some(team) = self.char_bag.teams.get(team_idx) {
            team.leader_index.object_id()
        } else {
            1 // fallback to first character
        }
    }
}
