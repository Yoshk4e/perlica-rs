use crate::error::{DbError, Result};
use perlica_logic::bitset::{BitsetManager, BitsetType};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

pub(crate) async fn load(pool: &SqlitePool, uid: &str) -> Result<BitsetManager> {
    let rows = sqlx::query("SELECT bitset_type, bit_value FROM beyond_bitsets WHERE uid = ?1")
        .bind(uid)
        .fetch_all(pool)
        .await?;

    let mut mgr = BitsetManager::new();
    for r in rows {
        let bt_i: i64 = r.try_get("bitset_type")?;
        let bv: i64 = r.try_get("bit_value")?;
        let bt = BitsetType::from_i32(bt_i as i32).ok_or_else(|| DbError::Corruption {
            uid: uid.to_string(),
            what: "bitset_type",
            reason: format!("unknown discriminant {bt_i}"),
        })?;
        mgr.set(bt, bv as u32);
    }
    Ok(mgr)
}

pub(crate) async fn write(
    tx: &mut Transaction<'_, Sqlite>,
    uid: &str,
    mgr: &BitsetManager,
) -> Result<()> {
    for bt in ALL_BITSET_TYPES {
        let bits = mgr.get_bits(*bt);
        for bit in &bits {
            sqlx::query(
                "INSERT OR IGNORE INTO beyond_bitsets (uid, bitset_type, bit_value)
                 VALUES (?1, ?2, ?3)",
            )
            .bind(uid)
            .bind(*bt as i32 as i64)
            .bind(*bit as i64)
            .execute(&mut **tx)
            .await?;
        }

        prune_for_type(tx, uid, *bt, &bits).await?;
    }
    Ok(())
}

async fn prune_for_type(
    tx: &mut Transaction<'_, Sqlite>,
    uid: &str,
    bt: BitsetType,
    keep: &[u32],
) -> Result<()> {
    if keep.is_empty() {
        sqlx::query("DELETE FROM beyond_bitsets WHERE uid = ?1 AND bitset_type = ?2")
            .bind(uid)
            .bind(bt as i32 as i64)
            .execute(&mut **tx)
            .await?;
        return Ok(());
    }

    let keep_i64: Vec<i64> = keep.iter().map(|v| *v as i64).collect();
    let chunk_size: usize = 500;
    for chunk in keep_i64.chunks(chunk_size) {
        let placeholders = (0..chunk.len()).map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!(
            "DELETE FROM beyond_bitsets
             WHERE uid = ?1 AND bitset_type = ?2 AND bit_value NOT IN ({placeholders})"
        );
        let mut q = sqlx::query(&sql).bind(uid).bind(bt as i32 as i64);
        for v in chunk {
            q = q.bind(*v);
        }
        q.execute(&mut **tx).await?;
    }
    Ok(())
}

const ALL_BITSET_TYPES: &[BitsetType] = &[
    BitsetType::FoundItem,
    BitsetType::Wiki,
    BitsetType::UnreadWiki,
    BitsetType::MonsterDrop,
    BitsetType::GotItem,
    BitsetType::AreaFirstView,
    BitsetType::UnreadGotItem,
    BitsetType::Prts,
    BitsetType::UnreadPrts,
    BitsetType::PrtsFirstLv,
    BitsetType::PrtsTerminalContent,
    BitsetType::LevelHaveBeen,
    BitsetType::LevelMapFirstView,
    BitsetType::UnreadFormula,
    BitsetType::NewChar,
    BitsetType::ElogChannel,
    BitsetType::FmvWatched,
    BitsetType::TimeLineWatched,
    BitsetType::MapFilter,
];
