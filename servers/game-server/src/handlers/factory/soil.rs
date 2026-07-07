//! `CsFactorySoil*` handlers -- thin wrappers around the logic
//! in `perlica_logic::factory::soil`.

use crate::net::NetContext;
use perlica_proto::{
    CsFactorySoilCancel, CsFactorySoilHarvest, CsFactorySoilPlant, ScFactorySoilCancel,
    ScFactorySoilHarvest, ScFactorySoilPlant,
};
use tracing::warn;

pub async fn on_cs_factory_soil_plant(
    ctx: &mut NetContext<'_>,
    req: CsFactorySoilPlant,
) -> ScFactorySoilPlant {
    if !ctx.player.factory.soil_plant(
        &ctx.assets.factory_table,
        &req.region,
        req.node_id,
        &req.seed_item_id,
    ) {
        warn!(seed = %req.seed_item_id, "soil plant failed");
    }
    ScFactorySoilPlant {}
}

pub async fn on_cs_factory_soil_harvest(
    ctx: &mut NetContext<'_>,
    req: CsFactorySoilHarvest,
) -> ScFactorySoilHarvest {
    if !ctx.player.factory.soil_harvest(
        &ctx.assets.factory_table,
        &req.region,
        req.node_id,
        req.harvest_type,
    ) {
        warn!("soil harvest failed -- not fully grown or no seed");
    }
    ScFactorySoilHarvest {}
}

pub async fn on_cs_factory_soil_cancel(
    ctx: &mut NetContext<'_>,
    req: CsFactorySoilCancel,
) -> ScFactorySoilCancel {
    if !ctx.player.factory.soil_cancel(&req.region, req.node_id) {
        warn!("soil cancel failed -- no soil state");
    }
    ScFactorySoilCancel {}
}
