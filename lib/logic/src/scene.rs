use crate::character::char_bag::{CharBag, CharIndex};
use crate::entity::{EntityKind, EntityManager, SceneEntity};
use crate::movement::MovementManager;
use config::BeyondAssets;
use config::tables::level_data::{LvInteractive, LvLevelScript, LvNpc, LvProperty};
use perlica_proto::{
    DynamicParameter, LeaveObjectInfo, LevelScriptInfo, ScEnterSceneNotify, ScLeaveSceneNotify,
    ScObjectEnterView, ScObjectLeaveView, ScSceneCreateEntity, ScSceneDestroyEntity,
    ScSceneRevival, ScSceneTeleport, ScSelfSceneInfo, SceneCharacter, SceneImplEmpty,
    SceneInteractive, SceneMonster, SceneNpc, SceneObjectCommonInfo, SceneObjectDetailContainer,
    Vector, sc_self_scene_info::SceneImpl,
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
            dead_entities: std::collections::HashMap::new(),
        }
    }
}

fn lv_property_to_dynamic_param(prop: &LvProperty) -> DynamicParameter {
    let val = &prop.value;
    let type_id = val.get("type").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

    let value_array = val
        .get("valueArray")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    match type_id {
        1 => DynamicParameter {
            value_type: 1,
            real_type: 1,
            value_bool_list: value_array
                .iter()
                .map(|v| v.get("valueBit64").and_then(|x| x.as_i64()).unwrap_or(0) != 0)
                .collect(),
            ..Default::default()
        },
        2 => DynamicParameter {
            value_type: 2,
            real_type: 2,
            value_int_list: value_array
                .iter()
                .map(|v| v.get("valueBit64").and_then(|x| x.as_i64()).unwrap_or(0))
                .collect(),
            ..Default::default()
        },
        3 => DynamicParameter {
            value_type: 3,
            real_type: 3,
            value_float_list: value_array
                .iter()
                .map(|v| {
                    let bits = v.get("valueBit64").and_then(|x| x.as_i64()).unwrap_or(0) as u32;
                    f32::from_bits(bits)
                })
                .collect(),
            ..Default::default()
        },
        7 => DynamicParameter {
            value_type: 7,
            real_type: 7,
            value_string_list: value_array
                .iter()
                .filter_map(|v| {
                    v.get("valueString")
                        .and_then(|s| s.as_str())
                        .map(|s| s.to_string())
                })
                .collect(),
            ..Default::default()
        },
        11 => DynamicParameter {
            value_type: 11,
            real_type: 11,
            value_float_list: value_array
                .iter()
                .map(|v| {
                    let bits = v.get("valueBit64").and_then(|x| x.as_i64()).unwrap_or(0) as u32;
                    f32::from_bits(bits)
                })
                .collect(),
            ..Default::default()
        },
        _ => DynamicParameter {
            value_type: type_id,
            real_type: type_id,
            value_int_list: value_array
                .iter()
                .map(|v| v.get("valueBit64").and_then(|x| x.as_i64()).unwrap_or(0))
                .collect(),
            ..Default::default()
        },
    }
}

fn lv_props_to_map(props: &[LvProperty]) -> HashMap<String, DynamicParameter> {
    props
        .iter()
        .map(|p| (p.key.clone(), lv_property_to_dynamic_param(p)))
        .collect()
}

// Interactive and NPC entity IDs start above the monster ID range to avoid collisions.
const INTERACTIVE_ID_BASE: u64 = 0x0004_0000_0000_0000;
const NPC_ID_BASE: u64 = 0x0005_0000_0000_0000;

fn interactive_id(level_logic_id: u64) -> u64 {
    INTERACTIVE_ID_BASE | level_logic_id
}

fn npc_id(level_logic_id: u64) -> u64 {
    NPC_ID_BASE | level_logic_id
}

