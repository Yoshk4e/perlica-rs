//! `CsFactoryRepairBuilding` handler -- thin wrapper around the logic
//! in `perlica_logic::factory::repair`.
//!
//! The depot bridge messages (bag <-> factory depot) are already handled
//! by the gridbox ops (move_bag_to_gridbox, move_gridbox_to_depot, etc.)
//! since the depot is the hub's Inventory component (node_id=2, comp=8).

use crate::net::NetContext;
use perlica_proto::{CsFactoryRepairBuilding, ScFactoryModifyRepair, ScdFactoryRepairBuilding};
use tracing::warn;

pub async fn on_cs_factory_repair_building(
    ctx: &mut NetContext<'_>,
    req: CsFactoryRepairBuilding,
) -> ScFactoryModifyRepair {
    let region_name = ctx.player.factory.current_region.clone();

    if let Some(node_id) = ctx.player.factory.repair_building(
        &ctx.assets.repair,
        &ctx.assets.factory_table,
        &region_name,
        &req.repair_id,
    ) {
        let repair_id = req.repair_id.clone();
        ScFactoryModifyRepair {
            buildings: vec![ScdFactoryRepairBuilding {
                repair_id: repair_id.clone(),
                node_id,
            }],
            repair_ids: vec![repair_id],
        }
    } else {
        warn!(repair_id = %req.repair_id, "repair building failed");
        ScFactoryModifyRepair {
            buildings: vec![],
            repair_ids: vec![],
        }
    }
}
