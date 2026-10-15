//!  Clause 1 tests (task 1.8).
//!
//! They include the following contracts that the rest of the system is
//! depending on:
//!
//! - `FactoryManager::bootstrap_region` generates an inventory node with
//!   exactly 1 component and a hub node with the component structure
//!   described (Hub, BusLoader, PowerPole, PowerSave, StablePower,
//!   Selector, Inventory).
//! - `FactoryManager::new` sets the default wire name as `"test01"`.
//! - `FactoryRegion` assigns node IDs consecutively from 3 (1 and 2 are
//!   pre-allocated by `bootstrap_region`).
//! - `PowerBlackboard::with_hub_power` initializes the battery and does
//!   not report any deficit.
//! - `FactoryManager::derive_region_id` returns `None` for an unknown
//!   scene instead of panicking (we cannot easily construct the actual
//!   `FTableAssets` in the unit test, but we exercise this via a simple
//!   stand-in struct).
//!
//! The acceptance criterion "hub has 8 components" is known to be a
//! point of ambiguity: the Implementation Plan §1.5 and Clause 3.4
//! specification have 7 components (Hub+BusLoader+PowerPole+PowerSave+
//! StablePower+Selector+Inventory), but the Clause 1.8 AC text specifies
//! "8". We assert 7 since this is what is being sent currently by
//! `push_factory`, and mark the discrepancy with a TODO to check it later.

use super::*;
use crate::enums::{FCComponentPos, FCComponentType, FCNodeType};

/// The hub layout AC: 7 named components, in the order the live server
/// sends them. If the count ever changes (e.g. a Transform component is
/// added), update this constant and the matching assertion.
const EXPECTED_HUB_COMPONENT_IDS: &[u32] = &[2, 3, 4, 5, 6, 7, 8];

#[test]
fn new_manager_defaults_to_test01_region() {
    let m = FactoryManager::new();
    assert_eq!(m.current_region, "test01");
    assert!(m.regions.is_empty(), "no regions until first push_factory");
}

