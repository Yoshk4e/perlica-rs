use crate::character::char_bag::{CharBag, CharIndex};
use crate::entity::{EntityKind, EntityManager, SceneEntity};
use crate::enums::{ParamRealType, ParamValueType};
use crate::interest::MAX_INTEREST_RADIUS_SQ;
use crate::interest::{InterestManager, ReplicationZone, StreamBucket, ZONE_NAMES};
use crate::level_script::LevelScriptManager;
use crate::movement::MovementManager;
use crate::spatial::SpatialGrid;
use crate::traits::{Positioned3D, PositionedExt, Rotated3D};
use config::BeyondAssets;
use config::tables::level_data::LvProperty;
use perlica_proto::{
    DynamicParameter, LeaveObjectInfo, ScEnterSceneNotify, ScLeaveSceneNotify, ScObjectEnterView,
    ScObjectLeaveView, ScSceneCreateEntity, ScSceneDestroyEntity, ScSceneRevival, ScSceneTeleport,
    ScSelfSceneInfo, SceneCharacter, SceneImplEmpty, SceneInteractive, SceneMonster, SceneNpc,
    SceneObjectCommonInfo, SceneObjectDetailContainer, Vector, sc_self_scene_info::SceneImpl,
};
use std::collections::{HashMap, HashSet};

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

// Spatial acceleration data built once per scene transition and discarded on
// the next one.  Keeps the hot path (`update_visible_entities`) out of the
// `BeyondAssets` lookup and linear-scan code.
//
// We build *three* grids, one per streamed entity bucket, because
// each bucket has its own clamped query radius and its own ghost-in path.
// Storing them in distinct grids keeps each query small (a 40 wu interactive
// query never has to walk the larger enemy grid's cells).
#[derive(Debug, Clone)]
struct SceneCache {
    enemy_grid: SpatialGrid,
    interactive_grid: SpatialGrid,
    npc_grid: SpatialGrid,
    resident_ids: HashSet<u64>,
    interactive_props: HashMap<u64, HashMap<String, perlica_proto::DynamicParameter>>,
}

