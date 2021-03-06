use crate::server::ServerData;

use actix_web::{error::ResponseError, http::StatusCode, HttpResponse};
use bson::oid::ObjectId;
use paperclip::actix::{api_v2_operation, web};
use smmdb_auth::Identity;

#[api_v2_operation(tags(SMM2))]
pub async fn delete_course(
    data: web::Data<ServerData>,
    path: web::Path<String>,
    identity: Identity,
) -> Result<HttpResponse, DeleteCourse2Error> {
    let course_id = path.into_inner();
    let course_oid = ObjectId::with_string(&course_id)?;
    let account = identity.get_account();
    if !data.does_account_own_course(account.get_id().clone(), course_oid.clone()) {
        return Err(DeleteCourse2Error::Unauthorized.into());
    }
    data.delete_course2(course_id, course_oid)?;
    Ok(HttpResponse::NoContent().into())
}

#[derive(Debug, Fail)]
pub enum DeleteCourse2Error {
    #[fail(display = "Object id invalid.\nReason: {}", _0)]
    MongoOid(bson::oid::Error),
    #[fail(display = "Course with ID {} not found", _0)]
    ObjectIdUnknown(String),
    #[fail(display = "[DeleteCourse2Error::Mongo]: {}", _0)]
    Mongo(mongodb::Error),
    #[fail(display = "")]
    Unauthorized,
}

impl From<bson::oid::Error> for DeleteCourse2Error {
    fn from(err: bson::oid::Error) -> Self {
        match err {
            bson::oid::Error::ArgumentError(s) => DeleteCourse2Error::ObjectIdUnknown(s),
            _ => DeleteCourse2Error::MongoOid(err),
        }
    }
}

impl From<mongodb::Error> for DeleteCourse2Error {
    fn from(err: mongodb::Error) -> Self {
        DeleteCourse2Error::Mongo(err)
    }
}

impl ResponseError for DeleteCourse2Error {
    fn error_response(&self) -> HttpResponse {
        match *self {
            DeleteCourse2Error::MongoOid(_) => HttpResponse::new(StatusCode::BAD_REQUEST),
            DeleteCourse2Error::ObjectIdUnknown(_) => HttpResponse::new(StatusCode::NOT_FOUND),
            DeleteCourse2Error::Mongo(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
            DeleteCourse2Error::Unauthorized => HttpResponse::new(StatusCode::UNAUTHORIZED),
        }
    }
}
