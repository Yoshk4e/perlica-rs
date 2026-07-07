//! `CsFactoryTrade*` handlers -- thin wrappers around the logic
//! in `perlica_logic::factory::trade`.

use crate::net::NetContext;
use perlica_proto::{
    CsFactoryTradeCashOrder, CsFactoryTradeDeleteOrder, CsFactoryTradeSetContract,
    ScFactoryModifyTrade, ScFactoryTradeCashOrder,
};

pub async fn on_cs_factory_trade_set_contract(
    ctx: &mut NetContext<'_>,
    req: CsFactoryTradeSetContract,
) -> ScFactoryModifyTrade {
    ctx.player.factory.trade_set_contract(
        &ctx.assets.contracts,
        &req.region,
        req.node_id,
        &req.contract_id,
    );

    ScFactoryModifyTrade {
        machines: vec![],
        del_list: vec![],
    }
}

pub async fn on_cs_factory_trade_cash_order(
    ctx: &mut NetContext<'_>,
    req: CsFactoryTradeCashOrder,
) -> ScFactoryTradeCashOrder {
    let success = ctx.player.factory.trade_cash_order(
        &ctx.assets.factory_table,
        &ctx.assets.contracts,
        &req.region,
        req.node_id,
        req.inst_id,
        &req.items,
    );

    ScFactoryTradeCashOrder { success }
}

pub async fn on_cs_factory_trade_delete_order(
    ctx: &mut NetContext<'_>,
    req: CsFactoryTradeDeleteOrder,
) -> ScFactoryModifyTrade {
    ctx.player
        .factory
        .trade_delete_order(&req.region, req.node_id, req.inst_id);

    ScFactoryModifyTrade {
        machines: vec![],
        del_list: vec![],
    }
}
