//! Character progression: experience accumulation and level calculation.

/// Cumulative exp required to reach `target_level` from level 1.
///
/// # Edge cases
/// * If `target_level` is 1 the result is 0 (no exp needed to be at level 1).
/// * If `level_up_exp` is shorter than `target_level - 1`, the missing
///   entries are treated as 0 exp.
/// * A negative entry in `level_up_exp` acts as a sentinel that stops
///   accumulation early (used by the game data tables to cap progression).
pub fn cumulative_exp(level_up_exp: &[i32], target_level: i32) -> i64 {
    let mut total = 0i64;
    for i in 0..(target_level - 1) as usize {
        let cost = level_up_exp.get(i).copied().unwrap_or(0);
        if cost < 0 {
            break;
        }
        total += cost as i64;
    }
    total
}

/// Advances `current_level` as far as possible given `new_total_exp`, capped at `max_level`.
/// Returns `(achieved_level, remaining_exp_within_that_level)`.
pub fn calculate_level_from_total_exp(
    level_up_exp: &[i32],
    current_level: i32,
    new_total_exp: i64,
    max_level: i32,
) -> (i32, i32) {
    let mut lv = current_level;
    loop {
        if lv >= max_level {
            break;
        }
        let cost = level_up_exp.get(lv as usize - 1).copied().unwrap_or(-1);
        if cost < 0 {
            break;
        }
        let cum_next = cumulative_exp(level_up_exp, lv + 1);
        if new_total_exp >= cum_next {
            lv += 1;
        } else {
            break;
        }
    }
    let cum_at_lv = cumulative_exp(level_up_exp, lv);
    let remaining = (new_total_exp - cum_at_lv).max(0) as i32;
    (lv, remaining)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cumulative_exp_level_1_is_zero() {
        let table = [100, 200, 300];
        assert_eq!(cumulative_exp(&table, 1), 0);
    }

    #[test]
    fn cumulative_exp_sums_correctly() {
        let table = [100, 200, 300];
        // Level 2 needs table[0] = 100
        assert_eq!(cumulative_exp(&table, 2), 100);
        // Level 3 needs table[0] + table[1] = 300
        assert_eq!(cumulative_exp(&table, 3), 300);
        // Level 4 needs table[0] + table[1] + table[2] = 600
        assert_eq!(cumulative_exp(&table, 4), 600);
    }

    #[test]
    fn cumulative_exp_empty_table() {
        let table: [i32; 0] = [];
        // Any target level beyond 1 yields 0 with an empty table
        assert_eq!(cumulative_exp(&table, 1), 0);
        assert_eq!(cumulative_exp(&table, 5), 0);
    }

    #[test]
    fn cumulative_exp_negative_entry_stops_accumulation() {
        let table = [100, -1, 300];
        // Level 3: would sum table[0]+table[1], but table[1] < 0 stops at table[0]
        assert_eq!(cumulative_exp(&table, 3), 100);
        // Level 4: same, stops at table[1] which is negative
        assert_eq!(cumulative_exp(&table, 4), 100);
    }

    #[test]
    fn cumulative_exp_table_shorter_than_target() {
        let table = [100, 200];
        // Level 4 needs table[0]+table[1]+table[2], but table[2] doesn't exist -> 0
        assert_eq!(cumulative_exp(&table, 4), 300);
    }

    #[test]
    fn calculate_level_no_exp_stays_at_current() {
        let table = [100, 200, 300];
        let (lv, rem) = calculate_level_from_total_exp(&table, 1, 0, 10);
        assert_eq!(lv, 1);
        assert_eq!(rem, 0);
    }

    #[test]
    fn calculate_level_enough_exp_to_level_up() {
        let table = [100, 200, 300];
        // 150 total exp: level 2 needs 100, level 3 needs 300 -> stays at 2
        let (lv, rem) = calculate_level_from_total_exp(&table, 1, 150, 10);
        assert_eq!(lv, 2);
        // remaining = 150 - cumulative_exp(2) = 150 - 100 = 50
        assert_eq!(rem, 50);
    }

    #[test]
    fn calculate_level_multiple_level_ups() {
        let table = [100, 200, 300];
        // 600 total exp: can reach level 4
        let (lv, rem) = calculate_level_from_total_exp(&table, 1, 600, 10);
        assert_eq!(lv, 4);
        assert_eq!(rem, 0);
    }

    #[test]
    fn calculate_level_respects_max_level() {
        let table = [100, 200, 300, 400];
        // Enough exp for level 5, but max_level = 2
        let (lv, _rem) = calculate_level_from_total_exp(&table, 1, 1000, 2);
        assert_eq!(lv, 2);
    }

    #[test]
    fn calculate_level_starting_from_higher_level() {
        let table = [100, 200, 300];
        // Already at level 2 with 500 total exp
        // cumulative_exp(3) = 300, cumulative_exp(4) = 600 -> reaches level 3
        let (lv, rem) = calculate_level_from_total_exp(&table, 2, 500, 10);
        assert_eq!(lv, 3);
        // remaining = 500 - 300 = 200
        assert_eq!(rem, 200);
    }

    #[test]
    fn calculate_level_negative_entry_caps_progression() {
        let table = [100, -1, 300];
        // Negative entry stops cumulative_exp at 100, so level 3+ is unreachable
        let (lv, _rem) = calculate_level_from_total_exp(&table, 1, 500, 10);
        assert_eq!(lv, 2);
    }
}
