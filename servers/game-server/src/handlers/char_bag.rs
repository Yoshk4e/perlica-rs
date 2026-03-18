use crate::net::NetContext;
use tracing::{debug, error};

/// Pushes the full character bag to the client as `ScSyncCharBagInfo`.
///
/// Includes all character metadata, team compositions, skill info, and the
/// current team index. Called during the login sequence and whenever the bag
/// state needs a full resync.
///
/// Returns `false` if serialisation fails or the send channel is closed.
pub async fn push_char_bag(ctx: &mut NetContext<'_>) -> bool {
    match ctx.player.char_bag.char_bag_info(ctx.assets) {
        Ok(msg) => {
            debug!(
                "push char bag: uid={}, chars={}, teams={}",
                ctx.player.uid,
                msg.char_info.len(),
                msg.team_info.len()
            );
            ctx.notify(msg).await.is_ok()
        }
        Err(e) => {
            error!("char bag info failed: uid={}, error={}", ctx.player.uid, e);
            false
        }
    }
}

/// Pushes the item bag (weapons and gems) as `ScItemBagSync`.
///
/// Called once during the login sequence so the client can display the player's
/// weapon inventory. Also call this after any operation that adds or removes
/// items (e.g. weapon foddering).
pub async fn push_item_bag_sync(ctx: &mut NetContext<'_>) -> bool {
    let msg = ctx.player.char_bag.item_bag_sync();
    debug!("push item bag sync: uid={}", ctx.player.uid);
    ctx.notify(msg).await.is_ok()
}

/// Pushes `ScSyncAttr` for every character in the bag.
///
/// Each message contains the full derived stat list (HP, ATK, DEF, etc.) for
/// one character at its current level and break stage. Sent during login and
/// after level-up or breakthrough operations.
pub async fn push_char_attrs(ctx: &mut NetContext<'_>) -> bool {
    let msgs = ctx.player.char_bag.char_attrs(ctx.assets);
    debug!(
        "push char attrs: uid={}, count={}",
        ctx.player.uid,
        msgs.len()
    );
    for msg in msgs {
        if let Err(e) = ctx.notify(msg).await {
            error!(
                "char attrs push failed: uid={}, error={}",
                ctx.player.uid, e
            );
            return false;
        }
    }
    true
}

/// Pushes `ScCharSyncStatus` for every character in the bag.
///
/// Each message contains the character's current HP, ultimate SP, and `is_dead`
/// flag. Sent during login and after any event that changes combat state (death,
/// revival, team switch).
pub async fn push_char_status(ctx: &mut NetContext<'_>) -> bool {
    let msgs = ctx.player.char_bag.char_status();
    debug!(
        "push char status: uid={}, count={}",
        ctx.player.uid,
        msgs.len()
    );
    for msg in msgs {
        if let Err(e) = ctx.notify(msg).await {
            error!(
                "char status push failed: uid={}, error={}",
                ctx.player.uid, e
            );
            return false;
        }
    }
    true
}

/// Pushes [`ScCharSyncStatus`] for a specific set of character object IDs.
///
/// Used after a team composition or active-team change to immediately sync each
/// member's HP and `is_dead` flag. Characters whose object ID is not found in
/// the bag are silently skipped.
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
        "push char status for ids: uid={}, count={}",
        ctx.player.uid,
        updates.len()
    );

    for msg in updates {
        if let Err(e) = ctx.notify(msg).await {
            error!(
                "char status push failed: uid={}, error={}",
                ctx.player.uid, e
            );
            return false;
        }
    }
    true
}
