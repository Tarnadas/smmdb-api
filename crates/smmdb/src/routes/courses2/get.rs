use crate::{
    server::{Data, ServerData},
    Database,
};

use actix_web::{error::ResponseError, http::StatusCode, Error, HttpResponse};
use bson::{oid::ObjectId, ordered::OrderedDocument, Bson};
use paperclip::actix::{api_v2_errors, api_v2_operation, web, Apiv2Schema};
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_qs::actix::QsQuery;
use smmdb_auth::Identity;
use smmdb_common::{Course2Response, Difficulty};
use std::{
    convert::{TryFrom, TryInto},
    io,
};

#[api_v2_operation(tags(SMM2))]
pub async fn get_courses(
    data: web::Data<ServerData>,
    query: QsQuery<GetCourses2>,
    identity: Option<Identity>,
) -> Result<web::Json<Vec<Course2Response>>, GetCourses2Error> {
    let res = data.get_courses2(
        query.into_inner(),
        identity.map(|identity| identity.get_account()),
    )?;
    Ok(web::Json(res))
}

fn is_true() -> bool {
    true
}

#[derive(Apiv2Schema, Deserialize, Debug)]
pub struct GetCourses2 {
    #[serde(default)]
    limit: Limit,
    skip: Option<u32>,
    id: Option<String>,
    ids: Option<Vec<String>>,
    title: Option<String>,
    #[serde(default)]
    title_exact: bool,
    #[serde(default)]
    title_case_sensitive: bool,
    #[serde(default = "is_true")]
    title_trimmed: bool,
    owner: Option<String>,
    uploader: Option<String>,
    sort: Option<Vec<Sort>>,
    difficulty: Option<Difficulty>,
}

impl GetCourses2 {
    pub fn into_ordered_document(
        self,
        database: &Database,
    ) -> Result<Vec<OrderedDocument>, GetCourses2Error> {
        let mut pipeline = vec![];

        if let Some(pipeline_match) = self.get_match(database)? {
            pipeline.push(doc! { "$match" => pipeline_match });
        }

        pipeline.push(self.get_sort_doc());

        let limit = self.get_limit();
        pipeline.push(doc! {
            "$limit" => limit
        });

        if let Some(skip) = self.skip {
            pipeline.push(doc! {
                "$skip" => skip
            });
        }

        Ok(pipeline)
    }

    fn get_match(&self, database: &Database) -> Result<Option<OrderedDocument>, GetCourses2Error> {
        let mut res = doc! {};
        if let Some(id) = &self.id {
            GetCourses2::insert_objectid(&mut res, "_id".to_string(), id)?;
        }

        if let Some(ids) = self.ids.clone() {
            let ids: Vec<Bson> = ids
                .iter()
                .map(|id| -> Result<Bson, GetCourses2Error> {
                    let object_id = ObjectId::with_string(id)
                        .map_err(|_| GetCourses2Error::DeserializeError("ids".to_string()))?;
                    Ok(Bson::ObjectId(object_id))
                })
                .filter_map(Result::ok)
                .collect();
            res.insert_bson(
                "_id".to_string(),
                Bson::Document(doc! {
                    "$in" => ids
                }),
            );
        }

        if let Some(title) = self.title.clone() {
            GetCourses2::insert_str_match(
                &mut res,
                "course.header.title".to_string(),
                title,
                self.title_exact,
                self.title_case_sensitive,
                self.title_trimmed,
            );
        }

        if let Some(owner) = &self.owner {
            GetCourses2::insert_objectid(&mut res, "owner".to_string(), owner)?;
        }

        if let Some(uploader) = &self.uploader {
            let filter = doc! {
                "username" => Bson::RegExp(format!("^{}$", uploader), "i".to_string())
            };
            match Data::find_account(database, filter) {
                Some(account) => {
                    res.insert_bson(
                        "owner".to_string(),
                        Bson::ObjectId(account.get_id().clone()),
                    );
                }
                None => return Err(GetCourses2Error::UploaderUnknown(uploader.clone())),
            };
        }

        if let Some(difficulty) = &self.difficulty {
            res.insert("difficulty", difficulty.clone());
        }

        if res.is_empty() {
            Ok(None)
        } else {
            Ok(Some(res))
        }
    }

    fn get_sort_doc(&self) -> OrderedDocument {
        let mut query = OrderedDocument::new();
        for sort in self.get_sort() {
            let sort_dir: String = sort.val.try_into().unwrap_or_default();
            query.insert(sort_dir, sort.dir);
        }
        doc! {
            "$sort" => query
        }
    }

    fn get_sort(&self) -> Vec<Sort> {
        let mut res = if self.sort.is_some() {
            self.sort.clone().unwrap()
        } else {
            vec![Sort::default()]
        };
        if res
            .iter()
            .find(|sort| sort.val == SortValue::CourseHeaderTitle)
            .is_none()
        {
            res.push(Sort {
                val: SortValue::CourseHeaderTitle,
                dir: -1,
            })
        }
        res
    }

