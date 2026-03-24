use crate::net::NetContext;
use perlica_proto::{
    ScFactorySyncContext, ScdFactorySyncBlackboard, ScdFactorySyncBlackboardPower,
    ScdFactorySyncRegion, ScdFactorySyncScene,
};
use tracing::{debug, error};

// Thanks xeondev for pointing this out
pub async fn push_factory(ctx: &mut NetContext<'_>) -> bool {
    let msg = ScFactorySyncContext {
        tms: 0,
        current_region: "test01".to_string(),
        regions: vec![ScdFactorySyncRegion {
            name: "test01".to_string(),
            blackboard: Some(ScdFactorySyncBlackboard {
                inventory_node_id: 0,
                power: Some(ScdFactorySyncBlackboardPower {
                    power_cost: 0,
                    power_gen: 0,
                    power_save_max: 0,
                    power_save_current: 0,
                    is_stop_by_power: false,
                }),
            }),
            nodes: vec![],
            scenes: vec![ScdFactorySyncScene {
                name: ctx.player.world.last_scene.clone(),
                level: 0,
                main_mesh: vec![],
                connections: vec![],
                bandwidth: None,
            }],
        }],
        quickbars: vec![],
    };

    debug!(
        "Pushing factory context: uid={}, regions={}",
        ctx.player.uid,
        msg.regions.len()
    );

    if let Err(error) = ctx.notify(msg).await {
        error!(
            "Failed to push factory context: uid={}, error={:?}",
            ctx.player.uid,
            error
        );
        return false;
    }

    true
}
