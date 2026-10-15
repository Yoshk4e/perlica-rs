//! Factory state projection onto the wire (clause 20.5).
//!
//! One method per subsystem, each returning its `Scd*` container, and
//! `to_sync()` aggregating them into a fully-populated `ScFactorySync`.
//!
//! This lives in `lib/logic` rather than in the handler for the same reason
//! `component.rs`/`node.rs`/`region.rs` keep their `to_proto()` here: the
//! handlers were each hand-rolling their own snapshot, and the login push and
//! the per-op responses drifted apart field by field -- most visibly in the
//! timing quartet, which the trade handler was filling with literal zeroes
//! while the internal `MachineTiming` held real progress. A single projection
//! makes drift impossible by construction.
//!
//! Every method takes `now` explicitly rather than reading the clock, so a
//! whole sync is one consistent instant and tests can drive it from `T0`.

use config::factory_recycler_const::FRecyclerConstAssets;
use config::factory_sttree::FSttreeAssets;
use config::factory_table::FTableAssets;

use perlica_proto::{
    ItemBundle, ScFactorySync, ScdFactoryManufacture, ScdFactoryManufactureMachine,
    ScdFactoryProcessor, ScdFactoryProcessorMachine, ScdFactoryRecycler, ScdFactoryRecyclerMachine,
    ScdFactoryRepair, ScdFactoryRepairBuilding, ScdFactorySoil, ScdFactorySoilMachine,
    ScdFactorySttNode, ScdFactorySyncFormulaMan, ScdFactorySyncManuallyWork,
    ScdFactorySyncManuallyWorkUnit, ScdFactorySyncStt, ScdFactoryTrade, ScdFactoryTradeMachine,
    ScdFactoryTradeOrder, ScdFactoryWorkshop, ScdFactoryWorkshopMachine, ScdItemGrid,
};

use crate::factory::character_work::SKILL_TYPE_CRAFT_COST;
use crate::factory::tick::TimingWire;
use crate::factory::{FactoryManager, ItemSlot, Tick, current_tick};

/// Clamp a `u32` count onto the wire's `i32`.
fn count_i32(count: u32) -> i32 {
    i32::try_from(count).unwrap_or(i32::MAX)
}

/// Clamp a `u64` tick or value onto the wire's `i64`.
fn value_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// The timing quartet for a machine with nothing running.
///
/// `TimingWire` deliberately has no `Default`: a zeroed envelope is a claim
/// ("stopped at progress 0, never paused"), not an absence, so it is spelled
/// out at the one place that means it rather than summoned by `unwrap_or_default`.
const IDLE_TIMING: TimingWire = TimingWire {
    update_ts: 0,
    current_progress: 0,
    current_speed: 0,
    pause_ts: 0,
};

/// Project an item buffer onto the client's grid, indexed by slot position.
fn item_grid(slots: &[ItemSlot]) -> Vec<ScdItemGrid> {
    slots
        .iter()
        .enumerate()
        .map(|(index, slot)| ScdItemGrid {
            grid_index: i32::try_from(index).unwrap_or(i32::MAX),
            id: slot.item_id.clone(),
            count: i64::from(count_i32(slot.count)),
            inst: None,
        })
        .collect()
}

impl FactoryManager {
    /// Manufacture machines in `region_name`, as of `now`.
    pub fn sync_manufacture(
        &self,
        assets: &FTableAssets,
        region_name: &str,
        now: Tick,
    ) -> ScdFactoryManufacture {
        let machines = self
            .manufacture_in(region_name)
            .into_iter()
            .map(|(key, machine)| {
                let (formula, timing) = machine.active_recipe.as_ref().map_or_else(
                    || (String::new(), IDLE_TIMING),
                    |recipe| (recipe.recipe_id.clone(), recipe.timing.wire_at(now)),
                );

                // The recipe's own round size, not `set_count`: `set_count` is
                // how many sets are still queued, which the client tracks
                // through the craft responses. `least_multi` is the batch
                // granularity, the same quantity `describe_discarded` reports
                // as `old_least_multi` when a round is thrown away.
                let least_multi = assets
                    .get_manufact_craft(&formula)
                    .map_or(0, |recipe| count_i32(recipe.round_count));

                let current_saved: u32 = machine.outcome_buffer.iter().map(|slot| slot.count).sum();

                ScdFactoryManufactureMachine {
                    region: key.region.clone(),
                    node_id: key.node_id,
                    machine_id: machine.machine_id.clone(),
                    building_level: count_i32(machine.building_level),
                    update_ts: timing.update_ts_i64(),
                    formula,
                    least_multi,
                    current_saved: count_i32(current_saved),
                    current_progress: timing.current_progress_i64(),
                    current_speed: timing.current_speed_i64(),
                    pause_ts: timing.pause_ts_i64(),
                    // Live rate versus the rate this round was charged at. They
                    // differ once a worker punches in or out mid-round.
                    cost_delta_scale: self.skill_cost_scale(
                        assets,
                        region_name,
                        &machine.machine_id,
                        SKILL_TYPE_CRAFT_COST,
                    ),
                    current_round_cost_delta_scale: machine.round_cost_delta_scale,
                }
            })
            .collect();

        ScdFactoryManufacture { machines }
    }

