use crate::tables::weapon::{BreakthroughTemplate, Weapon, WeaponTable};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

pub struct WeaponAssets {
    data: HashMap<String, Weapon>,
    breakthrough: HashMap<String, BreakthroughTemplate>,
}

impl WeaponAssets {
    pub(super) fn load(tables_dir: &Path) -> Result<Self> {
        let path = tables_dir.join("Weapon.json");
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        let table: WeaponTable = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        Ok(Self {
            data: table.weapon_basic_table,
            breakthrough: table.weapon_break_through_template_table,
        })
    }

    pub fn get(&self, weapon_id: &str) -> Option<&Weapon> {
        self.data.get(weapon_id)
    }

    pub fn get_by_type(&self, weapon_type: u32) -> Vec<&Weapon> {
        self.data
            .values()
            .filter(|w| w.weapon_type == weapon_type)
            .collect()
    }

    pub fn get_suitable_for_char(&self, char_weapon_type: u32) -> Vec<&Weapon> {
        self.get_by_type(char_weapon_type)
    }

    pub fn get_best_for_char(&self, char_weapon_type: u32) -> Option<&Weapon> {
        self.get_by_type(char_weapon_type)
            .into_iter()
            .max_by_key(|w| w.rarity)
    }

    pub fn get_by_rarity(&self, rarity: u32) -> Vec<&Weapon> {
        self.data.values().filter(|w| w.rarity == rarity).collect()
    }

    pub fn get_by_rarity_and_type(&self, rarity: u32, weapon_type: u32) -> Vec<&Weapon> {
        self.data
            .values()
            .filter(|w| w.rarity == rarity && w.weapon_type == weapon_type)
            .collect()
    }

    pub fn get_signature_weapons_for_type(&self, weapon_type: u32) -> Vec<&Weapon> {
        self.get_by_rarity_and_type(6, weapon_type)
    }

    pub fn get_premium_weapons_for_type(&self, weapon_type: u32) -> Vec<&Weapon> {
        self.get_by_type(weapon_type)
            .into_iter()
            .filter(|w| w.rarity >= 5)
            .collect()
    }

    pub fn get_max_breakthrough_lv(&self, weapon_id: &str) -> u64 {
        let Some(weapon) = self.data.get(weapon_id) else {
            return 0;
        };
        let Some(template) = self.breakthrough.get(&weapon.breakthrough_template_id) else {
            return 0;
        };
        template
            .list
            .last()
            .map(|e| e.breakthrough_lv as u64)
            .unwrap_or(0)
    }

    pub fn contains(&self, weapon_id: &str) -> bool {
        self.data.contains_key(weapon_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Weapon)> {
        self.data.iter()
    }

    pub fn all_weapons(&self) -> impl Iterator<Item = &Weapon> {
        self.data.values()
    }

    pub fn count(&self) -> usize {
        self.data.len()
    }

    pub fn get_breakthrough_template(&self, template_id: &str) -> Option<&BreakthroughTemplate> {
        self.breakthrough.get(template_id)
    }

    pub fn get_breakthrough_required_level(&self, weapon_id: &str, target_lv: u32) -> Option<u32> {
        let weapon = self.data.get(weapon_id)?;
        let template = self.breakthrough.get(&weapon.breakthrough_template_id)?;
        template
            .list
            .iter()
            .find(|e| e.breakthrough_lv == target_lv)
            .map(|e| e.breakthrough_show_lv)
    }

    pub fn count_by_type(&self) -> HashMap<u32, usize> {
        let mut map = HashMap::new();
        for w in self.data.values() {
            *map.entry(w.weapon_type).or_insert(0) += 1;
        }
        map
    }
}
