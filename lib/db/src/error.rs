use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("Failed to create saves dir {path}: {source}")]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to read save {path}: {source}")]
    ReadSave {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to deserialize save for {uid}: {source}")]
    Deserialize { uid: String, source: bincode::Error },

    #[error("Failed to serialize save: {0}")]
    Serialize(#[from] bincode::Error),

    #[error("Failed to write tmp {path}: {source}")]
    WriteTmp {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to rename {path}: {source}")]
    Rename {
        path: PathBuf,
        source: std::io::Error,
    },
}

pub type Result<T> = std::result::Result<T, DbError>;
