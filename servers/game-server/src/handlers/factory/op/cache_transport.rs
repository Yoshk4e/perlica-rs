use crate::net::NetContext;
use perlica_logic::factory::ops;
use perlica_proto::{
    CsdFactoryOpCacheTransportEnable, CsdFactoryOpCacheTransportTransfer, FactoryOpRetCode,
    FactoryOpType, ScFactoryOpRet,
};

use super::super::response;

pub async fn handle_enable(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpCacheTransportEnable,
) -> ScFactoryOpRet {
    match ctx
        .player
        .factory
        .cache_transport_enable(&region_name, req.component_id, req.enable)
    {
        Ok(()) => response::ok(index, FactoryOpType::CacheTransportEnable),
        Err(e) => response::fail(
            index,
            FactoryOpType::CacheTransportEnable,
            FactoryOpRetCode::Fail,
            match e {
                ops::TargetError::RegionNotFound => format!("region {region_name} not found"),
                ops::TargetError::ComponentNotFound => {
                    format!("component {} not found", req.component_id)
                }
                ops::TargetError::WrongComponentType => {
                    format!("component {} is not a CacheTransport", req.component_id)
                }
            },
        ),
    }
}

pub async fn handle_transfer(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpCacheTransportTransfer,
) -> ScFactoryOpRet {
    match ctx
        .player
        .factory
        .cache_transport_transfer(&region_name, req.component_id)
    {
        Ok(success) => response::ok_with_cache_transport_transfer(index, success),
        Err(e) => response::fail(
            index,
            FactoryOpType::CacheTransportTransfer,
            FactoryOpRetCode::Fail,
            match e {
                ops::TargetError::RegionNotFound => format!("region {region_name} not found"),
                ops::TargetError::ComponentNotFound => {
                    format!("component {} not found", req.component_id)
                }
                ops::TargetError::WrongComponentType => {
                    format!("component {} is not a CacheTransport", req.component_id)
                }
            },
        ),
    }
}
