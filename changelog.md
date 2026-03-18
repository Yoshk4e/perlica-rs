## [Unreleased]

### Added

#### `lib/logic` — new modules
- **`lib/logic/src/bitset.rs`** — Introduced `BitsetType` enum (20 variants mirroring
  `Beyond.GEnums.BitsetType`) and `BitsetManager`, a typed `HashMap<BitsetType,
  HashSet<u32>>` with ergonomic per-type helpers (`mark_item_found`, `has_visited_area`,
  etc.). `BitsetManager` is `Serialize`/`Deserialize` and is now the canonical home of
  all boolean flag sets. The ad-hoc `HashMap<u32, HashSet<u32>>` on `Player` is gone.

- **`lib/logic/src/entity.rs`** — `EntityManager` tracks all live scene entities
  (`SceneEntity`) keyed by `u64` ID. Provides typed iterators for monsters /
  characters, an auto-incrementing monster ID allocator starting at 1000 (above
  character IDs), and a `clear()` to wipe state on scene transitions. `EntityKind`
  covers Character, Enemy, Npc, Interactive, Projectile, Creature.

- **`lib/logic/src/item.rs`** — Full weapon inventory system:
  - `WeaponInstId` — newtype wrapper around `u64` for weapon instance IDs.
  - `WeaponInstance` — per-instance state: template ID, exp, level, refinement,
    breakthrough, equip owner, gem slot, lock/new flags, timestamp.
  - `WeaponDepot` — owns all `WeaponInstance` records for a player; maintains a
    reverse `equipped_weapons: HashMap<char_id, WeaponInstId>` for O(1) lookups;
    exposes `add_weapon`, `equip_weapon`, `unequip_weapon`, `remove_weapon`,
    `add_exp`, `breakthrough`, `attach_gem`, `detach_gem`, `build_item_bag_sync`.
    Handles swap-equip (auto-unequips old weapon from both sides) atomically.

- **`lib/logic/src/movement.rs`** — `MovementManager` caches player position and
  rotation; initialized from `WorldState` on login via `from_world`; synced back to
  `WorldState` on disconnect via `sync_to_world`. Ensures last-known position is
  saved rather than the login spawn.

- **`lib/logic/src/scene.rs`** — `SceneManager` wraps scene transition logic,
  checkpoint tracking (`CheckpointInfo`), and revival mode (`RevivalMode`). Provides
  `handle_team_index_switch`, `handle_active_team_update`, and
  `handle_inactive_team_update` to produce the correct `ScLeaveView` / `ScEnterView`
  / `ScSelfInfo` notification triple on team changes.

#### `servers/game-server` — new handlers
- **`src/handlers/weapon.rs`** — Five new weapon command handlers, each delegating to
  the corresponding `CharBag` method:
  - `on_cs_weapon_puton` — swap-equips a weapon to a character.
  - `on_cs_weapon_add_exp` — feeds fodder weapons to gain exp/levels; fodder is
    consumed (removed from depot).
  - `on_cs_weapon_breakthrough` — advances breakthrough level if level cap reached.
  - `on_cs_weapon_attach_gem` — sockets a gem; detaches any existing gem first.
  - `on_cs_weapon_detach_gem` — removes the socketed gem and returns it to the bag.


#### `Config.toml` (project root)
- Added default runtime configuration template with `[server]`, `[assets]`,
  `[world_state]`, and `[default_team]` sections. Includes commented-out database
  presets (SQLite / PostgreSQL) and spawn-position reference comments for all maps.

---

### Changed

#### `lib/logic/src/character/char_bag.rs` — weapon model refactor + API expansion
- **Removed** `WeaponIndex` type; weapon references are now `WeaponInstId` from
  `item.rs`.
- **`Char`** fields `weapon_id: WeaponIndex` and `weapon_template_id: String` replaced
  with `cached_weapon_inst_id: Option<WeaponInstId>` (`#[serde(skip)]`) — the depot
  is the single source of truth; the cache avoids per-frame depot lookups.
- **`CharBag`** now owns `weapon_depot: WeaponDepot`; the depot is initialized in
  `CharBag::new()` and populated with one default weapon per character.
- `CharBag::new()` refactored: character creation and weapon assignment are now two
  separate passes, enabling the weapon depot to be fully built before equipping.
- `add_char` no longer takes `&BeyondAssets`; weapon assignment responsibility removed.
- Added `TeamSlot::object_id()` convenience method.
- Added public methods: `get_char_mut`, `get_char_by_objid`, `equip_weapon`,
  `unequip_weapon`, `get_equipped_weapon`, `char_bag_info`, `char_attrs`,
  `char_status`, `item_bag_sync` (the last four consolidated here from scattered
  locations).
- Added `validate_after_load()` — called by `PlayerDb::load` to repair any data
  inconsistencies after deserialization (mismatched weapon references, stale caches).
