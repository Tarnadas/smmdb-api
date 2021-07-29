use crate::server::ServerData;

use actix_web::{error::ResponseError, http::StatusCode, HttpResponse};
use bson::oid::ObjectId;
use paperclip::actix::{api_v2_errors, api_v2_operation, web, Apiv2Schema, NoContent};
use smmdb_auth::Identity;
use smmdb_db::DatabaseError;
use thiserror::Error;

#[api_v2_operation(tags(SMM2))]
pub async fn delete_course(
    data: web::Data<ServerData>,
    path: web::Path<String>,
    identity: Identity,
) -> Result<NoContent, DeleteCourse2Error> {
    let course_id = path.into_inner();
    let course_oid = ObjectId::with_string(&course_id)?;
    let account = identity.get_account();
    if !data.does_account_own_course(account.get_id().clone(), course_oid.clone()) {
        return Err(DeleteCourse2Error::Unauthorized);
    }
    data.delete_course2(course_id, course_oid).await?;
    Ok(NoContent)
}

#[api_v2_errors(code = 400, code = 401, code = 404, code = 500)]
#[derive(Apiv2Schema, Debug, Error)]
pub enum DeleteCourse2Error {
    #[error("Object id invalid.\nReason: {0}")]
    MongoOid(#[from] bson::oid::Error),
    #[error("[DeleteCourse2Error::Mongo]: {0}")]
    Mongo(#[from] mongodb::error::Error),
    #[error("[DatabaseError]: {0}")]
    Database(#[from] DatabaseError),
    #[error("[DeleteCourse2Error::Unauthorized]")]
    Unauthorized,
}

impl ResponseError for DeleteCourse2Error {
    fn error_response(&self) -> HttpResponse {
        match *self {
            DeleteCourse2Error::MongoOid(_) => HttpResponse::new(StatusCode::BAD_REQUEST),
            DeleteCourse2Error::Mongo(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
            DeleteCourse2Error::Database(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
            DeleteCourse2Error::Unauthorized => HttpResponse::new(StatusCode::UNAUTHORIZED),
        }
    }
}
