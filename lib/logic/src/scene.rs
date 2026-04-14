use crate::character::char_bag::{CharBag, CharIndex};
use crate::entity::{EntityKind, EntityManager, SceneEntity};
use crate::enums::{ParamRealType, ParamValueType};
use crate::level_script::LevelScriptManager;
use crate::movement::MovementManager;
use config::BeyondAssets;
use config::tables::level_data::LvProperty;
use perlica_proto::{
    DynamicParameter, LeaveObjectInfo, ScEnterSceneNotify, ScLeaveSceneNotify, ScObjectEnterView,
    ScObjectLeaveView, ScSceneCreateEntity, ScSceneDestroyEntity, ScSceneRevival, ScSceneTeleport,
    ScSelfSceneInfo, SceneCharacter, SceneImplEmpty, SceneInteractive, SceneMonster, SceneNpc,
    SceneObjectCommonInfo, SceneObjectDetailContainer, Vector, sc_self_scene_info::SceneImpl,
};
use std::collections::HashMap;

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfInfoReason {
    EnterScene = 0,
    ReviveDead = 1,
    ReviveRest = 2,
    ChangeTeam = 3,
    ReviveByItem = 4,
    ResetDungeon = 5,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityDestroyReason {
    Immediately = 0,
    Dead = 1,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum RevivalMode {
    #[default]
    Default = 0,
    RepatriatePoint = 1,
    CheckPoint = 2,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CheckpointInfo {
    pub scene_name: String,
    pub pos_x: f32,
    pub pos_y: f32,
    pub pos_z: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SceneLoadingState {
    #[default]
    Idle,
    Loading,
    Active,
}

#[derive(Debug, Clone)]
pub struct SceneManager {
    pub current_scene: String,
    pub scene_id: u64,
    pub loading_state: SceneLoadingState,
    pub in_battle: bool,
    pub checkpoint: Option<CheckpointInfo>,
    pub current_revival_mode: RevivalMode,
    pub level_scripts: LevelScriptManager,
    /// Maps level_logic_id to the timestamp (ms) when it was killed.
    pub dead_entities: std::collections::HashMap<u64, u64>,
}

impl Default for SceneManager {
    fn default() -> Self {
        Self {
            current_scene: "map01_lv001".to_string(),
            scene_id: 0,
            loading_state: SceneLoadingState::Idle,
            in_battle: false,
            checkpoint: None,
            current_revival_mode: RevivalMode::Default,
            level_scripts: LevelScriptManager::default(),
            dead_entities: std::collections::HashMap::new(),
        }
    }
}

fn lv_property_to_dynamic_param(prop: &LvProperty) -> DynamicParameter {
    let value = &prop.value;
    let real_type_int = value
        .get("type")
        .and_then(|entry| entry.as_i64())
        .unwrap_or(0) as i32;
    let real_type = ParamRealType::from(real_type_int);

    let value_array = value
        .get("valueArray")
        .and_then(|entry| entry.as_array())
        .cloned()
        .unwrap_or_default();

    let as_i64 = |entry: &serde_json::Value| {
        entry
            .get("valueBit64")
            .and_then(|value| value.as_i64())
            .unwrap_or(0)
    };
    let as_u32 = |entry: &serde_json::Value| as_i64(entry) as u32;
    let as_string = |entry: &serde_json::Value| {
        entry
            .get("valueString")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string()
    };

    match real_type {
        ParamRealType::Invalid | ParamRealType::ENum => DynamicParameter {
            value_type: ParamValueType::Invalid as i32,
            real_type: real_type_int,
            ..Default::default()
        },
        ParamRealType::Bool | ParamRealType::BoolList => DynamicParameter {
            value_type: real_type_int,
            real_type: real_type_int,
            value_bool_list: value_array.iter().map(|entry| as_i64(entry) != 0).collect(),
            ..Default::default()
        },
        ParamRealType::Int
        | ParamRealType::IntList
        | ParamRealType::EntityPtr
        | ParamRealType::EntityPtrList
        | ParamRealType::UInt
        | ParamRealType::UIntList
        | ParamRealType::FromContextCurrent
        | ParamRealType::FromContextMsg
        | ParamRealType::FromContextInteractive1
        | ParamRealType::FromContextInteractive2
        | ParamRealType::FromContextInteractive3
        | ParamRealType::LevelScriptPtr
        | ParamRealType::LevelScriptPtrList
        | ParamRealType::UInt64
        | ParamRealType::UInt64List
        | ParamRealType::Node
        | ParamRealType::NodeList
        | ParamRealType::Buff
        | ParamRealType::BuffList => DynamicParameter {
            value_type: match real_type {
                ParamRealType::Int => ParamValueType::Int as i32,
                ParamRealType::IntList => ParamValueType::IntList as i32,
                ParamRealType::EntityPtr
                | ParamRealType::UInt
                | ParamRealType::FromContextCurrent
                | ParamRealType::FromContextMsg
                | ParamRealType::FromContextInteractive1
                | ParamRealType::FromContextInteractive2
                | ParamRealType::FromContextInteractive3
                | ParamRealType::LevelScriptPtr
                | ParamRealType::UInt64
                | ParamRealType::Node
                | ParamRealType::Buff => ParamValueType::Int as i32,
                ParamRealType::EntityPtrList
                | ParamRealType::UIntList
                | ParamRealType::LevelScriptPtrList
                | ParamRealType::UInt64List
                | ParamRealType::NodeList
                | ParamRealType::BuffList => ParamValueType::IntList as i32,
                _ => ParamValueType::IntList as i32, // Fallback, though should be covered
            },
            real_type: real_type_int,
            value_int_list: value_array.iter().map(as_i64).collect(),
            ..Default::default()
        },
        ParamRealType::Float => {
            let first = value_array.first().map(as_i64).unwrap_or_default();
            if first < 0 {
                DynamicParameter {
                    value_type: ParamValueType::Int as i32,
                    real_type: real_type_int,
                    value_int_list: value_array.iter().map(as_i64).collect(),
                    ..Default::default()
                }
            } else {
                DynamicParameter {
                    value_type: ParamValueType::Float as i32,
                    real_type: real_type_int,
                    value_float_list: value_array
                        .iter()
                        .map(|entry| f32::from_bits(as_u32(entry)))
                        .collect(),
                    ..Default::default()
                }
            }
        }
        ParamRealType::FloatList | ParamRealType::Vector3 | ParamRealType::Vector3List => {
            DynamicParameter {
                value_type: ParamValueType::FloatList as i32,
                real_type: real_type_int,
                value_float_list: value_array
                    .iter()
                    .map(|entry| f32::from_bits(as_u32(entry)))
                    .collect(),
                ..Default::default()
            }
        }
        ParamRealType::String
        | ParamRealType::StringList
        | ParamRealType::Path
        | ParamRealType::PathList
        | ParamRealType::Tag
        | ParamRealType::TagList
        | ParamRealType::LangKey
        | ParamRealType::LangKeyList
        | ParamRealType::Bytes => DynamicParameter {
            value_type: match real_type {
                ParamRealType::StringList
                | ParamRealType::PathList
                | ParamRealType::TagList
                | ParamRealType::LangKeyList => ParamValueType::StringList as i32,
                _ => ParamValueType::String as i32,
            },
            real_type: real_type_int,
            value_string_list: value_array.iter().map(as_string).collect(),
            ..Default::default()
        },
    }
}

pub(crate) fn lv_props_to_map(props: &[LvProperty]) -> HashMap<String, DynamicParameter> {
    props
        .iter()
        .map(|p| (p.key.clone(), lv_property_to_dynamic_param(p)))
        .collect()
}

pub struct SceneEntityLists {
    pub chars: Vec<SceneCharacter>,
    pub monsters: Vec<SceneMonster>,
    pub interactives: Vec<SceneInteractive>,
    pub npcs: Vec<SceneNpc>,
}

/// Rotates `leader_id` to the front of `chars` if it isn't already there.
fn move_leader_to_front(chars: &mut [SceneCharacter], leader_id: u64) {
    if let Some(pos) = chars
        .iter()
        .position(|c| c.common_info.as_ref().map(|ci| ci.id) == Some(leader_id))
        .filter(|&p| p != 0)
    {
        // rotate_right(1) on [0..=pos] shifts everything right and wraps
        // the last element (currently at `pos`) around to index 0.
        chars[0..=pos].rotate_right(1);
    }
}

fn pack_interactives(scene_id: &str, assets: &BeyondAssets) -> Vec<SceneInteractive> {
    assets
        .level_data
        .interactives(scene_id)
        .iter()
        .map(|i| SceneInteractive {
            common_info: Some(SceneObjectCommonInfo {
                id: i.base.level_logic_id,
                r#type: i.base.entity_type,
                templateid: i.base.template_id.clone(),
                position: Some(Vector {
                    x: i.base.position.x,
                    y: i.base.position.y,
                    z: i.base.position.z,
                }),
                rotation: Some(Vector {
                    x: i.base.rotation.x,
                    y: i.base.rotation.y,
                    z: i.base.rotation.z,
                }),
                belong_level_script_id: i.base.belong_level_script_id,
            }),
            origin_id: i.base.level_logic_id,
            properties: lv_props_to_map(&i.properties),
        })
        .collect()
}

fn pack_npcs(scene_id: &str, assets: &BeyondAssets) -> Vec<SceneNpc> {
    assets
        .level_data
        .npcs(scene_id)
        .iter()
        .map(|n| SceneNpc {
            common_info: Some(SceneObjectCommonInfo {
                id: n.base.level_logic_id,
                r#type: n.base.entity_type,
                templateid: n.base.template_id.clone(),
                position: Some(Vector {
                    x: n.base.position.x,
                    y: n.base.position.y,
                    z: n.base.position.z,
                }),
                rotation: Some(Vector {
                    x: n.base.rotation.x,
                    y: n.base.rotation.y,
                    z: n.base.rotation.z,
                }),
                belong_level_script_id: n.base.belong_level_script_id,
            }),
        })
        .collect()
}

impl SceneManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_scene_transition(
        &mut self,
        new_scene: &str,
        position: Vector,
        assets: &BeyondAssets,
        entities: &mut EntityManager,
    ) -> (ScEnterSceneNotify, ScLeaveSceneNotify) {
        entities.clear();
        self.dead_entities.clear();

        let leave_notify = ScLeaveSceneNotify {
            role_id: 1, //TODO: figure out why and where is this even used
            scene_name: self.current_scene.clone(),
            scene_id: self.scene_id,
        };

        self.current_scene = new_scene.to_string();
        self.scene_id = assets.str_id_num.get_scene_id(new_scene).unwrap_or(0);
        self.loading_state = SceneLoadingState::Loading;
        self.level_scripts.reset_scene(new_scene, assets);

        let enter_notify = ScEnterSceneNotify {
            role_id: 1,
            scene_name: self.current_scene.clone(),
            scene_id: self.scene_id,
            position: Some(position),
        };
        (enter_notify, leave_notify)
    }

    pub fn finish_scene_load(
        &mut self,
        char_bag: &CharBag,
        movement: &MovementManager,
        assets: &BeyondAssets,
        entities: &mut EntityManager,
    ) -> (ScObjectEnterView, ScSelfSceneInfo) {
        self.loading_state = SceneLoadingState::Active;

        self.scene_id = assets
            .str_id_num
            .get_scene_id(&self.current_scene)
            .unwrap_or(0);
        self.level_scripts.sync_scene(&self.current_scene, assets);

        let char_list = self.pack_scene_chars(char_bag, movement);
        let monster_list = self.pack_scene_monsters(assets, entities);
        let interactive_list = pack_interactives(&self.current_scene, assets);
        let npc_list = pack_npcs(&self.current_scene, assets);

        tracing::info!(
            "Scene '{}' loaded: {} chars, {} monsters, {} interactives, {} npcs",
            self.current_scene,
            char_list.len(),
            monster_list.len(),
            interactive_list.len(),
            npc_list.len()
        );

        let enter_view = self.object_enter_view_full(
            char_list.clone(),
            monster_list.clone(),
            interactive_list.clone(),
            npc_list.clone(),
        );
        let self_info = self.self_scene_info(
            SelfInfoReason::EnterScene,
            SceneEntityLists {
                chars: char_list,
                monsters: monster_list,
                interactives: interactive_list,
                npcs: npc_list,
            },
            vec![],
            assets,
        );

        (enter_view, self_info)
    }

    pub fn object_enter_view(
        &self,
        char_list: Vec<SceneCharacter>,
        monster_list: Vec<SceneMonster>,
    ) -> ScObjectEnterView {
        ScObjectEnterView {
            scene_name: self.current_scene.clone(),
            scene_id: self.scene_id,
            detail: Some(SceneObjectDetailContainer {
                char_list,
                monster_list,
                interactive_list: vec![],
                npc_list: vec![],
                summon_list: vec![],
            }),
            has_extra_object: false,
        }
    }

    pub fn object_enter_view_full(
        &self,
        char_list: Vec<SceneCharacter>,
        monster_list: Vec<SceneMonster>,
        interactive_list: Vec<SceneInteractive>,
        npc_list: Vec<SceneNpc>,
    ) -> ScObjectEnterView {
        ScObjectEnterView {
            scene_name: self.current_scene.clone(),
            scene_id: self.scene_id,
            detail: Some(SceneObjectDetailContainer {
                char_list,
                monster_list,
                interactive_list,
                npc_list,
                summon_list: vec![],
            }),
            has_extra_object: false,
        }
    }

    pub fn object_leave_view(&self, entity_ids: Vec<u64>) -> ScObjectLeaveView {
        let obj_list = entity_ids
            .into_iter()
            .map(|id| LeaveObjectInfo {
                obj_type: 0,
                obj_id: id,
            })
            .collect();

        ScObjectLeaveView {
            scene_name: self.current_scene.clone(),
            scene_id: self.scene_id,
            obj_list,
        }
    }

    pub fn self_scene_info(
        &self,
        reason: SelfInfoReason,
        lists: SceneEntityLists,
        revive_chars: Vec<u64>,
        assets: &BeyondAssets,
    ) -> ScSelfSceneInfo {
        let level_scripts = self
            .level_scripts
            .packed_level_scripts(&self.current_scene, assets);

        ScSelfSceneInfo {
            scene_name: self.current_scene.clone(),
            scene_id: self.scene_id,
            detail: Some(SceneObjectDetailContainer {
                char_list: lists.chars,
                monster_list: lists.monsters,
                interactive_list: lists.interactives,
                npc_list: lists.npcs,
                summon_list: vec![],
            }),
            last_camp_id: 0,
            revive_chars,
            level_scripts,
            self_info_reason: reason as i32,
            unlock_area: vec![self.current_scene.clone()],
            revival_mode: self.current_revival_mode as i32,
            scene_var: HashMap::new(),
            scene_impl: Some(SceneImpl::Empty(SceneImplEmpty {})), //since dungeons aren't implemented yet we'll default to empty for the time being
        }
    }

    // Called on CS_SCENE_SET_REPATRIATE_POINT or when the revival mode changes.
    pub fn set_revival_mode(&mut self, mode: RevivalMode) {
        self.current_revival_mode = mode;
    }

    pub fn destroy_entity(
        &self,
        entity_id: u64,
        reason: EntityDestroyReason,
    ) -> ScSceneDestroyEntity {
        ScSceneDestroyEntity {
            scene_name: self.current_scene.clone(),
            id: entity_id,
            reason: reason as i32,
        }
    }

    pub fn create_entity(&self, entity_id: u64) -> ScSceneCreateEntity {
        ScSceneCreateEntity {
            scene_name: self.current_scene.clone(),
            id: entity_id,
        }
    }

    pub fn handle_revival(
        &mut self,
        char_bag: &mut CharBag,
        movement: &MovementManager,
        assets: &BeyondAssets,
        entities: &mut EntityManager,
        revival_mode: Option<RevivalMode>,
    ) -> (ScObjectEnterView, ScSelfSceneInfo, ScSceneRevival) {
        if let Some(mode) = revival_mode {
            self.set_revival_mode(mode);
        }
        let team = &char_bag.teams[char_bag.meta.curr_team_index as usize];
        let revive_chars: Vec<u64> = team
            .char_team
            .iter()
            .filter_map(|slot| slot.char_index())
            .filter(|&idx| char_bag.chars[idx.as_usize()].is_dead)
            .map(|idx| idx.object_id())
            .collect();

        for &objid in &revive_chars {
            let idx = CharIndex::from_object_id(objid);
            if let Some(char) = char_bag.chars.get_mut(idx.as_usize()) {
                char.is_dead = false;
                char.hp = assets
                    .characters
                    .get_stats(&char.template_id, char.level, char.break_stage)
                    .map(|a| a.hp / 2.0)
                    .unwrap_or(50.0);
            }
        }

        let char_list = self.pack_scene_chars(char_bag, movement);
        let monster_list = self.pack_scene_monsters(assets, entities);
        let interactive_list = pack_interactives(&self.current_scene, assets);
        let npc_list = pack_npcs(&self.current_scene, assets);

        tracing::info!(
            "Revival in scene '{}': {} chars, {} monsters, {} interactives, {} npcs",
            self.current_scene,
            char_list.len(),
            monster_list.len(),
            interactive_list.len(),
            npc_list.len()
        );

        let enter_view = self.object_enter_view_full(
            char_list.clone(),
            monster_list.clone(),
            interactive_list.clone(),
            npc_list.clone(),
        );
        let self_info = self.self_scene_info(
            SelfInfoReason::ReviveDead,
            SceneEntityLists {
                chars: char_list,
                monsters: monster_list,
                interactives: interactive_list,
                npcs: npc_list,
            },
            revive_chars,
            assets,
        );
        let revival = ScSceneRevival {};

        (enter_view, self_info, revival)
    }

    pub fn handle_active_team_update(
        &mut self,
        old_team_ids: &[u64],
        new_team_ids: &[u64],
        char_bag: &CharBag,
        movement: &MovementManager,
        assets: &BeyondAssets,
        entities: &mut EntityManager,
    ) -> (
        Option<ScObjectLeaveView>,
        ScObjectEnterView,
        ScSelfSceneInfo,
    ) {
        let _ = assets;
        let new_set: std::collections::HashSet<u64> = new_team_ids.iter().copied().collect();
        let old_set: std::collections::HashSet<u64> = old_team_ids.iter().copied().collect();

        let leaving: Vec<u64> = old_team_ids
            .iter()
            .filter(|&&id| {
                if new_set.contains(&id) {
                    return false;
                }
                let idx = CharIndex::from_object_id(id);
                char_bag
                    .chars
                    .get(idx.as_usize())
                    .map(|c| !c.is_dead)
                    .unwrap_or(false)
            })
            .copied()
            .collect();

        let leave_view = if leaving.is_empty() {
            None
        } else {
            Some(self.object_leave_view(leaving))
        };

        let entering: Vec<u64> = new_team_ids
            .iter()
            .filter(|&&id| {
                if old_set.contains(&id) {
                    return false;
                }
                let idx = CharIndex::from_object_id(id);
                char_bag
                    .chars
                    .get(idx.as_usize())
                    .map(|c| !c.is_dead)
                    .unwrap_or(false)
            })
            .copied()
            .collect();

        let enter_view = self.object_enter_view(
            self.pack_scene_chars_for_ids(&entering, char_bag, movement),
            vec![],
        );

        let all_alive_ids: Vec<u64> = new_team_ids
            .iter()
            .filter(|&&id| {
                let idx = CharIndex::from_object_id(id);
                char_bag
                    .chars
                    .get(idx.as_usize())
                    .map(|c| !c.is_dead)
                    .unwrap_or(false)
            })
            .copied()
            .collect();

        let mut char_list = self.pack_scene_chars_for_ids(&all_alive_ids, char_bag, movement);
        let leader_id = char_bag.teams[char_bag.meta.curr_team_index as usize]
            .leader_index
            .object_id();
        move_leader_to_front(&mut char_list, leader_id);

        let monster_list = self.pack_monsters_from_manager(entities, assets);
        let self_info = self.self_scene_info(
            SelfInfoReason::ChangeTeam,
            SceneEntityLists {
                chars: char_list,
                monsters: monster_list,
                interactives: vec![],
                npcs: vec![],
            },
            vec![],
            assets,
        );

        (leave_view, enter_view, self_info)
    }

    pub fn pack_monsters_from_manager(
        &self,
        entities: &EntityManager,
        _assets: &BeyondAssets,
    ) -> Vec<SceneMonster> {
        use perlica_proto::SceneObjectCommonInfo;

        entities
            .monsters()
            .map(|e| SceneMonster {
                common_info: Some(SceneObjectCommonInfo {
                    id: e.id,
                    templateid: e.template_id.clone(),
                    position: Some(Vector {
                        x: e.pos_x,
                        y: e.pos_y,
                        z: e.pos_z,
                    }),
                    rotation: None,
                    belong_level_script_id: e.belong_level_script_id,
                    r#type: 16,
                }),
                origin_id: e.level_logic_id,
                // Level not re-sent on team switch; client already has it from
                // the initial ScObjectEnterView on scene load.
                level: 1,
            })
            .collect()
    }

    pub fn pack_scene_chars_for_ids(
        &self,
        char_ids: &[u64],
        char_bag: &CharBag,
        movement: &MovementManager,
    ) -> Vec<SceneCharacter> {
        let spawn_pos = Vector {
            x: movement.pos_x,
            y: movement.pos_y,
            z: movement.pos_z,
        };
        let spawn_rot = Vector {
            x: movement.rot_x,
            y: movement.rot_y,
            z: movement.rot_z,
        };

        char_ids
            .iter()
            .filter_map(|&objid| {
                let idx = CharIndex::from_object_id(objid);
                char_bag
                    .chars
                    .get(idx.as_usize())
                    .map(|char_data| SceneCharacter {
                        common_info: Some(SceneObjectCommonInfo {
                            id: objid,
                            templateid: char_data.template_id.clone(),
                            position: Some(spawn_pos),
                            rotation: Some(spawn_rot),
                            belong_level_script_id: 0,
                            r#type: 8,
                        }),
                        level: char_data.level,
                        name: "Player".to_string(),
                    })
            })
            .collect()
    }

    pub fn handle_team_index_switch(
        &mut self,
        old_team_ids: &[u64],
        new_team_ids: &[u64],
        char_bag: &CharBag,
        movement: &MovementManager,
        assets: &BeyondAssets,
        entities: &mut EntityManager,
    ) -> (
        Option<ScObjectLeaveView>,
        ScObjectEnterView,
        ScSelfSceneInfo,
    ) {
        self.handle_active_team_update(
            old_team_ids,
            new_team_ids,
            char_bag,
            movement,
            assets,
            entities,
        )
    }

    pub fn handle_inactive_team_update(
        &self,
        new_team_ids: &[u64],
        char_bag: &CharBag,
        movement: &MovementManager,
        assets: &BeyondAssets,
        entities: &EntityManager,
    ) -> ScSelfSceneInfo {
        let alive_ids: Vec<u64> = new_team_ids
            .iter()
            .filter(|&&id| {
                let idx = CharIndex::from_object_id(id);
                char_bag
                    .chars
                    .get(idx.as_usize())
                    .map(|c| !c.is_dead)
                    .unwrap_or(false)
            })
            .copied()
            .collect();

        let mut char_list = self.pack_scene_chars_for_ids(&alive_ids, char_bag, movement);
        let monster_list = self.pack_monsters_from_manager(entities, assets);

        // leader always goes first
        let leader_id = char_bag.teams[char_bag.meta.curr_team_index as usize]
            .leader_index
            .object_id();
        move_leader_to_front(&mut char_list, leader_id);

        self.self_scene_info(
            SelfInfoReason::ChangeTeam,
            SceneEntityLists {
                chars: char_list,
                monsters: monster_list,
                interactives: vec![],
                npcs: vec![],
            },
            vec![],
            assets,
        )
    }
    pub fn teleport(
        &self,
        obj_id_list: Vec<u64>,
        position: Vector,
        rotation: Option<Vector>,
        server_time: u32,
        teleport_reason: i32,
        scene_name: Option<String>,
    ) -> ScSceneTeleport {
        ScSceneTeleport {
            obj_id_list,
            scene_name: scene_name.unwrap_or(self.current_scene.clone()),
            position: Some(position),
            rotation,
            server_time,
            teleport_reason,
        }
    }

    pub fn set_battle_mode(&mut self, in_battle: bool) {
        self.in_battle = in_battle;
    }

    pub fn pack_scene_chars(
        &self,
        char_bag: &CharBag,
        movement: &MovementManager,
    ) -> Vec<SceneCharacter> {
        let team = &char_bag.teams[char_bag.meta.curr_team_index as usize];

        let spawn_pos = Vector {
            x: movement.pos_x,
            y: movement.pos_y,
            z: movement.pos_z,
        };
        let spawn_rot = Vector {
            x: movement.rot_x,
            y: movement.rot_y,
            z: movement.rot_z,
        };

        let mut chars: Vec<SceneCharacter> = team
            .char_team
            .iter()
            .filter_map(|slot| slot.char_index())
            .map(|idx| {
                let char_data = &char_bag.chars[idx.as_usize()];
                SceneCharacter {
                    common_info: Some(SceneObjectCommonInfo {
                        id: idx.object_id(),
                        templateid: char_data.template_id.clone(),
                        position: Some(spawn_pos),
                        rotation: Some(spawn_rot),
                        belong_level_script_id: 0,
                        r#type: 8,
                    }),
                    level: char_data.level,
                    name: "Player".to_string(),
                }
            })
            .collect();

        let leader_id = team.leader_index.object_id();
        move_leader_to_front(&mut chars, leader_id);

        chars
    }

    pub fn pack_scene_monsters(
        &self,
        _assets: &BeyondAssets,
        _entities: &mut EntityManager,
    ) -> Vec<SceneMonster> {
        // We don't spawn anything by default now.
        // The dynamic radius-based system will handle it.
        vec![]
    }

    pub fn update_visible_entities(
        &mut self,
        pos: (f32, f32, f32),
        assets: &BeyondAssets,
        entities: &mut EntityManager,
    ) -> (Option<ScObjectEnterView>, Option<ScObjectLeaveView>) {
        const ENTER_RADIUS: f32 = 80.0;
        const LEAVE_RADIUS: f32 = 100.0;
        const RESPAWN_COOLDOWN_MS: u64 = 60_000; // 60 seconds

        let now = common::time::now_ms();

        self.dead_entities
            .retain(|_, &mut time| now - time < RESPAWN_COOLDOWN_MS);

        let mut enter_monsters = vec![];
        let mut leave_ids = vec![];

        // 1. Check which monsters should enter view
        let spawns = assets.level_data.enemies(&self.current_scene);
        for enemy in spawns {
            let dx = enemy.base.position.x - pos.0;
            let dy = enemy.base.position.y - pos.1;
            let dz = enemy.base.position.z - pos.2;
            let dist_sq = dx * dx + dy * dy + dz * dz;

            if dist_sq <= ENTER_RADIUS * ENTER_RADIUS {
                let already_exists = entities
                    .monsters()
                    .any(|e| e.level_logic_id == enemy.base.level_logic_id);

                let is_on_cooldown = self.dead_entities.contains_key(&enemy.base.level_logic_id);

                if !already_exists && !is_on_cooldown {
                    let id = enemy.base.level_logic_id;
                    entities.insert(SceneEntity {
                        id,
                        template_id: enemy.base.template_id.clone(),
                        kind: EntityKind::Enemy,
                        pos_x: enemy.base.position.x,
                        pos_y: enemy.base.position.y,
                        pos_z: enemy.base.position.z,
                        level_logic_id: enemy.base.level_logic_id,
                        belong_level_script_id: enemy.base.belong_level_script_id,
                    });

                    enter_monsters.push(SceneMonster {
                        common_info: Some(SceneObjectCommonInfo {
                            id,
                            templateid: enemy.base.template_id.clone(),
                            position: Some(Vector {
                                x: enemy.base.position.x,
                                y: enemy.base.position.y,
                                z: enemy.base.position.z,
                            }),
                            rotation: Some(Vector {
                                x: enemy.base.rotation.x,
                                y: enemy.base.rotation.y,
                                z: enemy.base.rotation.z,
                            }),
                            belong_level_script_id: enemy.base.belong_level_script_id,
                            r#type: enemy.base.entity_type,
                        }),
                        origin_id: enemy.base.level_logic_id,
                        level: enemy.level as i32,
                    });
                }
            }
        }

        // 2. Check which monsters should leave view
        let current_monsters: Vec<(u64, f32, f32, f32)> = entities
            .monsters()
            .map(|e| (e.id, e.pos_x, e.pos_y, e.pos_z))
            .collect();

        for (id, ex, ey, ez) in current_monsters {
            let dx = ex - pos.0;
            let dy = ey - pos.1;
            let dz = ez - pos.2;
            let dist_sq = dx * dx + dy * dy + dz * dz;

            if dist_sq > LEAVE_RADIUS * LEAVE_RADIUS {
                entities.remove(id);
                leave_ids.push(id);
            }
        }

        let enter_view = if !enter_monsters.is_empty() {
            Some(self.object_enter_view(vec![], enter_monsters))
        } else {
            None
        };

        let leave_view = if !leave_ids.is_empty() {
            Some(self.object_leave_view(leave_ids))
        } else {
            None
        };

        (enter_view, leave_view)
    }

    pub fn pack_single_monster(
        &self,
        entity: &SceneEntity,
        level: i32,
        origin_id: u64,
    ) -> SceneMonster {
        SceneMonster {
            common_info: Some(SceneObjectCommonInfo {
                id: entity.id,
                templateid: entity.template_id.clone(),
                position: Some(Vector {
                    x: entity.pos_x,
                    y: entity.pos_y,
                    z: entity.pos_z,
                }),
                rotation: None,
                belong_level_script_id: 0,
                r#type: 16,
            }),
            origin_id,
            level,
        }
    }

    // Pack a single character for dynamic spawning (multiplayer peer, future use)
    pub fn pack_single_char(
        &self,
        objid: u64,
        template_id: String,
        level: i32,
        position: Vector,
        rotation: Vector,
    ) -> SceneCharacter {
        SceneCharacter {
            common_info: Some(SceneObjectCommonInfo {
                id: objid,
                templateid: template_id,
                position: Some(position),
                rotation: Some(rotation),
                belong_level_script_id: 0,
                r#type: 8,
            }),
            level,
            name: "Player".to_string(),
        }
    }

    pub fn scene_name(&self) -> &str {
        &self.current_scene
    }

    pub fn is_in_scene(&self) -> bool {
        self.loading_state == SceneLoadingState::Active
    }

    pub fn set_checkpoint(&mut self, checkpoint: CheckpointInfo) {
        self.checkpoint = Some(checkpoint);
    }

    pub fn get_checkpoint(&self) -> Option<&CheckpointInfo> {
        self.checkpoint.as_ref()
    }

    pub fn update_from_world(&mut self, world: &crate::player::WorldState, assets: &BeyondAssets) {
        self.current_scene = world.last_scene.clone();
        self.scene_id = assets
            .str_id_num
            .get_scene_id(&world.last_scene)
            .unwrap_or(0);
        self.level_scripts.sync_scene(&self.current_scene, assets);
    }
}
