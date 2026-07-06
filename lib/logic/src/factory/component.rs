//! Component state + the 17-variant `FactoryComponent` enum.
//!
//! Phase 1 only owns the *internal* Rust model. `to_proto()` conversions
//! land in Phase 3 (tasks 3.1, 3.5–3.10). Until then, the variants here
//! mirror `FCComponentType` exactly so the Phase 3 work is mechanical.
//!
//! See §2.1 of the implementation plan for the per-variant state contract.

use std::collections::HashMap;

use super::GridPos;
use crate::factory::tick::Tick;

/// A port on a cache / selector / bus-loader: grid position + facing
/// direction. Distinct from `SubPort` (which is for conveyor belt ports)
/// even though the field shape is identical, because the live server
/// treats them as separate types in the wire format and Phase 3's
/// `to_proto()` needs to map each one independently.
#[derive(Debug, Clone)]
pub struct CachePort {
    pub position: GridPos,
    pub direction: i32,
}

/// One item slot in an inventory or cache.
///
/// `inst_id == 0` for stackable items (ores, ingredients, etc.) and the
/// instance ID for non-stackable items (equipments, weapons). The hub's
/// inventory mixes both freely, which is why this is a single struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSlot {
    pub item_id: String,
    pub count: u32,
    pub inst_id: u32,
}

/// State for a `Producer` component (miner, machine crafter).
///
/// `start_tick == None` means "not crafting right now", either no formula
/// selected, paused, or power-loss reset. `current_progress` is the
/// snapshotted progress at the moment we paused/saved; live progress is
/// `current_progress + (current_tick - start_tick) * speed`.
#[derive(Debug, Clone)]
pub struct ProducerState {
    pub formula_id: String,
    pub start_tick: Option<Tick>,
    pub current_progress: u64,
    pub in_power: bool,
    pub in_block: bool,
    pub power_cost: i64,
    /// Used by signal dispatch to remember the previous recipe after a
    /// cache clears. Empty string on first run.
    pub last_formula_id: String,
}

/// State for a `Cache` component: buffer slots on crafters.
#[derive(Debug, Clone)]
pub struct CacheState {
    pub items: Vec<ItemSlot>,
    pub ports: Vec<CachePort>,
}

/// State for a `Collector` component: miner output side.
#[derive(Debug, Clone)]
pub struct CollectorState {
    pub items_round: Vec<ItemSlot>,
    pub current_progress: u64,
    pub start_tick: Option<Tick>,
    pub in_power: bool,
    pub in_block: bool,
    pub power_cost: i64,
}

/// State for an `Inventory` component: hub storage, also used by depots.
///
/// Keyed by `inst_id` so non-stackable items don't collide. Stackable items
/// all use `inst_id == 0` and get merged on insert.
#[derive(Debug, Clone, Default)]
pub struct InventoryState {
    pub items: HashMap<u32, ItemSlot>,
}

/// State for a `Selector` component: item filter / output routing.
#[derive(Debug, Clone, Default)]
pub struct SelectorState {
    pub selected_item_id: String,
    pub ports: Vec<CachePort>,
}

/// State for a `BusLoader` component.
#[derive(Debug, Clone, Default)]
pub struct BusLoaderState {
    pub last_putin_item_id: String,
    pub ports: Vec<CachePort>,
}

/// State for a `PowerPole` component.
#[derive(Debug, Clone, Copy)]
pub struct PowerPoleState {
    pub in_power: bool,
}

/// State for a `PowerSave` component (battery).
#[derive(Debug, Clone, Copy)]
pub struct PowerSaveState {
    pub power_save: i64,
    pub in_power: bool,
}

/// State for a `StablePower` component (passive generation, e.g. hub).
#[derive(Debug, Clone, Copy)]
pub struct StablePowerState {
    pub in_power: bool,
    pub power_gen_per_sec: i64,
}

/// State for a `BurnPower` component (fuel-burning generator).
#[derive(Debug, Clone, Copy)]
pub struct BurnPowerState {
    pub fuel_remaining: i64,
    pub fuel_start_tick: Option<Tick>,
    pub in_power: bool,
}

/// State for a `GridBox` component: multi-slot storage grid used by
/// depot-style buildings. Capacity per slot is item-specific.
#[derive(Debug, Clone, Default)]
pub struct GridBoxState {
    pub items: Vec<ItemSlot>,
}

/// State for a `HealTower` component.
#[derive(Debug, Clone, Copy)]
pub struct HealTowerState {
    pub points: i64,
}

/// State for a `TravelPole` component: fast-travel waypoint.
#[derive(Debug, Clone, Copy, Default)]
pub struct TravelPoleState {
    /// Node ID of the next pole on the route. `None` = terminal.
    pub default_next: Option<u32>,
}

