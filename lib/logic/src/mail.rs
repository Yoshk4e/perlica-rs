use crate::traits::Expirable;
use common::time::now_ms;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMail {
    pub mail_id: u64,
    pub mail_type: i32,
    pub is_read: bool,
    pub is_attachment_got: bool,
    pub send_time: i64,
    // -1 = never expires
    pub expire_time: i64,
    // Game template ID (empty for server-generated mails)
    pub template_id: String,
    pub title: String,
    pub content: String,
    pub sender_name: String,
    pub sender_icon: String,
    pub items: Vec<(String, i64)>,
}

impl StoredMail {
    pub fn has_unclaimed_attachment(&self) -> bool {
        !self.items.is_empty() && !self.is_attachment_got
    }

    pub fn make_welcome_mail() -> StoredMail {
        let now = (now_ms() / 1000) as i64;
        StoredMail {
            mail_id: 0,
            mail_type: 0,
            is_read: false,
            is_attachment_got: false,
            send_time: now,
            expire_time: -1,
            template_id: String::new(),
            title: "Welcome to Perlica-rs!".to_string(),
            content: "Welcome to Talos-II, Endministrator!\n\n\
                      We are thrilled to have you here in the Arknights: Endfield technical test environment, proudly powered by the Perlica-rs emulator.\n\n\
                      This server is a completely open-source community project. We are building this from the ground up, and we're glad you're here to test the systems, break things, and help us improve the infrastructure.\n\n\
                      Want to contribute, report bugs, or just hang out with fellow developers and players?\n\
                      • Join our Discord: https://discord.gg/AgrhKzhP\n\
                      • Contribute on GitHub: https://github.com/Yoshk4e/perlica-rs\n\n\
                      Good luck out there on the frontier. Let's build something great together!\n\n\
                      - The Perlica-rs Team".to_string(),
            sender_name: "system".to_string(),
            sender_icon: "Mail/mail_endfield".to_string(),
            items: vec![],
        }
    }

