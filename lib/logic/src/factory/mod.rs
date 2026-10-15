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
//! ├── sync.rs                <- ScFactorySync projection (Clause 20.5)
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
//! - **Clause 20** (`sync.rs`): 20.5 done -- every subsystem projects onto its
//!   `Scd*` container and [`FactoryManager::sync_at`] fills all ten
//!   `ScFactorySync` fields. 20.6 done -- `db/migrations/0003_factory.sql`
//!   carries the 32-table schema. Still open: 20.7
//!   (`db/src/subsystems/factory.rs` load/write, which is what actually makes
//!   this struct survive a restart), 20.8 (incremental flush), and the 20.9
//!   tests that need an `FTableAssets` fixture (`sync_manufacture`,
//!   `sync_recycler`, `sync_stt`).
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
pub mod sync;
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
pub use tick::{
    MachineTiming, Tick, TimingWire, completion_tick, current_tick, elapsed_since, is_complete,
};

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

/// How many of `item_id` the inventory holds, across all stacks.
pub fn count_in_inv(inv: &InventoryState, item_id: &str) -> u32 {
    inv.items
        .values()
        .filter(|slot| slot.item_id == item_id)
        .fold(0u32, |acc, slot| acc.saturating_add(slot.count))
}

/// Remove `needed` of `item_id` from the inventory, or nothing at all.
///
/// The all-or-nothing guarantee is the whole point. Every op module used to
/// carry its own copy of this that decremented slots as it walked and only
/// *then* reported failure — so a bag one item short of a recipe had the rest of
/// the ingredients silently destroyed by the failed attempt.
pub fn take_from_inv(inv: &mut InventoryState, item_id: &str, needed: u32) -> bool {
    if needed == 0 {
        return true;
    }
    if count_in_inv(inv, item_id) < needed {
        return false;
    }

    let mut remaining = needed;
    let mut emptied = vec![];
    for (&key, slot) in &mut inv.items {
        if slot.item_id != item_id || slot.count == 0 {
            continue;
        }
        let take = slot.count.min(remaining);
        slot.count -= take;
        remaining -= take;
        if slot.count == 0 {
            emptied.push(key);
        }
        if remaining == 0 {
            break;
        }
    }
    for key in emptied {
        inv.items.remove(&key);
    }

    debug_assert_eq!(remaining, 0, "count_in_inv disagreed with the walk");
    true
}

/// Remove a whole bill of materials, or nothing at all.
///
/// Repeated ids in `bill` are summed before the affordability check, so a recipe
/// listing the same ingredient twice cannot pass on half of it.
pub fn take_bill_from_inv(inv: &mut InventoryState, bill: &[(&str, u32)]) -> bool {
    let mut needed: Vec<(&str, u32)> = Vec::with_capacity(bill.len());
    for &(item_id, count) in bill {
        if count == 0 {
            continue;
        }
        match needed.iter_mut().find(|(id, _)| *id == item_id) {
            Some((_, total)) => *total = total.saturating_add(count),
            None => needed.push((item_id, count)),
        }
    }

    if needed
        .iter()
        .any(|&(item_id, count)| count_in_inv(inv, item_id) < count)
    {
        return false;
    }

    for &(item_id, count) in &needed {
        let taken = take_from_inv(inv, item_id, count);
        debug_assert!(taken, "affordability was checked for the whole bill");
    }
    true
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

/// A recipe running on a machine, with its progress envelope.
///
/// Shared by [`ManufactureMachine::active_recipe`] and
/// [`SoilMachine::planted_seed`] -- a planted seed is a recipe whose output is
/// the crop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveRecipe {
    pub recipe_id: String,
    pub timing: MachineTiming,
}

impl ActiveRecipe {
    /// Start `recipe_id` now at `speed` progress per tick.
    pub fn start(recipe_id: impl Into<String>, speed: u64) -> Self {
        Self {
            recipe_id: recipe_id.into(),
            timing: MachineTiming::start(speed),
        }
    }

    pub fn start_at(now: Tick, recipe_id: impl Into<String>, speed: u64) -> Self {
        Self {
            recipe_id: recipe_id.into(),
            timing: MachineTiming::start_at(now, speed),
        }
    }
}

/// Identity of a single placed machine.
///
/// Every machine-bearing wire message is keyed by `(region, node_id)`, so the
/// machine tables on [`FactoryManager`] are too. Keying by region alone (as
/// Clause 1 did) collapses every machine of a given type in a region onto one
/// entry, which silently overwrites the first one built.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MachineKey {
    pub region: String,
    pub node_id: u32,
}

impl MachineKey {
    pub fn new(region: impl Into<String>, node_id: u32) -> Self {
        Self {
            region: region.into(),
            node_id,
        }
    }
}