/// One item on a conveyor belt. `progress` is `0.0..=1.0` along the belt.
#[derive(Debug, Clone)]
pub struct ConveyorItem {
    pub item: ItemSlot,
    pub progress: f32,
}

/// State for a `BoxConveyor` component: belt segment.
#[derive(Debug, Clone)]
pub struct BoxConveyorState {
    pub items: Vec<ConveyorItem>,
    pub direction: i32,
}

/// State for a `CacheTransport` component: explicit cache-to-cache mover.
#[derive(Debug, Clone, Copy)]
pub struct CacheTransportState {
    pub enabled: bool,
    pub source_node_id: u32,
    pub target_node_id: u32,
}

/// All 17 component variants matching `FCComponentType`. Order matters for
/// wire format on Phase 3; keep this in the same order as the proto enum.
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

impl FactoryComponent {
    /// Discriminant as the matching `FCComponentType` integer. Used by the
    /// Phase 3 `to_proto()` so we have one source of truth for the
    /// variant->integer mapping.
    // TODO(phase 3): replace body with `FCComponentType::from(self).into()` once
    // the From impl lands alongside to_proto.
    pub fn discriminant(&self) -> i32 {
        use crate::enums::FCComponentType as T;
        (match self {
            Self::Transform => T::Transform,
            Self::Bus => T::Bus,
            Self::Inventory(_) => T::Inventory,
            Self::Cache(_) => T::Cache,
            Self::Selector(_) => T::Selector,
            Self::Collector(_) => T::Collector,
            Self::Producer(_) => T::Producer,
            Self::FormulaMan => T::FormulaMan,
            Self::BoxConveyor(_) => T::BoxConveyor,
            Self::BoxRouterM1 => T::BoxRouterM1,
            Self::BusUnloader => T::BusUnloader,
            Self::BusLoader(_) => T::BusLoader,
            Self::Hub => T::Hub,
            Self::BurnPower(_) => T::BurnPower,
            Self::PowerPole(_) => T::PowerPole,
            Self::PowerSave(_) => T::PowerSave,
            Self::GridBox(_) => T::GridBox,
            Self::HealTower(_) => T::HealTower,
            Self::CacheTransport(_) => T::CacheTransport,
            Self::StablePower(_) => T::StablePower,
            Self::TravelPole(_) => T::TravelPole,
            Self::BoxBridge => T::BoxBridge,
            Self::SpecialDesc => T::SpecialDesc,
        }) as i32
    }

    /// True if this component participates in the power graph as a consumer
    /// (i.e. it should be reset on power loss — see §4.4).
    pub fn is_power_consumer(&self) -> bool {
        matches!(self, Self::Producer(_) | Self::Collector(_))
    }

    /// True if this component is a power source (`StablePower` or `BurnPower`).
    pub fn is_power_source(&self) -> bool {
        matches!(self, Self::StablePower(_) | Self::BurnPower(_))
    }

    /// True if this component is a power relay (`PowerPole`).
    pub fn is_power_relay(&self) -> bool {
        matches!(self, Self::PowerPole(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discriminant_matches_fc_component_type() {
        use crate::enums::FCComponentType as T;
        assert_eq!(FactoryComponent::Hub.discriminant(), T::Hub as i32);
        assert_eq!(
            FactoryComponent::Inventory(InventoryState::default()).discriminant(),
            T::Inventory as i32
        );
        assert_eq!(
            FactoryComponent::Producer(ProducerState {
                formula_id: String::new(),
                start_tick: None,
                current_progress: 0,
                in_power: false,
                in_block: false,
                power_cost: 0,
                last_formula_id: String::new(),
            })
            .discriminant(),
            T::Producer as i32
        );
    }

    #[test]
    fn power_role_helpers() {
        assert!(FactoryComponent::Hub.is_power_source() == false);
        assert!(
            FactoryComponent::StablePower(StablePowerState {
                in_power: true,
                power_gen_per_sec: 100
            })
            .is_power_source()
        );
        assert!(
            FactoryComponent::BurnPower(BurnPowerState {
                fuel_remaining: 0,
                fuel_start_tick: None,
                in_power: false
            })
            .is_power_source()
        );
        assert!(FactoryComponent::PowerPole(PowerPoleState { in_power: true }).is_power_relay());
        assert!(
            FactoryComponent::Producer(ProducerState {
                formula_id: String::new(),
                start_tick: None,
                current_progress: 0,
                in_power: false,
                in_block: false,
                power_cost: 0,
                last_formula_id: String::new(),
            })
            .is_power_consumer()
        );
    }
}