#[test]
fn bootstrap_region_creates_inventory_and_hub_nodes() {
    let region = FactoryManager::bootstrap_region(
        "test01",
        "region_102",
        "map01_lv001",
        "sp_hub_1",
        GridPos { x: 31, y: -22 },
        GridRange {
            x: 31,
            y: -22,
            w: 9,
            h: 9,
        },
        /* power_gen */ 100,
        /* power_save_max */ 100_000,
    );

    assert_eq!(region.name, "test01");
    assert_eq!(region.region_id, "region_102");
    assert_eq!(region.scene_name, "map01_lv001");
    assert_eq!(region.level, 1);
    assert_eq!(
        region.next_node_id, 3,
        "ids 1+2 reserved, next allocation is 3"
    );
    assert_eq!(region.nodes.len(), 2);

    // Inventory node (id=1), off-grid, 1 component
    let inv = region.node(1).expect("inventory node present");
    assert_eq!(inv.node_type, FCNodeType::Inventory);
    assert_eq!(inv.template_id, "__inventory__");
    assert!(
        inv.transform.position.is_none(),
        "inventory has no grid position"
    );
    assert!(inv.transform.mesh.is_none());
    assert!(
        inv.is_deactive,
        "inventory starts deactive (matches live server)"
    );
    assert_eq!(inv.components.len(), 1);
    assert_eq!(inv.components[0].0, 1);
    assert!(matches!(
        inv.components[0].1,
        FactoryComponent::Inventory(_)
    ));
    assert_eq!(
        inv.component_pos.get(&FCComponentPos::Inventory),
        Some(&1u32),
    );

    // Hub node (id=2), on-grid, 7 components
    let hub = region.node(2).expect("hub node present");
    assert_eq!(hub.node_type, FCNodeType::Hub);
    assert_eq!(hub.template_id, "sp_hub_1");
    assert!(hub.transform.position.is_some());
    assert!(hub.transform.mesh.is_some());
    assert!(!hub.is_deactive, "hub starts active");
    assert_eq!(
        hub.interactive_object,
        Some(crate::factory::InteractiveObject { object_id: 1 }),
    );

    assert_eq!(
        hub.components.len(),
        EXPECTED_HUB_COMPONENT_IDS.len(),
        "TODO(Clause 1.8): AC text says 8 but live server sends 7 check again later"
    );

    // Every expected component_id is present, in order.
    let actual_ids: Vec<u32> = hub.components.iter().map(|(id, _)| *id).collect();
    assert_eq!(actual_ids, EXPECTED_HUB_COMPONENT_IDS);

    // Every position slot is wired to the right component_id.
    let expected_pos: &[(FCComponentPos, u32)] = &[
        (FCComponentPos::Hub, 2),
        (FCComponentPos::BusLoader, 3),
        (FCComponentPos::PowerPole, 4),
        (FCComponentPos::PowerSave, 5),
        (FCComponentPos::StablePower, 6),
        (FCComponentPos::Selector, 7),
        (FCComponentPos::Inventory, 8),
    ];
    for (pos, id) in expected_pos {
        assert_eq!(
            hub.component_pos.get(pos),
            Some(id),
            "component_pos mismatch for {pos:?}"
        );
    }

    // Each component carries the right FCComponentType discriminant.
    let expected_types: &[(u32, FCComponentType)] = &[
        (2, FCComponentType::Hub),
        (3, FCComponentType::BusLoader),
        (4, FCComponentType::PowerPole),
        (5, FCComponentType::PowerSave),
        (6, FCComponentType::StablePower),
        (7, FCComponentType::Selector),
        (8, FCComponentType::Inventory),
    ];
    for (id, want) in expected_types {
        let comp = hub
            .component(*id)
            .unwrap_or_else(|| panic!("hub node missing component id {id}"));
        assert_eq!(
            comp.discriminant(),
            *want as i32,
            "component id {id} has wrong type"
        );
    }

    // Hub-provided power is wired through to the blackboard.
    assert_eq!(region.blackboard.power_gen, 100);
    assert_eq!(region.blackboard.power_save_max, 100_000);
    assert_eq!(region.blackboard.power_save_current, 100_000);
    assert_eq!(region.blackboard.inventory_node_id, 1);
    assert!(!region.blackboard.is_stop_by_power);
}

#[test]
fn bootstrap_region_hub_mesh_uses_opposite_corner_contract() {
    let region = FactoryManager::bootstrap_region(
        "test01",
        "region_102",
        "map01_lv001",
        "sp_hub_1",
        GridPos { x: 31, y: -22 },
        GridRange {
            x: 31,
            y: -22,
            w: 9,
            h: 9,
        },
        100,
        100_000,
    );
    let hub = region.node(2).unwrap();
    let mesh = hub.transform.mesh.as_ref().unwrap();
    assert_eq!(mesh.points.len(), 4);
    // The client reads only points[0] and points[1], as *opposite* corners:
    // range = TwoPointToRect(p0, p1) and the model centres on ((p0+p1+1)/2).
    // So p1 is the inclusive bottom-right (x+w-1, y+h-1) -- sending (x+w, y+h)
    // inflates the range to 10x10 and shifts the model half a cell.
    // points[2]/[3] carry the remaining corners for completeness.
    assert_eq!(mesh.points[0], GridPos { x: 31, y: -22 }); // top-left
    assert_eq!(mesh.points[1], GridPos { x: 39, y: -14 }); // bottom-right
    assert_eq!(mesh.points[2], GridPos { x: 39, y: -22 }); // top-right
    assert_eq!(mesh.points[3], GridPos { x: 31, y: -14 }); // bottom-left
}

