//! Component factory - build the component list for a node from its
//! building template id.
//!
//! Given a template like `"sp_hub_1"` or `"power_pole_1"`, this looks up
//! the building entry + any sub-table data and returns the correct set
//! of `(component_id, FactoryComponent)` pairs plus the position map.
//!
//! The dispatch is on `FCNodeType` (from `BuildingEntry.building_type`)
//! since that's the single source of truth for "what kind of building
//! is this".

use std::collections::HashMap;

use config::factory_table::FTableAssets;

use crate::enums::FCComponentPos;
use crate::factory::component::{
    BurnPowerState, BusLoaderState, FactoryComponent, InventoryState, PowerPoleState,
    PowerSaveState, ProducerState, SelectorState, StablePowerState,
};

#[derive(Debug, Clone)]
pub struct BuiltComponents {
    pub components: Vec<(u32, FactoryComponent)>,
    pub component_pos: HashMap<FCComponentPos, u32>,
}

/// Build components for `template_id` by looking up the building entry
/// and dispatching on its `FCNodeType`. Returns `None` if the template
/// isn't in `buildingData` or has an unknown type.
///
/// Component IDs start at 1 and increment in the order components are
/// pushed. The `component_pos` map uses `FCComponentPos` enum keys so
/// the client can find each component by its position slot.
pub fn create_components_from_template(
    template_id: &str,
    assets: &FTableAssets,
) -> Option<BuiltComponents> {
    let building = assets.get_building(template_id)?;
    let node_type = node_type_from_i32(building.building_type)?;

    let mut components = Vec::new();
    let mut component_pos = HashMap::new();
    let mut next_id = 1u32;

    let add = |pos: FCComponentPos,
               comp: FactoryComponent,
               components: &mut Vec<(u32, FactoryComponent)>,
               component_pos: &mut HashMap<FCComponentPos, u32>,
               next_id: &mut u32| {
        let id = *next_id;
        *next_id += 1;
        components.push((id, comp));
        component_pos.insert(pos, id);
    };

    match node_type {
        crate::enums::FCNodeType::Hub => {
            let hub_data = assets.get_hub(template_id);
            let (power_gen, power_save_max) = hub_data.map_or((100, 100_000), |h| {
                (h.power_generate, h.power_storage_capacity)
            });

            add(
                FCComponentPos::Hub,
                FactoryComponent::Hub,
                &mut components,
                &mut component_pos,
                &mut next_id,
            );
            add(
                FCComponentPos::BusLoader,
                FactoryComponent::BusLoader(BusLoaderState::default()),
                &mut components,
                &mut component_pos,
                &mut next_id,
            );
            add(
                FCComponentPos::PowerPole,
                FactoryComponent::PowerPole(PowerPoleState { in_power: true }),
                &mut components,
                &mut component_pos,
                &mut next_id,
            );
            add(
                FCComponentPos::PowerSave,
                FactoryComponent::PowerSave(PowerSaveState {
                    power_save: power_save_max,
                    in_power: true,
                }),
                &mut components,
                &mut component_pos,
                &mut next_id,
            );
            add(
                FCComponentPos::StablePower,
                FactoryComponent::StablePower(StablePowerState {
                    in_power: true,
                    power_gen_per_sec: power_gen,
                }),
                &mut components,
                &mut component_pos,
                &mut next_id,
            );
            add(
                FCComponentPos::Selector,
                FactoryComponent::Selector(SelectorState::default()),
                &mut components,
                &mut component_pos,
                &mut next_id,
            );
            add(
                FCComponentPos::Inventory,
                FactoryComponent::Inventory(InventoryState::default()),
                &mut components,
                &mut component_pos,
                &mut next_id,
            );
        }

        crate::enums::FCNodeType::PowerPole => {
            add(
                FCComponentPos::PowerPole,
                FactoryComponent::PowerPole(PowerPoleState { in_power: false }),
                &mut components,
                &mut component_pos,
                &mut next_id,
            );
        }

        crate::enums::FCNodeType::BurnPower => {
            let (power_gen, burn_speed) = assets
                .get_power_station(template_id)
                .map_or((0, 0), |ps| (ps.power_provide, ps.burn_speed));

            // BurnSpeed is stored on the component so fuel math can use it
            // without re-looking-up the config every tick. We stash it in
            // `current_burn_item_id` as a stringified i64 for now -- TODO:
            // add a real `burn_speed` field to BurnPowerState once the
            // power system lands.
            let _ = burn_speed;

            add(
                FCComponentPos::BurnPower,
                FactoryComponent::BurnPower(BurnPowerState {
                    fuel_remaining: 0,
                    fuel_start_tick: None,
                    in_power: false,
                    power_gen_per_sec: power_gen,
                    current_burn_item_id: String::new(),
                }),
                &mut components,
                &mut component_pos,
                &mut next_id,
            );
            add(
                FCComponentPos::PowerPole,
                FactoryComponent::PowerPole(PowerPoleState { in_power: false }),
                &mut components,
                &mut component_pos,
                &mut next_id,
            );
        }

        crate::enums::FCNodeType::Producer => {
            // Speed from machineCrafterData; falls back to the table default
            // (100) if the entry is missing or the field isn't set.
            let _speed = assets
                .get_machine_crafter(template_id)
                .map_or(100, |mc| mc.speed);

            add(
                FCComponentPos::Producer,
                FactoryComponent::Producer(ProducerState {
                    formula_id: String::new(),
                    start_tick: None,
                    current_progress: 0,
                    in_power: false,
                    in_block: false,
                    power_cost: building.power_consume,
                    last_formula_id: String::new(),
                }),
                &mut components,
                &mut component_pos,
                &mut next_id,
            );
            add(
                FCComponentPos::PowerPole,
                FactoryComponent::PowerPole(PowerPoleState { in_power: false }),
                &mut components,
                &mut component_pos,
                &mut next_id,
            );
        }

        crate::enums::FCNodeType::Collector => {
            // Miner speed lives in minerData.speed. We don't store it on
            // the Collector component itself yet -- the completion checker
            // (Clause 8) will look it up from config at tick time.
            let _speed = assets
                .get_miner(template_id)
                .map_or(250, |m| m.speed);

            add(
                FCComponentPos::Collector,
                FactoryComponent::Collector(crate::factory::component::CollectorState {
                    items_round: vec![],
                    current_progress: 0,
                    start_tick: None,
                    in_power: false,
                    in_block: false,
                    power_cost: building.power_consume,
                }),
                &mut components,
                &mut component_pos,
                &mut next_id,
            );
            add(
                FCComponentPos::PowerPole,
                FactoryComponent::PowerPole(PowerPoleState { in_power: false }),
                &mut components,
                &mut component_pos,
                &mut next_id,
            );
        }

        crate::enums::FCNodeType::HealTower => {
            add(
                FCComponentPos::HealTower,
                FactoryComponent::HealTower(crate::factory::component::HealTowerState {
                    points: 0,
                    in_power: false,
                    power_cost: building.power_consume,
                    current_progress: 0,
                }),
                &mut components,
                &mut component_pos,
                &mut next_id,
            );
            add(
                FCComponentPos::PowerPole,
                FactoryComponent::PowerPole(PowerPoleState { in_power: false }),
                &mut components,
                &mut component_pos,
                &mut next_id,
            );
        }

        crate::enums::FCNodeType::TravelPole => {
            add(
                FCComponentPos::TravelPole,
                FactoryComponent::TravelPole(crate::factory::component::TravelPoleState {
                    default_next: None,
                }),
                &mut components,
                &mut component_pos,
                &mut next_id,
            );
        }

        crate::enums::FCNodeType::DepositBox | crate::enums::FCNodeType::BoxBridge => {
            add(
                FCComponentPos::GridBox1,
                FactoryComponent::GridBox(crate::factory::component::GridBoxState::default()),
                &mut components,
                &mut component_pos,
                &mut next_id,
            );
        }

        // For types we don't have a layout for yet, just add a PowerPole
        // so the node at least exists on the wire. TODO: add proper layouts.
        _ => {
            add(
                FCComponentPos::PowerPole,
                FactoryComponent::PowerPole(PowerPoleState { in_power: false }),
                &mut components,
                &mut component_pos,
                &mut next_id,
            );
        }
    }

    Some(BuiltComponents {
        components,
        component_pos,
    })
}

fn node_type_from_i32(t: i32) -> Option<crate::enums::FCNodeType> {
    use crate::enums::FCNodeType;
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
