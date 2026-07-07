//! `CsFactoryManufacture*` handlers -- thin wrappers around the logic
//! in `perlica_logic::factory::manufacture`.

use crate::net::NetContext;
use perlica_proto::{
    CsFactoryManufactureCancel, CsFactoryManufactureSettle, CsFactoryManufactureStart,
    ScFactoryManufactureCancel, ScFactoryManufactureSettle, ScFactoryManufactureStart,
};

pub async fn on_cs_factory_manufacture_start(
    ctx: &mut NetContext<'_>,
    req: CsFactoryManufactureStart,
) -> ScFactoryManufactureStart {
    match ctx.player.factory.manufacture_start(
        &ctx.assets.factory_table,
        &req.region,
        req.node_id,
        &req.formula_id,
        req.count.max(1) as u32,
    ) {
        Ok(result) => ScFactoryManufactureStart {
            region: req.region,
            node_id: req.node_id,
            old_formula: result.old_formula,
            old_got: result.old_got,
            old_least_multi: result.old_least_multi,
        },
        Err(_) => ScFactoryManufactureStart {
            region: req.region,
            node_id: req.node_id,
            old_formula: String::new(),
            old_got: 0,
            old_least_multi: 0,
        },
    }
}

pub async fn on_cs_factory_manufacture_cancel(
    ctx: &mut NetContext<'_>,
    req: CsFactoryManufactureCancel,
) -> ScFactoryManufactureCancel {
    match ctx
        .player
        .factory
        .manufacture_cancel(&req.region, req.node_id)
    {
        Ok(result) => ScFactoryManufactureCancel {
            region: req.region,
            node_id: req.node_id,
            old_formula: result.old_formula,
            old_got: result.old_got,
            old_least_multi: result.old_least_multi,
        },
        Err(_) => ScFactoryManufactureCancel {
            region: req.region,
            node_id: req.node_id,
            old_formula: String::new(),
            old_got: 0,
            old_least_multi: 0,
        },
    }
}

pub async fn on_cs_factory_manufacture_settle(
    ctx: &mut NetContext<'_>,
    req: CsFactoryManufactureSettle,
) -> ScFactoryManufactureSettle {
    match ctx.player.factory.manufacture_settle(
        &ctx.assets.factory_table,
        &ctx.assets.factory_manufact_const,
        &req.region,
        req.node_id,
    ) {
        Ok(result) => ScFactoryManufactureSettle {
            region: req.region,
            node_id: req.node_id,
            settle_count: result.settle_count,
            auto_supple_count: result.auto_supple_count,
        },
        Err(_) => ScFactoryManufactureSettle {
            region: req.region,
            node_id: req.node_id,
            settle_count: 0,
            auto_supple_count: 0,
        },
    }
}
