//! `CsFactoryWorkshopMake` handler -- thin wrapper around the logic
//! in `perlica_logic::factory::workshop`.

use crate::net::NetContext;
use perlica_proto::{CsFactoryWorkshopMake, ScFactoryModifyWorkshop};
use tracing::warn;

pub async fn on_cs_factory_workshop_make(
    ctx: &mut NetContext<'_>,
    req: CsFactoryWorkshopMake,
) -> ScFactoryModifyWorkshop {
    let region_name = ctx.player.factory.current_region.clone();
    let multi = req.multi.max(1) as u32;

    match ctx.player.factory.workshop_make(
        &ctx.assets.factory_table,
        &region_name,
        &req.formula_id,
        multi,
    ) {
        Ok(_produced) => {
            // Workshop state is minimal (region_name + building_level).
            // The produced items went into the bag. The client will see
            // them via the bag inventory sync.
        }
        Err(e) => {
            warn!(?e, formula = %req.formula_id, "workshop make failed");
        }
    }

    ScFactoryModifyWorkshop {
        machines: vec![],
        del_list: vec![],
    }
}
