use tracing::debug;

use crate::net::NetContext;

pub async fn push_factory_sync(ctx: &mut NetContext<'_>) -> bool {
    let region_name = ctx.player.factory.current_region.clone();

    let sync = ctx.player.factory.to_sync(
        &ctx.assets.factory_table,
        &ctx.assets.factory_recycler_const,
        &ctx.assets.factory_sttree,
        &region_name,
    );

    debug!(
        uid = %ctx.player.uid,
        region = %region_name,
        stt_nodes = sync.stt.as_ref().map_or(0, |s| s.nodes.len()),
        formulas = sync.formula_man.as_ref().map_or(0, |f| f.unlocked.len()),
        manufacture = sync.manufacture.as_ref().map_or(0, |m| m.machines.len()),
        trade = sync.trade.as_ref().map_or(0, |t| t.machines.len()),
        recycler = sync.recycler.as_ref().map_or(0, |r| r.machines.len()),
        processor = sync.processor.as_ref().map_or(0, |p| p.machines.len()),
        soil = sync.soil.as_ref().map_or(0, |s| s.machines.len()),
        workshop = sync.workshop.as_ref().map_or(0, |w| w.machines.len()),
        repair = sync.repair.as_ref().map_or(0, |r| r.buildings.len()),
        "pushing factory sync"
    );

    ctx.notify(sync).await.is_ok()
}
