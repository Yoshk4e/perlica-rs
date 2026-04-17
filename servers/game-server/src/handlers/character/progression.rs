//! Character progression: experience gain (`on_cs_char_level_up`) and
//! break / ascension (`on_cs_char_break`).

use crate::net::NetContext;
use perlica_proto::{
    CsCharBreak, CsCharLevelUp, ScCharBreak, ScCharLevelUp, ScCharSyncLevelExp,
    ScItemBagSyncModify, ScdItemDepotModify,
};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

/// Cumulative exp required to reach `target_level` from level 1.
fn cumulative_exp(level_up_exp: &[i32], target_level: i32) -> i64 {
    let mut total = 0i64;
    for i in 0..(target_level - 1) as usize {
        let cost = level_up_exp.get(i).copied().unwrap_or(0);
        if cost < 0 {
            break;
        }
        total += cost as i64;
    }
    total
}

/// Advances `current_level` as far as possible given `new_total_exp`, capped at `max_level`.
/// Returns `(achieved_level, remaining_exp_within_that_level)`.
fn calculate_level_from_total_exp(
    level_up_exp: &[i32],
    current_level: i32,
    new_total_exp: i64,
    max_level: i32,
) -> (i32, i32) {
    let mut lv = current_level;
    loop {
        if lv >= max_level {
            break;
        }
        let cost = level_up_exp.get(lv as usize - 1).copied().unwrap_or(-1);
        if cost < 0 {
            break;
        }
        let cum_next = cumulative_exp(level_up_exp, lv + 1);
        if new_total_exp >= cum_next {
            lv += 1;
        } else {
            break;
        }
    }
    let cum_at_lv = cumulative_exp(level_up_exp, lv);
    let remaining = (new_total_exp - cum_at_lv).max(0) as i32;
    (lv, remaining)
}

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

    // Exp the character still needs to advance from their current level.
    let cum_at_current = cumulative_exp(level_up_exp, current_level);

    let mut total_exp_gained: i64 = 0;
    let mut consumed: HashMap<String, u32> = HashMap::new();

    use config::item::ItemDepotType;

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

        let consumed_ok = ctx
            .player
            .char_bag
            .item_manager
            .consume_stackable(ItemDepotType::SpecialItem, &item_info.res_id, count)
            .is_ok()
            || ctx
                .player
                .char_bag
                .item_manager
                .consume_stackable(ItemDepotType::Factory, &item_info.res_id, count)
                .is_ok();

        if consumed_ok {
            total_exp_gained += exp_per_unit * count as i64;
            *consumed.entry(item_info.res_id.clone()).or_insert(0) += count;
        } else {
            warn!(
                "CharLevelUp: could not consume {} × {}",
                count, item_info.res_id
            );
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
        "CharLevelUp complete: uid={}, char_id={}, level {}→{}, exp_gained={}, remaining={}",
        ctx.player.uid, req.char_obj_id, current_level, new_level, total_exp_gained, synced_exp
    );

    if let Some(attr_msg) = ctx
        .player
        .char_bag
        .char_attrs(ctx.assets)
        .into_iter()
        .find(|a| a.obj_id == req.char_obj_id)
    {
        let _ = ctx
            .notify(attr_msg)
            .await
            .inspect_err(|e| error!("Failed to sync attrs after level up: {:?}", e));
    }

    if let Err(e) = ctx
        .notify(ScCharSyncLevelExp {
            char_obj_id: req.char_obj_id,
            level: new_level,
            exp: synced_exp,
        })
        .await
    {
        error!("Failed to sync level/exp: {:?}", e);
    }

    if !consumed.is_empty() {
        let items: HashMap<String, i64> = consumed
            .keys()
            .map(|id| {
                let in_special = ctx
                    .player
                    .char_bag
                    .item_manager
                    .count_of(ItemDepotType::SpecialItem, id);
                let new_count = if ctx.player.char_bag.item_manager.has_stackable(
                    ItemDepotType::SpecialItem,
                    id,
                    0,
                ) {
                    in_special
                } else {
                    ctx.player
                        .char_bag
                        .item_manager
                        .count_of(ItemDepotType::Factory, id)
                };
                (id.clone(), new_count as i64)
            })
            .collect();

        let mut depot_modify = HashMap::new();
        depot_modify.insert(
            4i32,
            ScdItemDepotModify {
                items,
                inst_list: vec![],
                del_inst_list: vec![],
            },
        );

        if let Err(e) = ctx
            .notify(ScItemBagSyncModify {
                depot: depot_modify,
                bag: None,
                factory_depot: None,
                cannot_destroy: HashMap::new(),
                use_blackboard: None,
                is_new: false,
            })
            .await
        {
            error!("Failed to send item bag modify: {:?}", e);
        }
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
            "CharBreak complete: uid={}, char_id={}, stage {} → {}",
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
    ScCharBreak {
        char_obj_id: req.char_obj_id,
        stage: confirmed,
    }
}
