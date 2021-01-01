use actix_http::{body::Body, Response};
use actix_web::http::StatusCode;
use paperclip::actix::api_v2_operation;

static INDEX: &str = include_str!("../../swagger/index.html");

#[api_v2_operation]
pub fn index() -> Response {
    Response::with_body(StatusCode::OK, Body::from_message(INDEX))
}