    fn get_limit(&self) -> u32 {
        self.limit.0 + self.skip.unwrap_or_default()
    }

    fn insert_str_match(
        doc: &mut OrderedDocument,
        key: String,
        val: String,
        exact: bool,
        case_sensitive: bool,
        trimmed: bool,
    ) {
        let matched_str = if exact {
            if trimmed {
                format!("^ *{} *$", regex::escape(&val))
            } else {
                format!("^{}$", regex::escape(&val))
            }
        } else {
            format!(".*{}.*", regex::escape(&val))
        };
        let options_str = if case_sensitive {
            "".to_string()
        } else {
            "i".to_string()
        };
        doc.insert_bson(key, Bson::RegExp(matched_str, options_str));
    }

    fn insert_objectid(
        doc: &mut OrderedDocument,
        key: String,
        oid: &str,
    ) -> Result<(), GetCourses2Error> {
        doc.insert_bson(
            key.clone(),
            Bson::ObjectId(
                ObjectId::with_string(oid).map_err(|_| GetCourses2Error::DeserializeError(key))?,
            ),
        );
        Ok(())
    }
}

#[derive(Apiv2Schema, Debug, Deserialize)]
struct Limit(#[serde(deserialize_with = "deserialize_limit")] u32);

impl Default for Limit {
    fn default() -> Limit {
        Limit(120)
    }
}

fn deserialize_limit<'de, D>(de: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let val = u32::deserialize(de)?;
    if val == 0 {
        Err(de::Error::invalid_value(
            de::Unexpected::Unsigned(val.into()),
            &"limit must be at least 1",
        ))
    } else if val > 120 {
        Err(de::Error::invalid_value(
            de::Unexpected::Unsigned(val.into()),
            &"limit must be at most 120",
        ))
    } else {
        Ok(val)
    }
}

#[derive(Clone, Deserialize, Debug)]
struct Sort {
    pub val: SortValue,
    #[serde(deserialize_with = "deserialize_dir")]
    dir: i32,
}

impl Default for Sort {
    fn default() -> Self {
        Sort {
            val: SortValue::LastModified,
            dir: -1,
        }
    }
}

#[derive(Clone, Deserialize, Debug, PartialEq, Serialize)]
enum SortValue {
    #[serde(rename = "last_modified")]
    LastModified,
    #[serde(rename = "uploaded")]
    Uploaded,
    #[serde(rename = "course.header.title")]
    CourseHeaderTitle,
    #[serde(rename = "votes")]
    Votes,
}

impl TryFrom<SortValue> for String {
    type Error = Error;

    fn try_from(value: SortValue) -> Result<Self, Self::Error> {
        serde_json::to_value(&value)?
            .as_str()
            .map(String::from)
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "serde_json as_str failed").into())
    }
}

impl Default for SortValue {
    fn default() -> Self {
        SortValue::LastModified
    }
}

fn deserialize_dir<'de, D>(de: D) -> Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    let val = i32::deserialize(de)?;
    if val != -1 && val != 1 {
        Err(de::Error::invalid_value(
            de::Unexpected::Signed(val.into()),
            &"sort direction must either be -1 or 1",
        ))
    } else {
        Ok(val)
    }
}

#[api_v2_errors(
    code = 400,
    description = "Deserialization failed or bad JSON",
    code = 404,
    code = 500
)]
#[derive(Debug, Fail)]
pub enum GetCourses2Error {
    #[fail(display = "could not deserialize {} from hex string", _0)]
    DeserializeError(String),
    #[fail(display = "uploader with name {} unknown", _0)]
    UploaderUnknown(String),
    #[fail(display = "[PutCourses2Error::SerdeJson]: {}", _0)]
    SerdeJson(serde_json::Error),
    #[fail(display = "[GetCourses2Error::Mongo]: {}", _0)]
    Mongo(mongodb::Error),
}

impl From<serde_json::Error> for GetCourses2Error {
    fn from(err: serde_json::Error) -> Self {
        GetCourses2Error::SerdeJson(err)
    }
}

impl From<mongodb::Error> for GetCourses2Error {
    fn from(err: mongodb::Error) -> Self {
        GetCourses2Error::Mongo(err)
    }
}

impl ResponseError for GetCourses2Error {
    fn error_response(&self) -> HttpResponse {
        match *self {
            GetCourses2Error::DeserializeError(_) => HttpResponse::new(StatusCode::BAD_REQUEST),
            GetCourses2Error::UploaderUnknown(_) => HttpResponse::new(StatusCode::NOT_FOUND),
            GetCourses2Error::SerdeJson(_) => HttpResponse::new(StatusCode::BAD_REQUEST),
            GetCourses2Error::Mongo(_) => HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}
