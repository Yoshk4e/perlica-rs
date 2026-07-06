//! `CacheTransportEnable` and `CacheTransportTransfer` ops.
//!
//! Cache transports are explicit cache-to-cache movers (as opposed to
//! the implicit belt routing). `Enable` toggles whether a transport is
//! actively pushing items; `Transfer` triggers a single push pulse.

use crate::net::NetContext;
use perlica_logic::factory::{CacheTransportState, FactoryComponent};
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
    let region = match ctx.player.factory.region_mut(&region_name) {
        Some(r) => r,
        None => {
            return response::fail(
                index,
                FactoryOpType::CacheTransportEnable,
                FactoryOpRetCode::Fail,
                format!("region {} not found", region_name),
            );
        }
    };

    for node in region.nodes.values_mut() {
        if let Some(slot) = node.component_mut(req.component_id) {
            if let FactoryComponent::CacheTransport(state) = slot {
                state.enabled = req.enable;
                return response::ok(index, FactoryOpType::CacheTransportEnable);
            }
            return response::fail(
                index,
                FactoryOpType::CacheTransportEnable,
                FactoryOpRetCode::Fail,
                format!("component {} is not a CacheTransport", req.component_id),
            );
        }
    }

    response::fail(
        index,
        FactoryOpType::CacheTransportEnable,
        FactoryOpRetCode::Fail,
        format!("component {} not found", req.component_id),
    )
}

pub async fn handle_transfer(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpCacheTransportTransfer,
) -> ScFactoryOpRet {
    // The actual move logic needs the HS-transport layer (clause 5) to
    // know how to pull from `source_node_id` and push to `target_node_id`.
    // For now we just verify the component exists and is enabled, then
    // return success with `success: false` to indicate "no items moved
    // this tick".
    let region = match ctx.player.factory.region(&region_name) {
        Some(r) => r,
        None => {
            return response::fail(
                index,
                FactoryOpType::CacheTransportTransfer,
                FactoryOpRetCode::Fail,
                format!("region {} not found", region_name),
            );
        }
    };

    let mut found_state: Option<CacheTransportState> = None;
    for node in region.nodes.values() {
        if let Some(slot) = node.component(req.component_id) {
            if let FactoryComponent::CacheTransport(state) = slot {
                found_state = Some(*state);
                break;
            }
            return response::fail(
                index,
                FactoryOpType::CacheTransportTransfer,
                FactoryOpRetCode::Fail,
                format!("component {} is not a CacheTransport", req.component_id),
            );
        }
    }

    let state = match found_state {
        Some(s) => s,
        None => {
            return response::fail(
                index,
                FactoryOpType::CacheTransportTransfer,
                FactoryOpRetCode::Fail,
                format!("component {} not found", req.component_id),
            );
        }
    };

    if !state.enabled {
        // Disabled transports are a no-op. Report success=false so the
        // client can show "no items moved" feedback.
        return response::ok_with_cache_transport_transfer(index, false);
    }

    // TODO(hs-transport): pull one item-stack from `state.source_node_id`'s
    // cache and push it to `state.target_node_id`'s cache. Return
    // success=true if anything actually moved.
    let _ = ctx;
    response::ok_with_cache_transport_transfer(index, false)
}
