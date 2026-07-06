//! Power graph + fuel / storage math.
//!
//! Clause 1 stub: `PowerGraph::compute` is left as a `Default`-returning stub
//! because the real implementation (BFS from `StablePower` / `BurnPower`
//! sources through `PowerPole` connections, summing generation / consumption /
//! storage) is Clause 4 work, see §4.4 / §4.5 of the implementation plan and
//! tasks 4.1–4.8.

use std::collections::HashSet;

use super::{BurnPowerState, FactoryRegion, Tick};
use crate::factory::tick::elapsed_since;

/// Computed power graph. Not persisted, recomputed on demand whenever the
/// network changes (node placed/dismantled, connection added/removed, fuel
/// runs out, etc.). See §4.4 for the reset semantics that fire when this
/// flips from "powered" to "unpowered" for any given producer.
#[derive(Debug, Clone, Default)]
pub struct PowerGraph {
    /// Node IDs that have at least one power-source path through poles.
    pub powered_nodes: HashSet<u32>,
    /// Sum of all `StablePower.power_gen_per_sec` + active `BurnPower` outputs.
    pub total_generation: i64,
    /// Sum of all `Producer.power_cost` + `Collector.power_cost`.
    pub total_consumption: i64,
    /// Sum of all `PowerSave.power_save_max` (capacity, not current).
    pub total_storage: i64,
    /// Sum of all `PowerSave.power_save` (current stored energy).
    pub total_stored: i64,
}

impl PowerGraph {
    ///
    /// Reconstruct the graph from a region snapshot.
    ///
    /// TODO(Clause 4): implement. The algorithm should be:
    /// 1. Find all nodes having a `StablePower` or `BurnPower` component
    ///    (source nodes) and all nodes having a `PowerPole` component
    ///    (relay).
    /// 2. BFS the `FactoryConnection { connection_type: Power }` edges,
    ///    starting at the source nodes and reaching through relay nodes.
    /// 3. All reached nodes have `in_power = true` for their power
    ///    components, and non-reached nodes have `in_power = false`.
    /// 4. Total up the generation/consumption/storage from the powered
    ///    nodes.
    /// 5. If `total_consumption > total_generation + total_stored`,
    ///    toggle `blackboard.is_stop_by_power` to true (and the reset
    ///    handlers in §4.4 will fire). Otherwise toggle it to false and
    ///    the recovery handler will restart the `start_tick` of all
    ///    producers.
    pub fn compute(_region: &FactoryRegion) -> Self {
        Self::default()
    }

    pub fn has_deficit(&self) -> bool {
        self.total_consumption > self.total_generation + self.total_stored
    }
}

/// Result of computing fuel state for a `BurnPower` component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BurnPowerComputed {
    pub has_fuel: bool,
    pub remaining_ticks: u64,
}

/// Math regarding fuel consumption, from §4.5.
///
/// `burn_speed` is the energy-per-tick consumption rate of the station. `fuel_energy`
/// is the energy contained in each item (for example, `item_originium_ore` =
/// 6_000). With `burn_speed = 125`, an ore will burn for 6_000 / 125 = 48 ticks.
// TODO(Clause 4): once `PowerStationEntry` is accessible via `perlica-logic`,
// change the type to `&PowerStationEntry` to prevent outdated constant passing
pub fn compute_fuel_state(burn: &BurnPowerState, burn_speed: u64) -> BurnPowerComputed {
    let Some(start) = burn.fuel_start_tick else {
            return BurnPowerComputed {
                has_fuel: false,
                remaining_ticks: 0,
            };
    };
    let speed = burn_speed.max(1) as i64;
    let elapsed = elapsed_since(start) as i64;
    let consumed = elapsed.saturating_mul(speed);
    let remaining = burn.fuel_remaining.saturating_sub(consumed);
    BurnPowerComputed {
        has_fuel: remaining > 0,
        remaining_ticks: (remaining.max(0) as u64) / (speed as u64),
    }
}

/// Crafting speed with skill modifiers applied, according to §4.3.
///
/// `skill_modifiers` is the fractional additive bonus per applicable skill
/// (like `-0.2` for "20% faster"). They are added up, bounded to be
/// at least 0.01 to prevent a combination of penalties from reducing
/// the speed multiplier to 0 or less, and then floored to an integer.
pub fn effective_speed(base_speed: u64, skill_modifiers: &[f64]) -> u64 {
    let total_mod: f64 = skill_modifiers.iter().copied().sum();
    let multiplier = (1.0 + total_mod).max(0.01);
    let effective = base_speed as f64 * multiplier;
    effective.max(1.0) as u64
}

/// Reset all producers/collectors in that region after a power outage, according to §4.4.
///
/// TODO(Clause 4): call this from the `is_stop_by_power` transition function
/// once the `PowerGraph::compute` function fills in `powered_nodes`. Right now,
/// this is just a stub.
pub fn handle_power_loss(_region: &mut FactoryRegion) {
    // When implemented:
    //   for node in region.nodes.values_mut() {
    //       for (_, comp) in &mut node.components {
    //           match comp {
    //               FactoryComponent::Producer(s) | FactoryComponent::Collector(s) => {
    //                   s.start_tick = None;
    //                   s.current_progress = 0;   /// this should be paused where it stopped instead of being 0 so it doesn't reset the current progress! mb
    //                   s.in_power = false;
    //               }
    //               FactoryComponent::PowerPole(s) => s.in_power = false,
    //               FactoryComponent::PowerSave(s) => s.in_power = false,
    //               FactoryComponent::StablePower(s) => s.in_power = false,
    //               _ => {}
    //           }
    //       }
    //   }
    //   region.blackboard.is_stop_by_power = true;
}

