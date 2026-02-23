pub mod character;
pub mod skill;
pub mod tables;

use anyhow::Result;
use std::path::Path;

pub struct BeyondAssets {
    pub characters: character::CharacterAssets,
    pub char_skills: skill::SkillAssets,
}

impl BeyondAssets {
    pub fn load<P: AsRef<Path>>(base_path: P) -> Result<Self> {
        let base_path = base_path.as_ref();
        let tables_dir = base_path.join("tables");

        Ok(BeyondAssets {
            characters: character::CharacterAssets::load(&tables_dir)?,
            char_skills: skill::SkillAssets::load(&tables_dir)?,
        })
    }
}
