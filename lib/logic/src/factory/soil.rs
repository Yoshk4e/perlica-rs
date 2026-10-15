//! Soil / farming business logic.
//!
//! The soil component accepts a seed item, grows it over time based on
//! `growTotalProgress` (from `seedItemData`) and `growSpeed` (from
//! `soilData`), and when fully grown can be harvested to produce the
//! seed's outcome item.
//!
//! Three ops: `plant` (start growing), `harvest` (collect the grown
//! item), `cancel` (abort and clear the soil).

use config::factory_table::FTableAssets;

use crate::factory::{
    ActiveRecipe, FactoryManager, MachineKey, SoilMachine, current_tick, push_item_to_bag,
    take_from_inv,
};

impl FactoryManager {
    /// Plant a seed in a soil node. Consumes the seed from the player's
    /// bag, creates a `SoilMachine` entry with the seed's recipe + start
    /// tick. Growth is computed lazily from elapsed time.
    pub fn soil_plant(
        &mut self,
        assets: &FTableAssets,
        region_name: &str,
        node_id: u32,
        seed_item_id: &str,
    ) -> bool {
        // Verify the seed exists in config.
        if assets.get_seed_item(seed_item_id).is_none() {
            return false;
        }

        // The node's `template_id` is both the wire `machine_id` and the
        // `soilData` key that yields this soil's grow speed, so a soil op
        // against an unplaced node is rejected outright.
        let Some(machine_id) = self.machine_id_of(region_name, node_id).map(str::to_string) else {
            return false;
        };

        // Resolved once, at plant time, and carried in the timing envelope. A
        // soil upgraded mid-growth changes rate via `set_speed_at`, which banks
        // the progress already earned at the old rate.
        let grow_speed = assets
            .get_soil(&machine_id)
            .map_or(1, |soil| soil.grow_speed.max(1));

        let key = MachineKey::new(region_name, node_id);

        // Refuse to plant over a seed that is still growing; `insert` alone
        // would silently discard the standing crop.
        if let Some(existing) = self.soil_state.get(&key)
            && existing.planted_seed.is_some()
        {
            return false;
        }

        // Consume the seed from the bag.
        let Some(region) = self.region_mut(region_name) else {
            return false;
        };
        let Some(bag_node) = region.node_mut(1) else {
            return false;
        };
        let Some(bag_comp) = bag_node.component_mut(1) else {
            return false;
        };
        let crate::factory::FactoryComponent::Inventory(bag_inv) = bag_comp else {
            return false;
        };

        if !take_from_inv(bag_inv, seed_item_id, 1) {
            return false;
        }

        // Create or update the soil machine state.
        let now = current_tick();
        self.soil_state.insert(
            key,
            SoilMachine {
                region_name: region_name.to_string(),
                node_id,
                machine_id,
                planted_seed: Some(ActiveRecipe::start_at(now, seed_item_id, grow_speed)),
                doodad_state: crate::factory::SoilDoodadState::Growing,
            },
        );

        true
    }

    /// Harvest a grown seed. Checks if the growth is complete
    /// (`progress >= growTotalProgress`), and if so produces the outcome item
    /// into the bag and clears the soil.
    pub fn soil_harvest(
        &mut self,
        assets: &FTableAssets,
        region_name: &str,
        node_id: u32,
        _harvest_type: i32,
    ) -> bool {
        let key = MachineKey::new(region_name, node_id);
        let Some(machine) = self.soil_state.get(&key) else {
            return false;
        };
        let Some(recipe) = &machine.planted_seed else {
            return false;
        };

        let seed_id = recipe.recipe_id.clone();

        let Some(seed_entry) = assets.get_seed_item(&seed_id) else {
            return false;
        };

        // The grow speed is baked into the timing envelope at plant time.
        if !recipe.timing.is_complete(seed_entry.grow_total_progress) {
            return false;
        }

        // Growth complete -- produce the outcome. The seed's outcome
        // is the seed item itself (harvesting yields more seeds or the
        // crop item). The `doodad_id` field tells the client which
        // model to show; the actual outcome item is the seed's `id`.
        let outcome_item_id = seed_entry.id.clone();

        // Push the outcome into the bag.
        let Some(region) = self.region_mut(region_name) else {
            return false;
        };
        let Some(bag_node) = region.node_mut(1) else {
            return false;
        };
        let Some(bag_comp) = bag_node.component_mut(1) else {
            return false;
        };
        let crate::factory::FactoryComponent::Inventory(bag_inv) = bag_comp else {
            return false;
        };

        // Stack with existing or insert new.
        push_item_to_bag(bag_inv, &outcome_item_id, 1, 0);

        // Clear the soil.
        let Some(machine) = self.soil_state.get_mut(&key) else {
            return false;
        };
        machine.planted_seed = None;
        machine.doodad_state = crate::factory::SoilDoodadState::Empty;

        true
    }

    /// Cancel growing and clear the soil. The seed is lost (not returned).
    pub fn soil_cancel(&mut self, region_name: &str, node_id: u32) -> bool {
        let Some(machine) = self
            .soil_state
            .get_mut(&MachineKey::new(region_name, node_id))
        else {
            return false;
        };
        machine.planted_seed = None;
        machine.doodad_state = crate::factory::SoilDoodadState::Empty;
        true
    }
}
