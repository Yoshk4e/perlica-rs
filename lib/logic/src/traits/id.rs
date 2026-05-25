//! Instance-id newtype traits.
//!
//! Every depot uses a `u64`-backed newtype as its key (`WeaponInstId`,
//! `GemInstId`, `EquipInstId`). These traits let generic depot code talk
//! about *any* id without naming the concrete type, packet conversion,
//! save-migration, logging, id allocation all flow through here.

use crate::item::{EquipInstId, GemInstId, WeaponInstId};

/// A thin wrapper around `u64` returning its raw value.
pub trait AsU64 {
    fn as_u64(&self) -> u64;
}

/// Marker + factory for newtype instance ids.
///
/// `new` is what makes generic id allocators possible - `IdAllocator` can
/// hand back a fresh, correctly-typed id without knowing which depot it
/// belongs to.
pub trait InstanceId: AsU64 + Copy + Eq + std::hash::Hash {
    fn new(raw: u64) -> Self;

    #[inline]
    fn is_zero(&self) -> bool {
        self.as_u64() == 0
    }
}

// These newtypes already have inherent `new` / `as_u64`, so the trait impls
// are pure forwarders.  Kept here (rather than near the type definition) so
// the item module stays free of the cross-cutting traits crate.
macro_rules! impl_instance_id {
    ($($t:ty),* $(,)?) => {
        $(
            impl AsU64 for $t {
                #[inline]
                fn as_u64(&self) -> u64 { <$t>::as_u64(*self) }
            }
            impl InstanceId for $t {
                #[inline]
                fn new(raw: u64) -> Self { <$t>::new(raw) }
            }
        )*
    };
}

impl_instance_id!(WeaponInstId, GemInstId, EquipInstId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_id_round_trip() {
        let id: WeaponInstId = <WeaponInstId as InstanceId>::new(42);
        assert_eq!(<WeaponInstId as AsU64>::as_u64(&id), 42);
        assert!(!<WeaponInstId as InstanceId>::is_zero(&id));
        let zero: WeaponInstId = <WeaponInstId as InstanceId>::new(0);
        assert!(<WeaponInstId as InstanceId>::is_zero(&zero));
    }
}
