//! Factory op business logic, implemented as methods on `FactoryManager`.
//!
//! The manager owns all the state these ops need -- regions, nodes,
//! connections, machine tables. Config assets are passed as parameters
//! since the manager doesn't (and shouldn't) own config references.
//!
//! Error types are defined alongside the methods so the handler layer
//! can match on them and map to the right `FactoryOpRetCode`.

use std::collections::HashMap;

use config::factory_map::FRegionAssets;
use config::factory_table::FTableAssets;

use crate::enums::{FCConnectionPortType, FCConnectionType, FCDirection, FCNodeType};
use crate::factory::component_factory::create_components_from_template;
use crate::factory::grid::{is_in_bounds, range_within, ranges_overlap};
use crate::factory::tick::elapsed_since;
use crate::factory::{
    CollectorState, FactoryComponent, FactoryConnection, FactoryManager, FactoryNode,
    FactoryRegion, GridBoxState, GridPos, GridRange, ItemSlot, Mesh, NodeTransform,
};

#[derive(Debug)]
pub enum PlaceError {
    RegionNotFound,
    NoBuildingEntry,
    InvalidNodeType,
    OutOfBounds,
    Overlaps(u32),
    NoComponentLayout,
}

#[derive(Debug)]
pub enum DismantleError {
    RegionNotFound,
    NodeNotFound,
    ReservedNode,
}

#[derive(Debug)]
pub enum MoveNodeError {
    RegionNotFound,
    NodeNotFound,
    NoGridPosition,
    NoBuildingEntry,
    OutOfBounds,
    Overlaps(u32),
}

#[derive(Debug)]
pub enum EnableNodeError {
    RegionNotFound,
    NodeNotFound,
}

#[derive(Debug)]
pub enum TargetError {
    RegionNotFound,
    ComponentNotFound,
    WrongComponentType,
}

#[derive(Debug)]
pub enum ConveyorError {
    RegionNotFound,
    NoBuildingEntry,
    OutOfBounds,
    Overlaps(u32),
    NoFromTo,
}

#[derive(Debug)]
pub enum GridBoxError {
    RegionNotFound,
    ComponentNotFound,
    WrongComponentType,
    IndexOutOfRange,
    SlotEmpty,
    InventoryNodeMissing,
    HubNodeMissing,
    InventoryComponentMissing,
    ItemNotFound,
}

#[derive(Debug, Clone)]
pub struct HsFbEntry {
    pub component_id: u32,
    pub payload: HsFbPayload,
}

#[derive(Debug, Clone)]
pub enum HsFbPayload {
    Cache {
        items: Vec<u32>,
    },
    Producer {
        progress_incr_per_ms: i64,
        formula_id: String,
        current_progress: i64,
    },
    Collector {
        progress_incr_per_ms: i64,
        current_progress: i64,
    },
    BurnPower {
        progress_decr_per_ms: i64,
        current_least_progress: i64,
    },
    CacheTransport {
        progress_incr_per_ms: i64,
        current_progress: i64,
    },
    GridBox {
        items: Vec<u32>,
    },
    BoxRouterM1 {
        items: Vec<u32>,
    },
    BoxBridge {
        items: Vec<u32>,
    },
    HealTower {
        progress_incr_per_ms: i64,
        current_progress: i64,
        current_point: i32,
    },
}

impl FactoryManager {
    pub fn place(
        &mut self,
        assets: &FTableAssets,
        factory_map: &FRegionAssets,
        region_name: &str,
        template_id: &str,
        position: GridPos,
        direction: i32,
    ) -> Result<u32, PlaceError> {
        let building = assets
            .get_building(template_id)
            .ok_or(PlaceError::NoBuildingEntry)?;
        let node_type =
            node_type_from_i32(building.building_type).ok_or(PlaceError::InvalidNodeType)?;

        let footprint = GridRange {
            x: position.x,
            y: position.y,
            w: building.range.width,
            h: building.range.height,
        };

        let scene_name = {
            let region = self
                .region_mut(region_name)
                .ok_or(PlaceError::RegionNotFound)?;
            let map = factory_map
                .get(&region.scene_name, region.level)
                .ok_or(PlaceError::OutOfBounds)?;
            let main_mesh = GridRange {
                x: map.pos_x,
                y: map.pos_y,
                w: map.range_w,
                h: map.range_h,
            };
            if !range_within(footprint, main_mesh) {
                return Err(PlaceError::OutOfBounds);
            }
            for other in region.nodes.values() {
                let Some(other_pos) = other.transform.position else {
                    continue;
                };
                let Some(other_b) = assets.get_building(&other.template_id) else {
                    continue;
                };
                let other_fp = GridRange {
                    x: other_pos.x,
                    y: other_pos.y,
                    w: other_b.range.width,
                    h: other_b.range.height,
                };
                if ranges_overlap(footprint, other_fp) {
                    return Err(PlaceError::Overlaps(other.node_id));
                }
            }
            region.scene_name.clone()
        };

        let built = create_components_from_template(template_id, assets)
            .ok_or(PlaceError::NoComponentLayout)?;

        // Extract port layout from the building entry so the client
        // knows where belt I/O points are.
        let bc_port_in = building
            .input_ports
            .first()
            .map(|p| crate::factory::SubPort {
                position: GridPos {
                    x: p.point.x,
                    y: p.point.y,
                },
                direction: p.side,
            });
        let bc_port_out = building
            .output_ports
            .first()
            .map(|p| crate::factory::SubPort {
                position: GridPos {
                    x: p.point.x,
                    y: p.point.y,
                },
                direction: p.side,
            });

        let node_id = {
            let region = self.region_mut(region_name).unwrap();
            let node_id = region.allocate_node_id();
            let mesh = Mesh {
                mesh_type: crate::enums::FCMeshType::Rect,
                points: rect_mesh_points(footprint),
            };
            let node = FactoryNode {
                node_id,
                node_type,
                template_id: template_id.to_string(),
                transform: NodeTransform {
                    position: Some(position),
                    direction: direction_from_i32(direction),
                    mesh: Some(mesh),
                    scene_name,
                    world_position: None,
                    world_rotation: None,
                    bc_port_in,
                    bc_port_out,
                },
                is_deactive: false,
                // Assign a sequential interactive_object ID so the client
                // can route clicks. IDs 1 and 2 are reserved by the hub.
                interactive_object: Some(crate::factory::InteractiveObject { object_id: node_id }),
                dynamic_property: HashMap::new(),
                component_pos: built.component_pos,
                components: built.components,
            };
            region.nodes.insert(node_id, node);
            node_id
        };

        // Power graph may have changed (new producer/pole/consumer).
        if let Some(region) = self.region_mut(region_name) {
            crate::factory::power::recompute_blackboard(region);
        }

        Ok(node_id)
    }

