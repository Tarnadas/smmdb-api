use crate::Course;

use serde::{Deserialize, Serialize};
use smmdb_auth::Account;
use smmdb_lib::proto::SMMCourse::smmcourse::{AutoScroll, CourseTheme, GameStyle};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseResponse {
    id: String,
    title: String,
    maker: String,
    owner: String,
    uploader: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    game_style: GameStyle,
    course_theme: CourseTheme,
    course_theme_sub: CourseTheme,
    auto_scroll: AutoScroll,
    auto_scroll_sub: AutoScroll,
    width: i32,
    width_sub: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    nintendoid: Option<String>,
    difficulty: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    videoid: Option<String>,
    lastmodified: i32,
    uploaded: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    v_full: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    v_prev: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stars: Option<i32>,
}

impl CourseResponse {
    pub fn from_course(course: Course, account: &Account) -> CourseResponse {
        CourseResponse {
            id: course.id.to_hex(),
            title: course.title,
            maker: course.maker,
            owner: course.owner.to_hex(),
            uploader: account.get_username().clone(),
            description: course.description,
            game_style: course.game_style,
            course_theme: course.course_theme,
            course_theme_sub: course.course_theme_sub,
            auto_scroll: course.auto_scroll,
            auto_scroll_sub: course.auto_scroll_sub,
            width: course.width,
            width_sub: course.width_sub,
            nintendoid: course.nintendoid,
            difficulty: course.difficulty,
            videoid: course.videoid,
            lastmodified: course.lastmodified,
            uploaded: course.uploaded,
            v_full: course.v_full,
            v_prev: course.v_prev,
            stars: course.stars,
        }
    }
}
