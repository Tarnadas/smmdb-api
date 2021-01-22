mod response;

pub use response::CourseResponse;

use bson::{oid::ObjectId, ordered::OrderedDocument, ValueAccessError};
use serde::Serialize;
use smmdb_lib::proto::SMMCourse::smmcourse::{AutoScroll, CourseTheme, GameStyle};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Course {
    #[serde(rename = "_id")]
    id: ObjectId,
    title: String,
    maker: String,
    owner: ObjectId,
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

impl From<OrderedDocument> for Course {
    fn from(document: OrderedDocument) -> Course {
        Course {
            id: document
                .get_object_id("_id")
                .expect("[Course::from] id unwrap failed")
                .to_owned(),
            title: document
                .get_str("title")
                .expect("[Course::from] title unwrap failed")
                .to_string(),
            maker: document
                .get_str("maker")
                .expect("[Course::from] maker unwrap failed")
                .to_string(),
            owner: document
                .get_object_id("owner")
                .expect("[Course::from] owner unwrap failed")
                .to_owned(),
            description: document.get_str("description").ok().map(|d| d.to_string()),
            game_style: Course::map_to_game_style(&document, "gameStyle")
                .expect("[Course::from] game_style unwrap failed"),
            course_theme: Course::map_to_course_theme(&document, "courseTheme")
                .expect("[Course::from] course_theme unwrap failed"),
            course_theme_sub: Course::map_to_course_theme(&document, "courseThemeSub")
                .expect("[Course::from] course_theme_sub unwrap failed"),
            auto_scroll: Course::map_to_auto_scroll(&document, "autoScroll")
                .expect("[Course::from] auto_scroll unwrap failed"),
            auto_scroll_sub: Course::map_to_auto_scroll(&document, "autoScrollSub")
                .expect("[Course::from] auto_scroll_sub unwrap failed"),
            width: document
                .get_i32("width")
                .expect("[Course::from] width unwrap failed"),
            width_sub: document
                .get_i32("widthSub")
                .expect("[Course::from] width_sub unwrap failed"),
            nintendoid: document.get_str("nintendoid").ok().map(|u| u.to_string()),
            difficulty: document.get_i32("difficulty").ok(),
            videoid: document.get_str("videoid").ok().map(|id| id.to_string()),
            lastmodified: document
                .get_i32("lastmodified")
                .expect("[Course::from] lastmodified unwrap failed"),
            uploaded: document
                .get_i32("uploaded")
                .expect("[Course::from] uploaded unwrap failed"),
            v_full: document.get_i32("vFull").ok(),
            v_prev: document.get_i32("vPrev").ok(),
            stars: document.get_i32("stars").ok(),
        }
    }
}

impl Course {
    pub fn get_owner(&self) -> &ObjectId {
        &self.owner
    }

    fn map_to_auto_scroll(
        document: &OrderedDocument,
        identifier: &str,
    ) -> Result<AutoScroll, ValueAccessError> {
        match document.get_i32(identifier)? {
            0 => Ok(AutoScroll::DISABLED),
            1 => Ok(AutoScroll::SLOW),
            2 => Ok(AutoScroll::MEDIUM),
            3 => Ok(AutoScroll::FAST),
            4 => Ok(AutoScroll::LOCK),
            _ => Err(ValueAccessError::UnexpectedType),
        }
    }

    fn map_to_course_theme(
        document: &OrderedDocument,
        identifier: &str,
    ) -> Result<CourseTheme, ValueAccessError> {
        match document.get_i32(identifier)? {
            0 => Ok(CourseTheme::GROUND),
            1 => Ok(CourseTheme::UNDERGROUND),
            2 => Ok(CourseTheme::CASTLE),
            3 => Ok(CourseTheme::AIRSHIP),
            4 => Ok(CourseTheme::UNDERWATER),
            5 => Ok(CourseTheme::GHOUST_HOUSE),
            _ => Err(ValueAccessError::UnexpectedType),
        }
    }

    fn map_to_game_style(
        document: &OrderedDocument,
        identifier: &str,
    ) -> Result<GameStyle, ValueAccessError> {
        match document.get_i32(identifier)? {
            0 => Ok(GameStyle::M1),
            1 => Ok(GameStyle::M3),
            2 => Ok(GameStyle::MW),
            3 => Ok(GameStyle::WU),
            _ => Err(ValueAccessError::UnexpectedType),
        }
    }
}
