use anyhow::{Context, Result, bail};
use common::time::now_ms;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use config::BeyondAssets;
use perlica_proto::{
    ItemInst, ScItemBagSync, ScWeaponAddExp, ScWeaponAttachGem, ScWeaponBreakthrough,
    ScWeaponDetachGem, ScWeaponPuton, ScdItemDepot, ScdItemGrid, WeaponData, item_inst::InstImpl,
};

// Unique identifier for weapon instances
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct WeaponInstId(u64);

impl WeaponInstId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

/// A single weapon instance in the depot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponInstance {
    /// Unique instance ID (not to be confused with template_id)
    pub inst_id: WeaponInstId,
    /// Template ID referencing the weapon configuration
    pub template_id: String,
    /// Current experience points
    pub exp: u64,
    /// Current weapon level
    pub weapon_lv: u64,
    /// Refinement level (superimpose)
    pub refine_lv: u64,
    /// Breakthrough level (ascension)
    pub breakthrough_lv: u64,
    /// Character ID this weapon is equipped to (0 if unequipped)
    pub equip_char_id: u64,
    /// Attached gem instance ID (0 if none)
    pub attach_gem_id: u64,
    /// Locked status - prevents accidental deletion/use as fodder
    pub is_lock: bool,
    /// New item flag
    pub is_new: bool,
    /// Timestamp when acquired (for sorting)
    pub own_time: i64,
}

impl WeaponInstance {
    // Create a new weapon instance with default values
    pub fn new(inst_id: WeaponInstId, template_id: String, own_time: i64) -> Self {
        Self {
            inst_id,
            template_id,
            exp: 0,
            weapon_lv: 1,
            refine_lv: 0,
            breakthrough_lv: 1,
            equip_char_id: 0,
            attach_gem_id: 0,
            is_lock: false,
            is_new: true,
            own_time,
        }
    }

    pub fn is_equipped(&self) -> bool {
        self.equip_char_id != 0
    }

    pub fn to_weapon_data(&self) -> WeaponData {
        WeaponData {
            inst_id: self.inst_id.as_u64(),
            template_id: self.template_id.clone(),
            exp: self.exp,
            weapon_lv: self.weapon_lv,
            refine_lv: self.refine_lv,
            breakthrough_lv: self.breakthrough_lv,
            equip_char_id: self.equip_char_id,
            attach_gem_id: self.attach_gem_id,
        }
    }

    pub fn to_item_inst(&self) -> ItemInst {
        ItemInst {
            inst_id: self.inst_id.as_u64(),
            is_lock: self.is_lock,
            is_new: self.is_new,
            inst_impl: Some(InstImpl::Weapon(self.to_weapon_data())),
        }
    }

    pub fn to_item_grid(&self) -> ScdItemGrid {
        ScdItemGrid {
            grid_index: 0, // Depot items don't use grid index
            id: self.template_id.clone(),
            count: 1, // Weapons are not stackable
            inst: Some(self.to_item_inst()),
        }
    }
}

/// Manages all weapon instances for a player
/// Weapons are stored in depot type 1
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WeaponDepot {
    /// All weapon instances keyed by inst_id
    weapons: HashMap<WeaponInstId, WeaponInstance>,
    /// Counter for generating unique instance IDs
    next_inst_id: u64,
    /// Reverse mapping: char_id -> weapon_inst_id for quick lookups
    equipped_weapons: HashMap<u64, WeaponInstId>,
}

impl WeaponDepot {
    pub const DEPOT_TYPE: i32 = 1;

    // Create a new empty weapon depot
    pub fn new() -> Self {
        Self {
            weapons: HashMap::new(),
            next_inst_id: 1, // Start from 1, 0 means unequipped/no weapon
            equipped_weapons: HashMap::new(),
        }
    }

    fn alloc_inst_id(&mut self) -> WeaponInstId {
        let id = WeaponInstId::new(self.next_inst_id);
        self.next_inst_id += 1;
        id
    }

    pub fn next_inst_id(&self) -> u64 {
        self.next_inst_id
    }

    pub fn set_next_inst_id(&mut self, id: u64) {
        self.next_inst_id = id;
    }

