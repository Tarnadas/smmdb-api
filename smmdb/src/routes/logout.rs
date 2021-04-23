use crate::server::ServerData;

use actix_web::{dev, error::ResponseError, http::StatusCode, HttpRequest, HttpResponse};
use paperclip::actix::{api_v2_operation, web, Mountable, NoContent};
use smmdb_auth::Identity;

pub fn service() -> impl dev::HttpServiceFactory + Mountable {
    web::resource("/logout").route(web::post().to(logout))
}

#[api_v2_operation(tags(Auth))]
async fn logout(
    data: web::Data<ServerData>,
    _req: HttpRequest,
    identity: Identity,
) -> Result<NoContent, LogoutError> {
    let account = identity.get_account();
    data.delete_account_session(account)?;
    Ok(NoContent)
}

#[derive(Fail, Debug)]
enum LogoutError {
    #[fail(display = "Mongodb error: {}", _0)]
    MongodbError(mongodb::Error),
}

impl From<mongodb::Error> for LogoutError {
    fn from(err: mongodb::Error) -> Self {
        LogoutError::MongodbError(err)
    }
}

impl ResponseError for LogoutError {
    fn error_response(&self) -> HttpResponse {
        match *self {
            LogoutError::MongodbError(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}
