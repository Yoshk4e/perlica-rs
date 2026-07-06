//! `MoveNode` op. Updates a placed node's grid position and direction.
//!
//! Validation mirrors `place`: new footprint must stay in-bounds and not
//! overlap existing nodes (other than itself, since we're moving it).

use crate::net::NetContext;
use perlica_logic::enums::FCDirection;
use perlica_logic::factory::{GridPos, GridRange};
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
    let new_pos = match req.position {
        Some(p) => grid_pos(&p),
        None => {
            return response::fail(
                index,
                FactoryOpType::MoveNode,
                FactoryOpRetCode::Fail,
                "MoveNode requires a position",
            );
        }
    };

    // Resolve direction enum; if client sent garbage, fall back to Up
    // rather than rejecting -- matches the live server's tolerance.
    let direction = match req.direction {
        0 => FCDirection::Up,
        1 => FCDirection::Right,
        2 => FCDirection::Down,
        3 => FCDirection::Left,
        _ => FCDirection::Up,
    };

    let (template_id, footprint, building_w, building_h) = {
        let region = match ctx.player.factory.region(&region_name) {
            Some(r) => r,
            None => {
                return response::fail(
                    index,
                    FactoryOpType::MoveNode,
                    FactoryOpRetCode::Fail,
                    format!("region {} not found", region_name),
                );
            }
        };

        let node = match region.node(req.node_id) {
            Some(n) => n,
            None => {
                return response::fail(
                    index,
                    FactoryOpType::MoveNode,
                    FactoryOpRetCode::Fail,
                    format!("node {} not found", req.node_id),
                );
            }
        };

        // We need the building's range to compute the new footprint.
        // Inventory-style nodes (template_id == "__inventory__") have no
        // grid presence and can't be moved.
        if node.transform.position.is_none() {
            return response::fail(
                index,
                FactoryOpType::MoveNode,
                FactoryOpRetCode::MustInMain,
                "node has no grid position to move",
            );
        }

        let building = match ctx.assets.factory_table.get_building(&node.template_id) {
            Some(b) => b,
            None => {
                return response::fail(
                    index,
                    FactoryOpType::MoveNode,
                    FactoryOpRetCode::Fail,
                    format!("no buildingData for template {}", node.template_id),
                );
            }
        };

        let footprint = GridRange {
            x: new_pos.x,
            y: new_pos.y,
            w: building.range.width,
            h: building.range.height,
        };

        (node.template_id.clone(), footprint, building.range.width, building.range.height)
    };

    // Bounds + overlap check. We treat the moving node as "not present"
    // for the overlap test so it can move to a position that overlaps
    // its own previous footprint.
    {
        let region = match ctx.player.factory.region_mut(&region_name) {
            Some(r) => r,
            None => {
                return response::fail(
                    index,
                    FactoryOpType::MoveNode,
                    FactoryOpRetCode::Fail,
                    format!("region {} not found", region_name),
                );
            }
        };

        let map = match ctx.assets.factory_map.get(&region.scene_name, region.level) {
            Some(m) => m,
            None => {
                return response::fail(
                    index,
                    FactoryOpType::MoveNode,
                    FactoryOpRetCode::MustInMain,
                    "no factory map for scene at this level",
                );
            }
        };

        let main_mesh = GridRange {
            x: map.pos_x,
            y: map.pos_y,
            w: map.range_w,
            h: map.range_h,
        };
        if !perlica_logic::factory::grid::range_within(footprint, main_mesh) {
            return response::fail(
                index,
                FactoryOpType::MoveNode,
                FactoryOpRetCode::MustInMain,
                "new position is outside the region mesh",
            );
        }

        // Overlap check against every other placed node.
        for other in region.nodes.values() {
            if other.node_id == req.node_id {
                continue;
            }
            let Some(other_pos) = other.transform.position else {
                continue;
            };
            let Some(other_building) = ctx.assets.factory_table.get_building(&other.template_id)
            else {
                continue;
            };
            let other_footprint = GridRange {
                x: other_pos.x,
                y: other_pos.y,
                w: other_building.range.width,
                h: other_building.range.height,
            };
            if perlica_logic::factory::grid::ranges_overlap(footprint, other_footprint) {
                return response::fail(
                    index,
                    FactoryOpType::MoveNode,
                    FactoryOpRetCode::MeshConflict,
                    format!("new position overlaps node {}", other.node_id),
                );
            }
        }

        // All clear -- apply the move.
        let node = region.node_mut(req.node_id).unwrap();
        node.transform.position = Some(new_pos);
        node.transform.direction = direction;

        // Rebuild the mesh points so the client renders the node at its
        // new spot. Same 4-point clockwise layout as `bootstrap_region`.
        node.transform.mesh = Some(perlica_logic::factory::Mesh {
            mesh_type: perlica_logic::enums::FCMeshType::Rect,
            points: vec![
                GridPos { x: footprint.x, y: footprint.y },
                GridPos { x: footprint.x + building_w as i32, y: footprint.y },
                GridPos { x: footprint.x + building_w as i32, y: footprint.y + building_h as i32 },
                GridPos { x: footprint.x, y: footprint.y + building_h as i32 },
            ],
        });
    }

    let _ = template_id; // silence unused binding in case future code drops it
    response::ok(index, FactoryOpType::MoveNode)
}

fn grid_pos(p: &ScdFactoryVector2Int) -> GridPos {
    GridPos { x: p.x, y: p.y }
}