    pub fn add_weapon(&mut self, template_id: String, own_time: i64) -> WeaponInstId {
        let inst_id = self.alloc_inst_id();
        let weapon = WeaponInstance::new(inst_id, template_id, own_time);

        debug!(
            "Adding weapon: inst_id={}, template_id={}",
            inst_id.as_u64(),
            weapon.template_id
        );

        self.weapons.insert(inst_id, weapon);
        inst_id
    }

    // Add a fully initialized weapon (for initialization/loading)
    pub fn insert_weapon(&mut self, weapon: WeaponInstance) {
        // Update equipped tracking
        if weapon.is_equipped() {
            self.equipped_weapons
                .insert(weapon.equip_char_id, weapon.inst_id);
        }

        // Update next_inst_id if necessary
        let id_val = weapon.inst_id.as_u64();
        if id_val >= self.next_inst_id {
            self.next_inst_id = id_val + 1;
        }

        self.weapons.insert(weapon.inst_id, weapon);
    }

    /// Get a weapon by instance ID
    pub fn get(&self, inst_id: WeaponInstId) -> Option<&WeaponInstance> {
        self.weapons.get(&inst_id)
    }

    /// Get a mutable weapon by instance ID
    pub fn get_mut(&mut self, inst_id: WeaponInstId) -> Option<&mut WeaponInstance> {
        self.weapons.get_mut(&inst_id)
    }

    /// Remove a weapon from the depot
    /// Returns the removed weapon if found
    /// Fails if the weapon is equipped or locked
    pub fn remove_weapon(&mut self, inst_id: WeaponInstId) -> Result<WeaponInstance> {
        let weapon = self.weapons.get(&inst_id).context("Weapon not found")?;

        if weapon.is_equipped() {
            bail!("Cannot remove equipped weapon");
        }

        if weapon.is_lock {
            bail!("Cannot remove locked weapon");
        }

        let weapon = self.weapons.remove(&inst_id).context("Weapon not found")?;

        debug!("Removed weapon: inst_id={}", inst_id.as_u64());
        Ok(weapon)
    }

    pub fn contains(&self, inst_id: WeaponInstId) -> bool {
        self.weapons.contains_key(&inst_id)
    }

    pub fn all_weapons(&self) -> &HashMap<WeaponInstId, WeaponInstance> {
        &self.weapons
    }

    pub fn len(&self) -> usize {
        self.weapons.len()
    }

    pub fn is_empty(&self) -> bool {
        self.weapons.is_empty()
    }

    /// Equip a weapon to a character
    /// Automatically unequips any weapon previously equipped to this character
    /// Returns the previous weapon's inst_id if one was unequipped
    pub fn equip_weapon(
        &mut self,
        weapon_inst_id: WeaponInstId,
        char_id: u64,
    ) -> Result<Option<WeaponInstId>> {
        // Verify weapon exists and get its current equipped status
        let weapon = self
            .weapons
            .get(&weapon_inst_id)
            .context("Weapon not found")?;

        // Check if already equipped to this character
        if weapon.equip_char_id == char_id {
            bail!("Weapon already equipped to this character");
        }

        let prev_char_id = weapon.equip_char_id;

        // Unequip any weapon currently on this character
        let prev_weapon = self.unequip_from_char(char_id);

        // Unequip from previous character if equipped elsewhere
        if prev_char_id != 0 {
            self.equipped_weapons.remove(&prev_char_id);
            if let Some(w) = self.weapons.get_mut(&weapon_inst_id) {
                w.equip_char_id = 0;
            }
            debug!(
                "Unequipped weapon {} from char {}",
                weapon_inst_id.as_u64(),
                prev_char_id
            );
        }

        // Equip to new character
        let weapon = self
            .weapons
            .get_mut(&weapon_inst_id)
            .context("Weapon not found")?;
        weapon.equip_char_id = char_id;

        self.equipped_weapons.insert(char_id, weapon_inst_id);

        info!(
            "Equipped weapon {} to char {} (prev: {:?})",
            weapon_inst_id.as_u64(),
            char_id,
            prev_weapon
        );

        Ok(prev_weapon)
    }

