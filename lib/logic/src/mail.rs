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
    pub fn is_expired(&self) -> bool {
        if self.expire_time < 0 {
            return false;
        }
        let now = (now_ms() / 1000) as i64;
        now >= self.expire_time
    }

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
