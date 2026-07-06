//! `CsFactoryProcessor*` handlers -- thin wrappers around the logic
//! in `perlica_logic::factory::processor`.
//!
//! Each handler decodes the proto request, calls the matching logic
//! function, and encodes the result into `ScFactoryProcessorRet`.

use crate::net::NetContext;
use perlica_logic::factory::processor;
use perlica_proto::{
    CsFactoryProcessorMakeEquip, CsFactoryProcessorMakeGem, CsFactoryProcessorMakeItem,
    CsFactoryProcessorMarkUnlockFormulaRead, CsFactoryProcessorRecastGem, ScFactoryProcessorRet,
    ScdItemGrid,
};

pub async fn on_cs_factory_processor_make_item(
    ctx: &mut NetContext<'_>,
    req: CsFactoryProcessorMakeItem,
) -> ScFactoryProcessorRet {
    let result = processor::make_item(
        &mut ctx.player.factory,
        &ctx.assets.factory_table,
        &req.region,
        &req.formula_id,
        req.count.max(1) as u32,
    );
    to_ret(result)
}

pub async fn on_cs_factory_processor_make_equip(
    ctx: &mut NetContext<'_>,
    req: CsFactoryProcessorMakeEquip,
) -> ScFactoryProcessorRet {
    let result = processor::make_equip(
        &mut ctx.player.factory,
        &ctx.assets.factory_table,
        &ctx.assets.factory_processor_const,
        &req.region,
        &req.formula_id,
        req.count.max(1) as u32,
        req.use_refine_point,
    );
    to_ret(result)
}

pub async fn on_cs_factory_processor_make_gem(
    ctx: &mut NetContext<'_>,
    req: CsFactoryProcessorMakeGem,
) -> ScFactoryProcessorRet {
    let result = processor::make_gem(
        &mut ctx.player.factory,
        &ctx.assets.factory_table,
        &req.region,
        &req.formula_id,
        req.count.max(1) as u32,
        &req.cost_gem_inst_ids,
    );
    to_ret(result)
}

pub async fn on_cs_factory_processor_recast_gem(
    ctx: &mut NetContext<'_>,
    req: CsFactoryProcessorRecastGem,
) -> ScFactoryProcessorRet {
    let result = processor::recast_gem(
        &mut ctx.player.factory,
        &ctx.assets.factory_table,
        &ctx.assets.factory_processor_const,
        &req.region,
        &req.formula_id,
        req.count.max(1) as u32,
        &req.cost_gem_inst_ids,
    );
    to_ret(result)
}

pub async fn on_cs_factory_processor_mark_unlock_formula_read(
    ctx: &mut NetContext<'_>,
    req: CsFactoryProcessorMarkUnlockFormulaRead,
) {
    processor::mark_formulas_read(&mut ctx.player.factory, &req.read_formula_ids);
}

fn to_ret(result: processor::CraftResult) -> ScFactoryProcessorRet {
    ScFactoryProcessorRet {
        new_items: result
            .new_items
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