    /// Unequip a weapon from its character
    /// Returns true if a weapon was unequipped
    pub fn unequip_weapon(&mut self, weapon_inst_id: WeaponInstId) -> Result<bool> {
        let weapon = self
            .weapons
            .get_mut(&weapon_inst_id)
            .context("Weapon not found")?;

        if !weapon.is_equipped() {
            return Ok(false);
        }

        let char_id = weapon.equip_char_id;
        weapon.equip_char_id = 0;

        self.equipped_weapons.remove(&char_id);

        debug!(
            "Unequipped weapon {} from char {}",
            weapon_inst_id.as_u64(),
            char_id
        );
        Ok(true)
    }

    /// Unequip any weapon from a specific character
    /// Returns the weapon inst_id that was unequipped, if any
    fn unequip_from_char(&mut self, char_id: u64) -> Option<WeaponInstId> {
        if let Some(&inst_id) = self.equipped_weapons.get(&char_id) {
            if let Some(weapon) = self.weapons.get_mut(&inst_id) {
                weapon.equip_char_id = 0;
            }
            self.equipped_weapons.remove(&char_id);
            Some(inst_id)
        } else {
            None
        }
    }

    /// Get the weapon equipped to a character
    pub fn get_equipped_weapon(&self, char_id: u64) -> Option<&WeaponInstance> {
        self.equipped_weapons
            .get(&char_id)
            .and_then(|&inst_id| self.weapons.get(&inst_id))
    }

    /// Get the weapon inst_id equipped to a character
    pub fn get_equipped_weapon_id(&self, char_id: u64) -> Option<WeaponInstId> {
        self.equipped_weapons.get(&char_id).copied()
    }

    /// Check if a character has a weapon equipped
    pub fn has_equipped_weapon(&self, char_id: u64) -> bool {
        self.equipped_weapons.contains_key(&char_id)
    }

    /// Set the lock status of a weapon
    pub fn set_lock(&mut self, inst_id: WeaponInstId, is_lock: bool) -> Result<()> {
        let weapon = self.weapons.get_mut(&inst_id).context("Weapon not found")?;

        weapon.is_lock = is_lock;
        debug!("Set weapon {} lock status to {}", inst_id.as_u64(), is_lock);
        Ok(())
    }

    /// Mark a weapon as no longer new
    pub fn clear_new_flag(&mut self, inst_id: WeaponInstId) -> Result<()> {
        let weapon = self.weapons.get_mut(&inst_id).context("Weapon not found")?;

        weapon.is_new = false;
        Ok(())
    }

    /// Calculate the EXP value of a weapon for use as fodder
    /// Based on weapon rarity and level
    fn calculate_fodder_exp(
        weapon: &WeaponInstance,
        weapon_config: Option<&config::tables::weapon::Weapon>,
    ) -> u64 {
        let base_exp = match weapon_config {
            Some(w) => match w.rarity {
                6 => 5000, // Signature/Legendary
                5 => 3000, // Premium
                4 => 1500, // Rare
                3 => 800,  // Uncommon
                _ => 400,  // Common
            },
            None => 400,
        };

        // Level bonus: each level adds 10% of base
        let level_bonus = (weapon.weapon_lv as f64 * 0.1 * base_exp as f64) as u64;

        base_exp + level_bonus
    }

    /// Calculate weapon level from total EXP
    /// Simple formula: each level requires more exp (linear scaling)
    fn calculate_level_from_exp(
        exp: u64,
        weapon_config: Option<&config::tables::weapon::Weapon>,
    ) -> u64 {
        let base_exp_per_level = match weapon_config {
            Some(w) => match w.rarity {
                6 => 2000u64,
                5 => 1500,
                4 => 1000,
                3 => 600,
                _ => 400,
            },
            None => 400,
        };

        let max_level = match weapon_config {
            Some(w) => match w.rarity {
                6 => 80,
                5 => 70,
                4 => 60,
                3 => 50,
                _ => 40,
            },
            None => 40,
        };

        let level = (exp / base_exp_per_level).max(1);
        level.min(max_level as u64)
    }

    /// Get the required level for a breakthrough stage
    fn get_breakthrough_required_level(
        &self,
        weapon_template_id: &str,
        target_breakthrough_lv: u64,
        assets: &BeyondAssets,
    ) -> Option<u64> {
        let weapon = assets.weapons.get(weapon_template_id)?;
        let template = assets
            .weapons
            .get_breakthrough_template(&weapon.breakthrough_template_id)?;

        template
            .list
            .iter()
            .find(|e| e.breakthrough_lv as u64 == target_breakthrough_lv)
            .map(|e| e.breakthrough_show_lv as u64)
    }

