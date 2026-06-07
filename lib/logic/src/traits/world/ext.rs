//! Blanket geometric helpers over any [`Positioned3D`].

use super::Positioned3D;

/// Generic geometric helpers built on [`Positioned3D`].
///
/// Implemented for every `T: Positioned3D` via a blanket impl - adding a
/// new positioned type gets every helper here for free.
pub trait PositionedExt: Positioned3D {
    /// Squared 3-D distance. Avoids the `sqrt` for hot-path radius checks.
    #[inline]
    fn distance_sq_to<T: Positioned3D + ?Sized>(&self, other: &T) -> f32 {
        let dx = self.pos_x() - other.pos_x();
        let dy = self.pos_y() - other.pos_y();
        let dz = self.pos_z() - other.pos_z();
        dx * dx + dy * dy + dz * dz
    }

    #[inline]
    fn distance_to<T: Positioned3D + ?Sized>(&self, other: &T) -> f32 {
        self.distance_sq_to(other).sqrt()
    }

    /// Horizontal (XZ-plane) distance squared - matches the convention the
    /// interest manager uses everywhere.
    #[inline]
    fn horiz_distance_sq_to<T: Positioned3D + ?Sized>(&self, other: &T) -> f32 {
        let dx = self.pos_x() - other.pos_x();
        let dz = self.pos_z() - other.pos_z();
        dx * dx + dz * dz
    }

    #[inline]
    fn within_radius<T: Positioned3D + ?Sized>(&self, other: &T, radius: f32) -> bool {
        self.distance_sq_to(other) <= radius * radius
    }

