//! Command router — maps incoming `cmd_id` values to their handler functions.
//!
//! To add a handler: create the fn in `handlers/<feature>.rs`, import the module
//! in `handlers/mod.rs`, then add `CsMyCommand => feature::on_cs_my_command` to
//! the `reply` or `no_reply` block in the `handlers!` macro below.
//! Use `reply` when the handler returns a response; `no_reply` for fire-and-forget.

use crate::handlers::{
    bitset, character, equip, heartbeat, login, mission, movement, scene, weapon,
};
use byteorder::{LittleEndian, ReadBytesExt};
use perlica_proto::{CsHead, CsMergeMsg, prost::Message};
use std::io::{Cursor, Read};
use tracing::{debug, warn};

// Generates the `handle_command` dispatch function from a list of (CsType => handler) pairs.
macro_rules! handlers {
    (
        reply    { $($msg_req:ty => $handler:path),* $(,)? }
        no_reply { $($nr_req:ty  => $nr_handler:path),* $(,)? }
    ) => {
        pub async fn handle_command(
            ctx: &mut crate::net::NetContext<'_>,
            cmd_id: i32,
            body: Vec<u8>,
        ) -> crate::error::Result<()> {
            use perlica_proto::*;
            use prost::Message;
            use crate::player::LoadingState;

            match cmd_id {
                // Handle merge packets - multiple commands bundled into one frame
                x if x == <CsMergeMsg as NetMessage>::CMD_ID => {
                    let req = CsMergeMsg::decode(&body[..])?;
                    debug!("Detected Bundled Messages From Client, {:?}", req.msg.len());
                    handle_merge_msg(ctx, req).await?;
                }

                // Reply handlers: decode, dispatch, send response
                $(
                    x if x == <$msg_req>::CMD_ID => {
                        let req = <$msg_req>::decode(&body[..])?;
                        let rsp = $handler(ctx, req).await;
                        ctx.send(rsp).await?;

                        if ctx.player.loading_state == LoadingState::Pending {
                            login::run_login_sequence(ctx).await;
                        }
                    }
                )*

                // No-reply handlers: decode and dispatch only
                $(
                    x if x == <$nr_req>::CMD_ID => {
                        let req = <$nr_req>::decode(&body[..])?;
                        $nr_handler(ctx, req).await;
                    }
                )*

                _ => {
                    warn!("Unhandled command, {:?}", cmd_id);
                }
            }
            Ok(())
        }
    };
}

