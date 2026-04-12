use crate::error::{ConfigError, Result};
use crate::tables::character::{Attributes, Character, CharacterTable};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Global character constants from `characterConst` in `Character.json`.
///
/// `level_up_exp[i]` = exp required to advance from level `i+1` to `i+2`.
/// A `-1` sentinel marks the absolute maximum level (no further advancement).
///
/// `level_up_gold[i]` = gold cost for the same transition (unused in the
/// emulator but stored for completeness).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CharacterConst {
    pub level_up_exp: Vec<i32>,
    pub level_up_gold: Vec<i32>,
    pub max_level: u32,
    pub max_break: u32,
}

#[derive(Debug, Deserialize)]
struct RawCharacterConst {
    #[serde(rename = "maxLevel")]
    max_level: u32,
    #[serde(rename = "maxBreak")]
    max_break: u32,
    #[serde(rename = "levelUpExp")]
    level_up_exp: Vec<i32>,
    #[serde(rename = "levelUpGold")]
    level_up_gold: Vec<i32>,
}

pub struct CharacterAssets {
    data: HashMap<String, Character>,
    char_const: CharacterConst,
}

impl CharacterAssets {
    pub(super) fn load(tables_dir: &Path) -> Result<Self> {
        let path = tables_dir.join("Character.json");
        let contents = std::fs::read_to_string(&path).map_err(|e| ConfigError::ReadFile {
            path: path.clone(),
            source: e,
        })?;

        // Deserialise just what we need from the file without pulling in every
        // field, serde's `deny_unknown_fields` is intentionally *not* set.
        #[derive(Deserialize)]
        struct CharacterFile {
            #[serde(rename = "characterTable")]
            character_table: HashMap<String, Character>,
            #[serde(rename = "characterConst")]
            character_const: RawCharacterConst,
        }

        let file: CharacterFile =
            serde_json::from_str(&contents).map_err(|e| ConfigError::ParseJson {
                path: path.clone(),
                source: e,
            })?;

        let char_const = CharacterConst {
            max_level: file.character_const.max_level,
            max_break: file.character_const.max_break,
            level_up_exp: file.character_const.level_up_exp,
            level_up_gold: file.character_const.level_up_gold,
        };

        Ok(Self {
            data: file.character_table,
            char_const,
        })
    }

    pub fn get(&self, char_id: &str) -> Option<&Character> {
        self.data.get(char_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Character)> {
        self.data.iter()
    }

    pub fn get_stats(&self, char_id: &str, level: i32, break_stage: u32) -> Option<&Attributes> {
        let c = self.get(char_id)?;
        c.attributes
            .iter()
            .find(|a| a.level == level && a.break_stage == break_stage)
    }

    pub fn get_skills(&self, char_id: &str, break_stage: u32) -> Option<Vec<&str>> {
        let c = self.get(char_id)?;
        Some(
            c.fac_skills
                .iter()
                .filter(|s| s.break_stage <= break_stage)
                .map(|s| s.skill_id.as_str())
                .collect(),
        )
    }

    pub fn count(&self) -> usize {
        self.data.len()
    }

    /// Global levelling constants (`characterConst` section of `Character.json`).
    pub fn char_const(&self) -> &CharacterConst {
        &self.char_const
    }
}
