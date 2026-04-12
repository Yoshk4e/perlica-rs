use crate::net::NetContext;
use tracing::{debug, error};

/// Pushes `ScSyncCharBagInfo`, full character/team/skill snapshot. Returns `false` on send failure.
pub async fn push_char_bag(ctx: &mut NetContext<'_>) -> bool {
    match ctx.player.char_bag.char_bag_info(ctx.assets) {
        Ok(msg) => {
            debug!(
                "Pushing character bag: uid={}, chars={}, teams={}",
                ctx.player.uid,
                msg.char_info.len(),
                msg.team_info.len()
            );
            ctx.notify(msg).await.is_ok()
        }
        Err(error) => {
            error!(
                "Failed to build character bag info: uid={}, error={:?}",
                ctx.player.uid, error
            );
            false
        }
    }
}

/// Pushes `ScItemBagSync`. Call after login and after any add/remove operation.
pub async fn push_item_bag_sync(ctx: &mut NetContext<'_>) -> bool {
    let msg = ctx.player.char_bag.item_bag_sync(ctx.assets);
    debug!("Pushing item bag sync: uid={}", ctx.player.uid);
    ctx.notify(msg).await.is_ok()
}

/// Pushes `ScSyncAttr` for every character (full derived stats). Sent on login and after level/break changes.
pub async fn push_char_attrs(ctx: &mut NetContext<'_>) -> bool {
    let msgs = ctx.player.char_bag.char_attrs(ctx.assets);
    debug!(
        "Pushing character attributes: uid={}, count={}",
        ctx.player.uid,
        msgs.len()
    );

    for msg in msgs {
        if let Err(error) = ctx.notify(msg).await {
            error!(
                "Failed to push character attributes: uid={}, error={:?}",
                ctx.player.uid, error
            );
            return false;
        }
    }

    true
}

/// Pushes `ScCharSyncStatus` for every character (HP, SP, is_dead). Sent on login and after combat-state changes.
pub async fn push_char_status(ctx: &mut NetContext<'_>) -> bool {
    let msgs = ctx.player.char_bag.char_status();
    debug!(
        "Pushing character status: uid={}, count={}",
        ctx.player.uid,
        msgs.len()
    );

    for msg in msgs {
        if let Err(error) = ctx.notify(msg).await {
            error!(
                "Failed to push character status: uid={}, error={:?}",
                ctx.player.uid, error
            );
            return false;
        }
    }

    true
}

/// Pushes `ScCharSyncStatus` for a specific set of characters. Unknown IDs are silently skipped.
pub async fn push_char_status_for_ids(ctx: &mut NetContext<'_>, obj_ids: &[u64]) -> bool {
    use perlica_proto::{BattleInfo, ScCharSyncStatus};

    let updates: Vec<ScCharSyncStatus> = obj_ids
        .iter()
        .filter_map(|&id| {
            ctx.player
                .char_bag
                .get_char_by_objid(id)
                .map(|c| ScCharSyncStatus {
                    objid: id,
                    is_dead: c.is_dead,
                    battle_info: Some(BattleInfo {
                        hp: c.hp,
                        ultimatesp: c.ultimate_sp,
                    }),
                })
        })
        .collect();

    debug!(
        "Pushing character status for IDs: uid={}, count={}",
        ctx.player.uid,
        updates.len()
    );

    for msg in updates {
        if let Err(error) = ctx.notify(msg).await {
            error!(
                "Failed to push character status: uid={}, error={:?}",
                ctx.player.uid, error
            );
            return false;
        }
    }

    true
}
