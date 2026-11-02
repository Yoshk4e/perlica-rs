use crate::error::Result;
use perlica_logic::scene::{CheckpointInfo, RevivalMode, SceneProgress};
use sqlx::{Sqlite, Transaction};

pub(crate) async fn load_progress(pool: &sqlx::SqlitePool, uid: &str) -> Result<SceneProgress> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT scene_name, level_logic_id FROM beyond_destroyed_interactives WHERE uid = ?1",
    )
    .bind(uid)
    .fetch_all(pool)
    .await?;
    let mut progress = SceneProgress::default();
    for (scene, id) in rows {
        let id = id
            .parse::<u64>()
            .map_err(|e| crate::error::DbError::Corruption {
                uid: uid.to_owned(),
                what: "destroyed interactive ID",
                reason: e.to_string(),
            })?;
        progress.destroy_interactive(&scene, id);
    }
    progress.mark_saved();
    Ok(progress)
}

pub(crate) async fn write_progress(
    tx: &mut Transaction<'_, Sqlite>,
    uid: &str,
    progress: &SceneProgress,
) -> Result<()> {
    for (scene, id) in progress.destroyed_interactives() {
        sqlx::query(
            "INSERT OR IGNORE INTO beyond_destroyed_interactives (uid, scene_name, level_logic_id) VALUES (?1, ?2, ?3)",
        )
        .bind(uid)
        .bind(scene)
        .bind(id.to_string())
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub(crate) async fn update_checkpoint(
    tx: &mut Transaction<'_, Sqlite>,
    uid: &str,
    checkpoint: Option<&CheckpointInfo>,
    revival_mode: RevivalMode,
) -> Result<()> {
    sqlx::query(
        "UPDATE beyond_players SET
            checkpoint_scene = ?2,
            checkpoint_x     = ?3,
            checkpoint_y     = ?4,
            checkpoint_z     = ?5,
            revival_mode     = ?6,
            updated_at       = ?7
         WHERE uid = ?1",
    )
    .bind(uid)
    .bind(checkpoint.map(|c| c.scene_name.as_str()))
    .bind(checkpoint.map(|c| c.pos_x as f64))
    .bind(checkpoint.map(|c| c.pos_y as f64))
    .bind(checkpoint.map(|c| c.pos_z as f64))
    .bind(revival_mode as i32 as i64)
    .bind(common::time::now_ms() as i64)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
