-- Clause 20.6 -- factory persistence.
--
-- Fully normalized, no JSON blob columns (house rule). Table families:
--
--   1. topology  -- regions, nodes, meshes, props, components, slots, ports,
--                   scalars, connections. Must restore before machines: a
--                   machine is keyed by `node_id` and is unrestorable if the
--                   node it names does not exist.
--   2. machines  -- one table per subsystem, PK (uid, region, node_id), plus
--                   child tables for the item buffers and trade orders.
--   3. player-scoped -- manual work, character work, quickbars, STT.
--
-- `region` is the *wire* region name, which is also the key of
-- `FactoryManager.regions` and the `region` half of `MachineKey`. It is spelled
-- the same in every table so the composite foreign keys read straight across.
--
-- `MachineTiming` (`factory/tick.rs`) is five scalars -- start_tick, speed,
-- banked_progress, paused_at, update_tick -- and is inlined wherever one lives
-- rather than given its own table: a timing envelope has no identity apart
-- from the machine that owns it, and a join per machine per load would buy
-- nothing. `paused_at` is NULL for a running envelope; that is the same
-- `Option<Tick>` the model holds, not a sentinel.
--
-- Following `0001_initial_schema.sql`, child tables carry a composite foreign
-- key to their immediate parent rather than a second one to `beyond_players`:
-- the cascade chains through the parent, so only the top-level factory tables
-- reference the player row directly.

-- `current_region` is one scalar and belongs on the player row, next to
-- `curr_team_index` and `track_mission_id`.
ALTER TABLE beyond_players
    ADD COLUMN current_region TEXT NOT NULL DEFAULT 'test01';

-- ---------------------------------------------------------------------------
-- 1. Topology
-- ---------------------------------------------------------------------------

-- One row per `FactoryRegion`. The six `PowerBlackboard` scalars live here
-- rather than in their own table: the blackboard is a fixed-shape struct owned
-- by exactly one region, so a separate table would be a 1:1 join forever.
CREATE TABLE IF NOT EXISTS beyond_factory_regions (
    uid                 TEXT    NOT NULL REFERENCES beyond_players(uid) ON DELETE CASCADE,
    region              TEXT    NOT NULL,   -- wire name, e.g. 'test01'
    region_id           TEXT    NOT NULL,
    scene_name          TEXT    NOT NULL,
    level               INTEGER NOT NULL DEFAULT 1,
    next_node_id        INTEGER NOT NULL DEFAULT 3,
    -- PowerBlackboard
    bb_inventory_node_id INTEGER NOT NULL DEFAULT 1,
    bb_power_cost       INTEGER NOT NULL DEFAULT 0,
    bb_power_gen        INTEGER NOT NULL DEFAULT 0,
    bb_power_save_max   INTEGER NOT NULL DEFAULT 0,
    bb_power_save_current INTEGER NOT NULL DEFAULT 0,
    bb_is_stop_by_power INTEGER NOT NULL DEFAULT 0,   -- bool
    PRIMARY KEY (uid, region)
);

-- `FactoryRegion.production_totals`: lifetime produced count per item.
CREATE TABLE IF NOT EXISTS beyond_factory_region_production (
    uid      TEXT    NOT NULL,
    region   TEXT    NOT NULL,
    item_id  TEXT    NOT NULL,
    total    INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (uid, region, item_id),
    FOREIGN KEY (uid, region)
        REFERENCES beyond_factory_regions(uid, region) ON DELETE CASCADE
);