/// Clause 9, manufacture machine state.
#[derive(Debug, Clone)]
pub struct ManufactureMachine {
    pub region_name: String,
    pub node_id: u32,
    /// The node's `template_id`; the wire calls this `machine_id`.
    pub machine_id: String,
    pub building_level: u32,
    pub active_recipe: Option<ActiveRecipe>,
    /// Outcome buffer capped at `manufactOutcomeBufferStackMaxCount = 99`
    /// (from `FacManufactConst.json`).
    pub outcome_buffer: Vec<ItemSlot>,
    pub set_count: u32,
    /// Ingredient-cost scale baked in when the active recipe started.
    ///
    /// Held separately from the live scale because the client shows both: the
    /// round in flight was already paid for at the old rate, so re-deriving it
    /// from the current worker loadout would retroactively rewrite what the
    /// player was charged. `1.0` is neutral (no cost-modifying worker).
    pub round_cost_delta_scale: f64,
}

/// Clause 7, processor state.
///
/// `refine_points` recharges once every `refinePointRecoverTime = 14_400`
/// ticks (4 hours). The variable `last_recovery_tick` allows to calculate
/// the currently available quantity of points lazily.
#[derive(Debug, Clone)]
pub struct ProcessorMachine {
    pub region_name: String,
    pub node_id: u32,
    pub machine_id: String,
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
    pub node_id: u32,
    pub machine_id: String,
    pub building_level: u32,
    pub accumulated_value: u64,
    /// Material stacks still in the hopper, in commit order.
    ///
    /// `accumulated_value` is the authoritative ledger; this is the grid the
    /// client renders beside it. Both grow together on commit, and a stack
    /// only leaves once a completed round has actually spent its value, so
    /// the grid never shows material the machine has already consumed.
    pub materials: Vec<ItemSlot>,
    /// Temp storage capped at `recTempStorageLength = 10`.
    pub temp_storage: Vec<ItemSlot>,
    /// Runs while the machine has banked enough value for a product; each
    /// `recBasicGenerateTime = 1_800` progress lands one item in
    /// `temp_storage`. `None` when there is nothing to make.
    pub product_timer: Option<MachineTiming>,
}

/// Clause 12, single trade order.
#[derive(Debug, Clone)]
pub struct TradeOrder {
    pub order_id: String,
    pub accumulated_value: u64,
    pub items_committed: Vec<ItemSlot>,
    /// The contract this order was generated under.
    ///
    /// Stored per order rather than read off the machine: the wire carries it
    /// per order, and orders outlive nothing today only because switching
    /// contracts clears them. Keeping the provenance here means a future
    /// contract switch that *retains* orders cannot mislabel them.
    pub contract_id: String,
    /// Cost scale baked in when the order was generated. `1.0` is neutral.
    ///
    /// Per order for the same reason as `contract_id`: the price the player
    /// was quoted is a property of the order, not of the current roster.
    pub cost_value_delta: f64,
    /// Unique per-order instance ID, assigned by the server when the
    /// order is generated. Used by the client to target cash/delete ops.
    ///
    /// `u64` to match `SCD_FACTORY_TRADE_ORDER.inst_id`: narrowing to `u32`
    /// let a client's high-bit inst id truncate onto an unrelated order.
    pub inst_id: u64,
}

/// Clause 12, trader state.
#[derive(Debug, Clone)]
pub struct TradeMachine {
    pub region_name: String,
    pub node_id: u32,
    pub machine_id: String,
    pub building_level: u32,
    pub active_contract: Option<String>,
    pub orders: Vec<TradeOrder>,
    /// Accrues toward the next order slot at `facTradeOrderGenerateSpeed`.
    ///
    /// Was a bare `last_gen_tick` that every read reset to `now`, discarding
    /// the sub-interval remainder -- so a player checking in every 359s of a
    /// 360s interval never received an order at all.
    pub order_timing: MachineTiming,
}

/// Clause 10, workshop state.
#[derive(Debug, Clone)]
pub struct WorkshopMachine {
    pub region_name: String,
    pub node_id: u32,
    pub machine_id: String,
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
    pub node_id: u32,
    pub machine_id: String,
    pub planted_seed: Option<ActiveRecipe>,
    pub doodad_state: SoilDoodadState,
}

/// `key()` for every machine struct: the table key is derivable from the
/// machine, so a machine can always be re-filed without the caller having to
/// remember which region/node it came from.
macro_rules! impl_machine_key {
    ($($ty:ty),+ $(,)?) => {
        $(impl $ty {
            pub fn key(&self) -> MachineKey {
                MachineKey::new(self.region_name.clone(), self.node_id)
            }
        })+
    };
}

impl_machine_key!(
    ManufactureMachine,
    ProcessorMachine,
    RecyclerMachine,
    TradeMachine,
    WorkshopMachine,
    SoilMachine,
);

/// Clause 15, manual work queue.
///
/// One unit is a *stack* of identical crafts: the wire sends `{id, count}`, and
/// `count` is how many crafts of `recipe_id` remain in this stack. Only the
/// head stack's current craft accrues progress.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualWorkUnit {
    pub recipe_id: String,
    /// Crafts remaining in this stack, including the one in flight. Always >= 1
    /// for a unit that is in the queue.
    pub count: u32,
    pub timing: MachineTiming,
}

