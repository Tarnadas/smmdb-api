use crate::{
    server::{Data, ServerData},
    Database,
};

use actix_web::{dev, error::ResponseError, http::StatusCode, HttpRequest, HttpResponse};
use bson::{oid::ObjectId, Bson, Document, Regex};
use paperclip::actix::{api_v2_errors, api_v2_operation, web, Apiv2Schema, Mountable};
use protobuf::ProtobufEnum;
use serde::Deserialize;
use serde_qs::actix::QsQuery;
use smmdb_lib::proto::SMMCourse::{
    SMMCourse_AutoScroll, SMMCourse_CourseTheme, SMMCourse_GameStyle,
};
use thiserror::Error;

pub fn service() -> impl dev::HttpServiceFactory + Mountable {
    web::resource("/courses").route(web::get().to(get_courses))
}

#[api_v2_operation(tags(SMM1))]
async fn get_courses(
    data: web::Data<ServerData>,
    query: QsQuery<GetCourses>,
    _req: HttpRequest,
) -> Result<String, GetCoursesError> {
    data.get_courses(query.into_inner()).await
}

#[derive(Apiv2Schema, Deserialize, Debug)]
pub struct GetCourses {
    #[serde(default)]
    limit: Limit,
    skip: Option<u32>,
    id: Option<String>,
    ids: Option<Vec<String>>,
    title: Option<String>,
    maker: Option<String>,
    owner: Option<String>,
    uploader: Option<String>,
    game_style: Option<Vec<SMMCourse_GameStyle>>,
    course_theme: Option<Vec<SMMCourse_CourseTheme>>,
    course_theme_sub: Option<Vec<SMMCourse_CourseTheme>>,
    auto_scroll: Option<Vec<SMMCourse_AutoScroll>>,
    auto_scroll_sub: Option<Vec<SMMCourse_AutoScroll>>,
    width_gte: Option<i32>,
    width_lte: Option<i32>,
    width_sub_gte: Option<i32>,
    width_sub_lte: Option<i32>,
    nintendo_id: Option<String>,
    difficulty_gte: Option<i32>,
    difficulty_lte: Option<i32>,
    video_id: Option<String>,
    lastmodified_gte: Option<i64>,
    lastmodified_lte: Option<i64>,
    uploaded_gte: Option<i64>,
    uploaded_lte: Option<i64>,
    stars_gte: Option<i32>,
    stars_lte: Option<i32>,
}

impl GetCourses {
    pub async fn into_ordered_document(
        self,
        database: &Database,
    ) -> Result<Vec<Document>, GetCoursesError> {
        let mut pipeline = vec![];

        if let Some(pipeline_match) = self.get_match(database).await? {
            pipeline.push(doc! { "$match": pipeline_match });
        }

        let limit = self.get_limit()?;
        pipeline.push(doc! {
            "$limit": limit
        });

        if let Some(skip) = self.skip {
            pipeline.push(doc! {
                "$skip": skip
            });
        }

        Ok(pipeline)
    }

    async fn get_match(&self, database: &Database) -> Result<Option<Document>, GetCoursesError> {
        let mut res = doc! {};
        if let Some(id) = &self.id {
            GetCourses::insert_objectid(&mut res, "_id".to_string(), id)?;
        }

        if let Some(ids) = self.ids.clone() {
            let ids: Vec<Bson> = ids
                .iter()
                .map(|id| -> Result<Bson, GetCoursesError> {
                    let object_id = ObjectId::with_string(id)
                        .map_err(|_| GetCoursesError::Deserialize("ids".to_string()))?;
                    Ok(Bson::ObjectId(object_id))
                })
                .filter_map(Result::ok)
                .collect();
            res.insert(
                "_id".to_string(),
                Bson::Document(doc! {
                    "$in": ids
                }),
            );
        }

        if let Some(title) = self.title.clone() {
            GetCourses::insert_regexp(&mut res, "title".to_string(), title);
        }

        if let Some(maker) = self.maker.clone() {
            GetCourses::insert_regexp(&mut res, "maker".to_string(), maker);
        }

        if let Some(owner) = &self.owner {
            GetCourses::insert_objectid(&mut res, "owner".to_string(), owner)?;
        }

        if let Some(uploader) = &self.uploader {
            let filter = doc! {
                "username": Bson::RegularExpression(Regex {
                    pattern: format!("^{}$", uploader),
                    options: "i".to_string()
                })
            };
            match Data::find_account(database, filter).await {
                Some(account) => {
                    res.insert(
                        "owner".to_string(),
                        Bson::ObjectId(account.get_id().clone()),
                    );
                }
                None => return Err(GetCoursesError::UploaderUnknown(uploader.clone())),
            };
        }

        if let Some(game_styles) = &self.game_style {
            GetCourses::insert_enum(&mut res, "gameStyle".to_string(), game_styles);
        }

        if let Some(course_themes) = &self.course_theme {
            GetCourses::insert_enum(&mut res, "courseTheme".to_string(), course_themes);
        }

        if let Some(course_themes) = &self.course_theme_sub {
            GetCourses::insert_enum(&mut res, "courseThemeSub".to_string(), course_themes);
        }

        if let Some(auto_scrolls) = &self.auto_scroll {
            GetCourses::insert_enum(&mut res, "autoScroll".to_string(), auto_scrolls);
        }

        if let Some(auto_scrolls) = &self.auto_scroll_sub {
            GetCourses::insert_enum(&mut res, "autoScrollSub".to_string(), auto_scrolls);
        }

        GetCourses::insert_boundaries(
            &mut res,
            "width".to_string(),
            self.width_gte,
            self.width_lte,
        );

        GetCourses::insert_boundaries(
            &mut res,
            "widthSub".to_string(),
            self.width_sub_gte,
            self.width_sub_lte,
        );

        if let Some(nintendo_id) = &self.nintendo_id {
            res.insert("nintendoid".to_string(), Bson::String(nintendo_id.clone()));
        }

        GetCourses::insert_boundaries(
            &mut res,
            "difficulty".to_string(),
            self.difficulty_gte,
            self.difficulty_lte,
        );

        GetCourses::insert_boundaries(
            &mut res,
            "stars".to_string(),
            self.stars_gte,
            self.stars_lte,
        );

        if res.is_empty() {
            Ok(None)
        } else {
            Ok(Some(res))
        }
    }

