//! Tick + timing math.
//!
//! 1 tick = 1 second. Stored as Unix epoch seconds so it persists naturally
//! without any epoch-of-server-start bookkeeping.
//!
//! All building progress is computed from timestamps, never accumulated
//! iteratively. See §4.1 of the implementation plan for the core formula:
//!
//! ```text
//! current_progress = (current_tick - start_tick) * speed
//! is_complete      = current_progress >= recipe.total_progress
//! completion_tick  = start_tick + ceil(recipe.total_progress / speed)
//! ```

/// Global monotonic tick counter. 1 tick = 1 second.
pub type Tick = u64;

/// Offline time counts as progress, fine for now, revisit if we need a
/// "pause when offline" toggle.
///
/// Returns 0 if the system clock is before the Unix epoch (shouldn't happen
/// on real hardware, but we prefer a graceful degradation over a panic).
pub fn current_tick() -> Tick {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) // 0 is our "clock is wonky" sentinel
}

/// Ticks elapsed since `start`, bottoming out at zero.
///
/// Saturates so a future `start` (clock skew, corrupted save) doesn't cause
/// underflow or negative progress.
pub fn elapsed_since(start: Tick) -> Tick {
    current_tick().saturating_sub(start)
}

/// Straight from the spec: `(now - start) * speed >= total`.
///
/// One source of truth for the formula, if it ever changes, this is the
/// only spot to edit.
pub fn is_complete(start: Tick, speed: u64, total_progress: u64) -> bool {
    let elapsed = elapsed_since(start);
    elapsed.saturating_mul(speed) >= total_progress
}

/// The tick when the operation finishes (ceil division).
///
/// Used by the completion checker so it can skip nodes that aren’t due yet
/// without walking the whole world every tick.
pub fn completion_tick(start: Tick, speed: u64, total_progress: u64) -> Tick {
    let speed = speed.max(1); // avoid divide-by-zero; speed 0 makes no progress anyway
    let needed = total_progress.div_ceil(speed);
    start.saturating_add(needed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_tick_is_unix_seconds() {
        // Sanity check: if this fails, the OS clock is probably broken.
        assert!(current_tick() > 1_700_000_000);
    }

    #[test]
    fn elapsed_since_saturates_on_future_start() {
        let future = current_tick().saturating_add(1_000);
        assert_eq!(elapsed_since(future), 0);
    }

    #[test]
    fn is_complete_matches_formula() {
        let now = current_tick();
        // Not started yet (future start -> elapsed 0).
        assert!(!is_complete(now.saturating_add(1_000), 100, 24_000));
        // Exactly finished after 240 ticks.
        assert!(is_complete(now.saturating_sub(240), 100, 24_000));
        // Over-complete by one tick.
        assert!(is_complete(now.saturating_sub(241), 100, 24_000));
    }

    #[test]
    fn completion_tick_uses_ceil() {
        // 24_001 total progress at 100/tick needs 241 ticks.
        assert_eq!(completion_tick(0, 100, 24_001), 241);
        // 24_000 exactly needs 240 ticks.
        assert_eq!(completion_tick(0, 100, 24_000), 240);
        // speed 0 is bumped to 1 so we don't panic.
        assert_eq!(completion_tick(0, 0, 100), 100);
    }
}
