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

    ScFactoryHs {
        tms: 0,
        ct_list: vec![],
        fb_list,
        ce_list: vec![],
        blackboard: None,
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