-- One row per placed `FactoryNode`. `NodeTransform` is flattened in: its
-- optional members each get their columns plus the NULL that means `None`
-- (`has_pos` is separate because a node legitimately sits at (0, 0), so NULL
-- x/y and origin x/y must stay distinguishable).
CREATE TABLE IF NOT EXISTS beyond_factory_nodes (
    uid                   TEXT    NOT NULL,
    region                TEXT    NOT NULL,
    node_id               INTEGER NOT NULL,
    node_type             INTEGER NOT NULL,   -- FCNodeType
    template_id           TEXT    NOT NULL,
    is_deactive           INTEGER NOT NULL DEFAULT 0,   -- bool
    -- NodeTransform
    direction             INTEGER NOT NULL DEFAULT 0,   -- FCDirection
    scene_name            TEXT    NOT NULL DEFAULT '',
    has_pos               INTEGER NOT NULL DEFAULT 0,   -- bool
    pos_x                 INTEGER NOT NULL DEFAULT 0,
    pos_y                 INTEGER NOT NULL DEFAULT 0,
    mesh_type             INTEGER,            -- FCMeshType; NULL = no mesh
    world_pos_x           REAL,               -- NULL trio = no world position
    world_pos_y           REAL,
    world_pos_z           REAL,
    world_rot_x           REAL,               -- NULL trio = no world rotation
    world_rot_y           REAL,
    world_rot_z           REAL,
    bc_port_in_x          INTEGER,            -- NULL triple = no inbound port
    bc_port_in_y          INTEGER,
    bc_port_in_direction  INTEGER,
    bc_port_out_x         INTEGER,            -- NULL triple = no outbound port
    bc_port_out_y         INTEGER,
    bc_port_out_direction INTEGER,
    -- InteractiveObject: NULL = none
    interactive_object_id INTEGER,
    PRIMARY KEY (uid, region, node_id),
    FOREIGN KEY (uid, region)
        REFERENCES beyond_factory_regions(uid, region) ON DELETE CASCADE
);

-- `Mesh.points`, ordered. `idx` is the position in the Vec, which is
-- load-bearing: the client reads points[0] and points[1] as opposite corners.
CREATE TABLE IF NOT EXISTS beyond_factory_node_mesh (
    uid      TEXT    NOT NULL,
    region   TEXT    NOT NULL,
    node_id  INTEGER NOT NULL,
    idx      INTEGER NOT NULL,
    x        INTEGER NOT NULL,
    y        INTEGER NOT NULL,
    PRIMARY KEY (uid, region, node_id, idx),
    FOREIGN KEY (uid, region, node_id)
        REFERENCES beyond_factory_nodes(uid, region, node_id) ON DELETE CASCADE
);

-- `FactoryNode.dynamic_property`, a `HashMap<FCPropertyKey, String>`.
--
-- One value column, not the typed trio the wire's
-- `ScdFactorySyncDynamicPropertyValue` implies: the internal model stores every
-- property as a `String` regardless of how the client interprets it, and three
-- typed columns would be storing a shape the model cannot produce. If the model
-- ever gains a typed property value, that is the migration that adds them.
CREATE TABLE IF NOT EXISTS beyond_factory_node_props (
    uid      TEXT    NOT NULL,
    region   TEXT    NOT NULL,
    node_id  INTEGER NOT NULL,
    prop_key INTEGER NOT NULL,   -- FCPropertyKey
    value    TEXT    NOT NULL DEFAULT '',
    PRIMARY KEY (uid, region, node_id, prop_key),
    FOREIGN KEY (uid, region, node_id)
        REFERENCES beyond_factory_nodes(uid, region, node_id) ON DELETE CASCADE
);

-- `FactoryNode.component_pos`, a `HashMap<FCComponentPos, u32>`.
--
-- Not derivable from the component list: `Cache` occupies the `Cache`,
-- `CacheIn1..4` and `CacheOut1..4` slots, so the slot a component answers to is
-- placement information the component type does not carry. Dropping this would
-- break every `component_pos.get(..)` lookup after a reload.
CREATE TABLE IF NOT EXISTS beyond_factory_node_component_pos (
    uid          TEXT    NOT NULL,
    region       TEXT    NOT NULL,
    node_id      INTEGER NOT NULL,
    pos_key      INTEGER NOT NULL,   -- FCComponentPos
    component_id INTEGER NOT NULL,
    PRIMARY KEY (uid, region, node_id, pos_key),
    FOREIGN KEY (uid, region, node_id)
        REFERENCES beyond_factory_nodes(uid, region, node_id) ON DELETE CASCADE
);

