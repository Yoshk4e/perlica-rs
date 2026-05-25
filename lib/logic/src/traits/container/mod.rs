//! Container traits, the unifying surface over every depot / manager.
//!
//! Depot types in `perlica-logic` follow a near-identical shape: an inner
//! `HashMap<Id, Instance>`, a monotonically-increasing id counter, a fixed
//! `DEPOT_TYPE` constant for the wire protocol, and the usual
//! `len / is_empty / contains / get / get_mut` accessors. These traits
//! formalize that shape so:
//!
//!   * generic helpers (`ContainerExt`, `KeyedContainerExt`,
//!     `LockableContainerExt`) can replace hand-rolled iteration code,
//!   * `IdAllocator` lets the save-migration layer reseed every depot with
//!     a single generic function,
//!   * `DepotKind` exposes the wire-protocol constant without forcing
//!     callers to import the depot type.
//!
//! Concrete impls live in [`impls`] so this module reads as a contract,
//! not a wiring diagram.

mod ext;
mod impls;

pub use ext::{ContainerExt, KeyedContainerExt, LockableContainerExt};

use super::id::InstanceId;

/// A collection that knows its size.
///
/// Implemented by every depot and every "manager-of-things" struct.
pub trait Container {
    type Item;

    fn len(&self) -> usize;

    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A [`Container`] that is also addressable by a typed key.
pub trait KeyedContainer: Container {
    type Key: Copy;

    fn contains_key(&self, key: Self::Key) -> bool;
    fn get_ref(&self, key: Self::Key) -> Option<&<Self as Container>::Item>;
    fn get_mut_ref(&mut self, key: Self::Key) -> Option<&mut <Self as Container>::Item>;
}

/// A depot that can hand out fresh ids and accept full instances.
///
/// Mirrors the `next_inst_id` / `set_next_inst_id` pattern that every depot
/// has - generic save-load code uses `bump_next_id_to` to make sure the
/// counter is consistent with the highest restored id.
pub trait IdAllocator {
    type Id: InstanceId;

    fn peek_next_id(&self) -> u64;
    fn bump_next_id_to(&mut self, at_least: u64);
}

/// A container with an external `DEPOT_TYPE` integer the protocol uses to
/// identify it on the wire.
pub trait DepotKind {
    const DEPOT_TYPE: i32;

    #[inline]
    fn depot_type(&self) -> i32 {
        Self::DEPOT_TYPE
    }
}
