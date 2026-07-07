//! `CsFactoryRecycler*` handlers -- thin wrappers around the logic
//! in `perlica_logic::factory::recycler`.

use crate::net::NetContext;
use perlica_logic::factory::recycler::CommitMaterial;
use perlica_proto::{
    CsFactoryRecyclerCommitMaterial, CsFactoryRecyclerFetchProduct,
    ScFactoryRecyclerCommitMaterial, ScFactoryRecyclerFetchProduct, ScdItemGrid,
};

pub async fn on_cs_factory_recycler_commit_material(
    ctx: &mut NetContext<'_>,
    req: CsFactoryRecyclerCommitMaterial,
) -> ScFactoryRecyclerCommitMaterial {
    let materials: Vec<CommitMaterial> = req
        .materials
        .iter()
        .map(|m| CommitMaterial {
            item_id: m.item_id.clone(),
            count: m.count,
        })
        .collect();

    let success = ctx.player.factory.recycler_commit_material(
        &ctx.assets.factory_table,
        &ctx.assets.factory_recycler_const,
        &req.region,
        req.node_id,
        &materials,
    );

    ScFactoryRecyclerCommitMaterial { success }
}

pub async fn on_cs_factory_recycler_fetch_product(
    ctx: &mut NetContext<'_>,
    req: CsFactoryRecyclerFetchProduct,
) -> ScFactoryRecyclerFetchProduct {
    let items = ctx
        .player
        .factory
        .recycler_fetch_product(&ctx.assets.factory_recycler_const, &req.region, req.node_id);

    ScFactoryRecyclerFetchProduct {
        items: items
            .iter()
            .enumerate()
            .map(|(i, item)| ScdItemGrid {
                grid_index: i as i32,
                id: item.item_id.clone(),
                count: item.count as i64,
                inst: None,
            })
            .collect(),
    }
}
