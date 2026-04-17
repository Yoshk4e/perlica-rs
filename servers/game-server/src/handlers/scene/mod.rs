//! Scene command handlers.

pub mod dialog;
pub mod entity;
pub mod level_script;
pub mod load;
pub mod revival;
pub mod teleport;

pub use dialog::on_cs_finish_dialog;
pub use entity::{on_cs_scene_create_entity, on_cs_scene_destroy_entity, spawn_dynamic_monster};
pub use level_script::{
    on_cs_scene_commit_level_script_cache_step, on_cs_scene_level_script_event_trigger,
    on_cs_scene_set_level_script_active, on_cs_scene_update_interactive_property,
    on_cs_scene_update_level_script_property,
};
pub use load::{notify_enter_scene, on_scene_load_finish};
pub use revival::{
    on_cs_scene_kill_char, on_cs_scene_kill_monster, on_cs_scene_revival,
    on_cs_scene_set_last_record_campid,
};
pub use teleport::on_cs_scene_teleport;

// Small shared helpers that don't belong to any single sub-feature.
use crate::net::NetContext;

#[allow(dead_code)]
pub fn entity_exists(ctx: &NetContext<'_>, entity_id: u64) -> bool {
    ctx.player.entities.contains(entity_id)
}

#[allow(dead_code)]
pub fn current_scene_name<'a>(ctx: &'a NetContext<'_>) -> &'a str {
    ctx.player.scene.scene_name()
}
