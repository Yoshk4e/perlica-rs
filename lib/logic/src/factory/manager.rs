//! Highest level factory state, one for each player. Owns all regions, along
//! with the machine tables for manufacture/processing/recycling/etc which are
//! keyed by region names, rather than nodes being nested within those regions.
//!
//! TODO: This is not wired up yet. The `Player` struct currently only
//! contains this in memory, see the TODO on the `Player::factory` field in game-server.
//! The database table for this resides in §7 of the implementation plan
//! (`0003_factory.sql`) but has not yet been implemented.

use std::collections::HashMap;

use config::factory_table::FTableAssets;

use super::{
    CharacterWorkState, FCComponentPos, FCDirection, FCMeshType, FCNodeType, FactoryComponent,
    FactoryNode, FactoryRegion, GridPos, GridRange, InteractiveObject, InventoryState,
    ManualWorkQueue, ManufactureMachine, Mesh, NodeTransform, PowerBlackboard, PowerPoleState,
    PowerSaveState, ProcessorMachine, RecyclerMachine, SelectorState, SoilMachine, SttState,
    TradeMachine, WorkshopMachine,
};
use crate::factory::{BusLoaderState, StablePowerState};

#[derive(Debug, Clone, Default)]
pub struct FactoryManager {
    /// hardwired value, always “test01” for now (see §1.3 of the impl plan --
    /// the client will reject anything else). The `regions` map is still
    /// indexed by this same key, as that’s the only key we get from the
    /// client.
    pub current_region: String,
    pub regions: HashMap<String, FactoryRegion>,
    pub quickbars: Vec<super::QuickbarState>,
    pub stt_state: SttState,
    pub manufacture_state: HashMap<String, ManufactureMachine>,
    pub processor_state: HashMap<String, ProcessorMachine>,
    pub recycler_state: HashMap<String, RecyclerMachine>,
    pub trade_state: HashMap<String, TradeMachine>,
    pub workshop_state: HashMap<String, WorkshopMachine>,
    pub soil_state: HashMap<String, SoilMachine>,
    pub manual_work_state: ManualWorkQueue,
    pub character_work_state: CharacterWorkState,
}

impl FactoryManager {
    pub fn new() -> Self {
        Self {
            current_region: "test01".to_string(),
            ..Default::default()
        }
    }

    pub fn region(&self, wire_name: &str) -> Option<&FactoryRegion> {
        self.regions.get(wire_name)
    }

    pub fn region_mut(&mut self, wire_name: &str) -> Option<&mut FactoryRegion> {
        self.regions.get_mut(wire_name)
    }

    pub fn derive_region_id(factory_table: &FTableAssets, scene_name: &str) -> Option<String> {
        let region_id = factory_table
            .get_level_region(scene_name)?
            .list
            .first()?
            .clone();

        // tables should agree with each other; if they don't, something's off
        // in the JSON, but it's not worth hard-failing the caller over.
        if let Some(region) = factory_table.get_region(&region_id) {
            if region.level_id != scene_name {
                tracing::warn!(
                    scene_name,
                    region_id,
                    expected_scene = %region.level_id,
                    "factory regionData/levelRegionData disagree on scene mapping"
                );
            }
        }

        Some(region_id)
    }

    /// Returns the region for `wire_name`, creating + bootstrapping it via
    /// `make` on first access. `make` is only called on a cache miss.
    pub fn get_or_bootstrap(
        &mut self,
        wire_name: &str,
        make: impl FnOnce() -> FactoryRegion,
    ) -> &mut FactoryRegion {
        self.regions
            .entry(wire_name.to_string())
            .or_insert_with(make)
    }

