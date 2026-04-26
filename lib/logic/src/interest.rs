//! # Entity Replication Radius System, Interest Management
//!
//! This module implements a **multi-tiered interest management** system for the
//! authoritative game server.  Instead of a single flat enter/leave radius, it
//! divides the world around each player into four concentric *replication zones*
//! and drives per-zone check frequencies.
//!
//! ## Zone layout (world units from player)
//!
//! ```text
//!   ╔═══════════════════════════════════════════════════════╗
//!   ║  Zone 3, Background  (0 – 250 wu)   500 ms tick       ║
//!   ║  ╔═══════════════════════════════════╗                ║
//!   ║  ║  Zone 2, Distant  (0 – 150 wu)  160 ms tick        ║
//!   ║  ║  ╔═════════════════════╗          ║                ║
//!   ║  ║  ║ Zone 1, Combat      ║ 50 ms    ║                ║
//!   ║  ║  ║ (0 – 80 wu)         ║          ║                ║
//!   ║  ║  ║  ╔═══════════╗      ║          ║                ║
//!   ║  ║  ║  ║ Zone 0    ║16ms  ║          ║                ║
//!   ║  ║  ║  ║ Immediate ║      ║          ║                ║
//!   ║  ║  ║  ║ 0–40 wu   ║      ║          ║                ║
//!   ║  ║  ║  ╚═══════════╝      ║          ║                ║
//!   ║  ║  ╚═════════════════════╝          ║                ║
//!   ║  ╚═══════════════════════════════════╝                ║
//!   ╚═══════════════════════════════════════════════════════╝
//! ```
//!
//! ## Hysteresis (anti-flicker)
//!
//! Each zone has an **enter radius** and a larger **leave radius**.  An entity
//! enters awareness at the enter radius and only leaves when it crosses the
//! leave radius (enter + 25 wu).  This eliminates the oscillation ("popping")
//! that occurs when a player stands right at a boundary.
//!
//! ## Fast-mover prediction
//!
//! The manager tracks the player's speed via an exponential moving average.
//! When the player moves faster than 20 wu/s the effective query radius is
//! expanded by 40 wu so entities are ghosted-in *before* they would otherwise
//! appear, hiding the latency of the spawn notification.
//!
//! ## Occlusion culling, height-band heuristic
//!
//! `is_occluded` applies a geometry-free occlusion approximation that is
//! correct for the elevation structure of Endfield's maps:
//!
//! | Elevation tier      | Typical Y range (wu) |
//! |---------------------|---------------------|
//! | Ground floor        | 99 – 102            |
//! | Raised structures   | 107 – 115           |
//! | Elevated platforms  | 128 – 132           |
//!
//! When the absolute vertical separation between the observer and an entity
//! exceeds [`HEIGHT_THRESHOLD`] AND the horizontal distance exceeds
//! [`HORIZ_MIN`], terrain is assumed to obstruct line-of-sight.  This catches
//! enemies on a completely different floor without any collision mesh.
//!
//! Results are cached per entity for the duration of one Zone 0 tick
//! (16 ms) inside [`OcclusionCache`], so each entity is evaluated at most
//! once per tick even if the spatial grid returns it in multiple cells.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

// Enter radii (world units) for each zone tier.
pub const ZONE_ENTER_RADII: [f32; 4] = [
    40.0,  // Zone 0, Immediate
    80.0,  // Zone 1, Combat
    150.0, // Zone 2, Distant
    250.0, // Zone 3, Background
];

pub const ZONE_ENTER_RADII_SQ: [f32; 4] = [40.0 * 40.0, 80.0 * 80.0, 150.0 * 150.0, 250.0 * 250.0];

// Leave radii (enter + hysteresis).  Entities leave only when they cross this,
// preventing oscillation when a player lingers at a zone boundary.
pub const ZONE_LEAVE_RADII: [f32; 4] = [
    100.0, // Zone 0 leave (40 + 60)
    140.0, // Zone 1 leave (80 + 60)
    210.0, // Zone 2 leave (150 + 60)
    310.0, // Zone 3 leave (250 + 60)
];

pub const ZONE_LEAVE_RADII_SQ: [f32; 4] =
    [100.0 * 100.0, 140.0 * 140.0, 210.0 * 210.0, 310.0 * 310.0];

//
// The single biggest source of FPS drop on the client was *bulk* spawning:
// scene-load dumped every interactive in the map (often hundreds) plus all
// in-range enemies in one network burst.  The fix is two-fold:
//
//   1.  Cap the *outermost zone* each entity kind is allowed to occupy.
//       Decorative entities never need to live in Zone 2 / Zone 3.
//   2.  Cap *concurrent* spawned counts and *per-tick* new spawns so even a
//       teleport into a dense area trickles in entities over a few hundred
//       milliseconds rather than freezing the renderer.
//
// All numbers are tuned conservatively, Bump if a specific scene needs more.

/// The outermost replication zone enemies are allowed to occupy.  Anything
/// further than this enter radius is never ghosted-in.
///
/// Default is `Distant` (150 wu) instead of the previous `Background`
/// (250 wu), the latter pre-streamed enemies a player would not reach for
/// several seconds, paying for AI/animation work that the client never
/// actually used.  Cuts the queried area from 250² → 150² = ~2.8× fewer
/// candidates even at the worst-case (all-zones-due) tick.
pub const ENEMY_MAX_ZONE: ReplicationZone = ReplicationZone::Distant;

/// The outermost replication zone interactives may occupy.
///
/// Interactives are chests, pickups, switches, mining nodes, the player
/// only cares about them when they are within audio / interaction range.
/// `Combat` (80 wu) gives them a healthy buffer over the typical interact
/// prompt distance (~5–10 wu) without forcing the client to render every
/// chest in the map.
pub const INTERACTIVE_MAX_ZONE: ReplicationZone = ReplicationZone::Combat;

/// The outermost replication zone NPCs may occupy.  Same logic as
/// interactives, NPCs are conversation targets, only relevant up close.
pub const NPC_MAX_ZONE: ReplicationZone = ReplicationZone::Combat;

/// Hard ceiling on the number of *new* spawns the streamer is allowed to
/// emit per kind, per tick.
///
/// When a player teleports or fast-travels into a dense area, the spatial
/// query can return dozens of candidates simultaneously.  Sending all of
/// them in one `ScObjectEnterView` causes the client to instantiate /
/// register every prefab in a single frame, a guaranteed hitch.  Capping
/// the per-tick burst trickles them in over the next handful of ticks,
/// trading a few hundred ms of pop-in for a smooth framerate.
pub const ENEMY_SPAWN_BUDGET_PER_TICK: usize = 6;
pub const INTERACTIVE_SPAWN_BUDGET_PER_TICK: usize = 8;
pub const NPC_SPAWN_BUDGET_PER_TICK: usize = 4;

/// Hard ceiling on the *concurrent* ghosted-in count per kind.  Once the
/// limit is hit, additional candidates are deferred until the player walks
/// away from existing spawns and they ghost out.  The ceilings are
/// deliberately tight, based on my findings fewer entities always render better.
///
/// The interactive cap must comfortably exceed the resident count or the
/// streamer will refuse to add any *streamed* interactives once residents
/// are installed.
pub const ENEMY_CONCURRENT_CAP: usize = 64;
pub const INTERACTIVE_CONCURRENT_CAP: usize = 80;
pub const NPC_CONCURRENT_CAP: usize = 16;