    /// `(x, z)` tuple for direct use with `SpatialGrid::build`.
    #[inline]
    fn xz(&self) -> (f32, f32) {
        (self.pos_x(), self.pos_z())
    }
}
impl<T: Positioned3D + ?Sized> PositionedExt for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::SceneEntity;
    use crate::movement::{MovementManager, Pos};
    use crate::scene::CheckpointInfo;

    #[test]
    fn distance_sq_same_point_is_zero() {
        let a = MovementManager::new(Pos::new(10.0, 20.0, 30.0), Pos::new(0.0, 0.0, 0.0));
        assert_eq!(a.distance_sq_to(&a), 0.0);
    }

    #[test]
    fn distance_sq_axis_aligned() {
        let a = MovementManager::new(Pos::new(0.0, 0.0, 0.0), Pos::new(0.0, 0.0, 0.0));
        let b = MovementManager::new(Pos::new(3.0, 0.0, 0.0), Pos::new(0.0, 0.0, 0.0));
        assert!((a.distance_sq_to(&b) - 9.0).abs() < 1e-5);
    }

    #[test]
    fn distance_sq_3d() {
        let a = MovementManager::new(Pos::new(1.0, 2.0, 3.0), Pos::new(0.0, 0.0, 0.0));
        let b = MovementManager::new(Pos::new(4.0, 6.0, 3.0), Pos::new(0.0, 0.0, 0.0));
        // dx=3, dy=4, dz=0 => 9+16+0 = 25
        assert!((a.distance_sq_to(&b) - 25.0).abs() < 1e-5);
    }

    #[test]
    fn distance_sq_symmetry() {
        let a = MovementManager::new(Pos::new(1.0, 2.0, 3.0), Pos::new(0.0, 0.0, 0.0));
        let b = MovementManager::new(Pos::new(10.0, 20.0, 30.0), Pos::new(0.0, 0.0, 0.0));
        assert!((a.distance_sq_to(&b) - b.distance_sq_to(&a)).abs() < 1e-5);
    }

    #[test]
    fn distance_to_pythagorean_345() {
        let a = MovementManager::new(Pos::new(0.0, 0.0, 0.0), Pos::new(0.0, 0.0, 0.0));
        let b = MovementManager::new(Pos::new(3.0, 0.0, 4.0), Pos::new(0.0, 0.0, 0.0));
        assert!((a.distance_to(&b) - 5.0).abs() < 1e-5);
    }

    #[test]
    fn distance_to_same_point_is_zero() {
        let a = MovementManager::new(Pos::new(42.0, -7.0, 100.0), Pos::new(0.0, 0.0, 0.0));
        assert!((a.distance_to(&a)).abs() < 1e-5);
    }

    #[test]
    fn horiz_distance_sq_ignores_y() {
        let a = MovementManager::new(Pos::new(0.0, 0.0, 0.0), Pos::new(0.0, 0.0, 0.0));
        let b = MovementManager::new(Pos::new(0.0, 999.0, 0.0), Pos::new(0.0, 0.0, 0.0));
        assert!((a.horiz_distance_sq_to(&b)).abs() < 1e-5);
    }

    #[test]
    fn horiz_distance_sq_xz_only() {
        let a = MovementManager::new(Pos::new(0.0, 0.0, 0.0), Pos::new(0.0, 0.0, 0.0));
        let b = MovementManager::new(Pos::new(3.0, 100.0, 4.0), Pos::new(0.0, 0.0, 0.0));
        // dx=3, dz=4 => 9+16 = 25
        assert!((a.horiz_distance_sq_to(&b) - 25.0).abs() < 1e-5);
    }

    #[test]
    fn within_radius_inside() {
        let a = MovementManager::new(Pos::new(0.0, 0.0, 0.0), Pos::new(0.0, 0.0, 0.0));
        let b = MovementManager::new(Pos::new(1.0, 0.0, 0.0), Pos::new(0.0, 0.0, 0.0));
        assert!(a.within_radius(&b, 2.0));
    }

    #[test]
    fn within_radius_exact_boundary() {
        let a = MovementManager::new(Pos::new(0.0, 0.0, 0.0), Pos::new(0.0, 0.0, 0.0));
        let b = MovementManager::new(Pos::new(3.0, 0.0, 4.0), Pos::new(0.0, 0.0, 0.0));
        // distance = 5.0, radius = 5.0 => exactly on boundary, should be true
        assert!(a.within_radius(&b, 5.0));
    }

    #[test]
    fn within_radius_outside() {
        let a = MovementManager::new(Pos::new(0.0, 0.0, 0.0), Pos::new(0.0, 0.0, 0.0));
        let b = MovementManager::new(Pos::new(3.0, 0.0, 4.0), Pos::new(0.0, 0.0, 0.0));
        assert!(!a.within_radius(&b, 4.9));
    }

    #[test]
    fn xz_returns_x_and_z() {
        let m = MovementManager::new(Pos::new(10.0, 20.0, 30.0), Pos::new(0.0, 0.0, 0.0));
        assert_eq!(m.xz(), (10.0, 30.0));
    }

    #[test]
    fn xz_negative_coordinates() {
        let m = MovementManager::new(Pos::new(-5.5, 99.0, -3.3), Pos::new(0.0, 0.0, 0.0));
        assert!((m.xz().0 - (-5.5)).abs() < 1e-5);
        assert!((m.xz().1 - (-3.3)).abs() < 1e-5);
    }

    #[test]
    fn cross_type_distance_scene_entity_and_movement() {
        let m = MovementManager::new(Pos::new(0.0, 0.0, 0.0), Pos::new(0.0, 0.0, 0.0));
        let e = SceneEntity {
            id: 1,
            template_id: "test".to_string(),
            kind: crate::entity::EntityKind::Enemy,
            pos_x: 3.0,
            pos_y: 4.0,
            pos_z: 0.0,
            level_logic_id: 0,
            belong_level_script_id: 0,
        };
        assert!((m.distance_to(&e) - 5.0).abs() < 1e-5);
    }

    #[test]
    fn checkpoint_positioned_ext() {
        let cp = CheckpointInfo {
            scene_name: "test".to_string(),
            pos_x: 1.0,
            pos_y: 2.0,
            pos_z: 3.0,
        };
        assert_eq!(cp.xz(), (1.0, 3.0));
        assert!((cp.pos_x() - 1.0).abs() < 1e-5);
    }
}