    /// Add experience to a weapon, consuming fodder weapons
    /// Returns the new exp and level
    pub fn add_exp(
        &mut self,
        target_inst_id: WeaponInstId,
        fodder_inst_ids: &[WeaponInstId],
        assets: &BeyondAssets,
    ) -> Result<(u64, u64)> {
        // Validate target exists and is not locked
        let target = self
            .weapons
            .get(&target_inst_id)
            .context("Target weapon not found")?;

        if target.is_lock {
            bail!("Cannot upgrade locked weapon");
        }

        let target_template_id = target.template_id.clone();

        // Calculate total exp from fodder weapons
        let mut total_exp: u64 = 0;
        let fodder_count = fodder_inst_ids.len();

        for &fodder_id in fodder_inst_ids {
            if fodder_id == target_inst_id {
                bail!("Cannot use weapon as its own fodder");
            }

            let fodder = self
                .weapons
                .get(&fodder_id)
                .context("Fodder weapon not found")?;

            if fodder.is_lock {
                bail!("Cannot use locked weapon as fodder");
            }

            if fodder.is_equipped() {
                bail!("Cannot use equipped weapon as fodder");
            }

            // Calculate fodder exp value
            let fodder_exp =
                Self::calculate_fodder_exp(fodder, assets.weapons.get(&fodder.template_id));

            total_exp += fodder_exp;
        }

        // Remove fodder weapons
        for &fodder_id in fodder_inst_ids {
            self.weapons.remove(&fodder_id);
        }

        // Apply exp to target weapon
        let target = self
            .weapons
            .get_mut(&target_inst_id)
            .context("Target weapon not found")?;

        target.exp += total_exp;

        // Calculate new level based on exp
        let new_level =
            Self::calculate_level_from_exp(target.exp, assets.weapons.get(&target_template_id));

        let old_level = target.weapon_lv;
        target.weapon_lv = new_level;

        info!(
            "Weapon {} gained {} exp from {} fodder weapons, level {} -> {}",
            target_inst_id.as_u64(),
            total_exp,
            fodder_count,
            old_level,
            new_level
        );

        Ok((target.exp, target.weapon_lv))
    }

    /// Perform breakthrough on a weapon
    pub fn breakthrough(&mut self, inst_id: WeaponInstId, assets: &BeyondAssets) -> Result<u64> {
        // Validate weapon exists and is not locked
        let weapon = self.weapons.get(&inst_id).context("Weapon not found")?;

        if weapon.is_lock {
            bail!("Cannot breakthrough locked weapon");
        }

        let template_id = weapon.template_id.clone();
        let current_breakthrough = weapon.breakthrough_lv;
        let weapon_lv = weapon.weapon_lv;

        // Get max breakthrough from config
        let max_breakthrough = assets.weapons.get_max_breakthrough_lv(&template_id);

        if current_breakthrough >= max_breakthrough {
            bail!("Weapon is already at max breakthrough level");
        }

        // Get required level for next breakthrough
        let required_level = self
            .get_breakthrough_required_level(&template_id, current_breakthrough + 1, assets)
            .unwrap_or(1);

        if weapon_lv < required_level {
            bail!(
                "Weapon level {} is below required level {} for breakthrough",
                weapon_lv,
                required_level
            );
        }

        let weapon = self.weapons.get_mut(&inst_id).context("Weapon not found")?;

        weapon.breakthrough_lv += 1;

        info!(
            "Weapon {} breakthrough: {} -> {}",
            inst_id.as_u64(),
            current_breakthrough,
            weapon.breakthrough_lv
        );

        Ok(weapon.breakthrough_lv)
    }

    // Get max refinement level for a weapon
    fn get_max_refine(weapon_config: Option<&config::tables::weapon::Weapon>) -> u64 {
        match weapon_config {
            Some(w) => match w.rarity {
                6 => 5, // Signature: max 5 refinements
                5 => 5, // Premium: max 5 refinements
                4 => 4, // Rare: max 4 refinements
                3 => 3, // Uncommon: max 3 refinements
                _ => 2, // Common: max 2 refinements
            },
            None => 5,
        }
    }

