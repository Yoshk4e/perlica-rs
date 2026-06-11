//! Runtime wrapper around `assets/tables/FactoryTable.json`.
//!
//! Exposes typed accessors over the sub-tables most handlers care
//! about.  The raw [`FactoryTable`] is still public via the `data`
//! field for code that needs to walk an opaque sub-table.

use crate::error::{ConfigError, Result};
use crate::tables::factory_table::{
    BuildingEntry, BuildingItemEntry, BuildingPanelUnlockEntry, FactoryTable, FuelItemEntry,
    HubEntry, LevelRegionEntry, MachineCraftEntry, MachineCrafterEntry, ManualCraftEntry,
    ManufactCraftEntry, MedicEntry, MinerEntry, OrderEntry, PowerPoleEntry, PowerStationEntry,
    ProcessorCraftEntry, QuickBarTypeEntry, RecyclerMaterialEntry, RecyclerProductEntry,
    RegionEntry, SeedItemEntry, SkillEntry, SoilEntry, SpecialBuildingEntry, SpecialPowerPoleEntry,
    StoragerEntry, TraderEntry, WorkshopCraftEntry,
};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct FTableAssets {
    pub data: FactoryTable,
}

impl FTableAssets {
    pub(super) fn load(tables_dir: &Path) -> Result<Self> {
        let path = tables_dir.join("FactoryTable.json");
        let contents = std::fs::read_to_string(&path).map_err(|e| ConfigError::ReadFile {
            path: path.clone(),
            source: e,
        })?;
        let data: FactoryTable =
            serde_json::from_str(&contents).map_err(|e| ConfigError::ParseJson {
                path: path.clone(),
                source: e,
            })?;
        Ok(Self { data })
    }

    /// Hub power capacity stats.
    pub fn get_hub(&self, building_id: &str) -> Option<&HubEntry> {
        self.data.hub_data.get(building_id)
    }

    /// Grid dimensions and port layout.
    pub fn get_building(&self, building_id: &str) -> Option<&BuildingEntry> {
        self.data.building_data.get(building_id)
    }

    pub fn region_for_level(&self, level_id: &str) -> Option<&str> {
        self.data
            .level_region_data
            .get(level_id)
            .and_then(|e| e.list.first())
            .map(String::as_str)
    }

    pub fn get_region(&self, region_id: &str) -> Option<&RegionEntry> {
        self.data.region_data.get(region_id)
    }

    pub fn get_level_region(&self, level_id: &str) -> Option<&LevelRegionEntry> {
        self.data.level_region_data.get(level_id)
    }

    pub fn get_machine_crafter(&self, machine_id: &str) -> Option<&MachineCrafterEntry> {
        self.data.machine_crafter_data.get(machine_id)
    }

    pub fn get_recipe(&self, recipe_id: &str) -> Option<&MachineCraftEntry> {
        self.get_machine_craft(recipe_id)
    }

    pub fn get_machine_craft(&self, recipe_id: &str) -> Option<&MachineCraftEntry> {
        self.data.machine_craft_data.get(recipe_id)
    }

    pub fn get_manufact_craft(&self, recipe_id: &str) -> Option<&ManufactCraftEntry> {
        self.data.manufact_craft_data.get(recipe_id)
    }

    pub fn get_processor_craft(&self, recipe_id: &str) -> Option<&ProcessorCraftEntry> {
        self.data.processor_craft_data.get(recipe_id)
    }

    pub fn get_workshop_craft(&self, recipe_id: &str) -> Option<&WorkshopCraftEntry> {
        self.data.workshop_craft_data.get(recipe_id)
    }

    pub fn get_manual_craft(&self, recipe_id: &str) -> Option<&ManualCraftEntry> {
        self.data.manual_craft_data.get(recipe_id)
    }

    pub fn get_miner(&self, miner_id: &str) -> Option<&MinerEntry> {
        self.data.miner_data.get(miner_id)
    }

    pub fn get_power_station(&self, id: &str) -> Option<&PowerStationEntry> {
        self.data.power_station_data.get(id)
    }

    pub fn get_power_pole(&self, id: &str) -> Option<&PowerPoleEntry> {
        self.data.power_pole_data.get(id)
    }

    pub fn get_fuel_item(&self, item_id: &str) -> Option<&FuelItemEntry> {
        self.data.fuel_item_data.get(item_id)
    }

    pub fn get_recycler_material(&self, item_id: &str) -> Option<&RecyclerMaterialEntry> {
        self.data.recycler_material_data.get(item_id)
    }

    pub fn get_recycler_product(&self, item_id: &str) -> Option<&RecyclerProductEntry> {
        self.data.recycler_product_data.get(item_id)
    }

    /// All available recycler products (used for random selection on
    /// completion).
    pub fn recycler_product_ids(&self) -> impl Iterator<Item = &str> {
        self.data.recycler_product_data.keys().map(String::as_str)
    }

    pub fn get_order(&self, order_id: &str) -> Option<&OrderEntry> {
        self.data.order_data.get(order_id)
    }

    pub fn get_trader(&self, building_id: &str) -> Option<&TraderEntry> {
        self.data.trader_data.get(building_id)
    }

    pub fn get_seed_item(&self, item_id: &str) -> Option<&SeedItemEntry> {
        self.data.seed_item_data.get(item_id)
    }

    pub fn get_soil(&self, soil_id: &str) -> Option<&SoilEntry> {
        self.data.soil_data.get(soil_id)
    }

    pub fn get_storager(&self, id: &str) -> Option<&StoragerEntry> {
        self.data.storager_data.get(id)
    }

    pub fn get_medic(&self, id: &str) -> Option<&MedicEntry> {
        self.data.medic_data.get(id)
    }

    pub fn get_skill(&self, skill_id: &str) -> Option<&SkillEntry> {
        self.data.skill_data.get(skill_id)
    }

    pub fn get_special_power_pole(&self, id: &str) -> Option<&SpecialPowerPoleEntry> {
        self.data.special_power_pole.get(id)
    }

    /// All special-power-pole entries whose `map_id` matches a given scene.
    /// Used when bootstrapping a region with its pre-placed power gates.
    pub fn special_power_poles_in_scene<'a>(
        &'a self,
        scene: &'a str,
    ) -> impl Iterator<Item = &'a SpecialPowerPoleEntry> {
        self.data
            .special_power_pole
            .values()
            .filter(move |e| e.map_id == scene)
    }

    pub fn get_special_building(&self, id: &str) -> Option<&SpecialBuildingEntry> {
        self.data.special_building_data.get(id)
    }

    pub fn building_for_item(&self, item_id: &str) -> Option<&BuildingItemEntry> {
        self.data.building_item_data.get(item_id)
    }

    pub fn item_for_building(&self, building_id: &str) -> Option<&BuildingItemEntry> {
        self.data.building_item_reverse_data.get(building_id)
    }

    pub fn get_quick_bar_type(&self, id: &str) -> Option<&QuickBarTypeEntry> {
        self.data.quick_bar_type_data.get(id)
    }

    pub fn get_building_panel_unlock(&self, id: &str) -> Option<&BuildingPanelUnlockEntry> {
        self.data.building_panel_unlock_data.get(id)
    }
}
