//! Lifetime / expiry traits.

use crate::mail::StoredMail;
use common::time::now_ms;

/// Anything that has a hard expiry timestamp.
pub trait Expirable {
    /// Returns `true` once the deadline has elapsed.
    fn is_expired(&self) -> bool;
}

impl Expirable for StoredMail {
    #[inline]
    fn is_expired(&self) -> bool {
        if self.expire_time < 0 {
            return false;
        }
        let now = (now_ms() / 1000) as i64;
        now >= self.expire_time
    }
}
