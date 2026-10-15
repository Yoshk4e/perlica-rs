//! Recycler business logic.
//!
//! The recycler accepts material items, accumulates their `needValue`
//! (from `recyclerMaterialData`), and when enough value has accumulated
//! (`recRoundNeedValue = 100`), generates a random product after
//! `recBasicGenerateTime = 1800` ticks (30 min). Products go into a
//! temp storage capped at `recTempStorageLength = 10` slots.
//!
//! Two ops: `commit_material` (add items, accumulate value) and
//! `fetch_product` (pull generated products from temp storage).

use config::factory_recycler_const::FRecyclerConstAssets;
use config::factory_table::FTableAssets;

use crate::factory::{
    FactoryManager, ItemSlot, MachineKey, MachineTiming, RecyclerMachine, Tick, current_tick,
    push_item_to_bag, take_from_inv,
};

/// Progress per tick for the product timer.
///
/// `recBasicGenerateTime` is a duration in ticks, so the timer accrues 1:1.
/// Named so a future level- or skill-based speed-up has one place to land, and
/// so the wire's `current_speed` has something real to report.
const RECYCLER_SPEED: u64 = 1;

impl FactoryManager {
    /// Commit materials to the recycler. Each material's `needValue`
    /// (from `recyclerMaterialData`) is added to the accumulated value.
    /// Items are consumed from the player's bag. When accumulated value
    /// reaches `recRoundNeedValue`, a product generation timer starts.
    pub fn recycler_commit_material(
        &mut self,
        assets: &FTableAssets,
        recycler_const: &FRecyclerConstAssets,
        region_name: &str,
        node_id: u32,
        materials: &[CommitMaterial],
    ) -> bool {
        // Resolve the node's template id before taking any mutable borrow:
        // it is the wire `machine_id`, and a commit against an unplaced node
        // must fail rather than conjure a machine out of nothing.
        let Some(machine_id) = self.machine_id_of(region_name, node_id).map(str::to_string) else {
            return false;
        };

        // Bank anything the previous round already finished, so the value
        // accounting below starts from an up-to-date machine.
        self.recycler_advance(assets, recycler_const, region_name, node_id);

        // Consume materials from the bag and accumulate value.
        let mut value_added = 0u64;
        let mut committed: Vec<ItemSlot> = vec![];
        let Some(region) = self.region_mut(region_name) else {
            return false;
        };

        // Consume each material from the bag.
        let Some(bag_node) = region.node_mut(1) else {
            return false;
        };
        let Some(bag_comp) = bag_node.component_mut(1) else {
            return false;
        };
        let crate::factory::FactoryComponent::Inventory(bag_inv) = bag_comp else {
            return false;
        };

        for mat in materials {
            // Look up the material's needValue. Each material commits
            // independently -- a bag short on one of them still recycles the
            // rest -- but the take itself is all-or-nothing per material, so a
            // partial stack is never eaten for partial value.
            if let Some(entry) = assets.get_recycler_material(&mat.item_id) {
                let needed = mat.count;
                if take_from_inv(bag_inv, &mat.item_id, needed) {
                    value_added += entry.need_value * needed as u64;
                    committed.push(ItemSlot {
                        item_id: mat.item_id.clone(),
                        count: needed,
                        inst_id: 0,
                    });
                }
            }
        }

        if value_added == 0 {
            return false;
        }

        // Get or create the recycler machine state.
        let machine = self
            .recycler_state
            .entry(MachineKey::new(region_name, node_id))
            .or_insert(RecyclerMachine {
                region_name: region_name.to_string(),
                node_id,
                machine_id,
                building_level: 1,
                accumulated_value: 0,
                materials: vec![],
                temp_storage: vec![],
                product_timer: None,
            });

        machine.accumulated_value += value_added;
        // The hopper grid moves in lockstep with the ledger; `recycler_advance`
        // is what draws it down once a round has actually been paid for.
        for slot in committed {
            stack_into(&mut machine.materials, slot);
        }

        // Value is never deducted up front — only when a product actually
        // lands (see `recycler_advance`). Pre-deducting was how the original
        // `while` loop destroyed a round's worth of value per iteration, and it
        // also made a full temp storage risk losing the paid-for round.
        // This call just releases the timer now that the machine is funded.
        self.recycler_advance(assets, recycler_const, region_name, node_id);

        true
    }

    /// Fetch generated products from the recycler's temp storage.
    /// Pushes them into the player's bag, clears them from storage.
    /// Also runs the lazy timer check to generate pending products.
    pub fn recycler_fetch_product(
        &mut self,
        assets: &FTableAssets,
        recycler_const: &FRecyclerConstAssets,
        region_name: &str,
        node_id: u32,
    ) -> Vec<ItemSlot> {
        self.recycler_advance(assets, recycler_const, region_name, node_id);

        let Some(machine) = self
            .recycler_state
            .get_mut(&MachineKey::new(region_name, node_id))
        else {
            return vec![];
        };
        let items = std::mem::take(&mut machine.temp_storage);

        // Push fetched items into the player's bag.
        if !items.is_empty() {
            let Some(region) = self.region_mut(region_name) else {
                return items;
            };
            if let Some(bag_node) = region.node_mut(1)
                && let Some(bag_comp) = bag_node.component_mut(1)
                && let crate::factory::FactoryComponent::Inventory(bag_inv) = bag_comp
            {
                for item in &items {
                    push_item_to_bag(bag_inv, &item.item_id, item.count, item.inst_id);
                }
            }
        }

        // Emptying storage unblocks the timer.
        self.recycler_advance(assets, recycler_const, region_name, node_id);

        items
    }

