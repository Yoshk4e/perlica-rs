use crate::net::NetContext;
use perlica_logic::factory::ops::HsFbPayload;
use perlica_proto::{
    CsFactoryHsFb, ScFactoryHs, ScdFactoryHsFb, ScdFactoryHsFbBoxBridge, ScdFactoryHsFbBoxRouterM1,
    ScdFactoryHsFbBurnPower, ScdFactoryHsFbCache, ScdFactoryHsFbCacheTransport,
    ScdFactoryHsFbCollector, ScdFactoryHsFbGridBox, ScdFactoryHsFbHealTower,
    ScdFactoryHsFbProducer, scd_factory_hs_fb::ComponentPayload,
};

pub async fn on_cs_factory_hs_fb(ctx: &mut NetContext<'_>, req: CsFactoryHsFb) -> ScFactoryHs {
    let entries = ctx
        .player
        .factory
        .build_hs_fb_list(&req.name, &req.node_id_list);
    let fb_list = entries.into_iter().map(to_proto).collect();

    // Serialize the blackboard from the region so the client sees real
    // power state, not empty.
    let blackboard =
        ctx.player
            .factory
            .region(&req.name)
            .map(|region| perlica_proto::ScdFactoryHsBb {
                power: Some(perlica_proto::ScdFactoryHsBbPower {
                    is_stop_by_power: region.blackboard.is_stop_by_power,
                    power_cost_sum: region.blackboard.power_cost,
                    power_save_max: region.blackboard.power_save_max,
                    power_save_current: region.blackboard.power_save_current,
                    power_gen_last_sec: region.blackboard.power_gen,
                }),
            });

    ScFactoryHs {
        tms: perlica_logic::factory::current_tick() as i64,
        ct_list: vec![],
        fb_list,
        ce_list: vec![],
        blackboard,
    }
}

fn to_proto(entry: perlica_logic::factory::ops::HsFbEntry) -> ScdFactoryHsFb {
    let payload = match entry.payload {
        HsFbPayload::Cache { items } => ComponentPayload::Cache(ScdFactoryHsFbCache { items }),
        HsFbPayload::Producer {
            progress_incr_per_ms,
            formula_id,
            current_progress,
        } => ComponentPayload::Producer(ScdFactoryHsFbProducer {
            progress_incr_per_ms,
            formula_id,
            current_progress,
        }),
        HsFbPayload::Collector {
            progress_incr_per_ms,
            current_progress,
        } => ComponentPayload::Collector(ScdFactoryHsFbCollector {
            progress_incr_per_ms,
            current_progress,
        }),
        HsFbPayload::BurnPower {
            progress_decr_per_ms,
            current_least_progress,
        } => ComponentPayload::BurnPower(ScdFactoryHsFbBurnPower {
            progress_decr_per_ms,
            current_least_progress,
        }),
        HsFbPayload::CacheTransport {
            progress_incr_per_ms,
            current_progress,
        } => ComponentPayload::CacheTransport(ScdFactoryHsFbCacheTransport {
            progress_incr_per_ms,
            current_progress,
        }),
        HsFbPayload::GridBox { items } => {
            ComponentPayload::GridBox(ScdFactoryHsFbGridBox { items })
        }
        HsFbPayload::BoxRouterM1 { items } => {
            ComponentPayload::BoxRouterM1(ScdFactoryHsFbBoxRouterM1 { items })
        }
        HsFbPayload::BoxBridge { items } => {
            ComponentPayload::BoxBridge(ScdFactoryHsFbBoxBridge { items })
        }
        HsFbPayload::HealTower {
            progress_incr_per_ms,
            current_progress,
            current_point,
        } => ComponentPayload::HealTower(ScdFactoryHsFbHealTower {
            progress_incr_per_ms,
            current_progress,
            current_point,
        }),
    };
    ScdFactoryHsFb {
        component_id: entry.component_id,
        component_payload: Some(payload),
    }
}
