use crate::error::{LogicError, Result};
use crate::item::ItemManager;
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
    pub fn new(assets: &BeyondAssets, default_team: &[String; 4]) -> Result<Self> {
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
        // Add 4 empty placeholder teams so the client's squadManager has squads for all indexes (otherwise it will crash)
        for i in 1..5 {
            bag.teams.push(Team {
                name: format!("Team {}", i + 1),
                ..Default::default()
            });
        }

        bag.meta.curr_team_index = 0;
        info!("Default team created with leader: {:?}", leader);
        Ok(bag)
    }

    pub fn add_char(&mut self, char: Char) -> CharIndex {
        let idx = CharIndex::from_usize(self.chars.len());
        self.chars.push(char);
        idx
    }

    pub fn get_char(&self, idx: CharIndex) -> Option<&Char> {
        self.chars.get(idx.as_usize())
    }

    pub fn get_char_mut(&mut self, idx: CharIndex) -> Option<&mut Char> {
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
        self.chars
            .get_mut(CharIndex::from_object_id(objid).as_usize())
    }

    pub fn update_battle_info(&mut self, objid: u64, hp: f64, sp: f32) {
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
            .get(weapon_inst_id)
            .ok_or_else(|| LogicError::NotFound("Weapon not found".into()))?;
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

        Ok(self.item_manager.weapons.to_weapon_puton_sc(
            char_id,
            weapon_inst_id,
            off_weapon_id,
            prev_owner,
        ))
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
        self.get_equipped_weapon(char_id)
            .map(|w| w.to_weapon_data())
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

pub fn handle_weapon_add_exp(
    char_bag: &mut CharBag,
    weapon_id: u64,
    cost_weapon_ids: &[u64],
    assets: &BeyondAssets,
) -> Result<ScWeaponAddExp> {
    let target_id = WeaponInstId::new(weapon_id);
    let fodder_ids: Vec<WeaponInstId> = cost_weapon_ids
        .iter()
        .map(|&id| WeaponInstId::new(id))
        .collect();

    char_bag
        .item_manager
        .weapons
        .add_exp(target_id, &fodder_ids, assets)?;

    char_bag
        .item_manager
        .weapons
        .to_add_exp_sc(target_id)
        .ok_or_else(|| LogicError::NotFound("Weapon not found after add_exp".into()))
}

pub fn handle_weapon_breakthrough(
    char_bag: &mut CharBag,
    weapon_id: u64,
    assets: &BeyondAssets,
) -> Result<ScWeaponBreakthrough> {
    let inst_id = WeaponInstId::new(weapon_id);

    char_bag
        .item_manager
        .weapons
        .breakthrough(inst_id, assets)?;

    char_bag
        .item_manager
        .weapons
        .to_breakthrough_sc(inst_id)
        .ok_or_else(|| LogicError::NotFound("Weapon not found after breakthrough".into()))
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
        .get(weapon_inst_id)
        .ok_or_else(|| LogicError::NotFound("Weapon not found".into()))?;
    let prev_gem_id = if weapon.attach_gem_id != 0 {
        Some(weapon.attach_gem_id)
    } else {
        None
    };

    char_bag
        .item_manager
        .weapons
        .attach_gem(weapon_inst_id, gem_id)?;

    char_bag
        .item_manager
        .weapons
        .to_attach_gem_sc(
            weapon_inst_id,
            prev_gem_id,
            detached_from_weapon.map(|id| id.as_u64()),
        )
        .ok_or_else(|| LogicError::NotFound("Weapon not found".into()))
}

pub fn handle_weapon_detach_gem(
    char_bag: &mut CharBag,
    weapon_id: u64,
) -> Result<ScWeaponDetachGem> {
    let weapon_inst_id = WeaponInstId::new(weapon_id);

    let gem_id = char_bag.item_manager.weapons.detach_gem(weapon_inst_id)?;

    Ok(char_bag
        .item_manager
        .weapons
        .to_detach_gem_sc(weapon_inst_id, gem_id))
}

pub fn handle_weapon_puton(
    char_bag: &mut CharBag,
    char_id: u64,
    weapon_id: u64,
) -> Result<ScWeaponPuton> {
    char_bag.equip_weapon(char_id, weapon_id)
}
