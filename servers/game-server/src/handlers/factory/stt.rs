use crate::net::NetContext;
use perlica_proto::{CsFactorySttUnlockNode, ScFactoryModifyStt, ScdFactorySttNode};
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
        return ScFactoryModifyStt { nodes: vec![] };
    }

    // Send the updated node back. The client's
    // `FacTechTreeSystem::_HandleFactorySync` (SC_FACTORY_MODIFY_STT)
    // applies `state` + `values`/`flags` per node and fires the unlock
    // UI event when the state changes, so the tree must reflect the new
    // state or the player never sees the unlock.
    let Some(data) =
        ctx.player
            .factory
            .stt_node_sync(&ctx.assets.factory_sttree, &region_name, &req.node_id)
    else {
        warn!(node_id = %req.node_id, "STT unlock succeeded but node not found for sync");
        return ScFactoryModifyStt { nodes: vec![] };
    };

    ScFactoryModifyStt {
        nodes: vec![ScdFactorySttNode {
            id: req.node_id,
            state: data.state,
            values: data.values,
            flags: data.flags,
        }],
    }
}
