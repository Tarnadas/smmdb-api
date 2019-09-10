use super::Course2;
use crate::account::Account;
use cemu_smm::proto::SMM2Course::{
    SMM2Course, SMM2CourseArea_AutoScroll, SMM2CourseArea_CourseTheme, SMM2CourseHeader_GameStyle,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Course2Response {
    course: SMM2Course,
}

impl Course2Response {
    pub fn from_course(course: Course2, account: &Account) -> Course2Response {
        Course2Response {
            course: course.course,
        }
    }
}
