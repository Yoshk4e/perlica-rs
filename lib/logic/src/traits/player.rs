//! Top-level player-record subsystem marker.

use serde::{Serialize, de::DeserializeOwned};

use crate::bitset::BitsetManager;
use crate::character::char_bag::CharBag;
use crate::mail::MailManager;
use crate::mission::{GuideManager, MissionManager};
use crate::player::WorldState;

/// Marker for top-level subsystems that are stored inside `PlayerRecord`.
///
/// Each subsystem is `Serialize + DeserializeOwned + Default` and,
/// conceptually, can be re-created from scratch for a new player. The
/// trait gives generic save / migration / debug-dump code a single bound
/// to spell.
pub trait PlayerComponent: Serialize + DeserializeOwned + Default {
    /// Stable name used for log lines, error messages, and save-migration
    /// code.
    const NAME: &'static str;
}

impl PlayerComponent for WorldState {
    const NAME: &'static str = "world";
}
impl PlayerComponent for BitsetManager {
    const NAME: &'static str = "bitsets";
}
impl PlayerComponent for MissionManager {
    const NAME: &'static str = "missions";
}
impl PlayerComponent for GuideManager {
    const NAME: &'static str = "guides";
}
impl PlayerComponent for MailManager {
    const NAME: &'static str = "mail";
}
impl PlayerComponent for CharBag {
    const NAME: &'static str = "char_bag";
}
