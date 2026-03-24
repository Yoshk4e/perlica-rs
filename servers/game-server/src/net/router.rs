//! Network command router for the game server.
//!
//! This module provides the central routing mechanism for all client commands.
//! It uses a declarative macro to map command IDs to their handler functions,
//! making it easy to add new handlers and see at a glance which commands are
//! supported.
//!
//! # Architecture
//!
//! The router uses a two-tier dispatch system:
//! 1. **Merge packets**: Multiple commands bundled together are unpacked and
//!    each sub-command is dispatched individually.
//! 2. **Individual commands**: Each command is decoded and routed to its handler.
//!
//! # Handler Registration
//!
//! Handlers are registered in the `handlers!` macro with two categories:
//! - `reply`: Handlers that send a response back to the client
//! - `no_reply`: Fire-and-forget handlers that don't send a direct response
//!
//! # Adding New Handlers
//!
//! To add a new handler:
//! 1. Create the handler function in the appropriate module (e.g., `handlers/weapon.rs`)
//! 2. Import the handler module in `handlers/mod.rs`
//! 3. Add the CS message type and handler to the appropriate section in the `handlers!` macro below
//!
//! # Example
//!
//! ```rust
//! // In handlers/my_feature.rs:
//! pub async fn on_cs_my_command(ctx: &mut NetContext<'_>, req: CsMyCommand) -> ScMyCommand {
//!     // Handle the command
//!     ScMyCommand { ... }
//! }
//!
//! // In router.rs, add to the macro:
//! handlers! {
//!     reply {
//!         CsMyCommand => my_feature::on_cs_my_command,
//!         // ... other handlers
//!     }
//!     no_reply {
//!         // ... fire-and-forget handlers
//!     }
//! }
//! ```

use crate::handlers::{bitset, character, login, movement, ping, scene, weapon};
use byteorder::{LittleEndian, ReadBytesExt};
use perlica_proto::{CsHead, CsMergeMsg, prost::Message};
use std::io::{Cursor, Read};
use tracing::{debug, warn};