    /// Refine a weapon (superimpose)
    /// Consumes another weapon of the same template
    pub fn refine(
        &mut self,
        target_inst_id: WeaponInstId,
        fodder_inst_id: WeaponInstId,
        assets: &BeyondAssets,
    ) -> Result<u64> {
        let target = self
            .weapons
            .get(&target_inst_id)
            .context("Target weapon not found")?;

        if target.is_lock {
            bail!("Cannot refine locked weapon");
        }

        let fodder = self
            .weapons
            .get(&fodder_inst_id)
            .context("Fodder weapon not found")?;

        if fodder.is_lock {
            bail!("Cannot use locked weapon as refinement material");
        }

        if fodder.is_equipped() {
            bail!("Cannot use equipped weapon as refinement material");
        }

        // Must be same template
        if target.template_id != fodder.template_id {
            bail!("Refinement requires weapons of the same type");
        }

        let target_template = target.template_id.clone();

        // Get max refine based on rarity
        let max_refine = Self::get_max_refine(assets.weapons.get(&target_template));

        if target.refine_lv >= max_refine {
            bail!("Weapon is already at max refinement level");
        }

        // Remove fodder
        self.weapons.remove(&fodder_inst_id);

        // Apply refinement
        let target = self
            .weapons
            .get_mut(&target_inst_id)
            .context("Target weapon not found")?;

        target.refine_lv += 1;

        info!(
            "Weapon {} refined: {} -> {}",
            target_inst_id.as_u64(),
            target.refine_lv - 1,
            target.refine_lv
        );

        Ok(target.refine_lv)
    }

    /// Attach a gem to a weapon
    /// Returns the previously attached gem ID if any
    pub fn attach_gem(
        &mut self,
        weapon_inst_id: WeaponInstId,
        gem_inst_id: u64,
    ) -> Result<Option<u64>> {
        let weapon = self
            .weapons
            .get_mut(&weapon_inst_id)
            .context("Weapon not found")?;

        if weapon.is_lock {
            bail!("Cannot modify locked weapon");
        }

        let prev_gem = if weapon.attach_gem_id != 0 {
            Some(weapon.attach_gem_id)
        } else {
            None
        };

        weapon.attach_gem_id = gem_inst_id;

        info!(
            "Attached gem {} to weapon {} (prev: {:?})",
            gem_inst_id,
            weapon_inst_id.as_u64(),
            prev_gem
        );

        Ok(prev_gem)
    }

    /// Detach gem from a weapon
    /// Returns the detached gem ID
    pub fn detach_gem(&mut self, weapon_inst_id: WeaponInstId) -> Result<u64> {
        let weapon = self
            .weapons
            .get_mut(&weapon_inst_id)
            .context("Weapon not found")?;

        if weapon.is_lock {
            bail!("Cannot modify locked weapon");
        }

        if weapon.attach_gem_id == 0 {
            bail!("Weapon has no attached gem");
        }

        let gem_id = weapon.attach_gem_id;
        weapon.attach_gem_id = 0;

        info!(
            "Detached gem {} from weapon {}",
            gem_id,
            weapon_inst_id.as_u64()
        );

        Ok(gem_id)
    }

    pub fn to_depot_sync(&self) -> ScdItemDepot {
        let inst_list: Vec<ScdItemGrid> = self.weapons.values().map(|w| w.to_item_grid()).collect();

        ScdItemDepot { inst_list }
    }

    pub fn to_weapon_modify(&self, inst_id: WeaponInstId) -> Option<ScdItemGrid> {
        self.weapons.get(&inst_id).map(|w| w.to_item_grid())
    }

    pub fn to_weapon_delete(inst_id: WeaponInstId) -> u64 {
        inst_id.as_u64()
    }

    pub fn to_add_exp_sc(&self, inst_id: WeaponInstId) -> Option<ScWeaponAddExp> {
        self.weapons.get(&inst_id).map(|w| ScWeaponAddExp {
            weaponid: inst_id.as_u64(),
            new_exp: w.exp,
            weapon_lv: w.weapon_lv,
        })
    }

    pub fn to_breakthrough_sc(&self, inst_id: WeaponInstId) -> Option<ScWeaponBreakthrough> {
        self.weapons.get(&inst_id).map(|w| ScWeaponBreakthrough {
            weaponid: inst_id.as_u64(),
            breakthrough_lv: w.breakthrough_lv,
        })
    }