fn pack_interactives(scene_id: &str, assets: &BeyondAssets) -> Vec<SceneInteractive> {
    assets
        .level_data
        .interactives(scene_id)
        .iter()
        .filter(|i| !i.base.default_hide)
        .map(|i| SceneInteractive {
            common_info: Some(SceneObjectCommonInfo {
                id: interactive_id(i.base.level_logic_id),
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
        .filter(|n| !n.base.default_hide)
        .map(|n| SceneNpc {
            common_info: Some(SceneObjectCommonInfo {
                id: npc_id(n.base.level_logic_id),
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

fn pack_level_scripts(scene_id: &str, assets: &BeyondAssets) -> Vec<LevelScriptInfo> {
    assets
        .level_data
        .level_scripts(scene_id)
        .iter()
        .map(|ls| LevelScriptInfo {
            script_id: ls.script_id as i32,
            // All scripts start Inactive (0), the client activates via area triggers
            state: 0,
            properties: lv_props_to_map(&ls.properties),
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

        // Update scene info
        self.scene_id = assets
            .str_id_num
            .get_scene_id(&self.current_scene)
            .unwrap_or(0);

        let char_list = self.pack_scene_chars(char_bag, movement);
        let monster_list = self.pack_scene_monsters(assets, entities);
        let interactive_list = pack_interactives(&self.current_scene, assets);
        let npc_list = pack_npcs(&self.current_scene, assets);

        let enter_view = self.build_object_enter_view_full(
            char_list.clone(),
            monster_list.clone(),
            interactive_list.clone(),
            npc_list.clone(),
        );
        let self_info = self.build_self_scene_info(
            SelfInfoReason::EnterScene,
            char_list,
            monster_list,
            interactive_list,
            npc_list,
            vec![],
            assets,
        );

        (enter_view, self_info)
    }

    pub fn build_object_enter_view(
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

    pub fn build_object_enter_view_full(
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

    pub fn build_object_leave_view(&self, entity_ids: Vec<u64>) -> ScObjectLeaveView {
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

    pub fn build_self_scene_info(
        &self,
        reason: SelfInfoReason,
        char_list: Vec<SceneCharacter>,
        monster_list: Vec<SceneMonster>,
        interactive_list: Vec<SceneInteractive>,
        npc_list: Vec<SceneNpc>,
        revive_chars: Vec<u64>,
        assets: &BeyondAssets,
    ) -> ScSelfSceneInfo {
        let level_scripts = pack_level_scripts(&self.current_scene, assets);

        ScSelfSceneInfo {
            scene_name: self.current_scene.clone(),
            scene_id: self.scene_id,
            detail: Some(SceneObjectDetailContainer {
                char_list,
                monster_list,
                interactive_list,
                npc_list,
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

    // this should be called when you receive CS_SCENE_SET_REPATRIATE_POINT or change mode :)
    pub fn set_revival_mode(&mut self, mode: RevivalMode) {
        self.current_revival_mode = mode;
    }

    pub fn build_entity_destroy(
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

    pub fn build_entity_create(&self, entity_id: u64) -> ScSceneCreateEntity {
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
        // Find all dead characters in current team
        let team = &char_bag.teams[char_bag.meta.curr_team_index as usize];
        let revive_chars: Vec<u64> = team
            .char_team
            .iter()
            .filter_map(|slot| slot.char_index())
            .filter(|&idx| char_bag.chars[idx.as_usize()].is_dead)
            .map(|idx| idx.object_id())
            .collect();

        // Revive them (restore 50% HP)
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

        // Pack scene objects
        let char_list = self.pack_scene_chars(char_bag, movement);
        let monster_list = self.pack_scene_monsters(assets, entities);
        let interactive_list = pack_interactives(&self.current_scene, assets);
        let npc_list = pack_npcs(&self.current_scene, assets);

        let enter_view = self.build_object_enter_view_full(
            char_list.clone(),
            monster_list.clone(),
            interactive_list.clone(),
            npc_list.clone(),
        );
        let self_info = self.build_self_scene_info(
            SelfInfoReason::ReviveDead,
            char_list,
            monster_list,
            interactive_list,
            npc_list,
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
            Some(self.build_object_leave_view(leaving))
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

        let enter_view = self.build_object_enter_view(
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
        if let Some(pos) = char_list
            .iter()
            .position(|c| c.common_info.as_ref().map(|ci| ci.id) == Some(leader_id))
        {
            if pos != 0 {
                let leader_char = char_list.remove(pos);
                char_list.insert(0, leader_char);
            }
        }

        let monster_list = self.pack_monsters_from_manager(entities, assets);
        let self_info = self.build_self_scene_info(
            SelfInfoReason::ChangeTeam,
            char_list,
            monster_list,
            vec![],
            vec![],
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

        // Put leader first if it's part of this new_team_ids.
        let leader_id = char_bag.teams[char_bag.meta.curr_team_index as usize]
            .leader_index
            .object_id();
        if let Some(pos) = char_list
            .iter()
            .position(|c| c.common_info.as_ref().map(|ci| ci.id) == Some(leader_id))
        {
            if pos != 0 {
                let leader_char = char_list.remove(pos);
                char_list.insert(0, leader_char);
            }
        }

        self.build_self_scene_info(
            SelfInfoReason::ChangeTeam,
            char_list,
            monster_list,
            vec![],
            vec![],
            vec![],
            assets,
        )
    }

    pub fn build_teleport(
        &self,
        obj_id_list: Vec<u64>,
        position: Vector,
        rotation: Option<Vector>,
        server_time: u32,
        teleport_reason: i32,
    ) -> ScSceneTeleport {
        ScSceneTeleport {
            obj_id_list,
            scene_name: self.current_scene.clone(),
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

        // Collect in original order
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

        // Move leader to the front if present
        let leader_id = team.leader_index.object_id();
        if let Some(pos) = chars
            .iter()
            .position(|c| c.common_info.as_ref().map(|ci| ci.id) == Some(leader_id))
        {
            if pos != 0 {
                let leader_char = chars.remove(pos);
                chars.insert(0, leader_char);
            }
        }

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

        // Cleanup expired dead entities
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
                // Should be in view. Is it already?
                let already_exists = entities
                    .monsters()
                    .any(|e| e.level_logic_id == enemy.base.level_logic_id);

                // Is it on respawn cooldown?
                let is_on_cooldown = self.dead_entities.contains_key(&enemy.base.level_logic_id);

                if !already_exists && !is_on_cooldown {
                    let id = entities.next_monster_id();
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
            Some(self.build_object_enter_view(vec![], enter_monsters))
        } else {
            None
        };

        let leave_view = if !leave_ids.is_empty() {
            Some(self.build_object_leave_view(leave_ids))
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

    // Pack a single character for dynamic spawning (e.g., multiplayer peer(wink, wink))
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

    // Update scene from world state (call on login/restore)
    pub fn update_from_world(&mut self, world: &crate::player::WorldState, assets: &BeyondAssets) {
        self.current_scene = world.last_scene.clone();
        self.scene_id = assets
            .str_id_num
            .get_scene_id(&world.last_scene)
            .unwrap_or(0);
    }
}
