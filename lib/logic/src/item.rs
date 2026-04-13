use crate::error::{LogicError, Result};
use common::time::now_ms;
use config::item::{CraftShowingType, ItemDepotType, ItemKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use config::BeyondAssets;
use perlica_proto::{
    EquipData, GemData, ItemInst, ScItemBagSync, ScWeaponAddExp, ScWeaponAttachGem,
    ScWeaponBreakthrough, ScWeaponDetachGem, ScWeaponPuton, ScdItemBag, ScdItemDepot,
    ScdItemDepotModify, ScdItemGrid, ScdItemUseBlackboard, WeaponData, item_inst::InstImpl,
};

macro_rules! inst_id_newtype {
    ($Name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
        pub struct $Name(u64);
        impl $Name {
            #[inline]
            pub fn new(id: u64) -> Self {
                Self(id)
            }
            #[inline]
            pub fn as_u64(self) -> u64 {
                self.0
            }
        }
        impl std::fmt::Display for $Name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

inst_id_newtype!(WeaponInstId);
inst_id_newtype!(GemInstId);
inst_id_newtype!(EquipInstId);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponInstance {
    pub inst_id: WeaponInstId,
    pub template_id: String,
    pub exp: u64,
    pub weapon_lv: u64,
    pub refine_lv: u64,
    pub breakthrough_lv: u64,
    pub equip_char_id: u64,
    pub attach_gem_id: u64,
    pub is_lock: bool,
    pub is_new: bool,
    pub own_time: i64,
}

impl WeaponInstance {
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
            grid_index: 0,
            id: self.template_id.clone(),
            count: 1,
            inst: Some(self.to_item_inst()),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WeaponDepot {
    weapons: HashMap<WeaponInstId, WeaponInstance>,
    next_inst_id: u64,
    equipped_weapons: HashMap<u64, WeaponInstId>,
}

impl WeaponDepot {
    pub const DEPOT_TYPE: i32 = 1;

    pub fn new() -> Self {
        Self {
            weapons: HashMap::new(),
            next_inst_id: 1,
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
            inst_id, weapon.template_id
        );
        self.weapons.insert(inst_id, weapon);
        inst_id
    }

    pub fn insert_weapon(&mut self, weapon: WeaponInstance) {
        if weapon.is_equipped() {
            self.equipped_weapons
                .insert(weapon.equip_char_id, weapon.inst_id);
        }
        let v = weapon.inst_id.as_u64();
        if v >= self.next_inst_id {
            self.next_inst_id = v + 1;
        }
        self.weapons.insert(weapon.inst_id, weapon);
    }

    pub fn get(&self, id: WeaponInstId) -> Option<&WeaponInstance> {
        self.weapons.get(&id)
    }
    pub fn get_mut(&mut self, id: WeaponInstId) -> Option<&mut WeaponInstance> {
        self.weapons.get_mut(&id)
    }

    pub fn remove_weapon(&mut self, inst_id: WeaponInstId) -> Result<WeaponInstance> {
        let w = self
            .weapons
            .get(&inst_id)
            .ok_or_else(|| LogicError::NotFound("Weapon not found".into()))?;
        if w.is_equipped() {
            return Err(LogicError::InvalidOperation(
                "Cannot remove equipped weapon".into(),
            ));
        }
        if w.is_lock {
            return Err(LogicError::InvalidOperation(
                "Cannot remove locked weapon".into(),
            ));
        }
        Ok(self.weapons.remove(&inst_id).unwrap())
    }

    pub fn contains(&self, id: WeaponInstId) -> bool {
        self.weapons.contains_key(&id)
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

    pub fn equip_weapon(
        &mut self,
        weapon_inst_id: WeaponInstId,
        char_id: u64,
    ) -> Result<Option<WeaponInstId>> {
        let w = self
            .weapons
            .get(&weapon_inst_id)
            .ok_or_else(|| LogicError::NotFound("Weapon not found".into()))?;
        if w.equip_char_id == char_id {
            return Err(LogicError::InvalidOperation(
                "Weapon already equipped to this character".into(),
            ));
        }
        let prev_char = w.equip_char_id;
        let prev_weapon = self.unequip_from_char(char_id);
        if prev_char != 0 {
            self.equipped_weapons.remove(&prev_char);
            if let Some(w) = self.weapons.get_mut(&weapon_inst_id) {
                w.equip_char_id = 0;
            }
        }
        let w = self.weapons.get_mut(&weapon_inst_id).unwrap();
        w.equip_char_id = char_id;
        self.equipped_weapons.insert(char_id, weapon_inst_id);
        info!(
            "Equipped weapon {} to char {} (prev: {:?})",
            weapon_inst_id, char_id, prev_weapon
        );
        Ok(prev_weapon)
    }

    pub fn unequip_weapon(&mut self, id: WeaponInstId) -> Result<bool> {
        let w = self
            .weapons
            .get_mut(&id)
            .ok_or_else(|| LogicError::NotFound("Weapon not found".into()))?;
        if !w.is_equipped() {
            return Ok(false);
        }
        let char_id = w.equip_char_id;
        w.equip_char_id = 0;
        self.equipped_weapons.remove(&char_id);
        Ok(true)
    }

    fn unequip_from_char(&mut self, char_id: u64) -> Option<WeaponInstId> {
        if let Some(&inst_id) = self.equipped_weapons.get(&char_id) {
            if let Some(w) = self.weapons.get_mut(&inst_id) {
                w.equip_char_id = 0;
            }
            self.equipped_weapons.remove(&char_id);
            Some(inst_id)
        } else {
            None
        }
    }

    pub fn get_equipped_weapon(&self, char_id: u64) -> Option<&WeaponInstance> {
        self.equipped_weapons
            .get(&char_id)
            .and_then(|&id| self.weapons.get(&id))
    }
    pub fn get_equipped_weapon_id(&self, char_id: u64) -> Option<WeaponInstId> {
        self.equipped_weapons.get(&char_id).copied()
    }
    pub fn has_equipped_weapon(&self, char_id: u64) -> bool {
        self.equipped_weapons.contains_key(&char_id)
    }

    pub fn set_lock(&mut self, id: WeaponInstId, is_lock: bool) -> Result<()> {
        self.weapons
            .get_mut(&id)
            .ok_or_else(|| LogicError::NotFound("Weapon not found".into()))
            .map(|w| w.is_lock = is_lock)
    }
    pub fn clear_new_flag(&mut self, id: WeaponInstId) -> Result<()> {
        self.weapons
            .get_mut(&id)
            .ok_or_else(|| LogicError::NotFound("Weapon not found".into()))
            .map(|w| w.is_new = false)
    }

    fn calculate_fodder_exp(
        weapon: &WeaponInstance,
        cfg: Option<&config::tables::weapon::Weapon>,
    ) -> u64 {
        let base = match cfg {
            Some(w) => match w.rarity {
                6 => 5000,
                5 => 3000,
                4 => 1500,
                3 => 800,
                _ => 400,
            },
            None => 400,
        };
        base + (weapon.weapon_lv as f64 * 0.1 * base as f64) as u64
    }

    fn get_breakthrough_required_level(
        &self,
        template_id: &str,
        target: u64,
        assets: &BeyondAssets,
    ) -> Option<u64> {
        let w = assets.weapons.get(template_id)?;
        let t = assets
            .weapons
            .get_breakthrough_template(&w.breakthrough_template_id)?;
        t.list
            .iter()
            .find(|e| e.breakthrough_lv as u64 == target)
            .map(|e| e.breakthrough_show_lv as u64)
    }

    pub fn add_exp(
        &mut self,
        target_inst_id: WeaponInstId,
        fodder_inst_ids: &[WeaponInstId],
        assets: &BeyondAssets,
    ) -> Result<(u64, u64)> {
        let t = self
            .weapons
            .get(&target_inst_id)
            .ok_or_else(|| LogicError::NotFound("Target weapon not found".into()))?;
        if t.is_lock {
            return Err(LogicError::InvalidOperation(
                "Cannot upgrade locked weapon".into(),
            ));
        }
        let tmpl = t.template_id.clone();
        let mut total_exp = 0u64;
        let fodder_count = fodder_inst_ids.len();
        for &fid in fodder_inst_ids {
            if fid == target_inst_id {
                return Err(LogicError::InvalidOperation(
                    "Cannot use weapon as its own fodder".into(),
                ));
            }
            let f = self
                .weapons
                .get(&fid)
                .ok_or_else(|| LogicError::NotFound("Fodder weapon not found".into()))?;
            if f.is_lock {
                return Err(LogicError::InvalidOperation(
                    "Cannot use locked weapon as fodder".into(),
                ));
            }
            if f.is_equipped() {
                return Err(LogicError::InvalidOperation(
                    "Cannot use equipped weapon as fodder".into(),
                ));
            }
            total_exp += Self::calculate_fodder_exp(f, assets.weapons.get(&f.template_id));
        }
        for &fid in fodder_inst_ids {
            self.weapons.remove(&fid);
        }
        let t = self.weapons.get_mut(&target_inst_id).unwrap();
        t.exp += total_exp;
        let new_lv = assets.weapons.weapon_level_from_exp(&tmpl, t.exp);
        let old_lv = t.weapon_lv;
        t.weapon_lv = new_lv;
        info!(
            "Weapon {} +{}exp from {} fodder, lv {}→{}",
            target_inst_id, total_exp, fodder_count, old_lv, new_lv
        );
        Ok((t.exp, t.weapon_lv))
    }

    pub fn breakthrough(&mut self, inst_id: WeaponInstId, assets: &BeyondAssets) -> Result<u64> {
        let w = self
            .weapons
            .get(&inst_id)
            .ok_or_else(|| LogicError::NotFound("Weapon not found".into()))?;
        if w.is_lock {
            return Err(LogicError::InvalidOperation(
                "Cannot breakthrough locked weapon".into(),
            ));
        }
        let tmpl = w.template_id.clone();
        let cur = w.breakthrough_lv;
        let lv = w.weapon_lv;
        let max = assets.weapons.get_max_breakthrough_lv(&tmpl);
        if cur >= max {
            return Err(LogicError::InvalidOperation(
                "Already at max breakthrough".into(),
            ));
        }
        let req = self
            .get_breakthrough_required_level(&tmpl, cur + 1, assets)
            .unwrap_or(1);
        if lv < req {
            return Err(LogicError::InvalidOperation(format!(
                "Level {} below required {}",
                lv, req
            )));
        }
        let w = self.weapons.get_mut(&inst_id).unwrap();
        w.breakthrough_lv += 1;
        info!(
            "Weapon {} breakthrough: {}→{}",
            inst_id, cur, w.breakthrough_lv
        );
        Ok(w.breakthrough_lv)
    }

    fn get_max_refine(cfg: Option<&config::tables::weapon::Weapon>) -> u64 {
        match cfg {
            Some(w) => match w.rarity {
                6 | 5 => 5,
                4 => 4,
                3 => 3,
                _ => 2,
            },
            None => 5,
        }
    }

    pub fn refine(
        &mut self,
        target: WeaponInstId,
        fodder: WeaponInstId,
        assets: &BeyondAssets,
    ) -> Result<u64> {
        let t = self
            .weapons
            .get(&target)
            .ok_or_else(|| LogicError::NotFound("Target weapon not found".into()))?;
        if t.is_lock {
            return Err(LogicError::InvalidOperation(
                "Cannot refine locked weapon".into(),
            ));
        }
        let f = self
            .weapons
            .get(&fodder)
            .ok_or_else(|| LogicError::NotFound("Fodder weapon not found".into()))?;
        if f.is_lock {
            return Err(LogicError::InvalidOperation(
                "Cannot use locked weapon as material".into(),
            ));
        }
        if f.is_equipped() {
            return Err(LogicError::InvalidOperation(
                "Cannot use equipped weapon as material".into(),
            ));
        }
        if t.template_id != f.template_id {
            return Err(LogicError::InvalidOperation(
                "Refinement requires same weapon type".into(),
            ));
        }
        let tmpl = t.template_id.clone();
        if t.refine_lv >= Self::get_max_refine(assets.weapons.get(&tmpl)) {
            return Err(LogicError::InvalidOperation(
                "Already at max refinement".into(),
            ));
        }
        self.weapons.remove(&fodder);
        let t = self.weapons.get_mut(&target).unwrap();
        t.refine_lv += 1;
        info!(
            "Weapon {} refined: {}→{}",
            target,
            t.refine_lv - 1,
            t.refine_lv
        );
        Ok(t.refine_lv)
    }

    pub fn attach_gem(
        &mut self,
        weapon_inst_id: WeaponInstId,
        gem_inst_id: u64,
    ) -> Result<Option<u64>> {
        let w = self
            .weapons
            .get_mut(&weapon_inst_id)
            .ok_or_else(|| LogicError::NotFound("Weapon not found".into()))?;
        if w.is_lock {
            return Err(LogicError::InvalidOperation(
                "Cannot modify locked weapon".into(),
            ));
        }
        let prev = if w.attach_gem_id != 0 {
            Some(w.attach_gem_id)
        } else {
            None
        };
        w.attach_gem_id = gem_inst_id;
        Ok(prev)
    }

    pub fn detach_gem(&mut self, weapon_inst_id: WeaponInstId) -> Result<u64> {
        let w = self
            .weapons
            .get_mut(&weapon_inst_id)
            .ok_or_else(|| LogicError::NotFound("Weapon not found".into()))?;
        if w.is_lock {
            return Err(LogicError::InvalidOperation(
                "Cannot modify locked weapon".into(),
            ));
        }
        if w.attach_gem_id == 0 {
            return Err(LogicError::InvalidOperation(
                "Weapon has no attached gem".into(),
            ));
        }
        let gem_id = w.attach_gem_id;
        w.attach_gem_id = 0;
        Ok(gem_id)
    }

    pub fn to_depot_sync(&self) -> ScdItemDepot {
        ScdItemDepot {
            stackable_items: HashMap::new(),
            inst_list: self.weapons.values().map(|w| w.to_item_grid()).collect(),
        }
    }
    pub fn to_weapon_modify(&self, id: WeaponInstId) -> Option<ScdItemGrid> {
        self.weapons.get(&id).map(|w| w.to_item_grid())
    }
    pub fn to_weapon_delete(id: WeaponInstId) -> u64 {
        id.as_u64()
    }
    pub fn to_add_exp_sc(&self, id: WeaponInstId) -> Option<ScWeaponAddExp> {
        self.weapons.get(&id).map(|w| ScWeaponAddExp {
            weaponid: id.as_u64(),
            new_exp: w.exp,
            weapon_lv: w.weapon_lv,
        })
    }
    pub fn to_breakthrough_sc(&self, id: WeaponInstId) -> Option<ScWeaponBreakthrough> {
        self.weapons.get(&id).map(|w| ScWeaponBreakthrough {
            weaponid: id.as_u64(),
            breakthrough_lv: w.breakthrough_lv,
        })
    }
    pub fn to_attach_gem_sc(
        &self,
        wid: WeaponInstId,
        detached_gem: Option<u64>,
        detached_weapon: Option<u64>,
    ) -> Option<ScWeaponAttachGem> {
        self.weapons.get(&wid).map(|w| ScWeaponAttachGem {
            weaponid: wid.as_u64(),
            gemid: w.attach_gem_id,
            detach_gemid: detached_gem.unwrap_or(0),
            detach_gem_weaponid: detached_weapon.unwrap_or(0),
        })
    }
    pub fn to_detach_gem_sc(&self, wid: WeaponInstId, gem_id: u64) -> ScWeaponDetachGem {
        ScWeaponDetachGem {
            weaponid: wid.as_u64(),
            detach_gemid: gem_id,
        }
    }
    pub fn to_weapon_puton_sc(
        &self,
        charid: u64,
        wid: WeaponInstId,
        off_weapon: Option<u64>,
        put_off_char: Option<u64>,
    ) -> ScWeaponPuton {
        ScWeaponPuton {
            charid,
            weaponid: wid.as_u64(),
            offweaponid: off_weapon.unwrap_or(0),
            put_off_charid: put_off_char.unwrap_or(0),
        }
    }
    pub fn get_equipped_templates_for_chars(&self, char_ids: &[u64]) -> HashMap<u64, String> {
        char_ids
            .iter()
            .filter_map(|&c| {
                self.get_equipped_weapon(c)
                    .map(|w| (c, w.template_id.clone()))
            })
            .collect()
    }

    pub fn init_default_weapons_for_chars(
        &mut self,
        char_template_ids: &[(u64, String)],
        assets: &BeyondAssets,
    ) -> Vec<(u64, WeaponInstId)> {
        let mut equipped = Vec::new();
        let own_time = now_ms() as i64;
        for (char_id, char_template_id) in char_template_ids {
            if self.has_equipped_weapon(*char_id) {
                continue;
            }
            let char_data = match assets.characters.get(char_template_id) {
                Some(d) => d,
                None => {
                    warn!("Character template not found: {}", char_template_id);
                    continue;
                }
            };
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
            let inst_id = self.add_weapon(weapon.weapon_id.clone(), own_time);
            if self.equip_weapon(inst_id, *char_id).is_ok() {
                equipped.push((*char_id, inst_id));
                info!(
                    "Initialized default weapon {} for char {}",
                    weapon.weapon_id, char_id
                );
            }
        }
        equipped
    }

    pub fn validate_equipped_weapons(&mut self) {
        let mut to_fix: Vec<(u64, WeaponInstId)> = Vec::new();
        let mut orphaned: Vec<WeaponInstId> = Vec::new();
        for (&char_id, &inst_id) in &self.equipped_weapons {
            if let Some(w) = self.weapons.get(&inst_id) {
                if w.equip_char_id != char_id {
                    to_fix.push((char_id, inst_id));
                }
            } else {
                to_fix.push((char_id, inst_id));
            }
        }
        for (&inst_id, w) in &self.weapons {
            if w.is_equipped() && !self.equipped_weapons.contains_key(&w.equip_char_id) {
                orphaned.push(inst_id);
            }
        }
        for id in orphaned {
            if let Some(w) = self.weapons.get_mut(&id) {
                w.equip_char_id = 0;
            }
        }
        for (char_id, _) in to_fix {
            self.equipped_weapons.remove(&char_id);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GemInstance {
    pub inst_id: GemInstId,
    pub template_id: String,
    pub craft_slot: CraftShowingType,
    /// 0 = not socketed.
    pub attach_weapon_id: u64,
    pub is_lock: bool,
    pub is_new: bool,
    pub own_time: i64,
}

impl GemInstance {
    pub fn new(
        inst_id: GemInstId,
        template_id: String,
        craft_slot: CraftShowingType,
        own_time: i64,
    ) -> Self {
        Self {
            inst_id,
            template_id,
            craft_slot,
            attach_weapon_id: 0,
            is_lock: false,
            is_new: true,
            own_time,
        }
    }

    pub fn is_socketed(&self) -> bool {
        self.attach_weapon_id != 0
    }

    fn to_item_grid(&self) -> ScdItemGrid {
        ScdItemGrid {
            grid_index: 0,
            id: self.template_id.clone(),
            count: 1,
            inst: Some(ItemInst {
                inst_id: self.inst_id.as_u64(),
                is_lock: self.is_lock,
                is_new: self.is_new,
                // GemData carries the full instance identity the client needs to
                // show the gem in the valuable depot tab.
                inst_impl: Some(InstImpl::Gem(GemData {
                    gem_id: self.inst_id.as_u64(),
                    template_id: self.template_id.clone(),
                    total_cost: 0,
                    terms: vec![],
                    weapon_id: self.attach_weapon_id,
                })),
            }),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GemDepot {
    gems: HashMap<GemInstId, GemInstance>,
    next_inst_id: u64,
}

impl GemDepot {
    pub const DEPOT_TYPE: i32 = 2;
    pub fn new() -> Self {
        Self {
            gems: HashMap::new(),
            next_inst_id: 1,
        }
    }

    fn alloc_inst_id(&mut self) -> GemInstId {
        let id = GemInstId::new(self.next_inst_id);
        self.next_inst_id += 1;
        id
    }

    pub fn add_gem(
        &mut self,
        template_id: String,
        craft_slot: CraftShowingType,
        own_time: i64,
    ) -> GemInstId {
        let inst_id = self.alloc_inst_id();
        let gem = GemInstance::new(inst_id, template_id, craft_slot, own_time);
        debug!(
            "Adding gem: inst_id={}, template_id={}",
            inst_id, gem.template_id
        );
        self.gems.insert(inst_id, gem);
        inst_id
    }

    pub fn insert(&mut self, gem: GemInstance) {
        let v = gem.inst_id.as_u64();
        if v >= self.next_inst_id {
            self.next_inst_id = v + 1;
        }
        self.gems.insert(gem.inst_id, gem);
    }

    pub fn get(&self, id: GemInstId) -> Option<&GemInstance> {
        self.gems.get(&id)
    }
    pub fn get_mut(&mut self, id: GemInstId) -> Option<&mut GemInstance> {
        self.gems.get_mut(&id)
    }

    pub fn remove(&mut self, id: GemInstId) -> Result<GemInstance> {
        let g = self
            .gems
            .get(&id)
            .ok_or_else(|| LogicError::NotFound("Gem not found".into()))?;
        if g.is_socketed() {
            return Err(LogicError::InvalidOperation(
                "Cannot remove socketed gem".into(),
            ));
        }
        if g.is_lock {
            return Err(LogicError::InvalidOperation(
                "Cannot remove locked gem".into(),
            ));
        }
        Ok(self.gems.remove(&id).unwrap())
    }

    pub fn set_lock(&mut self, id: GemInstId, lock: bool) -> Result<()> {
        self.gems
            .get_mut(&id)
            .ok_or_else(|| LogicError::NotFound("Gem not found".into()))
            .map(|g| g.is_lock = lock)
    }
    pub fn clear_new_flag(&mut self, id: GemInstId) -> Result<()> {
        self.gems
            .get_mut(&id)
            .ok_or_else(|| LogicError::NotFound("Gem not found".into()))
            .map(|g| g.is_new = false)
    }
    pub(crate) fn set_socket(&mut self, id: GemInstId, weapon_id: u64) -> Result<()> {
        self.gems
            .get_mut(&id)
            .ok_or_else(|| LogicError::NotFound("Gem not found".into()))
            .map(|g| g.attach_weapon_id = weapon_id)
    }
    pub(crate) fn clear_socket(&mut self, id: GemInstId) -> Result<()> {
        self.gems
            .get_mut(&id)
            .ok_or_else(|| LogicError::NotFound("Gem not found".into()))
            .map(|g| g.attach_weapon_id = 0)
    }

    pub fn contains(&self, id: GemInstId) -> bool {
        self.gems.contains_key(&id)
    }
    pub fn len(&self) -> usize {
        self.gems.len()
    }
    pub fn is_empty(&self) -> bool {
        self.gems.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = &GemInstance> {
        self.gems.values()
    }

    pub fn to_depot_sync(&self) -> ScdItemDepot {
        ScdItemDepot {
            stackable_items: HashMap::new(),
            inst_list: self.gems.values().map(|g| g.to_item_grid()).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquipInstance {
    pub inst_id: EquipInstId,
    pub template_id: String,
    /// `EquipHead`, `EquipBody`, or `EquipRing`.
    pub slot: CraftShowingType,
    pub equip_char_id: u64,
    pub is_lock: bool,
    pub is_new: bool,
    pub own_time: i64,
}

impl EquipInstance {
    pub fn new(
        inst_id: EquipInstId,
        template_id: String,
        slot: CraftShowingType,
        own_time: i64,
    ) -> Self {
        Self {
            inst_id,
            template_id,
            slot,
            equip_char_id: 0,
            is_lock: false,
            is_new: true,
            own_time,
        }
    }

    pub fn is_equipped(&self) -> bool {
        self.equip_char_id != 0
    }

    fn to_item_grid(&self) -> ScdItemGrid {
        ScdItemGrid {
            grid_index: 0,
            id: self.template_id.clone(),
            count: 1,
            inst: Some(ItemInst {
                inst_id: self.inst_id.as_u64(),
                is_lock: self.is_lock,
                is_new: self.is_new,
                // EquipData carries instance identity and equipped-character binding.
                // attrs is empty, the client derives stats from the config table
                // using templateid; we don't need to send them here.(probably)
                inst_impl: Some(InstImpl::Equip(EquipData {
                    equipid: self.inst_id.as_u64(),
                    templateid: self.template_id.clone(),
                    equip_char_id: self.equip_char_id,
                    attrs: vec![],
                })),
            }),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EquipDepot {
    pieces: HashMap<EquipInstId, EquipInstance>,
    next_inst_id: u64,
    equipped_by_char: HashMap<u64, HashMap<CraftShowingType, EquipInstId>>,
}

impl EquipDepot {
    pub const DEPOT_TYPE: i32 = 3;
    pub fn new() -> Self {
        Self {
            pieces: HashMap::new(),
            next_inst_id: 1,
            equipped_by_char: HashMap::new(),
        }
    }

    fn alloc_inst_id(&mut self) -> EquipInstId {
        let id = EquipInstId::new(self.next_inst_id);
        self.next_inst_id += 1;
        id
    }

    pub fn add_equip(
        &mut self,
        template_id: String,
        slot: CraftShowingType,
        own_time: i64,
    ) -> EquipInstId {
        let inst_id = self.alloc_inst_id();
        let piece = EquipInstance::new(inst_id, template_id, slot, own_time);
        debug!(
            "Adding equip: inst_id={}, template_id={}, slot={:?}",
            inst_id, piece.template_id, piece.slot
        );
        self.pieces.insert(inst_id, piece);
        inst_id
    }

    pub fn insert(&mut self, piece: EquipInstance) {
        if piece.is_equipped() {
            self.equipped_by_char
                .entry(piece.equip_char_id)
                .or_default()
                .insert(piece.slot, piece.inst_id);
        }
        let v = piece.inst_id.as_u64();
        if v >= self.next_inst_id {
            self.next_inst_id = v + 1;
        }
        self.pieces.insert(piece.inst_id, piece);
    }

    pub fn get(&self, id: EquipInstId) -> Option<&EquipInstance> {
        self.pieces.get(&id)
    }
    pub fn get_mut(&mut self, id: EquipInstId) -> Option<&mut EquipInstance> {
        self.pieces.get_mut(&id)
    }

    pub fn equip(
        &mut self,
        piece_inst_id: EquipInstId,
        char_id: u64,
    ) -> Result<Option<EquipInstId>> {
        let p = self
            .pieces
            .get(&piece_inst_id)
            .ok_or_else(|| LogicError::NotFound("Equip piece not found".into()))?;
        if p.equip_char_id == char_id {
            return Err(LogicError::InvalidOperation(
                "Already equipped to this character".into(),
            ));
        }
        let slot = p.slot;
        let prev = self
            .equipped_by_char
            .get(&char_id)
            .and_then(|s| s.get(&slot))
            .copied();
        if let Some(prev_id) = prev {
            if let Some(p) = self.pieces.get_mut(&prev_id) {
                p.equip_char_id = 0;
            }
            self.equipped_by_char
                .entry(char_id)
                .or_default()
                .remove(&slot);
        }
        let prev_owner = self.pieces.get(&piece_inst_id).unwrap().equip_char_id;
        if prev_owner != 0 {
            self.equipped_by_char
                .entry(prev_owner)
                .or_default()
                .remove(&slot);
        }
        let p = self.pieces.get_mut(&piece_inst_id).unwrap();
        p.equip_char_id = char_id;
        self.equipped_by_char
            .entry(char_id)
            .or_default()
            .insert(slot, piece_inst_id);
        info!(
            "Equipped piece {} (slot {:?}) to char {}",
            piece_inst_id, slot, char_id
        );
        Ok(prev)
    }

    pub fn unequip(&mut self, id: EquipInstId) -> Result<bool> {
        let p = self
            .pieces
            .get_mut(&id)
            .ok_or_else(|| LogicError::NotFound("Equip piece not found".into()))?;
        if !p.is_equipped() {
            return Ok(false);
        }
        let char_id = p.equip_char_id;
        let slot = p.slot;
        p.equip_char_id = 0;
        if let Some(slots) = self.equipped_by_char.get_mut(&char_id) {
            slots.remove(&slot);
        }
        Ok(true)
    }

    pub fn remove(&mut self, id: EquipInstId) -> Result<EquipInstance> {
        let p = self
            .pieces
            .get(&id)
            .ok_or_else(|| LogicError::NotFound("Equip piece not found".into()))?;
        if p.is_equipped() {
            return Err(LogicError::InvalidOperation(
                "Cannot remove equipped piece".into(),
            ));
        }
        if p.is_lock {
            return Err(LogicError::InvalidOperation(
                "Cannot remove locked piece".into(),
            ));
        }
        Ok(self.pieces.remove(&id).unwrap())
    }

    pub fn set_lock(&mut self, id: EquipInstId, lock: bool) -> Result<()> {
        self.pieces
            .get_mut(&id)
            .ok_or_else(|| LogicError::NotFound("Equip piece not found".into()))
            .map(|p| p.is_lock = lock)
    }

    pub fn clear_new_flag(&mut self, id: EquipInstId) -> Result<()> {
        self.pieces
            .get_mut(&id)
            .ok_or_else(|| LogicError::NotFound("Equip piece not found".into()))
            .map(|p| p.is_new = false)
    }

    pub fn get_in_slot(&self, char_id: u64, slot: CraftShowingType) -> Option<&EquipInstance> {
        self.equipped_by_char
            .get(&char_id)?
            .get(&slot)
            .and_then(|&id| self.pieces.get(&id))
    }

    pub fn equipped_slots(
        &self,
        char_id: u64,
    ) -> impl Iterator<Item = (CraftShowingType, &EquipInstance)> {
        self.equipped_by_char
            .get(&char_id)
            .into_iter()
            .flat_map(|slots| {
                slots
                    .iter()
                    .filter_map(|(&slot, &id)| self.pieces.get(&id).map(move |p| (slot, p)))
            })
    }

    pub fn contains(&self, id: EquipInstId) -> bool {
        self.pieces.contains_key(&id)
    }
    pub fn len(&self) -> usize {
        self.pieces.len()
    }
    pub fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = &EquipInstance> {
        self.pieces.values()
    }

    pub fn to_depot_sync(&self) -> ScdItemDepot {
        ScdItemDepot {
            stackable_items: HashMap::new(),
            inst_list: self.pieces.values().map(|p| p.to_item_grid()).collect(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StackableDepot {
    counts: HashMap<String, u32>,
    depot_type: i32,
}

impl StackableDepot {
    pub fn new(depot_type: i32) -> Self {
        Self {
            counts: HashMap::new(),
            depot_type,
        }
    }

    pub fn add(&mut self, template_id: &str, count: u32) -> u32 {
        let e = self.counts.entry(template_id.to_owned()).or_insert(0);
        *e = e.saturating_add(count);
        debug!(
            "StackableDepot({}): +{} {} → {}",
            self.depot_type, count, template_id, e
        );
        *e
    }

    pub fn consume(&mut self, template_id: &str, count: u32) -> Result<u32> {
        let cur = self.counts.get(template_id).copied().unwrap_or(0);
        if cur < count {
            return Err(LogicError::Insufficient {
                item_id: template_id.to_string(),
                have: cur,
                need: count,
            });
        }
        let rem = cur - count;
        if rem == 0 {
            self.counts.remove(template_id);
        } else {
            *self.counts.get_mut(template_id).unwrap() = rem;
        }
        Ok(rem)
    }

    #[inline]
    pub fn count_of(&self, id: &str) -> u32 {
        self.counts.get(id).copied().unwrap_or(0)
    }
    #[inline]
    pub fn has(&self, id: &str, count: u32) -> bool {
        self.count_of(id) >= count
    }

    pub fn set(&mut self, id: &str, count: u32) {
        if count == 0 {
            self.counts.remove(id);
        } else {
            *self.counts.entry(id.to_owned()).or_insert(0) = count;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }
    pub fn len(&self) -> usize {
        self.counts.len()
    }
    pub fn iter(&self) -> impl Iterator<Item = (&str, u32)> {
        self.counts.iter().map(|(k, &v)| (k.as_str(), v))
    }

    pub fn to_depot_sync(&self) -> ScdItemDepot {
        ScdItemDepot {
            stackable_items: self
                .counts
                .iter()
                .map(|(k, &v)| (k.clone(), v as i64))
                .collect(),
            inst_list: Vec::new(),
        }
    }

    pub fn to_bag_grids(&self, start_index: &mut i32) -> Vec<ScdItemGrid> {
        let mut out = Vec::with_capacity(self.counts.len());
        for (id, &count) in &self.counts {
            out.push(ScdItemGrid {
                grid_index: *start_index,
                id: id.clone(),
                count: count as i64,
                inst: None,
            });
            *start_index += 1;
        }
        out
    }

    /// Build a `ScdItemDepotModify` reflecting a batch of consumed items.
    /// `consumed` is a map of template_id → count that was removed.
    pub fn consumed_modify(consumed: &HashMap<String, u32>) -> ScdItemDepotModify {
        ScdItemDepotModify {
            items: consumed
                .iter()
                .map(|(k, &v)| (k.clone(), -(v as i64)))
                .collect(),
            inst_list: vec![],
            del_inst_list: vec![],
        }
    }
}

const STARTER_SPECIAL_COUNT: u32 = 999;
const STARTER_MISSION_COUNT: u32 = 999;
const STARTER_FACTORY_COUNT: u32 = 9_999;
const BAG_GRID_LIMIT: i32 = 1_500;

/// Starter wallet amounts sent via `ScSyncWallet` on every login.
/// These are not persisted, the emulator gives them unconditionally.
pub const WALLET_GOLD_AMOUNT: u64 = 9_999_999;
pub const WALLET_DIAMOND_AMOUNT: u64 = 9_999_999;

/// Unified inventory for all item depots (weapons, gems, equips, stackables).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ItemManager {
    pub weapons: WeaponDepot,
    pub gems: GemDepot,
    pub equips: EquipDepot,
    pub special_items: StackableDepot,
    pub mission_items: StackableDepot,
    pub factory_items: StackableDepot,
}

impl ItemManager {
    pub fn new() -> Self {
        Self {
            weapons: WeaponDepot::new(),
            gems: GemDepot::new(),
            equips: EquipDepot::new(),
            special_items: StackableDepot::new(4),
            mission_items: StackableDepot::new(5),
            factory_items: StackableDepot::new(6),
        }
    }

    pub fn init_for_new_player(assets: &BeyondAssets, own_time: i64) -> Self {
        let mut mgr = Self::new();

        for cfg in assets.items.iter_by_depot(ItemDepotType::WeaponGem) {
            let craft_slot = match &cfg.kind {
                ItemKind::WeaponGem { craft_slot } => *craft_slot,
                _ => CraftShowingType::WeaponGemNormal,
            };
            mgr.gems.add_gem(cfg.id.clone(), craft_slot, own_time);
        }

        for cfg in assets.items.iter_by_depot(ItemDepotType::Equip) {
            let slot = match &cfg.kind {
                ItemKind::Equip { slot } => *slot,
                _ => CraftShowingType::None,
            };
            mgr.equips.add_equip(cfg.id.clone(), slot, own_time);
        }

        for cfg in assets.items.iter_by_depot(ItemDepotType::SpecialItem) {
            mgr.special_items.add(&cfg.id, STARTER_SPECIAL_COUNT);
        }
        for cfg in assets.items.iter_by_depot(ItemDepotType::MissionItem) {
            mgr.mission_items.add(&cfg.id, STARTER_MISSION_COUNT);
        }
        for cfg in assets.items.iter_by_depot(ItemDepotType::Factory) {
            mgr.factory_items.add(&cfg.id, STARTER_FACTORY_COUNT);
        }

        info!(
            "init_for_new_player: gems={}, equips={}, special={}, mission={}, factory={}",
            mgr.gems.len(),
            mgr.equips.len(),
            mgr.special_items.len(),
            mgr.mission_items.len(),
            mgr.factory_items.len(),
        );
        mgr
    }

    pub fn add_stackable(
        &mut self,
        depot_type: ItemDepotType,
        template_id: &str,
        count: u32,
    ) -> Result<u32> {
        self.stackable_depot_mut(depot_type)
            .ok_or_else(|| {
                LogicError::InvalidOperation(format!("Depot {:?} is instanced", depot_type))
            })
            .map(|d| d.add(template_id, count))
    }

    pub fn consume_stackable(
        &mut self,
        depot_type: ItemDepotType,
        template_id: &str,
        count: u32,
    ) -> Result<u32> {
        self.stackable_depot_mut(depot_type)
            .ok_or_else(|| {
                LogicError::InvalidOperation(format!("Depot {:?} is instanced", depot_type))
            })
            .and_then(|d| d.consume(template_id, count))
    }

    pub fn count_of(&self, depot_type: ItemDepotType, template_id: &str) -> u32 {
        self.stackable_depot(depot_type)
            .map(|d| d.count_of(template_id))
            .unwrap_or(0)
    }

    pub fn has_stackable(&self, depot_type: ItemDepotType, template_id: &str, count: u32) -> bool {
        self.stackable_depot(depot_type)
            .map(|d| d.has(template_id, count))
            .unwrap_or(false)
    }

    pub fn socket_gem(
        &mut self,
        weapon_inst_id: WeaponInstId,
        gem_inst_id: GemInstId,
    ) -> Result<Option<GemInstId>> {
        self.weapons
            .get(weapon_inst_id)
            .ok_or_else(|| LogicError::NotFound("Weapon not found".into()))?;
        let gem = self
            .gems
            .get(gem_inst_id)
            .ok_or_else(|| LogicError::NotFound("Gem not found".into()))?;
        if gem.is_socketed() {
            return Err(LogicError::InvalidOperation("Gem already socketed".into()));
        }
        let prev_raw = self.weapons.get(weapon_inst_id).unwrap().attach_gem_id;
        let prev_gem = if prev_raw != 0 {
            let id = GemInstId::new(prev_raw);
            self.gems.clear_socket(id)?;
            Some(id)
        } else {
            None
        };
        self.weapons
            .attach_gem(weapon_inst_id, gem_inst_id.as_u64())?;
        self.gems.set_socket(gem_inst_id, weapon_inst_id.as_u64())?;
        info!(
            "Socketed gem {} into weapon {}",
            gem_inst_id, weapon_inst_id
        );
        Ok(prev_gem)
    }

    pub fn unsocket_gem(&mut self, weapon_inst_id: WeaponInstId) -> Result<GemInstId> {
        let raw = self.weapons.detach_gem(weapon_inst_id)?;
        let gem_id = GemInstId::new(raw);
        self.gems.clear_socket(gem_id)?;
        info!("Unsocketed gem {} from weapon {}", gem_id, weapon_inst_id);
        Ok(gem_id)
    }

    pub fn build_full_bag_sync(&self, assets: &BeyondAssets) -> ScItemBagSync {
        let mut depot = HashMap::new();
        depot.insert(WeaponDepot::DEPOT_TYPE, self.weapons.to_depot_sync());
        depot.insert(GemDepot::DEPOT_TYPE, self.gems.to_depot_sync());
        depot.insert(EquipDepot::DEPOT_TYPE, self.equips.to_depot_sync());
        depot.insert(4, self.special_items.to_depot_sync());
        depot.insert(5, self.mission_items.to_depot_sync());

        let factory_depot = Some(self.factory_items.to_depot_sync());

        let bag = {
            let mut idx: i32 = 0;
            let mut grids: Vec<ScdItemGrid> = Vec::new();
            grids.extend(self.special_items.to_bag_grids(&mut idx));
            grids.extend(self.mission_items.to_bag_grids(&mut idx));
            grids.extend(self.factory_items.to_bag_grids(&mut idx));
            Some(ScdItemBag {
                grid_limit: BAG_GRID_LIMIT,
                grids,
            })
        };

        let cannot_destroy: HashMap<String, bool> = assets
            .items
            .iter()
            .filter(|cfg| !cfg.can_discard)
            .map(|cfg| (cfg.id.clone(), true))
            .collect();

        let use_blackboard = Some(ScdItemUseBlackboard {
            last_use_time: HashMap::new(),
        });

        ScItemBagSync {
            depot,
            bag,
            factory_depot,
            cannot_destroy,
            use_blackboard,
        }
    }

    pub fn sync_depot(&self, depot_type: ItemDepotType) -> Option<ScdItemDepot> {
        match depot_type {
            ItemDepotType::Weapon => Some(self.weapons.to_depot_sync()),
            ItemDepotType::WeaponGem => Some(self.gems.to_depot_sync()),
            ItemDepotType::Equip => Some(self.equips.to_depot_sync()),
            ItemDepotType::SpecialItem => Some(self.special_items.to_depot_sync()),
            ItemDepotType::MissionItem => Some(self.mission_items.to_depot_sync()),
            ItemDepotType::Factory => Some(self.factory_items.to_depot_sync()),
            ItemDepotType::Invalid => None,
        }
    }

    fn stackable_depot(&self, t: ItemDepotType) -> Option<&StackableDepot> {
        match t {
            ItemDepotType::SpecialItem => Some(&self.special_items),
            ItemDepotType::MissionItem => Some(&self.mission_items),
            ItemDepotType::Factory => Some(&self.factory_items),
            _ => None,
        }
    }

    fn stackable_depot_mut(&mut self, t: ItemDepotType) -> Option<&mut StackableDepot> {
        match t {
            ItemDepotType::SpecialItem => Some(&mut self.special_items),
            ItemDepotType::MissionItem => Some(&mut self.mission_items),
            ItemDepotType::Factory => Some(&mut self.factory_items),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_lifecycle() {
        let mut d = WeaponDepot::new();
        let id = d.add_weapon("wpn_test".into(), 0);
        d.equip_weapon(id, 1001).unwrap();
        assert!(d.get(id).unwrap().is_equipped());
        d.unequip_weapon(id).unwrap();
        d.remove_weapon(id).unwrap();
        assert!(!d.contains(id));
    }

    #[test]
    fn gem_inst_impl_populated() {
        let mut d = GemDepot::new();
        let id = d.add_gem(
            "item_gem_0007_rarity4".into(),
            CraftShowingType::WeaponGemNormal,
            0,
        );
        let grid = d.gems[&id].to_item_grid();
        match grid.inst.unwrap().inst_impl.unwrap() {
            InstImpl::Gem(g) => {
                assert_eq!(g.gem_id, id.as_u64());
                assert_eq!(g.template_id, "item_gem_0007_rarity4");
                assert_eq!(g.weapon_id, 0);
            }
            other => panic!("Expected Gem variant, got {:?}", other),
        }
    }

    #[test]
    fn gem_weapon_id_reflects_socket() {
        let mut d = GemDepot::new();
        let id = d.add_gem(
            "item_gem_0007_rarity4".into(),
            CraftShowingType::WeaponGemNormal,
            0,
        );
        d.set_socket(id, 42).unwrap();
        let grid = d.gems[&id].to_item_grid();
        match grid.inst.unwrap().inst_impl.unwrap() {
            InstImpl::Gem(g) => assert_eq!(g.weapon_id, 42),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn equip_inst_impl_populated() {
        let mut d = EquipDepot::new();
        let id = d.add_equip(
            "item_unit_t1_parts_body_01".into(),
            CraftShowingType::EquipBody,
            0,
        );
        let grid = d.pieces[&id].to_item_grid();
        match grid.inst.unwrap().inst_impl.unwrap() {
            InstImpl::Equip(e) => {
                assert_eq!(e.equipid, id.as_u64());
                assert_eq!(e.templateid, "item_unit_t1_parts_body_01");
                assert_eq!(e.equip_char_id, 0);
            }
            other => panic!("Expected Equip variant, got {:?}", other),
        }
    }

    #[test]
    fn equip_char_id_reflects_equipped_state() {
        let mut d = EquipDepot::new();
        let id = d.add_equip("item_body".into(), CraftShowingType::EquipBody, 0);
        d.equip(id, 1001).unwrap();
        let grid = d.pieces[&id].to_item_grid();
        match grid.inst.unwrap().inst_impl.unwrap() {
            InstImpl::Equip(e) => assert_eq!(e.equip_char_id, 1001),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn equip_slot_displacement() {
        let mut d = EquipDepot::new();
        let a = d.add_equip("body_a".into(), CraftShowingType::EquipBody, 0);
        let b = d.add_equip("body_b".into(), CraftShowingType::EquipBody, 0);
        d.equip(a, 1001).unwrap();
        let displaced = d.equip(b, 1001).unwrap();
        assert_eq!(displaced, Some(a));
        assert_eq!(
            d.get_in_slot(1001, CraftShowingType::EquipBody)
                .unwrap()
                .inst_id,
            b
        );
    }

    #[test]
    fn stackable_proto_i64() {
        let mut d = StackableDepot::new(6);
        d.add("item_iron_cmpt", 9_999);
        assert_eq!(
            *d.to_depot_sync()
                .stackable_items
                .get("item_iron_cmpt")
                .unwrap(),
            9_999i64
        );
    }

    #[test]
    fn consumed_modify_negative() {
        let mut consumed = HashMap::new();
        consumed.insert("item_expcard_2_1".to_string(), 5u32);
        let modify = StackableDepot::consumed_modify(&consumed);
        assert_eq!(*modify.items.get("item_expcard_2_1").unwrap(), -5i64);
    }

    #[test]
    fn item_manager_socket_roundtrip() {
        let mut mgr = ItemManager::new();
        let wpn = mgr.weapons.add_weapon("wpn_0002".into(), 0);
        let gem = mgr.gems.add_gem(
            "item_gem_0007_rarity4".into(),
            CraftShowingType::WeaponGemNormal,
            0,
        );
        assert_eq!(mgr.socket_gem(wpn, gem).unwrap(), None);
        let gem2 = mgr.gems.add_gem(
            "item_gem_0015_rarity4".into(),
            CraftShowingType::WeaponGemNormal,
            0,
        );
        assert_eq!(mgr.socket_gem(wpn, gem2).unwrap(), Some(gem));
        assert!(!mgr.gems.get(gem).unwrap().is_socketed());
        assert_eq!(mgr.unsocket_gem(wpn).unwrap(), gem2);
    }
}