    pub fn to_attach_gem_sc(
        &self,
        weapon_inst_id: WeaponInstId,
        detached_gem_id: Option<u64>,
        detached_gem_weapon_id: Option<u64>,
    ) -> Option<ScWeaponAttachGem> {
        self.weapons
            .get(&weapon_inst_id)
            .map(|w| ScWeaponAttachGem {
                weaponid: weapon_inst_id.as_u64(),
                gemid: w.attach_gem_id,
                detach_gemid: detached_gem_id.unwrap_or(0),
                detach_gem_weaponid: detached_gem_weapon_id.unwrap_or(0),
            })
    }

    pub fn to_detach_gem_sc(&self, weapon_inst_id: WeaponInstId, gem_id: u64) -> ScWeaponDetachGem {
        ScWeaponDetachGem {
            weaponid: weapon_inst_id.as_u64(),
            detach_gemid: gem_id,
        }
    }

    pub fn to_weapon_puton_sc(
        &self,
        charid: u64,
        weapon_inst_id: WeaponInstId,
        off_weapon_id: Option<u64>,
        put_off_char_id: Option<u64>,
    ) -> ScWeaponPuton {
        ScWeaponPuton {
            charid,
            weaponid: weapon_inst_id.as_u64(),
            offweaponid: off_weapon_id.unwrap_or(0),
            put_off_charid: put_off_char_id.unwrap_or(0),
        }
    }

    pub fn build_item_bag_sync(&self) -> ScItemBagSync {
        let mut depot = HashMap::new();
        depot.insert(Self::DEPOT_TYPE, self.to_depot_sync());

        ScItemBagSync {
            depot,
            bag: None,
            factory_depot: None,
            cannot_destroy: HashMap::new(),
            use_blackboard: None,
        }
    }

    /// Get all equipped weapon template IDs for a set of characters
    /// Used during CharBag initialization to ensure characters have their weapons
    pub fn get_equipped_templates_for_chars(&self, char_ids: &[u64]) -> HashMap<u64, String> {
        char_ids
            .iter()
            .filter_map(|&char_id| {
                self.get_equipped_weapon(char_id)
                    .map(|w| (char_id, w.template_id.clone()))
            })
            .collect()
    }

    /// Initialize default weapons for characters that don't have one
    /// Called during CharBag::new to ensure every character has a weapon
    pub fn init_default_weapons_for_chars(
        &mut self,
        char_template_ids: &[(u64, String)],
        assets: &BeyondAssets,
    ) -> Vec<(u64, WeaponInstId)> {
        let mut equipped = Vec::new();
        let own_time = now_ms() as i64;

        for (char_id, char_template_id) in char_template_ids {
            // Skip if character already has a weapon equipped
            if self.has_equipped_weapon(*char_id) {
                continue;
            }

            // Get character's weapon type from config
            let char_data = match assets.characters.get(char_template_id) {
                Some(data) => data,
                None => {
                    warn!("Character template not found: {}", char_template_id);
                    continue;
                }
            };

            // Find best default weapon for this character
            let weapon = assets
                .weapons
                .get_best_for_char(char_data.weapon_type)
                .or_else(|| {
                    assets
                        .weapons
                        .get_by_type(char_data.weapon_type)
                        .first()
                        .copied()
                })
                .unwrap_or_else(|| {
                    assets
                        .weapons
                        .get("wpn_0002")
                        .expect("Default weapon must exist")
                });

            // Create and equip the weapon
            let inst_id = self.add_weapon(weapon.weapon_id.clone(), own_time);

            if let Ok(_) = self.equip_weapon(inst_id, *char_id) {
                equipped.push((*char_id, inst_id));
                info!(
                    "Initialized default weapon {} for char {} (template: {})",
                    weapon.weapon_id, char_id, char_template_id
                );
            }
        }

        equipped
    }

