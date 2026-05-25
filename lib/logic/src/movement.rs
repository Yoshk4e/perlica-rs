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
