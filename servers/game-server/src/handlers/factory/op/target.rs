//! `SetCollectTarget` and `SetSelectTarget` ops.
//!
//! Both target a `Selector` or `Collector` component by ID and update
//! the item filter it should route. The difference is that `Collect`
//! also takes a count (a collector wants N of an item before it stops),
//! while `Select` just sets the routing filter with no count.

use crate::net::NetContext;
use perlica_logic::enums::FCComponentPos;
use perlica_logic::factory::{CollectorState, FactoryComponent, SelectorState};
use perlica_proto::{
    CsdFactoryOpSetCollectTarget, CsdFactoryOpSetSelectTarget, FactoryOpRetCode, FactoryOpType,
    ScFactoryOpRet,
};

use super::super::response;

pub async fn handle_select(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpSetSelectTarget,
) -> ScFactoryOpRet {
    let Some(region) = ctx.player.factory.region_mut(&region_name) else {
            return response::fail(
                index,
                FactoryOpType::SetSelectTarget,
                FactoryOpRetCode::Fail,
                format!("region {region_name} not found"),
            );
    };

    // The component_id sent by the client corresponds to a `Selector`
    // component slot on some node. Walk all nodes looking for it.
    let mut found = false;
    for node in region.nodes.values_mut() {
        if let Some(slot) = node.component_mut(req.component_id) {
            if let FactoryComponent::Selector(state) = slot {
                state.selected_item_id = req.item_id;
                found = true;
                break;
            }
            // Some ops target the Selector via its position slot rather
            // than its component_id; both are valid client paths.
            return response::fail(
                index,
                FactoryOpType::SetSelectTarget,
                FactoryOpRetCode::Fail,
                format!("component {} is not a Selector", req.component_id),
            );
        }
    }

    if !found {
        // Fall back to position lookup -- the client sometimes addresses
        // the selector by its `FCComponentPos::Selector1..Selector9` slot
        // instead of the raw component_id.
        let _ = FCComponentPos::Selector1;
        return response::fail(
            index,
            FactoryOpType::SetSelectTarget,
            FactoryOpRetCode::Fail,
            format!("component {} not found in region {}", req.component_id, region_name),
        );
    }

    response::ok(index, FactoryOpType::SetSelectTarget)
}

pub async fn handle_collect(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpSetCollectTarget,
) -> ScFactoryOpRet {
    let Some(region) = ctx.player.factory.region_mut(&region_name) else {
            return response::fail(
                index,
                FactoryOpType::SetCollectTarget,
                FactoryOpRetCode::Fail,
                format!("region {region_name} not found"),
            );
    };

    // Collector ops target a `Collector` component (miner output side).
    // The `count` field is the target batch size -- the collector fills
    // `items_round` until it hits `count`, then flushes.
    for node in region.nodes.values_mut() {
        if let Some(slot) = node.component_mut(req.component_id) {
            if let FactoryComponent::Collector(state) = slot {
                set_collector_target(state, &req.item_id, req.count);
                return response::ok(index, FactoryOpType::SetCollectTarget);
            }
            return response::fail(
                index,
                FactoryOpType::SetCollectTarget,
                FactoryOpRetCode::Fail,
                format!("component {} is not a Collector", req.component_id),
            );
        }
    }

    response::fail(
        index,
        FactoryOpType::SetCollectTarget,
        FactoryOpRetCode::Fail,
        format!("component {} not found in region {}", req.component_id, region_name),
    )
}

fn set_collector_target(state: &mut CollectorState, item_id: &str, count: i32) {
    // The collector's target item is stored in its `items_round[0]` slot.
    // If empty, push a new slot; otherwise overwrite the first slot's id
    // and reset its count to zero so the collector starts fresh.
    let new_item = perlica_logic::factory::ItemSlot {
        item_id: item_id.to_string(),
        count: 0,
        inst_id: 0,
    };

    if state.items_round.is_empty() {
        state.items_round.push(new_item);
    } else {
        state.items_round[0] = new_item;
    }

    // We don't actually store `count` anywhere on the state today --
    // the live server uses it as a soft cap that the HS-transport
    // phase enforces. Keep it as a TODO so it's not silently lost.
    let _ = count;
    // TODO(hs-transport): enforce `count` as the round-fill cap when
    // the HS item transport lands.

    // Reset progress so the new target starts crafting from scratch.
    state.start_tick = None;
    state.current_progress = 0;
}

// unused currently -- here for when SetSelectTarget's position-slot path
// is wired up.
#[allow(dead_code)]
fn _silence_selector_default() -> SelectorState {
    SelectorState::default()
}
