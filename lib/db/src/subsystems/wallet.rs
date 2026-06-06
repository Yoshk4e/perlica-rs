use crate::error::Result;
use perlica_logic::wallet::WalletState;
use sqlx::{SqlitePool, Transaction};

pub(crate) async fn load(pool: &SqlitePool, uid: &str) -> Result<WalletState> {
    let row =
        sqlx::query_as::<_, (i64, i64)>("SELECT gold, diamond FROM beyond_wallet WHERE uid = ?1")
            .bind(uid)
            .fetch_optional(pool)
            .await?;

    match row {
        Some((gold, diamond)) => Ok(WalletState::with_balances(gold as u64, diamond as u64)),
        None => Ok(WalletState::default()),
    }
}

pub(crate) async fn write(
    tx: &mut Transaction<'_, sqlx::Sqlite>,
    uid: &str,
    wallet: &WalletState,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO beyond_wallet (uid, gold, diamond) VALUES (?1, ?2, ?3)
         ON CONFLICT(uid) DO UPDATE SET gold = excluded.gold, diamond = excluded.diamond",
    )
    .bind(uid)
    .bind(wallet.gold as i64)
    .bind(wallet.diamond as i64)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
