use crate::net::NetContext;
use perlica_logic::factory::ops;
use perlica_proto::{
    CsdFactoryOpGridBoxInnerMove, CsdFactoryOpGridBoxInnerSplit, CsdFactoryOpMoveItemBagToCache,
    CsdFactoryOpMoveItemBagToGridBox, CsdFactoryOpMoveItemCacheToBag,
    CsdFactoryOpMoveItemCacheToCache, CsdFactoryOpMoveItemCacheToDepot,
    CsdFactoryOpMoveItemConveyorToBag, CsdFactoryOpMoveItemDepotToCache,
    CsdFactoryOpMoveItemDepotToGridBox, CsdFactoryOpMoveItemGridBoxToBag,
    CsdFactoryOpMoveItemGridBoxToDepot, FactoryOpRetCode, FactoryOpType, ScFactoryOpRet,
};

use super::super::response;

fn err_msg(e: &ops::GridBoxError) -> String {
    match e {
        ops::GridBoxError::RegionNotFound => "region not found".into(),
        ops::GridBoxError::ComponentNotFound => "component not found".into(),
        ops::GridBoxError::WrongComponentType => "wrong component type".into(),
        ops::GridBoxError::IndexOutOfRange => "index out of range".into(),
        ops::GridBoxError::SlotEmpty => "slot is empty".into(),
        ops::GridBoxError::InventoryNodeMissing => "inventory node not found".into(),
        ops::GridBoxError::HubNodeMissing => "hub node not found".into(),
        ops::GridBoxError::InventoryComponentMissing => "inventory component not found".into(),
        ops::GridBoxError::ItemNotFound => "item not found".into(),
    }
}

pub async fn handle_inner_move(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpGridBoxInnerMove,
) -> ScFactoryOpRet {
    match ctx.player.factory.gridbox_inner_move(
        &region_name,
        req.component_id,
        req.from_index,
        req.to_index,
    ) {
        Ok(()) => response::ok(index, FactoryOpType::GridBoxInnerMove),
        Err(e) => response::fail(
            index,
            FactoryOpType::GridBoxInnerMove,
            FactoryOpRetCode::Fail,
            err_msg(&e),
        ),
    }
}

pub async fn handle_inner_split(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpGridBoxInnerSplit,
) -> ScFactoryOpRet {
    match ctx.player.factory.gridbox_inner_split(
        &region_name,
        req.component_id,
        req.from_index,
        req.to_index,
        req.count,
    ) {
        Ok(()) => response::ok(index, FactoryOpType::GridBoxInnerSplit),
        Err(e) => response::fail(
            index,
            FactoryOpType::GridBoxInnerSplit,
            FactoryOpRetCode::Fail,
            err_msg(&e),
        ),
    }
}

pub async fn handle_move_bag_to_gridbox(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemBagToGridBox,
) -> ScFactoryOpRet {
    match ctx.player.factory.move_bag_to_gridbox(
        &region_name,
        req.component_id,
        req.bag_grid_index,
        req.grid_box_index,
    ) {
        Ok(()) => response::ok(index, FactoryOpType::MoveItemBagToGridBox),
        Err(e) => response::fail(
            index,
            FactoryOpType::MoveItemBagToGridBox,
            FactoryOpRetCode::Fail,
            err_msg(&e),
        ),
    }
}

pub async fn handle_move_gridbox_to_bag(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemGridBoxToBag,
) -> ScFactoryOpRet {
    match ctx.player.factory.move_gridbox_to_bag(
        &region_name,
        req.component_id,
        req.grid_box_index,
        req.bag_grid_index,
    ) {
        Ok(()) => response::ok(index, FactoryOpType::MoveItemGridBoxToBag),
        Err(e) => response::fail(
            index,
            FactoryOpType::MoveItemGridBoxToBag,
            FactoryOpRetCode::Fail,
            err_msg(&e),
        ),
    }
}

