use crate::server::ServerData;

use actix_web::{error::ResponseError, http::StatusCode, HttpRequest, HttpResponse};
use bson::oid::ObjectId;
use paperclip::actix::{api_v2_errors, api_v2_operation, web, Apiv2Schema};
use serde::Deserialize;
use serde_qs::actix::QsQuery;
use thiserror::Error;

#[api_v2_operation(tags(SMM2))]
pub async fn get_thumbnail(
    data: web::Data<ServerData>,
    path: web::Path<String>,
    query: QsQuery<GetThumbnail2>,
    _req: HttpRequest,
) -> Result<HttpResponse, GetCourse2ThumbnailError> {
    let course_id = path.into_inner();
    let course_id = ObjectId::with_string(&course_id)?;
    let thumb = data
        .get_course2_thumbnail(course_id, query.into_inner())
        .await?;
    Ok(HttpResponse::Ok().content_type("image/jpeg").body(thumb))
}

#[derive(Apiv2Schema, Debug, Deserialize)]
pub struct GetThumbnail2 {
    #[serde(default)]
    pub size: Size2,
}

#[derive(Apiv2Schema, Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Size2 {
    S,
    M,
    L,
    Original,
    Encrypted,
}

impl Size2 {
    pub fn get_dimensions(&self) -> (u32, u32) {
        match *self {
            Size2::S => (160, 90),
            Size2::M => (320, 180),
            Size2::L => (480, 270),
            Size2::Original => (640, 360),
            Size2::Encrypted => (640, 360),
        }
    }
}

impl Default for Size2 {
    fn default() -> Self {
        Size2::Original
    }
}

impl From<Size2> for String {
    fn from(val: Size2) -> Self {
        match val {
            Size2::S => "thumb_s".to_string(),
            Size2::M => "thumb_m".to_string(),
            Size2::L => "thumb_l".to_string(),
            Size2::Original => "thumb".to_string(),
            Size2::Encrypted => "thumb_encrypted".to_string(),
        }
    }
}

#[api_v2_errors(code = 400, code = 404, code = 500)]
#[derive(Apiv2Schema, Debug, Error)]
pub enum GetCourse2ThumbnailError {
    #[error("[GetCourse2ThumbnailError::CourseNotFound]")]
    CourseNotFound(ObjectId),
    #[error("[GetCourse2ThumbnailError::MongoOid]: {0}")]
    MongoOid(#[from] bson::oid::Error),
    #[error("[GetCourse2ThumbnailError::Mongo]: {0}")]
    Mongo(#[from] mongodb::error::Error),
    #[error("[GetCourse2ThumbnailError::Image]: {0}")]
    Image(#[from] image::ImageError),
}

impl ResponseError for GetCourse2ThumbnailError {
    fn error_response(&self) -> HttpResponse {
        match *self {
            GetCourse2ThumbnailError::CourseNotFound(_) => HttpResponse::new(StatusCode::NOT_FOUND),
            GetCourse2ThumbnailError::MongoOid(bson::oid::Error::FromHexError(_)) => {
                HttpResponse::new(StatusCode::BAD_REQUEST)
            }
            GetCourse2ThumbnailError::MongoOid(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            GetCourse2ThumbnailError::Mongo(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            GetCourse2ThumbnailError::Image(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}
