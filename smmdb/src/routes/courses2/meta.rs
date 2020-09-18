use crate::server::ServerData;

use actix_http::body::Body;
use actix_web::{error::ResponseError, http::StatusCode, post, web, HttpRequest, HttpResponse};
use bson::oid::ObjectId;
use serde::Deserialize;
use smmdb_auth::Identity;
use smmdb_common::Difficulty;

#[derive(Debug, Deserialize)]
pub struct PostCourse2Meta {
    difficulty: Option<Difficulty>,
}

#[post("meta/{course_id}")]
pub async fn post_meta(
    data: web::Data<ServerData>,
    path: web::Path<String>,
    meta: web::Json<PostCourse2Meta>,
    _req: HttpRequest,
    identity: Identity,
) -> Result<HttpResponse, PostCourse2MetaError> {
    let course_id = path.into_inner();
    let course_id = ObjectId::with_string(&course_id)?;
    let account = identity.get_account();
    if !data.does_account_own_course(account.get_id().clone(), course_id.clone()) {
        return Err(PostCourse2MetaError::Unauthorized);
    }
    let difficulty = meta.difficulty.clone();
    data.post_course2_meta(course_id, difficulty)?;
    Ok(HttpResponse::NoContent().into())
}

#[derive(Debug, Fail)]
pub enum PostCourse2MetaError {
    #[fail(display = "Object id invalid.\nReason: {}", _0)]
    MongoOid(bson::oid::Error),
    #[fail(display = "Course with ID {} not found", _0)]
    ObjectIdUnknown(String),
    #[fail(display = "[PutCourses2Error::Mongo]: {}", _0)]
    Mongo(mongodb::Error),
    #[fail(display = "[PutCourses2Error::MongoCollWriteException]: {}", _0)]
    MongoColl(mongodb::coll::error::WriteException),
    #[fail(display = "")]
    Unauthorized,
}

impl From<bson::oid::Error> for PostCourse2MetaError {
    fn from(err: bson::oid::Error) -> Self {
        match err {
            bson::oid::Error::ArgumentError(s) => PostCourse2MetaError::ObjectIdUnknown(s),
            _ => PostCourse2MetaError::MongoOid(err),
        }
    }
}

impl From<mongodb::Error> for PostCourse2MetaError {
    fn from(err: mongodb::Error) -> Self {
        PostCourse2MetaError::Mongo(err)
    }
}

impl From<mongodb::coll::error::WriteException> for PostCourse2MetaError {
    fn from(err: mongodb::coll::error::WriteException) -> Self {
        PostCourse2MetaError::MongoColl(err)
    }
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
            PostCourse2MetaError::ObjectIdUnknown(_) => HttpResponse::new(StatusCode::NOT_FOUND),
            PostCourse2MetaError::Mongo(_) => HttpResponse::new(StatusCode::NOT_FOUND),
            PostCourse2MetaError::MongoColl(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            PostCourse2MetaError::Unauthorized => HttpResponse::new(StatusCode::UNAUTHORIZED),
        };
        res.set_body(Body::from(format!("{}", self)))
    }
}
