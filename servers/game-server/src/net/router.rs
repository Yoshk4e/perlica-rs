use crate::handlers::{bitset, character, login, movement, ping, scene};
use byteorder::{LittleEndian, ReadBytesExt};
use perlica_proto::{CsHead, CsMergeMsg, prost::Message};
use std::io::{Cursor, Read};
use tracing::{debug, warn};

macro_rules! handlers {
    ($($msg_req:ty => $handler:path),* $(,)?) => {
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
                    debug!("Merge packet received");
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
                _ => {
                    warn!("unhandled command {cmd_id}");
                }
            }
            Ok(())
        }
    };
}

handlers! {
    CsLogin             => login::on_login,
    CsPing              => ping::on_csping,
    CsSceneLoadFinish   => scene::on_scene_load_finish,
    CsCharSetBattleInfo => character::on_cs_char_set_battle_info,
    CsMoveObjectMove    => movement::on_cs_move_object_move,
    CsBitsetRemove      => bitset::on_cs_bitset_remove,
    CsSceneKillMonster     => scene::on_cs_scene_kill_monster,
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
