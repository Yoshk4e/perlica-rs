# Changelog

## [Unreleased]

### Added

#### `lib/config/src/equip.rs` and `lib/config/src/tables/equip.rs`
- New `EquipBasicTable` and `EquipAttrTable` config structs backed by `assets/tables/Equip.json` (~5.4k entries).

#### `servers/game-server/src/handlers/equip.rs` (expanded)
- Slot-aware puton/putoff flow with `partType -> CraftShowingType` mapping fixed.
- Already-equipped on the same character is now a no-op instead of an error.
- Previous-owner tracking on equip swaps.

#### `lib/logic/src/item.rs`
- `EquipDepot::compute_suitinfo()` for set-bonus computation.

#### `lib/logic/src/interest.rs` (new module)
- Multi-tiered entity replication / interest manager.  Four concentric zones (Immediate / Combat / Distant / Background) at 40 / 80 / 150 / 250 wu enter radii, each with its own check-frequency (16 / 50 / 160 / 500 ms) and hysteresis-based leave radii (now +60 wu, was +25).
- `StreamBucket` enum (`Enemy` / `Interactive` / `Npc`) with per-bucket max-zone, per-tick spawn-budget, and concurrent-cap config:
    - `ENEMY_MAX_ZONE = Distant`, `ENEMY_SPAWN_BUDGET_PER_TICK = 6`, `ENEMY_CONCURRENT_CAP = 64`.
    - `INTERACTIVE_MAX_ZONE = Combat`, `INTERACTIVE_SPAWN_BUDGET_PER_TICK = 8`, `INTERACTIVE_CONCURRENT_CAP = 80`.
    - `NPC_MAX_ZONE = Combat`, `NPC_SPAWN_BUDGET_PER_TICK = 4`, `NPC_CONCURRENT_CAP = 16`.
- Adaptive query radius: `max_due_radius_for(max_zone)` returns the world-space radius of the outermost *currently due* zone clamped by per-kind cap, so most ticks query 40 wu instead of 275.
- O(1) `live_count: [usize; 3]` per bucket maintained alongside `entries`; `at_capacity(bucket)` is a single array index.
- Fast-mover predictive-radius bonus (+40 wu) when the player's EMA-tracked speed exceeds 20 wu/s.
- Inlined FxHash-style `BuildHasher` for `u64` keys.
- Height-band occlusion heuristic (`is_occluded`) for Zone 0 with a per-tick `OcclusionCache` (16 ms TTL).
- Always-resident entries via `ghost_in_resident()`: TPs, save points, dungeon entries, blockages, and doors are exempt from leave-radius and orphan-sweep passes.
- Open-world combat-stickiness: `last_close_ms` per entry, bumped automatically by `update_zone` / `touch_or_classify` / `ghost_in` whenever the entity is observed in Combat zone or closer.  `should_retain` keeps recently-engaged enemies (within 5 s of last close sighting) inside `COMBAT_STICKY_MAX_RADIUS = 200 wu` even past the normal leave radius - works without any client-side `in_battle` flag, which was confirmed dungeon-only.  Dungeon `in_battle = true` still triggers the same path as a fallback.
- 30+ unit tests covering zone scheduling, adaptive radius, fast-mover bonus, FxHasher distribution, capacity counters, time-based stickiness expiry, hard-cap release, resident retention, and the height-band occlusion heuristic.

#### `lib/logic/src/spatial.rs` (new module)
- 2-D XZ spatial grid with bucketed `query_radius_indices`.  One grid per streamed bucket inside `SceneCache`; rebuilt once per scene transition.

### Changed

#### `servers/game-server/src/handlers/scene` - split into modules
- The 600+ line `scene.rs` is now `scene/{mod,dialog,entity,level_script,load,revival,teleport}.rs`.
- No behaviour change, just easier to navigate.

#### `servers/game-server/src/handlers/character` - split into modules
- `character.rs` is now `character/{mod,battle,progression,skill,team}.rs`.

#### `servers/game-server/src/handlers/weapon` - split into modules
- `weapon.rs` is now `weapon/{mod,exp,equip,breakthrough,gem}.rs`.

