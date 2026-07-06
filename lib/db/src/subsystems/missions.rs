use crate::error::{DbError, Result};
use crate::subsystems::prune;
use perlica_logic::mission::MissionManager;
use perlica_proto::{MissionState, QuestObjective, QuestState};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use std::collections::HashMap;

pub(crate) async fn load(pool: &SqlitePool, uid: &str) -> Result<MissionManager> {
    let mut mgr = MissionManager::default();

    let mission_rows = sqlx::query(
        "SELECT mission_id, mission_state, succeed_id FROM beyond_missions WHERE uid = ?1",
    )
    .bind(uid)
    .fetch_all(pool)
    .await?;
    for r in mission_rows {
        let mission_id: String = r.try_get("mission_id")?;
        let state_i: i64 = r.try_get("mission_state")?;
        let succeed_id: i64 = r.try_get("succeed_id")?;
        let state = MissionState::try_from(state_i as i32).map_err(|_| DbError::Corruption {
            uid: uid.to_string(),
            what: "mission_state",
            reason: format!("unknown discriminant {state_i}"),
        })?;
        mgr.insert_loaded_mission(mission_id, state, succeed_id as i32);
    }

    let quest_rows = sqlx::query("SELECT quest_id, quest_state FROM beyond_quests WHERE uid = ?1")
        .bind(uid)
        .fetch_all(pool)
        .await?;

    for r in quest_rows {
        let quest_id: String = r.try_get("quest_id")?;
        let state_i: i64 = r.try_get("quest_state")?;
        let state = QuestState::try_from(state_i as i32).map_err(|_| DbError::Corruption {
            uid: uid.to_string(),
            what: "quest_state",
            reason: format!("unknown discriminant {state_i}"),
        })?;

        let obj_rows = sqlx::query(
            "SELECT condition_id, is_complete, objective_value
             FROM beyond_quest_objectives
             WHERE uid = ?1 AND quest_id = ?2",
        )
        .bind(uid)
        .bind(&quest_id)
        .fetch_all(pool)
        .await?;

        let objectives: Vec<QuestObjective> = obj_rows
            .into_iter()
            .map(|or| {
                let condition_id: String = or.try_get("condition_id")?;
                let is_complete: i64 = or.try_get("is_complete")?;
                let value: i64 = or.try_get("objective_value")?;
                let mut values = HashMap::new();
                values.insert(condition_id.clone(), value as i32);
                Ok::<_, DbError>(QuestObjective {
                    condition_id,
                    extra_details: HashMap::new(),
                    is_complete: is_complete != 0,
                    values,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        mgr.insert_loaded_quest(quest_id, state, objectives);
    }

    Ok(mgr)
}

pub(crate) async fn write(
    tx: &mut Transaction<'_, Sqlite>,
    uid: &str,
    mgr: &MissionManager,
) -> Result<()> {
    let missions = mgr.snapshot_missions();
    let quests = mgr.snapshot_quests();

    for (mission_id, state, succeed_id) in &missions {
        sqlx::query(
            "INSERT INTO beyond_missions (uid, mission_id, mission_state, succeed_id)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(uid, mission_id) DO UPDATE SET
                mission_state = excluded.mission_state,
                succeed_id    = excluded.succeed_id",
        )
        .bind(uid)
        .bind(mission_id)
        .bind(*state as i32 as i64)
        .bind(*succeed_id as i64)
        .execute(&mut **tx)
        .await?;
    }

    for (quest_id, state, objectives) in &quests {
        sqlx::query(
            "INSERT INTO beyond_quests (uid, quest_id, quest_state)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(uid, quest_id) DO UPDATE SET
                quest_state = excluded.quest_state",
        )
        .bind(uid)
        .bind(quest_id)
        .bind(*state as i32 as i64)
        .execute(&mut **tx)
        .await?;

        // Upsert each objective.
        for obj in objectives {
            let value = obj
                .values
                .get(&obj.condition_id)
                .copied()
                .unwrap_or_default();
            sqlx::query(
                "INSERT INTO beyond_quest_objectives
                    (uid, quest_id, condition_id, is_complete, objective_value)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(uid, quest_id, condition_id) DO UPDATE SET
                    is_complete     = excluded.is_complete,
                    objective_value = excluded.objective_value",
            )
            .bind(uid)
            .bind(quest_id)
            .bind(&obj.condition_id)
            .bind(if obj.is_complete { 1i64 } else { 0i64 })
            .bind(value as i64)
            .execute(&mut **tx)
            .await?;
        }

        // Prune objectives that no longer belong to this quest.
        prune_objectives_for_quest(tx, uid, quest_id, objectives).await?;
    }

    let mission_keep: Vec<&str> = missions.iter().map(|(m, _, _)| m.as_str()).collect();
    prune::prune_str_pk(tx, "beyond_missions", uid, "mission_id", &mission_keep).await?;

    let quest_keep: Vec<&str> = quests.iter().map(|(q, _, _)| q.as_str()).collect();
    prune::prune_str_pk(tx, "beyond_quests", uid, "quest_id", &quest_keep).await?;

    Ok(())
}

async fn prune_objectives_for_quest(
    tx: &mut Transaction<'_, Sqlite>,
    uid: &str,
    quest_id: &str,
    keep: &[QuestObjective],
) -> Result<()> {
    if keep.is_empty() {
        sqlx::query("DELETE FROM beyond_quest_objectives WHERE uid = ?1 AND quest_id = ?2")
            .bind(uid)
            .bind(quest_id)
            .execute(&mut **tx)
            .await?;
        return Ok(());
    }

    let chunk_size: usize = 500;
    for chunk in keep.chunks(chunk_size) {
        let placeholders = (0..chunk.len()).map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!(
            "DELETE FROM beyond_quest_objectives
             WHERE uid = ?1 AND quest_id = ?2
               AND condition_id NOT IN ({placeholders})"
        );
        let mut q = sqlx::query(&sql).bind(uid).bind(quest_id);
        for obj in chunk {
            q = q.bind(obj.condition_id.as_str());
        }
        q.execute(&mut **tx).await?;
    }
    Ok(())
}
