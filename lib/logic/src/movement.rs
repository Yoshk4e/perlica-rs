use crate::player::WorldState;

#[derive(Debug, Clone, Copy, Default)]
pub struct Pos<T> {
    pub inner: [T; 3],
}

impl<T> Pos<T> {
    pub fn new(x: T, y: T, z: T) -> Self {
        Self { inner: [x, y, z] }
    }

    pub fn get_x(&self) -> &T {
        &self.inner[0]
    }

    pub fn get_y(&self) -> &T {
        &self.inner[1]
    }

    pub fn get_z(&self) -> &T {
        &self.inner[2]
    }
}

#[derive(Debug, Clone)]
pub struct MovementManager {
    pub pos: Pos<f32>,
    pub rot: Pos<f32>,
}

impl From<&WorldState> for MovementManager {
    fn from(world: &WorldState) -> Self {
        Self {
            pos: Pos::new(world.pos_x, world.pos_y, world.pos_z),
            rot: Pos::new(world.rot_x, world.rot_y, world.rot_z),
        }
    }
}

impl MovementManager {
    pub fn new(pos: Pos<f32>, rot: Pos<f32>) -> Self {
        Self { pos, rot }
    }

    pub fn update_position(&mut self, x: f32, y: f32, z: f32) {
        self.pos = Pos::new(x, y, z);
    }

    pub fn update_rotation(&mut self, x: f32, y: f32, z: f32) {
        self.rot = Pos::new(x, y, z);
    }
}

impl Default for MovementManager {
    fn default() -> Self {
        Self::from(&WorldState::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pos_new_sets_components() {
        let p = Pos::new(1.0, 2.0, 3.0);
        assert_eq!(*p.get_x(), 1.0);
        assert_eq!(*p.get_y(), 2.0);
        assert_eq!(*p.get_z(), 3.0);
    }

    #[test]
    fn pos_default_is_zeroed() {
        let p: Pos<f32> = Pos::default();
        assert_eq!(p.inner, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn movement_manager_new() {
        let pos = Pos::new(10.0, 20.0, 30.0);
        let rot = Pos::new(1.0, 2.0, 3.0);
        let mm = MovementManager::new(pos, rot);
        assert_eq!(*mm.pos.get_x(), 10.0);
        assert_eq!(*mm.rot.get_y(), 2.0);
    }

    #[test]
    fn update_position_changes_coords() {
        let pos = Pos::new(0.0, 0.0, 0.0);
        let rot = Pos::new(0.0, 0.0, 0.0);
        let mut mm = MovementManager::new(pos, rot);
        mm.update_position(100.0, 200.0, 300.0);
        assert_eq!(*mm.pos.get_x(), 100.0);
        assert_eq!(*mm.pos.get_y(), 200.0);
        assert_eq!(*mm.pos.get_z(), 300.0);
        // Rotation should be unaffected
        assert_eq!(*mm.rot.get_x(), 0.0);
    }

    #[test]
    fn update_rotation_changes_coords() {
        let pos = Pos::new(0.0, 0.0, 0.0);
        let rot = Pos::new(0.0, 0.0, 0.0);
        let mut mm = MovementManager::new(pos, rot);
        mm.update_rotation(45.0, 90.0, 0.0);
        assert_eq!(*mm.rot.get_x(), 45.0);
        assert_eq!(*mm.rot.get_y(), 90.0);
        assert_eq!(*mm.rot.get_z(), 0.0);
        // Position should be unaffected
        assert_eq!(*mm.pos.get_x(), 0.0);
    }

    #[test]
    fn movement_from_world_state() {
        let ws = WorldState {
            pos_x: 469.0,
            pos_y: 107.11,
            pos_z: 217.83,
            rot_x: 0.0,
            rot_y: 60.0,
            rot_z: 0.0,
            ..Default::default()
        };
        let mm = MovementManager::from(&ws);
        assert_eq!(*mm.pos.get_x(), 469.0);
        assert_eq!(*mm.pos.get_y(), 107.11);
        assert_eq!(*mm.pos.get_z(), 217.83);
        assert_eq!(*mm.rot.get_y(), 60.0);
    }

    #[test]
    fn movement_default_matches_world_default() {
        let mm = MovementManager::default();
        let ws = WorldState::default();
        assert_eq!(*mm.pos.get_x(), ws.pos_x);
        assert_eq!(*mm.pos.get_y(), ws.pos_y);
        assert_eq!(*mm.pos.get_z(), ws.pos_z);
        assert_eq!(*mm.rot.get_x(), ws.rot_x);
        assert_eq!(*mm.rot.get_y(), ws.rot_y);
        assert_eq!(*mm.rot.get_z(), ws.rot_z);
    }
}