#### `lib/logic/src/scene.rs` - streaming scene manager
- Replaced bulk-spawn at scene load with proximity-based streaming for enemies, interactives, *and* NPCs.  `finish_scene_load` and `handle_revival` no longer dump every interactive in the map (often 200+ on map01_lv001) - the streamer fills the visible set in the next few ticks.
- Single `SceneCache` now builds three spatial grids (enemies / interactives / NPCs) at 50 wu cell size.
- `update_visible_entities` rewritten as three streaming helpers (`stream_enemies` / `stream_interactives` / `stream_npcs`) sharing the same skeleton: clamped query radius -> 3-D distance check -> capped zone classify -> single-probe `touch_or_classify` -> spawn-budget gate -> capacity gate -> ghost-in.
- Unified ghost-out pass walks `interest.entries` directly instead of `entities.monsters()`, saving one HashMap probe per ghosted-in entity.  Honours each entry's own zone leave radius plus the new sticky retention paths.
- Reusable scratch buffers (`candidates_buf`, `leave_ids_buf`) owned by `SceneManager` - zero allocation in steady state on the per-tick hot path.
- New always-resident classifier `is_always_resident_interactive(template_id, entity_type)` with a substring pattern list: `int_campfire`, `int_teleport_zone`, `int_save_{point,group}`, `int_dungeon_entry`, `int_barrierwall_*`, `int_edoor_*`, plus generic `_tp_` / `checkpoint` / `repatriate` / `levelgate` / `locked_door` / `blockage` patterns.  Sample data classification: 36 residents (TPs, saves, doors, barriers) sent at scene load; 154 streamed (chests, pickups, breakables, switches, etc.); 22 already filtered by `defaultHide` at config load.
- `pack_resident_interactives` builds the resident subset for the initial `ScObjectEnterView` / `ScSelfSceneInfo`; `install_resident_interactives` mirrors them into `EntityManager` + `InterestManager` so the streamer / leave-pass recognise them as already present.
- New `SceneManager::on_entity_killed(level_logic_id)` and `on_entity_despawned(level_logic_id)` so kill / destroy handlers can keep the interest counter in sync atomically.

#### `lib/logic/src/entity.rs`
- Added `interactives()` and `npcs()` iterator helpers, symmetric with the existing `monsters()` and `characters()`.

#### `servers/game-server/src/handlers/scene/revival.rs`
- `on_cs_scene_kill_monster` now calls `SceneManager::on_entity_killed` so the interest map and bucket `live_count` stay in sync with `EntityManager` after a kill.

#### `servers/game-server/src/handlers/scene/entity.rs`
- `on_cs_scene_destroy_entity` likewise routes through `on_entity_killed` / `on_entity_despawned` depending on `EntityKind`.

### Fixed

- **fix(equip)**: slot mapping, `suitinfo` computation, and attr loading on login.
- Removed leftover `to_xxx` helpers in `item.rs` superseded by the `From`/`Into` conversions introduced with the mail system.
- **fix(scene)**: monster disappearence mid-fight.  Two compounding causes were addressed:
    - The kill-monster handler removed the entity from `EntityManager` but never cleared the matching interest entry, so `live_count[Enemy]` stayed inflated for up to 500 ms (one Background tick).  Once the cap was reached, the streamer refused to refresh the active fight.  Now atomically cleaned via `on_entity_killed` from both kill / destroy paths.
    - The previous +25 wu hysteresis was too tight for active combat - a single laggy movement packet past the Combat leave radius would evict an actively-fought enemy.  Hysteresis bumped to +60 wu and the new time-based stickiness keeps recently-engaged enemies retained inside 200 wu for 5 s after the last sighting at Combat range.
- **fix(scene)**: scene-load FPS hitch.  Interactives are no longer bulk-spawned on scene entry / revival - only navigation-critical residents (TPs, blockages, doors, save points, dungeon entries) are sent up-front. 

---

## [0.2.0] - 2026-04-13

### Added

#### Mail system (`lib/logic/src/mail.rs`, `servers/game-server/src/handlers/mail.rs`)
- `StoredMail` and `MailManager` with expiry, attachment state, and CRUD ops.
- Canned welcome and login-greeting mail factories.
- `push_mail_sync`, `deliver_login_mails`, plus handlers for get/read/delete/claim mail and attachments.
- `LoginPhase` now has a `Mail` stage between `Bitsets` and `EnterScene` that pushes `ScSyncAllMail` and delivers welcome mail for new players or greeting mail for returning ones.
- `Player` stores a `MailManager` and a transient `is_new_player` flag set during login from whether the DB returned an existing record.
- `PlayerRecord` and `PlayerRecordRef` persist `MailManager` with `serde(default)` so older saves still load.

#### GM console / MUIP (`lib/muip/`, `servers/game-server/src/{gm.rs,handlers/gm.rs}`)
- New `lib/muip` crate and a 365-line `handlers/gm.rs` providing live testing commands for the various game systems.
- `assets/tables/Index.json` (~1.6k entries) added to support GM lookups.
- New `perlica-muip-server` binary mentioned in the README install steps.

#### Mission and Guide systems
- `MissionManager` and `GuideManager` added to `PlayerRecord` and `Player`.
- New `lib/config/src/mission.rs` config schema.
- Mission/guide command handlers wired into the game-server router.
- Locale tables landed: `assets/tables/I18nTextTable_EN.json` and `assets/tables/TextTable.json`.

