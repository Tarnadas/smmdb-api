use crate::server::ServerData;

use actix_web::{dev, error::ResponseError, http::StatusCode, HttpResponse};
use paperclip::actix::{api_v2_errors, api_v2_operation, web, Apiv2Schema, Mountable, NoContent};
use smmdb_auth::Identity;
use thiserror::Error;

pub fn service() -> impl dev::HttpServiceFactory + Mountable {
    web::resource("/logout").route(web::post().to(logout))
}

#[api_v2_operation(tags(Auth))]
async fn logout(data: web::Data<ServerData>, identity: Identity) -> Result<NoContent, LogoutError> {
    let account = identity.get_account();
    data.delete_account_session(account)?;
    Ok(NoContent)
}

#[api_v2_errors(code = 500)]
#[derive(Apiv2Schema, Debug, Error)]
enum LogoutError {
    #[error("[LogoutError::Mongodb]: {0}")]
    Mongodb(#[from] mongodb::error::Error),
}

impl ResponseError for LogoutError {
    fn error_response(&self) -> HttpResponse {
        match *self {
            LogoutError::Mongodb(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}
