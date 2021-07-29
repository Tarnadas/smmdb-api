use crate::server::ServerData;

use actix_http::http::header;
use actix_web::{error::ResponseError, http::StatusCode, HttpResponse};
use bson::{document::ValueAccessError, oid::ObjectId};
use paperclip::actix::{api_v2_errors, api_v2_operation, web, Apiv2Schema};
use serde::Deserialize;
use serde_qs::actix::QsQuery;
use smmdb_db::DatabaseError;
use std::{io, time::SystemTime};
use tar::{Builder, Header};
use thiserror::Error;

#[api_v2_operation(tags(SMM2))]
pub async fn download_course(
    data: web::Data<ServerData>,
    path: web::Path<String>,
    query: QsQuery<DownloadCourse2>,
) -> Result<HttpResponse, DownloadCourse2Error> {
    let course_id = path.into_inner();
    let course_oid = ObjectId::with_string(&course_id)?;

    let (data, thumb) = match (&query.course_format, &query.thumb_format) {
        (CourseFormat::Encrypted, ThumbFormat::Encrypted) => data.get_course2(course_oid).await?,
        (CourseFormat::Br, ThumbFormat::Encrypted) => data.get_course2_br(course_oid).await?,
        (CourseFormat::ProtobufBr, ThumbFormat::Encrypted) => {
            data.get_course2_proto(course_oid).await?
        }
    };

    match query.file_format {
        FileFormat::Tar => {
            let mut builder = Builder::new(vec![]);
            let mtime = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let mut header = Header::new_gnu();
            header
                .set_path(match &query.course_format {
                    CourseFormat::Encrypted => "course_data_000.bcd",
                    CourseFormat::Br => "course_data_000.br",
                    CourseFormat::ProtobufBr => "course_data_000.proto.br",
                })
                .unwrap();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_mtime(mtime);
            header.set_cksum();
            builder.append(&header, &data[..])?;

            let mut header = Header::new_gnu();
            header.set_path("course_thumb_000.btl").unwrap();
            header.set_size(thumb.len() as u64);
            header.set_mode(0o644);
            header.set_mtime(mtime);
            header.set_cksum();
            builder.append(&header, &thumb[..])?;

            Ok(HttpResponse::Ok()
                .content_type("application/x-tar")
                .set_header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}.tar\"", course_id),
                )
                .body(builder.into_inner()?))
        }
    }
}

#[derive(Apiv2Schema, Debug, Deserialize)]
pub struct DownloadCourse2 {
    #[serde(default)]
    pub file_format: FileFormat,
    #[serde(default)]
    pub course_format: CourseFormat,
    #[serde(default)]
    pub thumb_format: ThumbFormat,
}

#[derive(Apiv2Schema, Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FileFormat {
    Tar,
}

impl Default for FileFormat {
    fn default() -> Self {
        FileFormat::Tar
    }
}

#[derive(Apiv2Schema, Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CourseFormat {
    Encrypted,
    Br,
    ProtobufBr,
}

impl Default for CourseFormat {
    fn default() -> Self {
        CourseFormat::Encrypted
    }
}

#[derive(Apiv2Schema, Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ThumbFormat {
    Encrypted,
}

impl Default for ThumbFormat {
    fn default() -> Self {
        ThumbFormat::Encrypted
    }
}

#[api_v2_errors(code = 400, code = 404, code = 500)]
#[derive(Apiv2Schema, Debug, Error)]
pub enum DownloadCourse2Error {
    #[error("[DownloadCourse2Error::CourseNotFound]")]
    CourseNotFound(ObjectId),
    #[error("[DownloadCourse2Error::IoError]: {0}")]
    IoError(#[from] io::Error),
    #[error("[DownloadCourse2Error::MongoOid]: {0}")]
    MongoOid(#[from] bson::oid::Error),
    #[error("[DownloadCourse2Error::Mongo]: {0}")]
    Mongo(#[from] mongodb::error::Error),
    #[error("[DownloadCourse2Error::Database: {0}")]
    Database(#[from] DatabaseError),
    #[error("[DownloadCourse2Error::ValueAccessError: {0}")]
    ValueAccess(#[from] ValueAccessError),
}

impl ResponseError for DownloadCourse2Error {
    fn error_response(&self) -> HttpResponse {
        match *self {
            DownloadCourse2Error::IoError(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            DownloadCourse2Error::CourseNotFound(_) => HttpResponse::new(StatusCode::NOT_FOUND),
            DownloadCourse2Error::MongoOid(bson::oid::Error::FromHexError(_)) => {
                HttpResponse::new(StatusCode::BAD_REQUEST)
            }
            DownloadCourse2Error::MongoOid(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            DownloadCourse2Error::Mongo(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
            DownloadCourse2Error::Database(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
            DownloadCourse2Error::ValueAccess(_) => {
                HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}