#### Item system rewrite (`lib/logic/src/item.rs`, `lib/config/src/item.rs`)
- `WeaponDepot` generalised into `ItemManager` covering weapons, gems, equip, and stackables.
- `WeaponInstance`, `GemInstance`, `EquipInstance`, `WeaponDepot`, `GemDepot`, `EquipDepot`, and `StackableDepot` now use idiomatic `From`/`Into` conversions instead of bespoke `to_xxx` helpers.
- `WeaponAttachGemArgs`, `WeaponDetachGemArgs`, `WeaponPutonArgs` convert to their matching `Sc*` proto messages via `From`.
- `WeaponInstId` converts to and from `u64`.
- `assets/tables/Item.json` (~19.6k entries) added.

#### Equip handler (`servers/game-server/src/handlers/equip.rs`)
- New equipment puton/putoff handler set and wallet handler.

#### Character const (`lib/config/src/character.rs`)
- `CharacterConst` for global leveling data.

#### Scene: dynamic visibility and respawn (`lib/logic/src/scene.rs`)
- Replaced full-scene monster spawning with radius-based dynamic visibility driven by `EnterView`/`LeaveView` events.
- 60s respawn cooldown for killed monsters keyed by `level_logic_id`.
- 80/100 unit hysteresis to prevent entity flickering at vision edges.
- Visibility checks integrated into authoritative movement and scene-load handlers.
- Per-level spawn data in `assets/level_data/map01_lv001_lv_data_sub01.json` with a new `lib/config/src/{level_data,tables/level_data}.rs` schema.

#### Level scripts (`lib/logic/src/level_script.rs`)
- New ~415 line module covering teleportation, entity lifecycle, and level script events.
- Packet validation relaxed to allow empty bodies for certain command types.

#### Save layer
- `PlayerRecordRef` introduced so `PlayerDb::save` doesn't have to clone the whole `PlayerRecord` to serialise.

#### Project meta
- `README.md` and `CONTRIBUTING.md` added.
- `LICENSE`: GNU AGPL v3.
- CI workflow `.github/workflows/rust.yml`: fmt + clippy + build + test on `ubuntu-latest` and `windows-latest`; release-mode prebuilts as artifacts; rolling `dev-<sha>` pre-release on every master push; tagged release with linux/windows binaries on `v*` tags.
- Discord link replaced with a permanent invite.
- `assets/img/sleep.png` added for README.

### Changed

#### Errors: `anyhow` → typed `thiserror`
- New error enums in `lib/config/src/error.rs`, `lib/db/src/error.rs`, `lib/logic/src/error.rs`.
- Call sites across `config/{character,id_to_str,level_data,skill,str_to_id,weapon}`, `db/saves`, `logic/{character/char_bag,item}`, and the game-server handlers updated.
- Added `InvalidStructure` (config) and `Insufficient` (logic) variants so callers can branch on missing items or bad JSON without string matching.

#### Entity IDs
- Arbitrary IDs for NPCs, enemies, and interactives removed; logic IDs are used directly so level scripts and inter-entity interactions trigger correctly.
- `EntityDestroyReason` changed from `Immediately` to `Dead`.

#### Scene: spawn data replaced
- `assets/tables/EnemySpawns.json` removed in favour of per-level `assets/level_data/` files (see Scene entry above).

#### Factory / scene handler
- Factory now sends needed dummy data plus interacts, NPCs, etc. so the map loads correctly client-side.
- `DynamicParam` parsing fixed.

#### Workspace
- `Cargo.toml` `[workspace.package].version` bumped to `0.2.0`.

### Fixed

- **Revival flow**: scene handler revival path corrected; factory route registered.
- **`charBag` team UI crash**: client crashed when `max_indexes` tail entries were omitted. Side effect: editing a whole team no longer reloads the entire scene.
- **`set_team` leader**: when `CsCharBagSetTeam` removed the current leader, `leader_index` was left pointing at a missing character, causing `move_leader_to_front` to silently no-op and the client to receive a `ScSelfSceneInfo` with a `leader_id` matching no character - triggering "Can not find main character in SC_SELF_SCENE_INFO". The first occupied slot is now promoted to leader whenever the existing one is removed.
- Infinite loading on The Hub; missing enemies added; enemy logic fixes (contributed by inkursion).
- All clippy warnings cleared.

---

## [0.1.0] - 2026-03-18

### Added

- Weapon depot (`WeaponDepot`, `WeaponInstance`) with experience, breakthrough, and gem attach/detach handlers.
- Scene system: entity manager, monster spawning, NPC and interactive entity support, authoritative movement.
- Bitset persistence: player progress flags saved to and restored from the DB on login/logout.
- Initial game-server handler set: scene load, revival, teleport, dialog, and entity interactions.
- Core `PlayerRecord` / `PlayerDb` save layer backed by an embedded database.