    pub fn make_login_greeting_mail() -> StoredMail {
        let now = (now_ms() / 1000) as i64;

        StoredMail {
            mail_id: 0,
            mail_type: 0,
            is_read: false,
            is_attachment_got: false,
            send_time: now,
            expire_time: now + 86400 * 7, // 7-day expiry
            template_id: String::new(),
            title: "Frontier Sync: Signal Restored".to_string(),
            content: "Welcome back to Talos-II, Endministrator.\n\n\
                The Perlica-rs protocol has successfully re-initialized your session. The base is operating at peak efficiency in this technical test environment.\n\n\
                As a reminder, this is an open-source effort. If you encounter any anomalies or want to see the latest logic updates, check our logs here:\n\
                • Repository: https://github.com/Yoshk4e/perlica-rs\n\
                • Communication Hub: https://discord.gg/AgrhKzhP\n\n\
                Operational data is being synchronized. Good luck on the frontier.".to_string(),
            sender_name: "system".to_string(),
            sender_icon: "Mail/mail_endfield".to_string(),
            items: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailManager {
    pub mails: Vec<StoredMail>,
    next_id: u64,
}

impl Default for MailManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MailManager {
    pub fn new() -> Self {
        Self {
            mails: Vec::new(),
            next_id: 1,
        }
    }

    pub fn add_mail(&mut self, mut mail: StoredMail) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        mail.mail_id = id;
        self.mails.push(mail);
        id
    }

    pub fn next_id(&self) -> u64 {
        self.next_id
    }

    pub fn set_next_id(&mut self, id: u64) {
        self.next_id = id;
    }

    pub fn all_ids(&self) -> Vec<u64> {
        self.mails.iter().map(|m| m.mail_id).collect()
    }

    pub fn has_unread(&self) -> bool {
        self.mails.iter().any(|m| !m.is_read && !m.is_expired())
    }

    // Returns references to the mails matching the given IDs (expired mails included).
    pub fn get_by_ids(&self, ids: &[u64]) -> Vec<&StoredMail> {
        self.mails
            .iter()
            .filter(|m| ids.contains(&m.mail_id))
            .collect()
    }

    pub fn get_by_id_mut(&mut self, id: u64) -> Option<&mut StoredMail> {
        self.mails.iter_mut().find(|m| m.mail_id == id)
    }

    pub fn mark_read(&mut self, mail_id: u64) -> bool {
        match self.mails.iter_mut().find(|m| m.mail_id == mail_id) {
            Some(m) => {
                m.is_read = true;
                true
            }
            None => false,
        }
    }

    pub fn claim_attachment(&mut self, mail_id: u64) -> Option<Vec<(String, i64)>> {
        let m = self.mails.iter_mut().find(|m| m.mail_id == mail_id)?;
        if m.is_attachment_got {
            return None;
        }
        m.is_attachment_got = true;
        Some(m.items.clone())
    }

    pub fn delete_mail(&mut self, mail_id: u64) -> bool {
        let before = self.mails.len();
        self.mails.retain(|m| m.mail_id != mail_id);
        self.mails.len() != before
    }

    pub fn delete_by_types(&mut self, types: &[i32]) -> Vec<u64> {
        let to_delete: Vec<u64> = self
            .mails
            .iter()
            .filter(|m| types.is_empty() || types.contains(&m.mail_type))
            .map(|m| m.mail_id)
            .collect();
        self.mails.retain(|m| !to_delete.contains(&m.mail_id));
        to_delete
    }

    pub fn claim_all_attachments(&mut self, types: &[i32]) -> (Vec<u64>, Vec<u64>) {
        let mut success = Vec::new();
        let mut failed = Vec::new();

        for m in self.mails.iter_mut() {
            if !types.is_empty() && !types.contains(&m.mail_type) {
                continue;
            }
            if m.items.is_empty() {
                // Nothing to claim, skip silently
                continue;
            }
            if m.is_attachment_got {
                failed.push(m.mail_id);
            } else {
                m.is_attachment_got = true;
                success.push(m.mail_id);
            }
        }

        (success, failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mail(mail_type: i32, items: Vec<(String, i64)>) -> StoredMail {
        StoredMail {
            mail_id: 0,
            mail_type,
            is_read: false,
            is_attachment_got: false,
            send_time: 1000,
            expire_time: -1,
            template_id: String::new(),
            title: "Test".to_string(),
            content: "Test content".to_string(),
            sender_name: "system".to_string(),
            sender_icon: "icon".to_string(),
            items,
        }
    }

    #[test]
    fn has_unclaimed_attachment_with_items() {
        let mail = make_mail(0, vec![("item_001".to_string(), 5)]);
        assert!(mail.has_unclaimed_attachment());
    }

    #[test]
    fn has_unclaimed_attachment_empty_items() {
        let mail = make_mail(0, vec![]);
        assert!(!mail.has_unclaimed_attachment());
    }

    #[test]
    fn has_unclaimed_attachment_already_claimed() {
        let mut mail = make_mail(0, vec![("item_001".to_string(), 5)]);
        mail.is_attachment_got = true;
        assert!(!mail.has_unclaimed_attachment());
    }

    #[test]
    fn new_manager_has_no_mails() {
        let mgr = MailManager::new();
        assert!(mgr.mails.is_empty());
        assert_eq!(mgr.next_id(), 1);
        assert!(!mgr.has_unread());
    }

    #[test]
    fn add_mail_assigns_id() {
        let mut mgr = MailManager::new();
        let id1 = mgr.add_mail(make_mail(0, vec![]));
        let id2 = mgr.add_mail(make_mail(0, vec![]));
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(mgr.next_id(), 3);
    }

    #[test]
    fn set_next_id() {
        let mut mgr = MailManager::new();
        mgr.set_next_id(100);
        assert_eq!(mgr.next_id(), 100);
    }

    #[test]
    fn all_ids() {
        let mut mgr = MailManager::new();
        mgr.add_mail(make_mail(0, vec![]));
        mgr.add_mail(make_mail(0, vec![]));
        assert_eq!(mgr.all_ids(), vec![1, 2]);
    }

    #[test]
    fn has_unread_with_unread_mail() {
        let mut mgr = MailManager::new();
        mgr.add_mail(make_mail(0, vec![]));
        assert!(mgr.has_unread());
    }

    #[test]
    fn has_unread_false_when_all_read() {
        let mut mgr = MailManager::new();
        let id = mgr.add_mail(make_mail(0, vec![]));
        mgr.mark_read(id);
        assert!(!mgr.has_unread());
    }

    #[test]
    fn mark_read_nonexistent_returns_false() {
        let mut mgr = MailManager::new();
        assert!(!mgr.mark_read(999));
    }

    #[test]
    fn get_by_ids() {
        let mut mgr = MailManager::new();
        let id1 = mgr.add_mail(make_mail(0, vec![]));
        let id2 = mgr.add_mail(make_mail(0, vec![]));
        let result = mgr.get_by_ids(&[id1, id2, 999]);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn claim_attachment_success() {
        let mut mgr = MailManager::new();
        let id = mgr.add_mail(make_mail(0, vec![("item_001".to_string(), 5)]));
        let items = mgr.claim_attachment(id);
        assert!(items.is_some());
        let items = items.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].0, "item_001");
    }

    #[test]
    fn claim_attachment_already_claimed() {
        let mut mgr = MailManager::new();
        let id = mgr.add_mail(make_mail(0, vec![("item_001".to_string(), 5)]));
        mgr.claim_attachment(id);
        let second = mgr.claim_attachment(id);
        assert!(second.is_none());
    }

    #[test]
    fn claim_attachment_nonexistent() {
        let mut mgr = MailManager::new();
        let result = mgr.claim_attachment(999);
        assert!(result.is_none());
    }

    #[test]
    fn delete_mail() {
        let mut mgr = MailManager::new();
        let id = mgr.add_mail(make_mail(0, vec![]));
        assert!(mgr.delete_mail(id));
        assert!(mgr.mails.is_empty());
    }

    #[test]
    fn delete_mail_nonexistent() {
        let mut mgr = MailManager::new();
        assert!(!mgr.delete_mail(999));
    }

    #[test]
    fn delete_by_types_with_empty_types_deletes_all() {
        let mut mgr = MailManager::new();
        mgr.add_mail(make_mail(1, vec![]));
        mgr.add_mail(make_mail(2, vec![]));
        let deleted = mgr.delete_by_types(&[]);
        assert_eq!(deleted.len(), 2);
        assert!(mgr.mails.is_empty());
    }

    #[test]
    fn delete_by_types_filters_correctly() {
        let mut mgr = MailManager::new();
        mgr.add_mail(make_mail(1, vec![]));
        mgr.add_mail(make_mail(2, vec![]));
        mgr.add_mail(make_mail(3, vec![]));
        let deleted = mgr.delete_by_types(&[1, 3]);
        assert_eq!(deleted.len(), 2);
        assert_eq!(mgr.mails.len(), 1);
        assert_eq!(mgr.mails[0].mail_type, 2);
    }

    #[test]
    fn claim_all_attachments() {
        let mut mgr = MailManager::new();
        mgr.add_mail(make_mail(0, vec![("a".to_string(), 1)]));
        mgr.add_mail(make_mail(0, vec![("b".to_string(), 2)]));
        mgr.add_mail(make_mail(0, vec![])); // no items
        let (success, failed) = mgr.claim_all_attachments(&[]);
        assert_eq!(success.len(), 2);
        assert!(failed.is_empty());
    }

    #[test]
    fn claim_all_attachments_already_claimed_goes_to_failed() {
        let mut mgr = MailManager::new();
        let id1 = mgr.add_mail(make_mail(0, vec![("a".to_string(), 1)]));
        mgr.claim_attachment(id1);
        let (success, failed) = mgr.claim_all_attachments(&[]);
        assert!(success.is_empty());
        assert_eq!(failed.len(), 1);
    }

    #[test]
    fn claim_all_attachments_filters_by_type() {
        let mut mgr = MailManager::new();
        mgr.add_mail(make_mail(1, vec![("a".to_string(), 1)]));
        mgr.add_mail(make_mail(2, vec![("b".to_string(), 2)]));
        let (success, _failed) = mgr.claim_all_attachments(&[1]);
        assert_eq!(success.len(), 1);
    }

    #[test]
    fn get_by_id_mut() {
        let mut mgr = MailManager::new();
        let id = mgr.add_mail(make_mail(0, vec![]));
        if let Some(m) = mgr.get_by_id_mut(id) {
            m.title = "Modified".to_string();
        }
        assert_eq!(mgr.get_by_ids(&[id])[0].title, "Modified");
    }
}
