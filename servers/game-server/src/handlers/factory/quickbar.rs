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
        .map(|q| {
            let mut list = q.items.clone();
            // The client reads `SCD_FACTORY_SYNC_QUICKBAR.list` as a flat
            // 4x8 grid (indices 0..=31) in `QuickBar::SyncData` and the
            // context QuickBar constructor; any other length crashes it.
            list.resize(perlica_logic::factory::QUICKBAR_SIZE, String::new());
            ScdFactorySyncQuickbar {
                r#type: q.quickbar_type,
                list,
            }
        })
        .collect()
}
