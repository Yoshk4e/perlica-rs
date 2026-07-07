use crate::net::NetContext;
use perlica_logic::factory::ops;
use perlica_proto::{CsdFactoryOpDismantle, FactoryOpRetCode, FactoryOpType, ScFactoryOpRet};

use super::super::response;

pub async fn handle(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpDismantle,
) -> ScFactoryOpRet {
    match ctx
        .player
        .factory
        .dismantle(&ctx.assets.factory_table, &region_name, req.node_id)
    {
        Ok(_) => response::ok(index, FactoryOpType::Dismantle),
        Err(ops::DismantleError::RegionNotFound) => response::fail(
            index,
            FactoryOpType::Dismantle,
            FactoryOpRetCode::Fail,
            format!("region {region_name} not found"),
        ),
        Err(ops::DismantleError::NodeNotFound) => response::fail(
            index,
            FactoryOpType::Dismantle,
            FactoryOpRetCode::Fail,
            format!("node {} not found", req.node_id),
        ),
        Err(ops::DismantleError::ReservedNode) => response::fail(
            index,
            FactoryOpType::Dismantle,
            FactoryOpRetCode::Fail,
            "cannot dismantle the reserved inventory or hub node",
        ),
    }
}
