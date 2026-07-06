use crate::net::NetContext;
use perlica_logic::factory::ops;
use perlica_proto::{CsdFactoryOpEnableNode, FactoryOpRetCode, FactoryOpType, ScFactoryOpRet};

use super::super::response;

pub async fn handle(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpEnableNode,
) -> ScFactoryOpRet {
    match ctx
        .player
        .factory
        .enable_node(&region_name, req.node_id, req.enable)
    {
        Ok(()) => response::ok(index, FactoryOpType::EnableNode),
        Err(ops::EnableNodeError::RegionNotFound) => response::fail(
            index,
            FactoryOpType::EnableNode,
            FactoryOpRetCode::Fail,
            format!("region {region_name} not found"),
        ),
        Err(ops::EnableNodeError::NodeNotFound) => response::fail(
            index,
            FactoryOpType::EnableNode,
            FactoryOpRetCode::Fail,
            format!("node {} not found", req.node_id),
        ),
    }
}
