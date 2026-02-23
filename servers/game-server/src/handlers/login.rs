use crate::player::LoadingState;
use crate::session::NetContext;
use byteorder::{LittleEndian, ReadBytesExt};
use perlica_proto::{CsHead, CsLogin, CsMergeMsg, CsPing, ScLogin, ScPing, prost::Message};
use std::io::{self, Cursor, Read};
use tracing::debug;

pub async fn on_login(ctx: &mut NetContext<'_>, req: CsLogin) -> ScLogin {
    ctx.player.on_login(req.uid.clone());
    debug!("Player logged in: {}", req.uid);

    ScLogin {
        uid: req.uid,
        is_first_login: false,
        server_public_key: vec![],
        server_encryp_nonce: vec![],
        last_recv_up_seqid: ctx.client_seq_id,
        is_reconnect: false,
        is_enc: false,
        is_client_reconnect: false,
    }
}

pub async fn on_csping(_ctx: &mut NetContext<'_>, req: CsPing) -> ScPing {
    ScPing {
        client_ts: req.client_ts,
        server_ts: now_ms(),
    }
}

pub(crate) async fn run_login_sequence(ctx: &mut NetContext<'_>) {
    loop {
        // Each arm does the work for the current state, then advances.
        // States are named after what was JUST completed, not what's next.
        let ok = match ctx.player.loading_state {
            LoadingState::ScLogin => super::char_bag::send_char_bag_sync(ctx).await,
            LoadingState::CharBagSync => super::char_bag::send_unlock_sync(ctx).await,
            LoadingState::UnlockSync => super::scene::enter_scene_notify(ctx).await,
            LoadingState::EnterScene | LoadingState::Complete | LoadingState::Login => break,
        };
        if ok {
            ctx.player.advance_state();
        } else {
            break;
        }
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub async fn on_cs_merge_msg(ctx: &mut NetContext<'_>, req: CsMergeMsg) -> anyhow::Result<()> {
    let data = &req.msg;
    let mut cursor = Cursor::new(data);
    let mut sub_count = 0u32;

    loop {
        // Check if at least 3 bytes remain for header sizes
        let remaining = data.len() as u64 - cursor.position();
        if remaining < 3 {
            break;
        }

        let sub_head_size = cursor.read_u8()? as usize;
        let sub_body_size = cursor.read_u16::<LittleEndian>()? as usize;

        let needed = sub_head_size + sub_body_size;
        let available = data.len() - cursor.position() as usize;

        if sub_head_size == 0 || sub_body_size == 0 || needed > available {
            tracing::warn!(
                "Invalid merge sub-packet: head_size={}, body_size={}, available={}",
                sub_head_size,
                sub_body_size,
                available
            );
            break;
        }
        let mut sub_head_buf = vec![0u8; sub_head_size];
        cursor.read_exact(&mut sub_head_buf)?;
        let mut sub_body_buf = vec![0u8; sub_body_size];
        cursor.read_exact(&mut sub_body_buf)?;
        let sub_head = CsHead::decode(&sub_head_buf[..])?;
        let sub_cmd_id = sub_head.msgid;

        sub_count += 1;
        if let Err(e) = Box::pin(crate::handler::handle_command(
            ctx,
            sub_cmd_id,
            sub_body_buf,
        ))
        .await
        {
            tracing::warn!("Merge sub-packet failed cmd={} : {:?}", sub_cmd_id, e);
        }
    }

    if sub_count > 0 {
        tracing::debug!("Processed CsMergeMsg with {} sub-packets", sub_count);
    } else {
        tracing::warn!("Empty CsMergeMsg received");
    }

    Ok(())
}
