#[derive(Debug, thiserror::Error)]
pub enum LogicError {
    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    InvalidOperation(String),

    #[error(transparent)]
    Config(#[from] config::ConfigError),
}

pub type Result<T> = std::result::Result<T, LogicError>;
