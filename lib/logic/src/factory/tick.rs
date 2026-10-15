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

/// Progress envelope for one long-running machine operation.
///
/// Every machine-bearing wire message carries four correlated fields --
/// `update_ts`, `current_progress`, `current_speed`, `pause_ts` -- and the
/// bare `start_tick` the free functions above operate on cannot express
/// them. There is nowhere to record *when* a machine was paused, so
/// progress can neither freeze nor resume from where it froze.
///
/// Progress accrues at `speed` units per tick while running. Pausing banks
/// the running segment into `banked_progress` and freezes there; resuming
/// opens a fresh segment. [`progress_at`](Self::progress_at) is therefore
/// continuous across any number of pause/resume cycles.
///
/// Every mutator takes an explicit `now`, with a `current_tick()` wrapper
/// beside it. Tests need to drive time; production almost always wants the
/// wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineTiming {
    /// Start of the current running segment. Meaningless while paused --
    /// read progress through [`progress_at`](Self::progress_at), never by
    /// subtracting this field.
    pub start_tick: Tick,
    /// Progress units accrued per tick. Zero means "makes no progress",
    /// which is distinct from paused: a zero-speed machine is running but
    /// getting nowhere, and never completes.
    pub speed: u64,
    /// Progress accrued by segments that have already ended.
    pub banked_progress: u64,
    /// `Some(tick)` while paused. This is the wire's `pause_ts`.
    pub paused_at: Option<Tick>,
    /// Tick of the last *mutation* of this envelope.
    ///
    /// Deliberately not the wire's `update_ts`: the wire wants the instant
    /// at which `current_progress` was measured, which is the projection
    /// instant (see [`wire_at`](Self::wire_at)). This field exists for
    /// persistence and dirty-tracking, which need to know when the row last
    /// actually changed.
    pub update_tick: Tick,
}

impl MachineTiming {
    /// A running envelope with zero progress, starting at `now`.
    pub fn start_at(now: Tick, speed: u64) -> Self {
        Self {
            start_tick: now,
            speed,
            banked_progress: 0,
            paused_at: None,
            update_tick: now,
        }
    }

    /// A running envelope with zero progress, starting this instant.
    pub fn start(speed: u64) -> Self {
        Self::start_at(current_tick(), speed)
    }

    /// A running envelope that already holds `progress`, as if it had been
    /// running since `now - progress / speed`.
    ///
    /// For adopting a machine that tracked progress some other way.
    pub fn resumed_with(now: Tick, speed: u64, progress: u64) -> Self {
        Self {
            start_tick: now,
            speed,
            banked_progress: progress,
            paused_at: None,
            update_tick: now,
        }
    }

    /// A paused envelope holding `progress`, frozen at `now`.
    pub fn paused_with(now: Tick, speed: u64, progress: u64) -> Self {
        Self {
            start_tick: now,
            speed,
            banked_progress: progress,
            paused_at: Some(now),
            update_tick: now,
        }
    }

    pub fn is_paused(&self) -> bool {
        self.paused_at.is_some()
    }

    /// Progress as of `now`: banked, plus the current segment if running.
    ///
    /// Saturates rather than wrapping, so a corrupted save with a wild
    /// `speed` degrades to "very complete" instead of panicking in debug or
    /// silently wrapping to zero in release.
    pub fn progress_at(&self, now: Tick) -> u64 {
        if self.is_paused() {
            return self.banked_progress;
        }
        let running = now
            .saturating_sub(self.start_tick)
            .saturating_mul(self.speed);
        self.banked_progress.saturating_add(running)
    }

    pub fn progress(&self) -> u64 {
        self.progress_at(current_tick())
    }

    /// Freeze progress at `now`. Idempotent -- calling it on an already
    /// paused envelope is a no-op rather than banking the same window
    /// twice.
    pub fn pause_at(&mut self, now: Tick) {
        if self.is_paused() {
            return;
        }
        self.banked_progress = self.progress_at(now);
        self.paused_at = Some(now);
        self.update_tick = now;
    }

    pub fn pause(&mut self) {
        self.pause_at(current_tick());
    }

