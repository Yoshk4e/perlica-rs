//! Builders for `ScFactoryOpRet`.
//!
//! Every op handler returns one of these. The wire format is fussy about
//! the `op_type` discriminator matching the `op_payload` variant, so we
//! centralize the construction here and let the op handlers just hand over
//! the data they care about.
//!
//! Design: `ok_*` builders take the response payload (or just the request
//! index for ops that have nothing to say beyond "yep it worked"), `fail`
//! takes an error code + message, and the dispatcher wraps both into the
//! outer `ScFactoryOpRet` envelope.

use perlica_proto::{
    FactoryOpRetCode, FactoryOpType, ScFactoryOpRet, ScdFactoryOpRetAddConnection,
    ScdFactoryOpRetCacheTransportTransfer, ScdFactoryOpRetPlace, ScdFactoryOpRetPlaceBoxConveyor,
    ScdFactoryOpRetUseHealTowerPoint, sc_factory_op_ret,
};

/// Empty payload works for the ops whose Ret variant is just `{}`. We
/// pick the right `OpPayload` arm based on the `op_type` discriminator.
fn empty_payload(op_type: FactoryOpType) -> Option<sc_factory_op_ret::OpPayload> {
    use sc_factory_op_ret::OpPayload::*;
    Some(match op_type {
        FactoryOpType::Dismantle => Dismantle(Default::default()),
        FactoryOpType::DismantleBoxConveyor => DismantleBoxConveyor(Default::default()),
        FactoryOpType::EnableNode => EnableNode(Default::default()),
        FactoryOpType::MoveNode => MoveNode(Default::default()),
        FactoryOpType::SetCollectTarget => SetCollectTarget(Default::default()),
        FactoryOpType::SetSelectTarget => SetSelectTarget(Default::default()),
        FactoryOpType::SetTravelPoleDefaultNext => SetTravelPoleDefaultNext(Default::default()),
        FactoryOpType::DelConnection => DelConnection(Default::default()),
        FactoryOpType::GridBoxInnerMove => GridBoxInnerMove(Default::default()),
        FactoryOpType::GridBoxInnerSplit => GridBoxInnerSplit(Default::default()),
        FactoryOpType::MoveItemBagToCache => MoveItemBagToCache(Default::default()),
        FactoryOpType::MoveItemBagToGridBox => MoveItemBagToGridBox(Default::default()),
        FactoryOpType::MoveItemCacheToBag => MoveItemCacheToBag(Default::default()),
        FactoryOpType::MoveItemCacheToCache => MoveItemCacheToCache(Default::default()),
        FactoryOpType::MoveItemCacheToDepot => MoveItemCacheToDepot(Default::default()),
        FactoryOpType::MoveItemConveyorToBag => MoveItemConveyorToBag(Default::default()),
        FactoryOpType::MoveItemDepotToCache => MoveItemDepotToCache(Default::default()),
        FactoryOpType::MoveItemDepotToGridBox => MoveItemDepotToGridBox(Default::default()),
        FactoryOpType::MoveItemGridBoxToBag => MoveItemGridBoxToBag(Default::default()),
        FactoryOpType::CacheTransportEnable => CacheTransportEnable(Default::default()),
        _ => return None,
    })
}

/// Generic "yep, it worked" with no payload data. Covers every op whose
/// Ret variant is the empty struct.
pub fn ok(index: impl Into<String>, op_type: FactoryOpType) -> ScFactoryOpRet {
    ScFactoryOpRet {
        index: index.into(),
        ret_code: FactoryOpRetCode::OkA8d3 as i32,
        op_type: op_type as i32,
        err_message: String::new(),
        op_payload: empty_payload(op_type),
    }
}

pub fn ok_with_place(index: impl Into<String>, node_id: u32) -> ScFactoryOpRet {
    ScFactoryOpRet {
        index: index.into(),
        ret_code: FactoryOpRetCode::OkA8d3 as i32,
        op_type: FactoryOpType::Place as i32,
        err_message: String::new(),
        op_payload: Some(sc_factory_op_ret::OpPayload::Place(ScdFactoryOpRetPlace { node_id })),
    }
}

pub fn ok_with_place_box_conveyor(
    index: impl Into<String>,
    node_ids: Vec<u32>,
) -> ScFactoryOpRet {
    ScFactoryOpRet {
        index: index.into(),
        ret_code: FactoryOpRetCode::OkA8d3 as i32,
        op_type: FactoryOpType::PlaceBoxConveyor as i32,
        err_message: String::new(),
        op_payload: Some(sc_factory_op_ret::OpPayload::PlaceBoxConveyor(
            ScdFactoryOpRetPlaceBoxConveyor { node_id: node_ids },
        )),
    }
}

pub fn ok_with_add_connection(index: impl Into<String>, conn_index: u64) -> ScFactoryOpRet {
    ScFactoryOpRet {
        index: index.into(),
        ret_code: FactoryOpRetCode::OkA8d3 as i32,
        op_type: FactoryOpType::AddConnection as i32,
        err_message: String::new(),
        op_payload: Some(sc_factory_op_ret::OpPayload::AddConnection(
            ScdFactoryOpRetAddConnection { index: conn_index },
        )),
    }
}

pub fn ok_with_cache_transport_transfer(
    index: impl Into<String>,
    success: bool,
) -> ScFactoryOpRet {
    ScFactoryOpRet {
        index: index.into(),
        ret_code: FactoryOpRetCode::OkA8d3 as i32,
        op_type: FactoryOpType::CacheTransportTransfer as i32,
        err_message: String::new(),
        op_payload: Some(sc_factory_op_ret::OpPayload::CacheTransportTransfer(
            ScdFactoryOpRetCacheTransportTransfer { success },
        )),
    }
}

pub fn ok_with_use_heal_tower(
    index: impl Into<String>,
    used_count: u32,
) -> ScFactoryOpRet {
    ScFactoryOpRet {
        index: index.into(),
        ret_code: FactoryOpRetCode::OkA8d3 as i32,
        op_type: FactoryOpType::UseHealTowerPoint as i32,
        err_message: String::new(),
        op_payload: Some(sc_factory_op_ret::OpPayload::UseHealTowerPoint(
            ScdFactoryOpRetUseHealTowerPoint { used_count },
        )),
    }
}

/// Failure with a specific code. We don't carry a payload on failure --
/// the client just reads `err_message` and bails.
pub fn fail(
    index: impl Into<String>,
    op_type: FactoryOpType,
    code: FactoryOpRetCode,
    message: impl Into<String>,
) -> ScFactoryOpRet {
    ScFactoryOpRet {
        index: index.into(),
        ret_code: code as i32,
        op_type: op_type as i32,
        err_message: message.into(),
        op_payload: None,
    }
}

/// For ops that came in with a missing or unrecognized `op_payload` --
/// we can't even route them. The client sent an `op_type` but no body.
pub fn unknown_op_type(index: impl Into<String>, op_type: i32) -> ScFactoryOpRet {
    let op_type_enum = FactoryOpType::try_from(op_type).unwrap_or(FactoryOpType::NoneAd3b);
    fail(
        index,
        op_type_enum,
        FactoryOpRetCode::UnknownOpType,
        format!("unrecognized op_type {}", op_type),
    )
}
