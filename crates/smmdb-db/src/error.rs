use std::io;

use bson::ordered::OrderedDocument;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("[Course2ConvertError]: {0}\n{1}")]
    Course2Convert(OrderedDocument, serde_json::Error),
    #[error(transparent)]
    Smmdb(#[from] smmdb_lib::Error),
    #[error(transparent)]
    Mongo(#[from] mongodb::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}
