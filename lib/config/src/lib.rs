pub mod character;
pub mod contract_table;
pub mod equip;
pub mod error;
pub mod factory_const;
pub mod factory_manufact_const;
pub mod factory_map;
pub mod factory_processor_const;
pub mod factory_recycler_const;
pub mod factory_skill_const;
pub mod factory_sttree;
pub mod factory_table;
pub mod id_to_str;
pub mod item;
pub mod level_data;
pub mod mission;
pub mod repair_table;
pub mod reward;
pub mod skill;
pub mod str_to_id;
pub mod tables;
pub mod weapon;

use crate::character::CharacterAssets;
use crate::contract_table::ContractAssets;
use crate::equip::EquipmentAssets;
use crate::factory_const::FConstAssets;
use crate::factory_manufact_const::FManufactConstAssets;
use crate::factory_map::FRegionAssets;
use crate::factory_processor_const::FProcessorConstAssets;
use crate::factory_recycler_const::FRecyclerConstAssets;
use crate::factory_skill_const::FSkillConstAssets;
use crate::factory_sttree::FSttreeAssets;
use crate::factory_table::FTableAssets;
use crate::id_to_str::NumIdStrAssets;
use crate::item::ItemAssets;
use crate::level_data::LevelDataAssets;
use crate::mission::MissionAssets;
use crate::repair_table::RepairAssets;
use crate::reward::RewardAssets;
use crate::skill::SkillAssets;
use crate::str_to_id::StrIdNumAssets;
use crate::weapon::WeaponAssets;
pub use error::{ConfigError, Result};
pub use item::{CraftShowingType, ItemConfig, ItemDepotType, ItemKind};
use std::path::Path;

pub struct BeyondAssets {
    pub characters: CharacterAssets,
    pub char_skills: SkillAssets,
    pub weapons: WeaponAssets,
    pub equipment: EquipmentAssets,
    pub items: ItemAssets,
    pub level_data: LevelDataAssets,
    pub missions: MissionAssets,
    pub rewards: RewardAssets,
    pub str_id_num: StrIdNumAssets,
    pub num_id_str: NumIdStrAssets,
    pub factory_const: FConstAssets,
    pub factory_skill_const: FSkillConstAssets,
    pub factory_recycler_const: FRecyclerConstAssets,
    pub factory_processor_const: FProcessorConstAssets,
    pub factory_manufact_const: FManufactConstAssets,
    pub factory_map: FRegionAssets,
    pub factory_table: FTableAssets,
    pub factory_sttree: FSttreeAssets,
    pub contracts: ContractAssets,
    pub repair: RepairAssets,
}

impl BeyondAssets {
    pub fn load<P: AsRef<Path>>(base_path: P) -> Result<Self> {
        let tables_dir = base_path.as_ref().join("tables");
        let config_dir = base_path.as_ref().join("config");
        Ok(Self {
            characters: CharacterAssets::load(&tables_dir)?,
            char_skills: SkillAssets::load(&tables_dir)?,
            weapons: WeaponAssets::load(&tables_dir)?,
            equipment: EquipmentAssets::load(&tables_dir)?,
            items: ItemAssets::load(&tables_dir)?,
            level_data: LevelDataAssets::load(&config_dir)?,
            missions: MissionAssets::load(&tables_dir)?,
            rewards: RewardAssets::load(&tables_dir)?,
            str_id_num: StrIdNumAssets::load(&tables_dir)?,
            num_id_str: NumIdStrAssets::load(&tables_dir)?,
            factory_const: FConstAssets::load(&tables_dir)?,
            factory_skill_const: FSkillConstAssets::load(&tables_dir)?,
            factory_recycler_const: FRecyclerConstAssets::load(&tables_dir)?,
            factory_processor_const: FProcessorConstAssets::load(&tables_dir)?,
            factory_manufact_const: FManufactConstAssets::load(&tables_dir)?,
            factory_map: FRegionAssets::load(&tables_dir)?,
            factory_table: FTableAssets::load(&tables_dir)?,
            factory_sttree: FSttreeAssets::load(&tables_dir)?,
            contracts: ContractAssets::load(&tables_dir)?,
            repair: RepairAssets::load(&tables_dir)?,
        })
    }
}
