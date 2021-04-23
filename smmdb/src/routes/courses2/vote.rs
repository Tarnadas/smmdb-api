use crate::server::ServerData;

use actix_http::body::Body;
use actix_web::{error::ResponseError, http::StatusCode, HttpRequest, HttpResponse};
use bson::oid::ObjectId;
use paperclip::actix::{api_v2_operation, web, Apiv2Schema, NoContent};
use serde::Deserialize;
use smmdb_auth::Identity;

#[derive(Apiv2Schema, Debug, Deserialize)]
pub struct VoteCourse2 {
    value: i32,
}

#[api_v2_operation(tags(SMM2))]
pub async fn vote_course(
    data: web::Data<ServerData>,
    path: web::Path<String>,
    body: web::Json<VoteCourse2>,
    _req: HttpRequest,
    identity: Identity,
) -> Result<NoContent, VoteCourse2Error> {
    let course_id = path.into_inner();
    let course_oid = ObjectId::with_string(&course_id)?;
    let account = identity.get_account();

    // TODO this might be beneficial
    // if data.does_account_own_course(account.get_id().clone(), course_oid) {
    //     return Err(VoteCourse2Error::Unauthorized.into());
    // }

    if body.value > 1 || body.value < -1 {
        return Err(VoteCourse2Error::BadValue(body.value));
    }
    data.vote_course2(account.get_id().clone(), course_oid, body.value)?;
    Ok(NoContent)
}

#[derive(Debug, Fail)]
pub enum VoteCourse2Error {
    #[fail(display = "Object id invalid.\nReason: {}", _0)]
    MongoOid(bson::oid::Error),
    #[fail(display = "Course with ID {} not found", _0)]
    ObjectIdUnknown(String),
    #[fail(display = "[VoteCourse2Error::Mongo]: {}", _0)]
    Mongo(mongodb::Error),
    #[fail(display = "value not within allowed range: {}", _0)]
    BadValue(i32),
    // #[fail(display = "")]
    // Unauthorized,
}

impl From<bson::oid::Error> for VoteCourse2Error {
    fn from(err: bson::oid::Error) -> Self {
        match err {
            bson::oid::Error::ArgumentError(s) => VoteCourse2Error::ObjectIdUnknown(s),
            _ => VoteCourse2Error::MongoOid(err),
        }
    }
}

impl From<mongodb::Error> for VoteCourse2Error {
    fn from(err: mongodb::Error) -> Self {
        VoteCourse2Error::Mongo(err)
    }
}

impl ResponseError for VoteCourse2Error {
    fn error_response(&self) -> HttpResponse {
        let res = match *self {
            VoteCourse2Error::MongoOid(_) => HttpResponse::new(StatusCode::BAD_REQUEST),
            VoteCourse2Error::ObjectIdUnknown(_) => HttpResponse::new(StatusCode::NOT_FOUND),
            VoteCourse2Error::Mongo(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
            VoteCourse2Error::BadValue(_) => HttpResponse::new(StatusCode::BAD_REQUEST),
            // VoteCourse2Error::Unauthorized => HttpResponse::new(StatusCode::UNAUTHORIZED),
        };
        res.set_body(Body::from(format!("{}", self)))
    }
}
