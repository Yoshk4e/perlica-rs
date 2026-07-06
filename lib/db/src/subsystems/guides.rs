use crate::error::{DbError, Result};
use perlica_logic::mission::GuideManager;
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

const COMPLETION_GROUP: i64 = 0;
const COMPLETION_KEY_STEP: i64 = 1;

pub(crate) async fn load(pool: &SqlitePool, uid: &str) -> Result<GuideManager> {
    let rows = sqlx::query(
        "SELECT completion_type, guide_id FROM beyond_guide_completions WHERE uid = ?1",
    )
    .bind(uid)
    .fetch_all(pool)
    .await?;

    let mut mgr = GuideManager::default();
    for r in rows {
        let ct: i64 = r.try_get("completion_type")?;
        let guide_id: String = r.try_get("guide_id")?;
        match ct {
            COMPLETION_GROUP => mgr.mark_group_completed(&guide_id),
            COMPLETION_KEY_STEP => mgr.mark_key_step_completed(&guide_id),
            other => {
                return Err(DbError::Corruption {
                    uid: uid.to_string(),
                    what: "guide_completion_type",
                    reason: format!("unknown discriminant {other}"),
                });
            }
        }
    }
    Ok(mgr)
}

pub(crate) async fn write(
    tx: &mut Transaction<'_, Sqlite>,
    uid: &str,
    mgr: &GuideManager,
) -> Result<()> {
    sync_partition(tx, uid, COMPLETION_GROUP, mgr.completed_groups()).await?;
    sync_partition(tx, uid, COMPLETION_KEY_STEP, mgr.completed_key_steps()).await?;
    Ok(())
}

// One pass: upsert + partition-scoped prune for `completion_type`.
async fn sync_partition(
    tx: &mut Transaction<'_, Sqlite>,
    uid: &str,
    completion_type: i64,
    ids: &[String],
) -> Result<()> {
    for gid in ids {
        sqlx::query(
            "INSERT OR IGNORE INTO beyond_guide_completions
                (uid, completion_type, guide_id)
             VALUES (?1, ?2, ?3)",
        )
        .bind(uid)
        .bind(completion_type)
        .bind(gid)
        .execute(&mut **tx)
        .await?;
    }

    // Prune: drop anything in this partition not in `ids`.
    if ids.is_empty() {
        sqlx::query(
            "DELETE FROM beyond_guide_completions
             WHERE uid = ?1 AND completion_type = ?2",
        )
        .bind(uid)
        .bind(completion_type)
        .execute(&mut **tx)
        .await?;
        return Ok(());
    }

    let chunk_size: usize = 500;
    for chunk in ids.chunks(chunk_size) {
        let placeholders = (0..chunk.len()).map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!(
            "DELETE FROM beyond_guide_completions
             WHERE uid = ?1 AND completion_type = ?2
               AND guide_id NOT IN ({placeholders})"
        );
        let mut q = sqlx::query(&sql).bind(uid).bind(completion_type);
        for v in chunk {
            q = q.bind(v.as_str());
        }
        q.execute(&mut **tx).await?;
    }
    Ok(())
}
