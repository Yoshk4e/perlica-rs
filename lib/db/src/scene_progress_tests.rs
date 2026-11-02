use super::*;

async fn database() -> PlayerDb {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::from_str("sqlite::memory:")
                .unwrap()
                .foreign_keys(true),
        )
        .await
        .unwrap();
    MIGRATOR.run(&pool).await.unwrap();
    PlayerDb { pool }
}

#[tokio::test]
async fn destroyed_interactives_survive_reload_and_are_scoped_to_player_and_scene() {
    let db = database().await;
    let mut progress = SceneProgress::default();
    progress.destroy_interactive("map01_lv002", 1065);
    progress.destroy_interactive("map01_lv002", u64::MAX);
    db.persist_scene_progress("player", &mut progress)
        .await
        .unwrap();
    assert!(!progress.has_pending_changes());

    let mut restored = db.load("player").await.unwrap().unwrap().scene_progress;
    assert!(restored.is_destroyed("map01_lv002", 1065));
    assert!(restored.is_destroyed("map01_lv002", u64::MAX));
    assert!(!restored.is_destroyed("map01_lv001", 1065));
    assert!(!restored.has_pending_changes());
    restored.destroy_interactive("map01_lv001", 1065);
    db.persist_scene_progress("player", &mut restored)
        .await
        .unwrap();
    let restored = db.load("player").await.unwrap().unwrap().scene_progress;
    assert_eq!(restored.destroyed_interactives().count(), 3);
    assert!(
        !scene::load_progress(db.pool(), "other")
            .await
            .unwrap()
            .is_destroyed("map01_lv002", 1065)
    );

    sqlx::query("DELETE FROM beyond_players WHERE uid = 'player'")
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(
        scene::load_progress(db.pool(), "player")
            .await
            .unwrap()
            .destroyed_interactives()
            .count(),
        0
    );
}

#[tokio::test]
async fn failed_progress_save_retains_pending_changes() {
    let db = database().await;
    let mut progress = SceneProgress::default();
    progress.destroy_interactive("map01_lv002", 1065);
    db.pool.close().await;
    assert!(
        db.persist_scene_progress("player", &mut progress)
            .await
            .is_err()
    );
    assert!(progress.has_pending_changes());
}
