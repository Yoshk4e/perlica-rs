# Perlica RS

![sleep](assets/img/sleep.png)
![Rust](https://img.shields.io/badge/Rust-2024-000000?logo=rust&logoColor=white)
![License](https://img.shields.io/badge/License-AGPL3.0-yellow)
![Status](https://img.shields.io/badge/Status-Playable_Core-2E8B57)
<a href="https://discord.gg/AgrhKzhP"><img alt="Discord - Perlica Winery" src="https://img.shields.io/discord/1483939903753425056?label=Discord&logo=discord"></a>

## Features

### Core Systems
- Full phased login sequence (BaseData → Wallet → ItemBag → CharBag → Unlocks → Guides → Missions → CharAttrs → CharStatus → Factory → Bitsets → EnterScene)
- Character bag with multiple teams, team switching, skill levels, attributes, and normal/ultimate skills
- Character progression: level up via exp items, ascension / break stage, and skill level advancement.
- Complete weapon system: exp feeding from fodder weapons and stackable items , breakthrough levels, gem socket/unsocket, equip/unequip.
- Equipment system: head/body/ring slot equipping and unequipping, with per-character slot tracking.
- Gem depot, instanced weapon gems with socket binding tracked on both the weapon and gem side

### World & Scene
- Scene loading and full entity synchronisation on load finish.
- Dynamic radius-based monster spawning and despawning: enemies enter view within 80 u, leave beyond 100 u, with a 60-second respawn cooldown after being killed
- NPC and interactive entity packing on scene load, interactives and NPCs are sent to the client with their `DynamicParameter` property maps
- Scene teleport with intra- and inter-scene handling: inter-scene transitions send `ScEnterSceneNotify` / `ScLeaveSceneNotify`; intra-scene warps preserve level-script runtime state
- Revival system with campfire checkpoint persistence, revival modes (default, repatriate, checkpoint), and `CsSceneRevival` that revives dead characters at 50 % HP

### Mission & Guide (half broken state)
- `MissionManager` with quest objective tracking, multi-quest progression within a mission, and mission state transitions (processing → completed)
- Bootstrap: on first login the prologue mission (`mission_mai_e0m1`) is selected, or the first available mission if the prologue is absent from assets
- Mission tracking.
- All mission and guide state is persisted across sessions

### Movement & Authoritative Position
- Authoritative movement: only the team leader's position is tracked server-side; position triggers the dynamic entity visibility update each tick
- Position and rotation synced to `WorldState` before every save so the player respawns at their last known location

### Inventory & Economy
- Item bag sync on login and after modifications, depots 1 (weapons), 2 (gems), 3 (equips), 4 (special items), 5 (mission items); factory depot pushed separately
- Wallet: gold and diamond pushed unconditionally on every login (9,999,999 each)
- Factory context stub pushed on login and after every scene load (required for the client factory UI to initialise)

### Bitset & Unlock
- Bitset system covering all `BitsetType` values (FoundItem, Wiki, GotItem, AreaFirstView, LevelHaveBeen, and more), full sync on login
- All unlock systems pushed as fully unlocked on every login

### Persistence
- Bincode-based player saves (atomic write via tmp → rename) storing: `CharBag`, `WorldState`, `BitsetManager`, checkpoint, revival mode, `MissionManager`, `GuideManager`
- Automatic data validation after load, repairs mismatched weapon references and orphaned equip entries

### Administration
- **MUIP GM bridge** (`perlica-muip-server`): HTTP admin panel on port 8080 with endpoints `/muip/gm`, `/status`, `/api/players`; forwards commands to the game server over a local TCP socket
- **In-game GM commands** (via MUIP HTTP or the web panel):
  - `help` — list all commands
  - `heal [all|team]` — restore HP to full for all owned characters or the active team
  - `level <n>` — set the player's role level (1–100) live
  - `tp <scene> <x> <y> <z> [rot_y]` — teleport to any scene and position
  - `spawn <template> [x y z] [level] [entity_type]` — dynamically spawn a monster
  - `give weapon <template>` — add a weapon instance and sync the item bag
  - `kick [reason]` — disconnect the player


---

#### NOTE: Perlica RS is currently under active development
#### NOTE x2: contributions are always welcome, for that read the Contributing section on GitHub

## Getting started

### Requirements
- [Rust 1.85+](https://www.rust-lang.org/tools/install)

### Setup

#### a) Building from sources
```sh
git clone https://github.com/Yoshk4e/perlica-rs.git
cd perlica-rs
cargo build --bin perlica-config-server --release
cargo build --bin perlica-game-server --release
cargo build --bin perlica-muip-server --release
```

#### b) Using pre-built binaries

Download the latest release from the repository releases page and run the server binaries.

### Configuration

The server uses a single configuration file at the project root: `Config.toml` (auto-created from `servers/game-server/config.default.toml` on first run if absent).

Key sections:

| Section | Purpose |
|---|---|
| `[server]` | Network binding for the game server (default: `0.0.0.0:1337`) |
| `[assets]` | Path to your dumped JSON asset tables |
| `[world_state]` | New-player spawn scene, position, rotation, and role level |
| `[default_team]` | Starting team composition for new players (4 character IDs) |
| `[muip_gm]` | Local GM bridge the game server listens on (default: `127.0.0.1:2338`) |
| `[muip]` | MUIP HTTP server binding, auth token, and GM bridge address |

Example `Config.toml`:
```toml
[server]
host = "0.0.0.0"
port = 1337

[assets]
path = "assets"

[world_state]
role_level = 1
role_exp = 0
last_scene = "map01_lv001"
pos_x = 469.0
pos_y = 107.11
pos_z = 217.83
rot_x = 0.0
rot_y = 60.0
rot_z = 0.0

[default_team]
team = [
    "chr_0003_endmin",
    "chr_0013_aglina",
    "chr_0004_pelica",
    "chr_0009_azrila",
]

[muip_gm]
host = "127.0.0.1"
port = 2338
enabled = true

[muip]
host = "0.0.0.0"
port = 8080
token = "change-me"
gm_host = "127.0.0.1"
gm_port = 2338
```

Known good spawn positions:

| Scene | X | Y | Z |
|---|---|---|---|
| `map01_lv001` | 469.00 | 107.11 | 217.83 |
| `map01_lv002` | 414.41 | 29.53 | 4.01 |
| `map01_lv003` | 227.90 | 137.60 | 297.00 |
| `map01_lv004` | 469.00 | 107.11 | 217.83 |
| `map01_lv005` | 395.00 | 95.00 | 302.40 |
| `map01_lv006` | 687.00 | 68.00 | 120.00 |

### Running the server

All three binaries are needed for the full stack:

```sh
cargo run --bin perlica-config-server --release &
cargo run --bin perlica-game-server --release &
cargo run --bin perlica-muip-server --release &
```

Or with pre-built binaries:

```sh
./target/release/perlica-config-server &
./target/release/perlica-game-server &
./target/release/perlica-muip-server &
```

The config server must be reachable at `127.0.0.1:21041`. The game server listens on the address configured in `[server]`.

### Logging in

The server is compatible with the alpha client.
For more information consider joining the Discord server.

To connect to the local server, apply the provided [client patch](https://github.com/Yoshk4e/beyond-patch-universal) which replaces the server address so the client can communicate with local infrastructure.

PS. if you feel like disabling censorship use [xeon's client patch](https://git.xeondev.com/LR/C)

## Development notes

### Adding a command handler
1. Create (or extend) a handler function in `servers/game-server/src/handlers/<feature>.rs`.
2. Import the module in `handlers/mod.rs`.
3. Register the command in the `handlers!` macro in `servers/game-server/src/net/router.rs`:
   - Use `reply { CsMyCommand => feature::on_cs_my_command }` when the handler returns a direct response.
   - Use `no_reply { CsMyCommand => feature::on_cs_my_command }` for fire-and-forget updates and complete control over the wire.

### Error types

Each library crate has its own `error.rs`. Use the most specific variant available:

| Crate | Error enum | Key variants |
|---|---|---|
| `config` | `ConfigError` | `ReadFile`, `ParseJson`, `ReadDir`, `InvalidStructure` *(use instead of `Io(InvalidData, …)` for bad JSON shapes)*, `Io` |
| `db` | `DbError` | `CreateDir`, `ReadSave`, `Deserialize`, `Serialize`, `WriteTmp`, `Rename` |
| `logic` | `LogicError` | `NotFound`, `InvalidOperation`, `Insufficient` *(typed quantity error, use instead of string-formatted `InvalidOperation` for stackable-item shortfalls)*, `Config` |
| `game-server` | `ServerError` | `Config`, `Db`, `Logic`, `Io`, `Decode`, `ConfigRead`, `ConfigParse` |

**`LogicError::Insufficient { item_id, have, need }`** — use this instead of `InvalidOperation(format!("Insufficient …"))` whenever a stackable-item consume fails due to quantity. Callers can match on it without parsing the message string.

**`ConfigError::InvalidStructure { path, message }`** — use this for files that parse as valid JSON but whose top-level shape is wrong, instead of wrapping `std::io::Error::new(InvalidData, …)` inside `ConfigError::Io`.

### Other notes
- Logging starts at DEBUG level with an ANSI art startup banner.
- Player state is automatically validated after loading to repair inconsistencies.
- The login sequence is a phase state machine in `handlers/login.rs`. Adding a new phase requires an arm in `LoginPhase`, its `next()` transition, and a `push_*` call in `run_login_sequence`.
- Dynamic entity visibility uses `ENTER_RADIUS = 80.0` and `LEAVE_RADIUS = 100.0` constants defined in `logic/src/scene.rs`, updated on every leader movement packet.

For questions about the code, refer to the inline module documentation in the source code. Otherwise, join the Discord server.

## License

This project is licensed under the AGPL-3.0 license
