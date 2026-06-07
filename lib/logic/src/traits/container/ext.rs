//! Blanket extension traits for any [`Container`] / [`KeyedContainer`].
//!
//! These are deliberately *separate* from the core traits so adding a new
//! helper never touches an impl block, every type that satisfies the
//! bound picks up the new method for free.

use super::{Container, KeyedContainer};
use crate::error::{LogicError, Result};
use crate::traits::item::Lockable;

/// Generic helpers over any [`KeyedContainer`].
pub trait KeyedContainerExt: KeyedContainer {
    /// Lookup that converts the "missing key" case into a typed
    /// [`LogicError::NotFound`] with the supplied label.
    fn get_or_not_found(&self, key: Self::Key, label: &'static str) -> Result<&Self::Item> {
        self.get_ref(key)
            .ok_or_else(|| LogicError::NotFound(label.into()))
    }

    /// Mutable counterpart of [`Self::get_or_not_found`].
    fn get_mut_or_not_found(
        &mut self,
        key: Self::Key,
        label: &'static str,
    ) -> Result<&mut Self::Item> {
        self.get_mut_ref(key)
            .ok_or_else(|| LogicError::NotFound(label.into()))
    }
}
impl<T: KeyedContainer + ?Sized> KeyedContainerExt for T {}

/// Generic helpers that work on any [`Container`].
///
/// Reserved for future expansion, kept here so the prelude already pulls
/// it in and new methods land without churning every call-site's `use`.
pub trait ContainerExt: Container {}
impl<T: Container + ?Sized> ContainerExt for T {}

/// Helpers that operate on the items inside a [`Container`] of [`Lockable`]
/// things - generic "count how many are locked", "filter unlocked", ...
///
/// This trait can't be blanket-implemented over `Container<Item: Lockable>`
/// without GATs in some impls (depot's stored items are referenced through
/// inherent iterators of varying lifetimes), so the impls are explicit.
pub trait LockableContainerExt {
    /// Number of items currently marked `is_lock = true`.
    fn locked_count(&self) -> usize;
}

// Kept here so the trait + its impls live together; the depot types in
// crate::item already expose the iterators we need.
use crate::item::{EquipDepot, GemDepot, WeaponDepot};

impl LockableContainerExt for WeaponDepot {
    fn locked_count(&self) -> usize {
        self.all_weapons()
            .values()
            .filter(|w| w.is_locked())
            .count()
    }
}
impl LockableContainerExt for GemDepot {
    fn locked_count(&self) -> usize {
        self.iter().filter(|g| g.is_locked()).count()
    }
}
impl LockableContainerExt for EquipDepot {
    fn locked_count(&self) -> usize {
        self.iter().filter(|e| e.is_locked()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::{EquipDepot, EquipInstId, GemDepot, GemInstId, WeaponDepot, WeaponInstId};
    use crate::traits::KeyedContainerExt;

    #[test]
    fn weapon_depot_get_or_not_found_missing() {
        let depot = WeaponDepot::new();
        let result: Result<&_> = depot.get_or_not_found(WeaponInstId::new(999), "weapon");
        assert!(result.is_err());
        if let Err(LogicError::NotFound(label)) = result {
            assert_eq!(label, "weapon");
        } else {
            panic!("Expected NotFound error");
        }
    }

    #[test]
    fn weapon_depot_get_or_not_found_present() {
        let mut depot = WeaponDepot::new();
        let id = depot.add_weapon("wpn_test".to_string(), 0);
        let result = depot.get_or_not_found(id, "weapon");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().template_id, "wpn_test");
    }

    #[test]
    fn gem_depot_get_or_not_found_missing() {
        let depot = GemDepot::new();
        let result: Result<&_> = depot.get_or_not_found(GemInstId::new(999), "gem");
        assert!(result.is_err());
    }

    #[test]
    fn equip_depot_get_or_not_found_missing() {
        let depot = EquipDepot::new();
        let result: Result<&_> = depot.get_or_not_found(EquipInstId::new(999), "equip");
        assert!(result.is_err());
    }

    #[test]
    fn weapon_depot_get_mut_or_not_found_missing() {
        let mut depot = WeaponDepot::new();
        let result: Result<&mut _> = depot.get_mut_or_not_found(WeaponInstId::new(999), "weapon");
        assert!(result.is_err());
    }

    #[test]
    fn weapon_depot_get_mut_or_not_found_present() {
        let mut depot = WeaponDepot::new();
        let id = depot.add_weapon("wpn_test".to_string(), 0);
        let result = depot.get_mut_or_not_found(id, "weapon");
        assert!(result.is_ok());
    }

    #[test]
    fn weapon_depot_locked_count_empty() {
        let depot = WeaponDepot::new();
        assert_eq!(depot.locked_count(), 0);
    }

    #[test]
    fn weapon_depot_locked_count_with_weapons() {
        let mut depot = WeaponDepot::new();
        let id1 = depot.add_weapon("wpn_a".to_string(), 0);
        let id2 = depot.add_weapon("wpn_b".to_string(), 0);
        // New weapons are not locked by default
        assert_eq!(depot.locked_count(), 0);
        // Lock one weapon
        depot.set_lock(id1, true).unwrap();
        assert_eq!(depot.locked_count(), 1);
        // Lock the other
        depot.set_lock(id2, true).unwrap();
        assert_eq!(depot.locked_count(), 2);
        // Unlock one
        depot.set_lock(id1, false).unwrap();
        assert_eq!(depot.locked_count(), 1);
    }

    #[test]
    fn gem_depot_locked_count_empty() {
        let depot = GemDepot::new();
        assert_eq!(depot.locked_count(), 0);
    }

    #[test]
    fn gem_depot_locked_count_with_gems() {
        use config::item::CraftShowingType;
        let mut depot = GemDepot::new();
        let id = depot.add_gem("gem_a".to_string(), CraftShowingType::EquipHead, 0);
        assert_eq!(depot.locked_count(), 0);
        depot.set_lock(id, true).unwrap();
        assert_eq!(depot.locked_count(), 1);
    }

    #[test]
    fn equip_depot_locked_count_empty() {
        let depot = EquipDepot::new();
        assert_eq!(depot.locked_count(), 0);
    }
}