- Weapon inst_id used in `CharSyncState` is now sourced from the depot instead of
  being derived from the character's numeric ID.

#### `lib/config/src/weapon.rs`
- Added `get_breakthrough_template(&str) -> Option<&BreakthroughTemplate>`.
- Added `get_breakthrough_required_level(&str, u32) -> Option<u32>` — looks up the
  minimum character level required for a given breakthrough stage.

#### `lib/db/src/saves.rs`
- `PlayerRecord` gains three new `#[serde(default)]` fields: `bitsets: BitsetManager`,
  `checkpoint: Option<CheckpointInfo>`, `revival_mode: RevivalMode`. Old saves without
  these fields deserialize cleanly via the defaults.
- `PlayerDb::save` signature extended to `(uid, char_bag, world, bitsets, checkpoint,
  revival_mode)`.
- `PlayerDb::load` now calls `record.char_bag.validate_after_load()` after
  deserialization.
- Added detailed doc comment on `PlayerRecord` explaining what is saved and why.

#### `lib/logic/Cargo.toml`
- Added `common.workspace = true` dependency (needed for `common::time::now_ms`).

#### `lib/logic/src/lib.rs`
- Exported the five new modules: `bitset`, `entity`, `item`, `movement`, `scene`.

#### `servers/game-server/src/player/mod.rs` — major `Player` struct expansion
- `bitsets` field changed from `HashMap<u32, HashSet<u32>>` to `BitsetManager`.
- Added fields: `movement: MovementManager`, `scene: SceneManager`,
  `entities: EntityManager`.
- `Player::default()` now initialises `movement` from `WorldState` and creates a
  fresh `SceneManager` / `EntityManager`.
- `Player::on_login` initialises movement and scene from saved world state.
- Added helpers: `get_char_by_objid`, `get_char_by_objid_mut`, `get_leader_objid`.
- Replaced all informal/emoji inline comments with proper Rust doc comments; added
  full module-level architecture documentation.

#### `servers/game-server/src/handlers/bitset.rs`
- `BitsetType` enum removed from this file; imported from `perlica_logic::bitset`.
- Added `on_cs_bitset_add` handler — sets bits in `BitsetManager` and returns
  `ScBitsetAdd`.
- `push_bitsets` now serialises the actual persisted bits from `BitsetManager` instead
  of sending empty vectors for every type.
- Logging converted to positional format (`"key={}"` style).

#### `servers/game-server/src/handlers/char_bag.rs`
- `item_bag_sync()` call updated — assets argument removed (depot is self-contained).
- Added `push_char_status_for_ids(ctx, &[u64])` — targeted `ScCharSyncStatus` push
  for only the characters in a team after a team switch.
- Logging converted to positional format.

#### `servers/game-server/src/handlers/character.rs` — major expansion
- `on_cs_char_bag_set_team_leader` now validates the requested leader is actually a
  member of the target team before accepting; logs a warning if not.
- Added `on_cs_char_bag_set_curr_team_index` — switches active team; emits the
  leave/enter/self_info scene triple and pushes char status for new team members.
- Added `on_cs_char_bag_set_team` — replaces team slot composition; handles both
  active-team (full scene diff) and inactive-team (self_info only) paths.
- Added `on_cs_char_bag_set_team_name` — renames a team slot, returns empty name on
  invalid index.
- Added `on_cs_char_level_up` — increments character level up to break-stage cap;
  restores HP; pushes `ScSyncAttr` + `ScCharSyncLevelExp`.
- Added `on_cs_char_break` — advances `break_stage` if at level cap; pushes updated
  attrs.
- Added `on_cs_char_skill_level_up`, `on_cs_char_set_normal_skill`,
  `on_cs_char_set_team_skill`.

#### `servers/game-server/src/handlers/factory.rs`
- Registered all new handlers from `character.rs` and `weapon.rs` in the routing
  table.

#### `servers/game-server/src/handlers/login.rs`
- Login sequence now restores `bitsets`, `checkpoint`, and `revival_mode` from
  `PlayerRecord` after loading a save.

#### `servers/game-server/src/handlers/mod.rs`
- Added `pub mod weapon`.

#### `servers/game-server/src/handlers/movement.rs`
- Updated to delegate to `MovementManager`.

#### `servers/game-server/src/handlers/scene.rs`
- Updated to delegate to `SceneManager`.

#### `servers/game-server/src/net/session.rs`
- `logic_loop` extended save call to include `bitsets`, `checkpoint`, and
  `revival_mode`.
- Player position is now flushed from `MovementManager` into `WorldState` before
  saving (via `player.movement.sync_to_world`).
- All informal/emoji inline comments replaced with proper doc comments.

#### `servers/game-server/src/net/context.rs`
- Added full module-level and struct-level doc comments explaining purpose, usage,
  and lifetime semantics. No logic changes.

#### `servers/game-server/src/sconfig.rs`
- Fixed indentation: tab characters replaced with spaces.
