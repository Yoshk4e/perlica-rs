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

### Changed

#### `servers/game-server/src/handlers/scene` - split into modules
- The 600+ line `scene.rs` is now `scene/{mod,dialog,entity,level_script,load,revival,teleport}.rs`.
- No behaviour change, just easier to navigate.

#### `servers/game-server/src/handlers/character` - split into modules
- `character.rs` is now `character/{mod,battle,progression,skill,team}.rs`.

#### `servers/game-server/src/handlers/weapon` - split into modules
- `weapon.rs` is now `weapon/{mod,exp,equip,breakthrough,gem}.rs`.

### Fixed

- **fix(equip)**: slot mapping, `suitinfo` computation, and attr loading on login.
- Removed leftover `to_xxx` helpers in `item.rs` superseded by the `From`/`Into` conversions introduced with the mail system.

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
