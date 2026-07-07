use crate::net::NetContext;
use perlica_proto::{CsFactorySttUnlockNode, ScFactoryModifyStt};

pub async fn on_cs_factory_stt_unlock_node(
    ctx: &mut NetContext<'_>,
    req: CsFactorySttUnlockNode,
) -> ScFactoryModifyStt {
    let region_name = ctx.player.factory.current_region.clone();
    ctx.player.factory.stt_unlock_node(
        &ctx.assets.factory_sttree,
        &ctx.assets.factory_table,
        &region_name,
        &req.node_id,
    );

    ScFactoryModifyStt { nodes: vec![] }
}
