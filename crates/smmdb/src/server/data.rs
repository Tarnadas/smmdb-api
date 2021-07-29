use crate::{
    config::GOOGLE_CLIENT_ID,
    routes::{
        courses,
        courses2::{
            self,
            download::DownloadCourse2Error,
            meta::PostCourse2MetaError,
            thumbnail::{GetCourse2ThumbnailError, GetThumbnail2, Size2},
            PutCourses2Response,
        },
    },
    session::AuthReq,
};

use bson::{oid::ObjectId, spec::BinarySubtype, Binary, Bson, Document};
use futures::StreamExt;
use image::{
    error::{ImageError, ImageFormatHint, UnsupportedError, UnsupportedErrorKind},
    imageops::FilterType,
    jpeg::JpegEncoder,
    load_from_memory, DynamicImage,
};
use mongodb::error::ErrorKind;
use rayon::prelude::*;
use smmdb_auth::{Account, AccountReq, AuthSession};
use smmdb_common::{
    Course, Course2, Course2Response, Course2SimilarityError, CourseResponse, Difficulty, LshIndex,
    MinHash, PermGen, Vote,
};
use smmdb_db::{Database, DatabaseError};
use std::{
    convert::TryInto,
    io,
    sync::{Arc, Mutex},
    time::SystemTime,
};

const SIMILARITY_THRESHOLD: f64 = 0.95;

pub struct Data {
    database: Arc<Database>,
    pub google_client_id: &'static str,
    pub perm_gen: PermGen,
    pub lsh_index: Arc<Mutex<LshIndex>>,
}

pub type ServerData = Arc<Data>;

impl Data {
    pub async fn new(database: Arc<Database>, perm_gen: PermGen) -> Self {
        let mut lsh_index = LshIndex::new(8);
        println!("Filling LshIndex");
        Data::fill_lsh_index(&database, &mut lsh_index).await;
        println!("Filling LshIndex completed!");
        Data {
            database,
            google_client_id: GOOGLE_CLIENT_ID,
            perm_gen,
            lsh_index: Arc::new(Mutex::new(lsh_index)),
        }
    }

    pub async fn fill_lsh_index(database: &Database, lsh_index: &mut LshIndex) {
        if let Ok(mut cursor) = database.fill_lsh_index().await {
            while let Some(item) = cursor.next().await {
                let item = if let Ok(item) = item { item } else { continue };
                if let (Some(id), Some(hash)) = (item.get("_id"), item.get("hash")) {
                    let hash: serde_json::Value = hash.clone().into();
                    let hash: Result<MinHash, _> = serde_json::from_value(hash);
                    if let (Bson::ObjectId(id), Ok(hash)) = (id.clone(), hash) {
                        lsh_index.insert(id.to_hex(), &hash);
                    }
                }
            }
        }
    }

    pub async fn get_courses(
        &self,
        query: courses::GetCourses,
    ) -> Result<String, courses::GetCoursesError> {
        match query.into_ordered_document(&self.database).await {
            Ok(query) => Ok(match self.database.get_courses(query).await {
                Ok(mut cursor) => {
                    let mut account_ids: Vec<Bson> = vec![];
                    let mut courses: Vec<Course> = vec![];
                    while let Some(item) = cursor.next().await {
                        let course: Course = if let Ok(item) = item {
                            item.into()
                        } else {
                            continue;
                        };
                        account_ids.push(course.get_owner().clone().into());
                        courses.push(course);
                    }

                    let accounts = self.get_accounts(account_ids).await;
                    let courses: Vec<CourseResponse> = courses
                        .into_iter()
                        .map(|course| {
                            let account = accounts
                                .iter()
                                .find(|account| {
                                    account.get_id().to_string() == course.get_owner().to_string()
                                })
                                .unwrap();
                            CourseResponse::from_course(course, account)
                        })
                        .collect();

                    serde_json::to_string(&courses).unwrap()
                }
                Err(e) => e.to_string(),
            }),
            Err(error) => Err(error),
        }
    }

    pub async fn get_courses2(
        &self,
        query: courses2::GetCourses2,
        own_account: Option<Account>,
    ) -> Result<Vec<Course2Response>, courses2::GetCourses2Error> {
        let query = query.into_ordered_document(&self.database).await?;
        let mut cursor = self.database.get_courses2(query).await?;

        let mut courses: Vec<Course2Response> = vec![];
        while let Some(Ok(item)) = cursor.next().await {
            let course: Course2 = item.try_into()?;
            let account_id: Bson = course.get_owner().clone().into();
            if let Some(account) = &self.get_account(account_id).await {
                let course = Course2Response::from_course(
                    course,
                    account,
                    own_account.as_ref(),
                    &*self.database,
                )
                .await;
                courses.push(course);
            }
        }

        Ok(courses)
    }