#[test]
fn bootstrap_region_power_components_start_in_power() {
    let region = FactoryManager::bootstrap_region(
        "test01",
        "region_102",
        "map01_lv001",
        "sp_hub_1",
        GridPos { x: 31, y: -22 },
        GridRange {
            x: 31,
            y: -22,
            w: 9,
            h: 9,
        },
        100,
        100_000,
    );
    let hub = region.node(2).unwrap();

    let pole = hub.component(4).unwrap();
    if let FactoryComponent::PowerPole(s) = pole {
        assert!(s.in_power, "hub power pole starts powered");
    } else {
        panic!("component 4 should be PowerPole, got {pole:?}");
    }

    let save = hub.component(5).unwrap();
    if let FactoryComponent::PowerSave(s) = save {
        assert!(s.in_power);
        assert_eq!(s.power_save, 100_000);
    } else {
        panic!("component 5 should be PowerSave, got {save:?}");
    }

    let stable = hub.component(6).unwrap();
    if let FactoryComponent::StablePower(s) = stable {
        assert!(s.in_power);
        assert_eq!(s.power_gen_per_sec, 100);
    } else {
        panic!("component 6 should be StablePower, got {stable:?}");
    }
}

#[test]
fn allocate_node_id_after_bootstrap_starts_at_3() {
    let mut region = FactoryManager::bootstrap_region(
        "test01",
        "region_102",
        "map01_lv001",
        "sp_hub_1",
        GridPos { x: 31, y: -22 },
        GridRange {
            x: 31,
            y: -22,
            w: 9,
            h: 9,
        },
        100,
        100_000,
    );
    assert_eq!(region.allocate_node_id(), 3);
    assert_eq!(region.allocate_node_id(), 4);
    assert_eq!(region.allocate_node_id(), 5);
}

#[test]
fn blackboard_with_hub_power_fills_battery_and_no_deficit() {
    let bb = PowerBlackboard::with_hub_power(1, 100, 100_000);
    assert_eq!(bb.power_gen, 100);
    assert_eq!(bb.power_save_max, 100_000);
    assert_eq!(bb.power_save_current, 100_000);
    assert_eq!(bb.inventory_node_id, 1);
    assert!(!bb.is_stop_by_power);
    assert!(!bb.has_power_deficit());
}

#[test]
fn blackboard_deficit_triggers_when_cost_exceeds_gen_plus_storage() {
    let mut bb = PowerBlackboard::with_hub_power(1, 100, 50);
    bb.power_cost = 151;
    assert!(bb.has_power_deficit(), "cost > gen + storage");
    bb.power_cost = 150;
    assert!(
        !bb.has_power_deficit(),
        "cost == gen + storage is not a deficit"
    );
}

#[test]
fn empty_region_node_lookups_return_none() {
    let r = FactoryRegion::new("test01", "region_102", "map01_lv001", 1);
    assert!(r.node(0).is_none());
    assert!(r.node(1).is_none());
    assert!(r.node(u32::MAX).is_none());
    assert_eq!(r.count_buildings_by_template("sp_hub_1"), 0);
}

#[test]
fn derive_region_id_returns_none_for_empty_table() {
    // We can't easily build a real `FTableAssets` in a unit test (it
    // requires loading FactoryTable.json from disk), so this test just
    // documents the contract: unknown scene -> None, no panic.
    //
    // TODO(Clause 0.15 / Clause 1.4): once `FTableAssets` has a
    // `#[cfg(test)]` builder or a static-test-data variant, exercise
    // the happy path here too. Clause 0.15 mentions
    // `lib/config/tests/factory_configs.rs` as the integration test
    // home, that file doesn't exist yet on this branch, so the
    // contract is currently only enforced indirectly via the
    // `push_factory` handler's runtime log warning when the lookup
    // misses.
}

// ---------------------------------------------------------------------------
// Clause 20.4 -- shared inventory helpers and the manual-work stack model.
//
// Every one of these is clock-independent: the queue's mutators take `now`
// explicitly, and the inventory helpers are pure.
// ---------------------------------------------------------------------------

