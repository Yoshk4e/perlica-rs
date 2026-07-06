//! Power graph + fuel / storage math.
//!
//! The power network is a graph where `StablePower` and `BurnPower`
//! components are sources, `PowerPole` components are relays, and
//! `FactoryConnection { connection_type: Power }` edges wire them
//! together. A node is "powered" if it's reachable from a source via
//! power edges (or is itself a source).
//!
//! When total consumption exceeds generation + stored energy, every
//! producer and collector in the region gets paused (`start_tick =
//! None`, progress kept). When power returns, they restart from where
//! they left off.

use std::collections::{HashSet, VecDeque};

use super::{BurnPowerState, FactoryComponent, FactoryRegion, Tick};
use crate::enums::FCConnectionType;
use crate::factory::tick::{current_tick, elapsed_since};

/// Computed power graph. Not persisted -- recomputed on demand whenever
/// the network changes (node placed/dismantled, connection added/removed,
/// fuel runs out, etc.).
#[derive(Debug, Clone, Default)]
pub struct PowerGraph {
    pub powered_nodes: HashSet<u32>,
    pub total_generation: i64,
    pub total_consumption: i64,
    pub total_storage: i64,
    pub total_stored: i64,
}

impl PowerGraph {
    /// BFS from every power source through pole connections, marking
    /// reachable nodes as powered. Sums generation / consumption / storage
    /// across the powered set.
    ///
    /// Nodes that aren't reachable from any source are unpowered: their
    /// `in_power` flags get set to `false` by `recompute_blackboard`
    /// after this returns.
    pub fn compute(region: &FactoryRegion) -> Self {
        let mut graph = Self::default();

        // Find every node that carries a power-source component
        // (StablePower or BurnPower with fuel). These are the BFS roots.
        let mut sources: Vec<u32> = Vec::new();
        for (&node_id, node) in &region.nodes {
            for (_, comp) in &node.components {
                match comp {
                    FactoryComponent::StablePower(s) if s.in_power => {
                        graph.total_generation += s.power_gen_per_sec;
                        sources.push(node_id);
                    }
                    FactoryComponent::BurnPower(s) if s.in_power && has_fuel(s) => {
                        graph.total_generation += s.power_gen_per_sec;
                        sources.push(node_id);
                    }
                    _ => {}
                }
            }
        }

        // Build an adjacency list from power connections. Each connection
        // links node_id_a <-> node_id_b bidirectionally.
        let mut adjacency: HashMap<u32, Vec<u32>> = HashMap::new();
        for conn in &region.connections {
            if conn.connection_type != FCConnectionType::Power {
                continue;
            }
            adjacency
                .entry(conn.node_id_a)
                .or_default()
                .push(conn.node_id_b);
            adjacency
                .entry(conn.node_id_b)
                .or_default()
                .push(conn.node_id_a);
        }

        // BFS from sources. Every node we reach is powered. PowerPole
        // components act as relays -- they extend the BFS frontier. Other
        // components (Producer, Collector, etc.) are consumers that get
        // powered if reached but don't extend the frontier themselves.
        let mut visited = HashSet::new();
        let mut queue: VecDeque<u32> = sources.iter().copied().collect();
        for &s in &sources {
            visited.insert(s);
        }

        while let Some(node_id) = queue.pop_front() {
            graph.powered_nodes.insert(node_id);

            let Some(node) = region.nodes.get(&node_id) else {
                continue;
            };

            // Sum up consumption + storage for this node's powered components.
            for (_, comp) in &node.components {
                match comp {
                    FactoryComponent::Producer(s) => {
                        graph.total_consumption += s.power_cost;
                    }
                    FactoryComponent::Collector(s) => {
                        graph.total_consumption += s.power_cost;
                    }
                    FactoryComponent::PowerSave(s) => {
                        graph.total_storage += 0; // capacity isn't on the state yet
                        graph.total_stored += s.power_save;
                    }
                    FactoryComponent::BurnPower(s) => {
                        // BurnPower is both source and consumer of fuel;
                        // its power_cost is implicit (it generates what it
                        // generates). Already counted as generation above.
                        let _ = s;
                    }
                    _ => {}
                }
            }

            // Extend the frontier through power connections. Every node
            // that's wired to this one (directly or via a pole chain)
            // becomes reachable.
            if let Some(neighbors) = adjacency.get(&node_id) {
                for &neighbor_id in neighbors {
                    if visited.insert(neighbor_id) {
                        queue.push_back(neighbor_id);
                    }
                }
            }
        }

        graph
    }

    pub fn has_deficit(&self) -> bool {
        self.total_consumption > self.total_generation + self.total_stored
    }
}

/// Whether a BurnPower component currently has fuel to burn.
/// A station with no `fuel_start_tick` or zero `fuel_remaining` is dark.
fn has_fuel(burn: &BurnPowerState) -> bool {
    burn.fuel_remaining > 0
        && burn.fuel_start_tick.is_some_and(|start| {
            let elapsed = elapsed_since(start) as i64;
            burn.fuel_remaining > elapsed
        })
}

/// Result of computing fuel state for a `BurnPower` component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BurnPowerComputed {
    pub has_fuel: bool,
    pub remaining_ticks: u64,
}

/// Fuel consumption math from §4.5.
///
/// `burn_speed` is energy-per-tick consumed by the station. `fuel_energy`
/// is the per-item energy value (e.g. `item_originium_ore` = 6_000).
/// At `burn_speed = 125`, one ore burns for 6_000 / 125 = 48 ticks.
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