/// Minimum milliseconds between spatial checks for each zone.
/// Zones closer to the player are checked far more frequently.
pub const ZONE_TICK_MS: [u64; 4] = [
    16,  // Zone 0, ~every frame at 60 fps
    50,  // Zone 1, ~3 frames
    160, // Zone 2, ~10 frames
    500, // Zone 3, ~30 frames
];

// Human-readable zone labels., used in trace logs.
pub const ZONE_NAMES: [&str; 4] = ["Immediate", "Combat", "Distant", "Background"];

/// The absolute outermost leave radius.  Entities beyond this are always
/// removed by the orphan sweep.  Tracks Zone 3's leave radius.
pub const MAX_INTEREST_RADIUS: f32 = 310.0;

/// Squared maximum interest radius (avoids the multiply on the hot path).
pub const MAX_INTEREST_RADIUS_SQ: f32 = MAX_INTEREST_RADIUS * MAX_INTEREST_RADIUS;

/// Hard upper bound on enemy ghost-out distance during the
/// "recently-engaged" sticky window.
///
/// An enemy that has been within Combat range (80 wu) in the last
/// [`STICKY_GRACE_MS`] milliseconds is **never** ghosted-out while it
/// remains within this radius, regardless of zone hysteresis.  This
/// protects active fights from lag-spike position jumps, dash-knockback,
/// and kiting that would otherwise punch through the 60 wu
/// hysteresis buffer.
///
/// The cap exists so a stuck state can't pin every enemy on the map
/// forever, anything beyond 200 wu does ghost out even when the entity
/// was recently engaged.
pub const COMBAT_STICKY_MAX_RADIUS: f32 = 200.0;
pub const COMBAT_STICKY_MAX_RADIUS_SQ: f32 = COMBAT_STICKY_MAX_RADIUS * COMBAT_STICKY_MAX_RADIUS;

/// How long after the last time an enemy was inside Combat range (80 wu)
/// it remains "sticky", i.e. immune to ghost-out so long as it stays
/// within [`COMBAT_STICKY_MAX_RADIUS`].
///
/// 5 seconds is enough to absorb:
///   * One or two missed Zone 1 ticks (50 ms each) due to network jitter.
///   * A reasonable kite, a player sprinting at 20 wu/s for 5 s covers
///     100 wu, well inside the 200 wu sticky cap.
///
/// **Why this exists**: the client only sets `in_battle` for dungeon
/// encounters, in the open world the flag stays false, leaving open-world
/// enemies with the same disappearing-mid-fight bug.  This time-based
/// signal is set automatically by the streamer whenever an entity is
/// observed in or below Combat zone, so it works in both contexts.
pub const STICKY_GRACE_MS: u64 = 5_000;

/// Fast-mover speed threshold (wu/s)^2.  Above this the query radius is expanded.
const FAST_MOVER_SPEED_SQ: f32 = 20.0 * 20.0;

/// Extra radius added when the player is moving fast (world units).
/// Pre-ghosts entities ahead of the player to hide spawn latency.
const PREDICTIVE_RADIUS_BONUS: f32 = 40.0;

/// Minimum Y-axis separation (wu) for terrain occlusion to apply.
///
/// 12 wu sits safely above normal walkable slopes (a 12/20 ratio = 0.6 grade,
/// ~31 degrees) while catching genuine floor-to-floor transitions.
const HEIGHT_THRESHOLD: f32 = 12.0;

/// Minimum horizontal distance (wu) before height occlusion can apply.
///
/// At <= 20 wu horizontal range the player can almost certainly see the entity
/// regardless of Y, think of an enemy at the top of a nearby staircase.
const HORIZ_MIN: f32 = 20.0;
const HORIZ_MIN_SQ: f32 = HORIZ_MIN * HORIZ_MIN;

// Entity IDs are densely packed monotonically-increasing `u64`s.  The default
// `std::collections::HashMap` which is DoS-resistant but ~10×
// slower than necessary for trusted, pre-distributed integer keys.  We use a
// minimal FxHash-style hasher
//
// Inlining the hasher avoids pulling in `rustc-hash` / `ahash` as new
// dependencies

#[derive(Default)]
pub struct FxU64Hasher(u64);

impl Hasher for FxU64Hasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        // Generic byte path, only used by HashMap if a non-u64 key sneaks in.
        // We fall back to a slower XOR/multiply over chunks; correctness only.
        let mut h = self.0;
        for &b in bytes {
            h = h.rotate_left(5).wrapping_mul(0x517c_c1b7_2722_0a95) ^ (b as u64);
        }
        self.0 = h;
    }

    #[inline]
    fn write_u64(&mut self, n: u64) {
        // Single-multiply FxHash step, sufficient for evenly-distributed IDs.
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x517c_c1b7_2722_0a95);
    }

    #[inline]
    fn write_u32(&mut self, n: u32) {
        self.write_u64(n as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

// Type alias used by every interest-side `HashMap` keyed by entity ID.
pub type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FxU64Hasher>>;

/// The replication tier an entity currently occupies.
///
/// Variants are ordered innermost to outermost so comparisons are meaningful:
/// `Immediate < Combat < Distant < Background`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReplicationZone {
    /// 0, 40 wu: direct threat / interactive objects.  16 ms tick.
    Immediate = 0,
    /// 40, 80 wu: combat and audio range.  50 ms tick.
    Combat = 1,
    /// 80, 150 wu: visual horizon.  160 ms tick.
    Distant = 2,
    /// 150, 250 wu: background awareness.  500 ms tick.
    Background = 3,
}

impl ReplicationZone {
    // Classify a squared distance into the innermost applicable zone.
    #[inline]
    pub fn from_dist_sq(dist_sq: f32) -> Option<Self> {
        if dist_sq <= ZONE_ENTER_RADII_SQ[0] {
            Some(Self::Immediate)
        } else if dist_sq <= ZONE_ENTER_RADII_SQ[1] {
            Some(Self::Combat)
        } else if dist_sq <= ZONE_ENTER_RADII_SQ[2] {
            Some(Self::Distant)
        } else if dist_sq <= ZONE_ENTER_RADII_SQ[3] {
            Some(Self::Background)
        } else {
            None
        }
    }

    /// Same as `from_dist_sq` but rejects any classification beyond
    /// `max_zone`.  Used to enforce per-kind radius caps:
    /// interactives / NPCs cap at [`INTERACTIVE_MAX_ZONE`] (Combat) so they
    /// are never streamed at long range, regardless of how far the spatial
    /// grid query reaches.
    #[inline]
    pub fn from_dist_sq_capped(dist_sq: f32, max_zone: ReplicationZone) -> Option<Self> {
        let max_idx = max_zone.index();
        for i in 0..=max_idx {
            if dist_sq <= ZONE_ENTER_RADII_SQ[i] {
                // Safety: i is within 0..4 because max_idx is.
                return Some(match i {
                    0 => Self::Immediate,
                    1 => Self::Combat,
                    2 => Self::Distant,
                    _ => Self::Background,
                });
            }
        }
        None
    }

    /// Index into the zone config arrays (`ZONE_TICK_MS`, `ZONE_LEAVE_RADII`, etc.).
    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }

    /// Minimum milliseconds between checks for this zone.
    #[inline]
    pub fn tick_ms(self) -> u64 {
        ZONE_TICK_MS[self.index()]
    }

    /// Leave radius squared, entity is removed when its dist_sq exceeds this.
    #[inline]
    pub fn leave_radius_sq(self) -> f32 {
        ZONE_LEAVE_RADII_SQ[self.index()]
    }
}

