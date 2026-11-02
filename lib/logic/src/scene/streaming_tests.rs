use super::*;
use crate::interest::{INTERACTIVE_SPAWN_BUDGET_PER_TICK, NPC_CONCURRENT_CAP};
use std::sync::OnceLock;

fn assets() -> &'static BeyondAssets {
    static ASSETS: OnceLock<BeyondAssets> = OnceLock::new();
    ASSETS.get_or_init(|| {
        BeyondAssets::load(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets"))
            .expect("load assets")
    })
}

fn char_bag() -> CharBag {
    CharBag::new(assets(), &[]).expect("char bag")
}

fn load(scene: &str) -> (SceneManager, EntityManager) {
    let mut manager = SceneManager::new();
    let mut entities = EntityManager::new();
    manager.reset_for_scene(scene, assets(), &mut entities);
    manager.finish_scene_load(
        &char_bag(),
        &MovementManager::default(),
        assets(),
        &mut entities,
        1000,
    );
    (manager, entities)
}

fn stream(
    manager: &mut SceneManager,
    entities: &mut EntityManager,
    pos: (f32, f32, f32),
    start: u64,
) -> HashSet<u64> {
    let mut spawned = HashSet::new();
    for tick in 0..100 {
        let (enter, _) =
            manager.update_visible_entities(pos, start + tick * 50, assets(), entities);
        if let Some(detail) = enter.and_then(|msg| msg.detail) {
            assert!(detail.interactive_list.len() <= INTERACTIVE_SPAWN_BUDGET_PER_TICK);
            for interactive in detail.interactive_list {
                assert!(
                    spawned.insert(interactive.origin_id),
                    "duplicate spawn in one visit"
                );
            }
        }
    }
    spawned
}

#[test]
fn dense_interactives_all_spawn_and_return_after_leaving_view() {
    let (mut manager, mut entities) = load("map01_lv001");
    let pos = (212.89659, 115.85449, 312.49387);
    let expected: HashSet<_> = assets()
        .level_data
        .interactives("map01_lv001")
        .iter()
        .filter(|i| !is_always_resident_interactive(&i.base.template_id, i.base.entity_type))
        .filter(|i| i.base.position.distance_sq_to(&pos) <= 80.0 * 80.0)
        .map(|i| i.base.level_logic_id)
        .collect();
    assert!(expected.len() > 80, "fixture must exceed the old cap");
    assert_eq!(stream(&mut manager, &mut entities, pos, 1000), expected);
    stream(
        &mut manager,
        &mut entities,
        (10_000.0, 0.0, 10_000.0),
        10_000,
    );
    assert!(expected.iter().all(|&id| !entities.contains(id)));
    assert_eq!(stream(&mut manager, &mut entities, pos, 20_000), expected);
}

#[test]
fn destroyed_mud_and_linked_patch_stay_gone_through_reload_and_transition() {
    let (mut manager, mut entities) = load("map01_lv002");
    let core = assets()
        .level_data
        .interactives("map01_lv002")
        .iter()
        .find(|i| i.base.level_logic_id == 1065)
        .expect("mud core");
    let pos = core.base.position.position();
    let initial = stream(&mut manager, &mut entities, pos, 1000);
    assert!(initial.contains(&1063) && initial.contains(&1065));
    let destroyed = manager.destroy_interactive(1065, assets());
    assert_eq!(destroyed, vec![1065, 1063]);
    for id in destroyed {
        entities.remove(id);
    }
    stream(&mut manager, &mut entities, pos, 70_000);
    assert!(!entities.contains(1063) && !entities.contains(1065));

    manager.reset_for_scene("map01_lv001", assets(), &mut entities);
    assert!(!manager.progress.is_destroyed("map01_lv001", 1065));
    manager.reset_for_scene("map01_lv002", assets(), &mut entities);
    manager.finish_scene_load(
        &char_bag(),
        &MovementManager::default(),
        assets(),
        &mut entities,
        80_000,
    );
    stream(&mut manager, &mut entities, pos, 80_000);
    assert!(!entities.contains(1063) && !entities.contains(1065));
}