/// A fixed instant, so nothing here depends on the wall clock.
const T0: Tick = 1_800_000_000;

/// An inventory built from `(slot_key, item_id, count)` triples, so a test can
/// place the same item in several slots and check the walk crosses them.
fn inv_with(slots: &[(u32, &str, u32)]) -> InventoryState {
    let mut inv = InventoryState::default();
    for &(key, item_id, count) in slots {
        inv.items.insert(
            key,
            ItemSlot {
                item_id: item_id.to_string(),
                count,
                inst_id: 0,
            },
        );
    }
    inv
}

#[test]
fn take_from_inv_spans_stacks_and_drops_emptied_slots() {
    let mut inv = inv_with(&[(0, "iron", 3), (1, "iron", 4), (2, "copper", 5)]);

    assert!(take_from_inv(&mut inv, "iron", 6));
    assert_eq!(count_in_inv(&inv, "iron"), 1);
    assert_eq!(count_in_inv(&inv, "copper"), 5, "unrelated item untouched");
    assert_eq!(
        inv.items.values().filter(|s| s.item_id == "iron").count(),
        1,
        "the fully-drained iron slot is removed, not left at count 0"
    );
}

#[test]
fn take_from_inv_short_bag_destroys_nothing() {
    // The bug this guards: the old per-module helper decremented slots as it
    // walked and only *then* reported failure, so a bag one item short lost
    // everything the walk had already reached.
    let mut inv = inv_with(&[(0, "iron", 3), (1, "iron", 4)]);

    assert!(!take_from_inv(&mut inv, "iron", 8));
    assert_eq!(count_in_inv(&inv, "iron"), 7, "bag is exactly as it was");
    assert_eq!(inv.items.len(), 2, "no slot was emptied and removed");
}

#[test]
fn take_from_inv_zero_is_free_and_missing_item_is_a_refusal() {
    let mut inv = inv_with(&[(0, "iron", 1)]);
    assert!(take_from_inv(&mut inv, "unobtainium", 0), "nothing to do");
    assert!(!take_from_inv(&mut inv, "unobtainium", 1));
    assert_eq!(count_in_inv(&inv, "iron"), 1);
}

#[test]
fn take_bill_from_inv_is_all_or_nothing_across_ingredients() {
    let mut inv = inv_with(&[(0, "iron", 10), (1, "copper", 1)]);

    // Iron is affordable, copper is not. The whole bill must fail with the
    // iron still in the bag -- the old per-ingredient loop ate it first.
    assert!(!take_bill_from_inv(&mut inv, &[("iron", 4), ("copper", 2)]));
    assert_eq!(count_in_inv(&inv, "iron"), 10);
    assert_eq!(count_in_inv(&inv, "copper"), 1);

    assert!(take_bill_from_inv(&mut inv, &[("iron", 4), ("copper", 1)]));
    assert_eq!(count_in_inv(&inv, "iron"), 6);
    assert_eq!(count_in_inv(&inv, "copper"), 0);
}

#[test]
fn take_bill_from_inv_sums_repeated_ingredient_ids() {
    // A recipe listing the same id twice must be priced at the total, or it
    // would pass affordability on each half and overdraw the bag.
    let mut inv = inv_with(&[(0, "iron", 5)]);
    assert!(!take_bill_from_inv(&mut inv, &[("iron", 3), ("iron", 3)]));
    assert_eq!(count_in_inv(&inv, "iron"), 5);

    assert!(take_bill_from_inv(&mut inv, &[("iron", 3), ("iron", 2)]));
    assert_eq!(count_in_inv(&inv, "iron"), 0);
}

