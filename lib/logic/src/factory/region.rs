use std::collections::HashMap;

use super::{FactoryComponent, FactoryNode, PowerBlackboard};
use crate::enums::{FCConnectionPortType, FCConnectionType};
use crate::factory::component::ProducerState;
use crate::factory::tick::elapsed_since;

/// A power or travel link between two nodes.
///
/// `index` is a stable identifier used by the dismantle-connection handler
/// (task 2.6), `(node_id_a, node_id_b)` isn't unique enough because the
/// same pair can carry both a Power and a Travel link.
#[derive(Debug, Clone)]
pub struct FactoryConnection {
    pub connection_type: FCConnectionType,
    pub port_type: FCConnectionPortType,
    pub node_id_a: u32,
    pub node_id_b: u32,
    pub index: u64,
}

/// One factory region (one per scene / map level).
///
/// The `name` field contains the wire name that will be sent to the client,
/// which is always `"test01"` at present as this is the only value that
/// the alpha client supports (§1.3 impl plan). `region_id` is the true
/// ID used for database records and config lookups.
#[derive(Debug, Clone)]
pub struct FactoryRegion {
    pub name: String,
    pub region_id: String,
    pub scene_name: String,
    /// 1, 2, or 3, the expansion level of this region's `FactoryMapTable`
    /// entry. Higher levels unlock more grid space.
    pub level: u32,
    /// Sequential allocator for `FactoryNode::node_id`. Starts at 3 because
    /// 1 (inventory) and 2 (hub) are reserved by `bootstrap_region`.
    pub next_node_id: u32,
    pub blackboard: PowerBlackboard,
    pub nodes: HashMap<u32, FactoryNode>,
    pub connections: Vec<FactoryConnection>,
    /// Lifetime production totals: `item_id -> total produced`. Used by the
    /// STT condition evaluator (Appendix B, type 511).
    pub production_totals: HashMap<String, u64>,
}

impl FactoryRegion {
    pub fn new(
        name: impl Into<String>,
        region_id: impl Into<String>,
        scene_name: impl Into<String>,
        level: u32,
    ) -> Self {
        Self {
            name: name.into(),
            region_id: region_id.into(),
            scene_name: scene_name.into(),
            level,
            next_node_id: 3,
            blackboard: PowerBlackboard::default(),
            nodes: HashMap::new(),
            connections: Vec::new(),
            production_totals: HashMap::new(),
        }
    }

    pub fn allocate_node_id(&mut self) -> u32 {
        let id = self.next_node_id;
        self.next_node_id += 1;
        id
    }

    /// Count placed buildings by `template_id`, backs STT condition 504
    /// ("have N of building X placed", see Appendix B).
    pub fn count_buildings_by_template(&self, template_id: &str) -> usize {
        self.nodes
            .values()
            .filter(|n| n.template_id == template_id)
            .count()
    }

    /// Calculate the current progress for a `Producer` component.
    ///
    /// §4.1: `current_progress + elapsed * speed`, if the producer is active,
    /// that is, its `start_tick` is `Some`, otherwise just `current_progress`.
    /// Both saturated from below and above to avoid underflow from clock skew.
    pub fn compute_producer_progress(&self, state: &ProducerState, speed: u64) -> u64 {
        match state.start_tick {
            Some(start) => {
                let elapsed = elapsed_since(start);
                state
                    .current_progress
                    .saturating_add(elapsed.saturating_mul(speed))
            }
            None => state.current_progress,
        }
    }

    pub fn node(&self, node_id: u32) -> Option<&FactoryNode> {
        self.nodes.get(&node_id)
    }

    pub fn node_mut(&mut self, node_id: u32) -> Option<&mut FactoryNode> {
        self.nodes.get_mut(&node_id)
    }

    /// Get all node IDs that contain at least one instance of the provided
    /// discriminator (e.g., nodes that have a `Producer` discriminator).
    ///
    /// Used by the Clause 4 power graph BFS to identify sources/relays/consumers
    /// without rewalking all discriminators at each site.
    pub fn node_ids_with_component(
        &self,
        predicate: impl Fn(&FactoryComponent) -> bool,
    ) -> Vec<u32> {
        self.nodes
            .iter()
            .filter_map(|(id, n)| {
                n.components
                    .iter()
                    .any(|(_, c)| predicate(c))
                    .then_some(*id)
            })
            .collect()
    }