/// Streaming buckets the interest manager tracks separately for budgeting.
///
/// Indexed into [`InterestManager::live_count`] / spawn-budget arrays.  Kept
/// disjoint from `EntityKind` so the interest module doesn't need to know
/// the full kind taxonomy, only which kinds are actively streamed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamBucket {
    Enemy = 0,
    Interactive = 1,
    Npc = 2,
}

impl StreamBucket {
    pub const COUNT: usize = 3;

    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }

    /// Concurrent ghosted-in cap for this bucket.
    #[inline]
    pub fn concurrent_cap(self) -> usize {
        match self {
            Self::Enemy => ENEMY_CONCURRENT_CAP,
            Self::Interactive => INTERACTIVE_CONCURRENT_CAP,
            Self::Npc => NPC_CONCURRENT_CAP,
        }
    }

    /// Per-tick spawn budget for this bucket.
    #[inline]
    pub fn spawn_budget(self) -> usize {
        match self {
            Self::Enemy => ENEMY_SPAWN_BUDGET_PER_TICK,
            Self::Interactive => INTERACTIVE_SPAWN_BUDGET_PER_TICK,
            Self::Npc => NPC_SPAWN_BUDGET_PER_TICK,
        }
    }

    /// Outermost zone this bucket is allowed to occupy.
    #[inline]
    pub fn max_zone(self) -> ReplicationZone {
        match self {
            Self::Enemy => ENEMY_MAX_ZONE,
            Self::Interactive => INTERACTIVE_MAX_ZONE,
            Self::Npc => NPC_MAX_ZONE,
        }
    }
}

/// Per-entity interest state kept inside `InterestManager`.
#[derive(Debug, Clone)]
pub struct InterestEntry {
    /// Current replication zone of the entity.
    pub zone: ReplicationZone,
    /// Which streaming bucket this entry belongs to (governs the
    /// concurrent-cap counter when it is removed).
    pub bucket: StreamBucket,
    /// `true` for entities that must persist for the duration of the scene
    /// regardless of player position (TPs, blockages).  Such entries are
    /// installed at scene-load and ignore the leave-radius / orphan sweep.
    pub always_resident: bool,
    /// Timestamp (ms) when this entity was last synchronised to the client.
    pub last_sync_ms: u64,
    /// Timestamp (ms) when this entity was last observed inside Combat
    /// range (80 wu).  Drives the open-world combat-stickiness signal:
    /// while `now - last_close_ms < STICKY_GRACE_MS` we treat the entity
    /// as recently engaged and refuse to ghost it out, even if its
    /// instantaneous distance briefly exceeds the zone leave radius.
    ///
    /// `0` means "never close", used as the default for entities ghosted
    /// in directly at Distant range.
    pub last_close_ms: u64,
}

/// Caches per-entity occlusion results for the duration of one Zone 0 tick.
///
/// The spatial grid is a conservative approximation, a single entity can appear
/// in several overlapping cells during one `update_visible_entities` call.
/// Without a cache each of those hits would recompute the same arithmetic.
/// With a 16 ms TTL the cache entries are always fresh when used and are
/// automatically evicted on the following tick without an explicit sweep.
///
/// ## Design note
///
/// The current heuristic (`height_band_occluded`) is pure arithmetic and the
/// cache overhead is arguably larger than its savings for small enemy counts.
/// The cache is retained as forward-compatibility scaffolding: if this module
/// is ever upgraded to a real AABB ray-cast, the eviction logic and call-site
/// integration are already in place and the hot path gains the most from caching.
#[derive(Debug, Clone, Default)]
struct OcclusionCache {
    /// entity_id to occluded boolean.
    results: FastMap<u64, bool>,
    /// Entries are valid for ticks where `now_ms <= valid_until_ms`.
    /// When `now_ms` crosses this boundary the map is cleared and the
    /// deadline is advanced by one Zone 0 tick interval.
    valid_until_ms: u64,
}

impl OcclusionCache {
    /// Retrieve a cached result.  Returns `None` if the cache is stale or the
    /// entity has never been evaluated this tick.
    #[inline]
    fn get(&self, entity_id: u64, now_ms: u64) -> Option<bool> {
        if now_ms <= self.valid_until_ms {
            self.results.get(&entity_id).copied()
        } else {
            None
        }
    }

    /// Store a result.  If the current timestamp has crossed the TTL boundary
    /// all previous entries are evicted first (O(n) amortised over the tick).
    #[inline]
    fn insert(&mut self, entity_id: u64, occluded: bool, now_ms: u64) {
        if now_ms > self.valid_until_ms {
            // New tick: evict stale entries and advance the deadline.
            self.results.clear();
            self.valid_until_ms = now_ms + ZONE_TICK_MS[0];
        }
        self.results.insert(entity_id, occluded);
    }

    fn clear(&mut self) {
        self.results.clear();
        self.valid_until_ms = 0;
    }
}

/// Tracks per-entity ghost-in/out state, per-zone check timing, the player's
/// velocity for predictive radius expansion, and Zone 0 occlusion results.
///
/// One instance lives inside `SceneManager` and is cleared on every scene
/// transition via `clear()`.
#[derive(Debug, Clone)]
pub struct InterestManager {
    /// Ghosted-in entities: entity_id to InterestEntry.
    pub entries: FastMap<u64, InterestEntry>,
    /// Live ghosted-in count per [`StreamBucket`].  Maintained alongside
    /// `entries` so we can enforce concurrent caps in O(1) without scanning
    /// either the interest map or `EntityManager`.
    live_count: [usize; StreamBucket::COUNT],
    /// When each zone last ran its spatial check.
    zone_last_check: [u64; 4],
    /// Cached result of the last `zones_due` call, used by helpers that
    /// need to know which zones are currently active without re-running the
    /// scheduler (which mutates `zone_last_check`).
    last_due_mask: u8,
    /// Previous position sample, used to compute the velocity EMA.
    last_pos: (f32, f32, f32),
    /// Timestamp of `last_pos`.
    last_pos_ms: u64,
    /// Exponential moving average of speed squared ((wu/s)^2).
    speed_sq_ema: f32,
    /// Per-tick occlusion result cache (Zone 0 only).
    occlusion_cache: OcclusionCache,
}

impl Default for InterestManager {
    fn default() -> Self {
        Self {
            entries: FastMap::default(),
            live_count: [0; StreamBucket::COUNT],
            zone_last_check: [0u64; 4],
            last_due_mask: 0,
            last_pos: (f32::MAX, f32::MAX, f32::MAX),
            last_pos_ms: 0,
            speed_sq_ema: 0.0,
            occlusion_cache: OcclusionCache::default(),
        }
    }
}

