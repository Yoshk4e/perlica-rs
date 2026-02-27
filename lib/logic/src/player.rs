use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldState {
    pub role_level: i32,
    pub role_exp: i32,
    pub last_scene: String,
    pub pos_x: f32,
    pub pos_y: f32,
    pub pos_z: f32,
    pub rot_x: f32,
    pub rot_y: f32,
    pub rot_z: f32,
}

impl Default for WorldState {
    fn default() -> Self {
        Self {
            role_level: 1,
            role_exp: 0,
            last_scene: "map01_lv001".to_string(),
            pos_x: 469.0,
            pos_y: 107.11,
            pos_z: 217.83,
            rot_x: 0.0,
            rot_y: 60.00,
            rot_z: 0.0,
        }
    }
}
