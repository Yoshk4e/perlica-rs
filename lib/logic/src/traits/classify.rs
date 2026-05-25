//! Coarse entity classification.

use crate::entity::{EntityKind, SceneEntity};

/// Lets generic code talk about an entity's coarse classification.
///
/// `SceneEntity` is the obvious target; `EntityKind` also implements the
/// trait so filter helpers can be written once and called with either a
/// full entity or a bare kind value.
pub trait Classified {
    fn kind(&self) -> EntityKind;

    #[inline]
    fn is_enemy(&self) -> bool {
        self.kind() == EntityKind::Enemy
    }
    #[inline]
    fn is_character(&self) -> bool {
        self.kind() == EntityKind::Character
    }
    #[inline]
    fn is_interactive(&self) -> bool {
        self.kind() == EntityKind::Interactive
    }
    #[inline]
    fn is_npc(&self) -> bool {
        self.kind() == EntityKind::Npc
    }
}

impl Classified for SceneEntity {
    #[inline]
    fn kind(&self) -> EntityKind {
        self.kind
    }
}
impl Classified for EntityKind {
    #[inline]
    fn kind(&self) -> EntityKind {
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classified_filters() {
        let e = SceneEntity {
            id: 1,
            template_id: "tmpl".into(),
            kind: EntityKind::Enemy,
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
            level_logic_id: 0,
            belong_level_script_id: 0,
        };
        assert!(e.is_enemy());
        assert!(!e.is_character());
    }
}
