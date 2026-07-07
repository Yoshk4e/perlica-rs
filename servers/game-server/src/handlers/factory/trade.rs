//! `CsFactoryTrade*` handlers -- thin wrappers around the logic
//! in `perlica_logic::factory::trade`.

use crate::net::NetContext;
use perlica_logic::factory::current_tick;
use perlica_proto::{
    CsFactoryTradeCashOrder, CsFactoryTradeDeleteOrder, CsFactoryTradeSetContract,
    ScFactoryModifyTrade, ScFactoryTradeCashOrder, ScdFactoryTradeMachine, ScdFactoryTradeOrder,
};

fn serialize_trade(
    manager: &perlica_logic::factory::FactoryManager,
    region_name: &str,
    node_id: u32,
) -> Vec<ScdFactoryTradeMachine> {
    let Some(trade) = manager.trade_state.get(region_name) else {
        return vec![];
    };

    let order_list: Vec<ScdFactoryTradeOrder> = trade
        .orders
        .iter()
        .map(|o| ScdFactoryTradeOrder {
            contract_id: trade.active_contract.clone().unwrap_or_default(),
            inst_id: o.inst_id as u64,
            order_id: o.order_id.clone(),
            cost_value_delta: o.accumulated_value as f64,
        })
        .collect();

    vec![ScdFactoryTradeMachine {
        region: region_name.to_string(),
        node_id,
        machine_id: String::new(),
        building_level: trade.building_level as i32,
        update_ts: current_tick() as i64,
        contract_id: trade.active_contract.clone().unwrap_or_default(),
        order_list,
        current_progress: 0,
        pause_ts: 0,
        current_speed: 0.0,
    }]
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
