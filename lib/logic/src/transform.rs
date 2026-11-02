//! Finite 3D geometry values shared by movement, teleport and spawning.

/// A three-component vector that can never hold NaN or an infinity.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FiniteVec3 {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TransformError {
    #[error("component is not finite")]
    NotFinite,
}

impl FiniteVec3 {
    pub fn try_new(x: f32, y: f32, z: f32) -> Result<Self, TransformError> {
        if x.is_finite() && y.is_finite() && z.is_finite() {
            Ok(Self { x, y, z })
        } else {
            Err(TransformError::NotFinite)
        }
    }

    pub fn x(self) -> f32 {
        self.x
    }

    pub fn y(self) -> f32 {
        self.y
    }

    pub fn z(self) -> f32 {
        self.z
    }

    pub fn distance_squared(self, other: Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        dx * dx + dy * dy + dz * dz
    }
}

/// A finite position and rotation pair.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Transform {
    position: FiniteVec3,
    rotation: FiniteVec3,
}

impl Transform {
    pub fn new(position: FiniteVec3, rotation: FiniteVec3) -> Self {
        Self { position, rotation }
    }

    pub fn position(self) -> FiniteVec3 {
        self.position
    }

    pub fn rotation(self) -> FiniteVec3 {
        self.rotation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_finite_components() {
        assert_eq!(
            FiniteVec3::try_new(f32::NAN, 0.0, 0.0),
            Err(TransformError::NotFinite)
        );
        assert_eq!(
            FiniteVec3::try_new(0.0, f32::INFINITY, 0.0),
            Err(TransformError::NotFinite)
        );
        assert_eq!(
            FiniteVec3::try_new(0.0, 0.0, f32::NEG_INFINITY),
            Err(TransformError::NotFinite)
        );
    }

    #[test]
    fn accepts_finite_components() {
        let v = FiniteVec3::try_new(1.5, -2.0, 3.25).expect("finite");
        assert_eq!((v.x(), v.y(), v.z()), (1.5, -2.0, 3.25));
    }

    #[test]
    fn default_is_origin() {
        let v = FiniteVec3::default();
        assert_eq!((v.x(), v.y(), v.z()), (0.0, 0.0, 0.0));
    }

    #[test]
    fn distance_squared_matches_expectation() {
        let a = FiniteVec3::try_new(0.0, 0.0, 0.0).expect("finite");
        let b = FiniteVec3::try_new(3.0, 4.0, 0.0).expect("finite");
        assert_eq!(a.distance_squared(b), 25.0);
    }

    #[test]
    fn transform_carries_both_parts() {
        let position = FiniteVec3::try_new(1.0, 2.0, 3.0).expect("finite");
        let rotation = FiniteVec3::try_new(0.0, 90.0, 0.0).expect("finite");
        let t = Transform::new(position, rotation);
        assert_eq!(t.position(), position);
        assert_eq!(t.rotation(), rotation);
    }
}
