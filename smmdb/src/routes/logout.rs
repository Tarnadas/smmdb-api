use crate::server::ServerData;

use actix_web::{
    dev::{self, HttpResponseBuilder},
    error::ResponseError,
    http::StatusCode,
    post, web, HttpRequest, HttpResponse,
};
use smmdb_auth::Identity;

pub fn service() -> impl dev::HttpServiceFactory {
    web::scope("/logout").service(logout)
}

#[post("")]
async fn logout(
    data: web::Data<ServerData>,
    _req: HttpRequest,
    identity: Identity,
) -> Result<HttpResponse, LogoutError> {
    let account = identity.get_account();
    data.delete_account_session(account)?;
    Ok(HttpResponseBuilder::new(StatusCode::OK).finish())
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
