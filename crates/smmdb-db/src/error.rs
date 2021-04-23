use bson::ordered::OrderedDocument;

pub enum DatabaseError {
    Course2ConvertError(OrderedDocument, serde_json::Error),
    Mongo(mongodb::Error),
}

impl From<mongodb::Error> for DatabaseError {
    fn from(err: mongodb::Error) -> Self {
        DatabaseError::Mongo(err)
    }
}