    /// Validate and repair weapon-character relationships
    /// Should be called after loading save data
    pub fn validate_equipped_weapons(&mut self) {
        let mut to_fix: Vec<(u64, WeaponInstId)> = Vec::new();
        let mut orphaned_weapons: Vec<WeaponInstId> = Vec::new();

        // Check for inconsistencies
        for (&char_id, &inst_id) in &self.equipped_weapons {
            if let Some(weapon) = self.weapons.get(&inst_id) {
                if weapon.equip_char_id != char_id {
                    warn!(
                        "Weapon {} has equip_char_id {} but equipped_weapons maps to char {}",
                        inst_id.as_u64(),
                        weapon.equip_char_id,
                        char_id
                    );
                    to_fix.push((char_id, inst_id));
                }
            } else {
                warn!(
                    "Equipped weapon {} for char {} not found in depot",
                    inst_id.as_u64(),
                    char_id
                );
                to_fix.push((char_id, inst_id));
            }
        }

        // Check for weapons that claim to be equipped but aren't in equipped_weapons
        for (&inst_id, weapon) in &self.weapons {
            if weapon.is_equipped() && !self.equipped_weapons.contains_key(&weapon.equip_char_id) {
                warn!(
                    "Weapon {} claims equip_char_id {} but not in equipped_weapons",
                    inst_id.as_u64(),
                    weapon.equip_char_id
                );
                orphaned_weapons.push(inst_id);
            }
        }

        // Fix orphaned weapons
        for inst_id in orphaned_weapons {
            if let Some(w) = self.weapons.get_mut(&inst_id) {
                w.equip_char_id = 0;
            }
        }

        // Remove invalid entries
        for (char_id, _) in to_fix {
            self.equipped_weapons.remove(&char_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weapon_lifecycle() {
        let mut depot = WeaponDepot::new();

        // Add weapon
        let inst_id = depot.add_weapon("wpn_test_001".to_string(), 1234567890);
        assert!(depot.contains(inst_id));
        assert_eq!(depot.len(), 1);

        // Get weapon
        let weapon = depot.get(inst_id).unwrap();
        assert_eq!(weapon.template_id, "wpn_test_001");
        assert_eq!(weapon.weapon_lv, 1);
        assert!(!weapon.is_equipped());

        // Equip to character
        depot.equip_weapon(inst_id, 1001).unwrap();
        let weapon = depot.get(inst_id).unwrap();
        assert!(weapon.is_equipped());
        assert_eq!(weapon.equip_char_id, 1001);

        // Unequip
        depot.unequip_weapon(inst_id).unwrap();
        let weapon = depot.get(inst_id).unwrap();
        assert!(!weapon.is_equipped());

        // Remove
        depot.remove_weapon(inst_id).unwrap();
        assert!(!depot.contains(inst_id));
        assert_eq!(depot.len(), 0);
    }

    #[test]
    fn test_equip_swap() {
        let mut depot = WeaponDepot::new();

        let w1 = depot.add_weapon("wpn_001".to_string(), 1);
        let w2 = depot.add_weapon("wpn_002".to_string(), 1);

        // Equip w1 to char 1001
        depot.equip_weapon(w1, 1001).unwrap();
        assert_eq!(depot.get_equipped_weapon(1001).unwrap().inst_id, w1);

        // Equip w2 to same char - should unequip w1
        depot.equip_weapon(w2, 1001).unwrap();
        assert_eq!(depot.get_equipped_weapon(1001).unwrap().inst_id, w2);
        assert!(!depot.get(w1).unwrap().is_equipped());

        // w1 should be unequipped
        let w1_data = depot.get(w1).unwrap();
        assert_eq!(w1_data.equip_char_id, 0);
    }

    #[test]
    fn test_lock_prevents_removal() {
        let mut depot = WeaponDepot::new();
        let inst_id = depot.add_weapon("wpn_test".to_string(), 1);

        // Lock weapon
        depot.set_lock(inst_id, true).unwrap();

        // Should fail to remove
        assert!(depot.remove_weapon(inst_id).is_err());

        // Unlock and remove
        depot.set_lock(inst_id, false).unwrap();
        depot.remove_weapon(inst_id).unwrap();
    }

    #[test]
    fn test_equipped_prevents_removal() {
        let mut depot = WeaponDepot::new();
        let inst_id = depot.add_weapon("wpn_test".to_string(), 1);

        depot.equip_weapon(inst_id, 1001).unwrap();

        // Should fail to remove equipped weapon
        assert!(depot.remove_weapon(inst_id).is_err());

        depot.unequip_weapon(inst_id).unwrap();
        depot.remove_weapon(inst_id).unwrap();
    }
}
