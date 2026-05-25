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
