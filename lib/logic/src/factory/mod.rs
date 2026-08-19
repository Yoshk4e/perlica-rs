//! Shape at highest level (see §1 of implementation plan):
//!
//! - The [`FactoryManager`] (one per player) manages N [`FactoryRegion`]s.
//! - Each region contains placed [`FactoryNode`]s, power/transport
//!   connections [`FactoryConnection`]s, a [`PowerBlackboard`], and state
//!   specific to each machine.
//! - Each node contains a `Vec<(component_id, `[`FactoryComponent`]`)>`
//!   and a `FCComponentPos -> component_id` lookup table.
//! - Progress is always based on timestamps (`Tick` = Unix seconds) and
//!   there is no global tick function looping over every building every
//!   second.
//!
//! Clause 1 (this module) provides the basic types, the manager, the
//! blackboard and the bootstrap code which produces the inventory/hub
//! node pair. Individual machine implementations (manuf, proc, recycle,
//! trade, workshop, soil, manual and character work, STT, quickbar),
//! according to Clause 1, will go into separate modules when the clauses
//! arrive; currently the *types* definitions remain in this module (see
//! "machine state structs" below) since the shape of the
//! [`FactoryManager`] must be known.
//!
//! File layout according to §8 of implementation plan:
//!
//! ```text
//! factory/
//! ├── mod.rs                 <- this file: machine state structs + re-exports
//! ├── tick.rs                <- Tick + timing math
//! ├── grid.rs                <- grid validation utilities
//! ├── blackboard.rs          <- PowerBlackboard
//! ├── power.rs               <- PowerGraph + fuel/skill helpers (Clause 4 stub)
//! ├── component.rs           <- FactoryComponent + per-variant state structs
//! ├── component_factory.rs   <- template -> components (Clause 3 stub)
//! ├── node.rs                <- FactoryNode + NodeTransform + Mesh + SubPort
//! ├── region.rs              <- FactoryRegion + FactoryConnection
//! ├── manager.rs             <- FactoryManager + bootstrap_region
//! └── tests.rs               <- cross-module Clause 1 acceptance tests
//! ```
//!
//! # TODO (future-looking, per Clause)
//!
//! - **Clause 2** (`factory/op/...`): node op dispatcher, place, dismantle,
//!   move, enable, connection, target, conveyor, gridbox, cache_transport,
//!   special. Grid validation exists in [`grid`].
//! - **Clause 3** (`component.rs`, `node.rs`, `region.rs`): `to_proto()`s of all components + nodes + regions.
//! - **Clause 4** (`power.rs`): proper `PowerGraph::compute`, power loss /
//!   power recovery handling, `BurnPower` fuel use.
//! - **Clause 7** (`processor.rs`): `ProcessorMachine` implementation block +
//!   refine point recovery + 200 recipes.
//! - **Clause 8** (`manufacture.rs` / `machine_crafter.rs`): tick-based
//!   progress & completion checks wiring + 60 auto craft recipes.
//! - **Clause 9** (`manufacture.rs`): long-running crafts, settle =
//!   exploration card.
//! - **Clause 10** (`workshop.rs`): 22 workshop recipes.
//! - **Clause 11** (`recycler.rs`): value accumulation + production time.
//! - **Clause 12** (`trade.rs`): weighted random order creation, order commitment,
//!   reward distribution.
//! - **Clause 13** (`soil.rs`): seed -> doodad -> harvest.
//! - **Clause 14** (`character_work.rs`): 18 factory skills -> modifier.
//! - **Clause 15** (`manual_work.rs`): Queue length = 5, 16 manual work recipes.
//! - **Clause 16** (`quickbar.rs`): 2 quickbar pages (`FCQuickBarType`
//!   Inner=0 / Outer=1), each a flat 32-slot (4x8) grid, with set and move
//!   operations.
//! - **Clause 17** (`stt.rs`): 31 nodes + condition evaluator (see Appendix B).
//! - **Clause 19** (`repair.rs` / `depot.rs`): 7 repair buildings + depot bridge messages.
//! - **DB** (`db/migrations/0003_factory.sql`, `db/src/subsystems/f

pub mod blackboard;
pub mod character_work;
pub mod component;
pub mod component_factory;
pub mod grid;
pub mod machine_crafter;
pub mod manager;
pub mod manual_work;
pub mod manufacture;
pub mod node;
pub mod observer;
pub mod ops;
pub mod power;
pub mod processor;
pub mod quickbar;
pub mod recycler;
pub mod region;
pub mod repair;
pub mod soil;
pub mod stt;
pub mod tick;
pub mod trade;
pub mod workshop;

#[cfg(test)]
mod tests;

pub use blackboard::PowerBlackboard;
pub use component::{
    BoxConveyorState, BurnPowerState, BusLoaderState, CachePort, CacheState, CacheTransportState,
    CollectorState, ConveyorItem, FactoryComponent, GridBoxState, HealTowerState, InventoryState,
    ItemSlot, PowerPoleState, PowerSaveState, ProducerState, SelectorState, StablePowerState,
    TravelPoleState,
};
pub use manager::FactoryManager;
pub use node::{FactoryNode, InteractiveObject, Mesh, NodeTransform, SubPort, Vector3};
pub use region::{FactoryConnection, FactoryRegion};
pub use tick::{Tick, completion_tick, current_tick, elapsed_since, is_complete};

