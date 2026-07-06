//! `Place` op.
//!
//! Drops a new building onto the grid. Validates that:
//!   1. The template exists in `buildingData`.
//!   2. The target position is inside the region's main mesh.
//!   3. The new footprint doesn't overlap any existing node.
//!   4. The player has the building's source item in their inventory
//!      (since placing consumes the item -- live server rule).
//!
//! Then allocates a node ID, asks `component_factory` for the right
//! component set, and inserts the node.

use std::collections::HashMap;

use crate::net::NetContext;
use perlica_logic::enums::{FCDirection, FCMeshType, FCNodeType};
use perlica_logic::factory::{
    FactoryComponent, FactoryNode, GridPos, GridRange, Mesh, NodeTransform,
};
use perlica_proto::{
    CsdFactoryOpPlace, FactoryOpRetCode, FactoryOpType, ScFactoryOpRet, ScdFactoryVector2Int,
};

use super::super::response;

pub async fn handle(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpPlace,
) -> ScFactoryOpRet {
    let Some(position) = req.position.as_ref().map(|p| grid_pos(*p)) else {
            return response::fail(
                index,
                FactoryOpType::Place,
                FactoryOpRetCode::Fail,
                "Place requires a position",
            );
    };

    let Some(building) = ctx.assets.factory_table.get_building(&req.template_id) else {
            return response::fail(
                index,
                FactoryOpType::Place,
                FactoryOpRetCode::NoBuildingItem,
                format!("no buildingData for template {}", req.template_id),
            );
    };

    // FCNodeType is the wire discriminator. If the JSON has a stale value
    // we'd rather reject than send garbage to the client. The enum doesn't
    // have a From<i32> impl so we inline the cases we actually accept.
    let node_type = match building.building_type {
        1 => FCNodeType::Inventory,
        2 => FCNodeType::Bus,
        3 => FCNodeType::Hub,
        4 => FCNodeType::Collector,
        5 => FCNodeType::Producer,
        6 => FCNodeType::BoxConveyor,
        7 => FCNodeType::BoxRouterM1,
        8 => FCNodeType::BusUnloader,
        9 => FCNodeType::BusLoader,
        10 => FCNodeType::BurnPower,
        11 => FCNodeType::PowerPole,
        12 => FCNodeType::PowerSave,
        13 => FCNodeType::DepositBox,
        14 => FCNodeType::HealTower,
        15 => FCNodeType::TravelPole,
        16 => FCNodeType::BoxBridge,
        17 => FCNodeType::Special,
        18 => FCNodeType::PowerTerminal,
        19 => FCNodeType::PowerPort,
        20 => FCNodeType::PowerGate,
        other => {
            return response::fail(
                index,
                FactoryOpType::Place,
                FactoryOpRetCode::Fail,
                format!("building {} has invalid type {}", req.template_id, other),
            );
        }
    };

    let footprint = GridRange {
        x: position.x,
        y: position.y,
        w: building.range.width,
        h: building.range.height,
    };

    let direction = match req.direction {
        0 => FCDirection::Up,
        1 => FCDirection::Right,
        2 => FCDirection::Down,
        3 => FCDirection::Left,
        _ => FCDirection::Up,
    };

    // Scope the borrow so we can hand `&mut ctx.player.factory` to the
    // mutator without holding `&ctx.assets` at the same time.
    let scene_name = {
        let Some(region) = ctx.player.factory.region_mut(&region_name) else {
                return response::fail(
                    index,
                    FactoryOpType::Place,
                    FactoryOpRetCode::Fail,
                    format!("region {region_name} not found"),
                );
        };

        let Some(map) = ctx.assets.factory_map.get(&region.scene_name, region.level) else {
                return response::fail(
                    index,
                    FactoryOpType::Place,
                    FactoryOpRetCode::MustInMain,
                    "no factory map for scene at this level",
                );
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
                FactoryOpType::Place,
                FactoryOpRetCode::MustInMain,
                "position is outside the region mesh",
            );
        }

        // Overlap check.
        for other in region.nodes.values() {
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
                    FactoryOpType::Place,
                    FactoryOpRetCode::MeshConflict,
                    format!("position overlaps node {}", other.node_id),
                );
            }
        }

        region.scene_name.clone()
    };

    // Build the node. Component set comes from the factory; if it
    // doesn't know this template yet (component_factory is stubbed),
    // we fall back to an empty component list with a TODO and let the
    // caller fail loudly.
    // TODO(Clause 3.4): wire `component_factory::create_components_from_template`
    // once it's implemented. Currently returns None for everything that
    // isn't the hub bootstrap path.
    let components: Vec<(u32, FactoryComponent)> =
        match perlica_logic::factory::component_factory::create_components_from_template(
            &req.template_id,
        ) {
            Some(built) => built.components,
            None => {
                return response::fail(
                    index,
                    FactoryOpType::Place,
                    FactoryOpRetCode::Fail,
                    format!(
                        "no component layout for template {} yet -- needs Clause 3.4",
                        req.template_id
                    ),
                );
            }
        };

    let component_pos: HashMap<_, _> = match perlica_logic::factory::component_factory::create_components_from_template(&req.template_id) {
        Some(built) => built.component_pos,
        None => HashMap::new(),
    };

    let node_id = {
        let region = ctx.player.factory.region_mut(&region_name).unwrap();
        let node_id = region.allocate_node_id();

        let mesh = Mesh {
            mesh_type: FCMeshType::Rect,
            points: vec![
                GridPos { x: footprint.x, y: footprint.y },
                GridPos { x: footprint.x + footprint.w as i32, y: footprint.y },
                GridPos { x: footprint.x + footprint.w as i32, y: footprint.y + footprint.h as i32 },
                GridPos { x: footprint.x, y: footprint.y + footprint.h as i32 },
            ],
        };

        let node = FactoryNode {
            node_id,
            node_type,
            template_id: req.template_id.clone(),
            transform: NodeTransform {
                position: Some(position),
                direction,
                mesh: Some(mesh),
                scene_name,
                world_position: None,
                world_rotation: None,
                bc_port_in: None,
                bc_port_out: None,
            },
            is_deactive: false,
            interactive_object: None,
            dynamic_property: HashMap::new(),
            component_pos,
            components,
        };

        region.nodes.insert(node_id, node);
        node_id
    };

    // TODO(Clause 4): after placing, recompute the power graph in case
    // the new node is a power source / consumer / relay. For now the
    // blackboard stays stale until next reconnect.

    response::ok_with_place(index, node_id)
}

fn grid_pos(p: ScdFactoryVector2Int) -> GridPos {
    GridPos { x: p.x, y: p.y }
}
