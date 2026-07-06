use crate::net::NetContext;
use perlica_logic::factory::ops;
use perlica_proto::{
    CsdFactoryOpMoveNode, FactoryOpRetCode, FactoryOpType, ScFactoryOpRet, ScdFactoryVector2Int,
};

use super::super::response;

pub async fn handle(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpMoveNode,
) -> ScFactoryOpRet {
    let Some(pos) = req.position.as_ref().map(|p| grid_pos(*p)) else {
        return response::fail(
            index,
            FactoryOpType::MoveNode,
            FactoryOpRetCode::Fail,
            "MoveNode requires a position",
        );
    };

    match ctx.player.factory.move_node(
        &ctx.assets.factory_table,
        &ctx.assets.factory_map,
        &region_name,
        req.node_id,
        pos,
        req.direction,
    ) {
        Ok(()) => response::ok(index, FactoryOpType::MoveNode),
        Err(e) => response::fail(
            index,
            FactoryOpType::MoveNode,
            FactoryOpRetCode::Fail,
            match e {
                ops::MoveNodeError::RegionNotFound => format!("region {region_name} not found"),
                ops::MoveNodeError::NodeNotFound => format!("node {} not found", req.node_id),
                ops::MoveNodeError::NoGridPosition => "node has no grid position to move".into(),
                ops::MoveNodeError::NoBuildingEntry => "no buildingData for template".into(),
                ops::MoveNodeError::OutOfBounds => "position outside region mesh".into(),
                ops::MoveNodeError::Overlaps(id) => format!("overlaps node {id}"),
            },
        ),
    }
}

fn grid_pos(p: ScdFactoryVector2Int) -> perlica_logic::factory::GridPos {
    perlica_logic::factory::GridPos { x: p.x, y: p.y }
}
