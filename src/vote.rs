use bson::{oid::ObjectId, ordered::OrderedDocument, Bson};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Vote {
    #[serde(rename = "_id")]
    id: ObjectId,
    course_id: ObjectId,
    value: i32,
    timestamp: i64,
}

impl Vote {
    pub fn get_value(&self) -> i32 {
        self.value
    }
}

impl TryFrom<OrderedDocument> for Vote {
    type Error = serde_json::Error;

    fn try_from(document: OrderedDocument) -> Result<Vote, Self::Error> {
        let course = Bson::from(document);
        let course: serde_json::Value = course.into();
        Ok(serde_json::from_value(course)?)
    }
}
