//! `CsFactoryCharacterWork*` handlers -- thin wrappers around the logic
//! in `perlica_logic::factory::character_work`.

use crate::net::NetContext;
use perlica_proto::{
    CsFactoryCharacterWorkPunchIn, CsFactoryCharacterWorkPunchOut, ScFactoryModifyCharacterWork,
};

pub async fn on_cs_factory_character_work_punch_in(
    ctx: &mut NetContext<'_>,
    req: CsFactoryCharacterWorkPunchIn,
) -> ScFactoryModifyCharacterWork {
    ctx.player.factory.character_work_punch_in(
        &ctx.assets.factory_table,
        &req.region,
        req.node_id,
        &req.char_id_sequence,
    );

    ScFactoryModifyCharacterWork {
        characters: vec![],
        punch_out_list: vec![],
    }
}

pub async fn on_cs_factory_character_work_punch_out(
    ctx: &mut NetContext<'_>,
    req: CsFactoryCharacterWorkPunchOut,
) -> ScFactoryModifyCharacterWork {
    let punched = ctx
        .player
        .factory
        .character_work_punch_out(&req.char_id_list);

    ScFactoryModifyCharacterWork {
        characters: vec![],
        punch_out_list: punched,
    }
}
