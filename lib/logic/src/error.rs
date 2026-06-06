use crate::item::{EquipInstId, GemInstId, WeaponInstId};
use config::item::ItemDepotType;

#[derive(Debug, thiserror::Error)]
pub enum LogicError {
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    InvalidOperation(String),
    #[error("Insufficient {item_id}: have {have}, need {need}")]
    Insufficient {
        item_id: String,
        have: u32,
        need: u32,
    },
    #[error("Weapon {0} not found")]
    WeaponNotFound(WeaponInstId),
    #[error("Weapon {0} is currently equipped and cannot be removed")]
    WeaponEquipped(WeaponInstId),
    #[error("Weapon {0} is already equipped to character {1}")]
    WeaponAlreadyEquipped(WeaponInstId, u64),
    #[error("Weapon {0} is locked and cannot be removed")]
    WeaponLocked(WeaponInstId),
    #[error("Cannot use weapon {0} as its own fodder")]
    WeaponFodderSelf(WeaponInstId),
    #[error("Fodder weapon {0} not found")]
    WeaponFodderNotFound(WeaponInstId),
    #[error("Fodder weapon {0} is locked and cannot be used")]
    WeaponFodderLocked(WeaponInstId),
    #[error("Fodder weapon {0} is equipped and cannot be used")]
    WeaponFodderEquipped(WeaponInstId),
    #[error("Weapon {0} is already at max breakthrough level")]
    WeaponMaxBreakthrough(WeaponInstId),
    #[error("Weapon {id} level {current} is below required {required} for next breakthrough")]
    WeaponBreakthroughLevelTooLow {
        id: WeaponInstId,
        current: u64,
        required: u64,
    },
    #[error("Weapon {0} is locked and cannot be refined")]
    WeaponRefineTargetLocked(WeaponInstId),
    #[error("Refinement requires the same weapon template as the target")]
    WeaponRefineTypeMismatch,
    #[error("Weapon {0} is already at max refinement level")]
    WeaponRefineMaxLevel(WeaponInstId),
    #[error("Weapon {0} is locked and cannot be modified")]
    WeaponModifyLocked(WeaponInstId),
    #[error("Weapon {0} has no attached gem")]
    WeaponNoAttachedGem(WeaponInstId),
    #[error("Gem {0} not found")]
    GemNotFound(GemInstId),
    #[error("Gem {0} is socketed and cannot be removed")]
    GemSocketed(GemInstId),
    #[error("Gem {0} is locked and cannot be removed")]
    GemLocked(GemInstId),
    #[error("Equip piece {0} not found")]
    EquipNotFound(EquipInstId),
    #[error("Equip piece {0} is currently equipped and cannot be removed")]
    EquipEquipped(EquipInstId),
    #[error("Equip piece {0} is locked and cannot be removed")]
    EquipLocked(EquipInstId),
    #[error("Equip piece {0} is already equipped to character {1}")]
    EquipAlreadyEquipped(EquipInstId, u64),
    #[error("Depot {0:?} is instanced, expected stackable")]
    DepotInstanced(ItemDepotType),
    #[error(transparent)]
    Config(#[from] config::ConfigError),
}

pub type Result<T> = std::result::Result<T, LogicError>;
