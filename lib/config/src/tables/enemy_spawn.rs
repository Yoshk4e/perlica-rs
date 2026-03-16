use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnemySpawnsTable {
    #[serde(rename = "enemySpawnsTable")]
    pub scene: HashMap<String, Vec<SceneEnemyInfo>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneEnemyInfo {
    #[serde(rename = "entityType")]
    pub entity_type: i32,
    #[serde(rename = "entityDataIdKey")]
    pub template_id: String,
    pub position: Vector3f,
    pub rotation: Vector3f,
    #[serde(rename = "level")]
    pub level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vector3f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