    pub async fn get_course2(
        &self,
        course_id: ObjectId,
    ) -> Result<(Vec<u8>, Vec<u8>), DownloadCourse2Error> {
        let doc = doc! {
            "_id": course_id.clone()
        };
        let thumb: String = Size2::Encrypted.into();
        let projection = doc! {
            thumb.clone(): 1,
            "data_encrypted": 1
        };
        let course = self.database.get_course2(doc, projection).await?;
        if let Some(course) = course {
            let data = course.get_binary_generic(&"data_encrypted")?;
            let thumb = course.get_binary_generic(&thumb)?;
            Ok((data.clone(), thumb.clone()))
        } else {
            Err(DownloadCourse2Error::CourseNotFound(course_id))
        }
    }

    pub async fn get_course2_br(
        &self,
        course_id: ObjectId,
    ) -> Result<(Vec<u8>, Vec<u8>), DownloadCourse2Error> {
        let doc = doc! {
            "_id": course_id.clone()
        };
        let thumb: String = Size2::Encrypted.into();
        let projection = doc! {
            thumb.clone(): 1,
            "data_br": 1,
            "data_encrypted": 1,
        };
        let course = self.database.get_course2(doc, projection).await?;
        if let Some(course) = course {
            let thumb = course.get_binary_generic(&thumb)?;
            if let Ok(data) = course.get_binary_generic(&"data_br") {
                Ok((data.clone(), thumb.clone()))
            } else {
                Ok((
                    self.database
                        .add_course2_data_br(course_id, course.clone())
                        .await?,
                    thumb.clone(),
                ))
            }
        } else {
            Err(DownloadCourse2Error::CourseNotFound(course_id))
        }
    }

    pub async fn get_course2_proto(
        &self,
        course_id: ObjectId,
    ) -> Result<(Vec<u8>, Vec<u8>), DownloadCourse2Error> {
        let doc = doc! {
            "_id": course_id.clone()
        };
        let thumb: String = Size2::Encrypted.into();
        let projection = doc! {
            thumb.clone(): 1,
            "data_protobuf_br": 1
        };
        let course = self.database.get_course2(doc, projection).await?;
        if let Some(course) = course {
            let thumb = course.get_binary_generic(&thumb)?;
            if let Ok(data) = course.get_binary_generic(&"data_protobuf_br") {
                Ok((data.clone(), thumb.clone()))
            } else {
                Ok((
                    self.database
                        .add_course2_data_protobuf_br(course_id, course.clone())
                        .await?,
                    thumb.clone(),
                ))
            }
        } else {
            Err(DownloadCourse2Error::CourseNotFound(course_id))
        }
    }

    pub async fn get_course2_thumbnail(
        &self,
        course_id: ObjectId,
        query: GetThumbnail2,
    ) -> Result<Vec<u8>, GetCourse2ThumbnailError> {
        let doc = doc! {
            "_id": course_id.clone()
        };
        let size: String = query.size.clone().into();
        let projection = doc! {
            size.clone(): 1
        };
        let thumb = self.database.get_course2(doc, projection).await?;
        if let Some(thumb) = thumb {
            match thumb.get_binary_generic(&size) {
                Ok(thumb) => Ok(thumb.clone()),
                Err(_) => {
                    if query.size == Size2::Original {
                        Err(GetCourse2ThumbnailError::CourseNotFound(course_id))
                    } else {
                        let doc = doc! {
                            "_id": course_id.clone()
                        };
                        let size_original: String = Size2::Original.into();
                        let projection = doc! {
                            size_original.clone(): 1
                        };
                        let thumb = self.database.get_course2(doc, projection).await?.unwrap();
                        let thumb = thumb
                            .get_binary_generic(&size_original)
                            .unwrap_or_else(|_| {
                                panic!(
                                    "mongodb corrupted. thumbnail missing for course {}",
                                    course_id
                                )
                            })
                            .clone();

                        let image = load_from_memory(&thumb[..])?;
                        let (nwidth, nheight) = query.size.get_dimensions();
                        let image = image.resize_exact(nwidth, nheight, FilterType::Gaussian);
                        let color = image.color();

                        match image {
                            DynamicImage::ImageRgb8(buffer) => {
                                let (width, height) = buffer.dimensions();
                                let mut res = vec![];
                                let mut encoder = JpegEncoder::new_with_quality(&mut res, 85);
                                encoder
                                    .encode(&buffer.into_raw()[..], width, height, color)
                                    .map_err(ImageError::from)?;
                                self.database
                                    .update_course2_thumbnail(course_id, size, res.clone())
                                    .await?;
                                Ok(res)
                            }
                            _ => Err(ImageError::Unsupported(
                                UnsupportedError::from_format_and_kind(
                                    ImageFormatHint::Unknown,
                                    UnsupportedErrorKind::GenericFeature(
                                        "expected image rgb8".to_string(),
                                    ),
                                ),
                            )
                            .into()),
                        }
                    }
                }
            }
        } else {
            Err(GetCourse2ThumbnailError::CourseNotFound(course_id))
        }
    }

