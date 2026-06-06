use crate::error::{LogicError, Result};
use crate::item::{
    ConsumedItems, ItemManager, WeaponAttachGemArgs, WeaponDetachGemArgs, WeaponPutonArgs,
};
use crate::traits::{KeyedContainerExt, PendingChanges};
use common::time::now_ms;
use config::BeyondAssets;
use perlica_proto::{
    AttrInfo, BattleInfo, CharInfo, CharTeamInfo, CharTeamMemberInfo, ScCharSyncStatus,
    ScItemBagSync, ScSyncAttr, ScSyncCharBagInfo, ScWeaponAddExp, ScWeaponAttachGem,
    ScWeaponBreakthrough, ScWeaponDetachGem, ScWeaponPuton, WeaponData,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::enums::AttributeType;
use crate::item::{WeaponDepot, WeaponInstId, WeaponInstance};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct CharIndex(u64);

impl CharIndex {
    pub fn object_id(self) -> u64 {
        self.0 + 1
    }
    pub fn from_object_id(id: u64) -> Self {
        Self(id - 1)
    }
    pub fn from_usize(idx: usize) -> Self {
        Self(idx as u64)
    }
    pub fn as_usize(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TeamSlot {
    #[default]
    Empty,
    Occupied(CharIndex),
}

impl TeamSlot {
    pub fn char_index(&self) -> Option<CharIndex> {
        match self {
            TeamSlot::Occupied(idx) => Some(*idx),
            TeamSlot::Empty => None,
        }
    }

    pub fn object_id(&self) -> Option<u64> {
        self.char_index().map(|idx| idx.object_id())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Team {
    pub name: String,
    pub char_team: [TeamSlot; 4],
    pub leader_index: CharIndex,
}

impl Team {
    pub const SLOTS_COUNT: usize = 4;
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Char {
    pub template_id: String,
    pub level: i32,
    pub exp: i32,
    pub break_stage: u32,
    pub is_dead: bool,
    pub hp: f64,
    pub ultimate_sp: f32,
    // cached; always read from weapon_depot.get_equipped_weapon(char_obj_id)
    #[serde(skip)]
    pub cached_weapon_inst_id: Option<WeaponInstId>,
    pub own_time: i64,
    pub skill_levels: HashMap<String, u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Meta {
    pub curr_team_index: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CharBag {
    pub teams: Vec<Team>,
    pub chars: Vec<Char>,
    pub meta: Meta,
    pub item_manager: ItemManager,
    #[serde(skip)]
    pending_chars: PendingChanges<usize>,
    #[serde(skip)]
    pending_teams: PendingChanges<usize>,
    #[serde(skip)]
    meta_dirty: bool,
}

#[derive(Debug, Clone)]
pub struct CharSyncState {
    pub objid: u64,
    pub template_id: String,
    pub level: i32,
    pub exp: i32,
    pub break_stage: u32,
    pub hp: f64,
    pub ultimate_sp: f32,
    pub weapon_id: u64,
    pub own_time: i64,
    pub is_dead: bool,
    pub normal_skill: String,
    pub skill_levels: Vec<SkillLevelState>,
}

#[derive(Debug, Clone)]
pub struct SkillLevelState {
    pub skill_id: String,
    pub skill_level: i32,
    pub skill_max_level: i32,
}

#[derive(Debug, Clone)]
pub struct TeamSyncState {
    pub name: String,
    pub char_ids: Vec<u64>,
    pub leader_id: u64,
    pub member_skills: HashMap<u64, String>,
}

impl CharBag {
    pub fn new(assets: &BeyondAssets, default_team: &[String]) -> Result<Self> {
        let own_time = common::time::now_ms() as i64;
        let mut bag = Self {
            item_manager: ItemManager::init_for_new_player(assets, own_time),
            ..Default::default()
        };
        let mut index_map: HashMap<String, CharIndex> = HashMap::new();
        let own_time = now_ms() as i64;
        info!("Starting CharBag population with all characters");
        for (template_id, _char_data) in assets.characters.iter() {
            if assets.char_skills.get_char_skills(template_id).is_empty() {
                debug!("Skipping placeholder char: {}", template_id);
                continue;
            }
            let attrs = match assets.characters.get_stats(template_id, 1, 0) {
                Some(a) => a,
                None => {
                    debug!("No level 1 stats for char: {}", template_id);
                    continue;
                }
            };
            let skill_levels: HashMap<String, u32> = assets
                .char_skills
                .get_char_skills(template_id)
                .into_iter()
                .filter_map(|b| b.entries.first())
                .map(|e| (e.skill_id.clone(), 1u32))
                .collect();
            let char = Char {
                template_id: template_id.clone(),
                level: attrs.level,
                exp: 0,
                break_stage: attrs.break_stage,
                is_dead: false,
                hp: attrs.hp,
                ultimate_sp: 0.0,
                cached_weapon_inst_id: None,
                own_time,
                skill_levels,
            };
            let idx = bag.add_char(char);
            index_map.insert(template_id.clone(), idx);
        }
        debug!("Populated {} characters in CharBag", index_map.len());
        for (template_id, char_idx) in &index_map {
            let char_obj_id = char_idx.object_id();
            let char_data = match assets.characters.get(template_id) {
                Some(data) => data,
                None => continue,
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
            let weapon_inst_id = bag
                .item_manager
                .weapons
                .add_weapon(weapon.weapon_id.clone(), own_time);
            if let Err(e) = bag
                .item_manager
                .weapons
                .equip_weapon(weapon_inst_id, char_obj_id)
            {
                warn!(
                    "Failed to equip default weapon to char {}: {}",
                    char_obj_id, e
                );
            } else {
                debug!(
                    "Equipped default weapon {} (inst: {}) to char {} ({})",
                    weapon.weapon_id,
                    weapon_inst_id.as_u64(),
                    char_obj_id,
                    template_id
                );
            }
        }
        let mut team = Team {
            name: "Team 1".to_string(),
            ..Default::default()
        };
        let mut slot = 0;
        let mut leader = None;
        for template_id in default_team {
            if let Some(&idx) = index_map
                .get(template_id)
                .filter(|_| slot < Team::SLOTS_COUNT)
            {
                team.char_team[slot] = TeamSlot::Occupied(idx);
                leader.get_or_insert(idx);
                slot += 1;
            }
        }
        team.leader_index = leader.unwrap_or_default();
        bag.teams.push(team);
        bag.pending_teams.mark_dirty(0);
        // Add 4 empty placeholder teams so the client's squadManager has squads for all indexes (otherwise it will crash)
        for i in 1..5 {
            bag.teams.push(Team {
                name: format!("Team {}", i + 1),
                ..Default::default()
            });
            bag.pending_teams.mark_dirty(i);
        }

        bag.meta.curr_team_index = 0;
        bag.meta_dirty = true;
        info!("Default team created with leader: {:?}", leader);
        Ok(bag)
    }

    pub fn add_char(&mut self, char: Char) -> CharIndex {
        let idx = CharIndex::from_usize(self.chars.len());
        self.chars.push(char);
        self.pending_chars.mark_dirty(idx.as_usize());
        idx
    }

    /// Mark the `chars[idx]` row as needing a re-upsert. Cheap (a
    /// `HashSet` insert); safe to call from any mutator that touches a
    /// char field, including the `is_dead`/`hp`/`ultimate_sp` battle
    /// updates that fire on every hit during combat.
    #[inline]
    pub fn mark_char_dirty(&mut self, idx: CharIndex) {
        self.pending_chars.mark_dirty(idx.as_usize());
    }

    /// Same as [`mark_char_dirty`] but takes the wire-protocol object id
    /// (1-based). Useful at the network boundary where ids are received
    /// in object-id form.
    #[inline]
    pub fn mark_char_dirty_by_objid(&mut self, objid: u64) {
        if objid == 0 {
            return;
        }
        let idx = CharIndex::from_object_id(objid);
        if idx.as_usize() < self.chars.len() {
            self.pending_chars.mark_dirty(idx.as_usize());
        }
    }

    /// Mark `teams[idx]` (and all its slots) as dirty.
    #[inline]
    pub fn mark_team_dirty(&mut self, idx: usize) {
        self.pending_teams.mark_dirty(idx);
    }

    /// Mark the `beyond_players` scalar row as dirty. Call after
    /// mutating `meta.curr_team_index` or after a flow that bumps any
    /// depot's `next_inst_id`.
    #[inline]
    pub fn mark_meta_dirty(&mut self) {
        self.meta_dirty = true;
    }

    /// Read-only access used by the DB layer.
    #[inline]
    pub fn pending_chars(&self) -> &PendingChanges<usize> {
        &self.pending_chars
    }

    #[inline]
    pub fn pending_chars_mut(&mut self) -> &mut PendingChanges<usize> {
        &mut self.pending_chars
    }

    #[inline]
    pub fn pending_teams(&self) -> &PendingChanges<usize> {
        &self.pending_teams
    }

    #[inline]
    pub fn pending_teams_mut(&mut self) -> &mut PendingChanges<usize> {
        &mut self.pending_teams
    }

    #[inline]
    pub fn is_meta_dirty(&self) -> bool {
        self.meta_dirty
    }

    #[inline]
    pub fn clear_meta_dirty(&mut self) {
        self.meta_dirty = false;
    }

    /// Aggregate `has_changes` across every tracker the char-bag owns.
    /// Used by [`crate::traits::PendingChanges`]-aware persisters to
    /// short-circuit a flush when nothing has changed (the hot path).
    pub fn has_pending_changes(&self) -> bool {
        if self.meta_dirty || self.pending_chars.has_changes() || self.pending_teams.has_changes() {
            return true;
        }
        let im = &self.item_manager;
        im.weapons.pending().has_changes()
            || im.gems.pending().has_changes()
            || im.equips.pending().has_changes()
            || im.special_items.pending().has_changes()
            || im.mission_items.pending().has_changes()
            || im.factory_items.pending().has_changes()
    }

    /// Clear every dirty/removed set the char-bag owns. Used after a
    /// successful full save (the `PlayerDb::save` path) to declare the
    /// in-memory state in sync with disk.
    pub fn clear_all_pending(&mut self) {
        self.pending_chars.clear();
        self.pending_teams.clear();
        self.meta_dirty = false;
        let im = &mut self.item_manager;
        im.weapons.pending_mut().clear();
        im.gems.pending_mut().clear();
        im.equips.pending_mut().clear();
        im.special_items.pending_mut().clear();
        im.mission_items.pending_mut().clear();
        im.factory_items.pending_mut().clear();
    }

    pub fn get_char(&self, idx: CharIndex) -> Option<&Char> {
        self.chars.get(idx.as_usize())
    }

    pub fn get_char_mut(&mut self, idx: CharIndex) -> Option<&mut Char> {
        // We can't know what the caller is about to mutate, so be
        // conservative and mark this char dirty whenever a mutable
        // reference is handed out. This is the same trade-off Hibernate
        // makes for entity proxies and avoids "oops I forgot to mark it
        // dirty" bugs in every call site.
        if idx.as_usize() < self.chars.len() {
            self.pending_chars.mark_dirty(idx.as_usize());
        }
        self.chars.get_mut(idx.as_usize())
    }

    pub fn char_index_by_id(&self, template_id: &str) -> Option<CharIndex> {
        self.chars
            .iter()
            .position(|c| c.template_id == template_id)
            .map(CharIndex::from_usize)
    }

    pub fn get_char_by_objid(&self, objid: u64) -> Option<&Char> {
        self.chars.get(CharIndex::from_object_id(objid).as_usize())
    }

    pub fn get_char_by_objid_mut(&mut self, objid: u64) -> Option<&mut Char> {
        let idx = CharIndex::from_object_id(objid);
        if idx.as_usize() < self.chars.len() {
            self.pending_chars.mark_dirty(idx.as_usize());
        }
        self.chars.get_mut(idx.as_usize())
    }

    pub fn update_battle_info(&mut self, objid: u64, hp: f64, sp: f32) {
        // `get_char_by_objid_mut` already marks the row dirty.
        if let Some(char) = self.get_char_by_objid_mut(objid) {
            char.hp = hp;
            char.ultimate_sp = sp;
        }
    }

    pub fn equip_weapon(&mut self, char_id: u64, weapon_inst_id: u64) -> Result<ScWeaponPuton> {
        let weapon_inst_id = WeaponInstId::new(weapon_inst_id);

        let _char = self
            .get_char_by_objid(char_id)
            .ok_or_else(|| LogicError::NotFound("Character not found".into()))?;

        let weapon = self
            .item_manager
            .weapons
            .get_or_not_found(weapon_inst_id, "Weapon not found")?;
        let prev_owner = if weapon.is_equipped() && weapon.equip_char_id != char_id {
            Some(weapon.equip_char_id)
        } else {
            None
        };

        let off_weapon_id = self
            .item_manager
            .weapons
            .equip_weapon(weapon_inst_id, char_id)?
            .map(|id| id.as_u64());

        info!(
            "Equipped weapon {} to char {} (prev equipped: {:?}, prev owner: {:?})",
            weapon_inst_id.as_u64(),
            char_id,
            off_weapon_id,
            prev_owner
        );
        Ok(WeaponPutonArgs(char_id, weapon_inst_id, off_weapon_id, prev_owner).into())
    }

    pub fn unequip_weapon(&mut self, char_id: u64) -> Result<Option<WeaponInstId>> {
        if let Some(weapon_inst_id) = self.item_manager.weapons.get_equipped_weapon_id(char_id) {
            self.item_manager.weapons.unequip_weapon(weapon_inst_id)?;
            info!("Unequipped weapon from char {}", char_id);
            Ok(Some(weapon_inst_id))
        } else {
            Ok(None)
        }
    }

    pub fn get_equipped_weapon(&self, char_id: u64) -> Option<&WeaponInstance> {
        self.item_manager.weapons.get_equipped_weapon(char_id)
    }

    fn get_weapon_data_for_char(&self, char_id: u64) -> Option<WeaponData> {
        self.get_equipped_weapon(char_id).map(|w| w.into())
    }

    pub fn char_bag_info(&self, assets: &BeyondAssets) -> Result<ScSyncCharBagInfo> {
        let team_states = self.team_sync_states(assets);
        let char_states = self.char_sync_states(assets)?;

        let team_info = team_states
            .into_iter()
            .map(|t| CharTeamInfo {
                team_name: t.name,
                char_team: t.char_ids,
                leaderid: t.leader_id,
                member_info: t
                    .member_skills
                    .into_iter()
                    .map(|(id, skill)| {
                        (
                            id,
                            CharTeamMemberInfo {
                                normal_skillid: skill,
                            },
                        )
                    })
                    .collect(),
            })
            .collect();

        let char_info = char_states
            .into_iter()
            .map(|c| {
                let weapon_data = self.get_weapon_data_for_char(c.objid);
                let weapon_id = weapon_data.as_ref().map(|w| w.inst_id).unwrap_or(0);

                CharInfo {
                    objid: c.objid,
                    templateid: c.template_id,
                    level: c.level,
                    exp: c.exp,
                    finish_break_stage: c.break_stage as i32,
                    equip_col: Default::default(),
                    equip_suit: Default::default(),
                    normal_skill: c.normal_skill.clone(),
                    is_dead: c.is_dead,
                    weapon_id,
                    own_time: c.own_time,
                    battle_info: Some(BattleInfo {
                        hp: c.hp,
                        ultimatesp: c.ultimate_sp,
                    }),
                    skill_info: Some(perlica_proto::SkillInfo {
                        normal_skill: c.normal_skill,
                        level_info: c
                            .skill_levels
                            .into_iter()
                            .map(|s| perlica_proto::SkillLevelInfo {
                                skill_id: s.skill_id,
                                skill_level: s.skill_level,
                                skill_max_level: s.skill_max_level,
                            })
                            .collect(),
                    }),
                }
            })
            .collect();

        Ok(ScSyncCharBagInfo {
            char_info,
            team_info,
            curr_team_index: self.meta.curr_team_index as i32,
            max_char_team_member_count: Team::SLOTS_COUNT as u32,
        })
    }

    pub fn char_attrs(&self, assets: &BeyondAssets) -> Vec<ScSyncAttr> {
        self.chars
            .iter()
            .enumerate()
            .map(|(i, char)| {
                let objid = CharIndex::from_usize(i).object_id();
                let attr_list = assets
                    .characters
                    .get_stats(&char.template_id, char.level, char.break_stage)
                    .map(attrs_from_stats)
                    .unwrap_or_default();
                ScSyncAttr {
                    obj_id: objid,
                    attr_list,
                }
            })
            .collect()
    }

    pub fn char_status(&self) -> Vec<ScCharSyncStatus> {
        self.chars
            .iter()
            .enumerate()
            .map(|(i, char)| ScCharSyncStatus {
                objid: CharIndex::from_usize(i).object_id(),
                is_dead: char.is_dead,
                battle_info: Some(BattleInfo {
                    hp: char.hp,
                    ultimatesp: char.ultimate_sp,
                }),
            })
            .collect()
    }

    pub fn item_bag_sync(&self, assets: &config::BeyondAssets) -> ScItemBagSync {
        self.item_manager.build_full_bag_sync(assets)
    }

    fn team_sync_states(&self, assets: &BeyondAssets) -> Vec<TeamSyncState> {
        self.teams
            .iter()
            .map(|team| {
                let char_ids: Vec<u64> = team
                    .char_team
                    .iter()
                    .filter_map(|slot| slot.object_id())
                    .collect();

                let member_skills: HashMap<u64, String> = team
                    .char_team
                    .iter()
                    .filter_map(|slot| slot.char_index())
                    .map(|idx| {
                        let char_data = &self.chars[idx.as_usize()];
                        let skill = Self::get_normal_skill(&char_data.template_id, assets);
                        (idx.object_id(), skill)
                    })
                    .collect();

                TeamSyncState {
                    name: team.name.clone(),
                    char_ids,
                    leader_id: team.leader_index.object_id(),
                    member_skills,
                }
            })
            .collect()
    }

    fn char_sync_states(&self, assets: &BeyondAssets) -> Result<Vec<CharSyncState>> {
        self.chars
            .iter()
            .enumerate()
            .map(|(i, char)| {
                let objid = CharIndex::from_usize(i).object_id();
                let template = assets.characters.get(&char.template_id).ok_or_else(|| {
                    LogicError::NotFound(format!(
                        "Unknown character template: {}",
                        char.template_id
                    ))
                })?;

                let bundles = assets.char_skills.get_char_skills(&template.char_id);
                let normal_skill = Self::get_normal_skill(&char.template_id, assets);

                let skill_levels: Vec<SkillLevelState> = bundles
                    .iter()
                    .filter_map(|bundle| {
                        let first_id = &bundle.entries.first()?.skill_id;
                        let current_level = char.skill_levels.get(first_id).copied().unwrap_or(1);
                        let entry = bundle.entries.iter().find(|e| e.level == current_level)?;
                        let max = bundle.entries.iter().map(|e| e.level).max().unwrap_or(1);
                        Some(SkillLevelState {
                            skill_id: entry.skill_id.clone(),
                            skill_level: entry.level as i32,
                            skill_max_level: max as i32,
                        })
                    })
                    .collect();

                let weapon_id = self
                    .item_manager
                    .weapons
                    .get_equipped_weapon_id(objid)
                    .map(|id| id.as_u64())
                    .unwrap_or(0);

                Ok(CharSyncState {
                    objid,
                    template_id: char.template_id.clone(),
                    level: char.level,
                    exp: char.exp,
                    break_stage: char.break_stage,
                    hp: char.hp,
                    ultimate_sp: char.ultimate_sp,
                    weapon_id,
                    own_time: char.own_time,
                    is_dead: char.is_dead,
                    normal_skill,
                    skill_levels,
                })
            })
            .collect()
    }

    fn get_normal_skill(template_id: &str, assets: &BeyondAssets) -> String {
        assets
            .char_skills
            .get_char_skills(template_id)
            .into_iter()
            .find_map(|b| {
                b.entries
                    .first()
                    .filter(|e| e.skill_id.contains("normal_skill"))
                    .map(|e| e.skill_id.clone())
            })
            .unwrap_or_default()
    }

    pub fn validate_after_load(&mut self) {
        self.item_manager.weapons.validate_equipped_weapons();

        for i in 0..self.chars.len() {
            let char_obj_id = CharIndex::from_usize(i).object_id();

            if let Some(weapon) = self
                .item_manager
                .weapons
                .get_equipped_weapon(char_obj_id)
                .filter(|w| w.equip_char_id != char_obj_id)
            {
                warn!(
                    "Char {} has mismatched weapon reference: weapon claims char {}",
                    char_obj_id, weapon.equip_char_id
                );
            }
        }

        // We just rebuilt the bag from the on-disk row. Everything is,
        // by definition, in sync - wipe any dirty marks accidentally
        // set by the loader path.
        self.clear_all_pending();

        info!(
            "CharBag validation complete: {} chars, {} weapons",
            self.chars.len(),
            self.item_manager.weapons.len()
        );
    }

    pub fn item_manager_weapons(&self) -> &WeaponDepot {
        &self.item_manager.weapons
    }

    pub fn item_manager_weapons_mut(&mut self) -> &mut WeaponDepot {
        &mut self.item_manager.weapons
    }
}

fn attrs_from_stats(a: &config::tables::character::Attributes) -> Vec<AttrInfo> {
    let attr = |attr_type: AttributeType, value: f64| AttrInfo {
        attr_type: attr_type as i32,
        basic_value: value,
        value,
    };

    vec![
        attr(AttributeType::Hp, a.hp),
        attr(AttributeType::Atk, a.atk as f64),
        attr(AttributeType::Def, a.def as f64),
        attr(
            AttributeType::PhysicalResistance,
            a.physical_resistance as f64,
        ),
        attr(AttributeType::FireResistance, a.fire_resistance as f64),
        attr(AttributeType::PulseResistance, a.pulse_resistance as f64),
        attr(AttributeType::CrystResistance, a.cryst_resistance as f64),
        attr(AttributeType::Weight, a.weight as f64),
        attr(AttributeType::CriticalRate, a.critical_rate as f64),
        attr(AttributeType::CriticalDamage, a.critical_damage as f64),
        attr(AttributeType::Hatred, a.hatred as f64),
        attr(
            AttributeType::NormalAttackRange,
            a.normal_attack_range as f64,
        ),
        attr(AttributeType::AttackRate, a.attack_rate as f64),
        attr(AttributeType::Pen, a.pen as f64),
        attr(
            AttributeType::SpawnEnergyShardEfficiency,
            a.spawn_energy_shard_efficiency as f64,
        ),
    ]
}

pub struct WeaponAddExpResult {
    pub response: ScWeaponAddExp,
    pub consumed: ConsumedItems,
    pub removed_fodder: Vec<u64>,
    pub exp_gained: i64,
}

pub fn handle_weapon_add_exp(
    char_bag: &mut CharBag,
    weapon_id: u64,
    cost_items: &HashMap<String, u64>,
    cost_weapon_ids: &[u64],
    assets: &BeyondAssets,
) -> Result<WeaponAddExpResult> {
    let target_id = WeaponInstId::new(weapon_id);

    // weapon must exist.
    let weapon_data = char_bag
        .item_manager
        .weapons
        .get_or_not_found(target_id, "Target weapon not found")?;
    let template_id = weapon_data.template_id.clone();
    let current_level = weapon_data.weapon_lv;
    let current_exp = weapon_data.exp;
    let current_breakthrough = weapon_data.breakthrough_lv;

    // weapon template must exist.
    let weapon_template = match assets.weapons.get(&template_id) {
        Some(t) => t,
        None => {
            let weapon = char_bag.item_manager.weapons.get(target_id).unwrap();
            return Ok(WeaponAddExpResult {
                response: weapon.into(),
                consumed: ConsumedItems::new(),
                removed_fodder: vec![],
                exp_gained: 0,
            });
        }
    };

    let max_level = assets
        .weapons
        .get_effective_max_lv(&template_id, current_breakthrough);
    if current_level >= max_level {
        let weapon = char_bag.item_manager.weapons.get(target_id).unwrap();
        return Ok(WeaponAddExpResult {
            response: weapon.into(),
            consumed: ConsumedItems::new(),
            removed_fodder: vec![],
            exp_gained: 0,
        });
    }

    if assets
        .weapons
        .get_upgrade_sum(&weapon_template.level_template_id)
        .is_none()
    {
        let weapon = char_bag.item_manager.weapons.get(target_id).unwrap();
        return Ok(WeaponAddExpResult {
            response: weapon.into(),
            consumed: ConsumedItems::new(),
            removed_fodder: vec![],
            exp_gained: 0,
        });
    }

    let mut consumed = ConsumedItems::new();
    let mut item_exp: u64 = 0;
    for (item_id, &count) in cost_items {
        if count == 0 {
            continue;
        }
        let count = count as u32;
        let exp_per_unit = assets.weapons.weapon_exp_for_item(item_id);
        if exp_per_unit == 0 {
            continue;
        }
        match char_bag.item_manager.consume_stackable_auto(item_id, count) {
            Ok((depot_type, remaining)) => {
                item_exp += exp_per_unit * count as u64;
                consumed.record(depot_type, item_id.clone(), remaining);
            }
            Err(e) => {
                debug!(
                    "WeaponAddExp: skipping item {} * {} (not owned): {:?}",
                    item_id, count, e
                );
            }
        }
    }

    let fodder_ids: Vec<WeaponInstId> = cost_weapon_ids
        .iter()
        .map(|&id| WeaponInstId::new(id))
        .filter(|&id| {
            if id == target_id {
                return false;
            }
            char_bag
                .item_manager
                .weapons
                .get(id)
                .map(|f| !f.is_lock && !f.is_equipped())
                .unwrap_or(false)
        })
        .collect();

    char_bag
        .item_manager
        .weapons
        .add_exp(target_id, &fodder_ids, item_exp, assets)?;

    let weapon = char_bag
        .item_manager
        .weapons
        .get_or_not_found(target_id, "Weapon not found after add_exp")?;

    let exp_gained = item_exp as i64;
    let removed_fodder: Vec<u64> = fodder_ids.iter().map(|id| id.as_u64()).collect();

    info!(
        "WeaponAddExp complete: weapon={}, item_exp={}, fodder={}, lv {}->{}",
        weapon_id,
        item_exp,
        fodder_ids.len(),
        current_level,
        weapon.weapon_lv,
    );

    let _ = current_exp;

    Ok(WeaponAddExpResult {
        response: weapon.into(),
        consumed,
        removed_fodder,
        exp_gained,
    })
}

pub struct BreakthroughResult {
    pub response: ScWeaponBreakthrough,
    pub gold_cost: u32,
    pub consumed: ConsumedItems,
}

pub fn handle_weapon_breakthrough(
    char_bag: &mut CharBag,
    weapon_id: u64,
    assets: &BeyondAssets,
) -> Result<BreakthroughResult> {
    let inst_id = WeaponInstId::new(weapon_id);

    let (_new_lv, gold_cost, material_costs) = char_bag
        .item_manager
        .weapons
        .breakthrough(inst_id, assets)?;

    if let Err(e) = char_bag.item_manager.validate_materials(&material_costs) {
        // Roll back the breakthrough level since materials are insufficient
        if let Some(w) = char_bag.item_manager.weapons.get_mut(inst_id) {
            w.breakthrough_lv = w.breakthrough_lv.saturating_sub(1);
        }
        return Err(e);
    }

    let mut consumed = ConsumedItems::new();
    char_bag
        .item_manager
        .consume_materials(&material_costs, &mut consumed)?;

    let weapon = char_bag
        .item_manager
        .weapons
        .get_or_not_found(inst_id, "Weapon not found after breakthrough")?;

    info!(
        "Weapon breakthrough complete: weapon={}, new_lv={}, gold_cost={}, mats={:?}",
        inst_id, weapon.breakthrough_lv, gold_cost, material_costs
    );

    Ok(BreakthroughResult {
        response: weapon.into(),
        gold_cost,
        consumed,
    })
}

pub fn handle_weapon_attach_gem(
    char_bag: &mut CharBag,
    weapon_id: u64,
    gem_id: u64,
) -> Result<ScWeaponAttachGem> {
    let weapon_inst_id = WeaponInstId::new(weapon_id);

    // Check if gem is attached to another weapon
    let detached_from_weapon = char_bag
        .item_manager
        .weapons
        .all_weapons()
        .values()
        .find(|w| w.attach_gem_id == gem_id)
        .map(|w| w.inst_id);

    if let Some(other_weapon_id) = detached_from_weapon {
        char_bag.item_manager.weapons.detach_gem(other_weapon_id)?;
    }

    // Detach any existing gem from target weapon
    let weapon = char_bag
        .item_manager
        .weapons
        .get_or_not_found(weapon_inst_id, "Weapon not found")?;
    let prev_gem_id = if weapon.attach_gem_id != 0 {
        Some(weapon.attach_gem_id)
    } else {
        None
    };

    char_bag
        .item_manager
        .weapons
        .attach_gem(weapon_inst_id, gem_id)?;

    let weapon = char_bag
        .item_manager
        .weapons
        .get_or_not_found(weapon_inst_id, "Weapon not found")?;

    Ok(WeaponAttachGemArgs(
        weapon,
        prev_gem_id,
        detached_from_weapon.map(|id| id.as_u64()),
    )
    .into())
}

pub fn handle_weapon_detach_gem(
    char_bag: &mut CharBag,
    weapon_id: u64,
) -> Result<ScWeaponDetachGem> {
    let weapon_inst_id = WeaponInstId::new(weapon_id);

    let gem_id = char_bag.item_manager.weapons.detach_gem(weapon_inst_id)?;

    Ok(WeaponDetachGemArgs(weapon_inst_id, gem_id).into())
}

pub fn handle_weapon_puton(
    char_bag: &mut CharBag,
    char_id: u64,
    weapon_id: u64,
) -> Result<ScWeaponPuton> {
    char_bag.equip_weapon(char_id, weapon_id)
}
