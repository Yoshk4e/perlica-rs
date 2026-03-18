//! Player module - manages all player-related state and logic.
//!
//! This module defines the central `Player` struct which holds all mutable
//! game state for a connected player. The player state is the authoritative
//! source of truth for:
//!
//! - **Identity**: Player UID and account information
//! - **Loading State**: Tracks the login/loading sequence progress
//! - **Characters**: All owned characters, team configurations, and the
//!   weapon depot embedded in [`CharBag`] (`CharBag`)
//! - **World State**: Current scene, position, player level (`WorldState`)
//! - **Movement**: Current position and rotation in the world
//! - **Scene**: Scene management state (current scene, loading state)
//! - **Entities**: Runtime entity tracking for the current scene
//! - **Bitsets**: Various game state flags (items found, areas visited, etc.)
//! - **Weapons**: Weapon depot with all weapon instances
//!
//! # Architecture Notes
//!
//! ## State Persistence
//!
//! Player state is persisted to the database on disconnect and loaded on login.
//! The `PlayerDb` module handles serialization/deserialization via bincode.
//!
//! ## State vs Runtime
//!
//! Some state is persistent (saved to DB):
//! - `char_bag`: Characters, teams, and weapon depot
//! - `world`: Position, scene, level
//! - `bitsets`: Game progress flags
//! - `scene.checkpoint`: Last activated repatriate / rest point
//! - `scene.current_revival_mode`: Active respawn mode
//!
//! Some state is runtime-only (cleared on disconnect):
//! - `scene`: Scene loading state, checkpoint info
//! - `entities`: Active entities in current scene
//! - `movement`: Cached movement state
//!
//! ## Player Lifecycle
//!
//! ```text
//! [Connect] -> [Login Request] -> [Load/Create Player] -> [Run Login Sequence]
//!                                                          |
//!                                                          v
//!                                                     [Active Gameplay]
//!                                                          |
//!                                                          v
//! [Disconnect] -> [Save to DB] -> [Cleanup]
//! ```

use perlica_logic::{
    bitset::BitsetManager, character::char_bag::CharBag, entity::EntityManager,
    movement::MovementManager, player::WorldState, scene::SceneManager,
};

/// Tracks the player's loading state during the login sequence.
///
/// The login sequence is multi-step and requires the client to acknowledge
/// each stage before proceeding. This enum tracks progress.
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum LoadingState {
    /// Login is in progress, waiting for client to finish loading
    Pending,
    /// Login complete, player is fully in-game
    Complete,
}

/// The main player struct containing all player state.
///
/// This is the central hub for all player-related data. All handlers
/// receive a mutable reference to this struct via `NetContext.player`.
///
/// # Thread Safety
///
/// Player state is not thread-safe. All access must go through the
/// session's main loop which holds exclusive mutable access.
///
/// # Example Usage
///
/// ```rust
/// // In a handler function:
/// pub async fn on_cs_some_command(ctx: &mut NetContext<'_>, req: CsSomeCommand) -> ScSomeCommand {
///     // Access player data
///     let uid = &ctx.player.uid;
///     let team = &ctx.player.char_bag.teams[ctx.player.char_bag.meta.curr_team_index as usize];
///
///     // Modify player data
///     ctx.player.world.pos_x = req.target_x;
///
///     // Return response
///     ScSomeCommand { ... }
/// }
/// ```
pub struct Player {
    /// Unique identifier for this player.
    /// Set during login from the `CsLogin.uid` field.
    /// Empty string until login completes.
    pub uid: String,

    /// Tracks the loading state during login sequence.
    /// Set to `Pending` after `CsLogin` is processed.
    /// Set to `Complete` after the full login sequence finishes.
    pub loading_state: LoadingState,

    /// Character bag containing all characters and teams.
    /// This is the primary player progression data.
    ///
    /// See `perlica_logic::character::char_bag::CharBag` for details.
    pub char_bag: CharBag,

    /// World state including scene, position, and player level.
    /// Tracks where the player is and their progression level.
    ///
    /// See `perlica_logic::player::WorldState` for details.
    pub world: WorldState,

    /// Bitset manager for game state flags.
    /// Tracks various progress flags like items found, areas visited.
    ///
    /// See `perlica_logic::bitset::BitsetManager` for details.
    pub bitsets: BitsetManager,

    /// Movement manager tracking current position and rotation.
    /// Initialized from `WorldState` on login and synced back on disconnect.
    ///
    /// See `perlica_logic::movement::MovementManager` for details.
    pub movement: MovementManager,

    /// Scene manager handling current scene state and transitions.
    /// Tracks loading state, battle mode, checkpoints, etc.
    ///
    /// See `perlica_logic::scene::SceneManager` for details.
    pub scene: SceneManager,

    /// Entity manager for runtime scene entities.
    /// Tracks all active entities (characters, monsters, NPCs) in the current scene.
    /// Cleared on scene transitions.
    ///
    /// See `perlica_logic::entity::EntityManager` for details.
    pub entities: EntityManager,
}

impl Player {
    /// Called when a player logs in.
    ///
    /// Sets up the player identity and initializes runtime state from
    /// the persistent world state. After this call, the player is in
    /// `LoadingState::Pending` awaiting the login sequence.
    ///
    /// # Arguments
    /// * `uid` - The unique identifier from the login request
    pub fn on_login(&mut self, uid: String) {
        self.uid = uid;
        self.loading_state = LoadingState::Pending;

        // Initialize movement from saved world state
        self.movement = MovementManager::from_world(&self.world);

        // Initialize scene from saved world state
        // Note: scene_id will be set properly during login sequence
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
        }
    }
}

// Implement the helper trait for easier access to character data
impl Player {
    /// Get a character by object ID (convenience method).
    ///
    /// # Arguments
    /// * `objid` - The character's object ID (1-indexed)
    ///
    /// # Returns
    /// * `Option<&Char>` - Reference to the character if found
    pub fn get_char_by_objid(
        &self,
        objid: u64,
    ) -> Option<&perlica_logic::character::char_bag::Char> {
        self.char_bag.get_char_by_objid(objid)
    }

    /// Get a mutable character by object ID (convenience method).
    ///
    /// # Arguments
    /// * `objid` - The character's object ID (1-indexed)
    ///
    /// # Returns
    /// * `Option<&mut Char>` - Mutable reference to the character if found
    pub fn get_char_by_objid_mut(
        &mut self,
        objid: u64,
    ) -> Option<&mut perlica_logic::character::char_bag::Char> {
        self.char_bag.get_char_by_objid_mut(objid)
    }

    /// Get the current team's leader object ID.
    ///
    /// # Returns
    /// * `u64` - The leader's object ID
    pub fn get_leader_objid(&self) -> u64 {
        let team_idx = self.char_bag.meta.curr_team_index as usize;
        if let Some(team) = self.char_bag.teams.get(team_idx) {
            team.leader_index.object_id()
        } else {
            1 // Fallback to first character
        }
    }
}
