//! `CsFactoryTrade*` handlers -- thin wrappers around the logic
//! in `perlica_logic::factory::trade`.

use crate::net::NetContext;
use perlica_logic::factory::current_tick;
use perlica_proto::{
    CsFactoryTradeCashOrder, CsFactoryTradeDeleteOrder, CsFactoryTradeSetContract,
    ScFactoryModifyTrade, ScFactoryTradeCashOrder, ScdFactoryTradeMachine,
};

fn serialize_trade(
    manager: &perlica_logic::factory::FactoryManager,
    region_name: &str,
    node_id: u32,
) -> Vec<ScdFactoryTradeMachine> {
    manager
        .sync_trade(region_name, current_tick())
        .machines
        .into_iter()
        .filter(|machine| machine.node_id == node_id)
        .collect()
}

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

    let machines = serialize_trade(&ctx.player.factory, &req.region, req.node_id);
    ScFactoryModifyTrade {
        machines,
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

    let machines = serialize_trade(&ctx.player.factory, &req.region, req.node_id);
    ScFactoryModifyTrade {
        machines,
        del_list: vec![],
    }
}
