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
