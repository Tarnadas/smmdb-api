use bson::Bson;
use paperclip::actix::Apiv2Schema;
use serde::{Deserialize, Serialize};

#[derive(Apiv2Schema, Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Easy,
    Normal,
    Expert,
    SuperExpert,
}

impl From<Difficulty> for Bson {
    fn from(difficulty: Difficulty) -> Bson {
        Bson::String(
            serde_json::to_value(difficulty)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string(),
        )
    }
}
