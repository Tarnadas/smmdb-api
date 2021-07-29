use crate::server::ServerData;

use actix_http::body::Body;
use actix_web::{error::ResponseError, http::StatusCode, HttpRequest, HttpResponse};
use bson::oid::ObjectId;
use paperclip::actix::{api_v2_errors, api_v2_operation, web, Apiv2Schema, NoContent};
use serde::Deserialize;
use smmdb_auth::Identity;
use thiserror::Error;

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
    data.vote_course2(account.get_id().clone(), course_oid, body.value)
        .await?;
    Ok(NoContent)
}

#[api_v2_errors(code = 400, code = 404, code = 500)]
#[derive(Debug, Error)]
pub enum VoteCourse2Error {
    #[error("[VoteCourse2Error::MongoOid]: {0}")]
    MongoOid(#[from] bson::oid::Error),
    #[error("[VoteCourse2Error::Mongo]: {0}")]
    Mongo(#[from] mongodb::error::Error),
    #[error("[VoteCourse2Error::Anyhow]: {0}")]
    Anyhow(#[from] anyhow::Error),
    #[error("[VoteCourse2Error::BadValue]: {0}")]
    BadValue(i32),
}

impl ResponseError for VoteCourse2Error {
    fn error_response(&self) -> HttpResponse {
        let res = match *self {
            VoteCourse2Error::MongoOid(_) => HttpResponse::new(StatusCode::BAD_REQUEST),
            VoteCourse2Error::Mongo(_) | VoteCourse2Error::Anyhow(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            VoteCourse2Error::BadValue(_) => HttpResponse::new(StatusCode::BAD_REQUEST),
        };
        res.set_body(Body::from(format!("{}", self)))
    }
}
