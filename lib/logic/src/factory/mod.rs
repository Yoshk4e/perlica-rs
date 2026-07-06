//! core factory types -- region/node model, components, machines
//!
//! rough shape: FactoryManager (per player) -> N FactoryRegions -> nodes/connections
//! specialized machine stuff (manufacture, processor, etc) also lives on the manager
//!
//! ticks are just seconds. progress = current_tick - start_tick, no global loop.

use std::collections::{HashMap, HashSet};

use crate::enums::{
    FCComponentPos, FCConnectionPortType, FCConnectionType, FCDirection, FCMeshType, FCNodeType,
    FCPropertyKey,
};

pub mod blackboard;
pub mod manager;

#[cfg(test)]
mod tests;

pub use blackboard::PowerBlackboard;
pub use manager::FactoryManager;

// just unix seconds. keeps persistence simple
pub type Tick = u64;

/// wall clock as tick -- means offline time counts toward progress, which is
/// probably fine for now? revisit if we need offline pause
pub fn current_tick() -> Tick {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridRange {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

// used everywhere, keep it cheap to clone
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSlot {
    pub item_id: String,
    pub count: u32,
    // 0 for stackables, not totally sure this holds everywhere
    pub inst_id: u32,
}

#[derive(Debug, Clone)]
pub struct CachePort {
    pub position: GridPos,
    pub direction: i32,
}

#[derive(Debug, Clone)]
pub struct SubPort {
    pub position: GridPos,
    pub direction: i32,
}

#[derive(Debug, Clone)]
pub struct Mesh {
    pub mesh_type: FCMeshType,
    pub points: Vec<GridPos>,
}

#[derive(Debug, Clone, Copy)]
pub struct InteractiveObject {
    pub object_id: u32,
}

// miner + crafter share this, which is a little awkward but w/e
#[derive(Debug, Clone)]
pub struct ProducerState {
    pub formula_id: String,
    // None = idle/paused/no power
    pub start_tick: Option<Tick>,
    // snapshotted on pause/save. real value is current_progress + elapsed*speed
    pub current_progress: u64,
    pub in_power: bool,
    pub in_block: bool,
    pub power_cost: i64,
    pub last_formula_id: String,
}

// buffer slots attached to crafting buildings
#[derive(Debug, Clone)]
pub struct CacheState {
    pub items: Vec<ItemSlot>,
    pub ports: Vec<CachePort>,
}

// output side of a miner
#[derive(Debug, Clone)]
pub struct CollectorState {
    pub items_round: Vec<ItemSlot>,
    pub current_progress: u64,
    pub start_tick: Option<Tick>,
    pub in_power: bool,
    pub in_block: bool,
    pub power_cost: i64,
}

// hub storage, also used by depots
#[derive(Debug, Clone, Default)]
pub struct InventoryState {
    // inst_id==0 for stackables
    pub items: HashMap<u32, ItemSlot>,
}

#[derive(Debug, Clone, Default)]
pub struct SelectorState {
    pub selected_item_id: String,
    pub ports: Vec<CachePort>,
}

#[derive(Debug, Clone, Default)]
pub struct BusLoaderState {
    pub last_putin_item_id: String,
    pub ports: Vec<CachePort>,
}

#[derive(Debug, Clone, Copy)]
pub struct PowerPoleState {
    pub in_power: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct PowerSaveState {
    pub power_save: i64,
    pub in_power: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct StablePowerState {
    pub in_power: bool,
    pub power_gen_per_sec: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct BurnPowerState {
    pub fuel_remaining: i64,
    pub fuel_start_tick: Option<Tick>,
    pub in_power: bool,
}

#[derive(Debug, Clone, Default)]
pub struct GridBoxState {
    pub items: Vec<ItemSlot>,
}

#[derive(Debug, Clone, Copy)]
pub struct HealTowerState {
    pub points: i64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TravelPoleState {
    pub default_next: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ConveyorItem {
    pub item: ItemSlot,
    // 0..1 position along belt
    pub progress: f32,
}

#[derive(Debug, Clone)]
pub struct BoxConveyorState {
    pub items: Vec<ConveyorItem>,
    pub direction: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct CacheTransportState {
    pub enabled: bool,
    pub source_node_id: u32,
    pub target_node_id: u32,
}

#[derive(Debug, Clone)]
pub enum FactoryComponent {
    Transform,
    Bus,
    Inventory(InventoryState),
    Cache(CacheState),
    Selector(SelectorState),
    Collector(CollectorState),
    Producer(ProducerState),
    FormulaMan,
    BoxConveyor(BoxConveyorState),
    BoxRouterM1,
    BusUnloader,
    BusLoader(BusLoaderState),
    Hub,
    BurnPower(BurnPowerState),
    PowerPole(PowerPoleState),
    PowerSave(PowerSaveState),
    GridBox(GridBoxState),
    HealTower(HealTowerState),
    CacheTransport(CacheTransportState),
    StablePower(StablePowerState),
    TravelPole(TravelPoleState),
    BoxBridge,
    SpecialDesc,
}

#[derive(Debug, Clone)]
pub struct NodeTransform {
    pub position: Option<GridPos>,
    pub direction: FCDirection,
    pub mesh: Option<Mesh>,
    pub scene_name: String,
    pub world_position: Option<Vector3>,
    pub world_rotation: Option<Vector3>,
    pub bc_port_in: Option<SubPort>,
    pub bc_port_out: Option<SubPort>,
}

#[derive(Debug, Clone)]
pub struct FactoryNode {
    pub node_id: u32,
    pub node_type: FCNodeType,
    pub template_id: String,
    pub transform: NodeTransform,
    pub is_deactive: bool,
    pub interactive_object: Option<InteractiveObject>,
    pub dynamic_property: HashMap<FCPropertyKey, String>,
    pub component_pos: HashMap<FCComponentPos, u32>,
    /// order matters for wire format
    pub components: Vec<(u32, FactoryComponent)>,
}

#[derive(Debug, Clone)]
pub struct FactoryConnection {
    pub connection_type: FCConnectionType,
    pub port_type: FCConnectionPortType,
    pub node_id_a: u32,
    pub node_id_b: u32,
    // used when deleting connections, node_id pair isn't stable enough
    pub index: u64,
}

// one per map level. `name` is what goes on the wire ("test01" right now),
// `region_id` is our internal one ("region_102"). a bit annoying
#[derive(Debug, Clone)]
pub struct FactoryRegion {
    pub name: String,
    pub region_id: String,
    pub scene_name: String,
    pub level: u32,
    pub next_node_id: u32,
    pub blackboard: PowerBlackboard,
    pub nodes: HashMap<u32, FactoryNode>,
    pub connections: Vec<FactoryConnection>,
    // lifetime production totals, STT 504 reads this
    pub production_totals: HashMap<String, u64>,
}

impl FactoryRegion {
    pub fn new(
        name: impl Into<String>,
        region_id: impl Into<String>,
        scene_name: impl Into<String>,
        level: u32,
    ) -> Self {
        Self {
            name: name.into(),
            region_id: region_id.into(),
            scene_name: scene_name.into(),
            level,
            // 1+2 taken by inventory/hub from push_factory, start at 3
            next_node_id: 3,
            blackboard: PowerBlackboard::default(),
            nodes: HashMap::new(),
            connections: Vec::new(),
            production_totals: HashMap::new(),
        }
    }

    pub fn allocate_node_id(&mut self) -> u32 {
        let id = self.next_node_id;
        self.next_node_id += 1;
        id
    }

    // for STT 504
    pub fn count_buildings_by_template(&self, template_id: &str) -> usize {
        self.nodes
            .values()
            .filter(|n| n.template_id == template_id)
            .count()
    }

    // speed = progress per tick. if start_tick is None we're paused/unpowered
    // so just return the snapshot
    pub fn compute_producer_progress(&self, state: &ProducerState, speed: u64) -> u64 {
        match state.start_tick {
            Some(start) => {
                let elapsed = current_tick().saturating_sub(start);
                state
                    .current_progress
                    .saturating_add(elapsed.saturating_mul(speed))
            }
            None => state.current_progress,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ActiveRecipe {
    pub recipe_id: String,
    pub start_tick: Tick,
}

#[derive(Debug, Clone)]
pub struct ManufactureMachine {
    pub region_name: String,
    pub building_level: u32,
    pub active_recipe: Option<ActiveRecipe>,
    pub outcome_buffer: Vec<ItemSlot>,
    pub set_count: u32,
}

#[derive(Debug, Clone)]
pub struct ProcessorMachine {
    pub region_name: String,
    pub building_level: u32,
    // 0-6, regenerate over time
    pub refine_points: u32,
    // need this to know how many points came back since we last looked
    pub last_recovery_tick: Tick,
    pub unlocked_formulas: Vec<String>,
    pub unread_formulas: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RecyclerMachine {
    pub region_name: String,
    pub accumulated_value: u64,
    // max 10 (recTempStorageLength)
    pub temp_storage: Vec<ItemSlot>,
    pub product_timer_start: Option<Tick>,
}

#[derive(Debug, Clone)]
pub struct TradeOrder {
    pub order_id: String,
    pub accumulated_value: u64,
    pub items_committed: Vec<ItemSlot>,
}

#[derive(Debug, Clone)]
pub struct TradeMachine {
    pub region_name: String,
    pub building_level: u32,
    pub active_contract: Option<String>,
    pub orders: Vec<TradeOrder>,
    pub last_gen_tick: Tick,
}

#[derive(Debug, Clone)]
pub struct WorkshopMachine {
    pub region_name: String,
    pub building_level: u32,
}

#[derive(Debug, Clone)]
pub enum SoilDoodadState {
    Empty,
    Growing,
    Mature,
}

#[derive(Debug, Clone)]
pub struct SoilMachine {
    pub region_name: String,
    pub planted_seed: Option<ActiveRecipe>,
    pub doodad_state: SoilDoodadState,
}

#[derive(Debug, Clone)]
pub struct ManualWorkUnit {
    pub recipe_id: String,
    pub start_tick: Tick,
    pub progress: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ManualWorkQueue {
    pub queue: Vec<ManualWorkUnit>,
    pub is_paused: bool,
}

#[derive(Debug, Clone)]
pub struct CharacterWorker {
    pub region_name: String,
    pub char_inst_id: u32,
    pub skill_ids: Vec<String>,
    pub work_slot: u32,
}

#[derive(Debug, Clone, Default)]
pub struct CharacterWorkState {
    pub workers: Vec<CharacterWorker>,
}

#[derive(Debug, Clone)]
pub struct QuickbarState {
    pub quickbar_type: String,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SttState {
    pub unlocked_nodes: Vec<String>,
    pub visible_formulas: Vec<String>,
}

// not saved, rebuilt from region snapshot when needed. compute() is a stub until clause 4 in the implementaion docs is done.
#[derive(Debug, Clone, Default)]
pub struct PowerGraph {
    pub powered_nodes: HashSet<u32>,
    pub total_generation: i64,
    pub total_consumption: i64,
    pub total_storage: i64,
    pub total_stored: i64,
}

impl PowerGraph {
    // TODO clause 4: BFS from StablePower/BurnPower through pole connections,
    // mark reachable nodes powered, sum gen/consumption/storage
    pub fn compute(_region: &FactoryRegion) -> Self {
        Self::default()
    }
}