/// Restart every producer/collector after power returns, per §4.4.
///
/// TODO(Clause 4): same as `handle_power_loss`, wire when graph lands.
pub fn handle_power_recovery(_region: &mut FactoryRegion, _now: Tick) {
    // When implemented:
    //   for node in region.nodes.values_mut() {
    //       for (_, comp) in &mut node.components {
    //           match comp {
    //               FactoryComponent::Producer(s) => {
    //                   s.in_power = true;
    //                   if !s.formula_id.is_empty() {
    //                       s.start_tick = Some(now);
    //                   }
    //               }
    //               FactoryComponent::Collector(s) => {
    //                   s.in_power = true;
    //                   s.start_tick = Some(now);
    //               }
    //               FactoryComponent::PowerPole(s)
    //               | FactoryComponent::PowerSave(s)
    //               | FactoryComponent::StablePower(s) => s.in_power = true,
    //               _ => {}
    //           }
    //       }
    //   }
    //   region.blackboard.is_stop_by_power = false;
}

/// Recalculate the totals for the [`PowerBlackboard`] after there has been a
/// structural modification of the region (node inserted / removed / link inserted /
/// removed).
///
/// TODO(Clause 4): get real values from `PowerGraph::compute(region)`. For now,
/// we simply update `is_stop_by_power` depending on the deficit check.
pub fn recompute_blackboard(region: &mut FactoryRegion) {
    let graph = PowerGraph::compute(region);
    region.blackboard.power_gen = graph.total_generation;
    region.blackboard.power_cost = graph.total_consumption;
    region.blackboard.power_save_max = graph.total_storage;
    region.blackboard.power_save_current = graph.total_stored;
    region.blackboard.is_stop_by_power = graph.has_deficit();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::factory::{BurnPowerState, FactoryRegion, current_tick};

    #[test]
    fn compute_returns_default_stub() {
        let region = FactoryRegion::new("test01", "region_102", "map01_lv001", 1);
        let g = PowerGraph::compute(&region);
        assert!(g.powered_nodes.is_empty());
        assert_eq!(g.total_generation, 0);
        assert!(!g.has_deficit());
    }

    #[test]
    fn effective_speed_clamps_negative_modifiers() {
        // -50% + -60% = -110% multiplier -> clamped to 0.01 -> floor 1
        assert_eq!(effective_speed(100, &[-0.5, -0.6]), 1);
        // -20% modifier on 250 -> 200
        assert_eq!(effective_speed(250, &[-0.2]), 200);
        // no modifiers -> unchanged
        assert_eq!(effective_speed(100, &[]), 100);
        // +30% on 100 -> 130
        assert_eq!(effective_speed(100, &[0.3]), 130);
    }

    #[test]
    fn fuel_state_no_fuel_when_start_is_none() {
        let burn = BurnPowerState {
            fuel_remaining: 6_000,
            fuel_start_tick: None,
            in_power: true,
        };
        let c = compute_fuel_state(&burn, 125);
        assert!(!c.has_fuel);
        assert_eq!(c.remaining_ticks, 0);
    }

    #[test]
    fn fuel_state_depletes_over_time() {
        // 6_000 fuel, 125 per tick burn rate => fuel time is 48 ticks.
        // Started 10 ticks back: burned = 10 * 125 = 1_250, left = 4_750,
        // left_ticks = 4_750 / 125 = 38. (Real time calculation means
        // real ticks passed may be 10 or 11 ticks, so accept 37 or 38)
        let start = current_tick().saturating_sub(10);
        let burn = BurnPowerState {
            fuel_remaining: 6_000,
            fuel_start_tick: Some(start),
            in_power: true,
        };
        let c = compute_fuel_state(&burn, 125);
        assert!(c.has_fuel, "fuel should still be burning after ~10 ticks");
        assert!(
            matches!(c.remaining_ticks, 37 | 38),
            "remaining_ticks should be 37 or 38 (got {})",
            c.remaining_ticks
        );
    }

    #[test]
    fn fuel_state_depleted_after_burn_duration() {
        // 6_000 / 125 = 48 ticks of fuel. Start 100 ticks ago -> fuel gone.
        let start = current_tick().saturating_sub(100);
        let burn = BurnPowerState {
            fuel_remaining: 6_000,
            fuel_start_tick: Some(start),
            in_power: true,
        };
        let c = compute_fuel_state(&burn, 125);
        assert!(!c.has_fuel, "fuel should be gone after 100 ticks");
        assert_eq!(c.remaining_ticks, 0);
    }

    #[test]
    fn recompute_blackboard_is_safe_on_empty_region() {
        let mut region = FactoryRegion::new("test01", "region_102", "map01_lv001", 1);
        recompute_blackboard(&mut region);
        assert!(!region.blackboard.is_stop_by_power);
    }
}
