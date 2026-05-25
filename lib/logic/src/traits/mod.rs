//! # Cross-cutting traits for `perlica-logic`
//!
//! This module is the project's central **abstraction layer**.  Historically
//! every domain type (depots, instance-id newtypes, item instances, position
//! holders, mail entries, ...) carried hand-rolled, near-identical methods.
//! The traits in this directory unify those behaviours so generic helpers can
//! work over any conforming type using **static dispatch**.
//!
//! ## Design rules
//!
//! * **Additive** - existing inherent methods continue to work; Rust resolves
//!   inherent first, so trait methods coexist without breaking call sites.
//! * **Default methods** wherever possible, so concrete impls stay minimal
//!   (often just associated-type / field accessors).
//! * **Blanket extension traits** (`PositionedExt`, `KeyedContainerExt`, ...)
//!   carry generic algorithms - adding helpers does not require touching the
//!   underlying impl blocks.
//! * **Zero `dyn Trait`** - every helper is generic with explicit bounds.
//!
//! ## Module map
//!
//! ```text
//!   id           - AsU64, InstanceId
//!   item         - Identifiable, Templated, Lockable, NewFlaggable,
//!                  Owned, Attachable
//!   container/   - Container, KeyedContainer, IdAllocator, DepotKind
//!                  + KeyedContainerExt, LockableContainerExt
//!   world/       - Positioned3D[Mut], Rotated3D[Mut] + PositionedExt
//!   sync         - SyncWriteBack<T>
//!   lifecycle    - Expirable
//!   proto        - ToProto<P>, ToProtoWith<P, Ctx>
//!   player       - PlayerComponent marker
//!   classify     - Classified
//!   util         - count_by_template
//! ```
//!
//! ## Prelude
//!
//! Most downstream code wants everything in scope at once - use
//! [`prelude`] for that:
//!
//! ```ignore
//! use perlica_logic::traits::prelude::*;
//! ```

pub mod classify;
pub mod container;
pub mod id;
pub mod item;
pub mod lifecycle;
pub mod player;
pub mod proto;
pub mod sync;
pub mod util;
pub mod world;

pub use classify::Classified;
pub use container::{
    Container, ContainerExt, DepotKind, IdAllocator, KeyedContainer, KeyedContainerExt,
    LockableContainerExt,
};
pub use id::{AsU64, InstanceId};
pub use item::{Attachable, Identifiable, Lockable, NewFlaggable, Owned, Templated};
pub use lifecycle::Expirable;
pub use player::PlayerComponent;
pub use proto::{ToProto, ToProtoWith};
pub use sync::SyncWriteBack;
pub use util::count_by_template;
pub use world::{Positioned3D, Positioned3DMut, PositionedExt, Rotated3D, Rotated3DMut};

/// Glob-importable bundle of the traits used by 90 %+ of call-sites.
///
/// Importing the prelude pulls in every extension trait, so generic
/// helpers like `depot.get_or_not_found(id, "weapon")` or
/// `entity.distance_sq_to(&other)` resolve without further fuss.
pub mod prelude {
    pub use super::classify::Classified;
    pub use super::container::{
        Container, ContainerExt, DepotKind, IdAllocator, KeyedContainer, KeyedContainerExt,
        LockableContainerExt,
    };
    pub use super::id::{AsU64, InstanceId};
    pub use super::item::{Attachable, Identifiable, Lockable, NewFlaggable, Owned, Templated};
    pub use super::lifecycle::Expirable;
    pub use super::player::PlayerComponent;
    pub use super::proto::{ToProto, ToProtoWith};
    pub use super::sync::SyncWriteBack;
    pub use super::world::{Positioned3D, Positioned3DMut, PositionedExt, Rotated3D, Rotated3DMut};
}
