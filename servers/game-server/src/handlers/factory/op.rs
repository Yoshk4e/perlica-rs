//! `CsFactoryOp` dispatcher.
//!
//! One entry point (`on_cs_factory_op`), 26 op variants routed to their
//! respective handlers under `op/`. Each handler owns its own validation
//! and state mutation, then returns an `ScFactoryOpRet` -- the
//! dispatcher just hands the request off and forwards the response.
//!
//! Layout matches the file plan in §8 of the implementation doc: one
//! file per op family, grouped by the kind of work they do.

pub mod cache_transport;
pub mod connection;
pub mod conveyor;
pub mod dismantle;
pub mod enable_node;
pub mod gridbox;
pub mod move_node;
pub mod place;
pub mod special;
pub mod target;

use crate::net::NetContext;
use perlica_proto::{CsFactoryOp, FactoryOpType, ScFactoryOpRet, cs_factory_op::OpPayload as CsOp};
use tracing::{debug, warn};

use super::response;

/// Entry point registered in `net/router.rs`. Decodes the request,
/// dispatches on `op_type` / `op_payload`, and returns the reply.
pub async fn on_cs_factory_op(ctx: &mut NetContext<'_>, req: CsFactoryOp) -> ScFactoryOpRet {
    let Ok(op_type) = FactoryOpType::try_from(req.op_type) else {
        warn!(op_type = req.op_type, "CsFactoryOp with unknown op_type");
        return response::unknown_op_type(req.index, req.op_type);
    };

    debug!(
        uid = %ctx.player.uid,
        index = %req.index,
        op_type = ?op_type,
        region = %req.name,
        "CsFactoryOp"
    );

    // Most ops carry a payload in the oneof. A few (none currently, but
    // the proto reserves the slot) can come in bare -- if so we still
    // need to dispatch on `op_type` and let the handler complain.
    let payload = req.op_payload;

    match (op_type, payload) {
        (FactoryOpType::Place, Some(CsOp::Place(p))) => {
            place::handle(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::Place, _) => bad_payload(req.index, op_type),

        (FactoryOpType::PlaceBoxConveyor, Some(CsOp::PlaceBoxConveyor(p))) => {
            conveyor::handle_place(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::PlaceBoxConveyor, _) => bad_payload(req.index, op_type),

        (FactoryOpType::Dismantle, Some(CsOp::Dismantle(p))) => {
            dismantle::handle(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::Dismantle, _) => bad_payload(req.index, op_type),

        (FactoryOpType::DismantleBoxConveyor, Some(CsOp::DismantleBoxConveyor(p))) => {
            conveyor::handle_dismantle(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::DismantleBoxConveyor, _) => bad_payload(req.index, op_type),

        (FactoryOpType::EnableNode, Some(CsOp::EnableNode(p))) => {
            enable_node::handle(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::EnableNode, _) => bad_payload(req.index, op_type),

        (FactoryOpType::MoveNode, Some(CsOp::MoveNode(p))) => {
            move_node::handle(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::MoveNode, _) => bad_payload(req.index, op_type),

        (FactoryOpType::SetCollectTarget, Some(CsOp::SetCollectTarget(p))) => {
            target::handle_collect(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::SetCollectTarget, _) => bad_payload(req.index, op_type),

        (FactoryOpType::SetSelectTarget, Some(CsOp::SetSelectTarget(p))) => {
            target::handle_select(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::SetSelectTarget, _) => bad_payload(req.index, op_type),

        (FactoryOpType::AddConnection, Some(CsOp::AddConnection(p))) => {
            connection::handle_add(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::AddConnection, _) => bad_payload(req.index, op_type),

        (FactoryOpType::DelConnection, Some(CsOp::DelConnection(p))) => {
            connection::handle_del(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::DelConnection, _) => bad_payload(req.index, op_type),

        (FactoryOpType::SetTravelPoleDefaultNext, Some(CsOp::SetTravelPoleDefaultNext(p))) => {
            special::handle_set_travel_pole_next(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::SetTravelPoleDefaultNext, _) => bad_payload(req.index, op_type),

        (FactoryOpType::UseHealTowerPoint, Some(CsOp::UseHealTowerPoint(p))) => {
            special::handle_use_heal_tower(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::UseHealTowerPoint, _) => bad_payload(req.index, op_type),

        (FactoryOpType::CacheTransportEnable, Some(CsOp::CacheTransportEnable(p))) => {
            cache_transport::handle_enable(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::CacheTransportEnable, _) => bad_payload(req.index, op_type),

        (FactoryOpType::CacheTransportTransfer, Some(CsOp::CacheTransportTransfer(p))) => {
            cache_transport::handle_transfer(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::CacheTransportTransfer, _) => bad_payload(req.index, op_type),

        (FactoryOpType::MoveItemCacheToCache, Some(CsOp::MoveItemCacheToCache(p))) => {
            gridbox::handle_move_cache_to_cache(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::MoveItemCacheToCache, _) => bad_payload(req.index, op_type),

        (FactoryOpType::MoveItemBagToCache, Some(CsOp::MoveItemBagToCache(p))) => {
            gridbox::handle_move_bag_to_cache(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::MoveItemBagToCache, _) => bad_payload(req.index, op_type),

        (FactoryOpType::MoveItemCacheToBag, Some(CsOp::MoveItemCacheToBag(p))) => {
            gridbox::handle_move_cache_to_bag(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::MoveItemCacheToBag, _) => bad_payload(req.index, op_type),

        (FactoryOpType::MoveItemDepotToCache, Some(CsOp::MoveItemDepotToCache(p))) => {
            gridbox::handle_move_depot_to_cache(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::MoveItemDepotToCache, _) => bad_payload(req.index, op_type),

        (FactoryOpType::MoveItemCacheToDepot, Some(CsOp::MoveItemCacheToDepot(p))) => {
            gridbox::handle_move_cache_to_depot(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::MoveItemCacheToDepot, _) => bad_payload(req.index, op_type),

        (FactoryOpType::MoveItemConveyorToBag, Some(CsOp::MoveItemConveyorToBag(p))) => {
            gridbox::handle_move_conveyor_to_bag(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::MoveItemConveyorToBag, _) => bad_payload(req.index, op_type),

        (FactoryOpType::GridBoxInnerMove, Some(CsOp::GridBoxInnerMove(p))) => {
            gridbox::handle_inner_move(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::GridBoxInnerMove, _) => bad_payload(req.index, op_type),

        (FactoryOpType::GridBoxInnerSplit, Some(CsOp::GridBoxInnerSplit(p))) => {
            gridbox::handle_inner_split(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::GridBoxInnerSplit, _) => bad_payload(req.index, op_type),

        (FactoryOpType::MoveItemBagToGridBox, Some(CsOp::MoveItemBagToGridBox(p))) => {
            gridbox::handle_move_bag_to_gridbox(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::MoveItemBagToGridBox, _) => bad_payload(req.index, op_type),

        (FactoryOpType::MoveItemGridBoxToBag, Some(CsOp::MoveItemGridBoxToBag(p))) => {
            gridbox::handle_move_gridbox_to_bag(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::MoveItemGridBoxToBag, _) => bad_payload(req.index, op_type),

        (FactoryOpType::MoveItemDepotToGridBox, Some(CsOp::MoveItemDepotToGridBox(p))) => {
            gridbox::handle_move_depot_to_gridbox(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::MoveItemDepotToGridBox, _) => bad_payload(req.index, op_type),

        (FactoryOpType::MoveItemGridBoxToDepot, Some(CsOp::MoveItemGridBoxToDepot(p))) => {
            gridbox::handle_move_gridbox_to_depot(ctx, req.index, req.name, p).await
        }
        (FactoryOpType::MoveItemGridBoxToDepot, _) => bad_payload(req.index, op_type),

        (FactoryOpType::NoneAd3b, _) => {
            warn!(index = %req.index, "CsFactoryOp with NoneAd3b op_type, ignoring");
            response::unknown_op_type(req.index, req.op_type)
        }
    }
}

fn bad_payload(index: String, op_type: FactoryOpType) -> ScFactoryOpRet {
    warn!(?op_type, %index, "CsFactoryOp payload missing or mismatched with op_type");
    response::fail(
        index,
        op_type,
        perlica_proto::FactoryOpRetCode::Fail,
        "op payload missing or did not match op_type",
    )
}
