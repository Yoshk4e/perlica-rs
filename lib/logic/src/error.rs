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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_display() {
        let err = LogicError::NotFound("test item".to_string());
        assert_eq!(err.to_string(), "test item");
    }

    #[test]
    fn invalid_operation_display() {
        let err = LogicError::InvalidOperation("bad move".to_string());
        assert_eq!(err.to_string(), "bad move");
    }

    #[test]
    fn insufficient_display() {
        let err = LogicError::Insufficient {
            item_id: "gold".to_string(),
            have: 10,
            need: 50,
        };
        let msg = err.to_string();
        assert!(msg.contains("gold"));
        assert!(msg.contains("10"));
        assert!(msg.contains("50"));
    }

    #[test]
    fn weapon_not_found_display() {
        let err = LogicError::WeaponNotFound(WeaponInstId::new(42));
        let msg = err.to_string();
        assert!(msg.contains("42"));
    }

    #[test]
    fn weapon_equipped_display() {
        let err = LogicError::WeaponEquipped(WeaponInstId::new(5));
        let msg = err.to_string();
        assert!(msg.contains("5"));
        assert!(msg.contains("equipped"));
    }

    #[test]
    fn weapon_already_equipped_display() {
        let err = LogicError::WeaponAlreadyEquipped(WeaponInstId::new(7), 100);
        let msg = err.to_string();
        assert!(msg.contains("7"));
        assert!(msg.contains("100"));
    }

    #[test]
    fn weapon_locked_display() {
        let err = LogicError::WeaponLocked(WeaponInstId::new(3));
        let msg = err.to_string();
        assert!(msg.contains("3"));
        assert!(msg.contains("locked"));
    }

    #[test]
    fn weapon_fodder_self_display() {
        let err = LogicError::WeaponFodderSelf(WeaponInstId::new(1));
        let msg = err.to_string();
        assert!(msg.contains("own fodder"));
    }

    #[test]
    fn weapon_max_breakthrough_display() {
        let err = LogicError::WeaponMaxBreakthrough(WeaponInstId::new(10));
        let msg = err.to_string();
        assert!(msg.contains("max breakthrough"));
    }

    #[test]
    fn weapon_breakthrough_level_too_low_display() {
        let err = LogicError::WeaponBreakthroughLevelTooLow {
            id: WeaponInstId::new(10),
            current: 20,
            required: 40,
        };
        let msg = err.to_string();
        assert!(msg.contains("20"));
        assert!(msg.contains("40"));
    }

    #[test]
    fn gem_not_found_display() {
        let err = LogicError::GemNotFound(GemInstId::new(99));
        let msg = err.to_string();
        assert!(msg.contains("99"));
    }

    #[test]
    fn gem_socketed_display() {
        let err = LogicError::GemSocketed(GemInstId::new(5));
        let msg = err.to_string();
        assert!(msg.contains("socketed"));
    }

    #[test]
    fn equip_not_found_display() {
        let err = LogicError::EquipNotFound(EquipInstId::new(33));
        let msg = err.to_string();
        assert!(msg.contains("33"));
    }

    #[test]
    fn equip_equipped_display() {
        let err = LogicError::EquipEquipped(EquipInstId::new(2));
        let msg = err.to_string();
        assert!(msg.contains("2"));
        assert!(msg.contains("equipped"));
    }

    #[test]
    fn weapon_refine_type_mismatch_display() {
        let err = LogicError::WeaponRefineTypeMismatch;
        let msg = err.to_string();
        assert!(msg.contains("template"));
    }

    #[test]
    fn result_ok() {
        let r: Result<i32> = Ok(42);
        assert_eq!(r.unwrap(), 42);
    }

    #[test]
    fn result_err() {
        let r: Result<i32> = Err(LogicError::NotFound("oops".to_string()));
        assert!(r.is_err());
    }
}
