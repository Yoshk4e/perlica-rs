use crate::handlers::{char_bag, scene};
use crate::net::NetContext;
use common::time::now_ms;
use perlica_logic::character::char_bag::CharIndex;
use perlica_proto::{ScObjectEnterView, ScSyncBaseData, SceneObjectDetailContainer, Vector};
use std::collections::HashSet;

pub struct GmOutcome {
    pub message: String,
    pub disconnect: bool,
}

impl GmOutcome {
    fn ok(message: impl Into<String>) -> Result<Self, String> {
        Ok(Self {
            message: message.into(),
            disconnect: false,
        })
    }

    fn kick(message: impl Into<String>) -> Result<Self, String> {
        Ok(Self {
            message: message.into(),
            disconnect: true,
        })
    }
}

pub async fn execute(ctx: &mut NetContext<'_>, raw: &str) -> Result<GmOutcome, String> {
    let command = raw.trim();
    if command.is_empty() {
        return Err("empty command".to_string());
    }

    let parts: Vec<&str> = command.split_whitespace().collect();
    let head = parts[0].to_ascii_lowercase();

    match head.as_str() {
        "help" | "?" => GmOutcome::ok(help_text()),
        "heal" => heal_command(ctx, &parts[1..]).await,
        "setlevel" | "level" => set_level_command(ctx, &parts[1..]).await,
        "teleport" | "tp" => teleport_command(ctx, &parts[1..]).await,
        "spawn" => spawn_command(ctx, &parts[1..]).await,
        "give" => give_command(ctx, &parts[1..]).await,
        "kick" => kick_command(&parts[1..]),
        other => Err(format!(
            "unknown GM command `{}`. Try `help` for supported syntax.",
            other
        )),
    }
}

fn help_text() -> String {
    [
        "Supported commands:",
        "  help",
        "  heal [all|team]",
        "  level <value>",
        "  tp <scene> <x> <y> <z> [rot_y]",
        "  spawn <monster_template> [x y z] [level] [entity_type]",
        "  give weapon <weapon_template>",
        "  kick [reason]",
    ]
    .join("\n")
}

async fn heal_command(ctx: &mut NetContext<'_>, args: &[&str]) -> Result<GmOutcome, String> {
    let heal_all = args
        .first()
        .map(|v| !v.eq_ignore_ascii_case("team"))
        .unwrap_or(true);

    let mut targets = HashSet::new();
    if heal_all {
        for i in 0..ctx.player.char_bag.chars.len() {
            targets.insert(CharIndex::from_usize(i).object_id());
        }
    } else {
        let team_idx = ctx.player.char_bag.meta.curr_team_index as usize;
        if let Some(team) = ctx.player.char_bag.teams.get(team_idx) {
            for objid in team.char_team.iter().filter_map(|slot| slot.object_id()) {
                targets.insert(objid);
            }
        }
    }

    if targets.is_empty() {
        return Err("no characters available to heal".to_string());
    }

    let mut updates = Vec::new();
    for objid in targets {
        let Some(char_data) = ctx.player.char_bag.get_char_by_objid_mut(objid) else {
            continue;
        };
        let max_hp = ctx
            .assets
            .characters
            .get_stats(
                &char_data.template_id,
                char_data.level,
                char_data.break_stage,
            )
            .map(|attrs| attrs.hp)
            .unwrap_or(char_data.hp.max(1.0));
        char_data.hp = max_hp;
        char_data.ultimate_sp = 0.0;
        char_data.is_dead = false;
        updates.push(objid);
    }

    let synced = updates.len();
    if !char_bag::push_char_status_for_ids(ctx, &updates).await {
        return Err("failed to sync healed state".to_string());
    }

    GmOutcome::ok(format!(
        "healed {} character(s) {}",
        synced,
        if heal_all {
            "(all owned)"
        } else {
            "(active team)"
        }
    ))
}

async fn set_level_command(ctx: &mut NetContext<'_>, args: &[&str]) -> Result<GmOutcome, String> {
    let level = args
        .first()
        .ok_or_else(|| "usage: level <value>".to_string())?
        .parse::<i32>()
        .map_err(|_| "level must be an integer".to_string())?;

    if !(1..=100).contains(&level) {
        return Err("level must be between 1 and 100".to_string());
    }

    ctx.player.world.role_level = level;
    ctx.player.world.role_exp = 0;

    ctx.notify(ScSyncBaseData {
        roleid: 1,
        role_name: "BeyondDefault".to_string(),
        level: level as u32,
        exp: 0,
        server_time: now_ms() as i64,
        server_time_zone: 0,
    })
    .await
    .map_err(|e| format!("failed to sync player level: {e}"))?;

    GmOutcome::ok(format!("set live player level to {level}"))
}

