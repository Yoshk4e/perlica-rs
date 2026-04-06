use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GmRequest {
    Status,
    ListPlayers,
    Execute { player_uid: String, command: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GmResponse {
    pub retcode: i32,
    pub message: String,
    #[serde(default)]
    pub online: usize,
    #[serde(default)]
    pub players: Vec<String>,
}

impl GmResponse {
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            retcode: 0,
            message: message.into(),
            ..Default::default()
        }
    }

    pub fn err(retcode: i32, message: impl Into<String>) -> Self {
        Self {
            retcode,
            message: message.into(),
            ..Default::default()
        }
    }
}
