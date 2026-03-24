#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("Config error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("Database error: {0}")]
    Db(#[from] perlica_db::DbError),

    #[error("Logic error: {0}")]
    Logic(#[from] perlica_logic::LogicError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Protobuf decode error: {0}")]
    Decode(#[from] perlica_proto::prost::DecodeError),

    #[error("Failed to read config {path}: {source}")]
    ConfigRead {
        path: String,
        source: std::io::Error,
    },

    #[error("Failed to parse config: {0}")]
    ConfigParse(#[from] toml::de::Error),
}

pub type Result<T> = std::result::Result<T, ServerError>;