    /// Processor machines in `region_name`.
    pub fn sync_processor(&self, region_name: &str) -> ScdFactoryProcessor {
        let machines = self
            .processor_in(region_name)
            .into_iter()
            .map(|(key, machine)| ScdFactoryProcessorMachine {
                region: key.region.clone(),
                node_id: key.node_id,
                machine_id: machine.machine_id.clone(),
                building_level: count_i32(machine.building_level),
                refine_point: count_i32(machine.refine_points),
                last_refine_point_recover_ts: value_i64(machine.last_recovery_tick),
            })
            .collect();

        // The "new formula!" badge is per region, not per machine, and the same
        // formula can be unlocked at more than one processor -- so dedupe, or
        // the client shows the same badge twice.
        let mut new_unlocked_formula: Vec<String> = Vec::new();
        for (_, machine) in self.processor_in(region_name) {
            for formula in &machine.unread_formulas {
                if !new_unlocked_formula.contains(formula) {
                    new_unlocked_formula.push(formula.clone());
                }
            }
        }

        ScdFactoryProcessor {
            machines,
            new_unlocked_formula,
        }
    }

    /// Recycler machines in `region_name`, as of `now`.
    pub fn sync_recycler(
        &self,
        recycler_const: &FRecyclerConstAssets,
        region_name: &str,
        now: Tick,
    ) -> ScdFactoryRecycler {
        let round_need = recycler_const.data.rec_round_need_value as u64;

        let machines = self
            .recycler_in(region_name)
            .into_iter()
            .map(|(key, machine)| {
                let timing = machine
                    .product_timer
                    .as_ref()
                    .map_or(IDLE_TIMING, |timer| timer.wire_at(now));

                let products = machine
                    .temp_storage
                    .iter()
                    .map(|slot| ItemBundle {
                        id: slot.item_id.clone(),
                        count: count_i32(slot.count),
                    })
                    .collect();

                // Value banked toward the next product, i.e. what a full round
                // has not already claimed. A zero `recRoundNeedValue` would be
                // a config fault; report the whole balance rather than divide
                // by it, matching the guard in `recycler_advance`.
                let remainder_value = if round_need == 0 {
                    machine.accumulated_value
                } else {
                    machine.accumulated_value % round_need
                };

                ScdFactoryRecyclerMachine {
                    region: key.region.clone(),
                    node_id: key.node_id,
                    machine_id: machine.machine_id.clone(),
                    building_level: count_i32(machine.building_level),
                    update_ts: timing.update_ts_i64(),
                    materials: item_grid(&machine.materials),
                    products,
                    current_progress: timing.current_progress_i64(),
                    pause_ts: timing.pause_ts_i64(),
                    remainder_value: i32::try_from(remainder_value).unwrap_or(i32::MAX),
                    current_speed: timing.current_speed_f64(),
                }
            })
            .collect();

        ScdFactoryRecycler { machines }
    }

    /// Trade machines in `region_name`, as of `now`.
    pub fn sync_trade(&self, region_name: &str, now: Tick) -> ScdFactoryTrade {
        let machines = self
            .trade_in(region_name)
            .into_iter()
            .map(|(key, machine)| {
                let timing = machine.order_timing.wire_at(now);

                let order_list = machine
                    .orders
                    .iter()
                    .map(|order| ScdFactoryTradeOrder {
                        contract_id: order.contract_id.clone(),
                        inst_id: order.inst_id,
                        order_id: order.order_id.clone(),
                        cost_value_delta: order.cost_value_delta,
                    })
                    .collect();

                ScdFactoryTradeMachine {
                    region: key.region.clone(),
                    node_id: key.node_id,
                    machine_id: machine.machine_id.clone(),
                    building_level: count_i32(machine.building_level),
                    update_ts: timing.update_ts_i64(),
                    contract_id: machine.active_contract.clone().unwrap_or_default(),
                    order_list,
                    current_progress: timing.current_progress_i64(),
                    pause_ts: timing.pause_ts_i64(),
                    current_speed: timing.current_speed_f64(),
                }
            })
            .collect();

        ScdFactoryTrade { machines }
    }

    /// Workshop machines in `region_name`.
    pub fn sync_workshop(&self, region_name: &str) -> ScdFactoryWorkshop {
        let machines = self
            .workshop_in(region_name)
            .into_iter()
            .map(|(key, machine)| ScdFactoryWorkshopMachine {
                region: key.region.clone(),
                node_id: key.node_id,
                machine_id: machine.machine_id.clone(),
                building_level: count_i32(machine.building_level),
            })
            .collect();

        ScdFactoryWorkshop { machines }
    }

