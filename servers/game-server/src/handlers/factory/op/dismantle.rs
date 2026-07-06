//! `Dismantle` op. Removes a node and returns its source item to the
//! player's inventory.
//!
//! The "return the item" half is gated on the inventory system which
//! isn't wired into factory yet -- see the TODO in `handle`. The
//! structural removal (delete the node, drop connections referencing it)
//! is fully implemented.

use crate::net::NetContext;
use perlica_proto::{CsdFactoryOpDismantle, FactoryOpRetCode, FactoryOpType, ScFactoryOpRet};

use super::super::response;

pub async fn handle(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpDismantle,
) -> ScFactoryOpRet {
    let Some(region) = ctx.player.factory.region_mut(&region_name) else {
            return response::fail(
                index,
                FactoryOpType::Dismantle,
                FactoryOpRetCode::Fail,
                format!("region {region_name} not found"),
            );
    };

    // Verify the node exists and isn't the protected inventory/hub.
    let template_id = match region.node(req.node_id) {
        Some(n) if n.node_id <= 2 => {
            return response::fail(
                index,
                FactoryOpType::Dismantle,
                FactoryOpRetCode::Fail,
                "cannot dismantle the reserved inventory or hub node",
            );
        }
        Some(n) => n.template_id.clone(),
        None => {
            return response::fail(
                index,
                FactoryOpType::Dismantle,
                FactoryOpRetCode::Fail,
                format!("node {} not found", req.node_id),
            );
        }
    };

    // Drop the node.
    region.nodes.remove(&req.node_id);

    // Drop any connection that referenced it. Both endpoints need to be
    // alive for the link to make sense, so a single-endpoint removal is
    // enough.
    region
        .connections
        .retain(|c| c.node_id_a != req.node_id && c.node_id_b != req.node_id);

    // Hand the building's source item back to the player. The
    // `buildingItemData` table maps template_id -> item_id; we just need
    // to look it up and push one stack into the player's bag.
    //
    // TODO(inventory): the player bag API isn't exposed to factory
    // handlers yet. When it lands, look up the item via
    // `ctx.assets.factory_table.item_for_building(&template_id)` and
    // push a stack of count 1. For now we drop the item silently --
    // destructive but at least the node is gone.
    let _ = template_id;
    let _ = ctx;

    // TODO(Clause 4): recompute the power graph -- removing a node can
    // free up consumption and unflag `is_stop_by_power`.

    response::ok(index, FactoryOpType::Dismantle)
}
