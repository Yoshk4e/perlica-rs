mod saves;
pub use saves::PlayerDb;
pub use saves::{PlayerRecord, PlayerRecordRef};
pub mod error;

pub use error::{DbError, Result};
