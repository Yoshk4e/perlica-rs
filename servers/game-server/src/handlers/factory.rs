use std::collections::HashMap;

use crate::net::NetContext;
use perlica_proto::{
    ScFactorySyncContext, ScdFactoryRectInt, ScdFactorySubPort, ScdFactorySyncBlackboard,
    ScdFactorySyncBlackboardPower, ScdFactorySyncComponent, ScdFactorySyncComponentInventory,
    ScdFactorySyncDynamicProperty, ScdFactorySyncInteractiveObject, ScdFactorySyncMesh,
    ScdFactorySyncNode, ScdFactorySyncRegion, ScdFactorySyncScene, ScdFactorySyncSceneBandwidth,
    ScdFactorySyncTransform, ScdFactoryVector2Int, Vector, scd_factory_sync_component,
};
use tracing::debug;

// Thanks xeondev for pointing this out
//
// Hardcoded `sp_hub_1` factory bootstrap data sourced from the assets/tables/* files
// shipped with the repository:
//   - assets/tables/FactoryTable.json
//       * buildingData["sp_hub_1"]            -> type=1, range 9x9, powerConsume=0,
//                                                inputPorts (14, sides 0/2), outputPorts (6, sides 1/3),
//                                                bandwidth=0, onlyShowOnMain=true, canDelete=false
//       * hubData["sp_hub_1"]                 -> powerStorageCapacity=100000, powerGenerate=100
//       * buildingItemReverseData["sp_hub_1"] -> item_port_sp_hub_1
//       * regionData / levelRegionData        -> map01_lv001 <-> region_102
//   - assets/tables/FactoryMapTable.json
//       * map01_lv001 level 1 -> posX=17, posY=-36, rangeW=36, rangeH=36
//   - lib/logic/src/player.rs                 -> default last_scene = "map01_lv001"
pub async fn push_factory(ctx: &mut NetContext<'_>) -> bool {
    const REGION_NAME: &str = "test01";
    const SCENE_NAME: &str = "test01";
    const HUB_TEMPLATE: &str = "sp_hub_1";
    let hub_mesh = ScdFactorySyncMesh {
        mesh_type: 0,
        points: vec![
            ScdFactoryVector2Int { x: 0, y: 0 },
            ScdFactoryVector2Int { x: 9, y: 0 },
            ScdFactoryVector2Int { x: 9, y: 9 },
            ScdFactoryVector2Int { x: 0, y: 9 },
        ],
    };
    let bc_port_in = ScdFactorySubPort {
        position: Some(ScdFactoryVector2Int { x: 1, y: 8 }),
        direction: 2,
    };
    let bc_port_out = ScdFactorySubPort {
        position: Some(ScdFactoryVector2Int { x: 8, y: 1 }),
        direction: 1,
    };
    let hub_node = ScdFactorySyncNode {
        node_id: 1,
        node_type: 1,
        template_id: HUB_TEMPLATE.to_string(),
        transform: Some(ScdFactorySyncTransform {
            position: Some(ScdFactoryVector2Int { x: 0, y: 0 }),
            direction: 0,
            mesh: Some(hub_mesh),
            scene_name: SCENE_NAME.to_string(),
            world_position: Some(Vector {
                x: 480.00,
                y: 107.11,
                z: 217.83,
            }),
            world_rotation: Some(Vector {
                x: 0.0,
                y: 60.0,
                z: 0.0,
            }),
            bc_port_in: Some(bc_port_in),
            bc_port_out: Some(bc_port_out),
        }),
        is_deactive: false,
        interactive_object: Some(ScdFactorySyncInteractiveObject { object_id: 1 }),
        dynamic_property: Some(ScdFactorySyncDynamicProperty {
            values: HashMap::new(),
        }),
        component_pos: {
            let mut m = HashMap::new();
            m.insert(10, 1u32);
            m
        },
        components: vec![ScdFactorySyncComponent {
            component_id: 1,
            component_type: 10,
            component_payload: Some(scd_factory_sync_component::ComponentPayload::Inventory(
                ScdFactorySyncComponentInventory {
                    items: HashMap::new(),
                },
            )),
        }],
    };

    let main_mesh = vec![ScdFactoryRectInt {
        x: 17,
        y: -36,
        w: 36,
        h: 36,
    }];

    let scene = ScdFactorySyncScene {
        name: ctx.player.world.last_scene.clone(),
        level: 1,
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
        inventory_node_id: hub_node.node_id,
        power: Some(ScdFactorySyncBlackboardPower {
            power_cost: 0,
            power_gen: 100,
            power_save_max: 100_000,
            power_save_current: 100_000,
            is_stop_by_power: false,
        }),
    };

    let region = ScdFactorySyncRegion {
        name: REGION_NAME.to_string(),
        blackboard: Some(blackboard),
        nodes: vec![hub_node],
        scenes: vec![scene],
    };

    let msg = ScFactorySyncContext {
        tms: 0,
        current_region: REGION_NAME.to_string(),
        regions: vec![region],
        quickbars: vec![],
    };

    debug!(
        "Pushing factory context: uid={}, regions={}, hub_template={}",
        ctx.player.uid,
        msg.regions.len(),
        HUB_TEMPLATE,
    );

    ctx.notify(msg).await.is_ok()
}
