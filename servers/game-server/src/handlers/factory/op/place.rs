use crate::net::NetContext;
use perlica_logic::factory::ops;
use perlica_proto::{
    CsdFactoryOpPlace, FactoryOpRetCode, FactoryOpType, ScFactoryOpRet, ScdFactoryVector2Int,
};

use super::super::{notify, response};

pub async fn handle(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpPlace,
) -> ScFactoryOpRet {
    let Some(pos) = req.position.as_ref().map(|p| grid_pos(*p)) else {
        return response::fail(
            index,
            FactoryOpType::Place,
            FactoryOpRetCode::Fail,
            "Place requires a position",
        );
    };

    tracing::debug!(
        uid = %ctx.player.uid,
        region = %region_name,
        template = %req.template_id,
        direction = req.direction,
        ?pos,
        "Place grid coords (client grid space)"
    );

    match ctx.player.factory.place(
        &ctx.assets.factory_table,
        &ctx.assets.factory_map,
        &region_name,
        &req.template_id,
        pos,
        req.direction,
    ) {
        Ok(node_id) => {
            // Client's _OpRetHandler_Place lambda (b__50_1) reads the
            // just-placed node from its local cache via the node_id we
            // return here. That cache is only populated by the
            // ScFactoryModifyRegionNodes notification, so we MUST push
            // the new node before the OpRet or the client NPEs.
            let modify = notify::modify_nodes(&ctx.player.factory, &region_name, &[node_id]);
            let _ = ctx.notify(modify).await;
            response::ok_with_place(index, node_id)
        }
        Err(e) => {
            tracing::warn!(
                uid = %ctx.player.uid,
                region = %region_name,
                template = %req.template_id,
                direction = req.direction,
                ?pos,
                ?e,
                "Place rejected"
            );
            response::fail(
                index,
                FactoryOpType::Place,
                match e {
                    ops::PlaceError::RegionNotFound => FactoryOpRetCode::Fail,
                    ops::PlaceError::NoBuildingEntry => FactoryOpRetCode::NoBuildingItem,
                    ops::PlaceError::InvalidNodeType => FactoryOpRetCode::Fail,
                    ops::PlaceError::OutOfBounds => FactoryOpRetCode::MustInMain,
                    ops::PlaceError::Overlaps(_) => FactoryOpRetCode::MeshConflict,
                    ops::PlaceError::NoComponentLayout => FactoryOpRetCode::Fail,
                },
                match e {
                    ops::PlaceError::RegionNotFound => format!("region {region_name} not found"),
                    ops::PlaceError::NoBuildingEntry => {
                        format!("no buildingData for {}", req.template_id)
                    }
                    ops::PlaceError::InvalidNodeType => {
                        format!("invalid node type for {}", req.template_id)
                    }
                    ops::PlaceError::OutOfBounds => "position outside region mesh".into(),
                    ops::PlaceError::Overlaps(id) => format!("overlaps node {id}"),
                    ops::PlaceError::NoComponentLayout => {
                        format!("no component layout for {}", req.template_id)
                    }
                },
            )
        }
    }
}

fn grid_pos(p: ScdFactoryVector2Int) -> perlica_logic::factory::GridPos {
    perlica_logic::factory::GridPos { x: p.x, y: p.y }
}
