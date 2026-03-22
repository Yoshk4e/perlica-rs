use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LvDataFile {
    #[serde(rename = "sceneId")]
    pub scene_id: String,
    #[serde(default)]
    pub enemies: Vec<LvEnemy>,
    #[serde(rename = "enemyGroup", default)]
    pub enemy_groups: Vec<LvEnemyGroup>,
    #[serde(default)]
    pub patrols: Vec<LvPatrol>,
    #[serde(default)]
    pub interactives: Vec<LvInteractive>,
    #[serde(default)]
    pub npcs: Vec<LvNpc>,
    #[serde(rename = "levelScripts", default)]
    pub level_scripts: Vec<LvLevelScript>,
    #[serde(rename = "factoryRegions", default)]
    pub factory_regions: Vec<LvFactoryRegion>,
    #[serde(default)]
    pub splines: Vec<LvSpline>,
    #[serde(rename = "safeZone", default)]
    pub safe_zone: LvSafeZone,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Vector3f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Scale3f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Common fields every world entity shares.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LvEntityBase {
    #[serde(rename = "levelLogicId")]
    pub level_logic_id: u64,
    #[serde(rename = "entityType")]
    pub entity_type: i32,
    #[serde(rename = "entityDataIdKey")]
    pub template_id: String,
    #[serde(rename = "defaultHide", default)]
    pub default_hide: bool,
    pub position: Vector3f,
    pub rotation: Vector3f,
    #[serde(default)]
    pub scale: Scale3f,
    #[serde(rename = "belongLevelScriptId", default)]
    pub belong_level_script_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LvEnemy {
    #[serde(flatten)]
    pub base: LvEntityBase,
    pub level: u32,
    #[serde(rename = "enemyGroupId", default)]
    pub enemy_group_id: u64,
    #[serde(rename = "patrolData", default)]
    pub patrol_data: Option<LvPatrol>,
    #[serde(default)]
    pub respawnable: bool,
    #[serde(rename = "overrideAIConfig", default)]
    pub override_ai_config: String,
    #[serde(rename = "aiBlackboard", default)]
    pub ai_blackboard: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LvEnemyGroup {
    #[serde(rename = "groupId")]
    pub group_id: u64,
    #[serde(rename = "patrolId")]
    pub patrol_id: u64,
    #[serde(rename = "centerPos")]
    pub center_pos: Vector3f,
    #[serde(rename = "centerDir")]
    pub center_dir: Vector3f,
    #[serde(rename = "moveSpeed", default)]
    pub move_speed: f32,
    #[serde(rename = "returnSpeed", default)]
    pub return_speed: f32,
    #[serde(rename = "rotationSpeed", default)]
    pub rotation_speed: f32,
    #[serde(rename = "enemyLogicId", default)]
    pub enemy_logic_ids: Vec<u64>,
    #[serde(default)]
    pub slot: Vec<LvGroupSlot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LvGroupSlot {
    #[serde(rename = "logicId")]
    pub logic_id: u64,
    pub offset: LvOffset2D,
}

/// 2-D offset in the group's local XZ plane (field `y` = world Z axis).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LvOffset2D {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LvPatrol {
    #[serde(default)]
    pub id: u64,
    #[serde(rename = "worldOffset", default)]
    pub world_offset: Vector3f,
    #[serde(rename = "loop", default)]
    pub loop_mode: i32,
    #[serde(rename = "snap", default)]
    pub snap: i32,
    #[serde(rename = "inLocalSpace", default)]
    pub in_local_space: bool,
    #[serde(rename = "addBornPositionAsCheckpoint", default)]
    pub add_born_position_as_checkpoint: bool,
    #[serde(rename = "bornPositionWaitDuration", default)]
    pub born_position_wait_duration: f32,
    #[serde(default)]
    pub actions: Vec<LvPatrolAction>,
    #[serde(rename = "lead", default)]
    pub lead: bool,
    #[serde(rename = "waitDistance", default)]
    pub wait_distance: f32,
    #[serde(rename = "runningRadius", default)]
    pub running_radius: f32,
    #[serde(rename = "coolDownBetweenWalkAndStop", default)]
    pub cooldown: f32,
    #[serde(rename = "moveStyleWithoutLead", default)]
    pub move_style_without_lead: i32,
    #[serde(rename = "isUseCatmull", default)]
    pub is_use_catmull: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LvPatrolAction {
    #[serde(rename = "actionType", default)]
    pub action_type: i32,
    pub position: Vector3f,
    #[serde(rename = "subPositions", default)]
    pub sub_positions: Vec<Vector3f>,
    #[serde(rename = "subActions", default)]
    pub sub_actions: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LvInteractive {
    #[serde(flatten)]
    pub base: LvEntityBase,
    #[serde(default)]
    pub properties: Vec<LvProperty>,
    #[serde(rename = "componentProperties", default)]
    pub component_properties: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LvProperty {
    pub key: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LvNpc {
    #[serde(flatten)]
    pub base: LvEntityBase,
    #[serde(default)]
    pub properties: Vec<LvProperty>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LvLevelScript {
    #[serde(rename = "scriptId")]
    pub script_id: u64,
    #[serde(rename = "allowTick", default)]
    pub allow_tick: bool,
    #[serde(rename = "isEmbedded", default)]
    pub is_embedded: bool,
    /// Path to external action map, if not embedded.
    #[serde(rename = "refActionMapPath", default)]
    pub ref_action_map_path: Option<String>,
    /// Serialised JSON string of the embedded action map.
    #[serde(rename = "embeddedActionMap", default)]
    pub embedded_action_map: Option<String>,
    #[serde(rename = "resetModeWhenActive", default)]
    pub reset_mode_when_active: i32,
    #[serde(rename = "resetModeWhenEnd", default)]
    pub reset_mode_when_end: i32,
    #[serde(rename = "activeShapeList", default)]
    pub active_shapes: Vec<LvShape>,
    #[serde(rename = "startShapeList", default)]
    pub start_shapes: Vec<LvShape>,
    #[serde(default)]
    pub properties: Vec<LvProperty>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LvShape {
    /// 1 = box, 2 = sphere
    #[serde(rename = "type")]
    pub shape_type: i32,
    pub offset: Vector3f,
    #[serde(rename = "eulerAngles")]
    pub euler_angles: Vector3f,
    pub size: Vector3f,
    pub radius: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LvFactoryRegion {
    #[serde(flatten)]
    pub base: LvEntityBase,
    #[serde(default)]
    pub properties: Vec<LvProperty>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LvSpline {
    #[serde(rename = "splineId")]
    pub spline_id: u64,
    pub position: Vector3f,
    pub rotation: Vector3f,
    #[serde(default)]
    pub closed: bool,
    #[serde(default)]
    pub knots: Vec<LvSplineKnot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LvSplineKnot {
    #[serde(rename = "Position")]
    pub position: Vector3f,
    #[serde(rename = "TangentIn")]
    pub tangent_in: Vector3f,
    #[serde(rename = "TangentOut")]
    pub tangent_out: Vector3f,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LvSafeZone {
    #[serde(default)]
    pub boxes: Vec<serde_json::Value>,
}
