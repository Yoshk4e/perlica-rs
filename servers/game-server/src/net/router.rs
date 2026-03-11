use crate::handlers::{bitset, character, login, movement, ping, scene};
use byteorder::{LittleEndian, ReadBytesExt};
use perlica_proto::{CsHead, CsMergeMsg, prost::Message};
use std::io::{Cursor, Read};
use tracing::{debug, instrument, warn};

macro_rules! handlers {
    (
        reply    { $($msg_req:ty => $handler:path),* $(,)? }
        no_reply { $($nr_req:ty  => $nr_handler:path),* $(,)? }
    ) => {
        #[instrument(skip(ctx, body), fields(uid = %ctx.player.uid, cmd_id))]
        pub async fn handle_command(
            ctx: &mut crate::net::NetContext<'_>,
            cmd_id: i32,
            body: Vec<u8>,
        ) -> anyhow::Result<()> {
            use perlica_proto::*;
            use prost::Message;
            use crate::player::LoadingState;

            match cmd_id {
                x if x == <CsMergeMsg as NetMessage>::CMD_ID => {
                    let req = CsMergeMsg::decode(&body[..])?;
                    debug!(payload = req.msg.len(), "merge packet");
                    handle_merge_msg(ctx, req).await?;
                }

                $(
                    x if x == <$msg_req as NetMessage>::CMD_ID => {
                        let req = <$msg_req>::decode(&body[..])?;
                        let rsp = $handler(ctx, req).await;
                        ctx.send(rsp).await?;
                        if ctx.player.loading_state == LoadingState::ScLogin {
                            login::run_login_sequence(ctx).await;
                        }
                    }
                )*

                $(
                    x if x == <$nr_req as NetMessage>::CMD_ID => {
                        let req = <$nr_req>::decode(&body[..])?;
                        $nr_handler(ctx, req).await;
                    }
                )*

                _ => {
                    warn!(cmd_id, "unhandled command");
                }
            }
            Ok(())
        }
    };
}

handlers! {
    reply {
        CsLogin             => login::on_login,
        CsPing              => ping::on_csping,
        CsSceneLoadFinish   => scene::on_scene_load_finish,
        CsMoveObjectMove    => movement::on_cs_move_object_move,
        CsBitsetRemove      => bitset::on_cs_bitset_remove,
        CsCharBagSetTeamLeader => character::on_cs_char_bag_set_team_leader,
    }
    no_reply {
        CsCharSetBattleInfo => character::on_cs_char_set_battle_info,
        CsSceneKillChar     => scene::on_cs_scene_kill_char,
		CsSceneKillMonster  => scene::on_cs_scene_kill_monster,
    }
}

// Merge payload is a concatenation of multiple framed packets.
async fn handle_merge_msg(
    ctx: &mut crate::net::NetContext<'_>,
    req: CsMergeMsg,
) -> anyhow::Result<()> {
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

        if sub_head_size == 0 || sub_body_size == 0 || needed > available {
            warn!(
                head_size = sub_head_size,
                body_size = sub_body_size,
                available,
                "merge sub-packet invalid"
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
            warn!(cmd_id = sub_head.msgid, error = %e, "merge sub-packet failed");
        }
    }

    debug!(count = sub_count, "merge packet processed");
    Ok(())
}