/// Main handler registration macro.
///
/// This macro generates the `handle_command` function that routes incoming
/// commands to their appropriate handlers. It supports two types of handlers:
///
/// - `reply`: Handlers that return a response message to send back to the client
/// - `no_reply`: Handlers that process the request without sending a direct response
///
/// The macro ensures type-safe dispatch by matching command IDs to their
/// corresponding message types and handler functions.
macro_rules! handlers {
    (
        reply    { $($msg_req:ty => $handler:path),* $(,)? }
        no_reply { $($nr_req:ty  => $nr_handler:path),* $(,)? }
    ) => {
        /// Routes an incoming command to its appropriate handler.
        ///
        /// This function is the central dispatch point for all client commands.
        /// It decodes the command body, calls the appropriate handler, and
        /// manages the response flow.
        ///
        /// # Arguments
        /// * `ctx` - The network context for this request
        /// * `cmd_id` - The command ID from the message header
        /// * `body` - The raw message body bytes
        ///
        /// # Returns
        /// * `Result<(), ServerError>` - Ok on success, Err on parse/handle failure
        ///
        /// # Command Flow
        /// 1. If the command is a merge packet, unpack and process each sub-command
        /// 2. For reply handlers: decode -> handle -> send response
        /// 3. For no_reply handlers: decode -> handle (no response sent)
        /// 4. Unknown commands are logged as warnings
        pub async fn handle_command(
            ctx: &mut crate::net::NetContext<'_>,
            cmd_id: i32,
            body: Vec<u8>,
        ) -> crate::error::Result<()> {
            use perlica_proto::*;
            use prost::Message;
            use crate::player::LoadingState;

            match cmd_id {
                // Handle merge packets - multiple commands bundled together
                // This is an optimization for sending multiple commands in one network frame
                x if x == <CsMergeMsg as NetMessage>::CMD_ID => {
                    let req = CsMergeMsg::decode(&body[..])?;
                    debug!("Detected Bundled Messages From Client, {:?}", req.msg.len());
                    handle_merge_msg(ctx, req).await?;
                }

                // Reply handlers: decode request, call handler, send response
                $(
                    x if x == <$msg_req>::CMD_ID => {
                        let req = <$msg_req>::decode(&body[..])?;
                        let rsp = $handler(ctx, req).await;
                        ctx.send(rsp).await?;

                        // If player is in pending loading state, continue login sequence
                        if ctx.player.loading_state == LoadingState::Pending {
                            login::run_login_sequence(ctx).await;
                        }
                    }
                )*

                // No-reply handlers: decode request, call handler (no response)
                $(
                    x if x == <$nr_req>::CMD_ID => {
                        let req = <$nr_req>::decode(&body[..])?;
                        $nr_handler(ctx, req).await;
                    }
                )*

                // Unknown command, log warning but don't fail
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
// - `no_reply`: For fire-and-forget commands (e.g., status updates)
handlers! {
    reply {
        // Core System Commands
        CsLogin                => login::on_login,
        CsPing                 => ping::on_csping,
        CsFlushSync            => ping::on_cs_flush_sync,
        // Scene Commands
        CsSceneLoadFinish      => scene::on_scene_load_finish,
        CsSceneRevival         => scene::on_cs_scene_revival,
        CsSceneInteractiveEventTrigger     => scene::on_cs_scene_interactive_event_trigger,
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
        // Weapon Commands
        CsWeaponPuton          => weapon::on_cs_weapon_puton,
        CsWeaponAddExp         => weapon::on_cs_weapon_add_exp,
        CsWeaponBreakthrough   => weapon::on_cs_weapon_breakthrough,
        CsWeaponAttachGem      => weapon::on_cs_weapon_attach_gem,
        CsWeaponDetachGem      => weapon::on_cs_weapon_detach_gem,
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
    }
}

/// Handles a merge packet containing multiple sub-commands.
///
/// Merge packets are an optimization where the client bundles multiple
/// commands into a single network frame. This function unpacks each
/// sub-command and dispatches it to `handle_command` recursively.
///
/// # Packet Format
///
/// Each merge packet contains:
/// ```text
/// [sub_head_size: u8][sub_body_size: u16][sub_head: bytes][sub_body: bytes]...
/// ```
///
/// # Arguments
/// * `ctx` - The network context for this request
/// * `req` - The merge packet containing all sub-messages
///
/// # Returns
/// * `Result<(), ServerError>` - Ok if all sub-commands processed successfully
async fn handle_merge_msg(
    ctx: &mut crate::net::NetContext<'_>,
    req: CsMergeMsg,
) -> crate::error::Result<()> {
    let data = &req.msg;
    let mut cursor = Cursor::new(data);
    let mut sub_count = 0u32;

    loop {
        // Check if we have enough bytes for another packet header
        let remaining = data.len() as u64 - cursor.position();
        if remaining < 3 {
            break;
        }

        // Read sub-packet header sizes
        let sub_head_size = cursor.read_u8()? as usize;
        let sub_body_size = cursor.read_u16::<LittleEndian>()? as usize;

        // Validate we have enough data for the sub-packet
        let needed = sub_head_size + sub_body_size;
        let available = data.len() - cursor.position() as usize;

        if sub_head_size == 0 || sub_body_size == 0 || needed > available {
            warn!("Malformed sub-packet Detected, aborting {}, {} , {:?}", sub_head_size, sub_body_size, available);
            break;
        }

        // Read sub-packet data
        let mut sub_head_buf = vec![0u8; sub_head_size];
        cursor.read_exact(&mut sub_head_buf)?;
        let mut sub_body_buf = vec![0u8; sub_body_size];
        cursor.read_exact(&mut sub_body_buf)?;

        // Decode the sub-packet header to get the command ID
        let sub_head = CsHead::decode(&sub_head_buf[..])?;

        sub_count += 1;

        // Dispatch the sub-command to its handler
        // Using Box::pin to allow recursive async calls
        if let Err(e) = Box::pin(handle_command(ctx, sub_head.msgid, sub_body_buf)).await {
            warn!("Processing Sub-packet Failed {}, {:?}", e, sub_head);
        }
    }

    debug!("Count of Packets Processed {}", sub_count);
    Ok(())
}
