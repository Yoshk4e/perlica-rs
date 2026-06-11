use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct LoaderEntry {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LocalizedText {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub id: String,
}

/// An item id + count pair (used in ingredients/outcomes/costs).
#[derive(Debug, Clone, Deserialize)]
pub struct ItemCount {
    pub id: String,
    pub count: u32,
}

/// `{ "group": [ItemCount, ...] }`, wrapper used by machine craft recipes
/// to allow multiple ingredient/outcome variants per slot.
#[derive(Debug, Clone, Deserialize)]
pub struct ItemCountGroup {
    #[serde(default)]
    pub group: Vec<ItemCount>,
}

/// One entry from `hubData` (e.g. `"sp_hub_1"`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HubEntry {
    pub building_id: String,
    /// Max stored energy (powerSave capacity).
    pub power_storage_capacity: i64,
    /// Passive power generation per second.
    pub power_generate: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildingPoint {
    pub x: i32,
    pub y: i32,
}

/// A port on a building: grid position (relative to the building's top-left)
/// and the facing side (FCDirection: Up=0, Right=1, Down=2, Left=3).
#[derive(Debug, Clone, Deserialize)]
pub struct BuildingPort {
    pub point: BuildingPoint,
    pub side: i32,
}

/// Grid-space bounding box for a building (x/y are always 0,0 for relative coords).
#[derive(Debug, Clone, Deserialize)]
pub struct BuildingRange {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// One entry from `buildingData`.
///
/// Includes the optional UI/metadata fields so the whole table round-trips,
/// but the hot path only really uses `range` + `input_ports` + `output_ports`
/// + `power_consume`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildingEntry {
    pub id: String,
    /// FCNodeType (see `perlica_logic::enums::FCNodeType`).
    #[serde(default, rename = "type")]
    pub building_type: i32,
    #[serde(default)]
    pub quick_bar_type: String,
    pub range: BuildingRange,
    #[serde(default)]
    pub limit_type: i32,
    #[serde(default)]
    pub road_attach_side: i32,
    #[serde(default)]
    pub power_consume: i64,
    #[serde(default)]
    pub name: Option<LocalizedText>,
    #[serde(default)]
    pub icon_on_panel: String,
    #[serde(default)]
    pub desc: Option<LocalizedText>,
    #[serde(default)]
    pub input_ports: Vec<BuildingPort>,
    #[serde(default)]
    pub output_ports: Vec<BuildingPort>,
    #[serde(default)]
    pub only_show_on_main: bool,
    #[serde(default)]
    pub mark_info_id: String,
    #[serde(default)]
    pub bandwidth: i64,
    #[serde(default = "default_true")]
    pub can_delete: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionEntry {
    pub region_id: String,
    pub level_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LevelRegionEntry {
    pub list: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineCrafterEntry {
    pub id: String,
    #[serde(default)]
    pub signal_count: u32,
    #[serde(default)]
    pub can_insert_mod: bool,
    #[serde(default)]
    pub ingredient_buffer_binding: Vec<BufferBinding>,
    #[serde(default)]
    pub outcome_buffer_binding: Vec<BufferBinding>,
    #[serde(default = "default_machine_speed")]
    pub speed: u64,
    #[serde(default)]
    pub craft_list: Vec<String>,
}

fn default_machine_speed() -> u64 {
    100
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferBinding {
    #[serde(default)]
    pub binding_port_indices: Vec<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineCraftEntry {
    pub id: String,
    pub machine_id: String,
    #[serde(default)]
    pub signal: i32,
    #[serde(default)]
    pub sort_id: u32,
    pub total_progress: u64,
    #[serde(default)]
    pub ingredients: Vec<ItemCountGroup>,
    #[serde(default)]
    pub outcomes: Vec<ItemCountGroup>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManufactCraftEntry {
    pub id: String,
    #[serde(default)]
    pub rarity: u32,
    #[serde(default)]
    pub sort_id: u32,
    #[serde(default)]
    pub ingredients: Vec<ItemCount>,
    pub outcome: ManufactOutcome,
    pub total_progress: u64,
    #[serde(default)]
    pub usable_level: u32,
    #[serde(default = "default_one_u32")]
    pub round_count: u32,
    #[serde(default)]
    pub showing_type: u32,
}

fn default_one_u32() -> u32 {
    1
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManufactOutcome {
    pub id: String,
    #[serde(default = "default_one_u32")]
    pub count: u32,
    #[serde(default = "default_one_u32")]
    pub cost: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessorCraftEntry {
    pub id: String,
    /// `FCCraftType` int.
    #[serde(default, rename = "type")]
    pub craft_type: i32,
    /// Processor sub-type (gem/weapon/equip/etc.).
    #[serde(default)]
    pub proc_type: i32,
    #[serde(default)]
    pub rarity: u32,
    #[serde(default)]
    pub sort_id: u32,
    #[serde(default)]
    pub ingredients: Vec<ItemCount>,
    /// Single outcome (unlike machine crafts which can group).
    pub outcome: ItemCount,
    #[serde(default)]
    pub usable_level: u32,
    #[serde(default)]
    pub can_refine: bool,
    #[serde(default)]
    pub visible_by_level: u32,
    #[serde(default)]
    pub group_id: String,
    #[serde(default)]
    pub group_name: Option<LocalizedText>,
    #[serde(default)]
    pub filter_type: i32,
    #[serde(default)]
    pub ui_slot: u32,
    #[serde(default)]
    pub visible_by_item_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkshopCraftEntry {
    pub id: String,
    #[serde(default)]
    pub showing_type: u32,
    #[serde(default)]
    pub rarity: u32,
    #[serde(default)]
    pub sort_id: u32,
    #[serde(default)]
    pub usable_level: u32,
    #[serde(default)]
    pub belonging_group_ids: Vec<String>,
    #[serde(default)]
    pub ingredients: Vec<ItemCount>,
    #[serde(default)]
    pub outcomes: Vec<ItemCount>,
    #[serde(default)]
    pub default_unlock: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualCraftEntry {
    pub id: String,
    #[serde(default)]
    pub name: Option<LocalizedText>,
    #[serde(default)]
    pub showing_type: u32,
    #[serde(default)]
    pub rarity: u32,
    #[serde(default)]
    pub sort_id: u32,
    pub total_progress: u64,
    #[serde(default)]
    pub belonging_group_ids: Vec<String>,
    #[serde(default)]
    pub ingredients: Vec<ItemCount>,
    #[serde(default)]
    pub outcomes: Vec<ItemCount>,
    #[serde(default)]
    pub default_unlock: bool,
    #[serde(default)]
    pub item_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinerEntry {
    pub id: String,
    #[serde(default)]
    pub has_drone_mode: bool,
    pub speed: u64,
    #[serde(default)]
    pub mineable: Vec<String>,
    #[serde(default = "default_zero_point")]
    pub mine_position: BuildingPoint,
    #[serde(default)]
    pub transfer_cd: u64,
}

fn default_zero_point() -> BuildingPoint {
    BuildingPoint { x: 0, y: 0 }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerStationEntry {
    pub id: String,
    pub power_provide: i64,
    pub burn_speed: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerPoleEntry {
    pub id: String,
    #[serde(default)]
    pub range_extend_w: u32,
    #[serde(default)]
    pub range_extend_h: u32,
    #[serde(default)]
    pub power_range_size: u32,
    #[serde(default)]
    pub can_use_in_fast_move: bool,
    #[serde(default)]
    pub upgrade_target_pole_id: String,
    #[serde(default)]
    pub upgrade_cost_items: Vec<ItemCount>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FuelItemEntry {
    pub id: String,
    pub fuel_energy: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecyclerMaterialEntry {
    pub id: String,
    pub need_value: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecyclerProductEntry {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderEntry {
    pub order_id: String,
    #[serde(default)]
    pub name: Option<LocalizedText>,
    #[serde(default)]
    pub desc: Option<LocalizedText>,
    #[serde(default)]
    pub color: String,
    pub need_value: u64,
    #[serde(default)]
    pub reward_id: String,
    #[serde(default)]
    pub bloc_exp: u32,
    pub contract_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraderEntry {
    pub building_id: String,
    pub max_order_count: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedItemEntry {
    pub id: String,
    pub grow_total_progress: u64,
    #[serde(default)]
    pub doodad_id: String,
    #[serde(default)]
    pub model_key: String,
    #[serde(default)]
    pub growing_model_key: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoilEntry {
    pub id: String,
    pub grow_speed: u64,
}

/// `skillData[<skillId>]`, factory worker skills.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillEntry {
    pub id: String,
    #[serde(default)]
    pub name: Option<LocalizedText>,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub sort_id: u32,
    #[serde(default)]
    pub building_type: i32,
    /// Parallel arrays: `type[i]` is the modifier kind, `paramList[i]` is its value.
    #[serde(default, rename = "type")]
    pub type_list: Vec<i32>,
    #[serde(default)]
    pub param_list: Vec<f64>,
    #[serde(default)]
    pub desc: Option<LocalizedText>,
    #[serde(default)]
    pub effect_building_id: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoragerEntry {
    pub id: String,
    pub capacity: u32,
    #[serde(default)]
    pub transfer_cd: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MedicEntry {
    pub id: String,
    #[serde(default)]
    pub max_energy: u32,
    #[serde(default)]
    pub energy_charge_ticks: u32,
    #[serde(default)]
    pub healing_ticks: u32,
    #[serde(default)]
    pub healing_radius: u32,
    #[serde(default)]
    pub healing_hp: u32,
    #[serde(default)]
    pub healing_cost: u32,
    #[serde(default)]
    pub healing_percent: u32,
    #[serde(default)]
    pub charge_num: u32,
    #[serde(default)]
    pub charge_ticks: u32,
}

/// One entry from `specialPowerPole`, pre-placed map-anchored power
/// gate/port/terminal nodes.  Only the bits used at runtime are typed.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecialPowerPoleEntry {
    pub id: String,
    #[serde(default)]
    pub building_name: Option<LocalizedText>,
    #[serde(default)]
    pub building_desc: Option<LocalizedText>,
    #[serde(default)]
    pub map_name: Option<LocalizedText>,
    #[serde(default)]
    pub position_desc: Option<LocalizedText>,
    #[serde(default)]
    pub map_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecialBuildingEntry {
    pub building_id: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub sort_id: u32,
    /// `levelData` is `{ "<some_int_as_string>": <int> }`, kept opaque.
    #[serde(default)]
    pub level_data: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildingItemEntry {
    pub item_id: String,
    pub building_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickBarTypeEntry {
    pub id: String,
    #[serde(default)]
    pub name: Option<LocalizedText>,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub priority: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildingPanelUnlockEntry {
    pub id: String,
    #[serde(default)]
    pub list: Vec<BuildingPanelUnlockItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildingPanelUnlockItem {
    #[serde(default)]
    pub quest_id: String,
    #[serde(default)]
    pub radio_id: String,
}

/// long-tail UI/lookup tables are kept as `serde_json::Value` so we don't
/// over-commit to a schema we don't yet consume.
///
/// All `#[serde(default)]` annotations are intentional: they let the
/// struct still load when an upstream patch drops a key, and the
/// integration tests (in `lib/config/tests/factory_configs.rs`) verify
/// that the live JSON populates the typed sub-tables.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FactoryTable {
    #[serde(default)]
    pub hub_data: HashMap<String, HubEntry>,
    #[serde(default)]
    pub building_data: HashMap<String, BuildingEntry>,
    #[serde(default)]
    pub region_data: HashMap<String, RegionEntry>,
    #[serde(default)]
    pub level_region_data: HashMap<String, LevelRegionEntry>,
    #[serde(default)]
    pub machine_crafter_data: HashMap<String, MachineCrafterEntry>,
    #[serde(default)]
    pub machine_craft_data: HashMap<String, MachineCraftEntry>,
    #[serde(default)]
    pub manufact_craft_data: HashMap<String, ManufactCraftEntry>,
    #[serde(default)]
    pub processor_craft_data: HashMap<String, ProcessorCraftEntry>,
    #[serde(default)]
    pub workshop_craft_data: HashMap<String, WorkshopCraftEntry>,
    #[serde(default)]
    pub manual_craft_data: HashMap<String, ManualCraftEntry>,
    #[serde(default)]
    pub miner_data: HashMap<String, MinerEntry>,
    #[serde(default)]
    pub power_station_data: HashMap<String, PowerStationEntry>,
    #[serde(default)]
    pub power_pole_data: HashMap<String, PowerPoleEntry>,
    #[serde(default)]
    pub fuel_item_data: HashMap<String, FuelItemEntry>,
    #[serde(default)]
    pub recycler_material_data: HashMap<String, RecyclerMaterialEntry>,
    #[serde(default)]
    pub recycler_product_data: HashMap<String, RecyclerProductEntry>,
    #[serde(default)]
    pub order_data: HashMap<String, OrderEntry>,
    #[serde(default)]
    pub seed_item_data: HashMap<String, SeedItemEntry>,
    #[serde(default)]
    pub soil_data: HashMap<String, SoilEntry>,
    #[serde(default)]
    pub skill_data: HashMap<String, SkillEntry>,
    #[serde(default)]
    pub trader_data: HashMap<String, TraderEntry>,
    #[serde(default)]
    pub storager_data: HashMap<String, StoragerEntry>,
    #[serde(default)]
    pub medic_data: HashMap<String, MedicEntry>,
    #[serde(default)]
    pub special_power_pole: HashMap<String, SpecialPowerPoleEntry>,
    #[serde(default)]
    pub special_building_data: HashMap<String, SpecialBuildingEntry>,
    #[serde(default)]
    pub building_item_data: HashMap<String, BuildingItemEntry>,
    #[serde(default)]
    pub building_item_reverse_data: HashMap<String, BuildingItemEntry>,
    #[serde(default)]
    pub quick_bar_type_data: HashMap<String, QuickBarTypeEntry>,
    #[serde(default)]
    pub building_panel_unlock_data: HashMap<String, BuildingPanelUnlockEntry>,
    #[serde(default)]
    pub factory_craft_showing_type_table: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub workshop_craft_type_list_data: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub proc_craft_type_list_data: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub proc_gem_group_data: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub proc_gem_recast_data: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub processor_type_data: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub processor_gem_filter_type_data: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub power_data_type_data: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub productivity_data_type_data: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub skill_type_data: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub special_craft_data: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub special_craft_group_data: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub grid_belt_data: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub grid_connecter_data: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub grid_router_data: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub ingredient_tag_data: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub item_data: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub item_2_logistic_id_data: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub item_as_machine_crafter_income_table: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub item_as_machine_crafter_outcome_table: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub item_as_manual_craft_outcome_table: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub item_as_manufact_income_table: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub item_as_manufact_outcome_table: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub item_as_workshop_income_table: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub item_as_workshop_outcome_table: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub machine_id2tag_ids: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub resource_item_id2_machine_id_table: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub resource_item_id2_tag_id_table: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub loader_data: Vec<LoaderEntry>,
    #[serde(default)]
    pub unloader_data: Vec<LoaderEntry>,
}
