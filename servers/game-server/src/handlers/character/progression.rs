//! Character progression: experience gain (`on_cs_char_level_up`) and
//! break / ascension (`on_cs_char_break`).

use crate::net::NetContext;
use perlica_logic::character::progression::{calculate_level_from_total_exp, cumulative_exp};
use perlica_logic::item::ConsumedItems;
use perlica_proto::{
    CsCharBreak, CsCharLevelUp, ScCharBreak, ScCharLevelUp, ScCharSyncLevelExp, ScItemBagSyncModify,
};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Consumes exp items and advances the character's level.
pub async fn on_cs_char_level_up(ctx: &mut NetContext<'_>, req: CsCharLevelUp) -> ScCharLevelUp {
    debug!(
        "CharLevelUp: uid={}, char_id={}, items={}",
        ctx.player.uid,
        req.char_obj_id,
        req.items.len()
    );

    let Some(char_data) = ctx.player.char_bag.get_char_by_objid_mut(req.char_obj_id) else {
        warn!("CharLevelUp failed: unknown char_id={}", req.char_obj_id);
        return ScCharLevelUp {
            char_obj_id: req.char_obj_id,
        };
    };

    let template_id = char_data.template_id.clone();
    let break_stage = char_data.break_stage;
    let current_level = char_data.level;
    let current_exp = char_data.exp as i64;

    let max_level = ctx
        .assets
        .characters
        .get(&template_id)
        .and_then(|c| {
            c.break_data
                .iter()
                .find(|bd| bd.break_stage == break_stage)
                .map(|bd| bd.max_level as i32)
        })
        .unwrap_or(current_level);

    if current_level >= max_level {
        return ScCharLevelUp {
            char_obj_id: req.char_obj_id,
        };
    }

    let level_up_exp = ctx.assets.characters.char_const().level_up_exp.as_slice();

    let cum_at_current = cumulative_exp(level_up_exp, current_level);

    let mut total_exp_gained: i64 = 0;
    let mut consumed_items = ConsumedItems::new();

    for item_info in &req.items {
        if item_info.res_count <= 0 {
            continue;
        }
        let count = item_info.res_count as u32;

        let exp_per_unit = ctx.assets.items.char_exp_for_item(&item_info.res_id);

        if exp_per_unit == 0 {
            warn!(
                "CharLevelUp: item {} gives 0 exp, skipping",
                item_info.res_id
            );
            continue;
        }

        match ctx
            .player
            .char_bag
            .item_manager
            .consume_stackable_auto(&item_info.res_id, count)
        {
            Ok((depot_type, remaining)) => {
                total_exp_gained += exp_per_unit * count as i64;
                consumed_items.record(depot_type, item_info.res_id.clone(), remaining);
            }
            Err(e) => {
                warn!(
                    "CharLevelUp: could not consume {} * {}: {:?}",
                    count, item_info.res_id, e
                );
            }
        }
    }

    if total_exp_gained == 0 {
        return ScCharLevelUp {
            char_obj_id: req.char_obj_id,
        };
    }

    let new_total_exp = cum_at_current + current_exp + total_exp_gained;
    let (new_level, remaining_exp) =
        calculate_level_from_total_exp(level_up_exp, current_level, new_total_exp, max_level);

    let at_max = new_level >= max_level;
    let synced_exp = if at_max { 0 } else { remaining_exp };

    let char_data = ctx
        .player
        .char_bag
        .get_char_by_objid_mut(req.char_obj_id)
        .unwrap();
    char_data.level = new_level;
    char_data.exp = synced_exp;

    if let Some(attrs) = ctx
        .assets
        .characters
        .get_stats(&template_id, new_level, break_stage)
    {
        char_data.hp = attrs.hp;
    }

    info!(
        "CharLevelUp complete: uid={}, char_id={}, level {}->{}, exp_gained={}, remaining={}",
        ctx.player.uid, req.char_obj_id, current_level, new_level, total_exp_gained, synced_exp
    );

    if let Some(attr_msg) = ctx
        .player
        .char_bag
        .char_attrs(ctx.assets)
        .into_iter()
        .find(|a| a.obj_id == req.char_obj_id)
    {
        let _ = ctx.notify(attr_msg).await;
    }

    let _ = ctx
        .notify(ScCharSyncLevelExp {
            char_obj_id: req.char_obj_id,
            level: new_level,
            exp: synced_exp,
        })
        .await;

    if !consumed_items.is_empty() {
        let depot_modify = consumed_items.build_depot_map();

        if !depot_modify.is_empty() {
            let _ = ctx
                .notify(ScItemBagSyncModify {
                    depot: depot_modify,
                    bag: None,
                    factory_depot: None,
                    cannot_destroy: HashMap::new(),
                    use_blackboard: None,
                    is_new: false,
                })
                .await;
        }
    }

    if let Err(e) = ctx
        .db
        .persist_char_bag_incremental(&ctx.player.uid, &mut ctx.player.char_bag)
        .await
    {
        warn!(
            "Failed to persist char_bag after level up: uid={}, error={}",
            ctx.player.uid, e
        );
    }

    ScCharLevelUp {
        char_obj_id: req.char_obj_id,
    }
}

/// Advances the character's break stage (ascension).
pub async fn on_cs_char_break(ctx: &mut NetContext<'_>, req: CsCharBreak) -> ScCharBreak {
    debug!(
        "CharBreak: uid={}, char_id={}, from_stage={}",
        ctx.player.uid, req.char_obj_id, req.stage
    );
    let Some(char_data) = ctx.player.char_bag.get_char_by_objid_mut(req.char_obj_id) else {
        warn!("CharBreak failed: unknown char_id={}", req.char_obj_id);
        return ScCharBreak {
            char_obj_id: req.char_obj_id,
            stage: 0,
        };
    };
    let template_id = char_data.template_id.clone();
    let from_stage = req.stage as u32;

    if from_stage == char_data.break_stage {
        let new_stage = char_data.break_stage + 1;
        char_data.break_stage = new_stage;
        if let Some(attrs) =
            ctx.assets
                .characters
                .get_stats(&template_id, char_data.level, new_stage)
        {
            char_data.hp = attrs.hp;
        }
        info!(
            "CharBreak complete: uid={}, char_id={}, stage {} -> {}",
            ctx.player.uid, req.char_obj_id, from_stage, new_stage
        );
    } else {
        warn!(
            "CharBreak rejected: current={}, requested from={}",
            char_data.break_stage, from_stage
        );
    }

    let confirmed = char_data.break_stage as i32;
    if let Some(attr_msg) = ctx
        .player
        .char_bag
        .char_attrs(ctx.assets)
        .into_iter()
        .find(|a| a.obj_id == req.char_obj_id)
    {
        let _ = ctx.notify(attr_msg).await;
    }
    if let Err(e) = ctx
        .db
        .persist_char_bag_incremental(&ctx.player.uid, &mut ctx.player.char_bag)
        .await
    {
        warn!(
            "Failed to persist char_bag after break: uid={}, error={}",
            ctx.player.uid, e
        );
    }

    ScCharBreak {
        char_obj_id: req.char_obj_id,
        stage: confirmed,
    }
}