    /// Open a fresh running segment at `now`, keeping banked progress.
    /// A no-op if not paused.
    pub fn resume_at(&mut self, now: Tick) {
        if !self.is_paused() {
            return;
        }
        self.paused_at = None;
        self.start_tick = now;
        self.update_tick = now;
    }

    pub fn resume(&mut self) {
        self.resume_at(current_tick());
    }

    /// Change the rate future progress accrues at.
    ///
    /// Banks at the old rate first: assigning `speed` directly would apply
    /// the new rate retroactively to the whole elapsed window, so a
    /// speed-up would conjure progress that was never earned.
    pub fn set_speed_at(&mut self, now: Tick, speed: u64) {
        if self.speed == speed {
            return;
        }
        self.banked_progress = self.progress_at(now);
        self.start_tick = now;
        self.speed = speed;
        self.update_tick = now;
    }

    pub fn is_complete_at(&self, now: Tick, total_progress: u64) -> bool {
        self.progress_at(now) >= total_progress
    }

    pub fn is_complete(&self, total_progress: u64) -> bool {
        self.is_complete_at(current_tick(), total_progress)
    }

    /// The tick at which `total_progress` is reached.
    ///
    /// `None` when it will never be reached: a paused envelope is frozen,
    /// and a zero-speed one is running but going nowhere. Neither is
    /// "completes immediately", which is what clamping the divisor to 1
    /// (as the free [`completion_tick`] must, having no pause concept)
    /// would imply.
    pub fn completion_tick_at(&self, now: Tick, total_progress: u64) -> Option<Tick> {
        if self.is_paused() {
            return None;
        }
        let remaining = total_progress.saturating_sub(self.progress_at(now));
        if remaining == 0 {
            return Some(now);
        }
        if self.speed == 0 {
            return None;
        }
        Some(now.saturating_add(remaining.div_ceil(self.speed)))
    }

    pub fn completion_tick(&self, total_progress: u64) -> Option<Tick> {
        self.completion_tick_at(current_tick(), total_progress)
    }

    /// The tick this run did, or will, reach `total_progress` — measured from
    /// the start of the current segment rather than from now.
    ///
    /// Unlike [`completion_tick_at`](Self::completion_tick_at) this reports a
    /// tick in the *past* for an operation that already finished, which is
    /// what a lazy catch-up needs: chaining the next round from the real
    /// completion instant rather than from login time is the difference
    /// between a queue of five drying overnight and only its head finishing.
    ///
    /// `None` while paused or stalled at zero speed, as above.
    pub fn scheduled_completion(&self, total_progress: u64) -> Option<Tick> {
        self.completion_tick_at(self.start_tick, total_progress)
    }

    /// How many whole `total_progress` rounds have finished, without
    /// consuming any of them.
    ///
    /// For deciding how much work to attempt before committing to it — a
    /// round whose output cannot be produced (invalid config, no room) should
    /// not have its time deducted.
    pub fn completed_rounds_at(&self, now: Tick, total_progress: u64) -> u64 {
        if total_progress == 0 {
            return 0;
        }
        self.progress_at(now) / total_progress
    }

    /// Deduct and return however many whole `total_progress` rounds have
    /// finished, keeping the sub-round remainder.
    ///
    /// This is what every round-based machine needs, and what each of them
    /// previously open-coded as `last_tick = now` -- which discards
    /// `elapsed % total_progress`, so a player checking in just under one
    /// round short, repeatedly, never accrues anything at all.
    pub fn take_completed_rounds_at(&mut self, now: Tick, total_progress: u64) -> u64 {
        self.take_rounds_at(now, total_progress, None)
    }

    pub fn take_completed_rounds(&mut self, total_progress: u64) -> u64 {
        self.take_completed_rounds_at(current_tick(), total_progress)
    }

    /// [`take_completed_rounds_at`](Self::take_completed_rounds_at) with a
    /// ceiling, for a machine that cannot bank the result of every finished
    /// round (a full output buffer, an unaffordable next round).
    ///
    /// Rounds above `max` are left unconsumed, so their elapsed time is
    /// still there once the blockage clears.
    pub fn take_completed_rounds_capped_at(
        &mut self,
        now: Tick,
        total_progress: u64,
        max: u64,
    ) -> u64 {
        self.take_rounds_at(now, total_progress, Some(max))
    }

