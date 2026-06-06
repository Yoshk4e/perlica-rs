use crate::error::{DbError, Result};
use crate::persistable::Persistable;
use crate::subsystems::{bitsets, char_bag, guides, mail, missions, player_root, scene, wallet};
use perlica_logic::bitset::BitsetManager;
use perlica_logic::character::char_bag::CharBag;
use perlica_logic::mail::MailManager;
use perlica_logic::mission::{GuideManager, MissionManager};
use perlica_logic::player::WorldState;
use perlica_logic::scene::{CheckpointInfo, RevivalMode};
use perlica_logic::wallet::WalletState;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tracing::{debug, info};

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

pub struct PlayerRecord {
    pub char_bag: CharBag,
    pub world: WorldState,
    pub bitsets: BitsetManager,
    pub checkpoint: Option<CheckpointInfo>,
    pub revival_mode: RevivalMode,
    pub missions: MissionManager,
    pub guides: GuideManager,
    pub mail: MailManager,
    pub wallet: WalletState,
}

pub struct PlayerRecordRef<'a> {
    pub char_bag: &'a CharBag,
    pub world: &'a WorldState,
    pub bitsets: &'a BitsetManager,
    pub checkpoint: Option<&'a CheckpointInfo>,
    pub revival_mode: RevivalMode,
    pub missions: &'a MissionManager,
    pub guides: &'a GuideManager,
    pub mail: &'a MailManager,
    pub wallet: &'a WalletState,
}

