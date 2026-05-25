//! Position / rotation impls for every world-space type.

use super::{Positioned3D, Positioned3DMut, Rotated3D, Rotated3DMut};
use crate::entity::SceneEntity;
use crate::movement::{MovementManager, Pos};
use crate::player::WorldState;
use crate::scene::CheckpointInfo;

// Read-only, scene entities are spatial snapshots, mutated through the
// EntityManager rather than in-place.
impl Positioned3D for SceneEntity {
    #[inline]
    fn pos_x(&self) -> f32 {
        self.pos_x
    }
    #[inline]
    fn pos_y(&self) -> f32 {
        self.pos_y
    }
    #[inline]
    fn pos_z(&self) -> f32 {
        self.pos_z
    }
}

impl Positioned3D for MovementManager {
    #[inline]
    fn pos_x(&self) -> f32 {
        *self.pos.get_x()
    }
    #[inline]
    fn pos_y(&self) -> f32 {
        *self.pos.get_y()
    }
    #[inline]
    fn pos_z(&self) -> f32 {
        *self.pos.get_z()
    }
}
impl Positioned3DMut for MovementManager {
    #[inline]
    fn set_position(&mut self, x: f32, y: f32, z: f32) {
        self.update_position(x, y, z);
    }
}
impl Rotated3D for MovementManager {
    #[inline]
    fn rot_x(&self) -> f32 {
        *self.rot.get_x()
    }
    #[inline]
    fn rot_y(&self) -> f32 {
        *self.rot.get_y()
    }
    #[inline]
    fn rot_z(&self) -> f32 {
        *self.rot.get_z()
    }
}
impl Rotated3DMut for MovementManager {
    #[inline]
    fn set_rotation(&mut self, x: f32, y: f32, z: f32) {
        self.update_rotation(x, y, z);
    }
}

impl Positioned3D for WorldState {
    #[inline]
    fn pos_x(&self) -> f32 {
        self.pos_x
    }
    #[inline]
    fn pos_y(&self) -> f32 {
        self.pos_y
    }
    #[inline]
    fn pos_z(&self) -> f32 {
        self.pos_z
    }
}
impl Positioned3DMut for WorldState {
    #[inline]
    fn set_position(&mut self, x: f32, y: f32, z: f32) {
        self.pos_x = x;
        self.pos_y = y;
        self.pos_z = z;
    }
}
impl Rotated3D for WorldState {
    #[inline]
    fn rot_x(&self) -> f32 {
        self.rot_x
    }
    #[inline]
    fn rot_y(&self) -> f32 {
        self.rot_y
    }
    #[inline]
    fn rot_z(&self) -> f32 {
        self.rot_z
    }
}
impl Rotated3DMut for WorldState {
    #[inline]
    fn set_rotation(&mut self, x: f32, y: f32, z: f32) {
        self.rot_x = x;
        self.rot_y = y;
        self.rot_z = z;
    }
}

impl Positioned3D for CheckpointInfo {
    #[inline]
    fn pos_x(&self) -> f32 {
        self.pos_x
    }
    #[inline]
    fn pos_y(&self) -> f32 {
        self.pos_y
    }
    #[inline]
    fn pos_z(&self) -> f32 {
        self.pos_z
    }
}
impl Positioned3DMut for CheckpointInfo {
    #[inline]
    fn set_position(&mut self, x: f32, y: f32, z: f32) {
        self.pos_x = x;
        self.pos_y = y;
        self.pos_z = z;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::world::PositionedExt;

    #[test]
    fn movement_positioned_ext() {
        let a = MovementManager::new(Pos::new(0.0, 0.0, 0.0), Pos::new(0.0, 0.0, 0.0));
        let b = MovementManager::new(Pos::new(3.0, 0.0, 4.0), Pos::new(0.0, 0.0, 0.0));
        assert!((a.distance_to(&b) - 5.0).abs() < 1e-5);
        assert!(a.within_radius(&b, 6.0));
        assert!(!a.within_radius(&b, 4.0));
        assert_eq!(a.xz(), (0.0, 0.0));
    }
}