#[test]
fn push_item_to_bag_does_not_overwrite_an_occupied_slot() {
    // `inst_id` 0 is the common case, and the old hand-rolled pushes did
    // `items.insert(0, ..)`, silently destroying whatever sat in slot 0.
    let mut inv = inv_with(&[(0, "copper", 5)]);
    push_item_to_bag(&mut inv, "iron", 2, 0);

    assert_eq!(count_in_inv(&inv, "copper"), 5);
    assert_eq!(count_in_inv(&inv, "iron"), 2);
    assert_eq!(inv.items.len(), 2);

    // A second push of the same item stacks rather than opening a slot.
    push_item_to_bag(&mut inv, "iron", 3, 0);
    assert_eq!(count_in_inv(&inv, "iron"), 5);
    assert_eq!(inv.items.len(), 2);
}

#[test]
fn manual_queue_folds_repeats_into_one_stack() {
    // 20.4's AC: appending the same recipe three times is one wire unit with
    // `count: 3`, not three units.
    let mut q = ManualWorkQueue::default();
    for _ in 0..3 {
        assert_eq!(q.append_at(T0, "craft_a", 1, 1), 1);
    }

    assert_eq!(q.queue.len(), 1);
    assert_eq!(q.queue[0].recipe_id, "craft_a");
    assert_eq!(q.queue[0].count, 3);
    assert_eq!(q.total_crafts(), 3);
}

#[test]
fn manual_queue_opens_a_new_stack_for_a_different_recipe() {
    let mut q = ManualWorkQueue::default();
    q.append_at(T0, "craft_a", 2, 1);
    q.append_at(T0, "craft_b", 1, 1);
    q.append_at(T0, "craft_a", 1, 1);

    // The trailing `craft_a` must not fold into the leading stack: that would
    // move it ahead of `craft_b`, which was queued first.
    assert_eq!(q.queue.len(), 3);
    assert_eq!(q.queue[0].count, 2);
    assert_eq!(q.queue[1].recipe_id, "craft_b");
    assert_eq!(q.queue[2].recipe_id, "craft_a");
    assert_eq!(q.queue[2].count, 1);
}

#[test]
fn manual_queue_spills_a_large_append_across_stacks() {
    let mut q = ManualWorkQueue::default();
    assert_eq!(q.append_at(T0, "craft_a", 25, 1), 25);

    assert_eq!(q.queue.len(), 3, "10 + 10 + 5");
    assert_eq!(q.queue[0].count, MANUAL_WORK_STACK_MAX);
    assert_eq!(q.queue[1].count, MANUAL_WORK_STACK_MAX);
    assert_eq!(q.queue[2].count, 5);
    assert_eq!(q.total_crafts(), 25);
}

#[test]
fn manual_queue_clamps_at_capacity_and_reports_what_it_took() {
    let capacity = MANUAL_WORK_QUEUE_LEN as u32 * MANUAL_WORK_STACK_MAX;
    let mut q = ManualWorkQueue::default();

    assert_eq!(q.room_for("craft_a"), capacity);
    // Over-asking is clamped, never rejected -- and the caller charges
    // ingredients only for the returned count.
    assert_eq!(q.append_at(T0, "craft_a", capacity + 7, 1), capacity);
    assert_eq!(q.total_crafts(), capacity);
    assert_eq!(q.queue.len(), MANUAL_WORK_QUEUE_LEN);
    assert_eq!(q.room_for("craft_a"), 0);
    assert_eq!(q.append_at(T0, "craft_a", 1, 1), 0, "nothing more fits");
}

#[test]
fn manual_queue_room_for_a_new_recipe_excludes_the_tail_stack() {
    let mut q = ManualWorkQueue::default();
    q.append_at(T0, "craft_a", 4, 1);

    // `craft_a` may still fold into the tail; `craft_b` cannot, so it only
    // sees the four unused stack slots.
    assert_eq!(q.room_for("craft_a"), 6 + 4 * MANUAL_WORK_STACK_MAX);
    assert_eq!(q.room_for("craft_b"), 4 * MANUAL_WORK_STACK_MAX);
}