pub async fn handle_move_depot_to_gridbox(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemDepotToGridBox,
) -> ScFactoryOpRet {
    match ctx.player.factory.move_depot_to_gridbox(
        &region_name,
        req.component_id,
        &req.item_id,
        req.grid_box_index,
    ) {
        Ok(()) => response::ok(index, FactoryOpType::MoveItemDepotToGridBox),
        Err(e) => response::fail(
            index,
            FactoryOpType::MoveItemDepotToGridBox,
            FactoryOpRetCode::Fail,
            err_msg(&e),
        ),
    }
}

pub async fn handle_move_gridbox_to_depot(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemGridBoxToDepot,
) -> ScFactoryOpRet {
    match ctx.player.factory.move_gridbox_to_depot(
        &region_name,
        req.component_id,
        req.grid_box_index,
    ) {
        Ok(()) => response::ok(index, FactoryOpType::MoveItemGridBoxToDepot),
        Err(e) => response::fail(
            index,
            FactoryOpType::MoveItemGridBoxToDepot,
            FactoryOpRetCode::Fail,
            err_msg(&e),
        ),
    }
}

pub async fn handle_move_cache_to_cache(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemCacheToCache,
) -> ScFactoryOpRet {
    match ctx.player.factory.move_cache_to_cache(
        &region_name,
        req.from_component_id,
        req.to_component_id,
        &req.item_id,
    ) {
        Ok(()) => response::ok(index, FactoryOpType::MoveItemCacheToCache),
        Err(e) => response::fail(
            index,
            FactoryOpType::MoveItemCacheToCache,
            FactoryOpRetCode::Fail,
            err_msg(&e),
        ),
    }
}

pub async fn handle_move_bag_to_cache(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemBagToCache,
) -> ScFactoryOpRet {
    match ctx
        .player
        .factory
        .move_bag_to_cache(&region_name, req.component_id, req.grid_index)
    {
        Ok(()) => response::ok(index, FactoryOpType::MoveItemBagToCache),
        Err(e) => response::fail(
            index,
            FactoryOpType::MoveItemBagToCache,
            FactoryOpRetCode::Fail,
            err_msg(&e),
        ),
    }
}

pub async fn handle_move_cache_to_bag(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemCacheToBag,
) -> ScFactoryOpRet {
    match ctx
        .player
        .factory
        .move_cache_to_bag(&region_name, req.component_id, &req.item_id)
    {
        Ok(()) => response::ok(index, FactoryOpType::MoveItemCacheToBag),
        Err(e) => response::fail(
            index,
            FactoryOpType::MoveItemCacheToBag,
            FactoryOpRetCode::Fail,
            err_msg(&e),
        ),
    }
}

pub async fn handle_move_depot_to_cache(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemDepotToCache,
) -> ScFactoryOpRet {
    match ctx
        .player
        .factory
        .move_depot_to_cache(&region_name, req.component_id, &req.item_id)
    {
        Ok(()) => response::ok(index, FactoryOpType::MoveItemDepotToCache),
        Err(e) => response::fail(
            index,
            FactoryOpType::MoveItemDepotToCache,
            FactoryOpRetCode::Fail,
            err_msg(&e),
        ),
    }
}

pub async fn handle_move_cache_to_depot(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemCacheToDepot,
) -> ScFactoryOpRet {
    match ctx
        .player
        .factory
        .move_cache_to_depot(&region_name, req.component_id, &req.item_id)
    {
        Ok(()) => response::ok(index, FactoryOpType::MoveItemCacheToDepot),
        Err(e) => response::fail(
            index,
            FactoryOpType::MoveItemCacheToDepot,
            FactoryOpRetCode::Fail,
            err_msg(&e),
        ),
    }
}

pub async fn handle_move_conveyor_to_bag(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveItemConveyorToBag,
) -> ScFactoryOpRet {
    match ctx.player.factory.move_conveyor_to_bag(
        &region_name,
        req.component_id,
        req.index,
        req.all,
    ) {
        Ok(()) => response::ok(index, FactoryOpType::MoveItemConveyorToBag),
        Err(e) => response::fail(
            index,
            FactoryOpType::MoveItemConveyorToBag,
            FactoryOpRetCode::Fail,
            err_msg(&e),
        ),
    }
}