// Register all command handlers here.
// Add new handlers to the appropriate section:
// - `reply`: For commands that expect a response (most commands)
// - `no_reply`: For fire-and-forget commands (e.g., status updates) or if you want to have complete control over the wire
handlers! {
    reply {
        // Core System Commands
        CsLogin                => login::on_login,
        CsPing                 => heartbeat::on_csping,
        CsFlushSync            => heartbeat::on_cs_flush_sync,
        // Scene Commands
        CsSceneLoadFinish      => scene::on_scene_load_finish,
        CsSceneRevival         => scene::on_cs_scene_revival,
        //CsSceneInteractiveEventTrigger     => scene::on_cs_scene_interactive_event_trigger,
        CsSceneSetLastRecordCampid         => scene::on_cs_scene_set_last_record_campid,
        CsSceneTeleport        => scene::on_cs_scene_teleport,
        // Entity Lifecycle Commands
        CsSceneCreateEntity    => scene::on_cs_scene_create_entity,
        // Level Script Commands
        CsSceneUpdateLevelScriptProperty => scene::on_cs_scene_update_level_script_property,
        CsSceneUpdateInteractiveProperty => scene::on_cs_scene_update_interactive_property,
        CsSceneLevelScriptEventTrigger   => scene::on_cs_scene_level_script_event_trigger,
        // Movement Commands
        CsMoveObjectMove       => movement::on_cs_move_object_move,
        // Character & Team Commands
        CsCharBagSetTeamLeader    => character::on_cs_char_bag_set_team_leader,
        CsCharBagSetTeamName      => character::on_cs_char_bag_set_team_name,
        // Character Progression Commands
        CsCharLevelUp             => character::on_cs_char_level_up,
        CsCharBreak               => character::on_cs_char_break,
        CsCharSetNormalSkill      => character::on_cs_char_set_normal_skill,
        CsCharSkillLevelUp        => character::on_cs_char_skill_level_up,
        CsCharSetTeamSkill        => character::on_cs_char_set_team_skill,
        // Bitset Commands
        CsBitsetAdd            => bitset::on_cs_bitset_add,
        CsBitsetRemove         => bitset::on_cs_bitset_remove,
        // Mission & Guide Commands
        CsUpdateQuestObjective => mission::on_cs_update_quest_objective,
        CsCompleteGuideGroupKeyStep => mission::on_cs_complete_guide_group_key_step,
        CsCompleteGuideGroup   => mission::on_cs_complete_guide_group,
        CsTrackMission         => mission::on_cs_track_mission,
        CsStopTrackingMission  => mission::on_cs_stop_tracking_mission,
        // Weapon Commands
        CsWeaponPuton          => weapon::on_cs_weapon_puton,
        CsWeaponAddExp         => weapon::on_cs_weapon_add_exp,
        CsWeaponBreakthrough   => weapon::on_cs_weapon_breakthrough,
        CsWeaponAttachGem      => weapon::on_cs_weapon_attach_gem,
        CsWeaponDetachGem      => weapon::on_cs_weapon_detach_gem,
        // Equipment Commands
        CsEquipPuton           => equip::on_cs_equip_puton,
        CsEquipPutoff          => equip::on_cs_equip_putoff,
        // Item Tag Commands
        CsRemoveItemNewTags    => equip::on_cs_remove_item_new_tags,
        // Dialog or Story related commands
        CsFinishDialog                   => scene::on_cs_finish_dialog,
    }
    no_reply {
        // Team Composition (self-ACK, controls send order)
        CsCharBagSetTeam          => character::on_cs_char_bag_set_team,
        CsCharBagSetCurrTeamIndex => character::on_cs_char_bag_set_curr_team_index,
        // Character Status Updates
        CsCharSetBattleInfo    => character::on_cs_char_set_battle_info,
        // Scene Events (no response needed)
        CsSceneKillChar        => scene::on_cs_scene_kill_char,
        CsSceneKillMonster     => scene::on_cs_scene_kill_monster,
        // Entity/Script fire-and-forget
        CsSceneDestroyEntity   => scene::on_cs_scene_destroy_entity,
        CsSceneSetLevelScriptActive => scene::on_cs_scene_set_level_script_active,
        CsSceneCommitLevelScriptCacheStep => scene::on_cs_scene_commit_level_script_cache_step,
    }
}

/// Unpacks a `CsMergeMsg` and dispatches each sub-command individually.
///
/// Sub-packet wire format: `[head_size: u8][body_size: u16][head][body]`
async fn handle_merge_msg(
    ctx: &mut crate::net::NetContext<'_>,
    req: CsMergeMsg,
) -> crate::error::Result<()> {
    let data = &req.msg;
    let mut cursor = Cursor::new(data);
    let mut sub_count = 0u32;

    loop {
        let remaining = data.len() as u64 - cursor.position();
        if remaining < 3 {
            break;
        }

        let sub_head_size = cursor.read_u8()? as usize;
        let sub_body_size = cursor.read_u16::<LittleEndian>()? as usize;

        let needed = sub_head_size + sub_body_size;
        let available = data.len() - cursor.position() as usize;

        if sub_head_size == 0 || needed > available {
            // body may be empty; warn and abort rather than mis-parsing the rest
            warn!(
                "Malformed sub-packet Detected, aborting {}, {} , {:?}",
                sub_head_size, sub_body_size, available
            );
            break;
        }

        let mut sub_head_buf = vec![0u8; sub_head_size];
        cursor.read_exact(&mut sub_head_buf)?;
        let mut sub_body_buf = vec![0u8; sub_body_size];
        cursor.read_exact(&mut sub_body_buf)?;

        let sub_head = CsHead::decode(&sub_head_buf[..])?;

        sub_count += 1;

        if let Err(e) = Box::pin(handle_command(ctx, sub_head.msgid, sub_body_buf)).await {
            warn!("Processing Sub-packet Failed {}, {:?}", e, sub_head);
        }
    }

    debug!("Count of Packets Processed {}", sub_count);
    Ok(())
}
