//! Character/monster kills, revival, and campfire checkpoints.

use crate::net::NetContext;
use perlica_logic::character::char_bag::CharIndex;
use perlica_logic::scene::EntityDestroyReason;
use perlica_proto::{
    BattleInfo, CsSceneKillChar, CsSceneKillMonster, CsSceneRevival, CsSceneSetLastRecordCampid,
    ScCharSyncStatus, ScObjectEnterView, ScSceneSetLastRecordCampid,
};
use tracing::{debug, error, info};

/// Removes a monster entity and notifies the client with `ScSceneDestroyEntity`.
pub async fn on_cs_scene_kill_monster(ctx: &mut NetContext<'_>, req: CsSceneKillMonster) {
    debug!("Monster killed: {}", req.id);

    if let Some(entity) = ctx
        .player
        .entities
        .remove(req.id)
        .filter(|e| e.kind == perlica_logic::entity::EntityKind::Enemy)
    {
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

/// Marks a character dead and destroys the scene entity. The `CharBag` entry is preserved for revival.
pub async fn on_cs_scene_kill_char(ctx: &mut NetContext<'_>, req: CsSceneKillChar) {
    debug!("Character killed: {}", req.id);

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
}

/// Handles `CsSceneRevival`, revives all dead characters in the current team
/// at 50 % HP.
/// ORDER MATTERS!!
/// Send order:
/// 1. `ScCharSyncStatus` × N, HP per revived char
/// 2. `ScSelfSceneInfo` with `revive_chars`, triggers client revival logic
/// 3. `ScSceneRevival` — revival UI/effect
/// 4. `ScObjectEnterView` — reply, re-enters chars into the scene
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
/// Send order:
///   1. `ScSceneSetLastRecordCampid` — ACK echoing the camp id back.
pub async fn on_cs_scene_set_last_record_campid(
    ctx: &mut NetContext<'_>,
    req: CsSceneSetLastRecordCampid,
) -> ScSceneSetLastRecordCampid {
    info!("Setting last campfire: camp_id={}", req.last_camp_id);

    if let Some(pos) = &req.position {
        let checkpoint = perlica_logic::scene::CheckpointInfo {
            scene_name: ctx.player.scene.scene_name().to_string(),
            pos_x: pos.x,
            pos_y: pos.y,
            pos_z: pos.z,
        };
        ctx.player.scene.set_checkpoint(checkpoint);
    }

    ctx.player
        .scene
        .set_revival_mode(perlica_logic::scene::RevivalMode::CheckPoint);

    ScSceneSetLastRecordCampid {
        last_camp_id: req.last_camp_id,
    }
}
