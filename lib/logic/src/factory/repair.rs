//! Repair & depot business logic.
#![allow(clippy::question_mark)]
//!
//! Repair unlocks pre-placed broken buildings by deducting their cost
//! items from the bag. Once repaired, the building becomes active in
//! the region. Depot bridge messages move items between the player's
//! bag and the factory depot (hub inventory).

use config::factory_table::FTableAssets;
use config::repair_table::RepairAssets;

use crate::factory::{FactoryComponent, FactoryManager, FactoryNode, take_bill_from_inv};

impl FactoryManager {
    /// Repair a pre-placed broken building. Looks up the repair entry,
    /// deducts cost items from the bag, and activates the building by
    /// placing it as a node in the region.
    pub fn repair_building(
        &mut self,
        repair_assets: &RepairAssets,
        factory_assets: &FTableAssets,
        region_name: &str,
        repair_id: &str,
    ) -> Option<u32> {
        let Some(entry) = repair_assets.get_repair(repair_id) else {
            return None;
        };

        // The whole cost, priced before the bag is touched. The old
        // per-item loop returned on the first shortfall having already
        // eaten everything ahead of it -- and the repair still failed.
        let cost: Vec<(&str, u32)> = entry
            .cost_items
            .iter()
            .map(|c| (c.id.as_str(), c.count))
            .collect();

        // Deduct cost items from the bag.
        let Some(region) = self.region_mut(region_name) else {
            return None;
        };
        let Some(bag_node) = region.node_mut(1) else {
            return None;
        };
        let Some(bag_comp) = bag_node.component_mut(1) else {
            return None;
        };
        let FactoryComponent::Inventory(bag_inv) = bag_comp else {
            return None;
        };

        if !take_bill_from_inv(bag_inv, &cost) {
            return None;
        }

        // Activate the building by placing it as a node.
        let building_id = &entry.building_id;
        let Some(building) = factory_assets.get_building(building_id) else {
            return None;
        };

        let node_type = crate::enums::building_node_type_from_i32(building.building_type);
        let Some(node_type) = node_type else {
            return None;
        };

        // Build components from the building template.
        let mut built = crate::factory::component_factory::create_components_from_template(
            building_id,
            factory_assets,
        )
        .unwrap_or(crate::factory::component_factory::BuiltComponents {
            components: vec![],
            component_pos: std::collections::HashMap::new(),
        });
        // Region-unique ids before the layout is attached; see
        // `BuiltComponents::rebase_ids`.
        built.rebase_ids(&mut region.next_component_id);

        let node_id = region.allocate_node_id();
        let node = FactoryNode {
            node_id,
            node_type,
            template_id: building_id.clone(),
            transform: crate::factory::NodeTransform {
                position: None, // pre-placed buildings have baked scene positions
                direction: crate::enums::FCDirection::Up,
                mesh: None,
                scene_name: entry.level_id.clone(),
                world_position: None,
                world_rotation: None,
                bc_port_in: building
                    .input_ports
                    .first()
                    .map(|p| crate::factory::SubPort {
                        position: crate::factory::GridPos {
                            x: p.point.x,
                            y: p.point.y,
                        },
                        direction: p.side,
                    }),
                bc_port_out: building
                    .output_ports
                    .first()
                    .map(|p| crate::factory::SubPort {
                        position: crate::factory::GridPos {
                            x: p.point.x,
                            y: p.point.y,
                        },
                        direction: p.side,
                    }),
            },
            is_deactive: false,
            interactive_object: Some(crate::factory::InteractiveObject { object_id: node_id }),
            dynamic_property: std::collections::HashMap::new(),
            component_pos: built.component_pos,
            components: built.components,
        };

        region.nodes.insert(node_id, node);

        // Power graph may have changed (new producer/pole/consumer).
        crate::factory::power::recompute_blackboard(region);

        // Remember which repair unlocked this node. `SCD_FACTORY_REPAIR.buildings`
        // is a list of `(repair_id, node_id)`, so without this the association is
        // lost the moment the op returns and the sync has nothing to report.
        self.repair_state.insert(
            crate::factory::MachineKey::new(region_name, node_id),
            repair_id.to_string(),
        );

        Some(node_id)
    }
}