    pub fn dismantle(
        &mut self,
        assets: &FTableAssets,
        region_name: &str,
        node_id: u32,
    ) -> Result<String, DismantleError> {
        let region = self
            .region_mut(region_name)
            .ok_or(DismantleError::RegionNotFound)?;

        let template_id = match region.node(node_id) {
            Some(n) if n.node_id <= 2 => return Err(DismantleError::ReservedNode),
            Some(n) => n.template_id.clone(),
            None => return Err(DismantleError::NodeNotFound),
        };

        region.nodes.remove(&node_id);
        region
            .connections
            .retain(|c| c.node_id_a != node_id && c.node_id_b != node_id);

        // Return the building's source item to the player's bag.
        if let Some(entry) = assets.item_for_building(&template_id)
            && let Some(inv_node) = region.node_mut(1)
            && let Some(slot) = inv_node.component_mut(1)
            && let crate::factory::FactoryComponent::Inventory(inv_state) = slot
        {
            let item_id = entry.item_id.clone();
            if let Some(existing) = inv_state.items.get_mut(&0) {
                if existing.item_id == item_id {
                    existing.count += 1;
                }
            } else {
                inv_state.items.insert(
                    0,
                    crate::factory::ItemSlot {
                        item_id,
                        count: 1,
                        inst_id: 0,
                    },
                );
            }
        }

        // Power graph may have changed (removed producer/pole/consumer).
        crate::factory::power::recompute_blackboard(region);

        Ok(template_id)
    }

    pub fn move_node(
        &mut self,
        assets: &FTableAssets,
        factory_map: &FRegionAssets,
        region_name: &str,
        node_id: u32,
        position: GridPos,
        direction: i32,
    ) -> Result<(), MoveNodeError> {
        let (_template_id, footprint, building_w, building_h) = {
            let region = self
                .region(region_name)
                .ok_or(MoveNodeError::RegionNotFound)?;
            let node = region.node(node_id).ok_or(MoveNodeError::NodeNotFound)?;
            if node.transform.position.is_none() {
                return Err(MoveNodeError::NoGridPosition);
            }
            let building = assets
                .get_building(&node.template_id)
                .ok_or(MoveNodeError::NoBuildingEntry)?;
            let footprint = GridRange {
                x: position.x,
                y: position.y,
                w: building.range.width,
                h: building.range.height,
            };
            (
                node.template_id.clone(),
                footprint,
                building.range.width,
                building.range.height,
            )
        };

        {
            let region = self
                .region_mut(region_name)
                .ok_or(MoveNodeError::RegionNotFound)?;
            let map = factory_map
                .get(&region.scene_name, region.level)
                .ok_or(MoveNodeError::OutOfBounds)?;
            let main_mesh = GridRange {
                x: map.pos_x,
                y: map.pos_y,
                w: map.range_w,
                h: map.range_h,
            };
            if !range_within(footprint, main_mesh) {
                return Err(MoveNodeError::OutOfBounds);
            }
            for other in region.nodes.values() {
                if other.node_id == node_id {
                    continue;
                }
                let Some(other_pos) = other.transform.position else {
                    continue;
                };
                let Some(other_b) = assets.get_building(&other.template_id) else {
                    continue;
                };
                let other_fp = GridRange {
                    x: other_pos.x,
                    y: other_pos.y,
                    w: other_b.range.width,
                    h: other_b.range.height,
                };
                if ranges_overlap(footprint, other_fp) {
                    return Err(MoveNodeError::Overlaps(other.node_id));
                }
            }

            let node = region.node_mut(node_id).unwrap();
            node.transform.position = Some(position);
            node.transform.direction = direction_from_i32(direction);
            node.transform.mesh = Some(Mesh {
                mesh_type: crate::enums::FCMeshType::Rect,
                points: rect_mesh_points(GridRange {
                    x: footprint.x,
                    y: footprint.y,
                    w: building_w,
                    h: building_h,
                }),
            });
        }

        Ok(())
    }

    pub fn enable_node(
        &mut self,
        region_name: &str,
        node_id: u32,
        enable: bool,
    ) -> Result<(), EnableNodeError> {
        let region = self
            .region_mut(region_name)
            .ok_or(EnableNodeError::RegionNotFound)?;
        let node = region
            .node_mut(node_id)
            .ok_or(EnableNodeError::NodeNotFound)?;

        node.is_deactive = !enable;

        for (_, comp) in &mut node.components {
            if let FactoryComponent::Producer(state) = comp
                && !enable
                && let Some(start) = state.start_tick.take()
            {
                let elapsed = crate::factory::tick::elapsed_since(start);
                state.current_progress = state.current_progress.saturating_add(elapsed);
            }
        }

        Ok(())
    }