    pub async fn put_courses2(
        &self,
        mut courses: Vec<smmdb_lib::Course2>,
        account: &Account,
        difficulty: Option<Difficulty>,
    ) -> Result<PutCourses2Response, courses2::PutCourses2Error> {
        let lsh_index = self.lsh_index.clone();
        let response = Arc::new(Mutex::new(PutCourses2Response::new()));
        let succeeded: Vec<_> = courses
            .par_iter_mut()
            .map(
                |smm_course| -> Result<Course2Response, courses2::PutCourses2Error> {
                    let mut course = Course2::insert(
                        account.get_id().clone(),
                        smm_course,
                        difficulty.clone(),
                        &self.perm_gen,
                    );

                    let course_thumb = smm_course
                        .get_course_thumb_mut()
                        .ok_or(courses2::PutCourses2Error::ThumbnailMissing)?;
                    let mut thumb_data = course_thumb.get_jpeg().to_vec();
                    smmdb_lib::Thumbnail2::encrypt(&mut thumb_data);
                    let thumb_encrypted = Bson::Binary(Binary {
                        subtype: BinarySubtype::Generic,
                        bytes: thumb_data,
                    });
                    let thumb = Bson::Binary(Binary {
                        subtype: BinarySubtype::Generic,
                        bytes: course_thumb.get_jpeg().to_vec(),
                    });

                    if let Ok(doc_meta) = bson::ser::to_document(&course) {
                        let mut lsh_index = lsh_index.lock().unwrap();
                        let query: Vec<Bson> = lsh_index
                            .query(course.get_hash())
                            .into_iter()
                            .map(|id| -> Bson { ObjectId::with_string(&id).unwrap().into() })
                            .collect();
                        let query = doc! {
                            "_id": {
                                "$in": query
                            }
                        };
                        let similar_courses = self.find_courses2(query)?;
                        for similar_course in similar_courses {
                            let jaccard = course.get_hash().jaccard(similar_course.get_hash());
                            if jaccard > SIMILARITY_THRESHOLD {
                                return Err(courses2::PutCourses2Error::Similarity(
                                    Course2SimilarityError::new(
                                        similar_course.get_id().to_hex(),
                                        similar_course
                                            .get_course()
                                            .get_header()
                                            .get_title()
                                            .to_string(),
                                        jaccard,
                                    ),
                                ));
                            }
                        }

                        let inserted_id = self
                            .database
                            .put_course2(doc_meta, smm_course, thumb, thumb_encrypted)
                            .await?;
                        course.set_id(inserted_id);
                        lsh_index.insert(course.get_id().to_hex(), course.get_hash());
                        let course =
                            Course2Response::from_course(course, account, None, &*self.database)
                                .await;
                        Ok(course)
                    } else {
                        Err(io::Error::new(io::ErrorKind::Other, "".to_string()).into())
                    }
                },
            )
            .filter_map(|course| {
                if let Err(err) = course {
                    response.lock().unwrap().add_failed(err);
                    None
                } else {
                    Result::ok(course)
                }
            })
            .collect();
        let mut response = Arc::try_unwrap(response).unwrap().into_inner().unwrap();
        response.set_succeeded(succeeded);
        Ok(response)
    }

    pub async fn delete_course2(
        &self,
        course_id: String,
        course_oid: ObjectId,
    ) -> Result<(), DatabaseError> {
        let query = doc! {
            "_id": course_oid
        };
        self.database.delete_course2(course_id, query).await
    }

