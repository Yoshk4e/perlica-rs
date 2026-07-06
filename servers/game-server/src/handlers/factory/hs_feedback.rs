//! `CsFactoryHsFb` handler -- handshake feedback from the client.
//!
//! After the server pushes factory state, the client sends back an HsFb
//! packet listing component IDs it wants progress updates for (or, with
//! `undo: true`, ones it wants to unsubscribe from). The server responds
//! with `ScFactoryHs` containing the current state of each requested
//! component as an `ScdFactoryHsFb` entry.
//!
//! The 9 feedback payload variants map to the 9 component types that
//! have ticking progress: Cache, Producer, Collector, BurnPower,
//! CacheTransport, GridBox, BoxRouterM1, BoxBridge, HealTower.

use crate::net::NetContext;
use perlica_logic::factory::FactoryComponent;
use perlica_proto::{
    CsFactoryHsFb, ScFactoryHs, ScdFactoryHsFb, ScdFactoryHsFbBoxBridge, ScdFactoryHsFbBoxRouterM1,
    ScdFactoryHsFbBurnPower, ScdFactoryHsFbCache, ScdFactoryHsFbCacheTransport,
    ScdFactoryHsFbCollector, ScdFactoryHsFbGridBox, ScdFactoryHsFbHealTower,
    ScdFactoryHsFbProducer, scd_factory_hs_fb::ComponentPayload,
};
use tracing::warn;

pub async fn on_cs_factory_hs_fb(ctx: &mut NetContext<'_>, req: CsFactoryHsFb) -> ScFactoryHs {
    let region_name = req.name.clone();
    let mut fb_list = Vec::with_capacity(req.node_id_list.len());

    let Some(region) = ctx.player.factory.region(&region_name) else {
            warn!(region = %region_name, "CsFactoryHsFb for unknown region");
            return ScFactoryHs {
                tms: 0,
                ct_list: vec![],
                fb_list: vec![],
                ce_list: vec![],
                blackboard: None,
            };
    };

    // Walk every node_id the client asked about, find each component
    // on that node, and serialize it as an HsFb entry.
    for &node_id in &req.node_id_list {
        let Some(node) = region.node(node_id) else {
            continue;
        };

        for (component_id, comp) in &node.components {
            let Some(fb) = build_hs_fb(*component_id, comp) else {
                continue;
            };
            fb_list.push(fb);
        }
    }

    // TODO: track subscriptions for incremental updates. Right now we
    // just return a one-shot snapshot; the live server maintains a
    // per-player set of subscribed component IDs and pushes deltas
    // on every tick. That needs a subscription registry on Player.

    ScFactoryHs {
        tms: 0,
        ct_list: vec![],
        fb_list,
        ce_list: vec![],
        blackboard: None,
    }
}

/// Build a single `ScdFactoryHsFb` from a component's current state.
/// Returns `None` for component types that don't have an HsFb variant
/// (Transform, Bus, Hub, Selector, etc. -- they have no ticking progress).
fn build_hs_fb(component_id: u32, comp: &FactoryComponent) -> Option<ScdFactoryHsFb> {
    use perlica_logic::factory::tick::elapsed_since;

    let payload = match comp {
        FactoryComponent::Cache(state) => {
            // Cache feedback is just the list of item IDs currently held.
            Some(ComponentPayload::Cache(ScdFactoryHsFbCache {
                items: state.items.iter().map(|s| s.inst_id).collect(),
            }))
        }

        FactoryComponent::Producer(state) => {
            // Producer feedback carries the formula + current progress +
            // the progress rate (speed) so the client can animate the bar.
            let progress = state.start_tick.map_or(state.current_progress, |start| {
                state.current_progress.saturating_add(elapsed_since(start) * 100)
            });
            Some(ComponentPayload::Producer(ScdFactoryHsFbProducer {
                progress_incr_per_ms: 100,
                formula_id: state.formula_id.clone(),
                current_progress: progress as i64,
            }))
        }

        FactoryComponent::Collector(state) => {
            let progress = state.start_tick.map_or(state.current_progress, |start| {
                state.current_progress.saturating_add(elapsed_since(start) * 250)
            });
            Some(ComponentPayload::Collector(ScdFactoryHsFbCollector {
                progress_incr_per_ms: 250,
                current_progress: progress as i64,
            }))
        }

        FactoryComponent::BurnPower(state) => {
            // BurnPower feedback reports remaining fuel + depletion rate.
            let remaining = state.fuel_remaining;
            Some(ComponentPayload::BurnPower(ScdFactoryHsFbBurnPower {
                progress_decr_per_ms: 125,
                current_least_progress: remaining,
            }))
        }

        FactoryComponent::CacheTransport(state) => {
            Some(ComponentPayload::CacheTransport(ScdFactoryHsFbCacheTransport {
                progress_incr_per_ms: 0,
                current_progress: state.current_progress,
            }))
        }

        FactoryComponent::GridBox(state) => {
            Some(ComponentPayload::GridBox(ScdFactoryHsFbGridBox {
                items: state.items.iter().map(|s| s.inst_id).collect(),
            }))
        }

        FactoryComponent::BoxRouterM1 => {
            Some(ComponentPayload::BoxRouterM1(ScdFactoryHsFbBoxRouterM1 {
                items: vec![],
            }))
        }

        FactoryComponent::BoxBridge => {
            Some(ComponentPayload::BoxBridge(ScdFactoryHsFbBoxBridge {
                items: vec![],
            }))
        }

        FactoryComponent::HealTower(state) => {
            Some(ComponentPayload::HealTower(ScdFactoryHsFbHealTower {
                progress_incr_per_ms: 0,
                current_progress: state.current_progress,
                current_point: state.points as i32,
            }))
        }

        // Components without an HsFb variant.
        _ => None,
    };

    payload.map(|p| ScdFactoryHsFb {
        component_id,
        component_payload: Some(p),
    })
}
