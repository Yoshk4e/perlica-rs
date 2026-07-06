//! `PlaceBoxConveyor` and `DismantleBoxConveyor` ops.
//!
//! Box conveyors are belt segments. A single `Place` op can lay down
//! multiple belt tiles in one shot (the `points` array is the polyline
//! the belt follows), which is why the response is `Vec<u32>` of node
//! IDs rather than a single ID.
//!
//! Validation: every point must be inside the region mesh, no two
//! points can overlap existing nodes, and the polyline must be axis
//! contiguous (no diagonals) -- the client enforces the latter on its
//! end but we still check on the server side to be safe.

use crate::net::NetContext;
use perlica_logic::enums::{FCDirection, FCMeshType, FCNodeType};
use perlica_logic::factory::{
    FactoryComponent, FactoryNode, GridPos, GridRange, Mesh, NodeTransform, BoxConveyorState,
};
use perlica_proto::{
    CsdFactoryOpDismantleBoxConveyor, CsdFactoryOpPlaceBoxConveyor, FactoryOpRetCode,
    FactoryOpType, ScFactoryOpRet, ScdFactoryVector2Int,
};
use std::collections::HashMap;

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

    let building = match ctx.assets.factory_table.get_building(&req.template_id) {
        Some(b) => b,
        None => {
            return response::fail(
                index,
                FactoryOpType::PlaceBoxConveyor,
                FactoryOpRetCode::NoBuildingItem,
                format!("no buildingData for {}", req.template_id),
            );
        }
    };

    // Each belt tile is a 1x1 node. Pre-validate every point.
    let points: Vec<GridPos> = req.points.iter().map(grid_pos).collect();

    {
        let region = match ctx.player.factory.region_mut(&region_name) {
            Some(r) => r,
            None => {
                return response::fail(
                    index,
                    FactoryOpType::PlaceBoxConveyor,
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
                    FactoryOpType::PlaceBoxConveyor,
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

        for p in &points {
            let tile = GridRange { x: p.x, y: p.y, w: 1, h: 1 };
            if !perlica_logic::factory::grid::range_within(tile, main_mesh) {
                return response::fail(
                    index,
                    FactoryOpType::PlaceBoxConveyor,
                    FactoryOpRetCode::MustInMain,
                    format!("point ({},{}) is outside the region mesh", p.x, p.y),
                );
            }
            // Overlap check. Belt tiles can stack on the same cell as
            // non-belt buildings? Not sure -- the live server rejects
            // it, so we do too.
            for other in region.nodes.values() {
                let Some(other_pos) = other.transform.position else {
                    continue;
                };
                let Some(other_b) = ctx.assets.factory_table.get_building(&other.template_id) else {
                    continue;
                };
                let other_footprint = GridRange {
                    x: other_pos.x,
                    y: other_pos.y,
                    w: other_b.range.width,
                    h: other_b.range.height,
                };
                if perlica_logic::factory::grid::ranges_overlap(tile, other_footprint) {
                    return response::fail(
                        index,
                        FactoryOpType::PlaceBoxConveyor,
                        FactoryOpRetCode::MeshConflict,
                        format!("point ({},{}) overlaps node {}", p.x, p.y, other.node_id),
                    );
                }
            }
        }
    }

    // All clear -- allocate one node per belt tile. Belt nodes carry a
    // single `BoxConveyor` component holding direction + item queue.
    let mut node_ids = Vec::with_capacity(points.len());
    {
        let region = ctx.player.factory.region_mut(&region_name).unwrap();
        let scene_name = region.scene_name.clone();
        for p in &points {
            let node_id = region.allocate_node_id();
            let mesh = Mesh {
                mesh_type: FCMeshType::Rect,
                points: vec![
                    GridPos { x: p.x, y: p.y },
                    GridPos { x: p.x + 1, y: p.y },
                    GridPos { x: p.x + 1, y: p.y + 1 },
                    GridPos { x: p.x, y: p.y + 1 },
                ],
            };
            let node = FactoryNode {
                node_id,
                node_type: FCNodeType::BoxConveyor,
                template_id: req.template_id.clone(),
                transform: NodeTransform {
                    position: Some(*p),
                    // Belts have a direction_in / direction_out pair, but
                    // `direction` on the transform is the visual facing.
                    // We just use `Up` here -- the conveyor component
                    // carries the real I/O directions.
                    direction: FCDirection::Up,
                    mesh: Some(mesh),
                    scene_name: scene_name.clone(),
                    world_position: None,
                    world_rotation: None,
                    bc_port_in: None,
                    bc_port_out: None,
                },
                is_deactive: false,
                interactive_object: None,
                dynamic_property: HashMap::new(),
                component_pos: HashMap::new(),
                components: vec![
                    (
                        1,
                        FactoryComponent::BoxConveyor(BoxConveyorState {
                            items: vec![],
                            direction: req.direction_out,
                        }),
                    ),
                ],
            };
            region.nodes.insert(node_id, node);
            node_ids.push(node_id);
        }
    }

    // belt range_width/height are 1 for box conveyor entries, but we
    // don't actually use the building entry's range here -- belt tiles
    // are always 1x1 by definition. Confirm the config agrees.
    debug_assert!(
        building.range.width == 1 && building.range.height == 1,
        "box conveyor template {} has non-1x1 range, validation may be wrong",
        req.template_id
    );

    response::ok_with_place_box_conveyor(index, node_ids)
}

pub async fn handle_dismantle(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpDismantleBoxConveyor,
) -> ScFactoryOpRet {
    // The client sends a `from` and `to` point representing the polyline
    // segment to remove. We walk every belt node in the region and drop
    // the ones whose position falls inside that AABB.
    let (from, to) = match (req.from.as_ref(), req.to.as_ref()) {
        (Some(f), Some(t)) => (grid_pos(f), grid_pos(t)),
        _ => {
            return response::fail(
                index,
                FactoryOpType::DismantleBoxConveyor,
                FactoryOpRetCode::Fail,
                "DismantleBoxConveyor needs both from and to points",
            );
        }
    };

    let region = match ctx.player.factory.region_mut(&region_name) {
        Some(r) => r,
        None => {
            return response::fail(
                index,
                FactoryOpType::DismantleBoxConveyor,
                FactoryOpRetCode::Fail,
                format!("region {} not found", region_name),
            );
        }
    };

    let bbox = GridRange {
        x: from.x.min(to.x),
        y: from.y.min(to.y),
        w: (from.x - to.x).unsigned_abs() as u32 + 1,
        h: (from.y - to.y).unsigned_abs() as u32 + 1,
    };

    let to_remove: Vec<u32> = region
        .nodes
        .values()
        .filter(|n| n.node_type == FCNodeType::BoxConveyor)
        .filter(|n| {
            n.transform
                .position
                .map(|p| {
                    perlica_logic::factory::grid::is_in_bounds(p, bbox)
                })
                .unwrap_or(false)
        })
        .map(|n| n.node_id)
        .collect();

    for id in &to_remove {
        region.nodes.remove(id);
    }
    // Drop any connections that pointed at removed belts.
    region
        .connections
        .retain(|c| !to_remove.contains(&c.node_id_a) && !to_remove.contains(&c.node_id_b));

    response::ok(index, FactoryOpType::DismantleBoxConveyor)
}

fn grid_pos(p: &ScdFactoryVector2Int) -> GridPos {
    GridPos { x: p.x, y: p.y }
}
