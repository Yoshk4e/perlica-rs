use crate::error::{ConfigError, Result};
use crate::tables::character::{Attributes, Character, CharacterTable};
use std::collections::HashMap;
use std::path::Path;

pub struct CharacterAssets {
    data: HashMap<String, Character>,
}

impl CharacterAssets {
    pub(super) fn load(tables_dir: &Path) -> Result<Self> {
        let path = tables_dir.join("Character.json");
        let contents = std::fs::read_to_string(&path).map_err(|e| ConfigError::ReadFile {
            path: path.clone(),
            source: e,
        })?;
        let table: CharacterTable =
            serde_json::from_str(&contents).map_err(|e| ConfigError::ParseJson {
                path: path.clone(),
                source: e,
            })?;

        Ok(Self {
            data: table.character_table,
        })
    }

    pub fn get(&self, char_id: &str) -> Option<&Character> {
        self.data.get(char_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Character)> {
        self.data.iter()
    }

    pub fn get_stats(&self, char_id: &str, level: i32, break_stage: u32) -> Option<&Attributes> {
        let character = self.get(char_id)?;
        character
            .attributes
            .iter()
            .find(|attr| attr.level == level && attr.break_stage == break_stage)
    }

    pub fn get_skills(&self, char_id: &str, break_stage: u32) -> Option<Vec<&str>> {
        let character = self.get(char_id)?;
        let skills = character
            .fac_skills
            .iter()
            .filter(|s| s.break_stage <= break_stage)
            .map(|s| s.skill_id.as_str())
            .collect();
        Some(skills)
    }

    pub fn count(&self) -> usize {
        self.data.len()
    }
}
