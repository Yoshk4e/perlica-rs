pub mod character;
pub mod equip;
pub mod error;
pub mod id_to_str;
pub mod item;
pub mod level_data;
pub mod mission;
pub mod skill;
pub mod str_to_id;
pub mod tables;
pub mod weapon;

use crate::id_to_str::NumIdStrAssets;
use crate::str_to_id::StrIdNumAssets;
pub use error::{ConfigError, Result};
pub use item::{CraftShowingType, ItemConfig, ItemDepotType, ItemKind};
use std::path::Path;

pub struct BeyondAssets {
    pub characters: character::CharacterAssets,
    pub char_skills: skill::SkillAssets,
    pub weapons: weapon::WeaponAssets,
    pub equipment: equip::EquipmentAssets,
    pub items: item::ItemAssets,
    pub level_data: level_data::LevelDataAssets,
    pub missions: mission::MissionAssets,
    pub str_id_num: StrIdNumAssets,
    pub num_id_str: NumIdStrAssets,
}

impl BeyondAssets {
    pub fn load<P: AsRef<Path>>(base_path: P) -> Result<Self> {
        let tables_dir = base_path.as_ref().join("tables");
        let config_dir = base_path.as_ref().join("config");
        Ok(Self {
            characters: character::CharacterAssets::load(&tables_dir)?,
            char_skills: skill::SkillAssets::load(&tables_dir)?,
            weapons: weapon::WeaponAssets::load(&tables_dir)?,
            equipment: equip::EquipmentAssets::load(&tables_dir)?,
            items: item::ItemAssets::load(&tables_dir)?,
            level_data: level_data::LevelDataAssets::load(&config_dir)?,
            missions: mission::MissionAssets::load(&tables_dir)?,
            str_id_num: StrIdNumAssets::load(&tables_dir)?,
            num_id_str: NumIdStrAssets::load(&tables_dir)?,
        })
    }
}
