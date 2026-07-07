//! `CsFactoryWorkshopMake` handler -- thin wrapper around the logic
//! in `perlica_logic::factory::workshop`.

use crate::net::NetContext;
use perlica_proto::{CsFactoryWorkshopMake, ScFactoryModifyWorkshop};

pub async fn on_cs_factory_workshop_make(
    ctx: &mut NetContext<'_>,
    req: CsFactoryWorkshopMake,
) -> ScFactoryModifyWorkshop {
    // The region name comes from the player's current region since the
    // workshop request doesn't carry one.
    let region_name = ctx.player.factory.current_region.clone();
    let multi = req.multi.max(1) as u32;

    let _ = ctx
        .player
        .factory
        .workshop_make(&ctx.assets.factory_table, &region_name, &req.formula_id, multi);

    // The response is ScFactoryModifyWorkshop which carries the updated
    // machine list. For now we return an empty update -- the client will
    // refresh on next sync.
    // TODO: serialize the workshop state into the machine list once
    // ScdFactoryWorkshopMachine fields are confirmed.
    ScFactoryModifyWorkshop {
        machines: vec![],
        del_list: vec![],
    }
}
