use crate::handlers::{character, login, movement, scene};
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

handlers! {
    CsLogin            => login::on_login,
    CsPing             => login::on_csping,
    CsSceneLoadFinish  => scene::on_scene_load_finish,
    CsCharSetBattleInfo => character::on_cs_char_set_battle_info,
    CsMoveObjectMove   => movement::on_cs_move_object_move,
}