impl<'a> PlayerRecordRef<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        char_bag: &'a CharBag,
        world: &'a WorldState,
        bitsets: &'a BitsetManager,
        checkpoint: Option<&'a CheckpointInfo>,
        revival_mode: RevivalMode,
        missions: &'a MissionManager,
        guides: &'a GuideManager,
        mail: &'a MailManager,
        wallet: &'a WalletState,
    ) -> Self {
        Self {
            char_bag,
            world,
            bitsets,
            checkpoint,
            revival_mode,
            missions,
            guides,
            mail,
            wallet,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlayerDb {
    pool: SqlitePool,
}

impl PlayerDb {
    pub async fn open(dir: impl AsRef<Path>) -> Result<Self> {
        let dir: PathBuf = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir).map_err(|e| DbError::CreateDir {
            path: dir.clone(),
            source: e,
        })?;

        let db_path = dir.join("perlica.sqlite");
        let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.display()))
            .map_err(|e| DbError::Open {
                path: db_path.clone(),
                source: e,
            })?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(opts)
            .await
            .map_err(|e| DbError::Open {
                path: db_path.clone(),
                source: e,
            })?;

        // Apply embedded migrations.
        MIGRATOR.run(&pool).await?;

        info!("Opened SQLite player DB at {}", db_path.display());
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub(crate) async fn ensure_player_row(
        tx: &mut Transaction<'_, Sqlite>,
        uid: &str,
    ) -> Result<()> {
        sqlx::query("INSERT OR IGNORE INTO beyond_players (uid) VALUES (?1)")
            .bind(uid)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    pub async fn load(&self, uid: &str) -> Result<Option<PlayerRecord>> {
        let Some(root) = player_root::load(&self.pool, uid).await? else {
            return Ok(None);
        };

        let mut char_bag = char_bag::load(&self.pool, uid).await?;
        // Restore curr_team_index that lives on the player root row.
        char_bag.meta.curr_team_index = root.curr_team_index;
        // Restore inst-id allocators so newly added items don't collide
        // with the ids already in the depots.
        char_bag
            .item_manager
            .weapons
            .set_next_inst_id(root.weapon_next_inst_id);
        char_bag
            .item_manager
            .gems
            .set_next_inst_id(root.gem_next_inst_id);
        char_bag
            .item_manager
            .equips
            .set_next_inst_id(root.equip_next_inst_id);

        char_bag.validate_after_load();

        let bitsets = bitsets::load(&self.pool, uid).await?;

        let mut missions = missions::load(&self.pool, uid).await?;
        missions.update_track_mission(&root.track_mission_id);

        let guides = guides::load(&self.pool, uid).await?;

        let mut mail = mail::load(&self.pool, uid).await?;
        mail.set_next_id(root.mail_next_id);

        let wallet = wallet::load(&self.pool, uid).await?;

        let world = WorldState {
            role_level: root.role_level,
            role_exp: root.role_exp,
            last_scene: root.last_scene,
            pos_x: root.pos_x,
            pos_y: root.pos_y,
            pos_z: root.pos_z,
            rot_x: root.rot_x,
            rot_y: root.rot_y,
            rot_z: root.rot_z,
        };

        let checkpoint = match (
            root.checkpoint_scene,
            root.checkpoint_x,
            root.checkpoint_y,
            root.checkpoint_z,
        ) {
            (Some(s), Some(x), Some(y), Some(z)) => Some(CheckpointInfo {
                scene_name: s,
                pos_x: x,
                pos_y: y,
                pos_z: z,
            }),
            _ => None,
        };

        debug!("Loaded player from DB: uid={}", uid);
        Ok(Some(PlayerRecord {
            char_bag,
            world,
            bitsets,
            checkpoint,
            revival_mode: root.revival_mode,
            missions,
            guides,
            mail,
            wallet,
        }))
    }

    pub async fn save<'a>(&self, uid: &str, record: PlayerRecordRef<'a>) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        Self::ensure_player_row(&mut tx, uid).await?;

        player_root::write(
            &mut tx,
            uid,
            player_root::Row {
                role_level: record.world.role_level,
                role_exp: record.world.role_exp,
                last_scene: record.world.last_scene.clone(),
                pos_x: record.world.pos_x,
                pos_y: record.world.pos_y,
                pos_z: record.world.pos_z,
                rot_x: record.world.rot_x,
                rot_y: record.world.rot_y,
                rot_z: record.world.rot_z,
                curr_team_index: record.char_bag.meta.curr_team_index,
                track_mission_id: record.missions.track_mission_id().to_string(),
                revival_mode: record.revival_mode,
                checkpoint_scene: record.checkpoint.map(|c| c.scene_name.clone()),
                checkpoint_x: record.checkpoint.map(|c| c.pos_x),
                checkpoint_y: record.checkpoint.map(|c| c.pos_y),
                checkpoint_z: record.checkpoint.map(|c| c.pos_z),
                weapon_next_inst_id: record.char_bag.item_manager.weapons.next_inst_id(),
                gem_next_inst_id: record.char_bag.item_manager.gems.next_inst_id(),
                equip_next_inst_id: record.char_bag.item_manager.equips.next_inst_id(),
                mail_next_id: record.mail.next_id(),
                updated_at: common::time::now_ms() as i64,
            },
        )
        .await?;

        char_bag::write(&mut tx, uid, record.char_bag).await?;
        bitsets::write(&mut tx, uid, record.bitsets).await?;
        missions::write(&mut tx, uid, record.missions).await?;
        guides::write(&mut tx, uid, record.guides).await?;
        mail::write(&mut tx, uid, record.mail).await?;
        wallet::write(&mut tx, uid, record.wallet).await?;

        tx.commit().await?;
        debug!("Full save complete for uid={}", uid);
        Ok(())
    }
}

impl Persistable for WorldState {
    async fn persist(&self, uid: &str, db: &PlayerDb) -> Result<()> {
        let mut tx = db.pool.begin().await?;
        PlayerDb::ensure_player_row(&mut tx, uid).await?;
        player_root::update_world(&mut tx, uid, self).await?;
        tx.commit().await?;
        Ok(())
    }
}

impl Persistable for CharBag {
    async fn persist(&self, uid: &str, db: &PlayerDb) -> Result<()> {
        let mut tx = db.pool.begin().await?;
        PlayerDb::ensure_player_row(&mut tx, uid).await?;
        player_root::update_char_bag_scalars(&mut tx, uid, self).await?;
        char_bag::write(&mut tx, uid, self).await?;
        tx.commit().await?;
        Ok(())
    }
}

