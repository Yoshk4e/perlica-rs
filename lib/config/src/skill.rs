use crate::tables::skill_patch::{SkillPatchBundle, SkillPatchEntry, SkillPatchTable};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

pub struct SkillAssets {
    data: HashMap<String, SkillPatchBundle>,
}

impl SkillAssets {
    pub(super) fn load(tables_dir: &Path) -> Result<Self> {
        let path = tables_dir.join("SkillPatchTable.json");
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let table: SkillPatchTable = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse {}", path.display()))?;
        Ok(Self { data: table })
    }

    pub fn get(&self, skill_id: &str) -> Option<&SkillPatchBundle> {
        self.data.get(skill_id)
    }

    pub fn get_at_level(&self, skill_id: &str, level: u32) -> Option<&SkillPatchEntry> {
        self.data
            .get(skill_id)?
            .entries
            .iter()
            .find(|e| e.level == level)
    }

    pub fn get_char_skills(&self, char_id: &str) -> Vec<&SkillPatchBundle> {
        self.data
            .iter()
            .filter(|(id, _)| id.starts_with(char_id))
            .map(|(_, bundle)| bundle)
            .collect()
    }

    pub fn get_max_level(&self, skill_id: &str) -> u32 {
        self.data
            .get(skill_id)
            .map(|b| b.entries.iter().map(|e| e.level).max().unwrap_or(1))
            .unwrap_or(1)
    }

    pub fn contains(&self, skill_id: &str) -> bool {
        self.data.contains_key(skill_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &SkillPatchBundle)> {
        self.data.iter()
    }

    pub fn count(&self) -> usize {
        self.data.len()
    }
}
