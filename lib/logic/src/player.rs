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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let ws = WorldState::default();
        assert_eq!(ws.role_level, 1);
        assert_eq!(ws.role_exp, 0);
        assert_eq!(ws.last_scene, "map01_lv001");
        assert_eq!(ws.pos_x, 469.0);
        assert_eq!(ws.pos_y, 107.11);
        assert_eq!(ws.pos_z, 217.83);
        assert_eq!(ws.rot_x, 0.0);
        assert_eq!(ws.rot_y, 60.00);
        assert_eq!(ws.rot_z, 0.0);
    }

    #[test]
    fn custom_world_state() {
        let ws = WorldState {
            role_level: 10,
            role_exp: 500,
            last_scene: "map01_dg003".to_string(),
            pos_x: 100.0,
            pos_y: 200.0,
            pos_z: 300.0,
            rot_x: 1.0,
            rot_y: 2.0,
            rot_z: 3.0,
        };
        assert_eq!(ws.role_level, 10);
        assert_eq!(ws.last_scene, "map01_dg003");
    }

    #[test]
    fn serialization_roundtrip() {
        let ws = WorldState {
            role_level: 5,
            role_exp: 1234,
            last_scene: "test_scene".to_string(),
            pos_x: 1.0,
            pos_y: 2.0,
            pos_z: 3.0,
            rot_x: 4.0,
            rot_y: 5.0,
            rot_z: 6.0,
        };
        let json = serde_json::to_string(&ws).unwrap();
        let decoded: WorldState = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.role_level, ws.role_level);
        assert_eq!(decoded.role_exp, ws.role_exp);
        assert_eq!(decoded.last_scene, ws.last_scene);
        assert_eq!(decoded.pos_x, ws.pos_x);
    }
}
