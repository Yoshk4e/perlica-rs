use crate::player::{
    on_cs_char_set_battle_info, on_cs_move_object_move, on_csping, on_login, on_scene_load_finish,
};

macro_rules! handlers {
    ($($msg_req:ty => $handler:path),* $(,)?) => {
        pub async fn handle_command(
            ctx: &mut crate::session::NetContext<'_>,
            cmd_id: i32,
            body: Vec<u8>,
        ) -> anyhow::Result<()> {
            use perlica_proto::*;
            use prost::Message;

            match cmd_id {
                $(
                    x if x == <$msg_req as NetMessage>::CMD_ID => {
                        let req = <$msg_req>::decode(&body[..])?;
                        let rsp = $handler(ctx, req).await;
                        ctx.send(rsp).await?;
                    }
                )*
                _ => {
                    eprintln!("Unhandled command: {}", cmd_id);
                }
            }
            Ok(())
        }
    };
}

#[macro_export]
macro_rules! login_sequence {
    ($($state:path => $handler:path),* $(,)?) => {
        pub async fn handle_login_sequence(
            ctx: &mut $crate::session::NetContext<'_>,
        ) -> anyhow::Result<bool> {
            use $crate::player::LoadingState;

            if let Some(player) = ctx.player.as_mut() {
                match player.loading_state {
                    $(
                        $state => {
                            $handler(ctx, player).await?;
                            player.advance_state();
                            return Ok(true);
                        }
                    )*
                    LoadingState::Complete => return Ok(false),
                    _ => return Ok(false),
                }
            }
            Ok(false)
        }
    };
}

handlers! {
    CsLogin => on_login,
    CsPing => on_csping,
    CsSceneLoadFinish => on_scene_load_finish,
    CsCharSetBattleInfo => on_cs_char_set_battle_info,
    CsMoveObjectMove => on_cs_move_object_move,
}
