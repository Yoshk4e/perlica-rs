//! Character progression: experience accumulation and level calculation.

/// Cumulative exp required to reach `target_level` from level 1.
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
