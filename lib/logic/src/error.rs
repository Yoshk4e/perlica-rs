#[derive(Debug, thiserror::Error)]
pub enum LogicError {
    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    InvalidOperation(String),

    #[error("Insufficient {item_id}: have {have}, need {need}")]
    Insufficient {
        item_id: String,
        have: u32,
        need: u32,
    },

    #[error(transparent)]
    Config(#[from] config::ConfigError),
}

pub type Result<T> = std::result::Result<T, LogicError>;
