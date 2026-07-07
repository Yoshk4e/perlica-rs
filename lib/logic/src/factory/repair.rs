//! Repair & depot business logic.
#![allow(clippy::question_mark)]
//!
//! Repair unlocks pre-placed broken buildings by deducting their cost
//! items from the bag. Once repaired, the building becomes active in
//! the region. Depot bridge messages move items between the player's
//! bag and the factory depot (hub inventory).

use config::factory_table::FTableAssets;
use config::repair_table::RepairAssets;

use crate::enums::FCNodeType;
use crate::factory::{FactoryComponent, FactoryManager, FactoryNode};

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

        // Verify + consume cost items.
        for cost in &entry.cost_items {
            if !try_consume_from_inv(bag_inv, &cost.id, cost.count) {
                return None;
            }
        }

        // Activate the building by placing it as a node.
        let building_id = &entry.building_id;
        let Some(building) = factory_assets.get_building(building_id) else {
            return None;
        };

        let node_type = node_type_from_i32(building.building_type);
        let Some(node_type) = node_type else {
            return None;
        };

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
            component_pos: std::collections::HashMap::new(),
            components: vec![],
        };

        region.nodes.insert(node_id, node);
        Some(node_id)
    }
}

fn node_type_from_i32(t: i32) -> Option<FCNodeType> {
    Some(match t {
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
        _ => return None,
    })
}

fn try_consume_from_inv(
    inv: &mut crate::factory::InventoryState,
    item_id: &str,
    needed: u32,
) -> bool {
    let mut remaining = needed;
    let mut to_remove = vec![];

    for (&inst_id, slot) in &mut inv.items {
        if slot.item_id == item_id && slot.count > 0 {
            let take = slot.count.min(remaining);
            slot.count -= take;
            remaining -= take;
            if slot.count == 0 {
                to_remove.push(inst_id);
            }
            if remaining == 0 {
                break;
            }
        }
    }

    for inst_id in to_remove {
        inv.items.remove(&inst_id);
    }

    remaining == 0
}
