//! `UseHealTowerPoint` and `SetTravelPoleDefaultNext` ops.
//!
//! Both target a specific component on a specific node (the heal tower
//! or travel pole). They're the simplest of the op family because
//! there's no grid math, just state mutation on the component.

use crate::net::NetContext;
use perlica_logic::factory::{FactoryComponent, HealTowerState};
use perlica_proto::{
    CsdFactoryOpSetTravelPoleDefaultNext, CsdFactoryOpUseHealTowerPoint, FactoryOpRetCode,
    FactoryOpType, ScFactoryOpRet,
};

use super::super::response;

pub async fn handle_use_heal_tower(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpUseHealTowerPoint,
) -> ScFactoryOpRet {
    let region = match ctx.player.factory.region_mut(&region_name) {
        Some(r) => r,
        None => {
            return response::fail(
                index,
                FactoryOpType::UseHealTowerPoint,
                FactoryOpRetCode::Fail,
                format!("region {} not found", region_name),
            );
        }
    };

    for node in region.nodes.values_mut() {
        if let Some(slot) = node.component_mut(req.component_id) {
            if let FactoryComponent::HealTower(state) = slot {
                return apply_heal_use(state, req, index);
            }
            return response::fail(
                index,
                FactoryOpType::UseHealTowerPoint,
                FactoryOpRetCode::Fail,
                format!("component {} is not a HealTower", req.component_id),
            );
        }
    }

    response::fail(
        index,
        FactoryOpType::UseHealTowerPoint,
        FactoryOpRetCode::Fail,
        format!("component {} not found", req.component_id),
    )
}

fn apply_heal_use(
    state: &mut HealTowerState,
    req: CsdFactoryOpUseHealTowerPoint,
    index: String,
) -> ScFactoryOpRet {
    // Each tower has a pool of heal points that regenerates over time
    // (per `FacSkillConst`). The regen math lives elsewhere; here we
    // just deduct what the player asked for and report how many we
    // actually granted (may be less than requested if the pool ran dry).
    let requested = req.use_count as i64;
    let granted = requested.min(state.points.max(0));
    state.points -= granted;

    // TODO: apply the actual heal to the player's characters here. The
    // tower just deducts points; the heal target is implicit (the
    // active team's HP pool). Needs the character-bag integration.
    let _ = ctx_noop();

    response::ok_with_use_heal_tower(index, granted as u32)
}

pub async fn handle_set_travel_pole_next(
    ctx: &mut NetContext<'_>,
    index: String,
    region_name: String,
    req: CsdFactoryOpSetTravelPoleDefaultNext,
) -> ScFactoryOpRet {
    let region = match ctx.player.factory.region_mut(&region_name) {
        Some(r) => r,
        None => {
            return response::fail(
                index,
                FactoryOpType::SetTravelPoleDefaultNext,
                FactoryOpRetCode::Fail,
                format!("region {} not found", region_name),
            );
        }
    };

    // Verify the target node exists before wiring the link -- otherwise
    // we'd silently point at a non-existent pole and the travel network
    // would route players into the void.
    if !region.nodes.contains_key(&req.default_next) {
        return response::fail(
            index,
            FactoryOpType::SetTravelPoleDefaultNext,
            FactoryOpRetCode::Fail,
            format!("target node {} not found", req.default_next),
        );
    }

    for node in region.nodes.values_mut() {
        if let Some(slot) = node.component_mut(req.component_id) {
            if let FactoryComponent::TravelPole(state) = slot {
                state.default_next = Some(req.default_next);
                return response::ok(index, FactoryOpType::SetTravelPoleDefaultNext);
            }
            return response::fail(
                index,
                FactoryOpType::SetTravelPoleDefaultNext,
                FactoryOpRetCode::Fail,
                format!("component {} is not a TravelPole", req.component_id),
            );
        }
    }

    response::fail(
        index,
        FactoryOpType::SetTravelPoleDefaultNext,
        FactoryOpRetCode::Fail,
        format!("component {} not found", req.component_id),
    )
}

// Tiny noop so `apply_heal_use` can compile without dragging `ctx` into
// its signature just for the TODO line. Drop once the heal target is wired.
fn ctx_noop() {}
