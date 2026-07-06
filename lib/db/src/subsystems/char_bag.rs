use crate::error::{DbError, Result};
use crate::subsystems::prune;
use config::item::{CraftShowingType, ItemDepotType};
use perlica_logic::character::char_bag::{Char, CharBag, CharIndex, Team, TeamSlot};
use perlica_logic::item::{
    EquipInstId, EquipInstance, GemInstId, GemInstance, WeaponInstId, WeaponInstance,
};
use perlica_proto::EquipAttr;
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use std::collections::HashMap;

const DEPOT_SPECIAL: i64 = ItemDepotType::SpecialItem as i64;
const DEPOT_MISSION: i64 = ItemDepotType::MissionItem as i64;
const DEPOT_FACTORY: i64 = ItemDepotType::Factory as i64;

pub(crate) async fn load(pool: &SqlitePool, uid: &str) -> Result<CharBag> {
    let mut bag = CharBag::default();
    let char_rows = sqlx::query(
        "SELECT char_index, template_id, level, exp, break_stage,
                is_dead, hp, ultimate_sp, own_time
         FROM beyond_chars
         WHERE uid = ?1
         ORDER BY char_index",
    )
    .bind(uid)
    .fetch_all(pool)
    .await?;

    // The char_index column is the position in `CharBag.chars`. To
    // restore that position exactly we extend the Vec with placeholder
    // `Char::default()` entries up to the max index, then assign by
    // index. This handles the (admittedly rare) case where a char was
    // removed from the middle of the vec without renumbering.
    let mut by_index: Vec<(usize, Char)> = Vec::with_capacity(char_rows.len());
    let mut max_index: i64 = -1;
    for r in char_rows {
        let idx: i64 = r.try_get("char_index")?;
        max_index = max_index.max(idx);
        let template_id: String = r.try_get("template_id")?;
        let level: i64 = r.try_get("level")?;
        let exp: i64 = r.try_get("exp")?;
        let break_stage: i64 = r.try_get("break_stage")?;
        let is_dead: i64 = r.try_get("is_dead")?;
        let hp: f64 = r.try_get("hp")?;
        let ultimate_sp: f64 = r.try_get("ultimate_sp")?;
        let own_time: i64 = r.try_get("own_time")?;

        by_index.push((
            idx as usize,
            Char {
                template_id,
                level: level as i32,
                exp: exp as i32,
                break_stage: break_stage as u32,
                is_dead: is_dead != 0,
                hp,
                ultimate_sp: ultimate_sp as f32,
                cached_weapon_inst_id: None,
                own_time,
                skill_levels: HashMap::new(),
            },
        ));
    }

    if max_index >= 0 {
        bag.chars.resize((max_index + 1) as usize, Char::default());
        for (i, ch) in by_index {
            bag.chars[i] = ch;
        }
    }

    let skill_rows = sqlx::query(
        "SELECT char_index, skill_id, skill_level
         FROM beyond_char_skills WHERE uid = ?1",
    )
    .bind(uid)
    .fetch_all(pool)
    .await?;
    for r in skill_rows {
        let idx: i64 = r.try_get("char_index")?;
        let skill_id: String = r.try_get("skill_id")?;
        let lv: i64 = r.try_get("skill_level")?;
        if let Some(ch) = bag.chars.get_mut(idx as usize) {
            ch.skill_levels.insert(skill_id, lv as u32);
        }
    }

    let team_rows = sqlx::query(
        "SELECT team_index, team_name, leader_char_index
         FROM beyond_teams WHERE uid = ?1
         ORDER BY team_index",
    )
    .bind(uid)
    .fetch_all(pool)
    .await?;

    let mut teams_by_index: Vec<(usize, Team)> = Vec::with_capacity(team_rows.len());
    let mut max_team_index: i64 = -1;
    for r in team_rows {
        let idx: i64 = r.try_get("team_index")?;
        max_team_index = max_team_index.max(idx);
        let name: String = r.try_get("team_name")?;
        let leader: i64 = r.try_get("leader_char_index")?;
        teams_by_index.push((
            idx as usize,
            Team {
                name,
                char_team: [TeamSlot::Empty; Team::SLOTS_COUNT],
                leader_index: CharIndex::from_usize(leader as usize),
            },
        ));
    }
    if max_team_index >= 0 {
        bag.teams
            .resize((max_team_index + 1) as usize, Team::default());
        for (i, team) in teams_by_index {
            bag.teams[i] = team;
        }
    }

    let slot_rows = sqlx::query(
        "SELECT team_index, slot_index, char_index
         FROM beyond_team_slots WHERE uid = ?1",
    )
    .bind(uid)
    .fetch_all(pool)
    .await?;
    for r in slot_rows {
        let team_idx: i64 = r.try_get("team_index")?;
        let slot_idx: i64 = r.try_get("slot_index")?;
        let char_idx: Option<i64> = r.try_get("char_index")?;
        if let Some(team) = bag.teams.get_mut(team_idx as usize)
            && let Some(slot) = team.char_team.get_mut(slot_idx as usize) {
                *slot = match char_idx {
                    Some(c) => TeamSlot::Occupied(CharIndex::from_usize(c as usize)),
                    None => TeamSlot::Empty,
                };
            }
    }

    let weapon_rows = sqlx::query(
        "SELECT inst_id, template_id, exp, weapon_lv, refine_lv,
                breakthrough_lv, equip_char_id, attach_gem_id,
                is_lock, is_new, own_time
         FROM beyond_weapons WHERE uid = ?1",
    )
    .bind(uid)
    .fetch_all(pool)
    .await?;
    for r in weapon_rows {
        let inst_id: i64 = r.try_get("inst_id")?;
        let template_id: String = r.try_get("template_id")?;
        let exp: i64 = r.try_get("exp")?;
        let weapon_lv: i64 = r.try_get("weapon_lv")?;
        let refine_lv: i64 = r.try_get("refine_lv")?;
        let breakthrough_lv: i64 = r.try_get("breakthrough_lv")?;
        let equip_char_id: i64 = r.try_get("equip_char_id")?;
        let attach_gem_id: i64 = r.try_get("attach_gem_id")?;
        let is_lock: i64 = r.try_get("is_lock")?;
        let is_new: i64 = r.try_get("is_new")?;
        let own_time: i64 = r.try_get("own_time")?;
        bag.item_manager.weapons.insert_weapon(WeaponInstance {
            inst_id: WeaponInstId::new(inst_id as u64),
            template_id,
            exp: exp as u64,
            weapon_lv: weapon_lv as u64,
            refine_lv: refine_lv as u64,
            breakthrough_lv: breakthrough_lv as u64,
            equip_char_id: equip_char_id as u64,
            attach_gem_id: attach_gem_id as u64,
            is_lock: is_lock != 0,
            is_new: is_new != 0,
            own_time,
        });
    }

    let gem_rows = sqlx::query(
        "SELECT inst_id, template_id, craft_slot, attach_weapon_id,
                is_lock, is_new, own_time
         FROM beyond_gems WHERE uid = ?1",
    )
    .bind(uid)
    .fetch_all(pool)
    .await?;
    for r in gem_rows {
        let inst_id: i64 = r.try_get("inst_id")?;
        let template_id: String = r.try_get("template_id")?;
        let craft_slot_i: i64 = r.try_get("craft_slot")?;
        let attach_weapon_id: i64 = r.try_get("attach_weapon_id")?;
        let is_lock: i64 = r.try_get("is_lock")?;
        let is_new: i64 = r.try_get("is_new")?;
        let own_time: i64 = r.try_get("own_time")?;

        let craft_slot =
            CraftShowingType::try_from(craft_slot_i as u32).map_err(|v| DbError::Corruption {
                uid: uid.to_string(),
                what: "gem.craft_slot",
                reason: format!("unknown discriminant {v}"),
            })?;

        bag.item_manager.gems.insert(GemInstance {
            inst_id: GemInstId::new(inst_id as u64),
            template_id,
            craft_slot,
            attach_weapon_id: attach_weapon_id as u64,
            is_lock: is_lock != 0,
            is_new: is_new != 0,
            own_time,
        });
    }

    let equip_rows = sqlx::query(
        "SELECT inst_id, template_id, slot, equip_char_id,
                is_lock, is_new, own_time
         FROM beyond_equips WHERE uid = ?1",
    )
    .bind(uid)
    .fetch_all(pool)
    .await?;

    for r in equip_rows {
        let inst_id: i64 = r.try_get("inst_id")?;
        let template_id: String = r.try_get("template_id")?;
        let slot_i: i64 = r.try_get("slot")?;
        let equip_char_id: i64 = r.try_get("equip_char_id")?;
        let is_lock: i64 = r.try_get("is_lock")?;
        let is_new: i64 = r.try_get("is_new")?;
        let own_time: i64 = r.try_get("own_time")?;

        let slot = CraftShowingType::try_from(slot_i as u32).map_err(|v| DbError::Corruption {
            uid: uid.to_string(),
            what: "equip.slot",
            reason: format!("unknown discriminant {v}"),
        })?;

        // Fetch attrs for this piece in their original order.
        let attr_rows = sqlx::query(
            "SELECT attr_index, attr_type, modifier_type, modifier_value
             FROM beyond_equip_attrs
             WHERE uid = ?1 AND inst_id = ?2
             ORDER BY attr_index",
        )
        .bind(uid)
        .bind(inst_id)
        .fetch_all(pool)
        .await?;

        let attrs: Vec<EquipAttr> = attr_rows
            .into_iter()
            .map(|ar| {
                Ok::<_, sqlx::Error>(EquipAttr {
                    attr_type: ar.try_get::<i64, _>("attr_type")? as i32,
                    modifier_type: ar.try_get::<i64, _>("modifier_type")? as i32,
                    modifier_value: ar.try_get::<f64, _>("modifier_value")?,
                })
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;

        bag.item_manager.equips.insert(EquipInstance {
            inst_id: EquipInstId::new(inst_id as u64),
            template_id,
            slot,
            attrs,
            equip_char_id: equip_char_id as u64,
            is_lock: is_lock != 0,
            is_new: is_new != 0,
            own_time,
        });
    }

    let stack_rows = sqlx::query(
        "SELECT depot_type, template_id, count
         FROM beyond_stackable_items WHERE uid = ?1",
    )
    .bind(uid)
    .fetch_all(pool)
    .await?;
    for r in stack_rows {
        let depot_type: i64 = r.try_get("depot_type")?;
        let template_id: String = r.try_get("template_id")?;
        let count: i64 = r.try_get("count")?;
        let count_u = count as u32;
        match depot_type {
            DEPOT_SPECIAL => bag
                .item_manager
                .special_items
                .set_loaded(&template_id, count_u),
            DEPOT_MISSION => bag
                .item_manager
                .mission_items
                .set_loaded(&template_id, count_u),
            DEPOT_FACTORY => bag
                .item_manager
                .factory_items
                .set_loaded(&template_id, count_u),
            other => {
                return Err(DbError::Corruption {
                    uid: uid.to_string(),
                    what: "stackable_item.depot_type",
                    reason: format!("unexpected discriminant {other}"),
                });
            }
        }
    }

    Ok(bag)
}

/// Full sync: write every row in `bag` to disk and prune anything
/// that's no longer there. O(N) in the size of the bag - only used
/// for the first save of a brand-new player, the explicit
/// `PlayerDb::save` shutdown flow, and the GM "force full save" path.
/// The hot game-loop path uses [`write_incremental`] instead.
pub(crate) async fn write(
    tx: &mut Transaction<'_, Sqlite>,
    uid: &str,
    bag: &CharBag,
) -> Result<()> {
    write_chars(tx, uid, bag).await?;
    write_teams(tx, uid, bag).await?;
    write_weapons(tx, uid, bag).await?;
    write_gems(tx, uid, bag).await?;
    write_equips(tx, uid, bag).await?;
    write_stackables(tx, uid, bag).await?;
    Ok(())
}

/// Incremental sync: only touch the rows that the in-memory dirty
/// trackers point at. The number of SQL round-trips is
/// `O(actually-modified rows)` rather than `O(total bag size)`.
///
/// Caller responsibilities:
///   1. Call inside a `Transaction`.
///   2. On `Ok(())`, clear every tracker on `bag` so the next call is
///      a no-op until something changes again.
///   3. On `Err(_)`, do NOT clear the trackers - the same rows will be
///      retried next flush.
pub(crate) async fn write_incremental(
    tx: &mut Transaction<'_, Sqlite>,
    uid: &str,
    bag: &CharBag,
) -> Result<()> {
    write_chars_incremental(tx, uid, bag).await?;
    write_teams_incremental(tx, uid, bag).await?;
    write_weapons_incremental(tx, uid, bag).await?;
    write_gems_incremental(tx, uid, bag).await?;
    write_equips_incremental(tx, uid, bag).await?;
    write_stackables_incremental(tx, uid, bag).await?;
    Ok(())
}

async fn write_chars(tx: &mut Transaction<'_, Sqlite>, uid: &str, bag: &CharBag) -> Result<()> {
    // Upsert every char.
    for (i, ch) in bag.chars.iter().enumerate() {
        sqlx::query(
            "INSERT INTO beyond_chars (
                uid, char_index, template_id, level, exp, break_stage,
                is_dead, hp, ultimate_sp, own_time
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(uid, char_index) DO UPDATE SET
                template_id  = excluded.template_id,
                level        = excluded.level,
                exp          = excluded.exp,
                break_stage  = excluded.break_stage,
                is_dead      = excluded.is_dead,
                hp           = excluded.hp,
                ultimate_sp  = excluded.ultimate_sp,
                own_time     = excluded.own_time",
        )
        .bind(uid)
        .bind(i as i64)
        .bind(&ch.template_id)
        .bind(ch.level as i64)
        .bind(ch.exp as i64)
        .bind(ch.break_stage as i64)
        .bind(if ch.is_dead { 1i64 } else { 0i64 })
        .bind(ch.hp)
        .bind(ch.ultimate_sp as f64)
        .bind(ch.own_time)
        .execute(&mut **tx)
        .await?;

        // Per-char skills: upsert each, then prune within this char.
        for (skill_id, skill_lv) in &ch.skill_levels {
            sqlx::query(
                "INSERT INTO beyond_char_skills
                    (uid, char_index, skill_id, skill_level)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(uid, char_index, skill_id) DO UPDATE SET
                    skill_level = excluded.skill_level",
            )
            .bind(uid)
            .bind(i as i64)
            .bind(skill_id)
            .bind(*skill_lv as i64)
            .execute(&mut **tx)
            .await?;
        }
        prune_char_skills_for_char(tx, uid, i as i64, &ch.skill_levels).await?;
    }

    // Prune chars that were removed from the Vec.
    let char_keep: Vec<i64> = (0..bag.chars.len() as i64).collect();
    prune::prune_i64_pk(tx, "beyond_chars", uid, "char_index", &char_keep).await?;
    // ON DELETE CASCADE on `beyond_char_skills` takes care of orphan
    // skills for any char that just got pruned.

    Ok(())
}

async fn prune_char_skills_for_char(
    tx: &mut Transaction<'_, Sqlite>,
    uid: &str,
    char_index: i64,
    keep: &HashMap<String, u32>,
) -> Result<()> {
    if keep.is_empty() {
        sqlx::query("DELETE FROM beyond_char_skills WHERE uid = ?1 AND char_index = ?2")
            .bind(uid)
            .bind(char_index)
            .execute(&mut **tx)
            .await?;
        return Ok(());
    }
    let chunk_size: usize = 500;
    let ids: Vec<&str> = keep.keys().map(String::as_str).collect();
    for chunk in ids.chunks(chunk_size) {
        let placeholders = (0..chunk.len()).map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!(
            "DELETE FROM beyond_char_skills
             WHERE uid = ?1 AND char_index = ?2
               AND skill_id NOT IN ({placeholders})"
        );
        let mut q = sqlx::query(&sql).bind(uid).bind(char_index);
        for v in chunk {
            q = q.bind(*v);
        }
        q.execute(&mut **tx).await?;
    }
    Ok(())
}

async fn write_teams(tx: &mut Transaction<'_, Sqlite>, uid: &str, bag: &CharBag) -> Result<()> {
    for (i, team) in bag.teams.iter().enumerate() {
        sqlx::query(
            "INSERT INTO beyond_teams (uid, team_index, team_name, leader_char_index)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(uid, team_index) DO UPDATE SET
                team_name         = excluded.team_name,
                leader_char_index = excluded.leader_char_index",
        )
        .bind(uid)
        .bind(i as i64)
        .bind(&team.name)
        .bind(team.leader_index.as_usize() as i64)
        .execute(&mut **tx)
        .await?;

        for (slot_idx, slot) in team.char_team.iter().enumerate() {
            let char_idx: Option<i64> = slot.char_index().map(|c| c.as_usize() as i64);
            sqlx::query(
                "INSERT INTO beyond_team_slots
                    (uid, team_index, slot_index, char_index)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(uid, team_index, slot_index) DO UPDATE SET
                    char_index = excluded.char_index",
            )
            .bind(uid)
            .bind(i as i64)
            .bind(slot_idx as i64)
            .bind(char_idx)
            .execute(&mut **tx)
            .await?;
        }
    }

    // Prune teams that were removed.
    let team_keep: Vec<i64> = (0..bag.teams.len() as i64).collect();
    prune::prune_i64_pk(tx, "beyond_teams", uid, "team_index", &team_keep).await?;
    // Cascading FK from beyond_team_slots cleans those up.

    Ok(())
}

async fn write_weapons(tx: &mut Transaction<'_, Sqlite>, uid: &str, bag: &CharBag) -> Result<()> {
    let weapons = bag.item_manager.weapons.all_weapons();
    for w in weapons.values() {
        sqlx::query(
            "INSERT INTO beyond_weapons (
                uid, inst_id, template_id, exp, weapon_lv, refine_lv,
                breakthrough_lv, equip_char_id, attach_gem_id,
                is_lock, is_new, own_time
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(uid, inst_id) DO UPDATE SET
                template_id      = excluded.template_id,
                exp              = excluded.exp,
                weapon_lv        = excluded.weapon_lv,
                refine_lv        = excluded.refine_lv,
                breakthrough_lv  = excluded.breakthrough_lv,
                equip_char_id    = excluded.equip_char_id,
                attach_gem_id    = excluded.attach_gem_id,
                is_lock          = excluded.is_lock,
                is_new           = excluded.is_new,
                own_time         = excluded.own_time",
        )
        .bind(uid)
        .bind(w.inst_id.as_u64() as i64)
        .bind(&w.template_id)
        .bind(w.exp as i64)
        .bind(w.weapon_lv as i64)
        .bind(w.refine_lv as i64)
        .bind(w.breakthrough_lv as i64)
        .bind(w.equip_char_id as i64)
        .bind(w.attach_gem_id as i64)
        .bind(if w.is_lock { 1i64 } else { 0i64 })
        .bind(if w.is_new { 1i64 } else { 0i64 })
        .bind(w.own_time)
        .execute(&mut **tx)
        .await?;
    }

    let keep: Vec<i64> = weapons.keys().map(|k| k.as_u64() as i64).collect();
    prune::prune_i64_pk(tx, "beyond_weapons", uid, "inst_id", &keep).await?;
    Ok(())
}

async fn write_gems(tx: &mut Transaction<'_, Sqlite>, uid: &str, bag: &CharBag) -> Result<()> {
    let mut keep: Vec<i64> = Vec::with_capacity(bag.item_manager.gems.len());
    for g in bag.item_manager.gems.iter() {
        sqlx::query(
            "INSERT INTO beyond_gems (
                uid, inst_id, template_id, craft_slot, attach_weapon_id,
                is_lock, is_new, own_time
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(uid, inst_id) DO UPDATE SET
                template_id      = excluded.template_id,
                craft_slot       = excluded.craft_slot,
                attach_weapon_id = excluded.attach_weapon_id,
                is_lock          = excluded.is_lock,
                is_new           = excluded.is_new,
                own_time         = excluded.own_time",
        )
        .bind(uid)
        .bind(g.inst_id.as_u64() as i64)
        .bind(&g.template_id)
        .bind(g.craft_slot as u32 as i64)
        .bind(g.attach_weapon_id as i64)
        .bind(if g.is_lock { 1i64 } else { 0i64 })
        .bind(if g.is_new { 1i64 } else { 0i64 })
        .bind(g.own_time)
        .execute(&mut **tx)
        .await?;
        keep.push(g.inst_id.as_u64() as i64);
    }
    prune::prune_i64_pk(tx, "beyond_gems", uid, "inst_id", &keep).await?;
    Ok(())
}

async fn write_equips(tx: &mut Transaction<'_, Sqlite>, uid: &str, bag: &CharBag) -> Result<()> {
    let mut keep: Vec<i64> = Vec::with_capacity(bag.item_manager.equips.len());
    for e in bag.item_manager.equips.iter() {
        sqlx::query(
            "INSERT INTO beyond_equips (
                uid, inst_id, template_id, slot, equip_char_id,
                is_lock, is_new, own_time
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(uid, inst_id) DO UPDATE SET
                template_id   = excluded.template_id,
                slot          = excluded.slot,
                equip_char_id = excluded.equip_char_id,
                is_lock       = excluded.is_lock,
                is_new        = excluded.is_new,
                own_time      = excluded.own_time",
        )
        .bind(uid)
        .bind(e.inst_id.as_u64() as i64)
        .bind(&e.template_id)
        .bind(e.slot as u32 as i64)
        .bind(e.equip_char_id as i64)
        .bind(if e.is_lock { 1i64 } else { 0i64 })
        .bind(if e.is_new { 1i64 } else { 0i64 })
        .bind(e.own_time)
        .execute(&mut **tx)
        .await?;

        // Per-equip attrs: upsert by attr_index, then prune any
        // index past the current attrs length.
        for (attr_idx, attr) in e.attrs.iter().enumerate() {
            sqlx::query(
                "INSERT INTO beyond_equip_attrs (
                    uid, inst_id, attr_index, attr_type, modifier_type, modifier_value
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(uid, inst_id, attr_index) DO UPDATE SET
                    attr_type      = excluded.attr_type,
                    modifier_type  = excluded.modifier_type,
                    modifier_value = excluded.modifier_value",
            )
            .bind(uid)
            .bind(e.inst_id.as_u64() as i64)
            .bind(attr_idx as i64)
            .bind(attr.attr_type as i64)
            .bind(attr.modifier_type as i64)
            .bind(attr.modifier_value)
            .execute(&mut **tx)
            .await?;
        }
        prune::prune_tail(
            tx,
            "beyond_equip_attrs",
            uid,
            "inst_id",
            e.inst_id.as_u64() as i64,
            "attr_index",
            e.attrs.len(),
        )
        .await?;

        keep.push(e.inst_id.as_u64() as i64);
    }

    prune::prune_i64_pk(tx, "beyond_equips", uid, "inst_id", &keep).await?;
    // ON DELETE CASCADE on beyond_equip_attrs cleans the rest.

    Ok(())
}

async fn write_stackables(
    tx: &mut Transaction<'_, Sqlite>,
    uid: &str,
    bag: &CharBag,
) -> Result<()> {
    sync_stackable_depot(
        tx,
        uid,
        DEPOT_SPECIAL,
        bag.item_manager.special_items.all_counts(),
    )
    .await?;
    sync_stackable_depot(
        tx,
        uid,
        DEPOT_MISSION,
        bag.item_manager.mission_items.all_counts(),
    )
    .await?;
    sync_stackable_depot(
        tx,
        uid,
        DEPOT_FACTORY,
        bag.item_manager.factory_items.all_counts(),
    )
    .await?;
    Ok(())
}

/// Sync one stackable depot: upsert every (template_id, count) pair,
/// then prune any row in this depot whose `template_id` is no longer
/// present.
async fn sync_stackable_depot<'a, I>(
    tx: &mut Transaction<'_, Sqlite>,
    uid: &str,
    depot_type: i64,
    entries: I,
) -> Result<()>
where
    I: IntoIterator<Item = (&'a String, &'a u32)>,
{
    let entries: Vec<(&str, u32)> = entries.into_iter().map(|(k, v)| (k.as_str(), *v)).collect();

    for (template_id, count) in &entries {
        sqlx::query(
            "INSERT INTO beyond_stackable_items (uid, depot_type, template_id, count)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(uid, depot_type, template_id) DO UPDATE SET
                count = excluded.count",
        )
        .bind(uid)
        .bind(depot_type)
        .bind(*template_id)
        .bind(*count as i64)
        .execute(&mut **tx)
        .await?;
    }

    prune_stackable_depot(tx, uid, depot_type, &entries).await?;
    Ok(())
}

/// Per-depot prune. Scoped by `depot_type` so the IN-list contains
/// only that one depot's template ids.
async fn prune_stackable_depot(
    tx: &mut Transaction<'_, Sqlite>,
    uid: &str,
    depot_type: i64,
    keep: &[(&str, u32)],
) -> Result<()> {
    if keep.is_empty() {
        sqlx::query(
            "DELETE FROM beyond_stackable_items
             WHERE uid = ?1 AND depot_type = ?2",
        )
        .bind(uid)
        .bind(depot_type)
        .execute(&mut **tx)
        .await?;
        return Ok(());
    }
    let chunk_size: usize = 500;
    for chunk in keep.chunks(chunk_size) {
        let placeholders = (0..chunk.len()).map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!(
            "DELETE FROM beyond_stackable_items
             WHERE uid = ?1 AND depot_type = ?2
               AND template_id NOT IN ({placeholders})"
        );
        let mut q = sqlx::query(&sql).bind(uid).bind(depot_type);
        for (tid, _) in chunk {
            q = q.bind(*tid);
        }
        q.execute(&mut **tx).await?;
    }
    Ok(())
}

async fn write_chars_incremental(
    tx: &mut Transaction<'_, Sqlite>,
    uid: &str,
    bag: &CharBag,
) -> Result<()> {
    for &idx in bag.pending_chars().dirty() {
        let Some(ch) = bag.chars.get(idx) else {
            continue;
        };
        sqlx::query(
            "INSERT INTO beyond_chars (
                uid, char_index, template_id, level, exp, break_stage,
                is_dead, hp, ultimate_sp, own_time
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(uid, char_index) DO UPDATE SET
                template_id  = excluded.template_id,
                level        = excluded.level,
                exp          = excluded.exp,
                break_stage  = excluded.break_stage,
                is_dead      = excluded.is_dead,
                hp           = excluded.hp,
                ultimate_sp  = excluded.ultimate_sp,
                own_time     = excluded.own_time",
        )
        .bind(uid)
        .bind(idx as i64)
        .bind(&ch.template_id)
        .bind(ch.level as i64)
        .bind(ch.exp as i64)
        .bind(ch.break_stage as i64)
        .bind(if ch.is_dead { 1i64 } else { 0i64 })
        .bind(ch.hp)
        .bind(ch.ultimate_sp as f64)
        .bind(ch.own_time)
        .execute(&mut **tx)
        .await?;

        // A dirty char might have had its skill map changed too.
        // Skill changes are infrequent enough that re-upserting just
        // this one char's skills (and pruning anything past the current
        // map) is fine.
        for (skill_id, skill_lv) in &ch.skill_levels {
            sqlx::query(
                "INSERT INTO beyond_char_skills
                    (uid, char_index, skill_id, skill_level)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(uid, char_index, skill_id) DO UPDATE SET
                    skill_level = excluded.skill_level",
            )
            .bind(uid)
            .bind(idx as i64)
            .bind(skill_id)
            .bind(*skill_lv as i64)
            .execute(&mut **tx)
            .await?;
        }
        prune_char_skills_for_char(tx, uid, idx as i64, &ch.skill_levels).await?;
    }

    delete_chunked_i64(
        tx,
        "beyond_chars",
        uid,
        "char_index",
        bag.pending_chars().removed().iter().map(|&i| i as i64),
    )
    .await?;
    // ON DELETE CASCADE on beyond_char_skills cleans up orphan skills.
    Ok(())
}

async fn write_teams_incremental(
    tx: &mut Transaction<'_, Sqlite>,
    uid: &str,
    bag: &CharBag,
) -> Result<()> {
    for &idx in bag.pending_teams().dirty() {
        let Some(team) = bag.teams.get(idx) else {
            continue;
        };
        sqlx::query(
            "INSERT INTO beyond_teams (uid, team_index, team_name, leader_char_index)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(uid, team_index) DO UPDATE SET
                team_name         = excluded.team_name,
                leader_char_index = excluded.leader_char_index",
        )
        .bind(uid)
        .bind(idx as i64)
        .bind(&team.name)
        .bind(team.leader_index.as_usize() as i64)
        .execute(&mut **tx)
        .await?;

        for (slot_idx, slot) in team.char_team.iter().enumerate() {
            let char_idx: Option<i64> = slot.char_index().map(|c| c.as_usize() as i64);
            sqlx::query(
                "INSERT INTO beyond_team_slots
                    (uid, team_index, slot_index, char_index)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(uid, team_index, slot_index) DO UPDATE SET
                    char_index = excluded.char_index",
            )
            .bind(uid)
            .bind(idx as i64)
            .bind(slot_idx as i64)
            .bind(char_idx)
            .execute(&mut **tx)
            .await?;
        }
    }

    delete_chunked_i64(
        tx,
        "beyond_teams",
        uid,
        "team_index",
        bag.pending_teams().removed().iter().map(|&i| i as i64),
    )
    .await?;
    Ok(())
}

async fn write_weapons_incremental(
    tx: &mut Transaction<'_, Sqlite>,
    uid: &str,
    bag: &CharBag,
) -> Result<()> {
    let depot = &bag.item_manager.weapons;
    let pending = depot.pending();
    let weapons = depot.all_weapons();

    for inst_id in pending.dirty() {
        let Some(w) = weapons.get(inst_id) else {
            // Item was added and then removed in the same window. The
            // removed-set carries the DELETE; nothing to upsert.
            continue;
        };
        sqlx::query(
            "INSERT INTO beyond_weapons (
                uid, inst_id, template_id, exp, weapon_lv, refine_lv,
                breakthrough_lv, equip_char_id, attach_gem_id,
                is_lock, is_new, own_time
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(uid, inst_id) DO UPDATE SET
                template_id      = excluded.template_id,
                exp              = excluded.exp,
                weapon_lv        = excluded.weapon_lv,
                refine_lv        = excluded.refine_lv,
                breakthrough_lv  = excluded.breakthrough_lv,
                equip_char_id    = excluded.equip_char_id,
                attach_gem_id    = excluded.attach_gem_id,
                is_lock          = excluded.is_lock,
                is_new           = excluded.is_new,
                own_time         = excluded.own_time",
        )
        .bind(uid)
        .bind(w.inst_id.as_u64() as i64)
        .bind(&w.template_id)
        .bind(w.exp as i64)
        .bind(w.weapon_lv as i64)
        .bind(w.refine_lv as i64)
        .bind(w.breakthrough_lv as i64)
        .bind(w.equip_char_id as i64)
        .bind(w.attach_gem_id as i64)
        .bind(if w.is_lock { 1i64 } else { 0i64 })
        .bind(if w.is_new { 1i64 } else { 0i64 })
        .bind(w.own_time)
        .execute(&mut **tx)
        .await?;
    }

    delete_chunked_i64(
        tx,
        "beyond_weapons",
        uid,
        "inst_id",
        pending.removed().iter().map(|id| id.as_u64() as i64),
    )
    .await?;
    Ok(())
}

async fn write_gems_incremental(
    tx: &mut Transaction<'_, Sqlite>,
    uid: &str,
    bag: &CharBag,
) -> Result<()> {
    let depot = &bag.item_manager.gems;
    let pending = depot.pending();

    for inst_id in pending.dirty() {
        let Some(g) = depot.get(*inst_id) else {
            continue;
        };
        sqlx::query(
            "INSERT INTO beyond_gems (
                uid, inst_id, template_id, craft_slot, attach_weapon_id,
                is_lock, is_new, own_time
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(uid, inst_id) DO UPDATE SET
                template_id      = excluded.template_id,
                craft_slot       = excluded.craft_slot,
                attach_weapon_id = excluded.attach_weapon_id,
                is_lock          = excluded.is_lock,
                is_new           = excluded.is_new,
                own_time         = excluded.own_time",
        )
        .bind(uid)
        .bind(g.inst_id.as_u64() as i64)
        .bind(&g.template_id)
        .bind(g.craft_slot as u32 as i64)
        .bind(g.attach_weapon_id as i64)
        .bind(if g.is_lock { 1i64 } else { 0i64 })
        .bind(if g.is_new { 1i64 } else { 0i64 })
        .bind(g.own_time)
        .execute(&mut **tx)
        .await?;
    }

    delete_chunked_i64(
        tx,
        "beyond_gems",
        uid,
        "inst_id",
        pending.removed().iter().map(|id| id.as_u64() as i64),
    )
    .await?;
    Ok(())
}

async fn write_equips_incremental(
    tx: &mut Transaction<'_, Sqlite>,
    uid: &str,
    bag: &CharBag,
) -> Result<()> {
    let depot = &bag.item_manager.equips;
    let pending = depot.pending();

    for inst_id in pending.dirty() {
        let Some(e) = depot.get(*inst_id) else {
            continue;
        };
        sqlx::query(
            "INSERT INTO beyond_equips (
                uid, inst_id, template_id, slot, equip_char_id,
                is_lock, is_new, own_time
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(uid, inst_id) DO UPDATE SET
                template_id   = excluded.template_id,
                slot          = excluded.slot,
                equip_char_id = excluded.equip_char_id,
                is_lock       = excluded.is_lock,
                is_new        = excluded.is_new,
                own_time      = excluded.own_time",
        )
        .bind(uid)
        .bind(e.inst_id.as_u64() as i64)
        .bind(&e.template_id)
        .bind(e.slot as u32 as i64)
        .bind(e.equip_char_id as i64)
        .bind(if e.is_lock { 1i64 } else { 0i64 })
        .bind(if e.is_new { 1i64 } else { 0i64 })
        .bind(e.own_time)
        .execute(&mut **tx)
        .await?;

        for (attr_idx, attr) in e.attrs.iter().enumerate() {
            sqlx::query(
                "INSERT INTO beyond_equip_attrs (
                    uid, inst_id, attr_index, attr_type, modifier_type, modifier_value
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(uid, inst_id, attr_index) DO UPDATE SET
                    attr_type      = excluded.attr_type,
                    modifier_type  = excluded.modifier_type,
                    modifier_value = excluded.modifier_value",
            )
            .bind(uid)
            .bind(e.inst_id.as_u64() as i64)
            .bind(attr_idx as i64)
            .bind(attr.attr_type as i64)
            .bind(attr.modifier_type as i64)
            .bind(attr.modifier_value)
            .execute(&mut **tx)
            .await?;
        }
        prune::prune_tail(
            tx,
            "beyond_equip_attrs",
            uid,
            "inst_id",
            e.inst_id.as_u64() as i64,
            "attr_index",
            e.attrs.len(),
        )
        .await?;
    }

    delete_chunked_i64(
        tx,
        "beyond_equips",
        uid,
        "inst_id",
        pending.removed().iter().map(|id| id.as_u64() as i64),
    )
    .await?;
    Ok(())
}

async fn write_stackables_incremental(
    tx: &mut Transaction<'_, Sqlite>,
    uid: &str,
    bag: &CharBag,
) -> Result<()> {
    write_one_stackable_incremental(tx, uid, DEPOT_SPECIAL, &bag.item_manager.special_items)
        .await?;
    write_one_stackable_incremental(tx, uid, DEPOT_MISSION, &bag.item_manager.mission_items)
        .await?;
    write_one_stackable_incremental(tx, uid, DEPOT_FACTORY, &bag.item_manager.factory_items)
        .await?;
    Ok(())
}

async fn write_one_stackable_incremental(
    tx: &mut Transaction<'_, Sqlite>,
    uid: &str,
    depot_type: i64,
    depot: &perlica_logic::item::StackableDepot,
) -> Result<()> {
    let pending = depot.pending();

    for template_id in pending.dirty() {
        let count = depot.count_of(template_id);
        if count == 0 {
            // Reached zero between mark_dirty and flush - normalize to
            // a delete so the row doesn't get re-written with count=0.
            sqlx::query(
                "DELETE FROM beyond_stackable_items
                 WHERE uid = ?1 AND depot_type = ?2 AND template_id = ?3",
            )
            .bind(uid)
            .bind(depot_type)
            .bind(template_id)
            .execute(&mut **tx)
            .await?;
            continue;
        }
        sqlx::query(
            "INSERT INTO beyond_stackable_items (uid, depot_type, template_id, count)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(uid, depot_type, template_id) DO UPDATE SET
                count = excluded.count",
        )
        .bind(uid)
        .bind(depot_type)
        .bind(template_id)
        .bind(count as i64)
        .execute(&mut **tx)
        .await?;
    }

    // Removed stackable rows are deleted explicitly (one DELETE per
    // template_id; the volume is tiny because consume() only marks the
    // template_ids that actually hit zero).
    for template_id in pending.removed() {
        sqlx::query(
            "DELETE FROM beyond_stackable_items
             WHERE uid = ?1 AND depot_type = ?2 AND template_id = ?3",
        )
        .bind(uid)
        .bind(depot_type)
        .bind(template_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn delete_chunked_i64(
    tx: &mut Transaction<'_, Sqlite>,
    table: &str,
    uid: &str,
    pk_col: &str,
    ids: impl IntoIterator<Item = i64>,
) -> Result<()> {
    let all: Vec<i64> = ids.into_iter().collect();
    if all.is_empty() {
        return Ok(());
    }
    let chunk_size: usize = 500;
    for chunk in all.chunks(chunk_size) {
        let placeholders = (0..chunk.len()).map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!("DELETE FROM {table} WHERE uid = ?1 AND {pk_col} IN ({placeholders})");
        let mut q = sqlx::query(&sql).bind(uid);
        for &v in chunk {
            q = q.bind(v);
        }
        q.execute(&mut **tx).await?;
    }
    Ok(())
}
