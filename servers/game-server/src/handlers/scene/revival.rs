//! Character/monster kills, revival, and campfire checkpoints.

use crate::net::NetContext;
use perlica_db::{Persistable, SceneSaveState};
use perlica_logic::character::char_bag::CharIndex;
use perlica_logic::scene::EntityDestroyReason;
use perlica_logic::traits::Classified;
use perlica_proto::{
    BattleInfo, Code, CsSceneKillChar, CsSceneKillMonster, CsSceneRevival,
    CsSceneSetLastRecordCampid, ScCharSyncStatus, ScObjectEnterView, ScSceneSetLastRecordCampid,
};
use tracing::{debug, error, info, warn};

const CAMPFIRE_POS_MAX_DELTA: f32 = 5.0;

/// Removes a monster entity and notifies the client with `ScSceneDestroyEntity`.
pub async fn on_cs_scene_kill_monster(ctx: &mut NetContext<'_>, req: CsSceneKillMonster) {
    debug!("Monster killed: {}", req.id);

    // If the entity exists but isn't an enemy (e.g. it's an interactive or NPC),
    // silently ignore the request to prevent abuse.
    if let Some(entity) = ctx.player.entities.get(req.id).and_then(|entity| {
        if !entity.is_enemy() {
            Some(entity)
        } else {
            None
        }
    }) {
        warn!(
            "Rejected monster kill: id={} is not an enemy (kind={:?})",
            req.id, entity.kind
        );
        return;
    }

    if let Some(entity) = ctx.player.entities.remove(req.id).filter(|e| e.is_enemy()) {
        ctx.player.scene.on_entity_killed(entity.level_logic_id);
    }

    let msg = ctx
        .player
        .scene
        .destroy_entity(req.id, EntityDestroyReason::Dead);

    if let Err(error) = ctx.notify(msg).await {
        error!("Failed to send monster kill notification: {:?}", error);
    }
}

pub async fn on_cs_scene_kill_char(ctx: &mut NetContext<'_>, req: CsSceneKillChar) {
    debug!("Character killed: {}", req.id);

    // Check if the character is on the active team
    let team_idx = ctx.player.char_bag.meta.curr_team_index as usize;
    let in_active_team = ctx
        .player
        .char_bag
        .teams
        .get(team_idx)
        .map(|team| {
            team.char_team
                .iter()
                .any(|slot| slot.object_id() == Some(req.id))
        })
        .unwrap_or(false);

    if !in_active_team {
        warn!(
            "Rejected character kill: id={} not in active team (team_idx={})",
            req.id, team_idx
        );
        ctx.send_error(
            Code::ErrSceneCharNil,
            format!("id {} is not in the current active team", req.id),
        )
        .await;
        return;
    }

    if let Some(char_data) = ctx.player.char_bag.get_char_by_objid_mut(req.id) {
        char_data.is_dead = true;
    }

    let msg = ctx
        .player
        .scene
        .destroy_entity(req.id, EntityDestroyReason::Dead);

    if let Err(error) = ctx.notify(msg).await {
        error!("Failed to send character kill notification: {:?}", error);
    }
    // no `.persist()` here - the dirty flag is enough.
}

/// Handles `CsSceneRevival`, revives all dead characters in the current team
/// at 50 % HP.
/// ORDER MATTERS!!
/// Send order:
/// 1. `ScCharSyncStatus` × N, HP per revived char
/// 2. `ScSelfSceneInfo` with `revive_chars`, triggers client revival logic
/// 3. `ScSceneRevival` - revival UI/effect
/// 4. `ScObjectEnterView` - reply, re-enters chars into the scene
pub async fn on_cs_scene_revival(
    ctx: &mut NetContext<'_>,
    _req: CsSceneRevival,
) -> ScObjectEnterView {
    info!("Scene revival requested");

    let (enter_view, self_info, revival) = ctx.player.scene.handle_revival(
        &mut ctx.player.char_bag,
        &ctx.player.movement,
        ctx.assets,
        &mut ctx.player.entities,
        None,
    );

    send_revival_status_updates(ctx).await;

    if let Err(error) = ctx.notify(self_info).await {
        error!("Failed to send revival self info: {:?}", error);
    }

    if let Err(error) = ctx.notify(revival).await {
        error!("Failed to send revival notification: {:?}", error);
    }

    if let Err(e) = ctx
        .db
        .persist_char_bag_incremental(&ctx.player.uid, &mut ctx.player.char_bag)
        .await
    {
        warn!(
            "Failed to persist char_bag after revival: uid={}, error={}",
            ctx.player.uid, e
        );
    }

    enter_view
}

