pub mod character;
pub mod skill;
pub mod str_to_id;
pub mod tables;
pub mod weapon;
pub mod enemy_spawn;

use crate::str_to_id::StrIdNumAssets;

use anyhow::Result;
use std::path::Path;

pub struct BeyondAssets {
    pub characters: character::CharacterAssets,
    pub char_skills: skill::SkillAssets,
    pub weapons: weapon::WeaponAssets,
    pub str_id_num: StrIdNumAssets,
	pub enemy_spawns: enemy_spawn::EnemySpawnAssets,
}

impl BeyondAssets {
    pub fn load<P: AsRef<Path>>(base_path: P) -> Result<Self> {
        let base_path = base_path.as_ref();
        let tables_dir = base_path.join("tables");

        Ok(BeyondAssets {
            characters: character::CharacterAssets::load(&tables_dir)?,
            char_skills: skill::SkillAssets::load(&tables_dir)?,
            weapons: weapon::WeaponAssets::load(&tables_dir)?,
            str_id_num: StrIdNumAssets::load(&tables_dir)?,
			enemy_spawns: enemy_spawn::EnemySpawnAssets::load(&tables_dir)?,
        })
    }
}