/// Crafting speed with skill modifiers applied, per §4.3.
pub fn effective_speed(base_speed: u64, skill_modifiers: &[f64]) -> u64 {
    let total_mod: f64 = skill_modifiers.iter().copied().sum();
    let multiplier = (1.0 + total_mod).max(0.01);
    let effective = base_speed as f64 * multiplier;
    effective.max(1.0) as u64
}

/// Reset all producers/collectors after a power loss, per §4.4.
///
/// Producers keep their `current_progress` (so they resume from where
/// they stopped) but get `start_tick = None` so the timer freezes.
/// `in_power` flips to false on every power-related component.
pub fn handle_power_loss(region: &mut FactoryRegion) {
    for node in region.nodes.values_mut() {
        for (_, comp) in &mut node.components {
            match comp {
                FactoryComponent::Producer(s) => {
                    // Snapshot progress so the producer resumes from here
                    // when power comes back, then freeze the timer.
                    if let Some(start) = s.start_tick.take() {
                        let elapsed = elapsed_since(start);
                        s.current_progress = s.current_progress.saturating_add(elapsed);
                    }
                    s.in_power = false;
                }
                FactoryComponent::Collector(s) => {
                    if let Some(start) = s.start_tick.take() {
                        let elapsed = elapsed_since(start);
                        s.current_progress = s.current_progress.saturating_add(elapsed);
                    }
                    s.in_power = false;
                }
                FactoryComponent::PowerPole(s) => s.in_power = false,
                FactoryComponent::PowerSave(s) => s.in_power = false,
                FactoryComponent::StablePower(s) => s.in_power = false,
                FactoryComponent::BurnPower(s) => s.in_power = false,
                FactoryComponent::HealTower(s) => s.in_power = false,
                FactoryComponent::CacheTransport(s) => s.in_power = false,
                _ => {}
            }
        }
    }
    region.blackboard.is_stop_by_power = true;
}

/// Restart every producer/collector after power returns, per §4.4.
///
/// Producers with an active `formula_id` get `start_tick = Some(now)` so
/// the timer resumes. `in_power` flips back to true on every power-related
/// component. Collectors always restart (they don't need a formula).
pub fn handle_power_recovery(region: &mut FactoryRegion, now: Tick) {
    for node in region.nodes.values_mut() {
        for (_, comp) in &mut node.components {
            match comp {
                FactoryComponent::Producer(s) => {
                    s.in_power = true;
                    if !s.formula_id.is_empty() {
                        s.start_tick = Some(now);
                    }
                }
                FactoryComponent::Collector(s) => {
                    s.in_power = true;
                    s.start_tick = Some(now);
                }
                FactoryComponent::PowerPole(s) => s.in_power = true,
                FactoryComponent::PowerSave(s) => s.in_power = true,
                FactoryComponent::StablePower(s) => s.in_power = true,
                FactoryComponent::BurnPower(s) => s.in_power = true,
                FactoryComponent::HealTower(s) => s.in_power = true,
                FactoryComponent::CacheTransport(s) => s.in_power = true,
                _ => {}
            }
        }
    }
    region.blackboard.is_stop_by_power = false;
}

/// Recompute the power graph and update the blackboard. If the deficit
/// state changed (power lost or recovered), fire the matching transition
/// handler so producers get paused or resumed.
pub fn recompute_blackboard(region: &mut FactoryRegion) {
    let graph = PowerGraph::compute(region);
    let was_stop = region.blackboard.is_stop_by_power;
    let is_stop = graph.has_deficit();

    region.blackboard.power_gen = graph.total_generation;
    region.blackboard.power_cost = graph.total_consumption;
    region.blackboard.power_save_max = graph.total_storage;
    region.blackboard.power_save_current = graph.total_stored;

    if is_stop && !was_stop {
        handle_power_loss(region);
    } else if !is_stop && was_stop {
        handle_power_recovery(region, current_tick());
    } else {
        region.blackboard.is_stop_by_power = is_stop;
    }
}

use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::factory::{BurnPowerState, FactoryRegion, current_tick};

    #[test]
    fn compute_returns_default_on_empty_region() {
        let region = FactoryRegion::new("test01", "region_102", "map01_lv001", 1);
        let g = PowerGraph::compute(&region);
        assert!(g.powered_nodes.is_empty());
        assert_eq!(g.total_generation, 0);
        assert!(!g.has_deficit());
    }

    #[test]
    fn effective_speed_clamps_negative_modifiers() {
        assert_eq!(effective_speed(100, &[-0.5, -0.6]), 1);
        assert_eq!(effective_speed(250, &[-0.2]), 200);
        assert_eq!(effective_speed(100, &[]), 100);
        assert_eq!(effective_speed(100, &[0.3]), 130);
    }

    #[test]
    fn fuel_state_no_fuel_when_start_is_none() {
        let burn = BurnPowerState {
            fuel_remaining: 6_000,
            fuel_start_tick: None,
            in_power: true,
            power_gen_per_sec: 100,
            current_burn_item_id: String::new(),
        };
        let c = compute_fuel_state(&burn, 125);
        assert!(!c.has_fuel);
        assert_eq!(c.remaining_ticks, 0);
    }

    #[test]
    fn fuel_state_depletes_over_time() {
        let start = current_tick().saturating_sub(10);
        let burn = BurnPowerState {
            fuel_remaining: 6_000,
            fuel_start_tick: Some(start),
            in_power: true,
            power_gen_per_sec: 100,
            current_burn_item_id: String::new(),
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
        let start = current_tick().saturating_sub(100);
        let burn = BurnPowerState {
            fuel_remaining: 6_000,
            fuel_start_tick: Some(start),
            in_power: true,
            power_gen_per_sec: 100,
            current_burn_item_id: String::new(),
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