-- One row per `(component_id, FactoryComponent)` on a node. `ord` preserves the
-- `Vec` order, which the wire reproduces; `component_id` happens to ascend in
-- the bootstrap layout but nothing guarantees it in general.
CREATE TABLE IF NOT EXISTS beyond_factory_components (
    uid            TEXT    NOT NULL,
    region         TEXT    NOT NULL,
    node_id        INTEGER NOT NULL,
    component_id   INTEGER NOT NULL,
    ord            INTEGER NOT NULL DEFAULT 0,
    component_type INTEGER NOT NULL,   -- FCComponentType, = discriminant()
    PRIMARY KEY (uid, region, node_id, component_id),
    FOREIGN KEY (uid, region, node_id)
        REFERENCES beyond_factory_nodes(uid, region, node_id) ON DELETE CASCADE
);

-- Item buffers of the item-bearing components. Each such component owns exactly
-- one list, so `slot_key` alone identifies a slot within a component:
--
--   Inventory  -- `slot_key` is the `HashMap<u32, ItemSlot>` key (the inst id)
--   Cache / GridBox / Collector.items_round / BoxConveyor -- `slot_key` is the
--              index in the Vec
--
-- `belt_progress` is the `ConveyorItem.progress` fraction along a belt and is
-- NULL for every non-conveyor slot -- one nullable column rather than a second
-- near-identical table for the one variant that carries it.
CREATE TABLE IF NOT EXISTS beyond_factory_component_slots (
    uid           TEXT    NOT NULL,
    region        TEXT    NOT NULL,
    node_id       INTEGER NOT NULL,
    component_id  INTEGER NOT NULL,
    slot_key      INTEGER NOT NULL,
    item_id       TEXT    NOT NULL,
    count         INTEGER NOT NULL DEFAULT 0,
    inst_id       INTEGER NOT NULL DEFAULT 0,
    belt_progress REAL,
    PRIMARY KEY (uid, region, node_id, component_id, slot_key),
    FOREIGN KEY (uid, region, node_id, component_id)
        REFERENCES beyond_factory_components(uid, region, node_id, component_id)
        ON DELETE CASCADE
);

-- `CachePort` lists on `Cache`, `Selector` and `BusLoader`.
CREATE TABLE IF NOT EXISTS beyond_factory_component_ports (
    uid          TEXT    NOT NULL,
    region       TEXT    NOT NULL,
    node_id      INTEGER NOT NULL,
    component_id INTEGER NOT NULL,
    idx          INTEGER NOT NULL,
    pos_x        INTEGER NOT NULL,
    pos_y        INTEGER NOT NULL,
    direction    INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (uid, region, node_id, component_id, idx),
    FOREIGN KEY (uid, region, node_id, component_id)
        REFERENCES beyond_factory_components(uid, region, node_id, component_id)
        ON DELETE CASCADE
);

-- Per-variant scalars for the ~16 stateful `FactoryComponent` variants,
-- `field` being the Rust field name (`formula_id`, `in_power`, `power_cost`,
-- `start_tick`, ...).
--
-- One narrow table instead of sixteen near-empty ones. The trade is deliberate
-- and bounded: values stay in typed columns (never a serialized blob), the key
-- space is closed by `component.rs` rather than open-ended, and a new variant
-- costs no migration. A NULL in all three value columns is a present-but-empty
-- scalar, which is how `Option<Tick>: None` round-trips.
CREATE TABLE IF NOT EXISTS beyond_factory_component_scalars (
    uid          TEXT    NOT NULL,
    region       TEXT    NOT NULL,
    node_id      INTEGER NOT NULL,
    component_id INTEGER NOT NULL,
    field        TEXT    NOT NULL,
    int_value    INTEGER,
    float_value  REAL,
    str_value    TEXT,
    PRIMARY KEY (uid, region, node_id, component_id, field),
    FOREIGN KEY (uid, region, node_id, component_id)
        REFERENCES beyond_factory_components(uid, region, node_id, component_id)
        ON DELETE CASCADE
);

