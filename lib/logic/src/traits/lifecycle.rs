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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mail_with_expiry(expire_time: i64) -> StoredMail {
        StoredMail {
            mail_id: 1,
            mail_type: 0,
            is_read: false,
            is_attachment_got: false,
            send_time: 0,
            expire_time,
            template_id: String::new(),
            title: String::new(),
            content: String::new(),
            sender_name: String::new(),
            sender_icon: String::new(),
            items: vec![],
        }
    }

    #[test]
    fn expirable_never_expires_with_negative_one() {
        let mail = make_mail_with_expiry(-1);
        assert!(!mail.is_expired());
    }

    #[test]
    fn expirable_never_expires_with_any_negative() {
        let mail = make_mail_with_expiry(-999);
        assert!(!mail.is_expired());
    }

    #[test]
    fn expirable_far_future_not_expired() {
        // Set expire_time far in the future (year ~2100)
        let mail = make_mail_with_expiry(4102444800);
        assert!(!mail.is_expired());
    }

    #[test]
    fn expirable_epoch_zero_is_expired() {
        // expire_time = 0 (Jan 1 1970), current time is definitely past that
        let mail = make_mail_with_expiry(0);
        assert!(mail.is_expired());
    }

    #[test]
    fn expirable_past_timestamp_is_expired() {
        let mail = make_mail_with_expiry(1);
        assert!(mail.is_expired());
    }
}