    pub fn add_connection(
        &mut self,
        region_name: &str,
        conn_type: FCConnectionType,
        node_id_a: u32,
        node_id_b: u32,
    ) -> Result<u64, &'static str> {
        let region = self.region_mut(region_name).ok_or("region not found")?;
        if !region.nodes.contains_key(&node_id_a) || !region.nodes.contains_key(&node_id_b) {
            return Err("one or both endpoints missing");
        }
        let port_type = match conn_type {
            FCConnectionType::Power => FCConnectionPortType::PowerPole,
            FCConnectionType::Travel => FCConnectionPortType::Hub,
        };
        let idx = region.next_connection_index();
        region.connections.push(FactoryConnection {
            connection_type: conn_type,
            port_type,
            node_id_a,
            node_id_b,
            index: idx,
        });
        crate::factory::power::recompute_blackboard(region);
        Ok(idx)
    }

    pub fn del_connection(&mut self, region_name: &str, index: u64) -> Result<(), &'static str> {
        let region = self.region_mut(region_name).ok_or("region not found")?;
        let before = region.connections.len();
        region.connections.retain(|c| c.index != index);
        if region.connections.len() == before {
            return Err("no connection with that index");
        }
        crate::factory::power::recompute_blackboard(region);
        Ok(())
    }

    pub fn set_select_target(
        &mut self,
        region_name: &str,
        component_id: u32,
        item_id: String,
    ) -> Result<(), TargetError> {
        let region = self
            .region_mut(region_name)
            .ok_or(TargetError::RegionNotFound)?;
        for node in region.nodes.values_mut() {
            if let Some(slot) = node.component_mut(component_id) {
                if let FactoryComponent::Selector(state) = slot {
                    state.selected_item_id = item_id;
                    return Ok(());
                }
                return Err(TargetError::WrongComponentType);
            }
        }
        Err(TargetError::ComponentNotFound)
    }

    pub fn set_collect_target(
        &mut self,
        region_name: &str,
        component_id: u32,
        item_id: String,
        _count: i32,
    ) -> Result<(), TargetError> {
        let region = self
            .region_mut(region_name)
            .ok_or(TargetError::RegionNotFound)?;
        for node in region.nodes.values_mut() {
            if let Some(slot) = node.component_mut(component_id) {
                if let FactoryComponent::Collector(state) = slot {
                    set_collector_target(state, &item_id);
                    return Ok(());
                }
                return Err(TargetError::WrongComponentType);
            }
        }
        Err(TargetError::ComponentNotFound)
    }

    pub fn cache_transport_enable(
        &mut self,
        region_name: &str,
        component_id: u32,
        enable: bool,
    ) -> Result<(), TargetError> {
        let region = self
            .region_mut(region_name)
            .ok_or(TargetError::RegionNotFound)?;
        for node in region.nodes.values_mut() {
            if let Some(slot) = node.component_mut(component_id) {
                if let FactoryComponent::CacheTransport(state) = slot {
                    state.enabled = enable;
                    return Ok(());
                }
                return Err(TargetError::WrongComponentType);
            }
        }
        Err(TargetError::ComponentNotFound)
    }

    pub fn cache_transport_transfer(
        &mut self,
        region_name: &str,
        component_id: u32,
    ) -> Result<bool, TargetError> {
        // Find the CacheTransport component and read source/target IDs.
        let (source_id, target_id) = {
            let region = self
                .region(region_name)
                .ok_or(TargetError::RegionNotFound)?;
            let mut found = None;
            for node in region.nodes.values() {
                if let Some(slot) = node.component(component_id) {
                    if let FactoryComponent::CacheTransport(state) = slot {
                        if !state.enabled {
                            return Ok(false);
                        }
                        found = Some((state.source_node_id, state.target_node_id));
                        break;
                    }
                    return Err(TargetError::WrongComponentType);
                }
            }
            found.ok_or(TargetError::ComponentNotFound)?
        };

        // Pull one item from the source node's Cache, push to target's.
        let region = self
            .region_mut(region_name)
            .ok_or(TargetError::RegionNotFound)?;

        // Find and remove one item from the source cache.
        let mut moved_item = None;
        for node in region.nodes.values_mut() {
            if node.node_id == source_id {
                for (_, comp) in &mut node.components {
                    if let FactoryComponent::Cache(state) = comp
                        && let Some(slot) = state.items.first().cloned()
                        && slot.count > 0
                    {
                        let take = ItemSlot {
                            item_id: slot.item_id.clone(),
                            count: 1,
                            inst_id: slot.inst_id,
                        };
                        if let Some(first) = state.items.first_mut() {
                            first.count = first.count.saturating_sub(1);
                            if first.count == 0 {
                                state.items.remove(0);
                            }
                        }
                        moved_item = Some(take);
                        break;
                    }
                }
                break;
            }
        }

        let Some(item) = moved_item else {
            return Ok(false);
        };

        // Push into target cache.
        for node in region.nodes.values_mut() {
            if node.node_id == target_id {
                for (_, comp) in &mut node.components {
                    if let FactoryComponent::Cache(state) = comp {
                        if let Some(existing) =
                            state.items.iter_mut().find(|s| s.item_id == item.item_id)
                        {
                            existing.count += item.count;
                        } else {
                            state.items.push(item);
                        }
                        return Ok(true);
                    }
                }
                break;
            }
        }

        Ok(false)
    }

    pub fn use_heal_tower(
        &mut self,
        region_name: &str,
        component_id: u32,
        use_count: u32,
    ) -> Result<u32, TargetError> {
        let region = self
            .region_mut(region_name)
            .ok_or(TargetError::RegionNotFound)?;
        for node in region.nodes.values_mut() {
            if let Some(slot) = node.component_mut(component_id) {
                if let FactoryComponent::HealTower(state) = slot {
                    let requested = use_count as i64;
                    let granted = requested.min(state.points.max(0));
                    state.points -= granted;
                    return Ok(granted as u32);
                }
                return Err(TargetError::WrongComponentType);
            }
        }
        Err(TargetError::ComponentNotFound)
    }

    pub fn set_travel_pole_next(
        &mut self,
        region_name: &str,
        component_id: u32,
        default_next: u32,
    ) -> Result<(), TargetError> {
        let region = self
            .region_mut(region_name)
            .ok_or(TargetError::RegionNotFound)?;
        if !region.nodes.contains_key(&default_next) {
            return Err(TargetError::ComponentNotFound);
        }
        for node in region.nodes.values_mut() {
            if let Some(slot) = node.component_mut(component_id) {
                if let FactoryComponent::TravelPole(state) = slot {
                    state.default_next = Some(default_next);
                    return Ok(());
                }
                return Err(TargetError::WrongComponentType);
            }
        }
        Err(TargetError::ComponentNotFound)
    }

    pub fn place_box_conveyor(
        &mut self,
        assets: &FTableAssets,
        factory_map: &FRegionAssets,
        region_name: &str,
        template_id: &str,
        direction_out: i32,
        points: &[GridPos],
    ) -> Result<Vec<u32>, ConveyorError> {
        if points.is_empty() {
            return Err(ConveyorError::NoFromTo);
        }
        let building = assets
            .get_building(template_id)
            .ok_or(ConveyorError::NoBuildingEntry)?;

        {
            let region = self
                .region_mut(region_name)
                .ok_or(ConveyorError::RegionNotFound)?;
            let map = factory_map
                .get(&region.scene_name, region.level)
                .ok_or(ConveyorError::OutOfBounds)?;
            let main_mesh = GridRange {
                x: map.pos_x,
                y: map.pos_y,
                w: map.range_w,
                h: map.range_h,
            };

            for p in points {
                let tile = GridRange {
                    x: p.x,
                    y: p.y,
                    w: 1,
                    h: 1,
                };
                if !range_within(tile, main_mesh) {
                    return Err(ConveyorError::OutOfBounds);
                }
                for other in region.nodes.values() {
                    let Some(other_pos) = other.transform.position else {
                        continue;
                    };
                    let Some(other_b) = assets.get_building(&other.template_id) else {
                        continue;
                    };
                    let other_fp = GridRange {
                        x: other_pos.x,
                        y: other_pos.y,
                        w: other_b.range.width,
                        h: other_b.range.height,
                    };
                    if ranges_overlap(tile, other_fp) {
                        return Err(ConveyorError::Overlaps(other.node_id));
                    }
                }
            }
        }

        let mut node_ids = Vec::with_capacity(points.len());
        {
            let region = self.region_mut(region_name).unwrap();
            let scene_name = region.scene_name.clone();
            for p in points {
                let node_id = region.allocate_node_id();
                let node = FactoryNode {
                    node_id,
                    node_type: FCNodeType::BoxConveyor,
                    template_id: template_id.to_string(),
                    transform: NodeTransform {
                        position: Some(*p),
                        direction: FCDirection::Up,
                        mesh: Some(Mesh {
                            mesh_type: crate::enums::FCMeshType::Rect,
                            points: rect_mesh_points(GridRange {
                                x: p.x,
                                y: p.y,
                                w: 1,
                                h: 1,
                            }),
                        }),
                        scene_name: scene_name.clone(),
                        world_position: None,
                        world_rotation: None,
                        bc_port_in: None,
                        bc_port_out: None,
                    },
                    is_deactive: false,
                    interactive_object: Some(crate::factory::InteractiveObject {
                        object_id: node_id,
                    }),
                    dynamic_property: HashMap::new(),
                    component_pos: HashMap::new(),
                    components: vec![(
                        1,
                        FactoryComponent::BoxConveyor(crate::factory::BoxConveyorState {
                            items: vec![],
                            direction: direction_out,
                        }),
                    )],
                };
                region.nodes.insert(node_id, node);
                node_ids.push(node_id);
            }
        }

        let _ = building;
        Ok(node_ids)
    }

    pub fn dismantle_box_conveyor(
        &mut self,
        region_name: &str,
        from: GridPos,
        to: GridPos,
    ) -> Result<Vec<u32>, ConveyorError> {
        let region = self
            .region_mut(region_name)
            .ok_or(ConveyorError::RegionNotFound)?;
        let bbox = GridRange {
            x: from.x.min(to.x),
            y: from.y.min(to.y),
            w: (from.x - to.x).unsigned_abs() + 1,
            h: (from.y - to.y).unsigned_abs() + 1,
        };
        let to_remove: Vec<u32> = region
            .nodes
            .values()
            .filter(|n| n.node_type == FCNodeType::BoxConveyor)
            .filter(|n| n.transform.position.is_some_and(|p| is_in_bounds(p, bbox)))
            .map(|n| n.node_id)
            .collect();
        for id in &to_remove {
            region.nodes.remove(id);
        }
        region
            .connections
            .retain(|c| !to_remove.contains(&c.node_id_a) && !to_remove.contains(&c.node_id_b));
        Ok(to_remove)
    }

    pub fn gridbox_inner_move(
        &mut self,
        region_name: &str,
        component_id: u32,
        from_index: i32,
        to_index: i32,
    ) -> Result<(), GridBoxError> {
        let region = self
            .region_mut(region_name)
            .ok_or(GridBoxError::RegionNotFound)?;
        let state = find_gridbox(region, component_id).ok_or(GridBoxError::ComponentNotFound)?;
        if from_index < 0 || to_index < 0 {
            return Err(GridBoxError::IndexOutOfRange);
        }
        let from = from_index as usize;
        let to = to_index as usize;
        if from >= state.items.len() {
            return Err(GridBoxError::IndexOutOfRange);
        }
        if to >= state.items.len() {
            state.items.resize(
                to + 1,
                ItemSlot {
                    item_id: String::new(),
                    count: 0,
                    inst_id: 0,
                },
            );
            let moved = state.items[from].clone();
            state.items[to] = moved;
            state.items[from] = ItemSlot {
                item_id: String::new(),
                count: 0,
                inst_id: 0,
            };
        } else if state.items[to].item_id == state.items[from].item_id
            && state.items[to].inst_id == state.items[from].inst_id
            && !state.items[to].item_id.is_empty()
        {
            state.items[to].count += state.items[from].count;
            state.items[from] = ItemSlot {
                item_id: String::new(),
                count: 0,
                inst_id: 0,
            };
        } else {
            let (left, right) = state.items.split_at_mut(from.max(to));
            let split = from.max(to);
            let (lower_idx, higher_idx) = if from < to {
                (from, 0) // from is in left, to is right[0]
            } else {
                (to, from - split) // to is in left, from is right[from-split]
            };
            std::mem::swap(&mut left[lower_idx], &mut right[higher_idx]);
        }
        Ok(())
    }

    pub fn gridbox_inner_split(
        &mut self,
        region_name: &str,
        component_id: u32,
        from_index: i32,
        to_index: i32,
        count: i32,
    ) -> Result<(), GridBoxError> {
        let region = self
            .region_mut(region_name)
            .ok_or(GridBoxError::RegionNotFound)?;
        let state = find_gridbox(region, component_id).ok_or(GridBoxError::ComponentNotFound)?;
        if from_index < 0 || to_index < 0 || count < 0 {
            return Err(GridBoxError::IndexOutOfRange);
        }
        let from = from_index as usize;
        let to = to_index as usize;
        let count = count as u32;

        if from >= state.items.len() || state.items[from].count < count {
            return Err(GridBoxError::IndexOutOfRange);
        }
        if to >= state.items.len() {
            state.items.resize(
                to + 1,
                ItemSlot {
                    item_id: String::new(),
                    count: 0,
                    inst_id: 0,
                },
            );
        }
        let same_dest = state.items[to].item_id == state.items[from].item_id
            && state.items[to].inst_id == state.items[from].inst_id;
        let empty_dest = state.items[to].item_id.is_empty();
        if !same_dest && !empty_dest {
            return Err(GridBoxError::IndexOutOfRange);
        }
        state.items[from].count -= count;
        let (src_item_id, src_inst_id) =
            (state.items[from].item_id.clone(), state.items[from].inst_id);
        state.items[to].item_id = src_item_id;
        state.items[to].inst_id = src_inst_id;
        state.items[to].count += count;
        if state.items[from].count == 0 {
            state.items[from] = ItemSlot {
                item_id: String::new(),
                count: 0,
                inst_id: 0,
            };
        }
        Ok(())
    }

    pub fn move_bag_to_gridbox(
        &mut self,
        region_name: &str,
        component_id: u32,
        bag_grid_index: i32,
        grid_box_index: i32,
    ) -> Result<(), GridBoxError> {
        let region = self
            .region_mut(region_name)
            .ok_or(GridBoxError::RegionNotFound)?;
        let bag_item = {
            let inv_node = region
                .node_mut(1)
                .ok_or(GridBoxError::InventoryNodeMissing)?;
            let slot = inv_node
                .component_mut(1)
                .ok_or(GridBoxError::InventoryComponentMissing)?;
            let FactoryComponent::Inventory(inv_state) = slot else {
                return Err(GridBoxError::InventoryComponentMissing);
            };
            inv_state.items.remove(&(bag_grid_index as u32))
        };
        let item = bag_item.ok_or(GridBoxError::SlotEmpty)?;
        let gridbox = find_gridbox(region, component_id).ok_or(GridBoxError::ComponentNotFound)?;
        let idx = grid_box_index as usize;
        if idx >= gridbox.items.len() {
            gridbox.items.resize(
                idx + 1,
                ItemSlot {
                    item_id: String::new(),
                    count: 0,
                    inst_id: 0,
                },
            );
        }
        if gridbox.items[idx].item_id == item.item_id && gridbox.items[idx].count > 0 {
            gridbox.items[idx].count += item.count;
        } else {
            gridbox.items[idx] = item;
        }
        Ok(())
    }

    pub fn move_gridbox_to_bag(
        &mut self,
        region_name: &str,
        component_id: u32,
        grid_box_index: i32,
        bag_grid_index: i32,
    ) -> Result<(), GridBoxError> {
        let region = self
            .region_mut(region_name)
            .ok_or(GridBoxError::RegionNotFound)?;
        let gridbox = find_gridbox(region, component_id).ok_or(GridBoxError::ComponentNotFound)?;
        if grid_box_index < 0 || grid_box_index as usize >= gridbox.items.len() {
            return Err(GridBoxError::IndexOutOfRange);
        }
        let item = std::mem::replace(
            &mut gridbox.items[grid_box_index as usize],
            ItemSlot {
                item_id: String::new(),
                count: 0,
                inst_id: 0,
            },
        );
        if item.count == 0 {
            return Err(GridBoxError::SlotEmpty);
        }
        let inv_node = region
            .node_mut(1)
            .ok_or(GridBoxError::InventoryNodeMissing)?;
        let slot = inv_node
            .component_mut(1)
            .ok_or(GridBoxError::InventoryComponentMissing)?;
        let FactoryComponent::Inventory(inv_state) = slot else {
            return Err(GridBoxError::InventoryComponentMissing);
        };
        inv_state.items.insert(bag_grid_index as u32, item);
        Ok(())
    }

    pub fn move_depot_to_gridbox(
        &mut self,
        region_name: &str,
        component_id: u32,
        item_id: &str,
        grid_box_index: i32,
    ) -> Result<(), GridBoxError> {
        let region = self
            .region_mut(region_name)
            .ok_or(GridBoxError::RegionNotFound)?;
        let depot_item = {
            let hub_node = region.node_mut(2).ok_or(GridBoxError::HubNodeMissing)?;
            let slot = hub_node
                .component_mut(8)
                .ok_or(GridBoxError::InventoryComponentMissing)?;
            let FactoryComponent::Inventory(inv_state) = slot else {
                return Err(GridBoxError::InventoryComponentMissing);
            };
            let mut found = None;
            for (&inst_id, s) in &mut inv_state.items {
                if s.item_id == item_id && s.count > 0 {
                    s.count -= 1;
                    found = Some(ItemSlot {
                        item_id: item_id.to_string(),
                        count: 1,
                        inst_id,
                    });
                    if s.count == 0 {
                        inv_state.items.remove(&inst_id);
                    }
                    break;
                }
            }
            found
        };
        let item = depot_item.ok_or(GridBoxError::ItemNotFound)?;
        let gridbox = find_gridbox(region, component_id).ok_or(GridBoxError::ComponentNotFound)?;
        let idx = grid_box_index as usize;
        if idx >= gridbox.items.len() {
            gridbox.items.resize(
                idx + 1,
                ItemSlot {
                    item_id: String::new(),
                    count: 0,
                    inst_id: 0,
                },
            );
        }
        if gridbox.items[idx].item_id == item.item_id && gridbox.items[idx].count > 0 {
            gridbox.items[idx].count += item.count;
        } else {
            gridbox.items[idx] = item;
        }
        Ok(())
    }

    pub fn move_gridbox_to_depot(
        &mut self,
        region_name: &str,
        component_id: u32,
        grid_box_index: i32,
    ) -> Result<(), GridBoxError> {
        let region = self
            .region_mut(region_name)
            .ok_or(GridBoxError::RegionNotFound)?;
        let gridbox = find_gridbox(region, component_id).ok_or(GridBoxError::ComponentNotFound)?;
        if grid_box_index < 0 || grid_box_index as usize >= gridbox.items.len() {
            return Err(GridBoxError::IndexOutOfRange);
        }
        let item = std::mem::replace(
            &mut gridbox.items[grid_box_index as usize],
            ItemSlot {
                item_id: String::new(),
                count: 0,
                inst_id: 0,
            },
        );
        if item.count == 0 {
            return Err(GridBoxError::SlotEmpty);
        }
        let hub_node = region.node_mut(2).ok_or(GridBoxError::HubNodeMissing)?;
        let slot = hub_node
            .component_mut(8)
            .ok_or(GridBoxError::InventoryComponentMissing)?;
        let FactoryComponent::Inventory(inv_state) = slot else {
            return Err(GridBoxError::InventoryComponentMissing);
        };
        let mut stacked = false;
        for existing in inv_state.items.values_mut() {
            if existing.item_id == item.item_id && existing.inst_id == item.inst_id {
                existing.count += item.count;
                stacked = true;
                break;
            }
        }
        if !stacked {
            inv_state.items.insert(item.inst_id, item);
        }
        Ok(())
    }

    pub fn move_cache_to_cache(
        &mut self,
        region_name: &str,
        from_component_id: u32,
        to_component_id: u32,
        item_id: &str,
    ) -> Result<(), GridBoxError> {
        let region = self
            .region_mut(region_name)
            .ok_or(GridBoxError::RegionNotFound)?;
        let from_items = {
            let mut found = None;
            for node in region.nodes.values_mut() {
                if let Some(slot) = node.component_mut(from_component_id)
                    && let FactoryComponent::Cache(state) = slot
                {
                    found = Some(std::mem::take(&mut state.items));
                    break;
                }
            }
            found.ok_or(GridBoxError::ComponentNotFound)?
        };
        for node in region.nodes.values_mut() {
            if let Some(slot) = node.component_mut(to_component_id)
                && let FactoryComponent::Cache(state) = slot
            {
                move_item_into_cache(&mut state.items, item_id, from_items);
                return Ok(());
            }
        }
        Err(GridBoxError::ComponentNotFound)
    }

    pub fn move_bag_to_cache(
        &mut self,
        region_name: &str,
        component_id: u32,
        bag_grid_index: i32,
    ) -> Result<(), GridBoxError> {
        let region = self
            .region_mut(region_name)
            .ok_or(GridBoxError::RegionNotFound)?;
        let bag_item = {
            let inv_node = region
                .node_mut(1)
                .ok_or(GridBoxError::InventoryNodeMissing)?;
            let slot = inv_node
                .component_mut(1)
                .ok_or(GridBoxError::InventoryComponentMissing)?;
            let FactoryComponent::Inventory(inv_state) = slot else {
                return Err(GridBoxError::InventoryComponentMissing);
            };
            inv_state.items.remove(&(bag_grid_index as u32))
        };
        let item = bag_item.ok_or(GridBoxError::SlotEmpty)?;
        for node in region.nodes.values_mut() {
            if let Some(slot) = node.component_mut(component_id)
                && let FactoryComponent::Cache(state) = slot
            {
                state.items.push(item);
                return Ok(());
            }
        }
        Err(GridBoxError::ComponentNotFound)
    }

    pub fn move_cache_to_bag(
        &mut self,
        region_name: &str,
        component_id: u32,
        item_id: &str,
    ) -> Result<(), GridBoxError> {
        let region = self
            .region_mut(region_name)
            .ok_or(GridBoxError::RegionNotFound)?;
        let mut moved_item = None;
        for node in region.nodes.values_mut() {
            if let Some(slot) = node.component_mut(component_id)
                && let FactoryComponent::Cache(state) = slot
            {
                if let Some(s) = state
                    .items
                    .iter_mut()
                    .find(|s| s.item_id == item_id && s.count > 0)
                {
                    s.count = s.count.saturating_sub(1);
                    moved_item = Some(ItemSlot {
                        item_id: item_id.to_string(),
                        count: 1,
                        inst_id: 0,
                    });
                }
                break;
            }
        }
        let item = moved_item.ok_or(GridBoxError::ItemNotFound)?;
        let inv_node = region
            .node_mut(1)
            .ok_or(GridBoxError::InventoryNodeMissing)?;
        let slot = inv_node
            .component_mut(1)
            .ok_or(GridBoxError::InventoryComponentMissing)?;
        let FactoryComponent::Inventory(inv_state) = slot else {
            return Err(GridBoxError::InventoryComponentMissing);
        };
        inv_state.items.insert(0, item);
        Ok(())
    }

    pub fn move_depot_to_cache(
        &mut self,
        region_name: &str,
        component_id: u32,
        item_id: &str,
    ) -> Result<(), GridBoxError> {
        let region = self
            .region_mut(region_name)
            .ok_or(GridBoxError::RegionNotFound)?;
        let depot_item = {
            let hub_node = region.node_mut(2).ok_or(GridBoxError::HubNodeMissing)?;
            let slot = hub_node
                .component_mut(8)
                .ok_or(GridBoxError::InventoryComponentMissing)?;
            let FactoryComponent::Inventory(inv_state) = slot else {
                return Err(GridBoxError::InventoryComponentMissing);
            };
            let mut found = None;
            for (&inst_id, s) in &mut inv_state.items {
                if s.item_id == item_id && s.count > 0 {
                    s.count -= 1;
                    found = Some(ItemSlot {
                        item_id: item_id.to_string(),
                        count: 1,
                        inst_id,
                    });
                    if s.count == 0 {
                        inv_state.items.remove(&inst_id);
                    }
                    break;
                }
            }
            found
        };
        let item = depot_item.ok_or(GridBoxError::ItemNotFound)?;
        for node in region.nodes.values_mut() {
            if let Some(slot) = node.component_mut(component_id)
                && let FactoryComponent::Cache(state) = slot
            {
                state.items.push(item);
                return Ok(());
            }
        }
        Err(GridBoxError::ComponentNotFound)
    }

    pub fn move_cache_to_depot(
        &mut self,
        region_name: &str,
        component_id: u32,
        item_id: &str,
    ) -> Result<(), GridBoxError> {
        let region = self
            .region_mut(region_name)
            .ok_or(GridBoxError::RegionNotFound)?;
        let mut moved_item = None;
        for node in region.nodes.values_mut() {
            if let Some(slot) = node.component_mut(component_id)
                && let FactoryComponent::Cache(state) = slot
            {
                if let Some(s) = state
                    .items
                    .iter_mut()
                    .find(|s| s.item_id == item_id && s.count > 0)
                {
                    s.count = s.count.saturating_sub(1);
                    moved_item = Some(ItemSlot {
                        item_id: item_id.to_string(),
                        count: 1,
                        inst_id: 0,
                    });
                }
                break;
            }
        }
        let item = moved_item.ok_or(GridBoxError::ItemNotFound)?;
        let hub_node = region.node_mut(2).ok_or(GridBoxError::HubNodeMissing)?;
        let slot = hub_node
            .component_mut(8)
            .ok_or(GridBoxError::InventoryComponentMissing)?;
        let FactoryComponent::Inventory(inv_state) = slot else {
            return Err(GridBoxError::InventoryComponentMissing);
        };
        let mut stacked = false;
        for existing in inv_state.items.values_mut() {
            if existing.item_id == item.item_id && existing.inst_id == item.inst_id {
                existing.count += item.count;
                stacked = true;
                break;
            }
        }
        if !stacked {
            inv_state.items.insert(item.inst_id, item);
        }
        Ok(())
    }

    pub fn move_conveyor_to_bag(
        &mut self,
        region_name: &str,
        component_id: u32,
        index: i32,
        all: bool,
    ) -> Result<(), GridBoxError> {
        let region = self
            .region_mut(region_name)
            .ok_or(GridBoxError::RegionNotFound)?;
        let mut moved_items = vec![];
        for node in region.nodes.values_mut() {
            if let Some(slot) = node.component_mut(component_id)
                && let FactoryComponent::BoxConveyor(state) = slot
            {
                if all {
                    moved_items = std::mem::take(&mut state.items);
                } else if index >= 0 && (index as usize) < state.items.len() {
                    moved_items.push(state.items.remove(index as usize));
                }
                break;
            }
        }
        if moved_items.is_empty() {
            return Err(GridBoxError::SlotEmpty);
        }
        let inv_node = region
            .node_mut(1)
            .ok_or(GridBoxError::InventoryNodeMissing)?;
        let slot = inv_node
            .component_mut(1)
            .ok_or(GridBoxError::InventoryComponentMissing)?;
        let FactoryComponent::Inventory(inv_state) = slot else {
            return Err(GridBoxError::InventoryComponentMissing);
        };
        for item in moved_items {
            let item_id = &item.item.item_id;
            let inst_id = item.item.inst_id;
            // Stack with existing same-item slot (matching item_id + inst_id).
            let mut stacked = false;
            for slot in inv_state.items.values_mut() {
                if slot.item_id == *item_id && slot.inst_id == inst_id {
                    slot.count += item.item.count;
                    stacked = true;
                    break;
                }
            }
            if !stacked {
                inv_state.items.insert(inst_id, item.item);
            }
        }
        Ok(())
    }

    pub fn build_hs_fb_list(&self, region_name: &str, node_ids: &[u32]) -> Vec<HsFbEntry> {
        let Some(region) = self.region(region_name) else {
            return vec![];
        };
        let mut entries = vec![];
        for &node_id in node_ids {
            let Some(node) = region.node(node_id) else {
                continue;
            };
            for (component_id, comp) in &node.components {
                let Some(payload) = build_hs_fb_payload(comp) else {
                    continue;
                };
                entries.push(HsFbEntry {
                    component_id: *component_id,
                    payload,
                });
            }
        }
        entries
    }

    pub fn hs_inout(&mut self, region_name: &str, in_out: bool) {
        let Some(region) = self.region_mut(region_name) else {
            return;
        };
        let new_dir = if in_out { 0 } else { 2 };
        for node in region.nodes.values_mut() {
            for (_, comp) in &mut node.components {
                if let FactoryComponent::BoxConveyor(state) = comp {
                    state.direction = new_dir;
                }
            }
        }
    }
}

