//! `CsFactoryManuallyWork*` handlers -- thin wrappers around the logic
//! in `perlica_logic::factory::manual_work`.

use crate::net::NetContext;
use perlica_logic::factory::current_tick;
use perlica_proto::{
    CsFactoryManuallyWorkAppend, CsFactoryManuallyWorkCancel, CsFactoryManuallyWorkPause,
    CsFactoryManuallyWorkResume, ScFactoryManuallyWorkCancel, ScFactoryModifyManuallyWork,
};

fn serialize_queue(
    manager: &perlica_logic::factory::FactoryManager,
) -> ScFactoryModifyManuallyWork {
    let sync = manager.sync_manual_work(current_tick());
    ScFactoryModifyManuallyWork {
        queue: sync.queue,
        in_block: sync.in_block,
        head_start_tms: sync.head_start_tms,
        pause_tms: sync.pause_tms,
    }
}

pub async fn on_cs_factory_manually_work_append(
    ctx: &mut NetContext<'_>,
    req: CsFactoryManuallyWorkAppend,
) -> ScFactoryModifyManuallyWork {
    let region_name = ctx.player.factory.current_region.clone();


    ctx.player
        .factory
        .tick_manual_work(&ctx.assets.factory_table, &region_name);

    let _ = ctx.player.factory.manual_work_append(
        &ctx.assets.factory_table,
        &region_name,
        &req.formula_id,
        req.count,
    );

    serialize_queue(&ctx.player.factory)
}

pub async fn on_cs_factory_manually_work_cancel(
    ctx: &mut NetContext<'_>,
    _req: CsFactoryManuallyWorkCancel,
) -> ScFactoryManuallyWorkCancel {
    let region_name = ctx.player.factory.current_region.clone();

    ctx.player
        .factory
        .tick_manual_work(&ctx.assets.factory_table, &region_name);

    let (back_items, break_items) = ctx
        .player
        .factory
        .manual_work_cancel(&ctx.assets.factory_table, &region_name);

    ScFactoryManuallyWorkCancel {
        back_items,
        break_items,
    }
}

pub async fn on_cs_factory_manually_work_pause(
    ctx: &mut NetContext<'_>,
    _req: CsFactoryManuallyWorkPause,
) -> ScFactoryModifyManuallyWork {
    let region_name = ctx.player.factory.current_region.clone();


    ctx.player
        .factory
        .tick_manual_work(&ctx.assets.factory_table, &region_name);

    ctx.player.factory.manual_work_pause();
    serialize_queue(&ctx.player.factory)
}

pub async fn on_cs_factory_manually_work_resume(
    ctx: &mut NetContext<'_>,
    _req: CsFactoryManuallyWorkResume,
) -> ScFactoryModifyManuallyWork {
    ctx.player.factory.manual_work_resume();
    serialize_queue(&ctx.player.factory)
}