    fn get_limit(&self) -> Result<u32, GetCoursesError> {
        let limit = self.limit.0;
        if limit == 0 {
            return Err(GetCoursesError::LimitTooLow);
        }
        if limit > 120 {
            return Err(GetCoursesError::LimitTooHigh);
        }
        Ok(limit + self.skip.unwrap_or_default())
    }

    fn insert_regexp(doc: &mut Document, key: String, regexp: String) {
        doc.insert(
            key,
            Bson::RegularExpression(Regex {
                pattern: format!(".*{}.*", regexp),
                options: "i".to_string(),
            }),
        );
    }

    fn insert_objectid(doc: &mut Document, key: String, oid: &str) -> Result<(), GetCoursesError> {
        doc.insert(
            key.clone(),
            Bson::ObjectId(
                ObjectId::with_string(oid).map_err(|_| GetCoursesError::Deserialize(key))?,
            ),
        );
        Ok(())
    }

    fn insert_enum<T>(doc: &mut Document, key: String, enums: &[T])
    where
        T: ProtobufEnum,
    {
        let enums: Vec<Bson> = enums.iter().map(|val| Bson::Int32(val.value())).collect();
        doc.insert(
            key,
            Bson::Document(doc! {
                "$in": enums
            }),
        );
    }

    fn insert_boundaries(doc: &mut Document, key: String, gte: Option<i32>, lte: Option<i32>) {
        let mut boundaries = None;
        if let Some(gte) = gte {
            boundaries = Some(doc! {
                "$gte": gte
            });
        }
        if let Some(lte) = lte {
            match &mut boundaries {
                Some(boundaries) => {
                    boundaries.insert("$lte", lte);
                }
                None => {
                    boundaries = Some(doc! {
                        "$lte": lte
                    })
                }
            }
        }
        if let Some(boundaries) = boundaries {
            doc.insert(key, Bson::Document(boundaries));
        }
    }
}

#[derive(Apiv2Schema, Deserialize, Debug)]
struct Limit(u32);

impl Default for Limit {
    fn default() -> Limit {
        Limit(120)
    }
}

#[api_v2_errors(code = 400)]
#[derive(Apiv2Schema, Debug, Error)]
pub enum GetCoursesError {
    #[error("[GetCoursesError::LimitTooLow]: limit must be at least 1")]
    LimitTooLow,
    #[error("[GetCoursesError::LimitTooHigh]: limit must be at most 120")]
    LimitTooHigh,
    #[error("[GetCoursesError::Deserialize]: {0}")]
    Deserialize(String),
    #[error("[GetCoursesError::UploaderUnknown]: {0}")]
    UploaderUnknown(String),
}

impl ResponseError for GetCoursesError {
    fn error_response(&self) -> HttpResponse {
        match *self {
            GetCoursesError::LimitTooLow => HttpResponse::new(StatusCode::BAD_REQUEST),
            GetCoursesError::LimitTooHigh => HttpResponse::new(StatusCode::BAD_REQUEST),
            GetCoursesError::Deserialize(_) => HttpResponse::new(StatusCode::BAD_REQUEST),
            GetCoursesError::UploaderUnknown(_) => HttpResponse::new(StatusCode::BAD_REQUEST),
        }
    }
}