-- `FactoryRegion.connections`. `index` is the connection's own id from the
-- model, so it doubles as the key within a region.
CREATE TABLE IF NOT EXISTS beyond_factory_connections (
    uid             TEXT    NOT NULL,
    region          TEXT    NOT NULL,
    conn_index      INTEGER NOT NULL,
    connection_type INTEGER NOT NULL,   -- FCConnectionType
    port_type       INTEGER NOT NULL,   -- FCConnectionPortType
    node_id_a       INTEGER NOT NULL,
    node_id_b       INTEGER NOT NULL,
    PRIMARY KEY (uid, region, conn_index),
    FOREIGN KEY (uid, region)
        REFERENCES beyond_factory_regions(uid, region) ON DELETE CASCADE
);

-- ---------------------------------------------------------------------------
-- 2. Machines, keyed by (region, node_id) -- never by region alone, which
--    would collapse every machine of a type in a region onto one row.
-- ---------------------------------------------------------------------------

-- `ManufactureMachine`. The active recipe is inlined: `recipe_id` empty means
-- no recipe, in which case the timing quintet is unused.
CREATE TABLE IF NOT EXISTS beyond_factory_manufacture (
    uid                    TEXT    NOT NULL,
    region                 TEXT    NOT NULL,
    node_id                INTEGER NOT NULL,
    machine_id             TEXT    NOT NULL,
    building_level         INTEGER NOT NULL DEFAULT 1,
    set_count              INTEGER NOT NULL DEFAULT 0,
    round_cost_delta_scale REAL    NOT NULL DEFAULT 1.0,
    -- ActiveRecipe: recipe_id = '' means none
    recipe_id              TEXT    NOT NULL DEFAULT '',
    start_tick             INTEGER NOT NULL DEFAULT 0,
    speed                  INTEGER NOT NULL DEFAULT 0,
    banked_progress        INTEGER NOT NULL DEFAULT 0,
    paused_at              INTEGER,            -- NULL = running
    update_tick            INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (uid, region, node_id),
    FOREIGN KEY (uid, region)
        REFERENCES beyond_factory_regions(uid, region) ON DELETE CASCADE
);

