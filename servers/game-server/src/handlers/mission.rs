use crate::net::NetContext;
use common::time::now_ms;
use perlica_proto::{
    CsCompleteGuideGroup, CsCompleteGuideGroupKeyStep, CsStopTrackingMission, CsTrackMission,
    CsUpdateQuestObjective, MissionState, QuestState, RoleBaseInfo, ScCompleteGuideGroup,
    ScCompleteGuideGroupKeyStep, ScQuestObjectivesUpdate, ScTrackMissionChange,
};
use tracing::{debug, warn};

pub async fn push_missions(ctx: &mut NetContext<'_>) -> bool {
    ctx.player.missions.ensure_bootstrap(&ctx.assets.missions);
    ctx.notify(ctx.player.missions.sync_packet()).await.is_ok()
}

pub async fn push_guides(ctx: &mut NetContext<'_>) -> bool {
    ctx.notify(perlica_proto::ScSyncAllGuide {
        guide_group_list: vec![],
        completed_repeat_accept_guide_group_list: vec![],
    })
    .await
    .is_ok()
}

pub async fn on_cs_update_quest_objective(
    ctx: &mut NetContext<'_>,
    req: CsUpdateQuestObjective,
) -> ScQuestObjectivesUpdate {
    debug!(
        "Quest objective update: quest_id={}, ops={:?}",
        req.quest_id, req.objective_value_ops
    );

    let role_base_info = Some(build_role_base_info(ctx));
    let update = ctx.player.missions.apply_objective_ops(
        &req.quest_id,
        &req.objective_value_ops,
        &ctx.assets.missions,
        role_base_info.clone(),
    );

    for objective_update in &update.notify_objective_updates {
        if let Err(error) = ctx.notify(objective_update.clone()).await {
            warn!("failed to notify follow-up quest objective update: {error}");
        }
    }

    for state_update in &update.state_updates {
        if let Err(error) = ctx.notify(state_update.clone()).await {
            warn!("failed to notify quest state update: {error}");
        }

        if let Ok(quest_state) = QuestState::try_from(state_update.quest_state) {
            let scene_name = ctx.player.scene.scene_name().to_string();
            let activated = ctx.player.scene.level_scripts.on_quest_state_changed(
                &scene_name,
                &state_update.quest_id,
                quest_state,
                ctx.assets,
            );
            notify_activated_scripts(ctx, activated).await;
        }
    }

    for mission_update in &update.mission_updates {
        if let Err(error) = ctx.notify(mission_update.clone()).await {
            warn!("failed to notify mission state update: {error}");
        }

        if let Ok(mission_state) = MissionState::try_from(mission_update.mission_state) {
            let scene_name = ctx.player.scene.scene_name().to_string();
            let activated = ctx.player.scene.level_scripts.on_mission_state_changed(
                &scene_name,
                &mission_update.mission_id,
                mission_state,
                ctx.assets,
            );
            notify_activated_scripts(ctx, activated).await;
        }
    }

    update.reply_objective_update.unwrap_or_default()
}

pub async fn on_cs_complete_guide_group(
    ctx: &mut NetContext<'_>,
    req: CsCompleteGuideGroup,
) -> ScCompleteGuideGroup {
    ctx.player.guides.mark_group_completed(&req.guide_group_id);

    let scene_name = ctx.player.scene.scene_name().to_string();
    let activated = ctx
        .player
        .scene
        .level_scripts
        .on_guide_group_completed(&scene_name, ctx.assets);
    notify_activated_scripts(ctx, activated).await;

    ScCompleteGuideGroup {
        guide_group_id: req.guide_group_id,
    }
}

pub async fn on_cs_complete_guide_group_key_step(
    ctx: &mut NetContext<'_>,
    req: CsCompleteGuideGroupKeyStep,
) -> ScCompleteGuideGroupKeyStep {
    ctx.player
        .guides
        .mark_key_step_completed(&req.guide_group_id);
    ScCompleteGuideGroupKeyStep {
        guide_group_id: req.guide_group_id,
    }
}

pub async fn on_cs_track_mission(
    ctx: &mut NetContext<'_>,
    req: CsTrackMission,
) -> ScTrackMissionChange {
    ctx.player.missions.update_track_mission(&req.mission_id);
    ScTrackMissionChange {
        mission_id: req.mission_id,
    }
}

pub async fn on_cs_stop_tracking_mission(
    ctx: &mut NetContext<'_>,
    _req: CsStopTrackingMission,
) -> ScTrackMissionChange {
    ctx.player.missions.stop_tracking();
    ScTrackMissionChange {
        mission_id: String::new(),
    }
}

async fn notify_activated_scripts(ctx: &mut NetContext<'_>, script_ids: Vec<i32>) {
    let scene_name = ctx.player.scene.scene_name().to_string();

    for script_id in script_ids {
        if let Some(notify) = ctx
            .player
            .scene
            .level_scripts
            .state_notify(&scene_name, script_id)
        {
            let _ = ctx.notify(notify).await.inspect_err(|error| {
                warn!("failed to notify level script state change: {error}");
            });
        }
    }
}

fn build_role_base_info(ctx: &NetContext<'_>) -> RoleBaseInfo {
    RoleBaseInfo {
        leader_char_id: ctx.player.get_leader_objid(),
        leader_position: Some(perlica_proto::Vector {
            x: ctx.player.movement.pos_x,
            y: ctx.player.movement.pos_y,
            z: ctx.player.movement.pos_z,
        }),
        leader_rotation: Some(perlica_proto::Vector {
            x: ctx.player.movement.rot_x,
            y: ctx.player.movement.rot_y,
            z: ctx.player.movement.rot_z,
        }),
        scene_name: ctx.player.scene.scene_name().to_string(),
        server_ts: now_ms(),
    }
}