    /// Constructs the two nodes with which each newly-created region is seeded,
    /// namely the player's inventory (node 1) and the hub building (node 2).
    /// This is the internal model equivalent of the proto that `push_factory`
    /// creates in `handlers/factory.rs` right now, but they haven't been
    /// connected yet because the `to_proto()` translation is clause 3. When
    /// the clause 3 lands, `push_factory` should create this instead.
    ///
    /// TODO(clause 3): replaces the hard-coded 8-component hub building
    /// structure with the generic `component_factory` (building template ->
    /// components), so that other buildings won't require their own bootstrap
    /// function.
    #[allow(clippy::too_many_arguments)]
    pub fn bootstrap_region(
        wire_name: impl Into<String>,
        region_id: impl Into<String>,
        scene_name: impl Into<String>,
        hub_template_id: &str,
        hub_position: GridPos,
        hub_range: GridRange,
        power_gen: i64,
        power_save_max: i64,
    ) -> FactoryRegion {
        let scene_name = scene_name.into();
        let mut region = FactoryRegion::new(wire_name, region_id, scene_name.clone(), 1);

        let inventory_node = FactoryNode {
            node_id: 1,
            node_type: FCNodeType::Inventory,
            template_id: "__inventory__".to_string(),
            transform: NodeTransform {
                // not placed on the grid, so no position/mesh
                position: None,
                direction: FCDirection::Up,
                mesh: None,
                scene_name: scene_name.clone(),
                world_position: None,
                world_rotation: None,
                bc_port_in: None,
                bc_port_out: None,
            },
            is_deactive: true,
            interactive_object: None,
            dynamic_property: HashMap::new(),
            component_pos: HashMap::from([(FCComponentPos::Inventory, 1u32)]),
            components: vec![(1, FactoryComponent::Inventory(InventoryState::default()))],
        };

        let hub_mesh = Mesh {
            mesh_type: FCMeshType::Rect,
            points: vec![
                GridPos {
                    x: hub_range.x,
                    y: hub_range.y,
                },
                GridPos {
                    x: hub_range.x + hub_range.w as i32,
                    y: hub_range.y,
                },
                GridPos {
                    x: hub_range.x + hub_range.w as i32,
                    y: hub_range.y + hub_range.h as i32,
                },
                GridPos {
                    x: hub_range.x,
                    y: hub_range.y + hub_range.h as i32,
                },
            ],
        };

        // 8 components -- Hub, BusLoader, PowerPole, PowerSave, StablePower,
        // Selector, Inventory. matches the sp_hub_1 layout push_factory sends.
        let hub_node = FactoryNode {
            node_id: 2,
            node_type: FCNodeType::Hub,
            template_id: hub_template_id.to_string(),
            transform: NodeTransform {
                position: Some(hub_position),
                direction: FCDirection::Up,
                mesh: Some(hub_mesh),
                scene_name: scene_name.clone(),
                // TODO: world position/rotation are baked scene
                // geometry with no table source; push_factory hardcodes them
                // for now. carry that over here once nodes flow through here.
                world_position: None,
                world_rotation: None,
                bc_port_in: None,
                bc_port_out: None,
            },
            is_deactive: false,
            interactive_object: Some(InteractiveObject { object_id: 1 }),
            dynamic_property: HashMap::new(),
            component_pos: HashMap::from([
                (FCComponentPos::Hub, 2u32),
                (FCComponentPos::BusLoader, 3u32),
                (FCComponentPos::PowerPole, 4u32),
                (FCComponentPos::PowerSave, 5u32),
                (FCComponentPos::StablePower, 6u32),
                (FCComponentPos::Selector, 7u32),
                (FCComponentPos::Inventory, 8u32),
            ]),
            components: vec![
                (2, FactoryComponent::Hub),
                (3, FactoryComponent::BusLoader(BusLoaderState::default())),
                (
                    4,
                    FactoryComponent::PowerPole(PowerPoleState { in_power: true }),
                ),
                (
                    5,
                    FactoryComponent::PowerSave(PowerSaveState {
                        power_save: power_save_max,
                        in_power: true,
                    }),
                ),
                (
                    6,
                    FactoryComponent::StablePower(StablePowerState {
                        in_power: true,
                        power_gen_per_sec: power_gen,
                    }),
                ),
                (7, FactoryComponent::Selector(SelectorState::default())),
                (8, FactoryComponent::Inventory(InventoryState::default())),
            ],
        };

        region.blackboard = PowerBlackboard::with_hub_power(1, power_gen, power_save_max);
        region.nodes.insert(1, inventory_node);
        region.nodes.insert(2, hub_node);
        region.next_node_id = 3;
        region
    }
}
