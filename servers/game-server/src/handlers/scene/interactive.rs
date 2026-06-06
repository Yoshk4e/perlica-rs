//! Interactive object event triggers (campfire activation, chest opening, ...).

use crate::net::NetContext;
use config::item::{CraftShowingType, ItemDepotType, ItemKind};
use perlica_db::{Persistable, SceneSaveState};
use perlica_logic::enums::{ParamRealType, ParamValueType};
use perlica_proto::{
    Code, CsSceneInteractiveEventTrigger, CsSceneSetSafeZone, DynamicParameter, RewardItem,
    ScRewardToSceneBegin, ScSceneCollectionSync, ScSceneInteractiveEventTrigger,
    ScSceneSetSafeZone, ScSceneUpdateInteractiveProperty, SceneCollection,
};
use tracing::{debug, info, warn};

// FIXME: keys i handle as of now but this should be handled better..
const WRITABLE_INTERACTIVE_KEYS: &[&str] = &["is_collected", "is_enabled"];

fn is_campfire(entity: &perlica_logic::entity::SceneEntity) -> bool {
    entity.template_id.to_ascii_lowercase().contains("campfire")
}

fn is_chest(entity: &perlica_logic::entity::SceneEntity) -> bool {
    let lower = entity.template_id.to_ascii_lowercase();
    lower.contains("trchest") || lower.contains("focus_chest")
}

fn bool_param_true() -> DynamicParameter {
    DynamicParameter {
        value_type: ParamValueType::Bool as i32,
        real_type: ParamRealType::Bool as i32,
        value_bool_list: vec![true],
        ..Default::default()
    }
}

fn read_string_param(p: &DynamicParameter) -> Option<&str> {
    p.value_string_list.first().map(|s| s.as_str())
}