impl SceneCache {
    fn build(scene_id: &str, assets: &BeyondAssets) -> Self {
        // Cell size tuned for the *enemy* outermost cap (Distant = 150 wu)
        // since enemies define the largest typical query.  Interactives /
        // NPCs cap at Combat (80 wu) so they fit comfortably inside the
        // same cell granularity, a single grid cell covers their full
        // query radius in many cases.
        const CELL_SIZE: f32 = 50.0;

        let enemy_grid = SpatialGrid::build(
            assets
                .level_data
                .enemies(scene_id)
                .iter()
                .map(|e| e.base.xz()),
            CELL_SIZE,
        );
        let interactive_grid = SpatialGrid::build(
            assets
                .level_data
                .interactives(scene_id)
                .iter()
                .map(|i| i.base.xz()),
            CELL_SIZE,
        );
        let npc_grid = SpatialGrid::build(
            assets.level_data.npcs(scene_id).iter().map(|n| n.base.xz()),
            CELL_SIZE,
        );

        let interactives = assets.level_data.interactives(scene_id);

        let resident_ids: HashSet<u64> = interactives
            .iter()
            .filter(|i| is_always_resident_interactive(&i.base.template_id, i.base.entity_type))
            .map(|i| i.base.level_logic_id)
            .collect();

        let interactive_props: HashMap<u64, HashMap<String, perlica_proto::DynamicParameter>> =
            interactives
                .iter()
                .map(|i| (i.base.level_logic_id, lv_props_to_map(&i.properties)))
                .collect();

        tracing::debug!(
            "Built spatial grids for '{}': {} enemies, {} interactives ({} resident), {} npcs (cell_size={CELL_SIZE})",
            scene_id,
            enemy_grid.len(),
            interactive_grid.len(),
            resident_ids.len(),
            npc_grid.len(),
        );
        Self {
            enemy_grid,
            interactive_grid,
            npc_grid,
            resident_ids,
            interactive_props,
        }
    }
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
    /// Spatial acceleration structure, rebuilt on each scene transition.
    scene_cache: Option<SceneCache>,
    /// Multi-tiered entity interest / replication state.
    /// Cleared on every scene transition alongside `scene_cache`.
    interest: InterestManager,
    last_check_pos: (f32, f32, f32),
    last_check_ms: u64,
    last_cleanup_ms: u64,
    /// Indices returned by the spatial-grid query.
    candidates_buf: Vec<usize>,
    // Entity IDs scheduled for ghost-out this tick.
    leave_ids_buf: Vec<u64>,
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
            scene_cache: None,
            interest: InterestManager::new(),
            last_check_pos: (f32::MAX, f32::MAX, f32::MAX),
            last_check_ms: 0,
            last_cleanup_ms: 0,
            candidates_buf: Vec::with_capacity(64),
            leave_ids_buf: Vec::with_capacity(32),
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

pub const ALWAYS_RESIDENT_TEMPLATE_PATTERNS: &[&str] = &[
    "campfire",   // int_campfire
    "teleport",   // int_teleport_zone
    "_tp_",       // generic *_tp_* naming
    "save_point", // int_save_point
    "save_group", // int_save_group
    "checkpoint", // generic *_checkpoint_*
    "repatriate", // generic *_repatriate_*
    "dungeon_entry",
    //TODO: not send barriers in case the player already triggered what removed them
    "barrierwall", // int_barrierwall_adv, int_barrierwall_battle,
    "blockage",    // generic *_blockage_*
    "levelgate",   // generic level gates
    "locked_door", // generic locked doors
    "_edoor",
];

// Optional: should classify by `entity_type` integer as well.  Leave empty if i
// don't have stable type IDs; the substring list above is usually enough.
// In map01_lv001 every interactive has `entityType=32` so this is unused.
pub const ALWAYS_RESIDENT_ENTITY_TYPES: &[i32] = &[];

#[inline]
pub fn is_always_resident_interactive(template_id: &str, entity_type: i32) -> bool {
    if ALWAYS_RESIDENT_ENTITY_TYPES.contains(&entity_type) {
        return true;
    }
    let lower = template_id.to_ascii_lowercase();
    ALWAYS_RESIDENT_TEMPLATE_PATTERNS
        .iter()
        .filter(|p| !p.is_empty())
        .any(|p| lower.contains(p))
}

// Pack the *always-resident* subset of a scene's interactives.  These are
// sent in the initial `ScObjectEnterView` / `ScSelfSceneInfo` and the
// streamer skips them thereafter (see `stream_interactives`).
fn pack_resident_interactives(scene_id: &str, assets: &BeyondAssets) -> Vec<SceneInteractive> {
    assets
        .level_data
        .interactives(scene_id)
        .iter()
        .filter(|i| is_always_resident_interactive(&i.base.template_id, i.base.entity_type))
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

// Pack all interactives for a scene as a single batch.
//
// Retained for potential GM / debug paths.  No longer called on the
// scene-load hot path, the streamer in `update_visible_entities` builds
// `SceneInteractive` values one at a time as entities ghost in.
#[allow(dead_code)]
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

#[allow(dead_code)]
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

        // Drop the spatial cache and interest state for the old scene immediately;
        // both will be rebuilt for the new scene in `finish_scene_load`.
        self.scene_cache = None;
        self.interest.clear();
        self.last_check_pos = (f32::MAX, f32::MAX, f32::MAX);
        self.last_check_ms = 0;
        self.last_cleanup_ms = 0;

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

        self.scene_cache = Some(SceneCache::build(&self.current_scene, assets));

        // Scene-load no longer dumps every interactive / NPC up-front.
        // Empirically the bulk-spawn (often hundreds of objects in one
        // packet) is the largest contributor to client-side FPS hitches
        // on scene entry. ( Tehee :3 )
        //
        // Exception: navigation-critical interactives (TPs / campfires /
        // blockages, see `is_always_resident_interactive`) MUST be sent
        // at scene load.  The client's map UI and revival system depend
        // on them being visible from anywhere on the map; streaming them
        // would leave the fast-travel UI broken until the player walked
        // close to each TP.  The list is small (typically a handful per
        // scene) so the cost is negligible.
        let char_list = self.pack_scene_chars(char_bag, movement);
        let monster_list = self.pack_scene_monsters(assets, entities);
        let interactive_list = pack_resident_interactives(&self.current_scene, assets);
        let npc_list: Vec<SceneNpc> = Vec::new();

        // Mirror the residents into the EntityManager + InterestManager
        // so subsequent ghost-out passes (and the streamer's
        // touch_or_classify check) recognise them as already present.
        self.install_resident_interactives(&interactive_list, assets, entities);

        tracing::info!(
            "Scene '{}' loaded: {} chars, {} resident interactives \
             sent up-front; {} streamed interactives and {} npcs",
            self.current_scene,
            char_list.len(),
            interactive_list.len(),
            assets
                .level_data
                .interactives(&self.current_scene)
                .len()
                .saturating_sub(interactive_list.len()),
            assets.level_data.npcs(&self.current_scene).len(),
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

    //TODO: pass correct objecr type
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

        // Notes: the interactives are *already* installed in the interest +
        // entity managers from the original scene load.  We re-pack the
        // protobuf list so the client gets a fresh copy after the revival
        // notify churn, but we don't double-install them server-side.
        let char_list = self.pack_scene_chars(char_bag, movement);
        let monster_list = self.pack_scene_monsters(assets, entities);
        let interactive_list = pack_resident_interactives(&self.current_scene, assets);
        let npc_list: Vec<SceneNpc> = Vec::new();

        tracing::info!(
            "Revival in scene '{}': {} chars, {} monsters, {} resident interactives \
             (rest stream on demand)",
            self.current_scene,
            char_list.len(),
            monster_list.len(),
            interactive_list.len(),
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
        char_ids
            .iter()
            .filter_map(|&objid| {
                let idx = CharIndex::from_object_id(objid);
                char_bag.chars.get(idx.as_usize()).map(|char_data| {
                    let spawn_pos = Vector {
                        x: *movement.pos.get_x(),
                        y: *movement.pos.get_y(),
                        z: *movement.pos.get_z(),
                    };
                    let spawn_rot = Vector {
                        x: *movement.rot.get_x(),
                        y: *movement.rot.get_y(),
                        z: *movement.rot.get_z(),
                    };

                    SceneCharacter {
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
                    }
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

        let mut chars: Vec<SceneCharacter> = team
            .char_team
            .iter()
            .filter_map(|slot| slot.char_index())
            .map(|idx| {
                let char_data = &char_bag.chars[idx.as_usize()];
                let spawn_pos = Vector {
                    x: *movement.pos.get_x(),
                    y: *movement.pos.get_y(),
                    z: *movement.pos.get_z(),
                };
                let spawn_rot = Vector {
                    x: *movement.rot.get_x(),
                    y: *movement.rot.get_y(),
                    z: *movement.rot.get_z(),
                };

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

    #[deprecated]
    pub fn pack_scene_monsters(
        &self,
        _assets: &BeyondAssets,
        _entities: &mut EntityManager,
    ) -> Vec<SceneMonster> {
        // We don't spawn anything by default now.
        // The dynamic radius-based system will handle it.
        vec![]
    }

    // Mirrors the always-resident interactive list into the
    // `EntityManager` + `InterestManager` at scene load.  Once installed,
    // these entries are excluded from the streamer's ghost-in pass (their
    // IDs are already in `interest.entries`) and from the leave-pass
    // (their `always_resident` flag forces retention regardless of
    // distance).
    fn install_resident_interactives(
        &mut self,
        residents: &[SceneInteractive],
        assets: &BeyondAssets,
        entities: &mut EntityManager,
    ) {
        if residents.is_empty() {
            return;
        }
        // Need the position from the source `LvInteractive` (the
        // `SceneInteractive` proto wraps it but going through our own
        // assets path is cheaper than re-parsing common_info).
        let spawns = assets.level_data.interactives(&self.current_scene);
        let now = common::time::now_ms();

        for r in residents {
            let Some(common) = &r.common_info else {
                continue;
            };
            let logic_id = common.id;
            let Some(src) = spawns.iter().find(|i| i.base.level_logic_id == logic_id) else {
                continue;
            };

            entities.insert(SceneEntity {
                id: logic_id,
                template_id: src.base.template_id.clone(),
                kind: EntityKind::Interactive,
                pos_x: src.base.position.x,
                pos_y: src.base.position.y,
                pos_z: src.base.position.z,
                level_logic_id: logic_id,
                belong_level_script_id: src.base.belong_level_script_id,
            });
            self.interest.ghost_in_resident(
                logic_id,
                ReplicationZone::Immediate,
                StreamBucket::Interactive,
                now,
            );
        }
    }

    // Called by the kill-monster network handler.  Atomically:
    //   1. records the kill in `dead_entities` (respawn cooldown), and
    //   2. removes the entity's interest entry so its slot is freed in
    //      the bucket's `live_count`.
    //
    // Without this the interest map and `live_count` desynchronise from
    // the `EntityManager` for up to 500 ms (one Background tick) after a
    // kill, wasting concurrent-cap slots on phantom entries and
    // occasionally producing something like monster disappearing mid combat
    // symptom when the cap rejects the next ghost-in refresh.
    pub fn on_entity_killed(&mut self, level_logic_id: u64) {
        let now = common::time::now_ms();
        self.dead_entities.insert(level_logic_id, now);
        self.interest.ghost_out(level_logic_id);
    }

    // Like `on_entity_killed` but without the respawn-cooldown record.
    // Used for non-enemy entities (interactives, NPCs) that the client
    // reports destroyed.  Only the interest counter is cleaned up.
    pub fn on_entity_despawned(&mut self, level_logic_id: u64) {
        self.interest.ghost_out(level_logic_id);
    }

    /// Per-tick interest update, invoked from the movement packet handler
    /// at ~30-60 Hz.
    ///
    /// # Streaming model (post 4th-pass: bulk-spawn elimination)
    ///
    /// Three entity buckets are streamed independently:
    ///
    /// | Bucket        | Max zone   | Spawns/tick | Concurrent cap |
    /// |---------------|-----------|-------------|----------------|
    /// | Enemy         | Distant   |    6        |     48         |
    /// | Interactive   | Combat    |    8        |     32         |
    /// | NPC           | Combat    |    4        |     16         |
    ///
    /// confirmed by playtesting, is that the client's
    /// FPS is dominated by *how many entities are currently spawned*, not by
    /// the per-tick scheduler overhead.  Two complementary mechanisms keep
    /// the live count low:
    ///
    ///   1. **Per-kind radius caps**: interactives / NPCs never reach Zone 2
    ///      or Zone 3.  A chest 200 wu away is simply not streamed.
    ///
    ///   2. **Per-tick spawn budgets**: even when a spatial query returns
    ///      dozens of candidates (e.g. after a teleport), we cap the number
    ///      of new entities sent per packet.  The remainder trickle in over
    ///      the next few ticks instead of all at once.
    ///
    ///   3. **Concurrent caps**: an absolute ceiling on the number of
    ///      ghosted-in entities per kind.  Beyond this, new candidates are
    ///      simply ignored until existing spawns ghost out.
    pub fn update_visible_entities(
        &mut self,
        pos: (f32, f32, f32),
        assets: &BeyondAssets,
        entities: &mut EntityManager,
    ) -> (Option<ScObjectEnterView>, Option<ScObjectLeaveView>) {
        const RESPAWN_COOLDOWN_MS: u64 = 60_000;
        const CLEANUP_INTERVAL_MS: u64 = 5_000;

        let now = common::time::now_ms();

        self.interest.update_velocity(pos, now);
        let zones_due = self.interest.zones_due(now);
        if self.interest.due_mask() == 0 {
            return (None, None);
        }

        if now.saturating_sub(self.last_cleanup_ms) >= CLEANUP_INTERVAL_MS {
            self.dead_entities
                .retain(|_, &mut killed_at| now.saturating_sub(killed_at) < RESPAWN_COOLDOWN_MS);
            self.last_cleanup_ms = now;
        }

        // The three streamers below all share the same shape but produce
        // different protobuf types.  We accumulate everything into a single
        // `ScObjectEnterView` so the client gets at most one notify per tick.
        let mut enter_monsters: Vec<SceneMonster> = Vec::new();
        let mut enter_interactives: Vec<SceneInteractive> = Vec::new();
        let mut enter_npcs: Vec<SceneNpc> = Vec::new();

        self.stream_enemies(pos, now, &zones_due, assets, entities, &mut enter_monsters);
        self.stream_interactives(
            pos,
            now,
            &zones_due,
            assets,
            entities,
            &mut enter_interactives,
        );
        self.stream_npcs(pos, now, &zones_due, assets, entities, &mut enter_npcs);

        // Iterate the interest map exactly once, regardless of bucket.  We
        // honour each entry's *own* leave-radius (set when it was ghosted in
        // for its bucket), and skip entries whose zone isn't due this tick.
        //
        // Three retention paths bypass the leave-radius check:
        //   (1) `always_resident` (TPs, blockages), never evicted.
        //   (2) Recently-engaged enemies in the open world: enemies whose
        //       `last_close_ms` was bumped in the last STICKY_GRACE_MS
        //       (5 s) AND who remain within COMBAT_STICKY_MAX_RADIUS
        //       (200 wu).  This is the primary fix for the
        //       "monster mid-fight just vanished" symptom.  It works
        //       without any client-side `in_battle` flag, which
        //       is set only for dungeon encounters.
        //   (3) Dungeon battle path: when `in_battle == true`, all
        //       enemies inside the sticky cap are retained as before.
        let in_battle = self.in_battle;
        self.leave_ids_buf.clear();
        for (id, entry) in self.interest.iter_entries() {
            if entry.always_resident {
                continue;
            }
            if !zones_due[entry.zone.index()] {
                continue;
            }
            let Some(e) = entities.get(id) else {
                // Interest map / entity manager out of sync, schedule the
                // orphan id for cleanup so the counters re-sync.
                self.leave_ids_buf.push(id);
                continue;
            };
            let dx = e.pos_x - pos.0;
            let dy = e.pos_y - pos.1;
            let dz = e.pos_z - pos.2;
            let dist_sq = dx * dx + dy * dy + dz * dz;
            if dist_sq <= entry.zone.leave_radius_sq() {
                continue;
            }
            if self.interest.should_retain(entry, dist_sq, now, in_battle) {
                continue;
            }
            self.leave_ids_buf.push(id);
        }

        // Edge case: orphan sweep when Zone 3 is due.  Anything in
        // `entities` but not in `interest` and beyond the global outer
        // radius is unconditionally evicted.
        if zones_due[ReplicationZone::Background.index()] {
            for e in entities.iter() {
                if self.interest.is_ghosted_in(e.id) {
                    continue;
                }
                // Only sweep the kinds we stream, don't accidentally evict
                // characters / projectiles which live in the same map.
                match e.kind {
                    EntityKind::Enemy | EntityKind::Interactive | EntityKind::Npc => {}
                    _ => continue,
                }
                let dx = e.pos_x - pos.0;
                let dy = e.pos_y - pos.1;
                let dz = e.pos_z - pos.2;
                if dx * dx + dy * dy + dz * dz > MAX_INTEREST_RADIUS_SQ {
                    self.leave_ids_buf.push(e.id);
                }
            }
        }

        for &id in &self.leave_ids_buf {
            entities.remove(id);
            self.interest.ghost_out(id);
            tracing::trace!(id, "entity ghosted out");
        }

        tracing::trace!(
            enemies = self.interest.live_count(StreamBucket::Enemy),
            interactives = self.interest.live_count(StreamBucket::Interactive),
            npcs = self.interest.live_count(StreamBucket::Npc),
            speed_wu_s = self.interest.speed_wu_per_s(),
            "interest tick complete",
        );

        let any_enter =
            !enter_monsters.is_empty() || !enter_interactives.is_empty() || !enter_npcs.is_empty();
        let enter_view = if any_enter {
            Some(self.object_enter_view_full(
                vec![],
                enter_monsters,
                enter_interactives,
                enter_npcs,
            ))
        } else {
            None
        };

        let leave_view = if !self.leave_ids_buf.is_empty() {
            Some(self.object_leave_view(self.leave_ids_buf.clone()))
        } else {
            None
        };

        (enter_view, leave_view)
    }

    // The three helpers below all follow the same skeleton:
    //
    //   1. Compute the bucket's clamped query radius.  If the outermost-due
    //      zone is beyond the bucket's `max_zone`, the radius shrinks
    //      accordingly (or returns 0, in which case we skip the bucket).
    //   2. Walk the bucket's spatial grid for candidates.
    //   3. For each candidate: 3-D distance check -> zone classification
    //      (capped) -> single-probe touch-or-classify -> occlusion check (Zone 0
    //      enemies only) -> ghost-in.
    //   4. Stop ghosting in once the per-tick spawn budget is exhausted OR
    //      the concurrent cap is reached.  Remaining candidates are silently
    //      deferred to subsequent ticks.
    //
    // The helpers are kept separate (rather than generic) because the
    // per-bucket protobuf builder differs significantly between
    // SceneMonster / SceneInteractive / SceneNpc, and a generic shim would
    // need a closure or trait object that's harder to inline.

    fn stream_enemies(
        &mut self,
        pos: (f32, f32, f32),
        now: u64,
        zones_due: &[bool; 4],
        assets: &BeyondAssets,
        entities: &mut EntityManager,
        out: &mut Vec<SceneMonster>,
    ) {
        let bucket = StreamBucket::Enemy;
        let max_zone = bucket.max_zone();
        let query_radius = self.interest.max_due_radius_for(max_zone);
        let query_radius_sq = self.interest.max_due_radius_sq_for(max_zone);
        if query_radius <= 0.0 {
            return;
        }

        let spawns = assets.level_data.enemies(&self.current_scene);
        self.candidates_buf.clear();
        match &self.scene_cache {
            Some(cache) => {
                let hits = cache
                    .enemy_grid
                    .query_radius_indices(pos.0, pos.2, query_radius);
                self.candidates_buf.extend_from_slice(&hits);
            }
            None => self.candidates_buf.extend(0..spawns.len()),
        }

        let mut budget = bucket.spawn_budget();

        for &idx in &self.candidates_buf {
            let Some(enemy) = spawns.get(idx) else {
                continue;
            };
            let dist_sq = enemy.base.position.distance_sq_to(&pos);
            if dist_sq > query_radius_sq {
                continue;
            }

            let logic_id = enemy.base.level_logic_id;
            let Some(zone) = ReplicationZone::from_dist_sq_capped(dist_sq, max_zone) else {
                continue;
            };

            // Reclassify-only path for entities whose zone isn't due.
            if !zones_due[zone.index()] {
                self.interest.update_zone(logic_id, zone, now);
                continue;
            }

            // Already ghosted-in?  Refresh the zone in place.
            if self.interest.touch_or_classify(logic_id, zone, now) {
                continue;
            }

            if budget == 0 {
                continue;
            }
            // Concurrent cap reached, stop entirely for this bucket.
            if self.interest.at_capacity(bucket) {
                break;
            }

            if self.dead_entities.contains_key(&logic_id) {
                continue;
            }

            // Height-band occlusion only on the inner-most zone.
            if zone == ReplicationZone::Immediate
                && self
                    .interest
                    .is_occluded(logic_id, pos, enemy.base.position.position(), now)
            {
                continue;
            }

            let (epx, epy, epz) = enemy.base.position.position();
            entities.insert(SceneEntity {
                id: logic_id,
                template_id: enemy.base.template_id.clone(),
                kind: EntityKind::Enemy,
                pos_x: epx,
                pos_y: epy,
                pos_z: epz,
                level_logic_id: logic_id,
                belong_level_script_id: enemy.base.belong_level_script_id,
            });
            self.interest.ghost_in(logic_id, zone, bucket, now);
            budget -= 1;

            if tracing::enabled!(tracing::Level::TRACE) {
                tracing::trace!(
                    zone = ZONE_NAMES[zone.index()],
                    id = logic_id,
                    dist_wu = dist_sq.sqrt(),
                    "enemy ghosted in",
                );
            }

            out.push(SceneMonster {
                common_info: Some(SceneObjectCommonInfo {
                    id: logic_id,
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
                origin_id: logic_id,
                level: enemy.level as i32,
            });
        }
    }

    fn stream_interactives(
        &mut self,
        pos: (f32, f32, f32),
        now: u64,
        zones_due: &[bool; 4],
        assets: &BeyondAssets,
        entities: &mut EntityManager,
        out: &mut Vec<SceneInteractive>,
    ) {
        let bucket = StreamBucket::Interactive;
        let max_zone = bucket.max_zone();
        let query_radius = self.interest.max_due_radius_for(max_zone);
        let query_radius_sq = self.interest.max_due_radius_sq_for(max_zone);
        if query_radius <= 0.0 {
            return;
        }

        let spawns = assets.level_data.interactives(&self.current_scene);
        self.candidates_buf.clear();
        match &self.scene_cache {
            Some(cache) => {
                let hits = cache
                    .interactive_grid
                    .query_radius_indices(pos.0, pos.2, query_radius);
                self.candidates_buf.extend_from_slice(&hits);
            }
            None => self.candidates_buf.extend(0..spawns.len()),
        }

        let mut budget = bucket.spawn_budget();

        for &idx in &self.candidates_buf {
            let Some(interactive) = spawns.get(idx) else {
                continue;
            };

            let is_resident = self
                .scene_cache
                .as_ref()
                .map(|c| c.resident_ids.contains(&interactive.base.level_logic_id))
                .unwrap_or_else(|| {
                    is_always_resident_interactive(
                        &interactive.base.template_id,
                        interactive.base.entity_type,
                    )
                });
            if is_resident {
                continue;
            }

            let dist_sq = interactive.base.position.distance_sq_to(&pos);
            if dist_sq > query_radius_sq {
                continue;
            }

            let logic_id = interactive.base.level_logic_id;
            let Some(zone) = ReplicationZone::from_dist_sq_capped(dist_sq, max_zone) else {
                continue;
            };

            if !zones_due[zone.index()] {
                self.interest.update_zone(logic_id, zone, now);
                continue;
            }
            if self.interest.touch_or_classify(logic_id, zone, now) {
                continue;
            }
            if budget == 0 {
                continue;
            }
            if self.interest.at_capacity(bucket) {
                break;
            }

            entities.insert(SceneEntity {
                id: logic_id,
                template_id: interactive.base.template_id.clone(),
                kind: EntityKind::Interactive,
                pos_x: interactive.base.position.x,
                pos_y: interactive.base.position.y,
                pos_z: interactive.base.position.z,
                level_logic_id: logic_id,
                belong_level_script_id: interactive.base.belong_level_script_id,
            });
            self.interest.ghost_in(logic_id, zone, bucket, now);
            budget -= 1;

            if tracing::enabled!(tracing::Level::TRACE) {
                tracing::trace!(
                    zone = ZONE_NAMES[zone.index()],
                    id = logic_id,
                    dist_wu = dist_sq.sqrt(),
                    "interactive ghosted in",
                );
            }

            out.push(SceneInteractive {
                common_info: Some(SceneObjectCommonInfo {
                    id: logic_id,
                    r#type: interactive.base.entity_type,
                    templateid: interactive.base.template_id.clone(),
                    position: Some(Vector {
                        x: interactive.base.position.x,
                        y: interactive.base.position.y,
                        z: interactive.base.position.z,
                    }),
                    rotation: Some(Vector {
                        x: interactive.base.rotation.x,
                        y: interactive.base.rotation.y,
                        z: interactive.base.rotation.z,
                    }),
                    belong_level_script_id: interactive.base.belong_level_script_id,
                }),
                origin_id: logic_id,
                properties: self
                    .scene_cache
                    .as_ref()
                    .and_then(|c| c.interactive_props.get(&logic_id))
                    .cloned()
                    .unwrap_or_else(|| lv_props_to_map(&interactive.properties)),
            });
        }
    }

    fn stream_npcs(
        &mut self,
        pos: (f32, f32, f32),
        now: u64,
        zones_due: &[bool; 4],
        assets: &BeyondAssets,
        entities: &mut EntityManager,
        out: &mut Vec<SceneNpc>,
    ) {
        let bucket = StreamBucket::Npc;
        let max_zone = bucket.max_zone();
        let query_radius = self.interest.max_due_radius_for(max_zone);
        let query_radius_sq = self.interest.max_due_radius_sq_for(max_zone);
        if query_radius <= 0.0 {
            return;
        }

        let spawns = assets.level_data.npcs(&self.current_scene);
        self.candidates_buf.clear();
        match &self.scene_cache {
            Some(cache) => {
                let hits = cache
                    .npc_grid
                    .query_radius_indices(pos.0, pos.2, query_radius);
                self.candidates_buf.extend_from_slice(&hits);
            }
            None => self.candidates_buf.extend(0..spawns.len()),
        }

        let mut budget = bucket.spawn_budget();

        for &idx in &self.candidates_buf {
            let Some(npc) = spawns.get(idx) else {
                continue;
            };
            let dist_sq = npc.base.position.distance_sq_to(&pos);
            if dist_sq > query_radius_sq {
                continue;
            }

            let logic_id = npc.base.level_logic_id;
            let Some(zone) = ReplicationZone::from_dist_sq_capped(dist_sq, max_zone) else {
                continue;
            };

            if !zones_due[zone.index()] {
                self.interest.update_zone(logic_id, zone, now);
                continue;
            }
            if self.interest.touch_or_classify(logic_id, zone, now) {
                continue;
            }
            if budget == 0 {
                continue;
            }
            if self.interest.at_capacity(bucket) {
                break;
            }

            entities.insert(SceneEntity {
                id: logic_id,
                template_id: npc.base.template_id.clone(),
                kind: EntityKind::Npc,
                pos_x: npc.base.position.x,
                pos_y: npc.base.position.y,
                pos_z: npc.base.position.z,
                level_logic_id: logic_id,
                belong_level_script_id: npc.base.belong_level_script_id,
            });
            self.interest.ghost_in(logic_id, zone, bucket, now);
            budget -= 1;

            if tracing::enabled!(tracing::Level::TRACE) {
                tracing::trace!(
                    zone = ZONE_NAMES[zone.index()],
                    id = logic_id,
                    dist_wu = dist_sq.sqrt(),
                    "npc ghosted in",
                );
            }

            out.push(SceneNpc {
                common_info: Some(SceneObjectCommonInfo {
                    id: logic_id,
                    r#type: npc.base.entity_type,
                    templateid: npc.base.template_id.clone(),
                    position: Some(Vector {
                        x: npc.base.position.x,
                        y: npc.base.position.y,
                        z: npc.base.position.z,
                    }),
                    rotation: Some(Vector {
                        x: npc.base.rotation.x,
                        y: npc.base.rotation.y,
                        z: npc.base.rotation.z,
                    }),
                    belong_level_script_id: npc.base.belong_level_script_id,
                }),
            });
        }
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

    /// Updates (or inserts) a single property on an interactive object in the
    /// scene cache.  Returns `Some(old_value)` if the property existed before,
    /// or `None` if it was newly inserted.  Returns `None` (no-op) if the
    /// scene cache has not been built yet or the interactive `id` is unknown.
    pub fn update_interactive_property(
        &mut self,
        id: u64,
        key: &str,
        value: DynamicParameter,
    ) -> Option<DynamicParameter> {
        let cache = self.scene_cache.as_mut()?;
        let props = cache.interactive_props.get_mut(&id)?;
        props.insert(key.to_string(), value)
    }

    /// Returns a clone of the current properties for an interactive object,
    /// if the scene cache is active and the `id` is known.
    pub fn get_interactive_properties(&self, id: u64) -> Option<HashMap<String, DynamicParameter>> {
        let cache = self.scene_cache.as_ref()?;
        Some(cache.interactive_props.get(&id)?.clone())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resident_teleport() {
        assert!(is_always_resident_interactive("int_teleport_zone_001", 0));
    }

    #[test]
    fn resident_campfire() {
        assert!(is_always_resident_interactive("int_campfire_01", 0));
    }

    #[test]
    fn resident_save_point() {
        assert!(is_always_resident_interactive("int_save_point_02", 0));
    }

    #[test]
    fn resident_checkpoint() {
        assert!(is_always_resident_interactive("int_checkpoint_main", 0));
    }

    #[test]
    fn resident_repatriate() {
        assert!(is_always_resident_interactive("int_repatriate_a", 0));
    }

    #[test]
    fn resident_barrierwall() {
        assert!(is_always_resident_interactive("int_barrierwall_adv", 0));
    }

    #[test]
    fn resident_blockage() {
        assert!(is_always_resident_interactive("int_blockage_path", 0));
    }

    #[test]
    fn resident_levelgate() {
        assert!(is_always_resident_interactive("int_levelgate_01", 0));
    }

    #[test]
    fn resident_dungeon_entry() {
        assert!(is_always_resident_interactive("int_dungeon_entry", 0));
    }

    #[test]
    fn resident_tp_substring() {
        assert!(is_always_resident_interactive("obj_tp_zone", 0));
    }

    #[test]
    fn resident_edoor() {
        assert!(is_always_resident_interactive("int_edoor_main", 0));
    }

    #[test]
    fn non_resident_regular_interactive() {
        assert!(!is_always_resident_interactive("int_loot_chest_01", 0));
    }

    #[test]
    fn non_resident_random_string() {
        assert!(!is_always_resident_interactive("enemy_spawn_01", 0));
    }

    #[test]
    fn resident_case_insensitive() {
        assert!(is_always_resident_interactive("INT_TELEPORT_ZONE", 0));
        assert!(is_always_resident_interactive("Int_Campfire_01", 0));
    }

    #[test]
    fn resident_by_entity_type() {
        // With empty ALWAYS_RESIDENT_ENTITY_TYPES, no entity_type matches
        assert!(!is_always_resident_interactive("some_template", 32));
    }

    fn make_scene_char(id: u64) -> SceneCharacter {
        SceneCharacter {
            common_info: Some(SceneObjectCommonInfo {
                id,
                templateid: format!("char_{}", id),
                position: None,
                rotation: None,
                belong_level_script_id: 0,
                r#type: 8,
            }),
            level: 1,
            name: "Test".to_string(),
        }
    }

    #[test]
    fn move_leader_already_at_front() {
        let mut chars = vec![make_scene_char(1), make_scene_char(2), make_scene_char(3)];
        move_leader_to_front(&mut chars, 1);
        assert_eq!(chars[0].common_info.as_ref().unwrap().id, 1);
    }

    #[test]
    fn move_leader_from_back_to_front() {
        let mut chars = vec![make_scene_char(2), make_scene_char(3), make_scene_char(1)];
        move_leader_to_front(&mut chars, 1);
        assert_eq!(chars[0].common_info.as_ref().unwrap().id, 1);
    }

    #[test]
    fn move_leader_from_middle() {
        let mut chars = vec![make_scene_char(2), make_scene_char(1), make_scene_char(3)];
        move_leader_to_front(&mut chars, 1);
        assert_eq!(chars[0].common_info.as_ref().unwrap().id, 1);
    }

    #[test]
    fn move_leader_not_found_no_change() {
        let mut chars = vec![make_scene_char(2), make_scene_char(3)];
        move_leader_to_front(&mut chars, 999);
        assert_eq!(chars[0].common_info.as_ref().unwrap().id, 2);
    }

    #[test]
    fn move_leader_empty_slice() {
        let mut chars: Vec<SceneCharacter> = vec![];
        move_leader_to_front(&mut chars, 1);
        assert!(chars.is_empty());
    }

    #[test]
    fn checkpoint_info_default() {
        let cp = CheckpointInfo::default();
        assert!(cp.scene_name.is_empty());
    }

    #[test]
    fn scene_loading_state_default_is_idle() {
        assert_eq!(SceneLoadingState::default(), SceneLoadingState::Idle);
    }

    #[test]
    fn revival_mode_default() {
        assert_eq!(RevivalMode::default(), RevivalMode::Default);
    }

    #[test]
    fn self_info_reason_values() {
        assert_eq!(SelfInfoReason::EnterScene as i32, 0);
        assert_eq!(SelfInfoReason::ChangeTeam as i32, 3);
    }

    #[test]
    fn entity_destroy_reason_values() {
        assert_eq!(EntityDestroyReason::Immediately as i32, 0);
        assert_eq!(EntityDestroyReason::Dead as i32, 1);
    }

    #[test]
    fn scene_manager_new_matches_default() {
        let new_mgr = SceneManager::new();
        let default_mgr = SceneManager::default();
        assert_eq!(new_mgr.current_scene, default_mgr.current_scene);
        assert_eq!(new_mgr.scene_id, default_mgr.scene_id);
        assert_eq!(new_mgr.loading_state, default_mgr.loading_state);
        assert_eq!(new_mgr.in_battle, default_mgr.in_battle);
        assert_eq!(
            new_mgr.checkpoint.is_none(),
            default_mgr.checkpoint.is_none()
        );
        assert_eq!(
            new_mgr.current_revival_mode,
            default_mgr.current_revival_mode
        );
    }

    #[test]
    fn scene_manager_default_values() {
        let mgr = SceneManager::default();
        assert_eq!(mgr.current_scene, "map01_lv001");
        assert_eq!(mgr.scene_id, 0);
        assert_eq!(mgr.loading_state, SceneLoadingState::Idle);
        assert!(!mgr.in_battle);
        assert!(mgr.checkpoint.is_none());
        assert_eq!(mgr.current_revival_mode, RevivalMode::Default);
        assert!(mgr.dead_entities.is_empty());
    }

    #[test]
    fn set_revival_mode_changes_mode() {
        let mut mgr = SceneManager::default();
        assert_eq!(mgr.current_revival_mode, RevivalMode::Default);
        mgr.set_revival_mode(RevivalMode::RepatriatePoint);
        assert_eq!(mgr.current_revival_mode, RevivalMode::RepatriatePoint);
        mgr.set_revival_mode(RevivalMode::CheckPoint);
        assert_eq!(mgr.current_revival_mode, RevivalMode::CheckPoint);
        mgr.set_revival_mode(RevivalMode::Default);
        assert_eq!(mgr.current_revival_mode, RevivalMode::Default);
    }

    #[test]
    fn set_battle_mode_toggles() {
        let mut mgr = SceneManager::default();
        assert!(!mgr.in_battle);
        mgr.set_battle_mode(true);
        assert!(mgr.in_battle);
        mgr.set_battle_mode(false);
        assert!(!mgr.in_battle);
    }

    #[test]
    fn is_in_scene_false_when_idle() {
        let mgr = SceneManager::default();
        assert!(!mgr.is_in_scene());
    }

    #[test]
    fn is_in_scene_false_when_loading() {
        let mut mgr = SceneManager::default();
        mgr.loading_state = SceneLoadingState::Loading;
        assert!(!mgr.is_in_scene());
    }

    #[test]
    fn is_in_scene_true_when_active() {
        let mut mgr = SceneManager::default();
        mgr.loading_state = SceneLoadingState::Active;
        assert!(mgr.is_in_scene());
    }

    #[test]
    fn scene_name_returns_current_scene() {
        let mgr = SceneManager::default();
        assert_eq!(mgr.scene_name(), "map01_lv001");
    }

    #[test]
    fn destroy_entity_immediately() {
        let mgr = SceneManager::default();
        let result = mgr.destroy_entity(42, EntityDestroyReason::Immediately);
        assert_eq!(result.id, 42);
        assert_eq!(result.reason, EntityDestroyReason::Immediately as i32);
        assert_eq!(result.scene_name, "map01_lv001");
    }

    #[test]
    fn destroy_entity_dead() {
        let mgr = SceneManager::default();
        let result = mgr.destroy_entity(99, EntityDestroyReason::Dead);
        assert_eq!(result.id, 99);
        assert_eq!(result.reason, EntityDestroyReason::Dead as i32);
    }

    #[test]
    fn create_entity_contains_id() {
        let mgr = SceneManager::default();
        let result = mgr.create_entity(123);
        assert_eq!(result.id, 123);
        assert_eq!(result.scene_name, "map01_lv001");
    }

    #[test]
    fn object_enter_view_empty_lists() {
        let mgr = SceneManager::default();
        let result = mgr.object_enter_view(vec![], vec![]);
        assert_eq!(result.scene_name, "map01_lv001");
        assert_eq!(result.scene_id, 0);
        assert!(!result.has_extra_object);
        let detail = result.detail.unwrap();
        assert!(detail.char_list.is_empty());
        assert!(detail.monster_list.is_empty());
        assert!(detail.interactive_list.is_empty());
        assert!(detail.npc_list.is_empty());
        assert!(detail.summon_list.is_empty());
    }

    #[test]
    fn object_enter_view_with_chars() {
        let mgr = SceneManager::default();
        let chars = vec![make_scene_char(1), make_scene_char(2)];
        let result = mgr.object_enter_view(chars, vec![]);
        let detail = result.detail.unwrap();
        assert_eq!(detail.char_list.len(), 2);
        assert!(detail.interactive_list.is_empty());
        assert!(detail.npc_list.is_empty());
    }

    #[test]
    fn object_enter_view_full_with_data() {
        let mgr = SceneManager::default();
        let chars = vec![make_scene_char(1)];
        let result = mgr.object_enter_view_full(chars, vec![], vec![], vec![]);
        let detail = result.detail.unwrap();
        assert_eq!(detail.char_list.len(), 1);
        assert!(detail.monster_list.is_empty());
        assert!(detail.interactive_list.is_empty());
        assert!(detail.npc_list.is_empty());
        assert!(detail.summon_list.is_empty());
    }

    #[test]
    fn object_leave_view_maps_ids() {
        let mgr = SceneManager::default();
        let result = mgr.object_leave_view(vec![10, 20, 30]);
        assert_eq!(result.scene_name, "map01_lv001");
        assert_eq!(result.obj_list.len(), 3);
        assert_eq!(result.obj_list[0].obj_id, 10);
        assert_eq!(result.obj_list[1].obj_id, 20);
        assert_eq!(result.obj_list[2].obj_id, 30);
        // obj_type is always 0 in the current implementation
        assert_eq!(result.obj_list[0].obj_type, 0);
    }

    #[test]
    fn object_leave_view_empty_ids() {
        let mgr = SceneManager::default();
        let result = mgr.object_leave_view(vec![]);
        assert!(result.obj_list.is_empty());
    }

    #[test]
    fn teleport_uses_current_scene_by_default() {
        let mgr = SceneManager::default();
        let result = mgr.teleport(
            vec![1, 2],
            Vector {
                x: 10.0,
                y: 20.0,
                z: 30.0,
            },
            None,
            12345,
            5,
            None,
        );
        assert_eq!(result.scene_name, "map01_lv001");
        assert_eq!(result.obj_id_list, vec![1, 2]);
        let pos = result.position.unwrap();
        assert_eq!(pos.x, 10.0);
        assert_eq!(pos.y, 20.0);
        assert_eq!(pos.z, 30.0);
        assert_eq!(result.server_time, 12345);
        assert_eq!(result.teleport_reason, 5);
    }

    #[test]
    fn teleport_overrides_scene_name() {
        let mgr = SceneManager::default();
        let result = mgr.teleport(
            vec![],
            Vector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            None,
            0,
            0,
            Some("custom_scene".to_string()),
        );
        assert_eq!(result.scene_name, "custom_scene");
    }

    #[test]
    fn teleport_with_rotation() {
        let mgr = SceneManager::default();
        let rot = Vector {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        let result = mgr.teleport(
            vec![42],
            Vector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            Some(rot.clone()),
            0,
            0,
            None,
        );
        let result_rot = result.rotation.unwrap();
        assert_eq!(result_rot.x, 1.0);
        assert_eq!(result_rot.y, 2.0);
        assert_eq!(result_rot.z, 3.0);
    }

    #[test]
    fn set_and_get_checkpoint() {
        let mut mgr = SceneManager::default();
        assert!(mgr.get_checkpoint().is_none());
        let cp = CheckpointInfo {
            scene_name: "map01_lv002".to_string(),
            pos_x: 100.0,
            pos_y: 200.0,
            pos_z: 300.0,
        };
        mgr.set_checkpoint(cp);
        let result = mgr.get_checkpoint().unwrap();
        assert_eq!(result.scene_name, "map01_lv002");
        assert_eq!(result.pos_x, 100.0);
        assert_eq!(result.pos_y, 200.0);
        assert_eq!(result.pos_z, 300.0);
    }

    #[test]
    fn checkpoint_overwrite() {
        let mut mgr = SceneManager::default();
        mgr.set_checkpoint(CheckpointInfo {
            scene_name: "first".to_string(),
            pos_x: 1.0,
            pos_y: 2.0,
            pos_z: 3.0,
        });
        mgr.set_checkpoint(CheckpointInfo {
            scene_name: "second".to_string(),
            pos_x: 4.0,
            pos_y: 5.0,
            pos_z: 6.0,
        });
        assert_eq!(mgr.get_checkpoint().unwrap().scene_name, "second");
    }

    #[test]
    fn pack_single_monster_from_entity() {
        let mgr = SceneManager::default();
        let entity = SceneEntity {
            id: 1001,
            template_id: "monster_001".to_string(),
            kind: EntityKind::Enemy,
            pos_x: 10.0,
            pos_y: 20.0,
            pos_z: 30.0,
            level_logic_id: 500,
            belong_level_script_id: 7,
        };
        let result = mgr.pack_single_monster(&entity, 5, 500);
        let common = result.common_info.unwrap();
        assert_eq!(common.id, 1001);
        assert_eq!(common.templateid, "monster_001");
        assert_eq!(common.r#type, 16);
        let pos = common.position.unwrap();
        assert_eq!(pos.x, 10.0);
        assert_eq!(pos.y, 20.0);
        assert_eq!(pos.z, 30.0);
        assert!(common.rotation.is_none());
        assert_eq!(common.belong_level_script_id, 0);
        assert_eq!(result.origin_id, 500);
        assert_eq!(result.level, 5);
    }

    #[test]
    fn pack_single_char_basic() {
        let mgr = SceneManager::default();
        let result = mgr.pack_single_char(
            42,
            "char_template".to_string(),
            10,
            Vector {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            Vector {
                x: 4.0,
                y: 5.0,
                z: 6.0,
            },
        );
        let common = result.common_info.unwrap();
        assert_eq!(common.id, 42);
        assert_eq!(common.templateid, "char_template");
        assert_eq!(common.r#type, 8);
        let pos = common.position.unwrap();
        assert_eq!(pos.x, 1.0);
        assert_eq!(pos.y, 2.0);
        assert_eq!(pos.z, 3.0);
        let rot = common.rotation.unwrap();
        assert_eq!(rot.x, 4.0);
        assert_eq!(rot.y, 5.0);
        assert_eq!(rot.z, 6.0);
        assert_eq!(result.level, 10);
        assert_eq!(result.name, "Player");
    }

    #[test]
    fn self_info_reason_all_discriminants() {
        assert_eq!(SelfInfoReason::EnterScene as i32, 0);
        assert_eq!(SelfInfoReason::ReviveDead as i32, 1);
        assert_eq!(SelfInfoReason::ReviveRest as i32, 2);
        assert_eq!(SelfInfoReason::ChangeTeam as i32, 3);
        assert_eq!(SelfInfoReason::ReviveByItem as i32, 4);
        assert_eq!(SelfInfoReason::ResetDungeon as i32, 5);
    }

    #[test]
    fn revival_mode_all_variants() {
        assert_eq!(RevivalMode::Default as i32, 0);
        assert_eq!(RevivalMode::RepatriatePoint as i32, 1);
        assert_eq!(RevivalMode::CheckPoint as i32, 2);
        assert_eq!(RevivalMode::default(), RevivalMode::Default);
    }

    #[test]
    fn scene_loading_state_variants() {
        assert_ne!(SceneLoadingState::Idle, SceneLoadingState::Loading);
        assert_ne!(SceneLoadingState::Loading, SceneLoadingState::Active);
        assert_ne!(SceneLoadingState::Idle, SceneLoadingState::Active);
        assert_eq!(SceneLoadingState::default(), SceneLoadingState::Idle);
    }

    #[test]
    fn checkpoint_info_with_values() {
        let cp = CheckpointInfo {
            scene_name: "test_scene".to_string(),
            pos_x: 1.0,
            pos_y: 2.0,
            pos_z: 3.0,
        };
        assert_eq!(cp.scene_name, "test_scene");
        assert_eq!(cp.pos_x, 1.0);
        assert_eq!(cp.pos_y, 2.0);
        assert_eq!(cp.pos_z, 3.0);
    }

    #[test]
    fn lv_property_invalid_type() {
        let prop = LvProperty {
            key: "test".to_string(),
            value: serde_json::json!({
                "type": 0,
                "valueArray": []
            }),
        };
        let result = lv_property_to_dynamic_param(&prop);
        assert_eq!(result.value_type, ParamValueType::Invalid as i32);
        assert_eq!(result.real_type, 0);
    }

    #[test]
    fn lv_property_enum_type() {
        let prop = LvProperty {
            key: "test".to_string(),
            value: serde_json::json!({
                "type": 35,
                "valueArray": []
            }),
        };
        let result = lv_property_to_dynamic_param(&prop);
        assert_eq!(result.value_type, ParamValueType::Invalid as i32);
        assert_eq!(result.real_type, 35);
    }

    #[test]
    fn lv_property_bool_type() {
        let prop = LvProperty {
            key: "test".to_string(),
            value: serde_json::json!({
                "type": 1,
                "valueArray": [
                    {"valueBit64": 1},
                    {"valueBit64": 0},
                    {"valueBit64": 1}
                ]
            }),
        };
        let result = lv_property_to_dynamic_param(&prop);
        assert_eq!(result.value_type, ParamRealType::Bool as i32);
        assert_eq!(result.value_bool_list, vec![true, false, true]);
    }

    #[test]
    fn lv_property_bool_list_type() {
        let prop = LvProperty {
            key: "test".to_string(),
            value: serde_json::json!({
                "type": 2,
                "valueArray": [
                    {"valueBit64": 0},
                    {"valueBit64": 1}
                ]
            }),
        };
        let result = lv_property_to_dynamic_param(&prop);
        assert_eq!(result.value_type, ParamRealType::BoolList as i32);
        assert_eq!(result.value_bool_list, vec![false, true]);
    }

    #[test]
    fn lv_property_int_type() {
        let prop = LvProperty {
            key: "test".to_string(),
            value: serde_json::json!({
                "type": 3,
                "valueArray": [
                    {"valueBit64": 42},
                    {"valueBit64": -10}
                ]
            }),
        };
        let result = lv_property_to_dynamic_param(&prop);
        assert_eq!(result.value_type, ParamValueType::Int as i32);
        assert_eq!(result.value_int_list, vec![42, -10]);
    }

    #[test]
    fn lv_property_int_list_type() {
        let prop = LvProperty {
            key: "test".to_string(),
            value: serde_json::json!({
                "type": 4,
                "valueArray": [
                    {"valueBit64": 1},
                    {"valueBit64": 2},
                    {"valueBit64": 3}
                ]
            }),
        };
        let result = lv_property_to_dynamic_param(&prop);
        assert_eq!(result.value_type, ParamValueType::IntList as i32);
        assert_eq!(result.value_int_list, vec![1, 2, 3]);
    }

    #[test]
    fn lv_property_uint_type_mapped_to_int() {
        let prop = LvProperty {
            key: "test".to_string(),
            value: serde_json::json!({
                "type": 17,
                "valueArray": [{"valueBit64": 999}]
            }),
        };
        let result = lv_property_to_dynamic_param(&prop);
        assert_eq!(result.value_type, ParamValueType::Int as i32);
        assert_eq!(result.value_int_list, vec![999]);
    }

    #[test]
    fn lv_property_float_type_positive() {
        let bits = 3.14f32.to_bits() as i64;
        let prop = LvProperty {
            key: "test".to_string(),
            value: serde_json::json!({
                "type": 5,
                "valueArray": [{"valueBit64": bits}]
            }),
        };
        let result = lv_property_to_dynamic_param(&prop);
        assert_eq!(result.value_type, ParamValueType::Float as i32);
        assert!((result.value_float_list[0] - 3.14f32).abs() < 0.01);
    }

    #[test]
    fn lv_property_float_type_negative_fallback_to_int() {
        let prop = LvProperty {
            key: "test".to_string(),
            value: serde_json::json!({
                "type": 5,
                "valueArray": [{"valueBit64": -1}]
            }),
        };
        let result = lv_property_to_dynamic_param(&prop);
        assert_eq!(result.value_type, ParamValueType::Int as i32);
        assert_eq!(result.value_int_list, vec![-1]);
    }

    #[test]
    fn lv_property_float_list_type() {
        let val1 = 1.5f32.to_bits() as i64;
        let val2 = 2.5f32.to_bits() as i64;
        let prop = LvProperty {
            key: "test".to_string(),
            value: serde_json::json!({
                "type": 6,
                "valueArray": [
                    {"valueBit64": val1},
                    {"valueBit64": val2}
                ]
            }),
        };
        let result = lv_property_to_dynamic_param(&prop);
        assert_eq!(result.value_type, ParamValueType::FloatList as i32);
        assert!((result.value_float_list[0] - 1.5f32).abs() < 0.01);
        assert!((result.value_float_list[1] - 2.5f32).abs() < 0.01);
    }

    #[test]
    fn lv_property_string_type() {
        let prop = LvProperty {
            key: "test".to_string(),
            value: serde_json::json!({
                "type": 7,
                "valueArray": [
                    {"valueString": "hello"},
                    {"valueString": "world"}
                ]
            }),
        };
        let result = lv_property_to_dynamic_param(&prop);
        assert_eq!(result.value_type, ParamValueType::String as i32);
        assert_eq!(result.value_string_list, vec!["hello", "world"]);
    }

    #[test]
    fn lv_property_string_list_type() {
        let prop = LvProperty {
            key: "test".to_string(),
            value: serde_json::json!({
                "type": 8,
                "valueArray": [
                    {"valueString": "a"},
                    {"valueString": "b"}
                ]
            }),
        };
        let result = lv_property_to_dynamic_param(&prop);
        assert_eq!(result.value_type, ParamValueType::StringList as i32);
        assert_eq!(result.value_string_list, vec!["a", "b"]);
    }

    #[test]
    fn lv_property_path_type_maps_to_string() {
        let prop = LvProperty {
            key: "test".to_string(),
            value: serde_json::json!({
                "type": 9,
                "valueArray": [{"valueString": "/path/to/file"}]
            }),
        };
        let result = lv_property_to_dynamic_param(&prop);
        assert_eq!(result.value_type, ParamValueType::String as i32);
        assert_eq!(result.value_string_list, vec!["/path/to/file"]);
    }

    #[test]
    fn lv_property_path_list_type_maps_to_string_list() {
        let prop = LvProperty {
            key: "test".to_string(),
            value: serde_json::json!({
                "type": 10,
                "valueArray": [{"valueString": "/a"}, {"valueString": "/b"}]
            }),
        };
        let result = lv_property_to_dynamic_param(&prop);
        assert_eq!(result.value_type, ParamValueType::StringList as i32);
    }

    #[test]
    fn lv_property_vector3_type() {
        let x_bits = 1.0f32.to_bits() as i64;
        let y_bits = 2.0f32.to_bits() as i64;
        let z_bits = 3.0f32.to_bits() as i64;
        let prop = LvProperty {
            key: "test".to_string(),
            value: serde_json::json!({
                "type": 11,
                "valueArray": [
                    {"valueBit64": x_bits},
                    {"valueBit64": y_bits},
                    {"valueBit64": z_bits}
                ]
            }),
        };
        let result = lv_property_to_dynamic_param(&prop);
        assert_eq!(result.value_type, ParamValueType::FloatList as i32);
        assert_eq!(result.value_float_list.len(), 3);
    }

    #[test]
    fn lv_property_entity_ptr_maps_to_int() {
        let prop = LvProperty {
            key: "test".to_string(),
            value: serde_json::json!({
                "type": 13,
                "valueArray": [{"valueBit64": 42}]
            }),
        };
        let result = lv_property_to_dynamic_param(&prop);
        assert_eq!(result.value_type, ParamValueType::Int as i32);
        assert_eq!(result.value_int_list, vec![42]);
    }

    #[test]
    fn lv_property_tag_type_maps_to_string() {
        let prop = LvProperty {
            key: "test".to_string(),
            value: serde_json::json!({
                "type": 15,
                "valueArray": [{"valueString": "some_tag"}]
            }),
        };
        let result = lv_property_to_dynamic_param(&prop);
        assert_eq!(result.value_type, ParamValueType::String as i32);
        assert_eq!(result.value_string_list, vec!["some_tag"]);
    }

    #[test]
    fn lv_property_missing_type_defaults_to_invalid() {
        let prop = LvProperty {
            key: "test".to_string(),
            value: serde_json::json!({
                "valueArray": []
            }),
        };
        let result = lv_property_to_dynamic_param(&prop);
        assert_eq!(result.value_type, ParamValueType::Invalid as i32);
        assert_eq!(result.real_type, 0);
    }

    #[test]
    fn lv_property_missing_value_array_defaults_empty() {
        let prop = LvProperty {
            key: "test".to_string(),
            value: serde_json::json!({
                "type": 3
            }),
        };
        let result = lv_property_to_dynamic_param(&prop);
        assert_eq!(result.value_type, ParamValueType::Int as i32);
        assert!(result.value_int_list.is_empty());
    }

    #[test]
    fn lv_props_to_map_converts_multiple() {
        let props = vec![
            LvProperty {
                key: "int_prop".to_string(),
                value: serde_json::json!({"type": 3, "valueArray": [{"valueBit64": 10}]}),
            },
            LvProperty {
                key: "bool_prop".to_string(),
                value: serde_json::json!({"type": 1, "valueArray": [{"valueBit64": 1}]}),
            },
        ];
        let map = lv_props_to_map(&props);
        assert_eq!(map.len(), 2);
        assert!(map.contains_key("int_prop"));
        assert!(map.contains_key("bool_prop"));
        assert_eq!(map["int_prop"].value_int_list, vec![10]);
        assert_eq!(map["bool_prop"].value_bool_list, vec![true]);
    }

    #[test]
    fn lv_props_to_map_empty() {
        let map = lv_props_to_map(&[]);
        assert!(map.is_empty());
    }

    #[test]
    fn on_entity_killed_records_dead_and_ghosts_out() {
        let mut mgr = SceneManager::default();
        let now = common::time::now_ms();
        mgr.interest
            .ghost_in(1001, ReplicationZone::Immediate, StreamBucket::Enemy, now);
        mgr.on_entity_killed(1001);
        assert!(mgr.dead_entities.contains_key(&1001));
        assert!(!mgr.interest.is_ghosted_in(1001));
    }

    #[test]
    fn on_entity_despawned_ghosts_out_without_dead_record() {
        let mut mgr = SceneManager::default();
        let now = common::time::now_ms();
        mgr.interest.ghost_in(
            2001,
            ReplicationZone::Immediate,
            StreamBucket::Interactive,
            now,
        );
        mgr.on_entity_despawned(2001);
        assert!(!mgr.dead_entities.contains_key(&2001));
        assert!(!mgr.interest.is_ghosted_in(2001));
    }

    #[test]
    fn dead_entities_initially_empty() {
        let mgr = SceneManager::default();
        assert!(mgr.dead_entities.is_empty());
    }

    #[test]
    fn resident_empty_pattern_is_ignored() {
        // The filter(|p| !p.is_empty()) should skip empty patterns
        assert!(!is_always_resident_interactive("some_random_thing", 0));
    }

    #[test]
    fn resident_save_group() {
        assert!(is_always_resident_interactive("int_save_group_01", 0));
    }

    #[test]
    fn resident_locked_door() {
        assert!(is_always_resident_interactive("int_locked_door_north", 0));
    }

    #[test]
    fn move_leader_single_element_noop() {
        let mut chars = vec![make_scene_char(5)];
        move_leader_to_front(&mut chars, 5);
        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].common_info.as_ref().unwrap().id, 5);
    }

    #[test]
    fn move_leader_preserves_other_order() {
        let mut chars = vec![
            make_scene_char(2),
            make_scene_char(3),
            make_scene_char(4),
            make_scene_char(1),
        ];
        move_leader_to_front(&mut chars, 1);
        assert_eq!(chars[0].common_info.as_ref().unwrap().id, 1);
        // The remaining elements should be 2, 3, 4 (shifted right by the rotate)
        assert_eq!(chars[1].common_info.as_ref().unwrap().id, 2);
        assert_eq!(chars[2].common_info.as_ref().unwrap().id, 3);
        assert_eq!(chars[3].common_info.as_ref().unwrap().id, 4);
    }
}