impl ManualWorkUnit {
    /// A stack that has been queued but not started.
    ///
    /// Only the head of the queue accrues progress, so a newly appended unit
    /// is created frozen at zero and released by
    /// [`ManualWorkQueue::sync_head_at`] once it reaches the front.
    pub fn queued_at(now: Tick, recipe_id: impl Into<String>, count: u32, speed: u64) -> Self {
        Self {
            recipe_id: recipe_id.into(),
            count: count.max(1),
            timing: MachineTiming::paused_with(now, speed, 0),
        }
    }
}

/// Max stacks in the queue (`manualCraftQueueLength`).
pub const MANUAL_WORK_QUEUE_LEN: usize = 5;

/// Max crafts folded into one stack.
pub const MANUAL_WORK_STACK_MAX: u32 = 10;

#[derive(Debug, Clone, Default)]
pub struct ManualWorkQueue {
    /// At most [`MANUAL_WORK_QUEUE_LEN`] stacks of [`MANUAL_WORK_STACK_MAX`].
    pub queue: Vec<ManualWorkUnit>,
    /// `Some(tick)` while the player has the queue paused. This is the wire's
    /// `pause_tms`; a bare `is_paused` bool could not round-trip it.
    pub paused_at: Option<Tick>,
}

impl ManualWorkQueue {
    pub fn is_paused(&self) -> bool {
        self.paused_at.is_some()
    }

    /// Total crafts queued across every stack.
    pub fn total_crafts(&self) -> u32 {
        self.queue
            .iter()
            .fold(0u32, |acc, unit| acc.saturating_add(unit.count))
    }

    /// How many more crafts of `recipe_id` the queue can take.
    ///
    /// Room comes from two places: the tail stack, if it is the same recipe and
    /// not yet full, plus a full stack for each unused queue slot. Folding only
    /// into the *tail* keeps the queue FIFO — folding into an earlier stack of
    /// the same recipe would quietly move the new work ahead of what is already
    /// waiting.
    pub fn room_for(&self, recipe_id: &str) -> u32 {
        let tail_room = self
            .queue
            .last()
            .filter(|unit| unit.recipe_id == recipe_id)
            .map_or(0, |unit| MANUAL_WORK_STACK_MAX.saturating_sub(unit.count));

        let free_slots = MANUAL_WORK_QUEUE_LEN.saturating_sub(self.queue.len()) as u32;

        tail_room.saturating_add(free_slots.saturating_mul(MANUAL_WORK_STACK_MAX))
    }

    /// Add `count` crafts of `recipe_id`, folding into the tail stack first and
    /// then opening new stacks. Returns how many were actually admitted.
    ///
    /// Callers must clamp to [`Self::room_for`] *before* charging ingredients:
    /// this method admits only what fits, and paying for the rest would lose it.
    pub fn append_at(&mut self, now: Tick, recipe_id: &str, count: u32, speed: u64) -> u32 {
        let mut remaining = count.min(self.room_for(recipe_id));
        let admitted = remaining;

        if let Some(tail) = self.queue.last_mut()
            && tail.recipe_id == recipe_id
        {
            let fold = remaining.min(MANUAL_WORK_STACK_MAX.saturating_sub(tail.count));
            tail.count += fold;
            remaining -= fold;
        }

        while remaining > 0 && self.queue.len() < MANUAL_WORK_QUEUE_LEN {
            let take = remaining.min(MANUAL_WORK_STACK_MAX);
            self.queue
                .push(ManualWorkUnit::queued_at(now, recipe_id, take, speed));
            remaining -= take;
        }

        if admitted > 0 {
            self.sync_head_at(now);
        }
        admitted
    }

    /// Freeze the queue and its head unit at `now`. Idempotent.
    pub fn pause_at(&mut self, now: Tick) {
        if self.paused_at.is_none() {
            self.paused_at = Some(now);
        }
        if let Some(head) = self.queue.first_mut() {
            head.timing.pause_at(now);
        }
    }

    /// Release the queue and its head unit at `now`. Idempotent.
    pub fn resume_at(&mut self, now: Tick) {
        self.paused_at = None;
        if let Some(head) = self.queue.first_mut() {
            head.timing.resume_at(now);
        }
    }

    /// Bring the head unit's timing in line with the queue's pause state.
    ///
    /// Must be called after anything that changes which unit is at the front
    /// (a unit completed, was cancelled, or was appended to an empty queue).
    /// A unit promoted to head while the queue runs has to start accruing;
    /// one promoted while the queue is paused must not.
    pub fn sync_head_at(&mut self, now: Tick) {
        let paused = self.is_paused();
        if let Some(head) = self.queue.first_mut() {
            if paused {
                head.timing.pause_at(now);
            } else {
                head.timing.resume_at(now);
            }
        }
    }
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
