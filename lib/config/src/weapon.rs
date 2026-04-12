use crate::error::{ConfigError, Result};
use crate::tables::weapon::{BreakthroughTemplate, UpgradeTemplateSum, Weapon, WeaponTable};
use std::collections::HashMap;
use std::path::Path;

pub struct WeaponAssets {
    data: HashMap<String, Weapon>,
    breakthrough: HashMap<String, BreakthroughTemplate>,
    upgrade_sum: HashMap<String, UpgradeTemplateSum>,
    weapon_exp_by_item_id: HashMap<String, u64>,
}

impl WeaponAssets {
    pub(super) fn load(tables_dir: &Path) -> Result<Self> {
        let path = tables_dir.join("Weapon.json");
        let contents = std::fs::read_to_string(&path).map_err(|e| ConfigError::ReadFile {
            path: path.clone(),
            source: e,
        })?;

        let table: WeaponTable =
            serde_json::from_str(&contents).map_err(|e| ConfigError::ParseJson {
                path: path.clone(),
                source: e,
            })?;

        let weapon_exp_by_item_id = table
            .weapon_exp_item_table
            .values()
            .filter(|e| !e.exp_item_id.is_empty())
            .map(|e| (e.exp_item_id.clone(), e.weapon_exp as u64))
            .collect();

        Ok(Self {
            data: table.weapon_basic_table,
            breakthrough: table.weapon_break_through_template_table,
            upgrade_sum: table.weapon_upgrade_template_sum_table,
            weapon_exp_by_item_id,
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

    pub fn get_upgrade_sum(&self, template_id: &str) -> Option<&UpgradeTemplateSum> {
        self.upgrade_sum.get(template_id)
    }

    /// Returns the weapon exp granted by consuming one unit of `item_id`.
    ///
    /// Returns 0 for items not present in `weaponExpItemTable`.
    #[inline]
    pub fn weapon_exp_for_item(&self, item_id: &str) -> u64 {
        self.weapon_exp_by_item_id
            .get(item_id)
            .copied()
            .unwrap_or(0)
    }

    /// Returns the weapon level corresponding to `total_exp`, capped at `weapon.maxLv`.
    ///
    /// Uses the `weaponUpgradeTemplateSumTable` curve. Falls back to level 1 if the
    /// weapon or its level template cannot be found.
    pub fn weapon_level_from_exp(&self, weapon_id: &str, total_exp: u64) -> u64 {
        let Some(weapon) = self.data.get(weapon_id) else {
            return 1;
        };
        let max_lv = weapon.max_lv as u64;
        let Some(sum_table) = self.upgrade_sum.get(&weapon.level_template_id) else {
            return 1;
        };
        let mut level = 1u64;
        for entry in &sum_table.list {
            let lv = entry.weapon_lv as u64;
            if lv > max_lv {
                break;
            }
            if (entry.lv_up_exp_sum as u64) <= total_exp {
                level = lv;
            } else {
                break;
            }
        }
        level
    }
}
