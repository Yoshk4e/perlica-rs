//! `CsFactoryCharacterWork*` handlers -- thin wrappers around the logic
//! in `perlica_logic::factory::character_work`.

use crate::net::NetContext;
use perlica_logic::factory::current_tick;
use perlica_proto::{
    CsFactoryCharacterWorkPunchIn, CsFactoryCharacterWorkPunchOut, ScFactoryModifyCharacterWork,
    ScdFactoryCharacterWorkCharacter,
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

    let characters = serialize_workers(&ctx.player.factory, &req.region, req.node_id);

    ScFactoryModifyCharacterWork {
        characters,
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

    // Serialize remaining workers (all regions since punch_out doesn't carry a region).
    let characters: Vec<_> = ctx
        .player
        .factory
        .character_work_state
        .workers
        .iter()
        .map(|w| ScdFactoryCharacterWorkCharacter {
            char_id: w.char_inst_id.to_string(),
            region: w.region_name.clone(),
            node_id: 0,
            punch_in_ts: current_tick() as i64,
            index_in_node: w.work_slot as i32,
            active_skills: w.skill_ids.clone(),
        })
        .collect();

    ScFactoryModifyCharacterWork {
        characters,
        punch_out_list: punched,
    }
}

fn serialize_workers(
    manager: &perlica_logic::factory::FactoryManager,
    region_name: &str,
    node_id: u32,
) -> Vec<ScdFactoryCharacterWorkCharacter> {
    manager
        .character_work_state
        .workers
        .iter()
        .filter(|w| w.region_name == region_name)
        .map(|w| ScdFactoryCharacterWorkCharacter {
            char_id: w.char_inst_id.to_string(),
            region: w.region_name.clone(),
            node_id,
            punch_in_ts: current_tick() as i64,
            index_in_node: w.work_slot as i32,
            active_skills: w.skill_ids.clone(),
        })
        .collect()
}
