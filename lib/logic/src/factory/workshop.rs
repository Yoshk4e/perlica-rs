//! Workshop crafting business logic.
//!
//! The workshop produces buildings and items from recipes in
//! `workshopCraftData`. Each recipe has ingredients, outcomes, and a
//! `default_unlock` flag. Recipes that aren't `default_unlock` need to
//! be unlocked via STT (Clause 17) before they can be crafted.
//!
//! `multi` on the request means "craft N times" -- ingredients are
//! consumed N times and outcomes produced N times.

use config::factory_table::FTableAssets;

use crate::factory::{
    FactoryComponent, FactoryManager, ItemSlot, push_item_to_bag, take_bill_from_inv,
};

impl FactoryManager {
    /// Craft an item via the workshop. Consumes ingredients from the
    /// player's bag (inventory node), produces outcomes into the bag.
    /// Returns the produced items, or an error if the recipe doesn't
    /// exist, isn't unlocked, or ingredients are insufficient.
    pub fn workshop_make(
        &mut self,
        assets: &FTableAssets,
        region_name: &str,
        formula_id: &str,
        multi: u32,
    ) -> Result<Vec<ItemSlot>, WorkshopError> {
        let recipe = assets
            .get_workshop_craft(formula_id)
            .ok_or(WorkshopError::RecipeNotFound)?;

        // Check unlock -- if not default_unlock, the player needs it in
        // their unlocked formulas list. TODO: wire STT unlock check
        // once Clause 17 lands. For now, only default_unlock recipes
        // are craftable.
        if !recipe.default_unlock {
            return Err(WorkshopError::NotUnlocked);
        }

        let multi = multi.max(1);

        // The whole bill for all `multi` crafts, priced before the bag is
        // touched. The old per-ingredient loop returned on the first shortfall
        // having already consumed the ingredients ahead of it.
        let bill: Vec<(&str, u32)> = recipe
            .ingredients
            .iter()
            .map(|ing| (ing.id.as_str(), ing.count.saturating_mul(multi)))
            .collect();

        // Consume ingredients from the bag (inventory node_id=1).
        let region = self
            .region_mut(region_name)
            .ok_or(WorkshopError::RegionNotFound)?;

        let bag_node = region.node_mut(1).ok_or(WorkshopError::InventoryMissing)?;
        let bag_comp = bag_node
            .component_mut(1)
            .ok_or(WorkshopError::InventoryMissing)?;
        let FactoryComponent::Inventory(bag_inv) = bag_comp else {
            return Err(WorkshopError::InventoryMissing);
        };

        if !take_bill_from_inv(bag_inv, &bill) {
            return Err(WorkshopError::InsufficientIngredients);
        }

        // Produce outcomes into the bag.
        let mut produced = vec![];
        for outcome in &recipe.outcomes {
            let count = outcome.count.saturating_mul(multi);
            produced.push(ItemSlot {
                item_id: outcome.id.clone(),
                count,
                inst_id: 0,
            });
        }

        // Push outcomes into the bag. The old loop only ever looked at slot 0
        // and then `insert(0, ..)`'d on a mismatch, overwriting whatever item
        // happened to be sitting in that slot.
        for item in &produced {
            push_item_to_bag(bag_inv, &item.item_id, item.count, item.inst_id);
        }

        Ok(produced)
    }
}

#[derive(Debug)]
pub enum WorkshopError {
    RecipeNotFound,
    NotUnlocked,
    RegionNotFound,
    InventoryMissing,
    InsufficientIngredients,
}
