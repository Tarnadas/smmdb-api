use crate::{Course2, Difficulty};

use serde::{Deserialize, Serialize};
use smmdb_auth::Account;
use smmdb_db::Database;
use smmdb_lib::proto::SMM2Course::SMM2Course;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Course2Response {
    id: String,
    owner: String,
    uploader: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    difficulty: Option<Difficulty>,
    last_modified: i64,
    uploaded: i64,
    votes: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    own_vote: Option<i32>,
    course: SMM2Course,
}

impl Course2Response {
    pub fn from_course(
        course: Course2,
        account: &Account,
        own_account: Option<&Account>,
        database: &Database,
    ) -> Course2Response {
        Course2Response {
            id: course.get_id().to_hex(),
            owner: course.owner.to_hex(),
            uploader: account.get_username().clone(),
            difficulty: course.get_difficulty().clone(),
            last_modified: course.get_last_modified(),
            uploaded: course.get_uploaded(),
            votes: course.get_votes(),
            own_vote: if let Some(own_account) = own_account {
                course.get_own_vote(own_account.get_id(), course.get_id(), database)
            } else {
                None
            },
            course: course.course,
        }
    }
}

// TODO OpenAPI gen
// #[derive(Apiv2Schema, Debug, Deserialize, Serialize)]

// pub struct SMM2CourseReponse(SMM2Course);