fn filter_writable_properties(
    client_props: &std::collections::HashMap<String, DynamicParameter>,
) -> std::collections::HashMap<String, DynamicParameter> {
    client_props
        .iter()
        .filter(|(key, _)| WRITABLE_INTERACTIVE_KEYS.contains(&key.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub async fn on_cs_scene_interactive_event_trigger(
    ctx: &mut NetContext<'_>,
    req: CsSceneInteractiveEventTrigger,
) -> ScSceneInteractiveEventTrigger {
    info!(
        "Interactive event trigger: scene={}, id={}, event={}, props={:?}",
        req.scene_name, req.id, req.event_name, req.properties
    );

    // scene name must match the player's current scene
    if req.scene_name != ctx.player.scene.current_scene {
        warn!(
            "Rejected interactive event: request scene='{}' does not match current scene='{}'",
            req.scene_name, ctx.player.scene.current_scene
        );
        ctx.send_error(
            Code::ErrSceneNameNotExist,
            format!(
                "scene '{}' does not match current scene '{}'",
                req.scene_name, ctx.player.scene.current_scene
            ),
        )
        .await;
        return ScSceneInteractiveEventTrigger {};
    }

    let kind = ctx
        .player
        .entities
        .get(req.id)
        .map(|e| {
            if is_chest(e) {
                InteractiveKind::Chest
            } else if is_campfire(e) {
                InteractiveKind::Campfire
            } else {
                InteractiveKind::Other
            }
        })
        .unwrap_or(InteractiveKind::Other);

    match (kind, req.event_name.as_str()) {
        (InteractiveKind::Chest, _) => handle_chest_open(ctx, &req).await,
        (InteractiveKind::Campfire, _) => handle_activate(ctx, &req).await,
        (InteractiveKind::Other, "activate") => handle_activate(ctx, &req).await,
        (InteractiveKind::Other, other) => {
            warn!(
                "Unhandled interactive event '{}' for id={} in scene '{}'",
                other, req.id, req.scene_name
            );
        }
    }

    // If a campfire was activated, the checkpoint/revival mode changed — persist it.
    if kind == InteractiveKind::Campfire || kind == InteractiveKind::Other {
        if let Err(e) = (SceneSaveState {
            checkpoint: ctx.player.scene.get_checkpoint(),
            revival_mode: ctx.player.scene.current_revival_mode,
        })
        .persist(&ctx.player.uid, ctx.db)
        .await
        {
            warn!(
                "Failed to persist scene save state after interactive event: uid={}, error={}",
                ctx.player.uid, e
            );
        }
    };

    ScSceneInteractiveEventTrigger {}
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum InteractiveKind {
    Campfire,
    Chest,
    Other,
}

// FIXME: this is pretty messy but will be revamped in the future.... (it doesn't also work correctly as much please do not bug me about it)
async fn handle_activate(ctx: &mut NetContext<'_>, req: &CsSceneInteractiveEventTrigger) {
    let entity_id = req.id;
    let scene_name = req.scene_name.clone();

    let is_camp = ctx
        .player
        .entities
        .get(entity_id)
        .map(is_campfire)
        .unwrap_or(false);

    if !is_camp {
        warn!(
            "Activate event for non-campfire interactive id={} in '{}'",
            entity_id, scene_name
        );
    }

    let enabled_param = bool_param_true();
    ctx.player
        .scene
        .update_interactive_property(entity_id, "is_enabled", enabled_param.clone());

    let mut updated_props = std::collections::HashMap::new();
    updated_props.insert("is_enabled".to_string(), enabled_param);

    let filtered = filter_writable_properties(&req.properties);
    for (key, value) in filtered {
        updated_props.insert(key, value);
    }

    let prop_update = ScSceneUpdateInteractiveProperty {
        scene_name: scene_name.clone(),
        id: entity_id,
        properties: updated_props,
        client_operate: false,
    };

    let _ = ctx.notify(prop_update).await;

    if is_camp {
        if let Some(entity) = ctx.player.entities.get(entity_id) {
            ctx.player
                .scene
                .set_checkpoint(perlica_logic::scene::CheckpointInfo {
                    scene_name: scene_name.clone(),
                    pos_x: entity.pos_x,
                    pos_y: entity.pos_y,
                    pos_z: entity.pos_z,
                });
            ctx.player
                .scene
                .set_revival_mode(perlica_logic::scene::RevivalMode::CheckPoint);

            debug!(
                "Campfire activated: id={}, checkpoint set at ({}, {}, {}) in '{}'",
                entity_id, entity.pos_x, entity.pos_y, entity.pos_z, scene_name
            );
        }
    }

    info!(
        "Interactive 'activate' processed for id={} in '{}'",
        entity_id, scene_name
    );
}

const REWARD_SOURCE_TYPE_CHEST: i32 = 1;

async fn handle_chest_open(ctx: &mut NetContext<'_>, req: &CsSceneInteractiveEventTrigger) {
    let entity_id = req.id;
    let scene_name = req.scene_name.clone();

    let (template_id, reward_id_from_lv) = {
        let entity = match ctx.player.entities.get(entity_id) {
            Some(e) => e,
            None => {
                warn!(
                    "Chest open for unknown entity id={} in '{}'",
                    entity_id, scene_name
                );
                return;
            }
        };
        let template = entity.template_id.clone();
        let props = ctx.player.scene.get_interactive_properties(entity_id);
        let reward_id = props
            .as_ref()
            .and_then(|m| m.get("key"))
            .and_then(read_string_param)
            .map(str::to_string);
        (template, reward_id)
    };

    let reward_id = req
        .properties
        .get("key")
        .and_then(read_string_param)
        .map(str::to_string)
        .or(reward_id_from_lv);

    let Some(reward_id) = reward_id else {
        warn!(
            "Chest id={} ({}) has no `key` reward id in lv_data - opening without rewards",
            entity_id, template_id
        );
        mark_chest_collected(ctx, entity_id, &scene_name, &req.properties).await;
        return;
    };

    let bundles: Vec<(String, i64)> = match ctx.assets.rewards.get(&reward_id) {
        Some(entry) => entry
            .item_bundles
            .iter()
            .map(|b| (b.id.clone(), b.count))
            .collect(),
        None => {
            warn!(
                "Chest id={} ({}) references unknown rewardId `{}` - opening with empty drop",
                entity_id, template_id, reward_id
            );
            Vec::new()
        }
    };

    info!(
        "Opening chest id={} template={} rewardId={} with {} bundle(s)",
        entity_id,
        template_id,
        reward_id,
        bundles.len()
    );

    mark_chest_collected(ctx, entity_id, &scene_name, &req.properties).await;

    let _ = ctx
        .notify(ScRewardToSceneBegin {
            reward_source_type: REWARD_SOURCE_TYPE_CHEST,
            source_template_id: template_id.clone(),
        })
        .await;

    let collection_list = bundles
        .iter()
        .map(|(_item_id, count)| SceneCollection {
            scene_name: scene_name.clone(),
            prefab_id: "int_trchest_common_normal".to_string(),
            count: *count as i32,
        })
        .collect();

    let _ = ctx.notify(ScSceneCollectionSync { collection_list }).await;

    /*if let Err(e) = ctx.notify(ScRewardToSceneEnd {}).await {
        warn!("Failed to send ScRewardToSceneEnd: {:?}", e);
    }*/

    info!(
        "Chest id={} opened: template={}, rewardId={}, {} bundle(s) dropped to scene",
        entity_id,
        template_id,
        reward_id,
        bundles.len()
    );
}

async fn mark_chest_collected(
    ctx: &mut NetContext<'_>,
    entity_id: u64,
    scene_name: &str,
    client_props: &std::collections::HashMap<String, DynamicParameter>,
) {
    let collected = bool_param_true();
    ctx.player
        .scene
        .update_interactive_property(entity_id, "is_collected", collected.clone());

    let mut updated_props = std::collections::HashMap::new();
    updated_props.insert("is_collected".to_string(), collected);

    let filtered = filter_writable_properties(client_props);
    for (key, value) in filtered {
        updated_props.insert(key, value);
    }

    let msg = ScSceneUpdateInteractiveProperty {
        scene_name: scene_name.to_string(),
        id: entity_id,
        properties: updated_props,
        client_operate: false,
    };

    let _ = ctx.notify(msg).await;
}

#[allow(dead_code)]
fn grant_chest_rewards(ctx: &mut NetContext<'_>, bundles: &[(String, i64)]) -> Vec<RewardItem> {
    let own_time = common::time::now_ms() as i64;
    let mut out = Vec::with_capacity(bundles.len());

    for (id, count) in bundles {
        let count = *count;
        if count <= 0 {
            continue;
        }

        let Some(cfg) = ctx.assets.items.get(id) else {
            warn!(
                "Chest reward references unknown item id `{}` (count={}) - skipping",
                id, count
            );
            continue;
        };

        match &cfg.kind {
            ItemKind::Weapon => {
                for _ in 0..count {
                    ctx.player
                        .char_bag
                        .item_manager
                        .weapons
                        .add_weapon(id.clone(), own_time);
                }
            }
            ItemKind::WeaponGem { craft_slot } => {
                let slot = *craft_slot;
                for _ in 0..count {
                    ctx.player
                        .char_bag
                        .item_manager
                        .gems
                        .add_gem(id.clone(), slot, own_time);
                }
            }
            ItemKind::Equip { slot } => {
                let slot = *slot;
                let attrs: Vec<perlica_proto::EquipAttr> = ctx
                    .assets
                    .equipment
                    .get_equip(id)
                    .map(|e| {
                        e.attr_modifiers
                            .iter()
                            .map(|a| {
                                let attr: perlica_proto::EquipAttr =
                                    perlica_logic::item::AttrList(a).into();
                                attr
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                for _ in 0..count {
                    ctx.player.char_bag.item_manager.equips.add_equip(
                        id.clone(),
                        slot,
                        attrs.clone(),
                        own_time,
                    );
                }
            }
            ItemKind::SpecialItem | ItemKind::MissionItem | ItemKind::Factory => {
                let depot = match cfg.kind {
                    ItemKind::SpecialItem => ItemDepotType::SpecialItem,
                    ItemKind::MissionItem => ItemDepotType::MissionItem,
                    ItemKind::Factory => ItemDepotType::Factory,
                    _ => unreachable!(),
                };
                let count_u32 = count.clamp(0, i64::from(u32::MAX)) as u32;
                if let Err(e) = ctx
                    .player
                    .char_bag
                    .item_manager
                    .add_stackable(depot, id, count_u32)
                {
                    warn!(
                        "Failed to add stackable reward {} * {} to {:?}: {:?}",
                        count, id, depot, e
                    );
                    continue;
                }
            }
            ItemKind::Unknown { raw_tab_type } => {
                warn!(
                    "Chest reward item `{}` has unknown depot type {} - skipping",
                    id, raw_tab_type
                );
                continue;
            }
        }

        let _ = CraftShowingType::None;

        out.push(RewardItem {
            id: id.clone(),
            count,
            inst: None,
        });
        debug!("Chest granted {} * {}", count, id);
    }

    out
}

pub async fn on_cs_scene_set_safe_zone(
    _ctx: &mut NetContext<'_>,
    req: CsSceneSetSafeZone,
) -> ScSceneSetSafeZone {
    debug!("Set safe zone: in_zone={}, id={}", req.in_zone, req.id);

    ScSceneSetSafeZone {
        in_zone: req.in_zone,
        id: req.id,
    }
}
