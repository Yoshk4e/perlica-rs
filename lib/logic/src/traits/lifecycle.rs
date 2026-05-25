//! Lifetime / expiry traits.

use crate::mail::StoredMail;

/// Anything that has a hard expiry timestamp.
pub trait Expirable {
    /// Returns `true` once the deadline has elapsed.
    fn is_expired(&self) -> bool;
}

impl Expirable for StoredMail {
    #[inline]
    fn is_expired(&self) -> bool {
        StoredMail::is_expired(self)
    }
}
