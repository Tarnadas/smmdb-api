use crate::server::ServerData;

use actix_session::Session;
use actix_web::{
    client::Client, dev, error::ResponseError, http::StatusCode, HttpRequest, HttpResponse,
};
use awc::{error::JsonPayloadError, SendClientRequest};
use paperclip::actix::{api_v2_errors, api_v2_operation, web, Apiv2Schema, Mountable};
use serde::Deserialize;
use smmdb_auth::{AccountConvertError, AccountReq, AccountRes, AuthSession, IdInfo, Identity};
use std::convert::TryInto;
use thiserror::Error;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Login {
    token_obj: TokenObj,
}

#[derive(Debug, Deserialize)]
struct TokenObj {
    id_token: String,
    expires_at: i64,
}

pub fn service() -> impl dev::HttpServiceFactory + Mountable {
    web::scope("/login")
        .service(web::resource("").route(web::post().to(login)))
        .service(web::resource("/google").route(web::post().to(login_with_google)))
}

#[api_v2_operation(tags(Auth))]
fn login(
    _data: web::Data<ServerData>,
    _req: HttpRequest,
    identity: Identity,
) -> web::Json<AccountRes> {
    let account = identity.get_account();
    web::Json(AccountRes::new(&account))
}

#[api_v2_operation(tags(Auth))]
async fn login_with_google(
    data: web::Data<ServerData>,
    _req: HttpRequest,
    json: web::Json<Login>,
    client: web::Data<Client>,
    session: Session,
) -> Result<web::Json<AccountRes>, LoginError> {
    let id_token = json.token_obj.id_token.clone();
    let request: SendClientRequest = client
        .get(&format!(
            "https://oauth2.googleapis.com/tokeninfo?id_token={}",
            id_token
        ))
        .send();
    let mut response = request.await.map_err(|_| LoginError::Request)?;

    // TODO handle bad status codes
    match response.status() {
        x if x.is_success() => {}
        x if x.is_client_error() => {}
        x if x.is_server_error() => {}
        _ => {}
    };
    let id_info: IdInfo = response.json().await?;
    if data.google_client_id != id_info.aud {
        Err(LoginError::ClientIdInvalid(id_info.aud))
    } else {
        let account: AccountReq = id_info.try_into()?;
        session.set("id_token", id_token.clone()).unwrap();
        session
            .set("expires_at", json.token_obj.expires_at)
            .unwrap();
        let account = data.add_or_get_account(
            account,
            AuthSession::new(id_token.clone(), json.token_obj.expires_at),
        )?;
        // TODO get stars from database
        let account = AccountRes::new(&account);
        session.set("account_id", account.get_id()).unwrap();
        Ok(web::Json(account))
    }
}

#[api_v2_errors(code = 400, code = 500)]
#[derive(Apiv2Schema, Debug, Error)]
enum LoginError {
    #[error("[LoginError::ClientIdInvalid]: {0}")]
    ClientIdInvalid(String),
    #[error("[LoginError::Request]")]
    Request,
    #[error("[LoginError::JsonPayload]: {0}")]
    JsonPayload(#[from] JsonPayloadError),
    #[error("[LoginError::SerdeJson]: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("[LoginError::AccountConvert]: {0}")]
    AccountConvert(#[from] AccountConvertError),
    #[error("[LoginError::Mongodb]: {0}")]
    Mongodb(#[from] mongodb::Error),
}

impl ResponseError for LoginError {
    fn error_response(&self) -> HttpResponse {
        match *self {
            LoginError::ClientIdInvalid(_) => HttpResponse::new(StatusCode::BAD_REQUEST),
            LoginError::Request => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
            LoginError::JsonPayload(_) => HttpResponse::new(StatusCode::BAD_REQUEST),
            LoginError::SerdeJson(_) => HttpResponse::new(StatusCode::BAD_REQUEST),
            LoginError::AccountConvert(_) => HttpResponse::new(StatusCode::BAD_REQUEST),
            LoginError::Mongodb(_) => HttpResponse::new(StatusCode::BAD_REQUEST),
        }
    }
}