fn set_collector_target(state: &mut CollectorState, item_id: &str) {
    let new_item = ItemSlot {
        item_id: item_id.to_string(),
        count: 0,
        inst_id: 0,
    };
    if state.items_round.is_empty() {
        state.items_round.push(new_item);
    } else {
        state.items_round[0] = new_item;
    }
    state.start_tick = None;
    state.current_progress = 0;
}

fn build_hs_fb_payload(comp: &FactoryComponent) -> Option<HsFbPayload> {
    match comp {
        FactoryComponent::Cache(state) => Some(HsFbPayload::Cache {
            items: state.items.iter().map(|s| s.inst_id).collect(),
        }),
        FactoryComponent::Producer(state) => {
            let progress = state.start_tick.map_or(state.current_progress, |start| {
                state
                    .current_progress
                    .saturating_add(elapsed_since(start) * 100)
            });
            Some(HsFbPayload::Producer {
                progress_incr_per_ms: 100,
                formula_id: state.formula_id.clone(),
                current_progress: progress as i64,
            })
        }
        FactoryComponent::Collector(state) => {
            let progress = state.start_tick.map_or(state.current_progress, |start| {
                state
                    .current_progress
                    .saturating_add(elapsed_since(start) * 250)
            });
            Some(HsFbPayload::Collector {
                progress_incr_per_ms: 250,
                current_progress: progress as i64,
            })
        }
        FactoryComponent::BurnPower(state) => Some(HsFbPayload::BurnPower {
            progress_decr_per_ms: 125,
            current_least_progress: state.fuel_remaining,
        }),
        FactoryComponent::CacheTransport(state) => Some(HsFbPayload::CacheTransport {
            // Rate = 1000ms / total_progress (ticks per second).
            progress_incr_per_ms: if state.total_progress > 0 {
                1000 / state.total_progress
            } else {
                0
            },
            current_progress: state.current_progress,
        }),
        FactoryComponent::GridBox(state) => Some(HsFbPayload::GridBox {
            items: state.items.iter().map(|s| s.inst_id).collect(),
        }),
        FactoryComponent::BoxRouterM1 => {
            // BoxRouterM1 holds a single item internally; we don't track
            // it on the state yet (it's a unit variant). Return empty
            // until the state is extended.
            Some(HsFbPayload::BoxRouterM1 { items: vec![] })
        }
        FactoryComponent::BoxBridge => {
            // Same as BoxRouterM1 -- unit variant, no held-item state.
            Some(HsFbPayload::BoxBridge { items: vec![] })
        }
        FactoryComponent::HealTower(state) => Some(HsFbPayload::HealTower {
            // Heal rate = 1 point per 1000ms (1/sec). The actual rate
            // comes from FacSkillConst which we don't have on the state.
            progress_incr_per_ms: 1,
            current_progress: state.current_progress,
            current_point: state.points as i32,
        }),
        _ => None,
    }
}