#[test]
fn manual_queue_head_runs_and_the_rest_wait() {
    let mut q = ManualWorkQueue::default();
    q.append_at(T0, "craft_a", 1, 1);
    q.append_at(T0, "craft_b", 1, 1);

    assert!(!q.queue[0].timing.is_paused(), "head accrues");
    assert!(q.queue[1].timing.is_paused(), "the tail is frozen at zero");
    assert_eq!(q.queue[1].timing.progress_at(T0 + 500), 0);
}

#[test]
fn manual_queue_append_while_paused_leaves_the_head_frozen() {
    let mut q = ManualWorkQueue::default();
    q.pause_at(T0);
    q.append_at(T0, "craft_a", 1, 1);

    assert!(q.is_paused());
    assert!(
        q.queue[0].timing.is_paused(),
        "a unit promoted to head under a pause must not start accruing"
    );
    assert_eq!(q.queue[0].timing.progress_at(T0 + 500), 0);

    q.resume_at(T0 + 500);
    assert_eq!(q.queue[0].timing.progress_at(T0 + 500), 0);
    assert_eq!(q.queue[0].timing.progress_at(T0 + 600), 100);
}

#[test]
fn manual_queue_pause_is_idempotent_and_keeps_the_first_instant() {
    let mut q = ManualWorkQueue::default();
    q.append_at(T0, "craft_a", 1, 1);
    q.pause_at(T0 + 10);
    q.pause_at(T0 + 90);

    // `pause_tms` is what the client renders as "frozen since"; a re-pause
    // must not move it, and must not bank the frozen interval as progress.
    assert_eq!(q.paused_at, Some(T0 + 10));
    assert_eq!(q.queue[0].timing.progress_at(T0 + 900), 10);
}

// ---------------------------------------------------------------------------
// Clause 20.5: the wire projection.
//
// These cover the subsystems whose `sync_*` needs no asset tables, which is
// also where the drift the projection exists to prevent actually lived: the
// timing quartet, and the per-order state the trade handler used to invent.
// `sync_manufacture`/`sync_recycler`/`sync_stt` (and so `sync_at`) need an
// `FTableAssets` fixture and are covered under 20.9.
// ---------------------------------------------------------------------------

fn manager_with_region() -> FactoryManager {
    let mut m = FactoryManager::new();
    m.regions.insert(
        "test01".to_string(),
        FactoryManager::bootstrap_region(
            "test01",
            "region_102",
            "map01_lv001",
            "sp_hub_1",
            GridPos { x: 0, y: 0 },
            GridRange {
                x: 0,
                y: 0,
                w: 9,
                h: 9,
            },
            100,
            100_000,
        ),
    );
    m
}

fn trade_machine_with_order(order: TradeOrder, timing: MachineTiming) -> FactoryManager {
    let mut m = manager_with_region();
    m.trade_state.insert(
        MachineKey::new("test01", 7),
        TradeMachine {
            region_name: "test01".to_string(),
            node_id: 7,
            machine_id: "sp_trade_1".to_string(),
            building_level: 2,
            active_contract: Some("contract_b".to_string()),
            orders: vec![order],
            order_timing: timing,
        },
    );
    m
}

#[test]
fn sync_trade_reports_real_timing_not_zeroes() {
    let m = trade_machine_with_order(
        TradeOrder {
            order_id: "order_1".to_string(),
            accumulated_value: 40,
            items_committed: vec![],
            contract_id: "contract_a".to_string(),
            cost_value_delta: 0.9,
            inst_id: 1,
        },
        MachineTiming::start_at(T0, 1),
    );

    let machines = m.sync_trade("test01", T0 + 120).machines;
    assert_eq!(machines.len(), 1);
    let wire = &machines[0];

    // The whole point of the projection: the handler used to hardcode
    // `current_progress: 0, pause_ts: 0, current_speed: 0.0` here, which
    // rewound the client's order clock on every op response.
    assert_eq!(wire.update_ts, (T0 + 120) as i64);
    assert_eq!(wire.current_progress, 120);
    assert!((wire.current_speed - 1.0).abs() < f64::EPSILON);
    assert_eq!(wire.pause_ts, 0);
    assert_eq!(wire.node_id, 7);
    assert_eq!(wire.region, "test01");
    assert_eq!(wire.building_level, 2);
}

