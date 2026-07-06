use crate::net::NetContext;
use perlica_logic::factory::ops;
use perlica_proto::{
    CsdFactoryOpDismantleBoxConveyor, CsdFactoryOpPlaceBoxConveyor, FactoryOpRetCode,
    FactoryOpType, ScFactoryOpRet, ScdFactoryVector2Int,
};

use super::super::response;

pub async fn handle_place(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpPlaceBoxConveyor,
) -> ScFactoryOpRet {
    if req.points.is_empty() {
        return response::fail(
            index,
            FactoryOpType::PlaceBoxConveyor,
            FactoryOpRetCode::Fail,
            "PlaceBoxConveyor needs at least one point",
        );
    }

    let points: Vec<perlica_logic::factory::GridPos> =
        req.points.iter().map(|p| grid_pos(*p)).collect();

    match ctx.player.factory.place_box_conveyor(
        &ctx.assets.factory_table,
        &ctx.assets.factory_map,
        &region_name,
        &req.template_id,
        req.direction_out,
        &points,
    ) {
        Ok(node_ids) => response::ok_with_place_box_conveyor(index, node_ids),
        Err(e) => response::fail(
            index,
            FactoryOpType::PlaceBoxConveyor,
            match e {
                ops::ConveyorError::OutOfBounds => FactoryOpRetCode::MustInMain,
                ops::ConveyorError::Overlaps(_) => FactoryOpRetCode::MeshConflict,
                ops::ConveyorError::NoBuildingEntry => FactoryOpRetCode::NoBuildingItem,
                _ => FactoryOpRetCode::Fail,
            },
            match e {
                ops::ConveyorError::RegionNotFound => format!("region {region_name} not found"),
                ops::ConveyorError::NoBuildingEntry => {
                    format!("no buildingData for {}", req.template_id)
                }
                ops::ConveyorError::OutOfBounds => "point outside region mesh".into(),
                ops::ConveyorError::Overlaps(id) => format!("overlaps node {id}"),
                ops::ConveyorError::NoFromTo => "needs at least one point".into(),
            },
        ),
    }
}

pub async fn handle_dismantle(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpDismantleBoxConveyor,
) -> ScFactoryOpRet {
    let (Some(from), Some(to)) = (
        req.from.as_ref().map(|p| grid_pos(*p)),
        req.to.as_ref().map(|p| grid_pos(*p)),
    ) else {
        return response::fail(
            index,
            FactoryOpType::DismantleBoxConveyor,
            FactoryOpRetCode::Fail,
            "DismantleBoxConveyor needs both from and to points",
        );
    };

    match ctx
        .player
        .factory
        .dismantle_box_conveyor(&region_name, from, to)
    {
        Ok(_) => response::ok(index, FactoryOpType::DismantleBoxConveyor),
        Err(e) => response::fail(
            index,
            FactoryOpType::DismantleBoxConveyor,
            FactoryOpRetCode::Fail,
            match e {
                ops::ConveyorError::RegionNotFound => format!("region {region_name} not found"),
                _ => "dismantle failed".into(),
            },
        ),
    }
}

fn grid_pos(p: ScdFactoryVector2Int) -> perlica_logic::factory::GridPos {
    perlica_logic::factory::GridPos { x: p.x, y: p.y }
}
