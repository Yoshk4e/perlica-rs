use crate::net::NetContext;
use perlica_proto::{CsFactoryHsInout, ScFactoryHs};

pub async fn on_cs_factory_hs_inout(
    ctx: &mut NetContext<'_>,
    req: CsFactoryHsInout,
) -> ScFactoryHs {
    ctx.player.factory.hs_inout(&req.name, req.in_out);

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
        fb_list: vec![],
        ce_list: vec![],
        blackboard,
    }
}
