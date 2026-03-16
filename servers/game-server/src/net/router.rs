use crate::handlers::{bitset, character, login, movement, ping, scene};
use byteorder::{LittleEndian, ReadBytesExt};
use perlica_proto::{CsHead, CsMergeMsg, prost::Message};
use std::io::{Cursor, Read};
use tracing::{debug, warn};

macro_rules! handlers {
    (
        reply    { $($msg_req:ty => $handler:path),* $(,)? }
        no_reply { $($nr_req:ty  => $nr_handler:path),* $(,)? }
    ) => {
        pub async fn handle_command(
            // This function is like the traffic cop for all incoming commands. It directs each command to the right handler. So important! XD
            ctx: &mut crate::net::NetContext<'_>,
            cmd_id: i32,
            body: Vec<u8>,
        ) -> anyhow::Result<()> {
            use perlica_proto::*;
            use prost::Message;
            use crate::player::LoadingState;

            match cmd_id {
                // Checking the command ID to see what kind of request we got.
                x if x == <CsMergeMsg as NetMessage>::CMD_ID => {
                    // Oh, a merge packet! This means multiple commands are bundled together. Efficiency, baby!
                    let req = CsMergeMsg::decode(&body[..])?;
                    debug!(payload = req.msg.len(), "merge packet"); // Just a little debug info about the merge packet size. Don't mind us.
                    handle_merge_msg(ctx, req).await?; // Unpacking and handling all those merged messages. It's like opening a present, but with more code.
                }

                // Handlers that send a response back to the client.
                $(
                    x if x == <$msg_req>::CMD_ID => { // Found a command that needs a reply! We're not rude, we always respond.
                        let req = <$msg_req>::decode(&body[..])?; // Decoding the request. Turning bytes into something readable, like magic.
                        let rsp = $handler(ctx, req).await; // Calling the actual handler for this command. This is where the real work gets done.
                        ctx.send(rsp).await?; // Sending the response back to the client. Don't leave 'em hanging!
                        if ctx.player.loading_state == LoadingState::Pending { // If the player is still loading, let's get them sorted.
                            login::run_login_sequence(ctx).await; // Running the login sequence. Get that player fully in the game!
                        }
                    }
                )*

                // Fire-and-forget handlers
                $(
                    x if x == <$nr_req>::CMD_ID => { // A fire-and-forget command. No reply needed, just do the thing.
                        let req = <$nr_req>::decode(&body[..])?;
                        $nr_handler(ctx, req).await;
                    }
                )*

                _ => {
                    // Uh oh, we got a command we don't know. Better warn someone! LOL
                    warn!(cmd_id, "unhandled command");
                }
            }
            Ok(())
        }
    };
}

handlers! {
    reply {
        CsLogin                => login::on_login,
        CsPing                 => ping::on_csping,
        CsSceneLoadFinish      => scene::on_scene_load_finish,
        CsMoveObjectMove       => movement::on_cs_move_object_move,
        CsBitsetRemove         => bitset::on_cs_bitset_remove,
        CsCharBagSetTeamLeader => character::on_cs_char_bag_set_team_leader,
        CsSceneRevival         => scene::on_cs_scene_revival,
    }
    no_reply {
        CsCharSetBattleInfo => character::on_cs_char_set_battle_info,
        CsSceneKillChar     => scene::on_cs_scene_kill_char,
        CsSceneKillMonster  => scene::on_cs_scene_kill_monster,
    }
}

// Merge payload is a concatenation of multiple framed packets.
async fn handle_merge_msg(
    // This function is for when the client sends a bunch of messages at once. We gotta unbundle 'em.
    ctx: &mut crate::net::NetContext<'_>,
    req: CsMergeMsg,
) -> anyhow::Result<()> {
    let data = &req.msg; // The raw data from the merged message.
    let mut cursor = Cursor::new(data); // A cursor to help us read through the data. Like a bookmark, but for bytes.
    let mut sub_count = 0u32; // Keeping track of how many sub-messages we've processed. Gotta count 'em all!

    loop {
        let remaining = data.len() as u64 - cursor.position();
        if remaining < 3 {
            // If there's not enough data for another packet, we're done here.
            break;
        }

        let sub_head_size = cursor.read_u8()? as usize; // Reading the size of the sub-packet's header.
        let sub_body_size = cursor.read_u16::<LittleEndian>()? as usize; // Reading the size of the sub-packet's body.

        let needed = sub_head_size + sub_body_size;
        let available = data.len() - cursor.position() as usize;

        if sub_head_size == 0 || sub_body_size == 0 || needed > available {
            // Checking for malformed packets. Can't trust everyone, ya know?
            // Something's fishy with this sub-packet. Let's log a warning.
            warn!(
                head_size = sub_head_size,
                body_size = sub_body_size,
                available,
                "merge sub-packet invalid"
            );
            break;
        }

        let mut sub_head_buf = vec![0u8; sub_head_size]; // Buffer for the sub-packet's header.
        cursor.read_exact(&mut sub_head_buf)?;
        let mut sub_body_buf = vec![0u8; sub_body_size]; // Buffer for the sub-packet's body.
        cursor.read_exact(&mut sub_body_buf)?;

        let sub_head = CsHead::decode(&sub_head_buf[..])?; // Decoding the sub-packet's header.

        sub_count += 1; // Incrementing the sub-message counter. One down, many more to go (maybe).
        // Recursively handling the sub-command. It's like Inception, but with network packets.
        if let Err(e) = Box::pin(handle_command(ctx, sub_head.msgid, sub_body_buf)).await {
            // Something's fishy with this sub-packet. Let's log a warning.
            warn!(cmd_id = sub_head.msgid, error = %e, "merge sub-packet failed");
        }
    }

    debug!(count = sub_count, "merge packet processed"); // All merged packets processed! Phew, that was a lot.
    Ok(())
}
