use crate::net::NetContext;
use perlica_logic::factory::ops;
use perlica_proto::{
    CsdFactoryOpSetCollectTarget, CsdFactoryOpSetSelectTarget, FactoryOpRetCode, FactoryOpType,
    ScFactoryOpRet,
};

use super::super::response;

pub async fn handle_select(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpSetSelectTarget,
) -> ScFactoryOpRet {
    match ctx
        .player
        .factory
        .set_select_target(&region_name, req.component_id, req.item_id)
    {
        Ok(()) => response::ok(index, FactoryOpType::SetSelectTarget),
        Err(e) => response::fail(
            index,
            FactoryOpType::SetSelectTarget,
            FactoryOpRetCode::Fail,
            match e {
                ops::TargetError::RegionNotFound => format!("region {region_name} not found"),
                ops::TargetError::ComponentNotFound => {
                    format!("component {} not found", req.component_id)
                }
                ops::TargetError::WrongComponentType => {
                    format!("component {} is not a Selector", req.component_id)
                }
            },
        ),
    }
}

pub async fn handle_collect(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpSetCollectTarget,
) -> ScFactoryOpRet {
    match ctx.player.factory.set_collect_target(
        &region_name,
        req.component_id,
        req.item_id,
        req.count,
    ) {
        Ok(()) => response::ok(index, FactoryOpType::SetCollectTarget),
        Err(e) => response::fail(
            index,
            FactoryOpType::SetCollectTarget,
            FactoryOpRetCode::Fail,
            match e {
                ops::TargetError::RegionNotFound => format!("region {region_name} not found"),
                ops::TargetError::ComponentNotFound => {
                    format!("component {} not found", req.component_id)
                }
                ops::TargetError::WrongComponentType => {
                    format!("component {} is not a Collector", req.component_id)
                }
            },
        ),
    }
}
