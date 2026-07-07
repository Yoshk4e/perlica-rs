//! Factory command handlers.
//!
//! `push_factory` is the login-time dump of the player's whole factory
//! state. Everything else lives under `op/` and is dispatched from
//! `op::on_cs_factory_op` based on `FactoryOpType`.
//!
//! The `response` and `notify` modules hold the wire-format builders that
//! every op handler ends up needing, so they're collected in one place
//! rather than copy-pasted across the op files.

pub mod character_work;
pub mod hs_feedback;
pub mod hs_inout;
pub mod manual_work;
pub mod manufacture;
pub mod notify;
pub mod observer;
pub mod op;
pub mod processor;
pub mod quickbar;
pub mod recycler;
pub mod repair;
pub mod response;
pub mod soil;
pub mod stt;
pub mod trade;
pub mod workshop;

pub use character_work::{
    on_cs_factory_character_work_punch_in, on_cs_factory_character_work_punch_out,
};
pub use hs_feedback::on_cs_factory_hs_fb;
pub use hs_inout::on_cs_factory_hs_inout;
pub use manual_work::{
    on_cs_factory_manually_work_append, on_cs_factory_manually_work_cancel,
    on_cs_factory_manually_work_pause, on_cs_factory_manually_work_resume,
};
pub use manufacture::{
    on_cs_factory_manufacture_cancel, on_cs_factory_manufacture_settle,
    on_cs_factory_manufacture_start,
};
pub use observer::{
    on_cs_factory_observer_op, on_cs_factory_statistic_require,
    on_cs_factory_statistic_set_bookmark_item_ids,
};
pub use op::on_cs_factory_op;
pub use processor::{
    on_cs_factory_processor_make_equip, on_cs_factory_processor_make_gem,
    on_cs_factory_processor_make_item, on_cs_factory_processor_mark_unlock_formula_read,
    on_cs_factory_processor_recast_gem,
};
pub use quickbar::{on_cs_factory_quickbar_move_one, on_cs_factory_quickbar_set_one};
pub use recycler::{on_cs_factory_recycler_commit_material, on_cs_factory_recycler_fetch_product};
pub use repair::on_cs_factory_repair_building;
pub use soil::{on_cs_factory_soil_cancel, on_cs_factory_soil_harvest, on_cs_factory_soil_plant};
pub use stt::on_cs_factory_stt_unlock_node;
pub use trade::{
    on_cs_factory_trade_cash_order, on_cs_factory_trade_delete_order,
    on_cs_factory_trade_set_contract,
};
pub use workshop::on_cs_factory_workshop_make;

use std::collections::HashMap;

use crate::net::NetContext;
use perlica_logic::enums::{
    FCComponentPos, FCComponentType, FCDirection, FCMeshType, FCNodeType, FCPropertyKey,
};
use perlica_logic::factory::{FactoryManager, GridPos, GridRange};
use perlica_proto::{
    ScFactorySyncContext, ScdFactoryRectInt, ScdFactorySubPort, ScdFactorySyncBlackboard,
    ScdFactorySyncBlackboardPower, ScdFactorySyncComponent, ScdFactorySyncComponentBusLoader,
    ScdFactorySyncComponentInventory, ScdFactorySyncComponentPowerPole,
    ScdFactorySyncComponentPowerSave, ScdFactorySyncComponentSelector,
    ScdFactorySyncComponentStablePower, ScdFactorySyncDynamicProperty,
    ScdFactorySyncDynamicPropertyValue, ScdFactorySyncInteractiveObject, ScdFactorySyncMesh,
    ScdFactorySyncNode, ScdFactorySyncQuickbar, ScdFactorySyncRegion, ScdFactorySyncScene,
    ScdFactorySyncSceneBandwidth, ScdFactorySyncTransform, ScdFactoryVector2Int, Vector,
    scd_factory_sync_component, scd_factory_sync_dynamic_property_value,
};
use tracing::{debug, warn};

const HUB_TEMPLATE: &str = "sp_hub_1";

