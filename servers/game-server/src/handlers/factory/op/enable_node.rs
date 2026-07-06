//! `EnableNode` op. Flips the `is_deactive` flag on a placed node.
//!
//! Disabling a node should pause its producers (their `start_tick` gets
//! cleared so progress freezes), but the actual pause wiring lives in
//! the per-component logic and isn't fully hooked up yet -- see the TODO
//! in `handle`.

use crate::net::NetContext;
use perlica_logic::factory::FactoryComponent;
use perlica_proto::{CsdFactoryOpEnableNode, FactoryOpType, ScFactoryOpRet};

use super::super::response;

pub async fn handle(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpEnableNode,
) -> ScFactoryOpRet {
    let region = match ctx.player.factory.region_mut(&region_name) {
        Some(r) => r,
        None => {
            return response::fail(
                index,
                FactoryOpType::EnableNode,
                perlica_proto::FactoryOpRetCode::Fail,
                format!("region {} not found", region_name),
            );
        }
    };

    let node = match region.node_mut(req.node_id) {
        Some(n) => n,
        None => {
            return response::fail(
                index,
                FactoryOpType::EnableNode,
                perlica_proto::FactoryOpRetCode::Fail,
                format!("node {} not found", req.node_id),
            );
        }
    };

    node.is_deactive = !req.enable;

    // TODO: when a producer is paused via disable, we need to snapshot
    // its current progress into `current_progress` and clear `start_tick`
    // so the timer doesn't keep running while disabled. Reverse on
    // re-enable. Lives in the component layer once that lands.
    for (_, comp) in &mut node.components {
        if let FactoryComponent::Producer(state) = comp {
            if !req.enable && state.start_tick.is_some() {
                let _ = state.start_tick.take();
            }
            // re-enable resume is handled by the completion checker when
            // it next ticks -- don't restart `start_tick` here, that
            // would need the recipe speed lookup which we don't have yet.
        }
    }

    response::ok(index, FactoryOpType::EnableNode)
}
