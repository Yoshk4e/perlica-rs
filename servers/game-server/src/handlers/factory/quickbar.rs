use crate::net::NetContext;
use perlica_proto::{
    CsFactoryQuickbarMoveOne, CsFactoryQuickbarSetOne, ScFactoryModifyQuickbar,
    ScdFactorySyncQuickbar,
};

pub async fn on_cs_factory_quickbar_set_one(
    ctx: &mut NetContext<'_>,
    req: CsFactoryQuickbarSetOne,
) -> ScFactoryModifyQuickbar {
    ctx.player
        .factory
        .quickbar_set_one(req.r#type, req.index, &req.item_id);

    ScFactoryModifyQuickbar {
        quickbars: serialize_quickbars(&ctx.player.factory.quickbars),
    }
}

pub async fn on_cs_factory_quickbar_move_one(
    ctx: &mut NetContext<'_>,
    req: CsFactoryQuickbarMoveOne,
) -> ScFactoryModifyQuickbar {
    ctx.player
        .factory
        .quickbar_move_one(req.r#type, req.from_index, req.to_index);

    ScFactoryModifyQuickbar {
        quickbars: serialize_quickbars(&ctx.player.factory.quickbars),
    }
}

fn serialize_quickbars(
    quickbars: &[perlica_logic::factory::QuickbarState],
) -> Vec<ScdFactorySyncQuickbar> {
    quickbars
        .iter()
        .map(|q| ScdFactorySyncQuickbar {
            r#type: q.quickbar_type.parse().unwrap_or(0),
            list: q.items.clone(),
        })
        .collect()
}