pub async fn push_factory(ctx: &mut NetContext<'_>) -> bool {
    // "test01" is the only region name the client accepts for this version :thonk:
    // all others (e.g. "region_102" from levelRegionData) are rejected.
    let region_name = "test01".to_string();
    let scene_name = ctx.player.world.last_scene.clone();

    let Some(building) = ctx.assets.factory_table.get_building(HUB_TEMPLATE) else {
        warn!(
            template = HUB_TEMPLATE,
            "no buildingData entry, skipping factory push"
        );
        return false;
    };
    let Some(hub_data) = ctx.assets.factory_table.get_hub(HUB_TEMPLATE) else {
        warn!(
            template = HUB_TEMPLATE,
            "no hubData entry, skipping factory push"
        );
        return false;
    };

    let Some(map) = ctx.assets.factory_map.get(&scene_name, 1) else {
        warn!(
            scene = %scene_name,
            "no FactoryMapTable entry at level 1, skipping factory push"
        );
        return false;
    };

    // Compute hub grid position
    //
    // Center of main_mesh:
    //   center_x = pos_x + range_w / 2
    //   center_y = pos_y + range_h / 2
    //
    // Top-left of hub (floor of half-size):
    //   top_left = center - floor(hub_size / 2)
    //
    // Example (map01_lv001): pos=(17,−36) range=36×36 hub=9×9
    //   center = (35,−18)  ->  top_left = (31,−22)
    let hub_w = building.range.width as i32;
    let hub_h = building.range.height as i32;
    let center_x = map.pos_x + map.range_w as i32 / 2;
    let center_y = map.pos_y + map.range_h as i32 / 2;
    let top_left_x = center_x - hub_w / 2;
    let top_left_y = center_y - hub_h / 2;

    let Some(region_id) = FactoryManager::derive_region_id(&ctx.assets.factory_table, &scene_name)
    else {
        warn!(
            scene = %scene_name,
            "no levelRegionData entry for scene, skipping factory push"
        );
        return false;
    };

    // This generates the inventory node (node ID = 1) and hub node
    // (node ID = 2) using the 1+7 component layout provided by the
    // live server. The proto is generated manually here, Clause 3 will
    // change this block to `region.to_proto()` when the `to_proto`
    // implementations come into play (tasks 3.1 through 3.3).
    //
    // This code only does the bootstrapping if the player misses the
    // cached version: If they have already been on this region before
    // and created buildings on this wire name, we do not touch the
    // existing state.
    // TODO(DB): With the persistence, this same check will
    // restore state from the `factory_regions` table on login.
    let hub_position = GridPos {
        x: top_left_x,
        y: top_left_y,
    };
    let hub_range = GridRange {
        x: top_left_x,
        y: top_left_y,
        w: building.range.width,
        h: building.range.height,
    };
    let power_gen = hub_data.power_generate;
    let power_save_max = hub_data.power_storage_capacity;
    if ctx.player.factory.regions.contains_key(&region_name) {
        debug!(
            uid = %ctx.player.uid,
            scene = %scene_name,
            region = %region_name,
            region_id = %region_id,
            "factory region already present in internal model, skipping bootstrap"
        );
    } else {
        let region = FactoryManager::bootstrap_region(
            region_name.clone(),
            region_id.clone(),
            scene_name.clone(),
            HUB_TEMPLATE,
            hub_position,
            hub_range,
            power_gen,
            power_save_max,
        );
        ctx.player
            .factory
            .regions
            .insert(region_name.clone(), region);
        debug!(
            uid = %ctx.player.uid,
            scene = %scene_name,
            region = %region_name,
            region_id = %region_id,
            "factory region bootstrapped in internal model"
        );
    }

    let bc_port_in = building.input_ports.first().map(|p| ScdFactorySubPort {
        position: Some(ScdFactoryVector2Int {
            x: p.point.x,
            y: p.point.y,
        }),
        direction: p.side,
    });
    let bc_port_out = building.output_ports.first().map(|p| ScdFactorySubPort {
        position: Some(ScdFactoryVector2Int {
            x: p.point.x,
            y: p.point.y,
        }),
        direction: p.side,
    });

    //  Mesh points
    //
    // point[0] = TL = (top_left_x,           top_left_y          )
    // point[1] = TR = (top_left_x + hub_w,   top_left_y          )
    // point[2] = BR = (top_left_x + hub_w,   top_left_y + hub_h  )
    // point[3] = BL = (top_left_x,           top_left_y + hub_h  )
    let hub_mesh = ScdFactorySyncMesh {
        mesh_type: FCMeshType::Rect as i32,
        points: vec![
            ScdFactoryVector2Int {
                x: top_left_x,
                y: top_left_y,
            },
            ScdFactoryVector2Int {
                x: top_left_x + hub_w,
                y: top_left_y,
            },
            ScdFactoryVector2Int {
                x: top_left_x + hub_w,
                y: top_left_y + hub_h,
            },
            ScdFactoryVector2Int {
                x: top_left_x,
                y: top_left_y + hub_h,
            },
        ],
    };

    // There is no table source for this; the values come from scene geometry. so they're gotten manually ahaha.
    let world_pos = Vector {
        x: 302.0,
        y: 115.0,
        z: 370.0,
    };
    let world_rot = Vector {
        x: 0.0,
        y: 60.0,
        z: 0.0,
    };

    let inventory_node = ScdFactorySyncNode {
        node_id: 1,
        node_type: FCNodeType::Inventory as i32,
        // Just like how it is on live servers (1.2), although i'm not sure if it should be but the future will reveal.
        template_id: "__inventory__".to_string(),
        transform: Some(ScdFactorySyncTransform {
            position: None,
            direction: FCDirection::Up as i32,
            mesh: None,
            scene_name: scene_name.clone(),
            world_position: None,
            world_rotation: None,
            bc_port_in: None,
            bc_port_out: None,
        }),
        is_deactive: true,
        interactive_object: None,
        dynamic_property: Some(ScdFactorySyncDynamicProperty {
            values: HashMap::new(),
        }),
        component_pos: {
            let mut m = HashMap::new();
            m.insert(FCComponentPos::Inventory as i32, 1u32);
            m
        },
        components: vec![ScdFactorySyncComponent {
            component_id: 1,
            component_type: FCComponentType::Inventory as i32,
            component_payload: Some(scd_factory_sync_component::ComponentPayload::Inventory(
                ScdFactorySyncComponentInventory {
                    items: HashMap::new(),
                },
            )),
        }],
    };

    let hub_node = ScdFactorySyncNode {
        node_id: 2,
        node_type: FCNodeType::Hub as i32,
        template_id: HUB_TEMPLATE.to_string(),
        transform: Some(ScdFactorySyncTransform {
            position: Some(ScdFactoryVector2Int {
                x: top_left_x,
                y: top_left_y,
            }),
            direction: FCDirection::Up as i32,
            mesh: Some(hub_mesh),
            scene_name: scene_name.clone(),
            world_position: Some(world_pos),
            world_rotation: Some(world_rot),
            bc_port_in,
            bc_port_out,
        }),
        is_deactive: false,
        interactive_object: Some(ScdFactorySyncInteractiveObject { object_id: 1 }),
        dynamic_property: Some(ScdFactorySyncDynamicProperty {
            values: {
                let mut m = HashMap::new();
                m.insert(
                    FCPropertyKey::InstKey as i32,
                    ScdFactorySyncDynamicPropertyValue {
                        value: Some(scd_factory_sync_dynamic_property_value::Value::StringValue(
                            format!("{scene_name}_{HUB_TEMPLATE}"),
                        )),
                    },
                );
                m
            },
        }),
        // component_pos: FCComponentPos -> component_id
        component_pos: {
            let mut m = HashMap::new();
            m.insert(FCComponentPos::Hub as i32, 2u32);
            m.insert(FCComponentPos::BusLoader as i32, 3u32);
            m.insert(FCComponentPos::PowerPole as i32, 4u32);
            m.insert(FCComponentPos::PowerSave as i32, 5u32);
            m.insert(FCComponentPos::StablePower as i32, 6u32);
            m.insert(FCComponentPos::Selector as i32, 7u32);
            m.insert(FCComponentPos::Inventory as i32, 8u32);
            m
        },
        components: vec![
            // Hub component
            ScdFactorySyncComponent {
                component_id: 2,
                component_type: FCComponentType::Hub as i32,
                component_payload: None,
            },
            // BusLoader, item I/O on the conveyor bus.
            ScdFactorySyncComponent {
                component_id: 3,
                component_type: FCComponentType::BusLoader as i32,
                component_payload: Some(scd_factory_sync_component::ComponentPayload::BusLoader(
                    ScdFactorySyncComponentBusLoader {
                        last_putin_item_id: String::new(),
                        ports: vec![],
                    },
                )),
            },
            // PowerPole, power-grid connectivity.
            ScdFactorySyncComponent {
                component_id: 4,
                component_type: FCComponentType::PowerPole as i32,
                component_payload: Some(scd_factory_sync_component::ComponentPayload::PowerPole(
                    ScdFactorySyncComponentPowerPole { in_power: true },
                )),
            },
            // PowerSave, energy storage
            ScdFactorySyncComponent {
                component_id: 5,
                component_type: FCComponentType::PowerSave as i32,
                component_payload: Some(scd_factory_sync_component::ComponentPayload::PowerSave(
                    ScdFactorySyncComponentPowerSave {
                        power_save: hub_data.power_storage_capacity,
                        in_power: true,
                    },
                )),
            },
            // StablePower, passive generation.
            ScdFactorySyncComponent {
                component_id: 6,
                component_type: FCComponentType::StablePower as i32,
                component_payload: Some(scd_factory_sync_component::ComponentPayload::StablePower(
                    ScdFactorySyncComponentStablePower {
                        in_power: true,
                        power_gen_per_sec: hub_data.power_generate,
                    },
                )),
            },
            // Selector, item filter / output routing.
            ScdFactorySyncComponent {
                component_id: 7,
                component_type: FCComponentType::Selector as i32,
                component_payload: Some(scd_factory_sync_component::ComponentPayload::Selector(
                    ScdFactorySyncComponentSelector {
                        selected_item_id: String::new(),
                        ports: vec![],
                    },
                )),
            },
            // Inventory, hub's internal item buffer i think.
            ScdFactorySyncComponent {
                component_id: 8,
                component_type: FCComponentType::Inventory as i32,
                component_payload: Some(scd_factory_sync_component::ComponentPayload::Inventory(
                    ScdFactorySyncComponentInventory {
                        items: HashMap::new(),
                    },
                )),
            },
        ],
    };

    let main_mesh = vec![ScdFactoryRectInt {
        x: map.pos_x,
        y: map.pos_y,
        w: map.range_w as i32,
        h: map.range_h as i32,
    }];

    let scene = ScdFactorySyncScene {
        name: scene_name.clone(),
        level: map.level as i32,
        main_mesh,
        connections: vec![],
        bandwidth: Some(ScdFactorySyncSceneBandwidth {
            current: 0,
            max: 1_000_000,
            sp_current: 0,
            sp_max: 1_000_000,
        }),
    };

    let blackboard = ScdFactorySyncBlackboard {
        inventory_node_id: inventory_node.node_id,
        power: Some(ScdFactorySyncBlackboardPower {
            power_cost: 0,
            power_gen: hub_data.power_generate,
            power_save_max: hub_data.power_storage_capacity,
            power_save_current: hub_data.power_storage_capacity,
            is_stop_by_power: false,
        }),
    };

    let region = ScdFactorySyncRegion {
        name: region_name.clone(),
        blackboard: Some(blackboard),
        nodes: vec![inventory_node, hub_node],
        scenes: vec![scene],
    };

    let quickbars: Vec<_> = ctx
        .player
        .factory
        .quickbars
        .iter()
        .map(|q| ScdFactorySyncQuickbar {
            r#type: q.quickbar_type.parse().unwrap_or(0),
            list: q.items.clone(),
        })
        .collect();

    let msg = ScFactorySyncContext {
        tms: perlica_logic::factory::current_tick() as i64,
        current_region: region_name.clone(),
        regions: vec![region],
        quickbars,
    };

    debug!(
        uid = %ctx.player.uid,
        scene = %scene_name,
        region = %region_name,
        region_id = %region_id,
        hub = HUB_TEMPLATE,
        top_left_x,
        top_left_y,
        hub_w,
        hub_h,
        power_gen = hub_data.power_generate,
        power_cap = hub_data.power_storage_capacity,
        "pushing factory context"
    );

    // The proto construction above could be replaced with region.to_proto()
    // `region.to_proto()` when `FactoryRegion::to_proto()` is implemented
    // (tasks 3.1-3.3). As the internal representation has already been
    // initialized via `FactoryManager::bootstrap_region`, this is just
    // a format change.

    ctx.notify(msg).await.is_ok()
}