    /// Lazy catch-up for one recycler: land every product whose generation
    /// window has closed, charge for each, and leave the timer running only
    /// while the machine can actually work.
    ///
    /// Progress is timestamp-based, so this has to run before anything reads
    /// the machine — on commit, on fetch, and on sync projection.
    pub fn recycler_advance(
        &mut self,
        assets: &FTableAssets,
        recycler_const: &FRecyclerConstAssets,
        region_name: &str,
        node_id: u32,
    ) {
        let gen_time = recycler_const.data.rec_basic_generate_time as u64;
        let max_storage = recycler_const.data.rec_temp_storage_length as usize;
        let round_need = recycler_const.data.rec_round_need_value as u64;
        let product_id = assets.recycler_product_ids().next().map(String::from);
        let now = current_tick();

        let Some(machine) = self
            .recycler_state
            .get_mut(&MachineKey::new(region_name, node_id))
        else {
            return;
        };

        if machine.product_timer.is_some() {
            // Each closed window yields one product, so a player returning
            // after hours collects every round the machine could pay for and
            // store — not just the most recent one. Windows beyond that cap
            // stay banked in the timing rather than being silently dropped.
            let affordable = if round_need == 0 {
                0
            } else {
                machine.accumulated_value / round_need
            };
            let room = max_storage.saturating_sub(machine.temp_storage.len()) as u64;
            let cap = affordable.min(room);

            let produced = machine.product_timer.as_mut().map_or(0, |timer| {
                timer.take_completed_rounds_capped_at(now, gen_time, cap)
            });

            if produced > 0 {
                machine.accumulated_value -= produced * round_need;
                if let Some(product_id) = &product_id {
                    for _ in 0..produced {
                        machine.temp_storage.push(ItemSlot {
                            item_id: product_id.clone(),
                            count: 1,
                            inst_id: 0,
                        });
                    }
                }
                drain_spent_materials(machine, assets);
            }
        }

        sync_recycler_timer(machine, round_need, max_storage, now);
    }
}

/// Merge `slot` into a grid, folding onto an existing stack of the same item.
///
/// The client renders `materials` as a fixed grid of stacks, so a fresh entry
/// per commit would march the hopper off the end of the UI after a handful of
/// deposits of the same material.
fn stack_into(grid: &mut Vec<ItemSlot>, slot: ItemSlot) {
    if let Some(existing) = grid.iter_mut().find(|s| s.item_id == slot.item_id) {
        existing.count = existing.count.saturating_add(slot.count);
    } else {
        grid.push(slot);
    }
}

/// Total `needValue` still sitting in the hopper.
fn materials_value(machine: &RecyclerMachine, assets: &FTableAssets) -> u64 {
    machine
        .materials
        .iter()
        .map(|slot| unit_value(assets, &slot.item_id).saturating_mul(u64::from(slot.count)))
        .sum()
}

/// One unit of `item_id` as `needValue`, or 0 if it is not a recycler material.
fn unit_value(assets: &FTableAssets, item_id: &str) -> u64 {
    assets
        .get_recycler_material(item_id)
        .map_or(0, |entry| entry.need_value)
}

/// Draw the hopper grid down to what the ledger still holds value for.
///
/// Called only after a round has landed, so it removes exactly the material
/// that round consumed, oldest first. The residue -- always less than one
/// unit's worth -- stays on the grid rather than being rounded away, which
/// matches `accumulated_value` keeping its sub-round remainder. Items that
/// dropped out of `recyclerMaterialData` since they were committed are worth
/// nothing and cannot be drained by value, so they are discarded outright
/// rather than wedging the loop.
fn drain_spent_materials(machine: &mut RecyclerMachine, assets: &FTableAssets) {
    let mut held = materials_value(machine, assets);

    while let Some(front) = machine.materials.first_mut() {
        let unit = unit_value(assets, &front.item_id);
        if unit == 0 {
            machine.materials.remove(0);
            continue;
        }

        let spent = held.saturating_sub(machine.accumulated_value);
        let removable = spent / unit;
        if removable == 0 {
            break;
        }

        let take = u32::try_from(removable)
            .unwrap_or(u32::MAX)
            .min(front.count);
        front.count -= take;
        held -= u64::from(take) * unit;
        if front.count == 0 {
            machine.materials.remove(0);
        }
    }
}

/// Keep the product timer's run state in step with what the machine can do.
///
/// A recycler that is out of value, or whose temp storage is full, is *stalled*
/// rather than idle: progress freezes where it is. Letting it keep accruing
/// would hand the player a burst of free rounds the moment they topped it up,
/// and dropping the timer outright would throw away the sub-round remainder
/// they had already earned. Freezing is the only option that does neither, and
/// it gives the wire's `pause_ts` an honest "waiting for materials" value.
fn sync_recycler_timer(
    machine: &mut RecyclerMachine,
    round_need: u64,
    max_storage: usize,
    now: Tick,
) {
    let can_work =
        machine.accumulated_value >= round_need && machine.temp_storage.len() < max_storage;

    match (&mut machine.product_timer, can_work) {
        (Some(timer), true) => timer.resume_at(now),
        (Some(timer), false) => timer.pause_at(now),
        (None, true) => machine.product_timer = Some(MachineTiming::start_at(now, RECYCLER_SPEED)),
        (None, false) => {}
    }
}

#[derive(Debug, Clone)]
pub struct CommitMaterial {
    pub item_id: String,
    pub count: u32,
}