fn find_gridbox(region: &mut FactoryRegion, component_id: u32) -> Option<&mut GridBoxState> {
    for node in region.nodes.values_mut() {
        if let Some(slot) = node.component_mut(component_id) {
            if let FactoryComponent::GridBox(state) = slot {
                return Some(state);
            }
            return None;
        }
    }
    None
}

fn move_item_into_cache(dest: &mut Vec<ItemSlot>, item_id: &str, source: Vec<ItemSlot>) {
    // Move ALL matching items from source into dest, stacking by item_id.
    for src_slot in &source {
        if src_slot.item_id != item_id {
            // Keep non-matching items in dest (they were taken from source).
            continue;
        }
        if let Some(dest_slot) = dest.iter_mut().find(|s| s.item_id == item_id) {
            dest_slot.count += src_slot.count;
        } else {
            dest.push(src_slot.clone());
        }
    }
}

fn rect_mesh_points(r: GridRange) -> Vec<GridPos> {
    vec![
        GridPos { x: r.x, y: r.y },
        GridPos {
            x: r.x + r.w as i32,
            y: r.y,
        },
        GridPos {
            x: r.x + r.w as i32,
            y: r.y + r.h as i32,
        },
        GridPos {
            x: r.x,
            y: r.y + r.h as i32,
        },
    ]
}

fn direction_from_i32(d: i32) -> FCDirection {
    match d {
        0 => FCDirection::Up,
        1 => FCDirection::Right,
        2 => FCDirection::Down,
        3 => FCDirection::Left,
        _ => FCDirection::Up,
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
