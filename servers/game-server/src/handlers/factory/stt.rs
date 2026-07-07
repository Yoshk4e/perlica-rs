use crate::net::NetContext;
use perlica_proto::{CsFactorySttUnlockNode, ScFactoryModifyStt};
use tracing::warn;

pub async fn on_cs_factory_stt_unlock_node(
    ctx: &mut NetContext<'_>,
    req: CsFactorySttUnlockNode,
) -> ScFactoryModifyStt {
    let region_name = ctx.player.factory.current_region.clone();
    let success = ctx.player.factory.stt_unlock_node(
        &ctx.assets.factory_sttree,
        &ctx.assets.factory_table,
        &region_name,
        &req.node_id,
    );

    if !success {
        warn!(node_id = %req.node_id, "STT unlock failed");
    }

    // The proto ScFactoryModifyStt has a `nodes` field (Vec<ScdFactorySttNode>)
    // but the ScdFactorySttNode struct is complex and needs the full STT
    // node config to serialize properly. The client will refresh on next
    // sync. The unlock state is tracked server-side in stt_state.unlocked_nodes.
    ScFactoryModifyStt { nodes: vec![] }
}
