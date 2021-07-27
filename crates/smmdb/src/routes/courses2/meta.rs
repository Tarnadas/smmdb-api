use crate::server::ServerData;

use actix_http::body::Body;
use actix_web::{error::ResponseError, http::StatusCode, HttpRequest, HttpResponse};
use bson::oid::ObjectId;
use paperclip::actix::{api_v2_errors, api_v2_operation, web, Apiv2Schema, NoContent};
use serde::Deserialize;
use smmdb_auth::Identity;
use smmdb_common::Difficulty;
use thiserror::Error;

#[derive(Apiv2Schema, Debug, Deserialize)]
pub struct PostCourse2Meta {
    difficulty: Option<Difficulty>,
}

#[api_v2_operation(tags(SMM2))]
pub async fn post_meta(
    data: web::Data<ServerData>,
    path: web::Path<String>,
    meta: web::Json<PostCourse2Meta>,
    _req: HttpRequest,
    identity: Identity,
) -> Result<NoContent, PostCourse2MetaError> {
    let course_id = path.into_inner();
    let course_id = ObjectId::with_string(&course_id)?;
    let account = identity.get_account();
    if !data.does_account_own_course(account.get_id().clone(), course_id.clone()) {
        return Err(PostCourse2MetaError::Unauthorized);
    }
    let difficulty = meta.difficulty.clone();
    data.post_course2_meta(course_id, difficulty)?;
    Ok(NoContent)
}

#[api_v2_errors(code = 400, code = 401, code = 404, code = 500)]
#[derive(Apiv2Schema, Debug, Error)]
pub enum PostCourse2MetaError {
    #[error("[PutCourses2Error::MongoOid]: {0}")]
    MongoOid(#[from] bson::oid::Error),
    #[error("[PutCourses2Error::Mongo]: {0}")]
    Mongo(#[from] mongodb::Error),
    #[error("[PutCourses2Error::MongoColl]: {0}")]
    MongoColl(#[from] mongodb::coll::error::WriteException),
    #[error("[PutCourses2Error::Unauthorized]")]
    Unauthorized,
}

impl ResponseError for PostCourse2MetaError {
    fn error_response(&self) -> HttpResponse {
        let res = match *self {
            PostCourse2MetaError::MongoOid(bson::oid::Error::FromHexError(_)) => {
                HttpResponse::new(StatusCode::BAD_REQUEST)
            }
            PostCourse2MetaError::MongoOid(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            PostCourse2MetaError::Mongo(_) => HttpResponse::new(StatusCode::NOT_FOUND),
            PostCourse2MetaError::MongoColl(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            PostCourse2MetaError::Unauthorized => HttpResponse::new(StatusCode::UNAUTHORIZED),
        };
        res.set_body(Body::from(format!("{}", self)))
    }
}
