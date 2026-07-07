//! `CsFactoryManuallyWork*` handlers -- thin wrappers around the logic
//! in `perlica_logic::factory::manual_work`.

use crate::net::NetContext;
use perlica_logic::factory::current_tick;
use perlica_proto::{
    CsFactoryManuallyWorkAppend, CsFactoryManuallyWorkCancel, CsFactoryManuallyWorkPause,
    CsFactoryManuallyWorkResume, ScFactoryManuallyWorkCancel, ScFactoryModifyManuallyWork,
    ScdFactorySyncManuallyWorkUnit,
};

fn serialize_queue(
    manager: &perlica_logic::factory::FactoryManager,
) -> ScFactoryModifyManuallyWork {
    let queue: Vec<ScdFactorySyncManuallyWorkUnit> = manager
        .manual_work_state
        .queue
        .iter()
        .map(|unit| ScdFactorySyncManuallyWorkUnit {
            id: unit.recipe_id.clone(),
            count: 1,
        })
        .collect();

    let head_start_tms = manager
        .manual_work_state
        .queue
        .first()
        .map_or(0, |u| u.start_tick as i64);

    let pause_tms = if manager.manual_work_state.is_paused {
        current_tick() as i64
    } else {
        0
    };

    ScFactoryModifyManuallyWork {
        queue,
        in_block: manager.manual_work_state.is_paused,
        head_start_tms,
        pause_tms,
    }
}

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

    serialize_queue(&ctx.player.factory)
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
    serialize_queue(&ctx.player.factory)
}

pub async fn on_cs_factory_manually_work_resume(
    ctx: &mut NetContext<'_>,
    _req: CsFactoryManuallyWorkResume,
) -> ScFactoryModifyManuallyWork {
    ctx.player.factory.manual_work_resume();
    serialize_queue(&ctx.player.factory)
}
