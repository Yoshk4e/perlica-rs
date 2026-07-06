//! Power information for each region.
//!
//! This is the "current totals" representation -- the real state of each node's power is held by the component itself
//! (PowerPole/BurnPower/etc). The `compute()` function in `PowerGraph` (mod.rs) is responsible for recalculating all of these values.

/// Power/energy information for a particular `FactoryRegion`. Recalculated each time
/// the power network changes (a node is added/removed, a link is added/removed,
/// fuel runs out, etc.) – See §4.4 of the implementation plan for reset semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerBlackboard {
    pub inventory_node_id: u32,
    pub power_cost: i64,
    pub power_gen: i64,
    pub power_save_max: i64,
    pub power_save_current: i64,
    /// true once cost > gen + stored. flips all producers/collectors to
    /// paused (start_tick = None) until power recovers -- see power.rs (TODO, Clause 4)
    pub is_stop_by_power: bool,
}

impl Default for PowerBlackboard {
    fn default() -> Self {
        Self {
            inventory_node_id: 1,
            power_cost: 0,
            power_gen: 0,
            power_save_max: 0,
            power_save_current: 0,
            is_stop_by_power: false,
        }
    }
}

impl PowerBlackboard {
    pub fn new(inventory_node_id: u32) -> Self {
        Self {
            inventory_node_id,
            ..Self::default()
        }
    }

    /// Convenience for the hub bootstrap: hub provides passive gen + starts
    /// with a full battery. Doesn't touch `power_cost`, nothing's consuming yet.
    pub fn with_hub_power(inventory_node_id: u32, power_gen: i64, power_save_max: i64) -> Self {
        Self {
            inventory_node_id,
            power_cost: 0,
            power_gen,
            power_save_max,
            power_save_current: power_save_max,
            is_stop_by_power: false,
        }
    }

    /// gen + whatever's left in the battery vs what's being drawn right now.
    /// doesn't mutate anything -- caller decides what to do with the result.
    pub fn has_power_deficit(&self) -> bool {
        self.power_cost > self.power_gen + self.power_save_current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_unpowered_empty_region() {
        let bb = PowerBlackboard::default();
        assert_eq!(bb.inventory_node_id, 1);
        assert_eq!(bb.power_cost, 0);
        assert_eq!(bb.power_gen, 0);
        assert!(!bb.is_stop_by_power);
    }

    #[test]
    fn with_hub_power_fills_battery() {
        let bb = PowerBlackboard::with_hub_power(1, 100, 100_000);
        assert_eq!(bb.power_gen, 100);
        assert_eq!(bb.power_save_max, 100_000);
        assert_eq!(bb.power_save_current, 100_000);
        assert!(!bb.has_power_deficit());
    }

    #[test]
    fn deficit_detected_when_cost_exceeds_gen_and_storage() {
        let mut bb = PowerBlackboard::with_hub_power(1, 100, 50);
        bb.power_cost = 200;
        assert!(bb.has_power_deficit());
    }
}