    fn take_rounds_at(&mut self, now: Tick, total_progress: u64, max: Option<u64>) -> u64 {
        if total_progress == 0 {
            return 0;
        }
        let progress = self.progress_at(now);
        let mut rounds = progress / total_progress;
        if let Some(max) = max {
            rounds = rounds.min(max);
        }
        if rounds == 0 {
            return 0;
        }
        self.rebase_at(now, progress - rounds * total_progress);
        rounds
    }

    /// Reset progress to zero, keeping speed and pause state.
    pub fn restart_at(&mut self, now: Tick) {
        self.rebase_at(now, 0);
    }

    pub fn restart(&mut self) {
        self.restart_at(current_tick());
    }

    /// Set progress to an exact value as of `now`.
    ///
    /// `paused_at` is deliberately left alone: it records when the machine
    /// froze, and consuming a round that finished *before* the freeze must
    /// not backdate that instant to now, which would report a fresh pause
    /// to the client.
    fn rebase_at(&mut self, now: Tick, progress: u64) {
        self.banked_progress = progress;
        self.start_tick = now;
        self.update_tick = now;
    }

    /// The tick a client extrapolating from a bare start timestamp should
    /// be told, i.e. the virtual start that reproduces current progress.
    ///
    /// For the handful of wire fields (`head_start_tms`) that carry a start
    /// instant rather than a progress value, and so cannot express banked
    /// progress on their own.
    pub fn effective_start_at(&self, now: Tick) -> Tick {
        let reference = self.paused_at.unwrap_or(now);
        if self.speed == 0 {
            return reference;
        }
        reference.saturating_sub(self.progress_at(now) / self.speed)
    }

    /// The four correlated wire timing fields, all measured at one `now`.
    ///
    /// Deriving them together is the point: computing `current_progress` and
    /// `update_ts` from separate clock reads lets a client extrapolate from
    /// a progress value that was never true at the timestamp beside it.
    pub fn wire_at(&self, now: Tick) -> TimingWire {
        TimingWire {
            update_ts: now,
            current_progress: self.progress_at(now),
            current_speed: self.speed,
            pause_ts: self.paused_at.unwrap_or(0),
        }
    }

    pub fn wire(&self) -> TimingWire {
        self.wire_at(current_tick())
    }
}

/// The timing quartet every machine message carries, in internal units.
///
/// Kept unsigned here and narrowed at the proto boundary, since the wire
/// disagrees with itself on the type of `current_speed` (`int64` for
/// manufacture, `double` for the recycler and trader).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimingWire {
    pub update_ts: Tick,
    pub current_progress: u64,
    pub current_speed: u64,
    /// 0 when running -- the wire's "not paused" sentinel.
    pub pause_ts: Tick,
}

impl TimingWire {
    pub fn update_ts_i64(&self) -> i64 {
        to_i64(self.update_ts)
    }

    pub fn current_progress_i64(&self) -> i64 {
        to_i64(self.current_progress)
    }

    pub fn current_speed_i64(&self) -> i64 {
        to_i64(self.current_speed)
    }

    pub fn current_speed_f64(&self) -> f64 {
        self.current_speed as f64
    }

    pub fn pause_ts_i64(&self) -> i64 {
        to_i64(self.pause_ts)
    }
}