-- `ManufactureMachine.outcome_buffer`, ordered.
CREATE TABLE IF NOT EXISTS beyond_factory_manufacture_buffer (
    uid     TEXT    NOT NULL,
    region  TEXT    NOT NULL,
    node_id INTEGER NOT NULL,
    idx     INTEGER NOT NULL,
    item_id TEXT    NOT NULL,
    count   INTEGER NOT NULL DEFAULT 0,
    inst_id INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (uid, region, node_id, idx),
    FOREIGN KEY (uid, region, node_id)
        REFERENCES beyond_factory_manufacture(uid, region, node_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS beyond_factory_processor (
    uid                TEXT    NOT NULL,
    region             TEXT    NOT NULL,
    node_id            INTEGER NOT NULL,
    machine_id         TEXT    NOT NULL,
    building_level     INTEGER NOT NULL DEFAULT 1,
    refine_points      INTEGER NOT NULL DEFAULT 0,
    last_recovery_tick INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (uid, region, node_id),
    FOREIGN KEY (uid, region)
        REFERENCES beyond_factory_regions(uid, region) ON DELETE CASCADE
);

-- `unlocked_formulas` and `unread_formulas` in one table: the two lists overlap
-- (a formula is unlocked *and* not yet read), so a row per formula with two
-- flags stores each id once. `ord` keeps each list's order.
CREATE TABLE IF NOT EXISTS beyond_factory_processor_formulas (
    uid         TEXT    NOT NULL,
    region      TEXT    NOT NULL,
    node_id     INTEGER NOT NULL,
    formula_id  TEXT    NOT NULL,
    ord         INTEGER NOT NULL DEFAULT 0,
    is_unlocked INTEGER NOT NULL DEFAULT 0,   -- bool
    is_unread   INTEGER NOT NULL DEFAULT 0,   -- bool
    PRIMARY KEY (uid, region, node_id, formula_id),
    FOREIGN KEY (uid, region, node_id)
        REFERENCES beyond_factory_processor(uid, region, node_id) ON DELETE CASCADE
);

-- `RecyclerMachine`. `has_product_timer` distinguishes `None` from a timer that
-- happens to sit at zero: an idle recycler and one that just started are
-- different states, and only the second may complete a round.
CREATE TABLE IF NOT EXISTS beyond_factory_recycler (
    uid               TEXT    NOT NULL,
    region            TEXT    NOT NULL,
    node_id           INTEGER NOT NULL,
    machine_id        TEXT    NOT NULL,
    building_level    INTEGER NOT NULL DEFAULT 1,
    accumulated_value INTEGER NOT NULL DEFAULT 0,
    has_product_timer INTEGER NOT NULL DEFAULT 0,   -- bool
    start_tick        INTEGER NOT NULL DEFAULT 0,
    speed             INTEGER NOT NULL DEFAULT 0,
    banked_progress   INTEGER NOT NULL DEFAULT 0,
    paused_at         INTEGER,            -- NULL = running
    update_tick       INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (uid, region, node_id),
    FOREIGN KEY (uid, region)
        REFERENCES beyond_factory_regions(uid, region) ON DELETE CASCADE
);

-- The hopper grid (`RecyclerMachine.materials`), ordered: it drains FIFO, so
-- the order is state, not presentation.
CREATE TABLE IF NOT EXISTS beyond_factory_recycler_materials (
    uid     TEXT    NOT NULL,
    region  TEXT    NOT NULL,
    node_id INTEGER NOT NULL,
    idx     INTEGER NOT NULL,
    item_id TEXT    NOT NULL,
    count   INTEGER NOT NULL DEFAULT 0,
    inst_id INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (uid, region, node_id, idx),
    FOREIGN KEY (uid, region, node_id)
        REFERENCES beyond_factory_recycler(uid, region, node_id) ON DELETE CASCADE
);

-- `RecyclerMachine.temp_storage`, capped at `recTempStorageLength`.
CREATE TABLE IF NOT EXISTS beyond_factory_recycler_products (
    uid     TEXT    NOT NULL,
    region  TEXT    NOT NULL,
    node_id INTEGER NOT NULL,
    idx     INTEGER NOT NULL,
    item_id TEXT    NOT NULL,
    count   INTEGER NOT NULL DEFAULT 0,
    inst_id INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (uid, region, node_id, idx),
    FOREIGN KEY (uid, region, node_id)
        REFERENCES beyond_factory_recycler(uid, region, node_id) ON DELETE CASCADE
);

-- `TradeMachine`. `order_timing` is not optional, so its quintet has no
-- presence flag. `active_contract` NULL is `None`, distinct from '' -- which the
-- model cannot produce today but the column should not conflate.
CREATE TABLE IF NOT EXISTS beyond_factory_trade (
    uid             TEXT    NOT NULL,
    region          TEXT    NOT NULL,
    node_id         INTEGER NOT NULL,
    machine_id      TEXT    NOT NULL,
    building_level  INTEGER NOT NULL DEFAULT 1,
    active_contract TEXT,               -- NULL = no contract selected
    start_tick      INTEGER NOT NULL DEFAULT 0,
    speed           INTEGER NOT NULL DEFAULT 0,
    banked_progress INTEGER NOT NULL DEFAULT 0,
    paused_at       INTEGER,            -- NULL = running
    update_tick     INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (uid, region, node_id),
    FOREIGN KEY (uid, region)
        REFERENCES beyond_factory_regions(uid, region) ON DELETE CASCADE
);

-- `TradeMachine.orders`. `inst_id` is the order's identity within the machine
-- and the handle the client cashes/deletes against, so it is the key rather
-- than a Vec index. `contract_id` and `cost_value_delta` are the order's own
-- provenance, not the machine's current contract.
CREATE TABLE IF NOT EXISTS beyond_factory_trade_orders (
    uid               TEXT    NOT NULL,
    region            TEXT    NOT NULL,
    node_id           INTEGER NOT NULL,
    inst_id           INTEGER NOT NULL,
    ord               INTEGER NOT NULL DEFAULT 0,
    order_id          TEXT    NOT NULL,
    accumulated_value INTEGER NOT NULL DEFAULT 0,
    contract_id       TEXT    NOT NULL DEFAULT '',
    cost_value_delta  REAL    NOT NULL DEFAULT 1.0,
    PRIMARY KEY (uid, region, node_id, inst_id),
    FOREIGN KEY (uid, region, node_id)
        REFERENCES beyond_factory_trade(uid, region, node_id) ON DELETE CASCADE
);

-- `TradeOrder.items_committed`: what the player has already paid toward an
-- order. Lost on reload, the order would silently reset to unpaid.
CREATE TABLE IF NOT EXISTS beyond_factory_trade_order_items (
    uid     TEXT    NOT NULL,
    region  TEXT    NOT NULL,
    node_id INTEGER NOT NULL,
    inst_id INTEGER NOT NULL,
    idx     INTEGER NOT NULL,
    item_id TEXT    NOT NULL,
    count   INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (uid, region, node_id, inst_id, idx),
    FOREIGN KEY (uid, region, node_id, inst_id)
        REFERENCES beyond_factory_trade_orders(uid, region, node_id, inst_id)
        ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS beyond_factory_workshop (
    uid            TEXT    NOT NULL,
    region         TEXT    NOT NULL,
    node_id        INTEGER NOT NULL,
    machine_id     TEXT    NOT NULL,
    building_level INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (uid, region, node_id),
    FOREIGN KEY (uid, region)
        REFERENCES beyond_factory_regions(uid, region) ON DELETE CASCADE
);

-- `SoilMachine`. The planted seed is inlined like manufacture's recipe:
-- `seed_item_id` empty means a fallow plot. `doodad_state` is server-side only
-- (the wire has no field for it) but is still real state and persists.
CREATE TABLE IF NOT EXISTS beyond_factory_soil (
    uid             TEXT    NOT NULL,
    region          TEXT    NOT NULL,
    node_id         INTEGER NOT NULL,
    machine_id      TEXT    NOT NULL,
    doodad_state    INTEGER NOT NULL DEFAULT 0,   -- 0=Empty, 1=Growing, 2=Mature
    -- ActiveRecipe: seed_item_id = '' means nothing planted
    seed_item_id    TEXT    NOT NULL DEFAULT '',
    start_tick      INTEGER NOT NULL DEFAULT 0,
    speed           INTEGER NOT NULL DEFAULT 0,
    banked_progress INTEGER NOT NULL DEFAULT 0,
    paused_at       INTEGER,            -- NULL = running
    update_tick     INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (uid, region, node_id),
    FOREIGN KEY (uid, region)
        REFERENCES beyond_factory_regions(uid, region) ON DELETE CASCADE
);

-- `repair_state`: `MachineKey -> repair_id`. One scalar per key, so the whole
-- map is this table.
CREATE TABLE IF NOT EXISTS beyond_factory_repair (
    uid       TEXT    NOT NULL,
    region    TEXT    NOT NULL,
    node_id   INTEGER NOT NULL,
    repair_id TEXT    NOT NULL,
    PRIMARY KEY (uid, region, node_id),
    FOREIGN KEY (uid, region)
        REFERENCES beyond_factory_regions(uid, region) ON DELETE CASCADE
);

-- ---------------------------------------------------------------------------
-- 3. Player-scoped state (one per player, not per region)
-- ---------------------------------------------------------------------------

-- `ManualWorkQueue.paused_at`. A row exists whenever the player has a manual
-- work state at all; the queue itself is in the child table.
CREATE TABLE IF NOT EXISTS beyond_factory_manual_work (
    uid       TEXT    NOT NULL REFERENCES beyond_players(uid) ON DELETE CASCADE PRIMARY KEY,
    paused_at INTEGER             -- NULL = running
);

-- Queue stacks in order. `ord` is the queue position: 0 is the head, the only
-- stack that accrues progress, so the order is load-bearing.
CREATE TABLE IF NOT EXISTS beyond_factory_manual_work_units (
    uid             TEXT    NOT NULL,
    ord             INTEGER NOT NULL,
    recipe_id       TEXT    NOT NULL,
    count           INTEGER NOT NULL DEFAULT 0,
    start_tick      INTEGER NOT NULL DEFAULT 0,
    speed           INTEGER NOT NULL DEFAULT 0,
    banked_progress INTEGER NOT NULL DEFAULT 0,
    paused_at       INTEGER,            -- NULL = running
    update_tick     INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (uid, ord),
    FOREIGN KEY (uid)
        REFERENCES beyond_factory_manual_work(uid) ON DELETE CASCADE
);

-- `CharacterWorkState.workers`. Keyed by `char_inst_id`: a character is punched
-- in to at most one slot, and the ops address workers by instance id.
CREATE TABLE IF NOT EXISTS beyond_factory_character_work (
    uid          TEXT    NOT NULL REFERENCES beyond_players(uid) ON DELETE CASCADE,
    char_inst_id INTEGER NOT NULL,
    region       TEXT    NOT NULL,
    work_slot    INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (uid, char_inst_id)
);

-- `CharacterWorker.skill_ids` -- the factory skills that worker contributes.
CREATE TABLE IF NOT EXISTS beyond_factory_character_work_skills (
    uid          TEXT    NOT NULL,
    char_inst_id INTEGER NOT NULL,
    skill_id     TEXT    NOT NULL,
    ord          INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (uid, char_inst_id, skill_id),
    FOREIGN KEY (uid, char_inst_id)
        REFERENCES beyond_factory_character_work(uid, char_inst_id) ON DELETE CASCADE
);

-- One row per quickbar page. Kept even though the page carries no state beyond
-- its type, so an all-empty page still exists on reload instead of silently
-- disappearing from `FactoryManager.quickbars`.
CREATE TABLE IF NOT EXISTS beyond_factory_quickbars (
    uid           TEXT    NOT NULL REFERENCES beyond_players(uid) ON DELETE CASCADE,
    quickbar_type INTEGER NOT NULL,   -- FCQuickBarType: 0=Inner, 1=Outer
    PRIMARY KEY (uid, quickbar_type)
);

-- Occupied quickbar slots. Empty slots are absent rather than stored as '': the
-- page is resized to `QUICKBAR_SIZE` on projection, so storing 32 rows per page
-- would persist padding.
CREATE TABLE IF NOT EXISTS beyond_factory_quickbar_slots (
    uid           TEXT    NOT NULL,
    quickbar_type INTEGER NOT NULL,
    slot_index    INTEGER NOT NULL,
    item_id       TEXT    NOT NULL,
    PRIMARY KEY (uid, quickbar_type, slot_index),
    FOREIGN KEY (uid, quickbar_type)
        REFERENCES beyond_factory_quickbars(uid, quickbar_type) ON DELETE CASCADE
);

-- `SttState.unlocked_nodes`.
CREATE TABLE IF NOT EXISTS beyond_factory_stt_nodes (
    uid     TEXT    NOT NULL REFERENCES beyond_players(uid) ON DELETE CASCADE,
    node_id TEXT    NOT NULL,   -- STT node id from the config table
    ord     INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (uid, node_id)
);

-- `SttState.visible_formulas`, the formula book. Named `_formulas` because the
-- list is both `unlocked` and `visible` on the wire.
CREATE TABLE IF NOT EXISTS beyond_factory_formulas (
    uid        TEXT    NOT NULL REFERENCES beyond_players(uid) ON DELETE CASCADE,
    formula_id TEXT    NOT NULL,
    ord        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (uid, formula_id)
);

-- ---------------------------------------------------------------------------
-- Indexes for the load path, which reads whole subtrees per (uid, region)
-- rather than by primary key. Tables whose PK already starts (uid, region, ..)
-- are covered by the PK index and need nothing here.
-- ---------------------------------------------------------------------------

CREATE INDEX IF NOT EXISTS idx_factory_nodes_uid
    ON beyond_factory_nodes (uid);
CREATE INDEX IF NOT EXISTS idx_factory_components_uid_region
    ON beyond_factory_components (uid, region);
CREATE INDEX IF NOT EXISTS idx_factory_component_slots_uid_region
    ON beyond_factory_component_slots (uid, region);
CREATE INDEX IF NOT EXISTS idx_factory_component_scalars_uid_region
    ON beyond_factory_component_scalars (uid, region);
CREATE INDEX IF NOT EXISTS idx_factory_trade_orders_uid_region
    ON beyond_factory_trade_orders (uid, region);