#[test]
fn scene_reload_resends_streamed_entities_and_preserves_properties() {
    let (mut manager, mut entities) = load("map01_lv001");
    let pos = (212.89659, 115.85449, 312.49387);
    let expected = stream(&mut manager, &mut entities, pos, 1000);
    let id = *expected.iter().next().unwrap();
    let property = DynamicParameter {
        value_bool_list: vec![true],
        ..Default::default()
    };
    manager.update_interactive_property(id, "is_collected", property.clone());
    manager.finish_scene_load(
        &char_bag(),
        &MovementManager::default(),
        assets(),
        &mut entities,
        10_000,
    );
    assert_eq!(stream(&mut manager, &mut entities, pos, 10_000), expected);
    assert_eq!(
        manager.get_interactive_properties(id).unwrap()["is_collected"],
        property
    );
}

#[test]
fn transition_clears_interest_before_any_new_scene_ticks() {
    let (mut manager, mut entities) = load("map01_lv001");
    stream(
        &mut manager,
        &mut entities,
        (212.89659, 115.85449, 312.49387),
        1000,
    );
    assert!(manager.interest.ghosted_count() > 80);
    manager.reset_for_scene("map01_lv002", assets(), &mut entities);
    assert_eq!(manager.interest.ghosted_count(), 0);
    assert!(manager.scene_cache.is_none());
    assert_eq!(entities.len(), 0);
    assert_eq!(manager.loading_state, SceneLoadingState::Loading);
    assert_eq!(
        manager.update_visible_entities((292.9, 33.0, 142.9), 10_000, assets(), &mut entities),
        (None, None)
    );
}

#[test]
fn capped_npc_scan_still_refreshes_existing_entries() {
    let (mut manager, mut entities) = load("map01_lv001");
    let npcs = assets().level_data.npcs("map01_lv001");
    let (pos, target) = npcs
        .iter()
        .find_map(|first| {
            let pos = first.base.position.position();
            npcs.iter()
                .find(|other| {
                    let distance = other.base.position.distance_sq_to(&pos);
                    distance > 1.0 && distance < 40.0 * 40.0
                })
                .map(|other| (pos, other))
        })
        .expect("nearby npc pair");
    for offset in 0..NPC_CONCURRENT_CAP {
        manager.interest.ghost_in(
            1_000_000 + offset as u64,
            ReplicationZone::Combat,
            StreamBucket::Npc,
            1000,
        );
    }
    let id = target.base.level_logic_id;
    manager
        .interest
        .ghost_in(id, ReplicationZone::Combat, StreamBucket::Npc, 1000);
    manager.interest.refill_spawn_credits(2000);
    let due = manager.interest.zones_due(2000);
    let mut out = Vec::new();
    manager.stream_npcs(pos, 2000, due, assets(), &mut entities, &mut out);
    assert!(out.is_empty());
    assert_eq!(
        manager.interest.zone_of(id),
        Some(ReplicationZone::Immediate)
    );
}

#[test]
fn dynamic_ids_do_not_reuse_level_ids_or_kill_the_source_spawn() {
    let (mut manager, mut entities) = load("map01_lv001");
    manager.reserve_level_entity_ids(assets(), &mut entities);
    let id = entities.next_monster_id();
    assert!(
        assets()
            .level_data
            .interactives("map01_lv001")
            .iter()
            .all(|i| i.base.level_logic_id < id)
    );
    let source_id = assets().level_data.enemies("map01_lv001")[0]
        .base
        .level_logic_id;
    manager.interest.ghost_in(
        source_id,
        ReplicationZone::Immediate,
        StreamBucket::Enemy,
        1000,
    );
    manager.on_entity_killed(id, source_id, 2000);
    assert!(!manager.dead_entities.contains_key(&source_id));
    assert!(manager.interest.is_ghosted_in(source_id));
}