/// Clamp on the way to the wire. `as i64` would wrap a corrupted `u64`
/// into a negative progress, which the client reads as "finished".
fn to_i64(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fixed, plausible epoch so the timing tests never touch the clock.
    const T0: Tick = 1_800_000_000;

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

    #[test]
    fn timing_accrues_at_speed() {
        let t = MachineTiming::start_at(T0, 3);
        assert_eq!(t.progress_at(T0), 0);
        assert_eq!(t.progress_at(T0 + 10), 30);
        assert!(!t.is_complete_at(T0 + 10, 31));
        assert!(t.is_complete_at(T0 + 10, 30));
    }

    #[test]
    fn timing_progress_is_continuous_across_pause() {
        let mut t = MachineTiming::start_at(T0, 1);
        t.pause_at(T0 + 40);
        assert_eq!(t.paused_at, Some(T0 + 40));

        // Frozen: an hour of wall clock adds nothing.
        assert_eq!(t.progress_at(T0 + 40), 40);
        assert_eq!(t.progress_at(T0 + 3_600), 40);

        t.resume_at(T0 + 3_600);
        assert_eq!(t.progress_at(T0 + 3_600), 40, "resume adds no progress");
        assert_eq!(t.progress_at(T0 + 3_610), 50, "continues from 40");
        assert_eq!(t.paused_at, None);
    }

    #[test]
    fn timing_pause_is_idempotent() {
        // The hand-rolled version of this in `manual_work_pause` banked
        // `elapsed_since(start)` unconditionally, so a second pause while
        // already paused double-counted the same window.
        let mut t = MachineTiming::start_at(T0, 1);
        t.pause_at(T0 + 40);
        t.pause_at(T0 + 100);
        assert_eq!(t.progress_at(T0 + 100), 40);
        assert_eq!(t.paused_at, Some(T0 + 40), "original pause instant kept");
    }

    #[test]
    fn timing_resume_when_running_is_a_noop() {
        let mut t = MachineTiming::start_at(T0, 1);
        t.resume_at(T0 + 50);
        assert_eq!(t.progress_at(T0 + 50), 50, "no progress lost");
    }

    #[test]
    fn timing_repeated_pause_resume_cycles_lose_nothing() {
        let mut t = MachineTiming::start_at(T0, 2);
        let mut now = T0;
        for _ in 0..5 {
            now += 10; // 20 progress
            t.pause_at(now);
            now += 1_000; // frozen
            t.resume_at(now);
        }
        assert_eq!(t.progress_at(now), 100);
    }

    #[test]
    fn timing_set_speed_does_not_rewrite_past_progress() {
        let mut t = MachineTiming::start_at(T0, 1);
        t.set_speed_at(T0 + 100, 10);
        assert_eq!(t.progress_at(T0 + 100), 100, "the first 100 ticks were 1x");
        assert_eq!(t.progress_at(T0 + 110), 200, "the next 10 are 10x");
    }

    #[test]
    fn timing_take_rounds_keeps_the_remainder() {
        // The `last_tick = now` idiom this replaces threw away the 50.
        let mut t = MachineTiming::start_at(T0, 1);
        assert_eq!(t.take_completed_rounds_at(T0 + 250, 100), 2);
        assert_eq!(t.progress_at(T0 + 250), 50);
        // The remainder counts toward the next round.
        assert_eq!(t.take_completed_rounds_at(T0 + 300, 100), 1);
        assert_eq!(t.progress_at(T0 + 300), 0);
    }

    #[test]
    fn timing_take_rounds_below_one_round_consumes_nothing() {
        let mut t = MachineTiming::start_at(T0, 1);
        assert_eq!(t.take_completed_rounds_at(T0 + 99, 100), 0);
        assert_eq!(t.progress_at(T0 + 99), 99, "progress untouched");
    }

    #[test]
    fn timing_capped_take_leaves_blocked_rounds_banked() {
        let mut t = MachineTiming::start_at(T0, 1);
        // Five rounds elapsed but only two can be stored.
        assert_eq!(t.take_completed_rounds_capped_at(T0 + 500, 100, 2), 2);
        assert_eq!(t.progress_at(T0 + 500), 300, "three rounds still banked");
        // Once the blockage clears the time is still there.
        assert_eq!(t.take_completed_rounds_capped_at(T0 + 500, 100, 10), 3);
    }

    #[test]
    fn timing_take_rounds_while_paused_keeps_pause_instant() {
        let mut t = MachineTiming::start_at(T0, 1);
        t.pause_at(T0 + 250);
        assert_eq!(t.take_completed_rounds_at(T0 + 9_999, 100), 2);
        assert_eq!(
            t.progress_at(T0 + 9_999),
            50,
            "still frozen at the remainder"
        );
        assert_eq!(
            t.paused_at,
            Some(T0 + 250),
            "consuming a round must not backdate the freeze"
        );
    }

    #[test]
    fn timing_take_rounds_with_zero_total_is_not_infinite() {
        let mut t = MachineTiming::start_at(T0, 1);
        assert_eq!(t.take_completed_rounds_at(T0 + 100, 0), 0);
    }

    #[test]
    fn timing_completion_tick_is_none_when_it_will_never_arrive() {
        let mut t = MachineTiming::start_at(T0, 5);
        assert_eq!(t.completion_tick_at(T0, 100), Some(T0 + 20));
        // Ceil: 101 at 5/tick needs 21 ticks.
        assert_eq!(t.completion_tick_at(T0, 101), Some(T0 + 21));
        // Already there.
        assert_eq!(t.completion_tick_at(T0 + 100, 100), Some(T0 + 100));

        t.pause_at(T0 + 1);
        assert_eq!(t.completion_tick_at(T0 + 1, 100), None, "frozen");

        let stalled = MachineTiming::start_at(T0, 0);
        assert_eq!(
            stalled.completion_tick_at(T0, 100),
            None,
            "no progress rate"
        );
    }

    #[test]
    fn timing_scheduled_completion_reports_the_historical_instant() {
        // Two rounds' worth of time has passed on a 100-progress operation.
        let t = MachineTiming::start_at(T0, 1);
        // `completion_tick_at` answers "when, from now" -- already there.
        assert_eq!(t.completion_tick_at(T0 + 250, 100), Some(T0 + 250));
        // `scheduled_completion` answers "when did it actually finish".
        assert_eq!(t.scheduled_completion(100), Some(T0 + 100));

        // Banked progress shortens the remaining window.
        let t = MachineTiming::resumed_with(T0, 1, 60);
        assert_eq!(t.scheduled_completion(100), Some(T0 + 40));

        // Ceil, and never for a frozen run.
        let t = MachineTiming::start_at(T0, 3);
        assert_eq!(t.scheduled_completion(100), Some(T0 + 34));
        let mut t = MachineTiming::start_at(T0, 3);
        t.pause_at(T0);
        assert_eq!(t.scheduled_completion(100), None);
    }

    #[test]
    fn timing_effective_start_reproduces_progress() {
        let mut t = MachineTiming::start_at(T0, 1);
        t.pause_at(T0 + 40);
        t.resume_at(T0 + 1_000);
        // 40 banked + 10 running = 50 progress at speed 1.
        let now = T0 + 1_010;
        assert_eq!(t.progress_at(now), 50);
        assert_eq!(
            t.effective_start_at(now),
            now - 50,
            "a client extrapolating from this start lands on the real progress"
        );
    }

    #[test]
    fn timing_wire_quartet_is_self_consistent() {
        let mut t = MachineTiming::start_at(T0, 7);
        let w = t.wire_at(T0 + 10);
        assert_eq!(w.update_ts, T0 + 10);
        assert_eq!(w.current_progress, 70);
        assert_eq!(w.current_speed, 7);
        assert_eq!(w.pause_ts, 0, "0 is the not-paused sentinel");

        t.pause_at(T0 + 10);
        let w = t.wire_at(T0 + 900);
        assert_eq!(w.pause_ts, T0 + 10);
        assert_eq!(w.current_progress, 70, "frozen progress reported");
        assert_eq!(w.update_ts, T0 + 900, "but measured now");
    }

    #[test]
    fn timing_saturates_instead_of_wrapping() {
        let t = MachineTiming {
            start_tick: 0,
            speed: u64::MAX,
            banked_progress: u64::MAX,
            paused_at: None,
            update_tick: 0,
        };
        assert_eq!(t.progress_at(T0), u64::MAX);
        assert_eq!(t.wire_at(T0).current_progress_i64(), i64::MAX);
    }

    #[test]
    fn timing_survives_a_clock_that_went_backwards() {
        let t = MachineTiming::start_at(T0, 10);
        // `now` before `start_tick` (clock skew, restored save): no
        // progress, rather than a huge wrapped one.
        assert_eq!(t.progress_at(T0 - 5_000), 0);
    }
}