pub fn push_item_to_bag(inv: &mut InventoryState, item_id: &str, count: u32, inst_id: u32) {
    for slot in inv.items.values_mut() {
        if slot.item_id == item_id && slot.inst_id == inst_id {
            slot.count += count;
            return;
        }
    }
    let mut key = inst_id;
    while inv.items.contains_key(&key) {
        key += 1;
    }
    inv.items.insert(
        key,
        ItemSlot {
            item_id: item_id.to_string(),
            count,
            inst_id,
        },
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridRange {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone)]
pub struct ActiveRecipe {
    pub recipe_id: String,
    pub start_tick: Tick,
}

/// Clause 9, manufacture machine state.
#[derive(Debug, Clone)]
pub struct ManufactureMachine {
    pub region_name: String,
    pub building_level: u32,
    pub active_recipe: Option<ActiveRecipe>,
    /// Outcome buffer capped at `manufactOutcomeBufferStackMaxCount = 99`
    /// (from `FacManufactConst.json`).
    pub outcome_buffer: Vec<ItemSlot>,
    pub set_count: u32,
}

/// Clause 7, processor state.
///
/// `refine_points` recharges once every `refinePointRecoverTime = 14_400`
/// ticks (4 hours). The variable `last_recovery_tick` allows to calculate
/// the currently available quantity of points lazily.
#[derive(Debug, Clone)]
pub struct ProcessorMachine {
    pub region_name: String,
    pub building_level: u32,
    pub refine_points: u32,
    pub last_recovery_tick: Tick,
    pub unlocked_formulas: Vec<String>,
    pub unread_formulas: Vec<String>,
}

/// Clause 11, recycler state.
#[derive(Debug, Clone)]
pub struct RecyclerMachine {
    pub region_name: String,
    pub accumulated_value: u64,
    /// Temp storage capped at `recTempStorageLength = 10`.
    pub temp_storage: Vec<ItemSlot>,
    /// Set when a product starts generating; clears when it lands in
    /// `temp_storage` after `recBasicGenerateTime = 1_800` ticks.
    pub product_timer_start: Option<Tick>,
}

/// Clause 12, single trade order.
#[derive(Debug, Clone)]
pub struct TradeOrder {
    pub order_id: String,
    pub accumulated_value: u64,
    pub items_committed: Vec<ItemSlot>,
    /// Unique per-order instance ID, assigned by the server when the
    /// order is generated. Used by the client to target cash/delete ops.
    pub inst_id: u32,
}

/// Clause 12, trader state.
#[derive(Debug, Clone)]
pub struct TradeMachine {
    pub region_name: String,
    pub building_level: u32,
    pub active_contract: Option<String>,
    pub orders: Vec<TradeOrder>,
    pub last_gen_tick: Tick,
}

/// Clause 10, workshop state.
#[derive(Debug, Clone)]
pub struct WorkshopMachine {
    pub region_name: String,
    pub building_level: u32,
}

/// Clause 13, soil/farming state.
#[derive(Debug, Clone)]
pub enum SoilDoodadState {
    Empty,
    Growing,
    Mature,
}

#[derive(Debug, Clone)]
pub struct SoilMachine {
    pub region_name: String,
    pub planted_seed: Option<ActiveRecipe>,
    pub doodad_state: SoilDoodadState,
}

/// Clause 15, manual work queue.
#[derive(Debug, Clone)]
pub struct ManualWorkUnit {
    pub recipe_id: String,
    pub start_tick: Tick,
    pub progress: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ManualWorkQueue {
    /// Capped at 5 (`manualCraftQueueLength`); each unit stacks up to 10.
    pub queue: Vec<ManualWorkUnit>,
    pub is_paused: bool,
}

/// Clause 14, character work state.
#[derive(Debug, Clone)]
pub struct CharacterWorker {
    pub region_name: String,
    pub char_inst_id: u32,
    pub skill_ids: Vec<String>,
    pub work_slot: u32,
}

#[derive(Debug, Clone, Default)]
pub struct CharacterWorkState {
    pub workers: Vec<CharacterWorker>,
}

/// Clause 16, quickbar state.
///
/// The wire `type` is the `FCQuickBarType` enum (Inner=0, Outer=1).
/// `items` is a flat 4 rows x 8 columns grid (32 slots, row-major), and the
/// wire `SCD_FACTORY_SYNC_QUICKBAR.list` must always carry exactly
/// [`QUICKBAR_SIZE`] entries.
#[derive(Debug, Clone)]
pub struct QuickbarState {
    pub quickbar_type: i32,
    pub items: Vec<String>,
}

/// Number of slots in one quickbar page: 4 bars x 8 slots.
pub const QUICKBAR_SIZE: usize = 32;

/// Clause 17, STT tech tree state.
#[derive(Debug, Clone, Default)]
pub struct SttState {
    pub unlocked_nodes: Vec<String>,
    pub visible_formulas: Vec<String>,
}