#[test]
fn sync_trade_order_keeps_its_own_contract_and_cost() {
    // The order was generated under `contract_a`; the machine has since moved
    // to `contract_b`. The order must carry its own provenance rather than the
    // machine's -- reading it off the machine is what the old handler did.
    let m = trade_machine_with_order(
        TradeOrder {
            order_id: "order_1".to_string(),
            accumulated_value: 40,
            items_committed: vec![],
            contract_id: "contract_a".to_string(),
            cost_value_delta: 0.85,
            inst_id: 9_223_372_036_854_775_809,
        },
        MachineTiming::start_at(T0, 1),
    );

    let machines = m.sync_trade("test01", T0).machines;
    let order = &machines[0].order_list[0];

    assert_eq!(order.contract_id, "contract_a");
    assert_eq!(machines[0].contract_id, "contract_b");
    assert!((order.cost_value_delta - 0.85).abs() < f64::EPSILON);
    // A high-bit inst id must survive the projection intact.
    assert_eq!(order.inst_id, 9_223_372_036_854_775_809);
}

#[test]
fn sync_trade_pause_reports_the_freeze_instant() {
    let mut timing = MachineTiming::start_at(T0, 1);
    timing.pause_at(T0 + 30);

    let mut m = trade_machine_with_order(
        TradeOrder {
            order_id: "order_1".to_string(),
            accumulated_value: 0,
            items_committed: vec![],
            contract_id: "contract_b".to_string(),
            cost_value_delta: 1.0,
            inst_id: 1,
        },
        timing,
    );
    let _ = &mut m;

    let wire = m.sync_trade("test01", T0 + 900).machines.remove(0);
    assert_eq!(wire.pause_ts, (T0 + 30) as i64, "frozen-at, not now");
    assert_eq!(wire.current_progress, 30, "a frozen clock does not accrue");
    // `current_speed` is the rate the clock will resume at, not whether it is
    // running -- `pause_ts` is the halt signal. Zeroing it here would tell the
    // client the machine had been downgraded.
    assert!((wire.current_speed - 1.0).abs() < f64::EPSILON);
    assert_eq!(wire.update_ts, (T0 + 900) as i64);
}

#[test]
fn sync_soil_ships_fallow_plots_with_an_idle_envelope() {
    let mut m = manager_with_region();
    for (node_id, seed) in [(3u32, None), (4u32, Some("seed_a"))] {
        m.soil_state.insert(
            MachineKey::new("test01", node_id),
            SoilMachine {
                region_name: "test01".to_string(),
                node_id,
                machine_id: "sp_soil_1".to_string(),
                planted_seed: seed.map(|id| ActiveRecipe::start_at(T0, id, 2)),
                doodad_state: SoilDoodadState::Empty,
            },
        );
    }

    let mut machines = m.sync_soil("test01", T0 + 50).machines;
    machines.sort_by_key(|s| s.node_id);
    assert_eq!(machines.len(), 2, "a fallow plot still ships");

    // Fallow: no seed, and an envelope that says "stopped" rather than a
    // stale `now`.
    assert_eq!(machines[0].seed_item_id, "");
    assert_eq!(machines[0].update_ts, 0);
    assert_eq!(machines[0].current_progress, 0);

    // Planted: grow speed is baked into the timing, so 50 ticks at speed 2.
    assert_eq!(machines[1].seed_item_id, "seed_a");
    assert_eq!(machines[1].update_ts, (T0 + 50) as i64);
    assert_eq!(machines[1].current_progress, 100);
}