    pub async fn vote_course2(
        &self,
        account_id: ObjectId,
        course_id: ObjectId,
        value: i32,
    ) -> anyhow::Result<()> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let filter = doc! {
            "account_id": account_id.clone(),
            "course_id": course_id.clone(),
        };
        match value {
            0 => self.database.unvote_course2(filter).await?,
            _ => {
                let update = doc! {
                    "$set": {
                        "account_id": account_id,
                        "course_id": course_id.clone(),
                        "value": value,
                        "timestamp": now,
                    }
                };
                self.database.vote_course2(filter, update).await?
            }
        }
        let filter = doc! {
            "course_id": course_id.clone(),
        };
        let projection = doc! {
            "course_id": course_id.clone(),
            "value": 1,
            "timestamp":1,
        };
        let votes: Vec<_> = self
            .database
            .get_votes_course2(filter, projection)
            .await?
            .collect()
            .await;
        let votes: Result<Vec<Vote>, _> = votes
            .into_iter()
            .map(|item| {
                item.map(|item| -> Result<Vote, serde_json::Error> { item.try_into() })
                    .map_err(|err| anyhow!("get_votes_course2 failed: {}", err))?
                    .map_err(|err| anyhow!("get_votes_course2 failed: {}", err))
            })
            .collect();
        let vote_value: i32 = votes?.iter().fold(0, |acc, vote| acc + vote.get_value());
        let filter = doc! {
            "_id": course_id,
        };
        let update = doc! {
            "$set": {
                "votes": vote_value,
            }
        };
        self.database.update_course2(filter, update).await?;
        Ok(())
    }

    pub async fn post_course2_meta(
        &self,
        course_id: ObjectId,
        difficulty: Option<Difficulty>,
    ) -> Result<(), PostCourse2MetaError> {
        let filter = doc! {
            "_id": course_id.clone()
        };
        let mut set = doc! {};
        let mut unset = doc! {};
        if let Some(difficulty) = difficulty {
            set.insert("difficulty", format!("{:?}", difficulty).to_lowercase());
        } else {
            unset.insert("difficulty", "");
        }
        let mut update = doc! {};
        if !set.is_empty() {
            update.insert("$set", set);
        }
        if !unset.is_empty() {
            update.insert("$unset", unset);
        }
        let update = self.database.update_courses2(filter, update).await?;
        if update.matched_count == 0 {
            Err(anyhow!("{}", course_id.to_string()).into())
        } else {
            Ok(())
        }
    }

    pub async fn add_or_get_account(
        &self,
        account: AccountReq,
        session: AuthSession,
    ) -> Result<Account, anyhow::Error> {
        match Data::find_account(&self.database, account.as_find()).await {
            Some(account) => {
                let filter = doc! {
                    "_id": account.get_id().clone()
                };
                let session: Document = session.into();
                let update = doc! {
                    "$set": {
                        "session": session
                    }
                };
                self.database.update_account(filter, update).await?;
                Ok(account)
            }
            None => {
                let res = self
                    .database
                    .insert_account(account.clone().into_ordered_document())
                    .await?;
                let account = Account::new(
                    account,
                    res.inserted_id.as_object_id().unwrap().clone(),
                    session,
                );
                let filter = doc! {
                    "_id": account.get_id()
                };
                let update = doc! {
                    "$set": {
                        "apikey": account.get_apikey()
                    }
                };
                self.database.update_account(filter, update).await?;
                Ok(account)
            }
        }
    }

    pub fn delete_account_session(&self, account: Account) -> Result<(), mongodb::error::Error> {
        self.database.delete_account_session(account.get_id())
    }

    pub fn get_account_from_auth(&self, auth_req: AuthReq) -> Option<Account> {
        Data::find_account(&self.database, auth_req.into())
    }

    pub fn does_account_own_course(&self, account_id: ObjectId, course_oid: ObjectId) -> bool {
        let query = doc! {
            "_id": course_oid,
            "owner": account_id
        };
        if let Ok(courses) = self.find_courses2(query) {
            courses.len() == 1
        } else {
            false
        }
    }

    fn find_courses2(&self, doc: OrderedDocument) -> Result<Vec<Course2>, mongodb::error::Error> {
        match self.database.find_courses2(doc) {
            Ok(cursor) => {
                let courses: Vec<Course2> = cursor
                    .map(|item| -> Result<Course2, serde_json::Error> {
                        let course: Course2 = item.unwrap().try_into()?;
                        Ok(course)
                    })
                    .filter_map(Result::ok)
                    .collect();
                Ok(courses)
            }
            Err(err) => Err(err),
        }
    }

    async fn get_accounts(&self, account_ids: Vec<Bson>) -> Vec<Account> {
        self.database
            .get_accounts(account_ids)
            .await
            .unwrap()
            .map(|item| item.unwrap().into())
            .collect()
    }

    async fn get_account(&self, account_id: Bson) -> Option<Account> {
        self.find_account(doc! {
            "_id": account_id
        })
        .unwrap()
        .map(|item| item.into())
    }

    pub async fn find_account(database: &Database, filter: Document) -> Option<Account> {
        database
            .find_account(filter)
            .await
            .unwrap()
            .map(|item| item.into())
    }
}