async fn teleport_command(ctx: &mut NetContext<'_>, args: &[&str]) -> Result<GmOutcome, String> {
    if args.len() < 4 {
        return Err("usage: tp <scene> <x> <y> <z> [rot_y]".to_string());
    }

    let scene_name = args[0].to_string();
    let x = parse_f32(args[1], "x")?;
    let y = parse_f32(args[2], "y")?;
    let z = parse_f32(args[3], "z")?;
    let rot_y = args
        .get(4)
        .map(|v| parse_f32(v, "rot_y"))
        .transpose()?
        .unwrap_or(ctx.player.movement.rot_y);

    let is_scene_change = ctx.player.scene.current_scene != scene_name;

    ctx.player.movement.update_position(x, y, z);
    ctx.player.movement.update_rotation(0.0, rot_y, 0.0);
    ctx.player.movement.sync_to_world(&mut ctx.player.world);

    if is_scene_change {
        ctx.player.world.last_scene = scene_name.clone();
        let (enter_notify, leave_notify) = ctx.player.scene.begin_scene_transition(
            &scene_name,
            Vector { x, y, z },
            ctx.assets,
            &mut ctx.player.entities,
        );

        ctx.notify(leave_notify)
            .await
            .map_err(|e| format!("failed to send leave-scene packet: {e}"))?;
        ctx.notify(enter_notify)
            .await
            .map_err(|e| format!("failed to send enter-scene packet: {e}"))?;

        return GmOutcome::ok(format!(
            "started scene transition to {} ({x:.2}, {y:.2}, {z:.2})",
            scene_name
        ));
    }

    ctx.player.world.last_scene = scene_name.clone();
    ctx.player.scene.current_scene = scene_name.clone();
    ctx.player
        .scene
        .level_scripts
        .sync_scene(&scene_name, ctx.assets);

    ctx.player.scene.scene_id = ctx
        .assets
        .str_id_num
        .get_scene_id(&scene_name)
        .unwrap_or(ctx.player.scene.scene_id);

    let team_idx = ctx.player.char_bag.meta.curr_team_index as usize;
    let obj_id_list = ctx
        .player
        .char_bag
        .teams
        .get(team_idx)
        .map(|team| {
            team.char_team
                .iter()
                .filter_map(|slot| slot.object_id())
                .collect::<Vec<u64>>()
        })
        .unwrap_or_default();

    let msg = ctx.player.scene.teleport(
        obj_id_list,
        Vector { x, y, z },
        Some(Vector {
            x: 0.0,
            y: rot_y,
            z: 0.0,
        }),
        now_ms() as u32,
        1,
        Some(scene_name.clone()),
    );

    ctx.notify(msg)
        .await
        .map_err(|e| format!("failed to send teleport packet: {e}"))?;

    GmOutcome::ok(format!(
        "teleported player to {} ({x:.2}, {y:.2}, {z:.2})",
        scene_name
    ))
}

async fn spawn_command(ctx: &mut NetContext<'_>, args: &[&str]) -> Result<GmOutcome, String> {
    let template_id = args
        .first()
        .ok_or_else(|| "usage: spawn <monster_template> [x y z] [level] [entity_type]".to_string())?
        .to_string();

    let (x, y, z, next_idx) = if args.len() >= 4 {
        (
            parse_f32(args[1], "x")?,
            parse_f32(args[2], "y")?,
            parse_f32(args[3], "z")?,
            4usize,
        )
    } else {
        (
            ctx.player.movement.pos_x,
            ctx.player.movement.pos_y,
            ctx.player.movement.pos_z,
            1usize,
        )
    };

    let level = args
        .get(next_idx)
        .map(|v| {
            v.parse::<i32>()
                .map_err(|_| "level must be an integer".to_string())
        })
        .transpose()?
        .unwrap_or(1);

    let entity_type = args
        .get(next_idx + 1)
        .map(|v| {
            v.parse::<i32>()
                .map_err(|_| "entity_type must be an integer".to_string())
        })
        .transpose()?
        .unwrap_or(2);

    let level_logic_id = now_ms();
    let (create, monster) = scene::spawn_dynamic_monster(
        ctx,
        template_id.clone(),
        Vector { x, y, z },
        level,
        entity_type,
        level_logic_id,
    );

    ctx.notify(create)
        .await
        .map_err(|e| format!("failed to send scene-create packet: {e}"))?;

    ctx.notify(ScObjectEnterView {
        scene_name: ctx.player.scene.scene_name().to_string(),
        scene_id: ctx.player.scene.scene_id,
        detail: Some(SceneObjectDetailContainer {
            char_list: vec![],
            monster_list: vec![monster],
            interactive_list: vec![],
            npc_list: vec![],
            summon_list: vec![],
        }),
        has_extra_object: false,
    })
    .await
    .map_err(|e| format!("failed to send object-enter-view packet: {e}"))?;

    GmOutcome::ok(format!(
        "spawned `{}` at ({x:.2}, {y:.2}, {z:.2}) in {}",
        template_id,
        ctx.player.scene.scene_name()
    ))
}

async fn give_command(ctx: &mut NetContext<'_>, args: &[&str]) -> Result<GmOutcome, String> {
    if args.len() < 2 {
        return Err("usage: give weapon <weapon_template>".to_string());
    }

    let kind = args[0].to_ascii_lowercase();
    match kind.as_str() {
        "weapon" => {
            let template_id = args[1].to_string();
            ctx.player
                .char_bag
                .item_manager
                .weapons
                .add_weapon(template_id.clone(), now_ms() as i64);

            if !char_bag::push_item_bag_sync(ctx).await {
                return Err("failed to push item bag sync".to_string());
            }

            GmOutcome::ok(format!(
                "granted live weapon `{}` and synced the item bag",
                template_id
            ))
        }
        _ => Err("only `give weapon <weapon_template>` is implemented right now".to_string()),
    }
}

fn kick_command(args: &[&str]) -> Result<GmOutcome, String> {
    let reason = if args.is_empty() {
        "kicked by MUIP".to_string()
    } else {
        args.join(" ")
    };
    GmOutcome::kick(format!("disconnecting player: {reason}"))
}

fn parse_f32(value: &str, label: &str) -> Result<f32, String> {
    value
        .parse::<f32>()
        .map_err(|_| format!("{label} must be a number"))
}
