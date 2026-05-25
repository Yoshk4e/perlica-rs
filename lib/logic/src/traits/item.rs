//! Item-instance facet traits.
//!
//! `WeaponInstance`, `GemInstance`, and `EquipInstance` all expose the same
//! behavioural surface, id, template id, lock flag, "new" flag, own-time,
//! and "what am I attached to". These traits carve up that surface so
//! generic code can talk about *any* item instance, and so future item
//! kinds (relics, accessories, ...) implement a fixed shopping list rather
//! than copy-pasting accessors.

use super::id::InstanceId;
use crate::item::{
    EquipInstId, EquipInstance, GemInstId, GemInstance, WeaponInstId, WeaponInstance,
};

/// An object that exposes a typed instance id.
pub trait Identifiable {
    type Id: InstanceId;
    fn id(&self) -> Self::Id;
}

/// An object that is born from a static template (a string key into the
/// `BeyondAssets` tables).
pub trait Templated {
    fn template_id(&self) -> &str;
}

/// "Did the player flag this thing so it can't be deleted / used as fodder?"
pub trait Lockable {
    fn is_locked(&self) -> bool;
    fn set_locked(&mut self, locked: bool);

    #[inline]
    fn lock(&mut self) {
        self.set_locked(true);
    }
    #[inline]
    fn unlock(&mut self) {
        self.set_locked(false);
    }
}

/// "Has the player ever looked at this thing in the UI?"
///
/// Cleared by UI hover handlers.
pub trait NewFlaggable {
    fn is_new(&self) -> bool;
    fn set_new(&mut self, is_new: bool);

    #[inline]
    fn mark_seen(&mut self) {
        self.set_new(false);
    }
}

/// "When (ms since unix epoch) did the player obtain this?"
pub trait Owned {
    fn own_time(&self) -> i64;
}

/// Generalization of `is_equipped` / `is_socketed`: the item is attached to
/// another entity (a character for weapons/equips, a weapon for gems).
///
/// `0` is the canonical "detached" sentinel used throughout the codebase.
pub trait Attachable {
    /// Id of the entity this is attached to, or `0` if detached.
    fn attached_to(&self) -> u64;

    #[inline]
    fn is_attached(&self) -> bool {
        self.attached_to() != 0
    }
    #[inline]
    fn is_detached(&self) -> bool {
        !self.is_attached()
    }
}

// The impls are repetitive but trivial, a macro keeps the noise down and
// makes it obvious that every item kind exposes the exact same surface.
macro_rules! impl_item_traits {
    (
        $Inst:ty, $Id:ty,
        attach: $attach_field:ident,
    ) => {
        impl Identifiable for $Inst {
            type Id = $Id;
            #[inline]
            fn id(&self) -> Self::Id {
                self.inst_id
            }
        }
        impl Templated for $Inst {
            #[inline]
            fn template_id(&self) -> &str {
                &self.template_id
            }
        }
        impl Lockable for $Inst {
            #[inline]
            fn is_locked(&self) -> bool {
                self.is_lock
            }
            #[inline]
            fn set_locked(&mut self, locked: bool) {
                self.is_lock = locked;
            }
        }
        impl NewFlaggable for $Inst {
            #[inline]
            fn is_new(&self) -> bool {
                self.is_new
            }
            #[inline]
            fn set_new(&mut self, v: bool) {
                self.is_new = v;
            }
        }
        impl Owned for $Inst {
            #[inline]
            fn own_time(&self) -> i64 {
                self.own_time
            }
        }
        impl Attachable for $Inst {
            #[inline]
            fn attached_to(&self) -> u64 {
                self.$attach_field
            }
        }
    };
}

impl_item_traits!(WeaponInstance, WeaponInstId, attach: equip_char_id,);
impl_item_traits!(GemInstance,    GemInstId,    attach: attach_weapon_id,);
impl_item_traits!(EquipInstance,  EquipInstId,  attach: equip_char_id,);

// `StoredMail` and `SceneEntity` aren't "items" but they share two facets
// every item carries:
//
//   * a stable, untyped `u64` id  -> `Identifiable<Id = u64>`
//   * a string template id        -> `Templated`
//
// Implementing the traits lets the same generic helpers
// (`count_by_template`, `KeyedContainerExt::get_or_not_found`, future
// rendering helpers) work over them without further specialisation.
use crate::entity::SceneEntity;
use crate::mail::StoredMail;

impl Identifiable for StoredMail {
    type Id = u64;
    #[inline]
    fn id(&self) -> u64 {
        self.mail_id
    }
}
impl Templated for StoredMail {
    #[inline]
    fn template_id(&self) -> &str {
        &self.template_id
    }
}

impl Identifiable for SceneEntity {
    type Id = u64;
    #[inline]
    fn id(&self) -> u64 {
        self.id
    }
}
impl Templated for SceneEntity {
    #[inline]
    fn template_id(&self) -> &str {
        &self.template_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::{WeaponInstId, WeaponInstance};

    use crate::entity::{EntityKind, SceneEntity};
    use crate::mail::StoredMail;

    #[test]
    fn stored_mail_facets() {
        let mut m = StoredMail::make_welcome_mail();
        m.mail_id = 7;
        m.template_id = "mail_welcome".into();
        assert_eq!(<StoredMail as Identifiable>::id(&m), 7);
        assert_eq!(<StoredMail as Templated>::template_id(&m), "mail_welcome");
    }

    #[test]
    fn scene_entity_facets() {
        let e = SceneEntity {
            id: 42,
            template_id: "npc_innkeeper".into(),
            kind: EntityKind::Npc,
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
            level_logic_id: 0,
            belong_level_script_id: 0,
        };
        assert_eq!(<SceneEntity as Identifiable>::id(&e), 42);
        assert_eq!(<SceneEntity as Templated>::template_id(&e), "npc_innkeeper");
    }

    #[test]
    fn weapon_instance_traits() {
        let mut w = WeaponInstance::new(WeaponInstId::new(1), "wpn_0002".into(), 0);
        assert_eq!(<WeaponInstance as Templated>::template_id(&w), "wpn_0002");
        assert!(!w.is_locked());
        assert!(<WeaponInstance as NewFlaggable>::is_new(&w));
        assert!(<WeaponInstance as Attachable>::is_detached(&w));
        w.lock();
        assert!(w.is_locked());
        w.mark_seen();
        assert!(!<WeaponInstance as NewFlaggable>::is_new(&w));
    }
}
