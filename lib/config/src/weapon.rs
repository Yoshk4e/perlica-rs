use crate::tables::weapon::{Weapon, WeaponTable};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

pub struct WeaponAssets {
    data: HashMap<String, Weapon>,
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
        })
    }

    // Get a single weapon by its ID (e.g. "wpn_0002")
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

    // Get weapons by rarity (e.g. 6 = 6★ / SSR)
    pub fn get_by_rarity(&self, rarity: u32) -> Vec<&Weapon> {
        self.data.values().filter(|w| w.rarity == rarity).collect()
    }

    pub fn get_by_rarity_and_type(&self, rarity: u32, weapon_type: u32) -> Vec<&Weapon> {
        self.data
            .values()
            .filter(|w| w.rarity == rarity && w.weapon_type == weapon_type)
            .collect()
    }

    // Get all 6★ (signature / limited) weapons for a given weapon_type
    pub fn get_signature_weapons_for_type(&self, weapon_type: u32) -> Vec<&Weapon> {
        self.get_by_rarity_and_type(6, weapon_type)
    }

    // Get premium weapons (rarity 5★ and 6★) for a type
    pub fn get_premium_weapons_for_type(&self, weapon_type: u32) -> Vec<&Weapon> {
        self.get_by_type(weapon_type)
            .into_iter()
            .filter(|w| w.rarity >= 5)
            .collect()
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

    // How many weapons exist for each weapon_type
    pub fn count_by_type(&self) -> HashMap<u32, usize> {
        let mut map = HashMap::new();
        for w in self.data.values() {
            *map.entry(w.weapon_type).or_insert(0) += 1;
        }
        map
    }
}
