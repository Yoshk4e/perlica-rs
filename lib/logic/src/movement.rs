use crate::player::WorldState;

#[derive(Debug, Clone)]
pub struct MovementManager {
    pub pos_x: f32,
    pub pos_y: f32,
    pub pos_z: f32,
    pub rot_x: f32,
    pub rot_y: f32,
    pub rot_z: f32,
}

impl From<&WorldState> for MovementManager {
    fn from(world: &WorldState) -> Self {
        Self {
            pos_x: world.pos_x,
            pos_y: world.pos_y,
            pos_z: world.pos_z,
            rot_x: world.rot_x,
            rot_y: world.rot_y,
            rot_z: world.rot_z,
        }
    }
}

impl MovementManager {
    pub fn new(pos_x: f32, pos_y: f32, pos_z: f32, rot_x: f32, rot_y: f32, rot_z: f32) -> Self {
        Self {
            pos_x,
            pos_y,
            pos_z,
            rot_x,
            rot_y,
            rot_z,
        }
    }

    // Write current position back into WorldState before saving to db
    pub fn sync_to_world(&self, world: &mut WorldState) {
        world.pos_x = self.pos_x;
        world.pos_y = self.pos_y;
        world.pos_z = self.pos_z;
        world.rot_x = self.rot_x;
        world.rot_y = self.rot_y;
        world.rot_z = self.rot_z;
    }

    pub fn update_position(&mut self, x: f32, y: f32, z: f32) {
        self.pos_x = x;
        self.pos_y = y;
        self.pos_z = z;
    }

    pub fn update_rotation(&mut self, x: f32, y: f32, z: f32) {
        self.rot_x = x;
        self.rot_y = y;
        self.rot_z = z;
    }

    // Teleport: same as update_position but signals intentional jumps,
    // e.g. respawn at checkpoint, scene warp, etc.
    pub fn teleport(&mut self, x: f32, y: f32, z: f32) {
        self.pos_x = x;
        self.pos_y = y;
        self.pos_z = z;
    }

    pub fn position_tuple(&self) -> (f32, f32, f32) {
        (self.pos_x, self.pos_y, self.pos_z)
    }

    pub fn rotation_tuple(&self) -> (f32, f32, f32) {
        (self.rot_x, self.rot_y, self.rot_z)
    }
}

impl Default for MovementManager {
    fn default() -> Self {
        Self::from(&WorldState::default())
    }
}