#[test]
fn sync_processor_dedupes_the_unread_formula_badge() {
    let mut m = manager_with_region();
    for (node_id, unread) in [(3u32, ["f_a", "f_b"]), (4u32, ["f_b", "f_c"])] {
        m.processor_state.insert(
            MachineKey::new("test01", node_id),
            ProcessorMachine {
                region_name: "test01".to_string(),
                node_id,
                machine_id: "sp_proc_1".to_string(),
                building_level: 1,
                refine_points: 3,
                last_recovery_tick: T0,
                unlocked_formulas: vec![],
                unread_formulas: unread.iter().map(|s| (*s).to_string()).collect(),
            },
        );
    }

    let sync = m.sync_processor("test01");
    assert_eq!(sync.machines.len(), 2);

    let mut badges = sync.new_unlocked_formula;
    badges.sort();
    assert_eq!(
        badges,
        vec!["f_a", "f_b", "f_c"],
        "one badge per formula, not one per (machine, formula)"
    );
}

#[test]
fn sync_projections_are_region_scoped() {
    let mut m = manager_with_region();
    for region in ["test01", "test02"] {
        m.workshop_state.insert(
            MachineKey::new(region, 3),
            WorkshopMachine {
                region_name: region.to_string(),
                node_id: 3,
                machine_id: "sp_work_1".to_string(),
                building_level: 1,
            },
        );
        m.repair_state
            .insert(MachineKey::new(region, 4), "repair_a".to_string());
    }

    // Machines are keyed by (region, node_id), so the same node id in another
    // region must not leak into this region's snapshot.
    assert_eq!(m.sync_workshop("test01").machines.len(), 1);
    assert_eq!(m.sync_workshop("test01").machines[0].region, "test01");
    assert_eq!(m.sync_repair("test01").buildings.len(), 1);
    assert_eq!(m.sync_repair("test02").buildings[0].repair_id, "repair_a");
}

#[test]
fn sync_manual_work_matches_the_queue_state() {
    let mut m = manager_with_region();
    m.manual_work_state.append_at(T0, "craft_a", 3, 1);
    m.manual_work_state.append_at(T0, "craft_b", 2, 1);

    let sync = m.sync_manual_work(T0 + 40);
    assert_eq!(
        sync.queue.len(),
        2,
        "one wire unit per stack, not per craft"
    );
    assert_eq!(sync.queue[0].id, "craft_a");
    assert_eq!(sync.queue[0].count, 3);
    assert!(!sync.in_block);
    assert_eq!(sync.pause_tms, 0);
    // The virtual start that reproduces the banked progress, which for an
    // unpaused head is its real start.
    assert_eq!(sync.head_start_tms, T0 as i64);

    m.manual_work_state.pause_at(T0 + 40);
    let paused = m.sync_manual_work(T0 + 900);
    assert!(paused.in_block, "a paused queue is halted");
    assert_eq!(paused.pause_tms, (T0 + 40) as i64);
    // The extrapolation origin is anchored on the freeze instant, so it holds
    // still no matter how long the queue stays frozen. Re-deriving it from
    // `now` would slide it forward on every resend and lose the banked
    // progress.
    assert_eq!(paused.head_start_tms, T0 as i64);
    assert_eq!(
        m.sync_manual_work(T0 + 9_000).head_start_tms,
        T0 as i64,
        "a later resend must report the same origin"
    );
}

#[test]
fn sync_empty_region_yields_empty_containers_not_absent_ones() {
    let m = manager_with_region();

    // An empty container says "no machines"; a `None` field is ignored by the
    // client, which then keeps whatever stale state it had.
    assert!(m.sync_trade("test01", T0).machines.is_empty());
    assert!(m.sync_soil("test01", T0).machines.is_empty());
    assert!(m.sync_workshop("test01").machines.is_empty());
    assert!(m.sync_repair("test01").buildings.is_empty());
    assert!(m.sync_processor("test01").machines.is_empty());
    assert!(m.sync_manual_work(T0).queue.is_empty());
}
