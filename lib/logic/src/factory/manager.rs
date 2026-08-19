//! # High-level factory state, one per player. Holds all regions as well
//! as the machine-specific tables (manufacture/processing/recycling/etc.),
//! which are indexed by region name, not embedded within the regions.
//! This corresponds to how the proto groups them.
//!
//! # Status of Clause 1
//!
//! - In [`FactoryManager::new` / `Default`,] we set `current_region = "test01"`,
//!   which is the only wire name that the alpha client accepts (§1.3).
//! - In [`FactoryManager::derive_region_id`], the cross-check against `regionData`
//!   is hooked to `FTableAssets::get_level_region`.
//! - In [`FactoryManager::bootstrap_region`], we create the inventory node (id=1)
//!   and hub node (id=2) with the 1+7 components layout, which the live server
//!   pushes in `push_factory`. Position/rotation of the nodes are baked into the
//!   scene geometry and therefore remain `None` in our proto builder in
//!   `handlers/factory.rs`, until Clause 3 replaces the hard-coded values with
//!   `region.to_proto()`.
//! - The way to retrieve/create a region is through
//!   [`FactoryManager::get_or_bootstrap`].
//!
//!
//! # TODO (forward-looking)
//!
//! - **Clause 1.6** `Player::factory: FactoryManager` connection is set up,
//!   but needs to be persisted yet. The database schema is specified in
//!   §7 (`0003_factory.sql`). `db/src/subsystems/factory.rs`
//!   implementation is Clause 4 and beyond work.
//! - **Clause 1.5** `push_factory` is still building the proto manually.
//!   Once Clause 3 `to_proto()` is implemented, use
//!   `manager.get_or_bootstrap(...).to_proto()` for `push_factory` body.
//! - **Clause 3.4** replace the current hub component layout in
//!   `bootstrap_region` with `component_factory::create_components_from_template("sp_hub_1")`.

use std::collections::HashMap;

use config::factory_table::FTableAssets;

use super::{
    BusLoaderState, CharacterWorkState, FactoryComponent, FactoryNode, FactoryRegion, GridPos,
    GridRange, InteractiveObject, InventoryState, ManualWorkQueue, ManufactureMachine, Mesh,
    NodeTransform, PowerBlackboard, PowerPoleState, PowerSaveState, ProcessorMachine,
    RecyclerMachine, SelectorState, SoilMachine, StablePowerState, SttState, TradeMachine,
    WorkshopMachine,
};
use crate::enums::{FCComponentPos, FCDirection, FCMeshType, FCNodeType};

#[derive(Debug, Clone, Default)]
pub struct FactoryManager {
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
        let mut manager = Self {
            current_region: "test01".to_string(),
            ..Default::default()
        };
        manager.bootstrap_quickbars();
        manager
    }

    /// Pre-seed both quickbar pages (`FCQuickBarType` Inner=0, Outer=1)
    /// with 32 empty slots. The client's `ModifyFactoryQuickbar` handler
    /// looks up each incoming quickbar in its local `quickBars` dict via
    /// `TryGetValue(type)` and silently *skips* any type that is missing,
    /// so a page must already be present in the initial
    /// `SC_FACTORY_SYNC_CONTEXT` for set/move responses to be applied.
    fn bootstrap_quickbars(&mut self) {
        self.quickbars = (0..2)
            .map(|quickbar_type| super::QuickbarState {
                quickbar_type,
                items: vec![String::new(); super::QUICKBAR_SIZE],
            })
            .collect();
    }

    /// Borrow the current region (the one the client thinks is active).
    pub fn current(&self) -> Option<&FactoryRegion> {
        self.regions.get(&self.current_region)
    }

    /// Mutably borrow the current region.
    pub fn current_mut(&mut self) -> Option<&mut FactoryRegion> {
        self.regions.get_mut(&self.current_region)
    }

    /// Borrow a region by wire name.
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

        if let Some(region) = factory_table.get_region(&region_id)
            && region.level_id != scene_name
        {
            tracing::warn!(
                scene_name,
                region_id,
                expected_scene = %region.level_id,
                "factory regionData/levelRegionData disagree on scene mapping"
            );
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

    /// Create an empty region with the inventory node (id=1) and hub node
    /// (id=2) already placed. This is the model-level equivalent of the
    /// proto created by `push_factory` that Clause 3 will eventually
    /// replace using `region.to_proto()` on the region created here.
    ///
    /// # Components layout
    ///
    /// Hub node contains 7 components in wire format:
    ///
    /// | id | position          | component    |
    /// |----|-------------------|--------------|
    /// | 2  | `Hub`             | `Hub`        |
    /// | 3  | `BusLoader`       | `BusLoader`  |
    /// | 4  | `PowerPole`       | `PowerPole`  |
    /// | 5  | `PowerSave`       | `PowerSave`  |
    /// | 6  | `StablePower`     | `StablePower`|
    /// | 7  | `Selector`        | `Selector`   |
    /// | 8  | `Inventory`       | `Inventory`  |
    ///
    /// The inventory node contains 1 component (the `Inventory`,
    /// component_id=1).
    ///
    /// Uses component_factory to build from template config.
    /// `component_factory::create_components_from_template("sp_hub_1")`
    /// to bootstrap this building in the same manner as others.
    // TODO(Clause 1.8): according to AC "The Hub node has 8 components",
    // verify if there is any "Transform" component (component_id=1) in live
    // server besides component positions, or if AC only counts
    // component_pos entries. Current implementation matches the
    // `push_factory` proto (7 components).
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
                // Off-grid: no position/mesh, no world transform.
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

        let hub_node = FactoryNode {
            node_id: 2,
            node_type: FCNodeType::Hub,
            template_id: hub_template_id.to_string(),
            transform: NodeTransform {
                position: Some(hub_position),
                direction: FCDirection::Up,
                mesh: Some(hub_mesh),
                scene_name: scene_name.clone(),
                // World position/rotation are baked scene
                // geometry with no table source; `push_factory` hardcodes
                // them for `sp_hub_1` on `map01_lv001`. Carry those over
                // here once nodes flow through `region.to_proto()`.
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