impl PlayerDb {
    /// Preferred persistence entry point for `CharBag`.
    ///
    /// Uses the per-depot `PendingChanges` trackers to only re-upsert
    /// the rows that actually changed since the last successful
    /// commit. Returns immediately if nothing is dirty.
    ///
    /// On success, every tracker on `bag` is cleared. On failure the
    /// trackers are left intact so the next call retries the same
    /// rows.
    pub async fn persist_char_bag_incremental(&self, uid: &str, bag: &mut CharBag) -> Result<()> {
        if !bag.has_pending_changes() {
            return Ok(());
        }

        let n_chars = bag.pending_chars().dirty_count();
        let n_teams = bag.pending_teams().dirty_count();
        let n_weapons = bag.item_manager.weapons.pending().dirty_count();
        let n_gems = bag.item_manager.gems.pending().dirty_count();
        let n_equips = bag.item_manager.equips.pending().dirty_count();
        let n_special = bag.item_manager.special_items.pending().dirty_count();
        let n_mission = bag.item_manager.mission_items.pending().dirty_count();
        let n_factory = bag.item_manager.factory_items.pending().dirty_count();

        let mut tx = self.pool.begin().await?;
        PlayerDb::ensure_player_row(&mut tx, uid).await?;

        // `beyond_players` is a single-row UPDATE; cheap and we update
        // it whenever any depot allocated a new inst id (which always
        // means at least one weapon/gem/equip was marked dirty).
        let needs_scalar_update =
            bag.is_meta_dirty() || n_weapons > 0 || n_gems > 0 || n_equips > 0;
        if needs_scalar_update {
            player_root::update_char_bag_scalars(&mut tx, uid, bag).await?;
        }

        char_bag::write_incremental(&mut tx, uid, bag).await?;
        tx.commit().await?;

        debug!(
            "Incremental char_bag flush for uid={}: chars={}, teams={}, weapons={}, gems={}, equips={}, stackables=({}+{}+{})",
            uid, n_chars, n_teams, n_weapons, n_gems, n_equips, n_special, n_mission, n_factory
        );

        bag.clear_all_pending();
        Ok(())
    }
}

impl Persistable for BitsetManager {
    async fn persist(&self, uid: &str, db: &PlayerDb) -> Result<()> {
        let mut tx = db.pool.begin().await?;
        PlayerDb::ensure_player_row(&mut tx, uid).await?;
        bitsets::write(&mut tx, uid, self).await?;
        tx.commit().await?;
        Ok(())
    }
}

impl Persistable for MissionManager {
    async fn persist(&self, uid: &str, db: &PlayerDb) -> Result<()> {
        let mut tx = db.pool.begin().await?;
        PlayerDb::ensure_player_row(&mut tx, uid).await?;
        player_root::update_track_mission(&mut tx, uid, self.track_mission_id()).await?;
        missions::write(&mut tx, uid, self).await?;
        tx.commit().await?;
        Ok(())
    }
}

impl Persistable for GuideManager {
    async fn persist(&self, uid: &str, db: &PlayerDb) -> Result<()> {
        let mut tx = db.pool.begin().await?;
        PlayerDb::ensure_player_row(&mut tx, uid).await?;
        guides::write(&mut tx, uid, self).await?;
        tx.commit().await?;
        Ok(())
    }
}

impl Persistable for MailManager {
    async fn persist(&self, uid: &str, db: &PlayerDb) -> Result<()> {
        let mut tx = db.pool.begin().await?;
        PlayerDb::ensure_player_row(&mut tx, uid).await?;
        player_root::update_mail_next_id(&mut tx, uid, self.next_id()).await?;
        mail::write(&mut tx, uid, self).await?;
        tx.commit().await?;
        Ok(())
    }
}

impl Persistable for WalletState {
    async fn persist(&self, uid: &str, db: &PlayerDb) -> Result<()> {
        let mut tx = db.pool.begin().await?;
        PlayerDb::ensure_player_row(&mut tx, uid).await?;
        wallet::write(&mut tx, uid, self).await?;
        tx.commit().await?;
        Ok(())
    }
}

/// `SceneManager` has lots of *runtime* state (entity caches, spatial
/// grids, …) that has no business being saved. Wrapping just the two
/// persisted fields in a tiny holder gives handlers a clean
/// `SceneSaveState { ... }.persist(uid, db).await?` call site without
/// dragging the whole manager into the DB crate.
pub struct SceneSaveState<'a> {
    pub checkpoint: Option<&'a CheckpointInfo>,
    pub revival_mode: RevivalMode,
}

impl<'a> Persistable for SceneSaveState<'a> {
    async fn persist(&self, uid: &str, db: &PlayerDb) -> Result<()> {
        let mut tx = db.pool.begin().await?;
        PlayerDb::ensure_player_row(&mut tx, uid).await?;
        scene::update_checkpoint(&mut tx, uid, self.checkpoint, self.revival_mode).await?;
        tx.commit().await?;
        Ok(())
    }
}