    /// Soil plots in `region_name`, as of `now`.
    ///
    /// An empty plot still ships: the client keys its plot UI off this list, so
    /// omitting the fallow ones would hide the plots the player owns.
    /// `doodad_state` stays server-side -- the wire has no field for it.
    pub fn sync_soil(&self, region_name: &str, now: Tick) -> ScdFactorySoil {
        let machines = self
            .soil_in(region_name)
            .into_iter()
            .map(|(key, machine)| {
                let (seed_item_id, timing) = machine.planted_seed.as_ref().map_or_else(
                    || (String::new(), IDLE_TIMING),
                    |seed| (seed.recipe_id.clone(), seed.timing.wire_at(now)),
                );

                ScdFactorySoilMachine {
                    region: key.region.clone(),
                    node_id: key.node_id,
                    machine_id: machine.machine_id.clone(),
                    update_ts: timing.update_ts_i64(),
                    seed_item_id,
                    current_progress: timing.current_progress_i64(),
                }
            })
            .collect();

        ScdFactorySoil { machines }
    }

    /// Repaired buildings in `region_name`.
    pub fn sync_repair(&self, region_name: &str) -> ScdFactoryRepair {
        let buildings = self
            .repaired_buildings_in(region_name)
            .into_iter()
            .map(|(repair_id, node_id)| ScdFactoryRepairBuilding {
                repair_id: repair_id.to_string(),
                node_id,
            })
            .collect();

        ScdFactoryRepair { buildings }
    }

    /// The manual work queue as of `now`.
    ///
    /// One wire unit per *stack*, with `count` = crafts still in that stack
    /// (the in-flight one included). Emitting a unit per craft with `count: 1`
    /// would overflow the client's 5-slot queue UI as soon as a stack folded.
    pub fn sync_manual_work(&self, now: Tick) -> ScdFactorySyncManuallyWork {
        let queue = self
            .manual_work_state
            .queue
            .iter()
            .map(|unit| ScdFactorySyncManuallyWorkUnit {
                id: unit.recipe_id.clone(),
                count: count_i32(unit.count),
            })
            .collect();

        ScdFactorySyncManuallyWork {
            queue,
            in_block: self.manual_work_in_block(),
            // The *virtual* start that reproduces the progress banked so far,
            // which is what the client extrapolates from -- not the tick the
            // unit was queued at.
            head_start_tms: value_i64(self.manual_work_head_start(now)),
            // The instant the queue actually froze. Sending `now` instead reset
            // the client's frozen-at reference on every resend.
            pause_tms: value_i64(self.manual_work_state.paused_at.unwrap_or(0)),
        }
    }

    /// The tech tree and formula book. Region-scoped because STT node
    /// conditions are evaluated against one region's buildings.
    pub fn sync_stt(&self, sttree: &FSttreeAssets, region_name: &str) -> ScdFactorySyncStt {
        let nodes = self
            .stt_sync_all(sttree, region_name)
            .into_iter()
            .map(|(id, data)| ScdFactorySttNode {
                id,
                state: data.state,
                values: data.values,
                flags: data.flags,
            })
            .collect();

        ScdFactorySyncStt { nodes }
    }

    /// The formula book. `unlocked` and `visible` carry the same list: a
    /// formula becomes visible by being unlocked, and the client uses the two
    /// for different views of it.
    pub fn sync_formula_man(&self) -> ScdFactorySyncFormulaMan {
        let formulas = self.stt_state.visible_formulas.clone();
        ScdFactorySyncFormulaMan {
            unlocked: formulas.clone(),
            visible: formulas,
        }
    }

    /// Every factory subsystem in one message, all fields populated.
    ///
    /// Convenience wrapper over [`FactoryManager::sync_at`] for callers that
    /// mean "right now" -- the handlers. Tests should prefer `sync_at`.
    pub fn to_sync(
        &self,
        assets: &FTableAssets,
        recycler_const: &FRecyclerConstAssets,
        sttree: &FSttreeAssets,
        region_name: &str,
    ) -> ScFactorySync {
        self.sync_at(assets, recycler_const, sttree, region_name, current_tick())
    }

    /// Every factory subsystem in one message, projected at `now`.
    ///
    /// Nothing here is left `None`: the client's per-field guards mean a
    /// missing container is silently ignored, so a subsystem that is absent
    /// from the message is indistinguishable from one that is empty -- and the
    /// client keeps whatever stale state it had. An empty container is the
    /// honest way to say "no machines".
    pub fn sync_at(
        &self,
        assets: &FTableAssets,
        recycler_const: &FRecyclerConstAssets,
        sttree: &FSttreeAssets,
        region_name: &str,
        now: Tick,
    ) -> ScFactorySync {
        ScFactorySync {
            stt: Some(self.sync_stt(sttree, region_name)),
            formula_man: Some(self.sync_formula_man()),
            manually_work: Some(self.sync_manual_work(now)),
            manufacture: Some(self.sync_manufacture(assets, region_name, now)),
            trade: Some(self.sync_trade(region_name, now)),
            repair: Some(self.sync_repair(region_name)),
            workshop: Some(self.sync_workshop(region_name)),
            recycler: Some(self.sync_recycler(recycler_const, region_name, now)),
            processor: Some(self.sync_processor(region_name)),
            soil: Some(self.sync_soil(region_name, now)),
        }
    }
}