    /// Borrow the next stable connection index (used when adding a new
    /// connection so the dismantle handler has a stable identifier).
    pub fn next_connection_index(&self) -> u64 {
        self.connections
            .iter()
            .map(|c| c.index)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    }

    // TODO(Clause 3.3): `to_proto(&self) -> ScdFactorySyncRegion`, a complete
    // conversion that matches the format used by the server. The `nodes`
    // should be serialized in the ascending order of `node_id`, while
    // `scenes` are emitted based on `FactoryMapTable`.

    // TODO (Clause 4): The power loss/power recovery routines defined in
    // `power.rs` require iterating through each node in the region and modifying
    // the producer/collector parts; they accept `&mut FactoryRegion`.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enums::{FCConnectionPortType, FCConnectionType, FCDirection, FCNodeType};
    use crate::factory::component::{FactoryComponent, PowerPoleState, ProducerState};
    use crate::factory::node::{FactoryNode, NodeTransform};
    use crate::factory::tick::current_tick;
    use std::collections::HashMap;

    #[test]
    fn new_initializes_reserved_ids() {
        let r = FactoryRegion::new("test01", "region_102", "map01_lv001", 1);
        assert_eq!(r.next_node_id, 3, "ids 1 (inv) and 2 (hub) reserved");
        assert_eq!(r.name, "test01");
        assert_eq!(r.region_id, "region_102");
        assert!(r.nodes.is_empty());
        assert!(r.connections.is_empty());
    }

    #[test]
    fn allocate_node_id_is_sequential() {
        let mut r = FactoryRegion::new("test01", "region_102", "map01_lv001", 1);
        assert_eq!(r.allocate_node_id(), 3);
        assert_eq!(r.allocate_node_id(), 4);
        assert_eq!(r.allocate_node_id(), 5);
        assert_eq!(r.next_node_id, 6);
    }

    #[test]
    fn count_buildings_by_template() {
        let mut r = FactoryRegion::new("test01", "region_102", "map01_lv001", 1);
        let mk = |id, tid: &str| FactoryNode {
            node_id: id,
            node_type: FCNodeType::PowerPole,
            template_id: tid.to_string(),
            transform: NodeTransform {
                position: None,
                direction: FCDirection::Up,
                mesh: None,
                scene_name: "map01_lv001".to_string(),
                world_position: None,
                world_rotation: None,
                bc_port_in: None,
                bc_port_out: None,
            },
            is_deactive: false,
            interactive_object: None,
            dynamic_property: HashMap::new(),
            component_pos: HashMap::new(),
            components: vec![(
                1,
                FactoryComponent::PowerPole(PowerPoleState { in_power: true }),
            )],
        };
        r.nodes.insert(3, mk(3, "power_pole_1"));
        r.nodes.insert(4, mk(4, "power_pole_1"));
        r.nodes.insert(5, mk(5, "power_pole_2"));
        assert_eq!(r.count_buildings_by_template("power_pole_1"), 2);
        assert_eq!(r.count_buildings_by_template("power_pole_2"), 1);
        assert_eq!(r.count_buildings_by_template("power_pole_3"), 0);
    }

    #[test]
    fn compute_producer_progress_uses_elapsed_times_speed() {
        let r = FactoryRegion::new("test01", "region_102", "map01_lv001", 1);
        let speed = 100u64;
        let start = current_tick().saturating_sub(10);

        let running = ProducerState {
            formula_id: "mc_iron_1".to_string(),
            start_tick: Some(start),
            current_progress: 50,
            in_power: true,
            in_block: false,
            power_cost: 10,
            last_formula_id: String::new(),
        };
        let p = r.compute_producer_progress(&running, speed);
        // 50 + (>=10 * 100) = at least 1050
        assert!(p >= 1050);

        let paused = ProducerState {
            start_tick: None,
            ..running
        };
        assert_eq!(r.compute_producer_progress(&paused, speed), 50);
    }

    #[test]
    fn next_connection_index_starts_at_one_and_grows() {
        let mut r = FactoryRegion::new("test01", "region_102", "map01_lv001", 1);
        assert_eq!(r.next_connection_index(), 1);
        r.connections.push(FactoryConnection {
            connection_type: FCConnectionType::Power,
            port_type: FCConnectionPortType::Hub,
            node_id_a: 2,
            node_id_b: 4,
            index: 1,
        });
        assert_eq!(r.next_connection_index(), 2);
    }
}
