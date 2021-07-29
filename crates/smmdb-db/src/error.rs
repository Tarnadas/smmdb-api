use std::io;

use bson::Document;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("[Course2ConvertError]: {0}\n{1}")]
    Course2Convert(Document, serde_json::Error),
    #[error(transparent)]
    Smmdb(#[from] smmdb_lib::Error),
    #[error(transparent)]
    Mongo(#[from] mongodb::error::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("Delete failed")]
    DeleteFailed,
}
