//! Builders for the server-push `ScFactoryModify*` notifications.
//!
//! After most op handlers mutate the internal model they need to push a
//! delta to the client so the UI matches. The three relevant messages
//! are `ScFactoryModifyRegionNodes` (added/changed/removed nodes),
//! `ScFactoryModifyRegionComponents` (changed component state on existing
//! nodes), and `ScFactoryModifyRegionScene` (mesh/connection changes).
//!
//! TODO(Clause 3): everything here currently builds empty proto structs
//! because the `FactoryNode -> ScdFactorySyncNode` conversion lives in
//! task 3.2. The helpers are written but not yet called from any op
//! handler -- once `to_proto()` lands, every successful op should fire
//! the matching `modify_*` after mutating state. Until then we'd just
//! be sending empty deltas, so callers are intentionally absent.

#![allow(dead_code)]

use perlica_proto::{
    ScFactoryModifyRegionComponents, ScFactoryModifyRegionNodes, ScFactoryModifyRegionScene,
};

/// "These nodes were added or changed". Pass the wire region name (`test01`).
pub fn modify_nodes(region_name: &str, _node_ids: &[u32]) -> ScFactoryModifyRegionNodes {
    ScFactoryModifyRegionNodes {
        tms: 0,
        name: region_name.to_string(),
        // TODO(Clause 3.2): look up each node in the region and serialize
        // via `node.to_proto()`. Until then, leave empty -- the client
        // will refresh on next reconnect.
        nodes: vec![],
        remove_nodes: vec![],
    }
}

/// "These nodes were removed". Same outer message as `modify_nodes` --
/// it's the same proto, just with the `remove_nodes` field populated.
pub fn remove_nodes(region_name: &str, node_ids: &[u32]) -> ScFactoryModifyRegionNodes {
    ScFactoryModifyRegionNodes {
        tms: 0,
        name: region_name.to_string(),
        nodes: vec![],
        remove_nodes: node_ids.to_vec(),
    }
}

/// "These components changed state on their existing nodes". Used by ops
/// that mutate component state without touching the node list (e.g.
/// EnableNode, SetSelectTarget, CacheTransportEnable).
pub fn modify_components(
    region_name: &str,
    _component_ids: &[(u32, u32)],
) -> ScFactoryModifyRegionComponents {
    ScFactoryModifyRegionComponents {
        tms: 0,
        name: region_name.to_string(),
        // TODO(Clause 3.1): serialize each (node_id, component_id) pair
        // via `component.to_proto()`. Empty for now, same reason as above.
        components: vec![],
    }
}

/// "Scene-level change" -- mesh grew (region upgraded), connections
/// added/removed, or bandwidth changed. Only `op::connection` and the
/// region-upgrade path use this today.
pub fn modify_scene(region_name: &str) -> ScFactoryModifyRegionScene {
    ScFactoryModifyRegionScene {
        tms: 0,
        name: region_name.to_string(),
        scene_name: String::new(),
        level: 0,
        main_mesh: vec![],
        connections: vec![],
        remove_connections: vec![],
        bandwidth: None,
    }
}
