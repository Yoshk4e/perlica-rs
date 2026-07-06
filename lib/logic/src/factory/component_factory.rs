//! Component factory - transform a building template id to the set of
//! `(component_id, FactoryComponent)` tuples that should seed the node.
//!
//! Work item for clause 3.4. Stub for clause 1 in order to allow `bootstrap_region`
//! to compile with a stable API and implement the clause 3 logic without changing
//! every call site.

use std::collections::HashMap;

use crate::enums::FCComponentPos;
use crate::factory::component::FactoryComponent;

/// Result of building a node from a template: the component list (in wire
/// order) plus the position->component_id map.
#[derive(Debug, Clone)]
pub struct BuiltComponents {
    pub components: Vec<(u32, FactoryComponent)>,
    pub component_pos: HashMap<FCComponentPos, u32>,
}

/// Creates the components of a building of `template_id` (e.g. `"sp_hub_1"`).
///
/// Clause 1 behavior: returns `None`, caller (`FactoryManager::bootstrap_region`)
/// hand-crafts the hub layout. Clause 3 should:
///
/// 1. Find `template_id` in `FTableAssets::get_building()` to get the
///    `BuildingEntry` (grid size, ports, FCNodeType).
/// 2. Find the sub-table corresponding to that building type (e.g.
///    `get_hub()` for `sp_hub_1`, `get_machine_crafter()` for `mc_*`,
///    `get_miner()` for `miner_*`, etc.) to extract individual component
///    configuration (power generation, fuel energy, crafting speed, etc.).
/// 3. Dispatch based on `FCNodeType`:
///    - `Hub` -> `Hub + BusLoader + PowerPole + PowerSave + StablePower +
///      Selector + Inventory` (7 components, same as `bootstrap_region`).
///    - `Producer` -> `Producer + Cache(in) + Cache(out) + Selector +
///      PowerPole` (see Clause 8 task 8.5).
///    - `PowerPole` -> `PowerPole`.
///    - `BurnPower` -> `BurnPower + PowerPole`.
///    - etc.
/// 4. Set ID for each component starting from 1 (or 2 for `Transform`
///    component, TODO: check with live server format).
///
/// Returns `None` for non-known templates, so that the caller can fall back
/// to its inline layout (currently just `sp_hub_1`) until Clause 3.
// TODO(Clause 3.4): implement properly.
pub fn create_components_from_template(_template_id: &str) -> Option<BuiltComponents> {
    None
}
