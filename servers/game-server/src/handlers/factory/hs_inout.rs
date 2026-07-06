use crate::net::NetContext;
use perlica_proto::{CsFactoryHsInout, ScFactoryHs};

pub async fn on_cs_factory_hs_inout(
    ctx: &mut NetContext<'_>,
    req: CsFactoryHsInout,
) -> ScFactoryHs {
    ctx.player.factory.hs_inout(&req.name, req.in_out);
    ScFactoryHs {
        tms: 0,
        ct_list: vec![],
        fb_list: vec![],
        ce_list: vec![],
        blackboard: None,
    }
}
