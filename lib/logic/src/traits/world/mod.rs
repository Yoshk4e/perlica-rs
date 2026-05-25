//! World-space geometry traits.
//!
//! `SceneEntity`, `MovementManager`, `WorldState`, and `CheckpointInfo` all
//! carry the same `pos_x / pos_y / pos_z` (+ optional rotation) triple but
//! never had a way to share code. These traits expose the triple and let
//! the blanket [`PositionedExt`] provide a uniform distance / radius
//! vocabulary.

mod ext;
mod impls;

pub use ext::PositionedExt;

/// 3-D world position holder.
pub trait Positioned3D {
    fn pos_x(&self) -> f32;
    fn pos_y(&self) -> f32;
    fn pos_z(&self) -> f32;

    #[inline]
    fn position(&self) -> (f32, f32, f32) {
        (self.pos_x(), self.pos_y(), self.pos_z())
    }
}

/// Mutable counterpart of [`Positioned3D`].
pub trait Positioned3DMut: Positioned3D {
    fn set_position(&mut self, x: f32, y: f32, z: f32);
}

/// 3-D Euler rotation holder.
pub trait Rotated3D {
    fn rot_x(&self) -> f32;
    fn rot_y(&self) -> f32;
    fn rot_z(&self) -> f32;

    #[inline]
    fn rotation(&self) -> (f32, f32, f32) {
        (self.rot_x(), self.rot_y(), self.rot_z())
    }
}

pub trait Rotated3DMut: Rotated3D {
    fn set_rotation(&mut self, x: f32, y: f32, z: f32);
}
