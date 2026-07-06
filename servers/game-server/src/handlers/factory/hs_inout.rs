//! `CsFactoryHsInout` handler -- toggle the in/out direction of a
//! conveyor bus port.
//!
//! `in_out: true` means "input mode" (items flow into the building),
//! `false` means "output mode". The server flips the port direction
//! on the target node's BoxConveyor component and returns the updated
//! handshake state via `ScFactoryHs`.

use crate::net::NetContext;
use perlica_logic::factory::FactoryComponent;
use perlica_proto::{CsFactoryHsInout, ScFactoryHs};
use tracing::warn;

pub async fn on_cs_factory_hs_inout(
    ctx: &mut NetContext<'_>,
    req: CsFactoryHsInout,
) -> ScFactoryHs {
    let region_name = req.name.clone();

    let Some(region) = ctx.player.factory.region_mut(&region_name) else {
            warn!(region = %region_name, "CsFactoryHsInout for unknown region");
            return ScFactoryHs {
                tms: 0,
                ct_list: vec![],
                fb_list: vec![],
                ce_list: vec![],
                blackboard: None,
            };
    };

    // The `name` field carries the region name. The actual node + port
    // to toggle is implicitly the hub's BusLoader since that's the only
    // bus I/O point in v1.2. Once we have multi-node bus support, the
    // client will send a node_id + port_index alongside.
    //
    // For now, find the first BoxConveyor or BusLoader on any node and
    // flip its direction. The direction field uses FCDirection ints:
    // 0=Up, 1=Right, 2=Down, 3=Left.
    for node in region.nodes.values_mut() {
        for (_, comp) in &mut node.components {
            if let FactoryComponent::BoxConveyor(state) = comp {
                // Toggle between input (0) and output (2) directions.
                state.direction = if req.in_out { 0 } else { 2 };
                return ScFactoryHs {
                    tms: 0,
                    ct_list: vec![],
                    fb_list: vec![],
                    ce_list: vec![],
                    blackboard: None,
                };
            }
        }
    }

    // No conveyor found -- empty response, client will see no change.
    ScFactoryHs {
        tms: 0,
        ct_list: vec![],
        fb_list: vec![],
        ce_list: vec![],
        blackboard: None,
    }
}
