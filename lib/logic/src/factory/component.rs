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
#[derive(Debug, Clone)]
pub struct BurnPowerState {
    pub fuel_remaining: i64,
    pub fuel_start_tick: Option<Tick>,
    pub in_power: bool,
    /// Power output per tick when fuel is burning. From `powerStationData.power_provide`.
    pub power_gen_per_sec: i64,
    /// What fuel item is currently burning. Empty when no fuel loaded.
    pub current_burn_item_id: String,
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
    pub in_power: bool,
    pub power_cost: i64,
    /// Current heal-cast progress, 0..total_progress.
    pub current_progress: i64,
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
    pub in_power: bool,
    pub in_use: bool,
    pub power_cost: i64,
    /// Transfer progress for the current item, 0..total_progress.
    pub current_progress: i64,
    /// Total ticks needed to move one item-stack from source to target.
    pub total_progress: i64,
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

    /// Serialize to the wire format. Maps each variant's state to the
    /// matching `ScdFactorySyncComponent` payload. Variants with no
    /// state (Hub, Transform, Bus, etc.) get `component_payload: None`.
    pub fn to_proto(&self, component_id: u32) -> perlica_proto::ScdFactorySyncComponent {
        use perlica_proto::{
            ScdFactorySyncComponent, ScdFactorySyncComponentBoxConveyor,
            ScdFactorySyncComponentBurnPower, ScdFactorySyncComponentBusLoader,
            ScdFactorySyncComponentCache, ScdFactorySyncComponentCacheTransport,
            ScdFactorySyncComponentCollector, ScdFactorySyncComponentGridBox,
            ScdFactorySyncComponentHealTower, ScdFactorySyncComponentInventory,
            ScdFactorySyncComponentPowerPole, ScdFactorySyncComponentPowerSave,
            ScdFactorySyncComponentProducer, ScdFactorySyncComponentSelector,
            ScdFactorySyncComponentStablePower,
            ScdFactorySyncComponentTravelPole,
            scd_factory_sync_component::ComponentPayload,
        };

        let component_type = self.discriminant();
        let payload = match self {
            Self::Transform | Self::Bus | Self::FormulaMan | Self::BoxRouterM1
            | Self::BusUnloader | Self::Hub | Self::BoxBridge | Self::SpecialDesc => None,

            Self::Inventory(state) => Some(ComponentPayload::Inventory(
                ScdFactorySyncComponentInventory {
                    // Proto uses inst_id -> count; our internal map already
                    // keys by inst_id (0 for stackables).
                    items: state
                        .items
                        .iter()
                        .map(|(&inst_id, slot)| (inst_id, slot.count as i32))
                        .collect(),
                },
            )),

            Self::Cache(state) => Some(ComponentPayload::Cache(ScdFactorySyncComponentCache {
                items: state.items.iter().map(item_to_proto).collect(),
                ports: state.ports.iter().map(cache_port_to_proto).collect(),
            })),

            Self::Selector(state) => Some(ComponentPayload::Selector(
                ScdFactorySyncComponentSelector {
                    selected_item_id: state.selected_item_id.clone(),
                    ports: state.ports.iter().map(cache_port_to_proto).collect(),
                },
            )),

            Self::Collector(state) => Some(ComponentPayload::Collector(
                ScdFactorySyncComponentCollector {
                    items_round: state.items_round.iter().map(item_to_proto).collect(),
                    current_progress: state.current_progress as i64,
                    in_power: state.in_power,
                    in_block: state.in_block,
                    power_cost: state.power_cost,
                },
            )),

            Self::Producer(state) => Some(ComponentPayload::Producer(
                ScdFactorySyncComponentProducer {
                    formula_id: state.formula_id.clone(),
                    current_progress: state.current_progress as i64,
                    in_power: state.in_power,
                    in_block: state.in_block,
                    power_cost: state.power_cost,
                    last_formula_id: state.last_formula_id.clone(),
                },
            )),

            Self::BoxConveyor(state) => Some(ComponentPayload::BoxConveyor(
                ScdFactorySyncComponentBoxConveyor {
                    last_pop_tms: 0,
                    items: state.items.iter().map(conveyor_item_to_proto).collect(),
                },
            )),

            Self::BusLoader(state) => Some(ComponentPayload::BusLoader(
                ScdFactorySyncComponentBusLoader {
                    last_putin_item_id: state.last_putin_item_id.clone(),
                    ports: state.ports.iter().map(cache_port_to_proto).collect(),
                },
            )),

            Self::BurnPower(state) => Some(ComponentPayload::BurnPower(
                ScdFactorySyncComponentBurnPower {
                    current_least_progress: state.fuel_remaining,
                    current_burn_item_id: state.current_burn_item_id.clone(),
                    power_gen_per_sec: state.power_gen_per_sec,
                    in_power: state.in_power,
                },
            )),

            Self::PowerPole(state) => Some(ComponentPayload::PowerPole(
                ScdFactorySyncComponentPowerPole {
                    in_power: state.in_power,
                },
            )),

            Self::PowerSave(state) => Some(ComponentPayload::PowerSave(
                ScdFactorySyncComponentPowerSave {
                    power_save: state.power_save,
                    in_power: state.in_power,
                },
            )),

            Self::GridBox(state) => Some(ComponentPayload::GridBox(
                ScdFactorySyncComponentGridBox {
                    items: state.items.iter().map(item_to_proto).collect(),
                    ports: vec![],
                },
            )),

            Self::HealTower(state) => Some(ComponentPayload::HealTower(
                ScdFactorySyncComponentHealTower {
                    in_power: state.in_power,
                    current_progress: state.current_progress,
                    current_point: state.points as i32,
                    power_cost: state.power_cost,
                },
            )),

            Self::CacheTransport(state) => Some(ComponentPayload::CacheTransport(
                ScdFactorySyncComponentCacheTransport {
                    current_progress: state.current_progress,
                    total_progress: state.total_progress,
                    auto_transport: state.enabled,
                    in_power: state.in_power,
                    in_use: state.in_use,
                    power_cost: state.power_cost,
                },
            )),

            Self::StablePower(state) => Some(ComponentPayload::StablePower(
                ScdFactorySyncComponentStablePower {
                    in_power: state.in_power,
                    power_gen_per_sec: state.power_gen_per_sec,
                },
            )),

            Self::TravelPole(state) => Some(ComponentPayload::TravelPole(
                ScdFactorySyncComponentTravelPole {
                    in_power: true,
                    power_cost: 0,
                    default_next: state.default_next.unwrap_or(0),
                },
            )),
        };

        ScdFactorySyncComponent {
            component_id,
            component_type,
            component_payload: payload,
        }
    }
}

fn item_to_proto(slot: &ItemSlot) -> perlica_proto::ScdFactorySyncItem {
    perlica_proto::ScdFactorySyncItem {
        id: slot.item_id.clone(),
        count: slot.count as i32,
        tms: 0,
    }
}

fn conveyor_item_to_proto(item: &ConveyorItem) -> perlica_proto::ScdFactorySyncItem {
    perlica_proto::ScdFactorySyncItem {
        id: item.item.item_id.clone(),
        count: item.item.count as i32,
        tms: (item.progress * 1000.0) as i64,
    }
}

fn cache_port_to_proto(port: &CachePort) -> perlica_proto::ScdFactorySyncComponentSubPort {
    perlica_proto::ScdFactorySyncComponentSubPort {
        index: port.direction,
        bind_com_id: 0,
        in_block: false,
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
        assert!(!FactoryComponent::Hub.is_power_source());
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
                in_power: false,
                power_gen_per_sec: 100,
                current_burn_item_id: String::new(),
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
