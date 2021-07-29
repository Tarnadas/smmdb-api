use crate::server::ServerData;

use actix_http::body::Body;
use actix_web::{
    error::{PayloadError, ResponseError},
    http::StatusCode,
    web::{self},
    HttpRequest, HttpResponse,
};
use futures::{self, StreamExt};
use paperclip::actix::{api_v2_errors, api_v2_operation, Apiv2Schema};
use serde::{Deserialize, Serialize, Serializer};
use serde_qs::actix::QsQuery;
use smmdb_auth::Identity;
use smmdb_common::{Course2Response, Course2SimilarityError, Difficulty};
use std::io;
use thiserror::Error;

#[derive(Apiv2Schema, Debug, Deserialize)]
pub struct PutCourses2 {
    difficulty: Option<Difficulty>,
}

#[api_v2_operation(tags(SMM2))]
pub async fn put_courses(
    data: web::Data<ServerData>,
    _req: HttpRequest,
    query: QsQuery<PutCourses2>,
    mut payload: web::Payload,
    identity: Identity,
) -> Result<web::Json<PutCourses2Response>, PutCourses2Error> {
    let query = query.into_inner();
    let mut bytes = web::BytesMut::new();
    while let Some(item) = payload.next().await {
        bytes.extend_from_slice(&item?);
    }
    match smmdb_lib::Course2::from_packed(&bytes[..]) {
        Ok(courses) => {
            let account = identity.get_account();
            match data.put_courses2(courses, &account, query.difficulty).await {
                Ok(res) => Ok(web::Json(res)),
                Err(err) => Err(err),
            }
        }
        Err(err) => Err(PutCourses2Error::from(err)),
    }
}

#[api_v2_errors(code = 400, code = 404, code = 500)]
#[derive(Apiv2Schema, Debug, Error)]
pub enum PutCourses2Error {
    #[error("[PutCourses2Error::Course2SimilarityError]: {0}")]
    Similarity(Course2SimilarityError),
    #[error("[PutCourses2Error::Io]: {0}")]
    Io(#[from] io::Error),
    #[error("[PutCourses2Error::Payload]: {0}")]
    Payload(#[from] PayloadError),
    #[error("[PutCourses2Error::Smmdb]: {0}")]
    Smmdb(#[from] smmdb_lib::Error),
    #[error("[PutCourses2Error::SerdeJson]: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("[PutCourses2Error::ThumbnailMissing]")]
    ThumbnailMissing,
    #[error("[PutCourses2Error::Mongo]: {0}")]
    Mongo(#[from] mongodb::error::Error),
}

impl ResponseError for PutCourses2Error {
    fn error_response(&self) -> HttpResponse {
        let res = match *self {
            PutCourses2Error::Io(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
            PutCourses2Error::Similarity(_) => HttpResponse::new(StatusCode::BAD_REQUEST),
            PutCourses2Error::Payload(_) => HttpResponse::new(StatusCode::BAD_REQUEST),
            PutCourses2Error::Smmdb(_) => HttpResponse::new(StatusCode::BAD_REQUEST),
            PutCourses2Error::SerdeJson(_) => HttpResponse::new(StatusCode::BAD_REQUEST),
            PutCourses2Error::ThumbnailMissing => HttpResponse::new(StatusCode::BAD_REQUEST),
            PutCourses2Error::Mongo(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
        };
        res.set_body(Body::from(format!("{}", self)))
    }
}

impl Serialize for PutCourses2Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let PutCourses2Error::Similarity(err) = self {
            err.serialize(serializer)
        } else {
            serializer.collect_str(&format!("{}", self))
        }
    }
}

#[derive(Apiv2Schema, Debug, Serialize)]
pub struct PutCourses2Response {
    succeeded: Vec<Course2Response>,
    failed: Vec<PutCourses2Error>,
}

impl PutCourses2Response {
    pub fn new() -> Self {
        PutCourses2Response {
            succeeded: vec![],
            failed: vec![],
        }
    }

    pub fn set_succeeded(&mut self, succeeded: Vec<Course2Response>) {
        self.succeeded = succeeded;
    }

    pub fn add_failed(&mut self, failed: PutCourses2Error) {
        self.failed.push(failed);
    }
}

impl Default for PutCourses2Response {
    fn default() -> Self {
        Self::new()
    }
}
