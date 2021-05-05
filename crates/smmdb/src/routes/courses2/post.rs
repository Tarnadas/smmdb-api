use crate::server::ServerData;

use smmdb_lib::{course2::Course2, proto::SMM2Course::SMM2Course};

use actix_web::{
    error::{PayloadError, ResponseError},
    http::StatusCode,
    HttpResponse,
};
use futures::{self, StreamExt};
use paperclip::actix::{api_v2_errors, api_v2_operation, web, Apiv2Schema};
use thiserror::Error;

#[api_v2_operation(tags(SMM2))]
pub async fn post_analyze_courses(
    _data: web::Data<ServerData>,
    mut payload: web::Payload,
) -> Result<web::Json<Vec<SMM2Course>>, PostCourses2Error> {
    let mut bytes = web::BytesMut::new();
    while let Some(item) = payload.next().await {
        bytes.extend_from_slice(&item?);
    }
    match Course2::from_packed(&bytes[..]) {
        Ok(courses) => {
            let courses: Vec<SMM2Course> = courses
                .into_iter()
                .map(|course| course.take_course())
                .collect();
            Ok(web::Json(courses))
        }
        Err(err) => Err(PostCourses2Error::from(err)),
    }
}

#[api_v2_errors(code = 400)]
#[derive(Apiv2Schema, Debug, Error)]
pub enum PostCourses2Error {
    #[error("[PostCourses2Error::Payload]: {0}")]
    Payload(#[from] PayloadError),
    #[error("[PostCourses2Error::Smmdb]: {0}")]
    Smmdb(#[from] smmdb_lib::Error),
}

impl ResponseError for PostCourses2Error {
    fn error_response(&self) -> HttpResponse {
        match *self {
            PostCourses2Error::Payload(_) => HttpResponse::new(StatusCode::BAD_REQUEST),
            PostCourses2Error::Smmdb(_) => HttpResponse::new(StatusCode::BAD_REQUEST),
        }
    }
}
