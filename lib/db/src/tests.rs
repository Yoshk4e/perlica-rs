//! Schema tests for the embedded migrations.
//!
//! A migration can only really be wrong in two ways: it fails to apply, or its
//! foreign keys do not chain the way the load/write path assumes. Both are
//! cheap to check and expensive to discover in production, so they are checked
//! here against an in-memory database rather than a fixture file.

use crate::saves::MIGRATOR;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;

/// An in-memory pool with the foreign-key setting `PlayerDb::open` uses, minus
/// the file. `max_connections(1)` matters: each connection to `:memory:` gets
/// its own database, so a larger pool would run the migrations on one
/// connection and the assertions against an empty other one.
async fn migrated_pool() -> SqlitePool {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("in-memory options parse")
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("in-memory pool connects");
    MIGRATOR.run(&pool).await.expect("migrations apply");
    pool
}

async fn table_exists(pool: &SqlitePool, name: &str) -> bool {
    let found: Option<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' AND name=?1")
            .bind(name)
            .fetch_optional(pool)
            .await
            .expect("sqlite_master is queryable");
    found.is_some()
}

async fn count(pool: &SqlitePool, table: &str) -> i64 {
    // `table` is a literal from the test bodies below, never user input.
    let (n,): (i64,) = sqlx::query_as(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .expect("count query runs");
    n
}

async fn insert(pool: &SqlitePool, what: &str, sql: &str) {
    sqlx::query(sql)
        .execute(pool)
        .await
        .unwrap_or_else(|e| panic!("{what} insert failed: {e}"));
}

#[tokio::test]
async fn migrations_apply_to_a_fresh_database() {
    let pool = migrated_pool().await;
    // The pre-factory schema still stands.
    assert!(table_exists(&pool, "beyond_players").await);
    assert!(table_exists(&pool, "beyond_wallet").await);
}

#[tokio::test]
async fn factory_migration_creates_every_table() {
    let pool = migrated_pool().await;

    // Every table clause 20.6 declares, listed rather than counted so a rename
    // shows up as the specific table that vanished.
    for table in [
        // topology
        "beyond_factory_regions",
        "beyond_factory_region_production",
        "beyond_factory_nodes",
        "beyond_factory_node_mesh",
        "beyond_factory_node_props",
        "beyond_factory_node_component_pos",
        "beyond_factory_components",
        "beyond_factory_component_slots",
        "beyond_factory_component_ports",
        "beyond_factory_component_scalars",
        "beyond_factory_connections",
        // machines
        "beyond_factory_manufacture",
        "beyond_factory_manufacture_buffer",
        "beyond_factory_processor",
        "beyond_factory_processor_formulas",
        "beyond_factory_recycler",
        "beyond_factory_recycler_materials",
        "beyond_factory_recycler_products",
        "beyond_factory_trade",
        "beyond_factory_trade_orders",
        "beyond_factory_trade_order_items",
        "beyond_factory_workshop",
        "beyond_factory_soil",
        "beyond_factory_repair",
        // player-scoped
        "beyond_factory_manual_work",
        "beyond_factory_manual_work_units",
        "beyond_factory_character_work",
        "beyond_factory_character_work_skills",
        "beyond_factory_quickbars",
        "beyond_factory_quickbar_slots",
        "beyond_factory_stt_nodes",
        "beyond_factory_formulas",
    ] {
        assert!(table_exists(&pool, table).await, "missing table {table}");
    }
}

#[tokio::test]
async fn current_region_lands_on_the_player_row() {
    let pool = migrated_pool().await;
    insert(
        &pool,
        "player",
        "INSERT INTO beyond_players (uid) VALUES ('u1')",
    )
    .await;

    let (region,): (String,) =
        sqlx::query_as("SELECT current_region FROM beyond_players WHERE uid='u1'")
            .fetch_one(&pool)
            .await
            .expect("current_region column exists");
    assert_eq!(region, "test01", "default matches FactoryManager::new");
}

/// The chain a factory load walks: player -> region -> node -> component ->
/// slot. Deleting the player must take the whole subtree with it, or an account
/// deletion leaves orphaned topology behind for the next load to find.
#[tokio::test]
async fn deleting_a_player_cascades_through_the_topology() {
    let pool = migrated_pool().await;

    insert(
        &pool,
        "player",
        "INSERT INTO beyond_players (uid) VALUES ('u1')",
    )
    .await;
    insert(
        &pool,
        "region",
        "INSERT INTO beyond_factory_regions (uid, region, region_id, scene_name)
         VALUES ('u1', 'test01', 'region_102', 'map01_lv001')",
    )
    .await;
    insert(
        &pool,
        "node",
        "INSERT INTO beyond_factory_nodes (uid, region, node_id, node_type, template_id)
         VALUES ('u1', 'test01', 2, 3, 'sp_hub_1')",
    )
    .await;
    insert(
        &pool,
        "component",
        "INSERT INTO beyond_factory_components
            (uid, region, node_id, component_id, ord, component_type)
         VALUES ('u1', 'test01', 2, 8, 6, 2)",
    )
    .await;
    insert(
        &pool,
        "slot",
        "INSERT INTO beyond_factory_component_slots
            (uid, region, node_id, component_id, slot_key, item_id, count, inst_id)
         VALUES ('u1', 'test01', 2, 8, 0, 'item_a', 5, 0)",
    )
    .await;

    assert_eq!(count(&pool, "beyond_factory_component_slots").await, 1);

    sqlx::query("DELETE FROM beyond_players WHERE uid='u1'")
        .execute(&pool)
        .await
        .expect("player delete");

    for table in [
        "beyond_factory_regions",
        "beyond_factory_nodes",
        "beyond_factory_components",
        "beyond_factory_component_slots",
    ] {
        assert_eq!(
            count(&pool, table).await,
            0,
            "{table} still holds rows after the player was deleted"
        );
    }
}

/// Machines hang off the region, not the player, so a region that goes away
/// must not leave machines keyed to a node that no longer exists.
#[tokio::test]
async fn deleting_a_region_cascades_through_machines_and_orders() {
    let pool = migrated_pool().await;

    insert(
        &pool,
        "player",
        "INSERT INTO beyond_players (uid) VALUES ('u1')",
    )
    .await;
    insert(
        &pool,
        "region",
        "INSERT INTO beyond_factory_regions (uid, region, region_id, scene_name)
         VALUES ('u1', 'test01', 'region_102', 'map01_lv001')",
    )
    .await;
    insert(
        &pool,
        "trade",
        "INSERT INTO beyond_factory_trade (uid, region, node_id, machine_id)
         VALUES ('u1', 'test01', 7, 'sp_trade_1')",
    )
    .await;
    insert(
        &pool,
        "order",
        "INSERT INTO beyond_factory_trade_orders
            (uid, region, node_id, inst_id, ord, order_id, contract_id, cost_value_delta)
         VALUES ('u1', 'test01', 7, 1, 0, 'order_1', 'contract_a', 0.85)",
    )
    .await;
    insert(
        &pool,
        "order item",
        "INSERT INTO beyond_factory_trade_order_items
            (uid, region, node_id, inst_id, idx, item_id, count)
         VALUES ('u1', 'test01', 7, 1, 0, 'item_a', 2)",
    )
    .await;

    sqlx::query("DELETE FROM beyond_factory_regions WHERE uid='u1' AND region='test01'")
        .execute(&pool)
        .await
        .expect("region delete");

    for table in [
        "beyond_factory_trade",
        "beyond_factory_trade_orders",
        "beyond_factory_trade_order_items",
    ] {
        assert_eq!(count(&pool, table).await, 0, "{table} survived its region");
    }
}

/// A machine keyed to a region that was never inserted must be refused
/// outright -- that is the write ordering 20.7 relies on being enforced.
#[tokio::test]
async fn a_machine_without_its_region_is_rejected() {
    let pool = migrated_pool().await;
    insert(
        &pool,
        "player",
        "INSERT INTO beyond_players (uid) VALUES ('u1')",
    )
    .await;

    let result = sqlx::query(
        "INSERT INTO beyond_factory_recycler (uid, region, node_id, machine_id)
         VALUES ('u1', 'nope', 5, 'sp_rec_1')",
    )
    .execute(&pool)
    .await;

    assert!(
        result.is_err(),
        "foreign keys are on; a machine must not outrun its region"
    );
}

/// `paused_at` is an `Option<Tick>` and NULL is the `None`. A running envelope
/// and one paused at tick 0 are different states, so the column must be
/// nullable rather than defaulting to 0.
#[tokio::test]
async fn paused_at_round_trips_as_null() {
    let pool = migrated_pool().await;
    insert(
        &pool,
        "player",
        "INSERT INTO beyond_players (uid) VALUES ('u1')",
    )
    .await;
    insert(
        &pool,
        "manual work",
        "INSERT INTO beyond_factory_manual_work (uid, paused_at) VALUES ('u1', NULL)",
    )
    .await;

    let (paused,): (Option<i64>,) =
        sqlx::query_as("SELECT paused_at FROM beyond_factory_manual_work WHERE uid='u1'")
            .fetch_one(&pool)
            .await
            .expect("select runs");
    assert!(paused.is_none());
}