impl InterestManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Discard all state.  Must be called on every scene transition.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.live_count = [0; StreamBucket::COUNT];
        self.zone_last_check = [0u64; 4];
        self.last_due_mask = 0;
        self.last_pos = (f32::MAX, f32::MAX, f32::MAX);
        self.last_pos_ms = 0;
        self.speed_sq_ema = 0.0;
        self.occlusion_cache.clear();
    }

    /// Update the internal speed estimate from the player's current position.
    ///
    /// Should be called once per movement packet, *before* `zones_due`.
    pub fn update_velocity(&mut self, pos: (f32, f32, f32), now_ms: u64) {
        let dt_ms = now_ms.saturating_sub(self.last_pos_ms);
        // Only integrate samples in the 1 ms to 2 s window (guards teleports
        // and the very first call when last_pos_ms == 0).
        if dt_ms > 0 && dt_ms < 2_000 && self.last_pos_ms != 0 {
            let dt_s = dt_ms as f32 / 1_000.0;
            let dx = pos.0 - self.last_pos.0;
            let dy = pos.1 - self.last_pos.1;
            let dz = pos.2 - self.last_pos.2;
            let sample_sq = (dx * dx + dy * dy + dz * dz) / (dt_s * dt_s);
            // alpha = 0.3 smooths packet-level jitter while tracking sustained sprints.
            self.speed_sq_ema = self.speed_sq_ema * 0.7 + sample_sq * 0.3;
        }
        self.last_pos = pos;
        self.last_pos_ms = now_ms;
    }

    /// Returns which zones are due for a spatial check this tick.
    ///
    /// This is the primary scheduler: callers should call this once per update
    /// and skip the spatial scan for zones whose entry is `false`.  The
    /// timestamps are updated in-place so subsequent calls within the same tick
    /// window return all-false.
    ///
    /// Returns `[bool; 4]` for ergonomics; the same information is also stored
    /// internally as a bitmask (`last_due_mask`) for cheap re-querying via
    /// [`is_zone_due`] / [`max_due_radius_sq`].
    pub fn zones_due(&mut self, now_ms: u64) -> [bool; 4] {
        let mut due = [false; 4];
        let mut mask: u8 = 0;
        for i in 0..4 {
            if now_ms.saturating_sub(self.zone_last_check[i]) >= ZONE_TICK_MS[i] {
                due[i] = true;
                mask |= 1 << i;
                self.zone_last_check[i] = now_ms;
            }
        }
        self.last_due_mask = mask;
        due
    }

    /// `true` if the given zone was flagged due in the most recent
    /// `zones_due` call.  Cheaper than re-running the scheduler.
    #[inline]
    pub fn is_zone_due(&self, zone: ReplicationZone) -> bool {
        (self.last_due_mask >> zone.index()) & 1 != 0
    }

    /// Bitmask of zones flagged due in the most recent `zones_due` call.
    #[inline]
    pub fn due_mask(&self) -> u8 {
        self.last_due_mask
    }

    /// Squared enter-radius of the **outermost** zone currently due,
    /// **clamped** by `kind_max_zone`.
    ///
    /// The previous implementation always queried out to
    /// `MAX_INTEREST_RADIUS` (275 wu) regardless of which zones were due.
    /// This pass narrows it twice over:
    ///
    ///   1. Per-tick: outermost-due zone (Zone 0 most ticks → 40 wu).
    ///   2. Per-kind: clamp by `kind_max_zone` (interactives / NPCs cap at
    ///      Combat = 80 wu, never reach Distant or Background).
    ///
    /// Returns `0.0` when no zone within the clamp is currently due.
    pub fn max_due_radius_sq_for(&self, kind_max_zone: ReplicationZone) -> f32 {
        let r = self.max_due_radius_for(kind_max_zone);
        r * r
    }

    /// World-space query radius for the spatial grid this tick, clamped by
    /// the per-kind maximum.  Returns `0.0` when nothing within the clamp
    /// is due.
    pub fn max_due_radius_for(&self, kind_max_zone: ReplicationZone) -> f32 {
        let cap_idx = kind_max_zone.index();
        // Mask out any due bits beyond the per-kind cap.  A `1u8 << (cap+1)`
        // wraps if cap == 7, but our cap is always < 4 so the shift is safe.
        let allowed_bits = (1u8 << (cap_idx + 1)) - 1; // bits 0..=cap_idx
        let masked = self.last_due_mask & allowed_bits;
        if masked == 0 {
            return 0.0;
        }
        let outermost = 7 - masked.leading_zeros() as usize;
        debug_assert!(outermost <= cap_idx);

        let r = ZONE_ENTER_RADII[outermost];
        if self.speed_sq_ema > FAST_MOVER_SPEED_SQ {
            r + PREDICTIVE_RADIUS_BONUS
        } else {
            r
        }
    }

    /// Squared enter-radius of the outermost zone currently due, with no
    /// per-kind clamp.
    pub fn max_due_radius_sq(&self) -> f32 {
        self.max_due_radius_sq_for(ReplicationZone::Background)
    }

    /// World-space query radius for the spatial grid this tick, unclamped.
    pub fn max_due_radius(&self) -> f32 {
        self.max_due_radius_for(ReplicationZone::Background)
    }

    /// The full-tier spatial grid query radius (Zone 3 + predictive).
    ///
    /// **Prefer [`max_due_radius`] in the per-tick hot path**, it shrinks the
    /// query radius to whatever the outermost due zone needs, which is usually
    /// Zone 0 only.  This method is retained for callers that genuinely want
    /// the full interest sphere (e.g. global state dumps, diagnostics).
    pub fn effective_query_radius(&self) -> f32 {
        if self.speed_sq_ema > FAST_MOVER_SPEED_SQ {
            MAX_INTEREST_RADIUS + PREDICTIVE_RADIUS_BONUS
        } else {
            MAX_INTEREST_RADIUS
        }
    }

    /// Returns `true` when the player's speed EMA exceeds the threshold.
    /// Useful for diagnostics / future decisions.
    #[allow(dead_code)]
    pub fn is_fast_moving(&self) -> bool {
        self.speed_sq_ema > FAST_MOVER_SPEED_SQ
    }

    /// Register a newly ghosted-in entity, accounting it against its
    /// streaming bucket's live counter.
    #[inline]
    pub fn ghost_in(&mut self, id: u64, zone: ReplicationZone, bucket: StreamBucket, now_ms: u64) {
        self.ghost_in_inner(id, zone, bucket, now_ms, false);
    }

    /// Register a newly ghosted-in entity that must remain resident for the
    /// rest of the scene (TPs, blockages).
    ///
    /// Always-resident entries are still counted toward their bucket's
    /// concurrent cap (so the cap remains a hard upper bound on memory
    /// usage), but they are exempt from the leave-radius and orphan-sweep
    /// passes, only `clear()` removes them.
    #[inline]
    pub fn ghost_in_resident(
        &mut self,
        id: u64,
        zone: ReplicationZone,
        bucket: StreamBucket,
        now_ms: u64,
    ) {
        self.ghost_in_inner(id, zone, bucket, now_ms, true);
    }

    fn ghost_in_inner(
        &mut self,
        id: u64,
        zone: ReplicationZone,
        bucket: StreamBucket,
        now_ms: u64,
        always_resident: bool,
    ) {
        // Stamp `last_close_ms` only if the entity is actually being
        // observed in Combat range or closer.  Otherwise the engagement
        // signal would be triggered by a fly-by sighting at 200 wu, which
        // we don't want to count as combat.
        let last_close_ms = if zone <= ReplicationZone::Combat {
            now_ms
        } else {
            0
        };

        // If the id is already known we MUST NOT double-count.  Insert
        // returns the previous value if any , only bump the counter when we
        // actually added a fresh entry.  If the id was re-inserted under a
        // different bucket (rare migration case) the counter is moved.
        let prev = self.entries.insert(
            id,
            InterestEntry {
                zone,
                bucket,
                always_resident,
                last_sync_ms: now_ms,
                last_close_ms,
            },
        );
        match prev {
            None => self.live_count[bucket.index()] += 1,
            Some(p) if p.bucket != bucket => {
                let old = &mut self.live_count[p.bucket.index()];
                *old = old.saturating_sub(1);
                self.live_count[bucket.index()] += 1;
            }
            _ => {} // same bucket, no counter change
        }
    }

    /// Remove a ghosted-in entity from the interest set, decrementing its
    /// bucket's live counter.
    #[inline]
    pub fn ghost_out(&mut self, id: u64) -> Option<InterestEntry> {
        let removed = self.entries.remove(&id);
        if let Some(ref entry) = removed {
            let slot = &mut self.live_count[entry.bucket.index()];
            *slot = slot.saturating_sub(1);
        }
        removed
    }

    /// Number of currently ghosted-in entities in the given bucket.
    /// O(1) , no map traversal.
    #[inline]
    pub fn live_count(&self, bucket: StreamBucket) -> usize {
        self.live_count[bucket.index()]
    }

    /// Returns `true` when the bucket is at or above its concurrent cap.
    /// The streamer should refuse new ghost-ins for this bucket while
    /// `at_capacity` holds.
    #[inline]
    pub fn at_capacity(&self, bucket: StreamBucket) -> bool {
        self.live_count[bucket.index()] >= bucket.concurrent_cap()
    }

    /// Returns `true` if the entity should be **retained** despite its
    /// distance exceeding the normal zone leave-radius.
    ///
    /// Three cases qualify for retention:
    ///
    /// 1. **Always-resident**: entries flagged at ghost-in time as
    ///    permanent residents (TPs, blockages).  Their leave radius is
    ///    effectively infinite , they only ghost out on scene transition.
    ///
    /// 2. **Recently-engaged enemies (open world)**: any enemy whose
    ///    `last_close_ms` is within [`STICKY_GRACE_MS`] AND whose current
    ///    distance is below [`COMBAT_STICKY_MAX_RADIUS`].  This is the
    ///    primary fix for the disappearing-mid-fight bug , it works
    ///    without any client-side `in_battle` signal, which the user
    ///    confirmed is dungeon-only.
    ///
    /// 3. **Explicit battle flag (dungeons)**: when `in_battle == true`
    ///    and the entity is an enemy within `COMBAT_STICKY_MAX_RADIUS`.
    ///    Kept as a belt-and-braces signal for dungeon encounters where
    ///    the client does set the flag.
    ///
    /// Returns `false` for the normal case (use the zone's leave radius).
    #[inline]
    pub fn should_retain(
        &self,
        entry: &InterestEntry,
        dist_sq: f32,
        now_ms: u64,
        in_battle: bool,
    ) -> bool {
        if entry.always_resident {
            return true;
        }
        if entry.bucket != StreamBucket::Enemy {
            return false;
        }
        if dist_sq > COMBAT_STICKY_MAX_RADIUS_SQ {
            // Beyond the hard cap, release regardless of any signal.
            return false;
        }
        // Path A: explicit battle flag (set by client in dungeons).
        if in_battle {
            return true;
        }
        // Path B: time-based engagement signal (open world).  Anchored
        // on `last_close_ms` which the streamer bumps automatically every
        // tick the enemy is observed at Combat range or closer.
        if entry.last_close_ms > 0 && now_ms.saturating_sub(entry.last_close_ms) < STICKY_GRACE_MS {
            return true;
        }
        false
    }

    /// Reclassify an existing entry into a new zone (called when the player
    /// moves closer to or further from a ghosted-in entity).  No-op if the
    /// entity isn't ghosted-in.
    ///
    /// `now_ms` lets us refresh the engagement-stickiness timestamp when
    /// the entity is observed in Combat zone or closer.
    #[inline]
    pub fn update_zone(&mut self, id: u64, zone: ReplicationZone, now_ms: u64) {
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.zone = zone;
            if zone <= ReplicationZone::Combat {
                entry.last_close_ms = now_ms;
            }
        }
    }

    /// Returns `true` when the entity was already known and its zone was
    /// updated (or already correct).  Returns `false` when the entity is not
    /// ghosted-in , meaning the caller should run the ghost-in path.
    ///
    /// Also refreshes `last_close_ms` when the new zone is Combat or
    /// closer , driving the open-world stickiness signal automatically
    /// without any external "in combat" flag from the client.
    #[inline]
    pub fn touch_or_classify(&mut self, id: u64, zone: ReplicationZone, now_ms: u64) -> bool {
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.zone = zone;
            if zone <= ReplicationZone::Combat {
                entry.last_close_ms = now_ms;
            }
            true
        } else {
            false
        }
    }

    /// Current zone of a ghosted-in entity, or `None` if not in the set.
    #[inline]
    pub fn zone_of(&self, id: u64) -> Option<ReplicationZone> {
        self.entries.get(&id).map(|e| e.zone)
    }

    /// Returns `true` if the entity is currently ghosted-in.
    #[inline]
    pub fn is_ghosted_in(&self, id: u64) -> bool {
        self.entries.contains_key(&id)
    }

    /// Iterator over every ghosted-in entity (id, zone).  Used by the
    /// ghost-out pass so it doesn't need to walk the EntityManager and
    /// re-resolve each id.
    #[inline]
    pub fn iter_entries(&self) -> impl Iterator<Item = (u64, &InterestEntry)> {
        self.entries.iter().map(|(&id, e)| (id, e))
    }

    /// Returns `true` when the entity at `entity_pos` is considered occluded
    /// from `observer_pos` by terrain.  See module docs for the heuristic.
    pub fn is_occluded(
        &mut self,
        entity_id: u64,
        observer_pos: (f32, f32, f32),
        entity_pos: (f32, f32, f32),
        now_ms: u64,
    ) -> bool {
        // Fast path: cache hit from earlier in this same tick.
        if let Some(cached) = self.occlusion_cache.get(entity_id, now_ms) {
            return cached;
        }

        // Slow path: compute and store.
        let occluded = Self::height_band_occluded(observer_pos, entity_pos);
        self.occlusion_cache.insert(entity_id, occluded, now_ms);

        if occluded && tracing::enabled!(tracing::Level::TRACE) {
            tracing::trace!(
                entity_id,
                observer_y = observer_pos.1,
                entity_y = entity_pos.1,
                dy = (entity_pos.1 - observer_pos.1).abs(),
                "entity occluded by terrain (height-band heuristic)",
            );
        }

        occluded
    }

    /// Pure geometric predicate , separated from the cache so it can be
    /// unit-tested and benchmarked independently.
    fn height_band_occluded(observer: (f32, f32, f32), entity: (f32, f32, f32)) -> bool {
        let dy = (entity.1 - observer.1).abs();

        // Short-circuit: most entities share the same elevation tier.
        if dy < HEIGHT_THRESHOLD {
            return false;
        }

        let dx = entity.0 - observer.0;
        let dz = entity.2 - observer.2;
        let horiz_sq = dx * dx + dz * dz;

        // Strictly greater-than: exactly at HORIZ_MIN is NOT occluded.
        horiz_sq > HORIZ_MIN_SQ
    }

    /// Total number of currently ghosted-in entities.
    #[inline]
    pub fn ghosted_count(&self) -> usize {
        self.entries.len()
    }

    /// Estimated player speed in world-units/second.
    pub fn speed_wu_per_s(&self) -> f32 {
        self.speed_sq_ema.sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_elevation_never_occluded() {
        // Two entities at ground-floor elevation (Y ~101).
        assert!(!InterestManager::height_band_occluded(
            (200.0, 101.0, 200.0),
            (230.0, 101.5, 210.0), // dY = 0.5 wu
        ));
    }

    #[test]
    fn shallow_slope_not_occluded() {
        // dY = 8 wu over 30 wu horizontal , steep ramp, below HEIGHT_THRESHOLD.
        assert!(!InterestManager::height_band_occluded(
            (200.0, 100.0, 200.0),
            (230.0, 108.0, 200.0), // dY = 8 wu < 12 wu threshold
        ));
    }

    #[test]
    fn different_floor_far_away_is_occluded() {
        // Observer on ground (Y ~101), entity on raised structure (Y ~115).
        // dY = 14 wu > HEIGHT_THRESHOLD, horizontal = 30 wu > HORIZ_MIN.
        assert!(InterestManager::height_band_occluded(
            (200.0, 101.0, 200.0),
            (230.0, 115.0, 200.0),
        ));
    }

    #[test]
    fn different_floor_very_close_not_occluded() {
        // dY = 14 wu but only 10 wu horizontal , top of a staircase next to player.
        // Horizontal guard prevents false positives at close range.
        assert!(!InterestManager::height_band_occluded(
            (200.0, 101.0, 200.0),
            (210.0, 115.0, 200.0), // horiz = 10 wu <= HORIZ_MIN
        ));
    }

    #[test]
    fn elevated_platform_looking_down_occluded() {
        // Observer on elevated platform (Y ~130), entity on ground (Y ~100).
        // dY = 30 wu, horizontal = 25 wu , looking down a cliff.
        assert!(InterestManager::height_band_occluded(
            (370.0, 130.0, 490.0),
            (395.0, 100.0, 490.0),
        ));
    }

    #[test]
    fn exactly_at_horizontal_min_not_occluded() {
        // horiz == HORIZ_MIN exactly , boundary is strictly greater-than.
        assert!(!InterestManager::height_band_occluded(
            (200.0, 101.0, 200.0),
            (220.0, 115.0, 200.0), // horiz = 20.0 wu == HORIZ_MIN, not >
        ));
    }

    #[test]
    fn just_over_horizontal_min_is_occluded() {
        // horiz = 20.01 wu > HORIZ_MIN , one floating-point epsilon over boundary.
        assert!(InterestManager::height_band_occluded(
            (200.0, 101.0, 200.0),
            (220.01, 115.0, 200.0),
        ));
    }

    #[test]
    fn cache_hit_within_ttl() {
        let mut cache = OcclusionCache::default();
        cache.insert(42, true, 1000);
        assert_eq!(cache.get(42, 1010), Some(true)); // within 16 ms window
    }

    #[test]
    fn cache_miss_after_ttl() {
        let mut cache = OcclusionCache::default();
        cache.insert(42, true, 1000);
        // valid_until = 1000 + 16 = 1016; querying at 1017 is stale.
        assert_eq!(cache.get(42, 1017), None);
    }

    #[test]
    fn cache_evicts_stale_entries_on_next_insert() {
        let mut cache = OcclusionCache::default();
        cache.insert(1, true, 1000);
        cache.insert(2, false, 1000);
        // Advance past TTL and insert , old entries must be gone.
        cache.insert(3, true, 1020);
        assert_eq!(cache.get(1, 1020), None, "stale entry 1 should be evicted");
        assert_eq!(cache.get(2, 1020), None, "stale entry 2 should be evicted");
        assert_eq!(cache.get(3, 1020), Some(true), "new entry should exist");
    }

    #[test]
    fn cache_clear_resets_all_state() {
        let mut cache = OcclusionCache::default();
        cache.insert(99, false, 5000);
        cache.clear();
        assert_eq!(cache.get(99, 5000), None);
    }

    #[test]
    fn is_occluded_caches_result_within_tick() {
        let mut mgr = InterestManager::new();
        let observer = (200.0, 101.0, 200.0);
        let entity_far_up = (230.0, 115.0, 200.0); // occluded: dY=14, horiz=30

        let first = mgr.is_occluded(7, observer, entity_far_up, 1000);
        assert!(first, "first call should compute true");

        // Pass a position that would compute false , cache must return the old result.
        let second = mgr.is_occluded(7, observer, (201.0, 102.0, 200.0), 1010);
        assert!(second, "second call within TTL must return cached true");
    }

    #[test]
    fn is_occluded_recomputes_after_cache_expires() {
        let mut mgr = InterestManager::new();
        let observer = (200.0, 101.0, 200.0);

        assert!(
            mgr.is_occluded(7, observer, (230.0, 115.0, 200.0), 1000),
            "entity on different floor should be occluded"
        );
        // New tick (1020 > 1016 TTL boundary) , entity is now at ground level.
        assert!(
            !mgr.is_occluded(7, observer, (202.0, 101.5, 200.0), 1020),
            "entity at same level after cache expiry should not be occluded"
        );
    }

    #[test]
    fn is_occluded_independent_entities_different_results() {
        let mut mgr = InterestManager::new();
        let observer = (200.0, 101.0, 200.0);
        let now = 2000;

        // Entity A: same floor , not occluded.
        let a = mgr.is_occluded(10, observer, (220.0, 101.5, 200.0), now);
        // Entity B: different floor , occluded.
        let b = mgr.is_occluded(20, observer, (230.0, 115.0, 200.0), now);

        assert!(!a, "entity A same floor should not be occluded");
        assert!(b, "entity B different floor should be occluded");
    }

    #[test]
    fn max_due_radius_zero_when_no_zone_due() {
        let mut mgr = InterestManager::new();
        // First call at a realistic wall-clock timestamp , all zones become
        // due because `now_ms - 0` is huge and exceeds every ZONE_TICK_MS.
        let _ = mgr.zones_due(1000);
        // Re-query at the same instant , nothing has elapsed since the
        // scheduler bumped each zone_last_check to 1000.
        let _ = mgr.zones_due(1000);
        assert_eq!(mgr.due_mask(), 0);
        assert_eq!(mgr.max_due_radius_sq(), 0.0);
        assert_eq!(mgr.max_due_radius(), 0.0);
    }

    #[test]
    fn max_due_radius_picks_outermost_zone() {
        let mut mgr = InterestManager::new();
        // First call at a realistic wall-clock-ish timestamp; every zone is
        // due because the saturating delta is `1000 - 0 == 1000 ms` which
        // exceeds every ZONE_TICK_MS entry.
        let due = mgr.zones_due(1000);
        assert_eq!(due, [true, true, true, true]);
        // Outermost due zone is Zone 3 (Background) at 250 wu.
        assert!((mgr.max_due_radius() - 250.0).abs() < 1e-3);

        // Advance 20 ms , only Zone 0 (16 ms tick) should be due again.
        let due = mgr.zones_due(1020);
        assert_eq!(due, [true, false, false, false]);
        assert!((mgr.max_due_radius() - 40.0).abs() < 1e-3);

        // Advance another 40 ms (now 60 ms after Zone 1's last check):
        //   * Zone 0 last check was at 1020 → 1060-1020 = 40 ms >= 16 ✓
        //   * Zone 1 last check was at 1000 → 1060-1000 = 60 ms >= 50 ✓
        //   * Zone 2 last check was at 1000 → 1060-1000 = 60 ms <  160 ✗
        let due = mgr.zones_due(1060);
        assert_eq!(due, [true, true, false, false]);
        // Outermost due zone is now Zone 1 (Combat) at 80 wu.
        assert!((mgr.max_due_radius() - 80.0).abs() < 1e-3);
    }

    #[test]
    fn max_due_radius_includes_predictive_bonus_when_fast() {
        let mut mgr = InterestManager::new();
        // Force speed EMA above threshold.
        // Use update_velocity twice to bypass the first-sample guard.
        mgr.update_velocity((0.0, 0.0, 0.0), 1);
        mgr.update_velocity((100.0, 0.0, 0.0), 1001); // 100 wu / 1 s = 100 wu/s

        let _ = mgr.zones_due(2000);
        // First scheduler call , all zones due, outermost is 250 wu.
        // Fast-mode adds the 40 wu predictive bonus on top.
        let r = mgr.max_due_radius();
        assert!(
            (r - (250.0 + 40.0)).abs() < 1e-3,
            "expected 290 wu, got {r}",
        );
    }

    #[test]
    fn touch_or_classify_returns_false_for_unknown_entity() {
        let mut mgr = InterestManager::new();
        assert!(!mgr.touch_or_classify(123, ReplicationZone::Combat, 1000));
    }

    #[test]
    fn touch_or_classify_updates_zone_for_known_entity() {
        let mut mgr = InterestManager::new();
        mgr.ghost_in(7, ReplicationZone::Background, StreamBucket::Enemy, 1000);
        assert!(mgr.touch_or_classify(7, ReplicationZone::Immediate, 2000));
        assert_eq!(mgr.zone_of(7), Some(ReplicationZone::Immediate));
    }

    #[test]
    fn touch_or_classify_bumps_last_close_when_in_combat_range() {
        let mut mgr = InterestManager::new();
        // Ghosted in at Distant → last_close_ms starts at 0.
        mgr.ghost_in(7, ReplicationZone::Distant, StreamBucket::Enemy, 1000);
        assert_eq!(mgr.entries.get(&7).unwrap().last_close_ms, 0);
        // Reclassify to Combat at t=2000 → last_close_ms must be bumped.
        assert!(mgr.touch_or_classify(7, ReplicationZone::Combat, 2000));
        assert_eq!(mgr.entries.get(&7).unwrap().last_close_ms, 2000);
    }

    #[test]
    fn update_zone_bumps_last_close_when_in_combat_range() {
        let mut mgr = InterestManager::new();
        mgr.ghost_in(7, ReplicationZone::Distant, StreamBucket::Enemy, 1000);
        // Reclassify to Immediate at t=3000 → last_close_ms updates.
        mgr.update_zone(7, ReplicationZone::Immediate, 3000);
        assert_eq!(mgr.entries.get(&7).unwrap().last_close_ms, 3000);
        // Reclassify to Distant at t=4000 → last_close_ms must NOT change.
        mgr.update_zone(7, ReplicationZone::Distant, 4000);
        assert_eq!(mgr.entries.get(&7).unwrap().last_close_ms, 3000);
    }

    #[test]
    fn ghost_in_at_close_zone_initialises_last_close() {
        let mut mgr = InterestManager::new();
        mgr.ghost_in(1, ReplicationZone::Immediate, StreamBucket::Enemy, 5000);
        assert_eq!(mgr.entries.get(&1).unwrap().last_close_ms, 5000);
        mgr.ghost_in(2, ReplicationZone::Combat, StreamBucket::Enemy, 5000);
        assert_eq!(mgr.entries.get(&2).unwrap().last_close_ms, 5000);
        // Distant ghost-in keeps last_close_ms at 0 (never engaged).
        mgr.ghost_in(3, ReplicationZone::Distant, StreamBucket::Enemy, 5000);
        assert_eq!(mgr.entries.get(&3).unwrap().last_close_ms, 0);
    }

    #[test]
    fn ghost_in_increments_bucket_count_once_per_id() {
        let mut mgr = InterestManager::new();
        mgr.ghost_in(1, ReplicationZone::Combat, StreamBucket::Enemy, 100);
        mgr.ghost_in(2, ReplicationZone::Combat, StreamBucket::Enemy, 100);
        // Re-inserting the same id MUST NOT double-count.
        mgr.ghost_in(1, ReplicationZone::Immediate, StreamBucket::Enemy, 200);
        assert_eq!(mgr.live_count(StreamBucket::Enemy), 2);
        assert_eq!(mgr.live_count(StreamBucket::Interactive), 0);
    }

    #[test]
    fn ghost_out_decrements_bucket_count() {
        let mut mgr = InterestManager::new();
        mgr.ghost_in(10, ReplicationZone::Immediate, StreamBucket::Interactive, 0);
        mgr.ghost_in(11, ReplicationZone::Immediate, StreamBucket::Interactive, 0);
        assert_eq!(mgr.live_count(StreamBucket::Interactive), 2);
        let removed = mgr.ghost_out(10);
        assert!(removed.is_some());
        assert_eq!(mgr.live_count(StreamBucket::Interactive), 1);
        // Removing an unknown id must not underflow.
        let removed = mgr.ghost_out(999);
        assert!(removed.is_none());
        assert_eq!(mgr.live_count(StreamBucket::Interactive), 1);
    }

    #[test]
    fn at_capacity_reflects_live_count() {
        let mut mgr = InterestManager::new();
        // NPC concurrent cap is 16.  Fill below → not at capacity.
        for i in 0..(NPC_CONCURRENT_CAP - 1) as u64 {
            mgr.ghost_in(i, ReplicationZone::Immediate, StreamBucket::Npc, 0);
        }
        assert!(!mgr.at_capacity(StreamBucket::Npc));
        // Reach exactly the cap → at capacity.
        mgr.ghost_in(999, ReplicationZone::Immediate, StreamBucket::Npc, 0);
        assert!(mgr.at_capacity(StreamBucket::Npc));
    }

    #[test]
    fn from_dist_sq_capped_clamps_to_max_zone() {
        // Distance well into Distant range (140 wu) but capped at Combat → None.
        assert_eq!(
            ReplicationZone::from_dist_sq_capped(140.0 * 140.0, ReplicationZone::Combat),
            None,
        );
        // Same distance with no cap (Background) classifies as Distant.
        assert_eq!(
            ReplicationZone::from_dist_sq_capped(140.0 * 140.0, ReplicationZone::Background),
            Some(ReplicationZone::Distant),
        );
        // Inside Combat range, capped at Combat → still classifies.
        assert_eq!(
            ReplicationZone::from_dist_sq_capped(70.0 * 70.0, ReplicationZone::Combat),
            Some(ReplicationZone::Combat),
        );
    }

    #[test]
    fn max_due_radius_for_clamps_when_outermost_due_zone_exceeds_cap() {
        let mut mgr = InterestManager::new();
        // First call , every zone due, mask = 0b1111.
        let _ = mgr.zones_due(1000);
        // Unclamped → Background (250 wu).
        assert!((mgr.max_due_radius() - 250.0).abs() < 1e-3);
        // Clamped at Combat → 80 wu.
        assert!((mgr.max_due_radius_for(ReplicationZone::Combat) - 80.0).abs() < 1e-3);
        // Clamped at Immediate → 40 wu.
        assert!((mgr.max_due_radius_for(ReplicationZone::Immediate) - 40.0).abs() < 1e-3);
    }

    #[test]
    fn should_retain_returns_false_for_distant_enemy_no_recent_engagement() {
        let mut mgr = InterestManager::new();
        // Ghosted in at Distant → last_close_ms = 0 (never engaged).
        mgr.ghost_in(1, ReplicationZone::Distant, StreamBucket::Enemy, 1000);
        let entry = mgr.entries.get(&1).unwrap().clone();
        // 130 wu away , within sticky cap, but never engaged.  Must NOT
        // be retained: relies on normal leave path.
        assert!(!mgr.should_retain(&entry, 130.0 * 130.0, 2000, false));
    }

    #[test]
    fn should_retain_keeps_recently_engaged_enemy_open_world() {
        let mut mgr = InterestManager::new();
        // Ghosted in at Combat range at t=1000 → last_close_ms = 1000.
        mgr.ghost_in(1, ReplicationZone::Combat, StreamBucket::Enemy, 1000);
        let entry = mgr.entries.get(&1).unwrap().clone();
        // 1500 ms later, player kited to 150 wu (past the 140 wu Combat
        // leave).  Recently engaged → retained (no in_battle needed).
        assert!(mgr.should_retain(&entry, 150.0 * 150.0, 2500, false));
    }

    #[test]
    fn should_retain_releases_enemy_after_sticky_grace_expires() {
        let mut mgr = InterestManager::new();
        mgr.ghost_in(1, ReplicationZone::Combat, StreamBucket::Enemy, 1000);
        let entry = mgr.entries.get(&1).unwrap().clone();
        // 6 seconds later , past STICKY_GRACE_MS (5 s).  Release.
        assert!(!mgr.should_retain(&entry, 150.0 * 150.0, 7000, false));
    }

    #[test]
    fn should_retain_releases_enemy_beyond_sticky_radius_even_when_engaged() {
        let mut mgr = InterestManager::new();
        mgr.ghost_in(1, ReplicationZone::Combat, StreamBucket::Enemy, 1000);
        let entry = mgr.entries.get(&1).unwrap().clone();
        // Just engaged but flung 220 wu away (lag teleport).  Release.
        assert!(!mgr.should_retain(&entry, 220.0 * 220.0, 1200, false));
        // Same scenario with in_battle=true also releases at this distance.
        assert!(!mgr.should_retain(&entry, 220.0 * 220.0, 1200, true));
    }

    #[test]
    fn should_retain_keeps_enemy_in_battle_dungeon_path() {
        let mut mgr = InterestManager::new();
        // Distant ghost-in , never engaged in open-world sense.
        mgr.ghost_in(1, ReplicationZone::Distant, StreamBucket::Enemy, 1000);
        let entry = mgr.entries.get(&1).unwrap().clone();
        // In dungeons the client sets in_battle=true , sticky kicks in even
        // without the time-based engagement signal.
        assert!(mgr.should_retain(&entry, 150.0 * 150.0, 2000, true));
    }

    #[test]
    fn should_retain_does_not_stick_interactives() {
        let mut mgr = InterestManager::new();
        mgr.ghost_in(1, ReplicationZone::Combat, StreamBucket::Interactive, 1000);
        let entry = mgr.entries.get(&1).unwrap().clone();
        // Stickiness applies only to enemies.
        assert!(!mgr.should_retain(&entry, 150.0 * 150.0, 1500, true));
    }

    #[test]
    fn should_retain_keeps_always_resident_at_any_distance() {
        let mut mgr = InterestManager::new();
        mgr.ghost_in_resident(1, ReplicationZone::Combat, StreamBucket::Interactive, 0);
        let entry = mgr.entries.get(&1).unwrap().clone();
        // Always-resident → retained no matter how far, no matter the time.
        assert!(mgr.should_retain(&entry, 9_999.0 * 9_999.0, 0, false));
        assert!(mgr.should_retain(&entry, 9_999.0 * 9_999.0, 999_999_999, true));
    }

    #[test]
    fn ghost_in_resident_sets_always_resident_flag() {
        let mut mgr = InterestManager::new();
        mgr.ghost_in_resident(
            42,
            ReplicationZone::Immediate,
            StreamBucket::Interactive,
            100,
        );
        let entry = mgr.entries.get(&42).unwrap();
        assert!(entry.always_resident);
        assert_eq!(mgr.live_count(StreamBucket::Interactive), 1);
    }

    #[test]
    fn is_zone_due_reflects_last_scheduler_call() {
        let mut mgr = InterestManager::new();
        // Bootstrap: first call at wall-clock-ish time makes every zone due.
        let _ = mgr.zones_due(1000);
        assert!(mgr.is_zone_due(ReplicationZone::Immediate));
        assert!(mgr.is_zone_due(ReplicationZone::Background));

        // 20 ms later, only Zone 0 (16 ms tick) qualifies.
        let _ = mgr.zones_due(1020);
        assert!(mgr.is_zone_due(ReplicationZone::Immediate));
        assert!(!mgr.is_zone_due(ReplicationZone::Combat));
        assert!(!mgr.is_zone_due(ReplicationZone::Distant));
        assert!(!mgr.is_zone_due(ReplicationZone::Background));
    }

    #[test]
    fn fx_hasher_distributes_sequential_u64() {
        // Sanity: hashing a few sequential IDs should not collide trivially.
        // (Not a quality guarantee , just verifies write_u64 actually mutates.)
        let mut a = FxU64Hasher::default();
        a.write_u64(1);
        let mut b = FxU64Hasher::default();
        b.write_u64(2);
        let mut c = FxU64Hasher::default();
        c.write_u64(1000);
        assert_ne!(a.finish(), b.finish());
        assert_ne!(a.finish(), c.finish());
        assert_ne!(b.finish(), c.finish());
    }
}