async fn send_revival_status_updates(ctx: &mut NetContext<'_>) {
    let team_idx = ctx.player.char_bag.meta.curr_team_index as usize;
    let team = &ctx.player.char_bag.teams[team_idx];

    let updates: Vec<(u64, f64, f32)> = ctx
        .player
        .char_bag
        .chars
        .iter()
        .enumerate()
        .filter(|(i, c)| {
            !c.is_dead
                && team.char_team.iter().any(|slot| {
                    slot.char_index()
                        .map(|idx| idx.as_usize() == *i)
                        .unwrap_or(false)
                })
        })
        .map(|(i, c)| (CharIndex::from_usize(i).object_id(), c.hp, c.ultimate_sp))
        .collect();

    for (objid, hp, ultimatesp) in updates {
        if let Err(error) = ctx
            .notify(ScCharSyncStatus {
                objid,
                is_dead: false,
                battle_info: Some(BattleInfo { hp, ultimatesp }),
            })
            .await
        {
            error!(
                "Failed to send revival status update for {}: {:?}",
                objid, error
            );
        }
    }
}

/// Stores the campfire as the current checkpoint so revival/repatriation return here.
///
/// The client sends `req.position` which could be spoofed. We
/// cross-check against the server-known position of the campfire
/// entity in `ctx.player.entities`. If the entity exists and the
/// client position is too far from the server position, the server
/// position overrides.
///
/// Send order:
///   1. `ScSceneSetLastRecordCampid` - ACK echoing the camp id back.
pub async fn on_cs_scene_set_last_record_campid(
    ctx: &mut NetContext<'_>,
    req: CsSceneSetLastRecordCampid,
) -> ScSceneSetLastRecordCampid {
    info!("Setting last campfire: camp_id={}", req.last_camp_id);

    if let Some(client_pos) = &req.position {
        let (pos_x, pos_y, pos_z) = if let Some(entity) =
            ctx.player.entities.get(req.last_camp_id as u64)
        {
            let dx = client_pos.x - entity.pos_x;
            let dy = client_pos.y - entity.pos_y;
            let dz = client_pos.z - entity.pos_z;
            let dist_sq = dx * dx + dy * dy + dz * dz;

            if dist_sq > CAMPFIRE_POS_MAX_DELTA * CAMPFIRE_POS_MAX_DELTA {
                warn!(
                    "Campfire position spoofing detected: camp_id={}, client=({:.1}, {:.1}, {:.1}), server=({:.1}, {:.1}, {:.1}), dist={:.1} > max={}",
                    req.last_camp_id,
                    client_pos.x,
                    client_pos.y,
                    client_pos.z,
                    entity.pos_x,
                    entity.pos_y,
                    entity.pos_z,
                    dist_sq.sqrt(),
                    CAMPFIRE_POS_MAX_DELTA,
                );
                // Use the server-known position instead.
                (entity.pos_x, entity.pos_y, entity.pos_z)
            } else {
                // Close enough, accept the client position.
                (client_pos.x, client_pos.y, client_pos.z)
            }
        } else {
            // No server entity for this camp_id (e.g. entity was already
            // cleaned up or the ID doesn't correspond to a tracked entity).
            // Accept the client position as-is since we have nothing to
            // cross-check against.
            (client_pos.x, client_pos.y, client_pos.z)
        };

        let checkpoint = perlica_logic::scene::CheckpointInfo {
            scene_name: ctx.player.scene.scene_name().to_string(),
            pos_x,
            pos_y,
            pos_z,
        };
        ctx.player.scene.set_checkpoint(checkpoint);
    }

    ctx.player
        .scene
        .set_revival_mode(perlica_logic::scene::RevivalMode::CheckPoint);

    if let Err(e) = (SceneSaveState {
        checkpoint: ctx.player.scene.get_checkpoint(),
        revival_mode: ctx.player.scene.current_revival_mode,
    })
    .persist(&ctx.player.uid, ctx.db)
    .await
    {
        warn!(
            "Failed to persist scene save state after set last camp: uid={}, error={}",
            ctx.player.uid, e
        );
    }

    ScSceneSetLastRecordCampid {
        last_camp_id: req.last_camp_id,
    }
}
