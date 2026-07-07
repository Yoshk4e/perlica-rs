//! `CsFactoryManuallyWork*` handlers -- thin wrappers around the logic
//! in `perlica_logic::factory::manual_work`.

use crate::net::NetContext;
use perlica_proto::{
    CsFactoryManuallyWorkAppend, CsFactoryManuallyWorkCancel, CsFactoryManuallyWorkPause,
    CsFactoryManuallyWorkResume, ScFactoryManuallyWorkCancel, ScFactoryModifyManuallyWork,
};

pub async fn on_cs_factory_manually_work_append(
    ctx: &mut NetContext<'_>,
    req: CsFactoryManuallyWorkAppend,
) -> ScFactoryModifyManuallyWork {
    let region_name = ctx.player.factory.current_region.clone();
    let _ = ctx.player.factory.manual_work_append(
        &ctx.assets.factory_table,
        &region_name,
        &req.formula_id,
        req.count,
    );

    ScFactoryModifyManuallyWork {
        queue: vec![],
        in_block: ctx.player.factory.manual_work_state.is_paused,
        head_start_tms: 0,
        pause_tms: 0,
    }
}

pub async fn on_cs_factory_manually_work_cancel(
    ctx: &mut NetContext<'_>,
    _req: CsFactoryManuallyWorkCancel,
) -> ScFactoryManuallyWorkCancel {
    let region_name = ctx.player.factory.current_region.clone();
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
    ctx.player.factory.manual_work_pause();

    ScFactoryModifyManuallyWork {
        queue: vec![],
        in_block: true,
        head_start_tms: 0,
        pause_tms: 0,
    }
}

pub async fn on_cs_factory_manually_work_resume(
    ctx: &mut NetContext<'_>,
    _req: CsFactoryManuallyWorkResume,
) -> ScFactoryModifyManuallyWork {
    ctx.player.factory.manual_work_resume();

    ScFactoryModifyManuallyWork {
        queue: vec![],
        in_block: false,
        head_start_tms: 0,
        pause_tms: 0,
    }
}
