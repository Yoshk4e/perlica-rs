//! Builders for the server-push `ScFactoryModify*` notifications.
//!
//! After most op handlers mutate the internal model they need to push a
//! delta to the client so the UI matches. These helpers use the existing
//! `FactoryNode::to_proto()` and `FactoryComponent::to_proto()` methods
//! to serialize real state into the notification proto.

#![allow(dead_code)]

use perlica_logic::factory::FactoryManager;
use perlica_proto::{
    ScFactoryModifyRegionComponents, ScFactoryModifyRegionNodes, ScFactoryModifyRegionScene,
    ScdFactorySyncComponent,
};

/// "These nodes were added or changed". Looks up each node by ID in the
/// region and serializes it via `to_proto()`.
pub fn modify_nodes(
    manager: &FactoryManager,
    region_name: &str,
    node_ids: &[u32],
) -> ScFactoryModifyRegionNodes {
    let mut nodes = vec![];
    if let Some(region) = manager.region(region_name) {
        for &id in node_ids {
            if let Some(node) = region.node(id) {
                nodes.push(node.to_proto());
            }
        }
    }

    ScFactoryModifyRegionNodes {
        tms: perlica_logic::factory::current_tick() as i64,
        name: region_name.to_string(),
        nodes,
        remove_nodes: vec![],
    }
}

/// "These nodes were removed". Same outer message as `modify_nodes` --
/// it's the same proto, just with the `remove_nodes` field populated.
pub fn remove_nodes(region_name: &str, node_ids: &[u32]) -> ScFactoryModifyRegionNodes {
    ScFactoryModifyRegionNodes {
        tms: perlica_logic::factory::current_tick() as i64,
        name: region_name.to_string(),
        nodes: vec![],
        remove_nodes: node_ids.to_vec(),
    }
}

/// "These components changed state on their existing nodes". Looks up
/// each `(node_id, component_id)` pair in the region and serializes the
/// component via `to_proto()`.
pub fn modify_components(
    manager: &FactoryManager,
    region_name: &str,
    component_ids: &[(u32, u32)],
) -> ScFactoryModifyRegionComponents {
    let mut components: Vec<ScdFactorySyncComponent> = vec![];
    if let Some(region) = manager.region(region_name) {
        for &(node_id, comp_id) in component_ids {
            if let Some(node) = region.node(node_id)
                && let Some(comp) = node.component(comp_id)
            {
                components.push(comp.to_proto(comp_id));
            }
        }
    }

    ScFactoryModifyRegionComponents {
        tms: perlica_logic::factory::current_tick() as i64,
        name: region_name.to_string(),
        components,
    }
}

/// "Scene-level change" -- mesh grew (region upgraded), connections
/// added/removed, or bandwidth changed. Only `op::connection` and the
/// region-upgrade path use this today.
pub fn modify_scene(region_name: &str) -> ScFactoryModifyRegionScene {
    ScFactoryModifyRegionScene {
        tms: perlica_logic::factory::current_tick() as i64,
        name: region_name.to_string(),
        scene_name: String::new(),
        level: 0,
        main_mesh: vec![],
        connections: vec![],
        remove_connections: vec![],
        bandwidth: None,
    }
}
