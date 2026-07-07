//! `CsFactoryObserver*` and `CsFactoryStatistic*` handlers -- thin
//! wrappers around the logic in `perlica_logic::factory::observer`.

use crate::net::NetContext;
use perlica_logic::factory::observer::ObserverResult;
use perlica_proto::{
    CsFactoryObserverOp, CsFactoryStatisticRequire, CsFactoryStatisticSetBookmarkItemIds,
    ScFactoryModifyStatistic, ScFactoryObserverRet,
};

pub async fn on_cs_factory_observer_op(
    ctx: &mut NetContext<'_>,
    req: CsFactoryObserverOp,
) -> ScFactoryObserverRet {
    let result = ctx.player.factory.observer_checkout(
        &req.region,
        req.node_id,
        req.component_id,
        &req.op_type,
    );

    let (success, err_message) = match &result {
        ObserverResult::Error(msg) => (false, msg.clone()),
        _ => (true, String::new()),
    };

    ScFactoryObserverRet {
        op_index: req.index,
        success,
        err_message,
        ret_type: req.op_type,
        ret_payload: None,
    }
}

pub async fn on_cs_factory_statistic_require(
    ctx: &mut NetContext<'_>,
    req: CsFactoryStatisticRequire,
) -> ScFactoryModifyStatistic {
    let region_name = ctx.player.factory.current_region.clone();
    let _stats = ctx.player.factory.statistic_require(
        &region_name,
        req.rank_power,
        req.rank_productivity,
        &req.productivity_item_ids,
        req.all_productivity,
    );

    // TODO: serialize stats into ScdFactoryStatisticOption/Other/Lastday
    // once those proto structs are confirmed against live data.
    ScFactoryModifyStatistic {
        option: None,
        other: None,
        last_day: None,
    }
}

pub async fn on_cs_factory_statistic_set_bookmark_item_ids(
    ctx: &mut NetContext<'_>,
    req: CsFactoryStatisticSetBookmarkItemIds,
) {
    ctx.player
        .factory
        .statistic_set_bookmark_item_ids(&req.item_ids, req.is_remove);
}
