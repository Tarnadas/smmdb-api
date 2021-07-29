#[macro_use]
extern crate bson;

mod collections;

use collections::Collections;

use brotli2::{read::BrotliEncoder, CompressParams};
use bson::{oid::ObjectId, spec::BinarySubtype, Binary, Bson, Document};
use mongodb::{
    options::{FindOneOptions, FindOptions, UpdateOptions},
    results::{InsertOneResult, UpdateResult},
    Client, Collection, Cursor,
};
use std::env;

mod error;

pub use error::*;

pub struct Database {
    courses: Collection,
    _course_data: Collection,
    // TODO non pub
    pub courses2: Collection,
    pub course2_data: Collection,
    pub accounts: Collection,
    votes: Collection,
    meta: Collection,
}

impl Database {
    pub async fn new() -> Self {
        let mongodb_uri = env::var("MONGODB_URI").unwrap();
        println!("Connecting to MongoDB at {}", mongodb_uri);
        let client = Client::with_uri_str(&mongodb_uri)
            .await
            .expect("Failed to connect to MongoDB");
        let courses = client
            .database("admin")
            .collection(Collections::Courses.as_str());
        let _course_data = client
            .database("admin")
            .collection(Collections::CourseData.as_str());
        let courses2 = client
            .database("admin")
            .collection(Collections::Courses2.as_str());
        let course2_data = client
            .database("admin")
            .collection(Collections::Course2Data.as_str());
        let accounts = client
            .database("admin")
            .collection(Collections::Accounts.as_str());
        let votes = client
            .database("admin")
            .collection(Collections::Votes.as_str());
        let migrations = client
            .database("admin")
            .collection(Collections::Meta.as_str());

        if let Err(err) = Database::generate_accounts_indexes(&client.database("admin")).await {
            println!("{}", err);
        }
        if let Err(err) = Database::generate_course2_indexes(&courses2) {
            println!("{}", err);
        }
        if let Err(err) = Database::generate_votes_indexes(&votes) {
            println!("{}", err);
        }

        Database {
            courses,
            _course_data,
            courses2,
            course2_data,
            accounts,
            votes,
            meta: migrations,
        }
    }

    async fn generate_accounts_indexes(
        database: &mongodb::Database,
    ) -> Result<(), mongodb::error::Error> {
        let indexes = vec![doc! {
            "apikey": 1,
        }];
        let listed_indexes: Document = database
            .run_command(
                doc! {
                    "listIndexes": "accounts"
                },
                None,
            )
            .await?;
        dbg!(&listed_indexes);
        // for index in indexes {
        //     if !listed_indexes.iter().any(|idx| idx == &index) {
        //         database
        //             .run_command(
        //                 doc! {
        //                     "createIndexed": "accounts",
        //                     "indexes": [
        //                         index
        //                     ]
        //                 },
        //                 None,
        //             )
        //             .await?;
        //     }
        // }
        Ok(())
    }

    fn generate_course2_indexes(courses2: &Collection) -> Result<(), mongodb::error::Error> {
        let indexes = vec![
            doc! {
                "last_modified": -1,
                "course.header.title": -1
            },
            doc! {
                "last_modified": -1,
                "course.header.title": 1
            },
            doc! {
                "last_modified": 1,
                "course.header.title": -1
            },
            doc! {
                "last_modified": 1,
                "course.header.title": 1
            },
            doc! {
                "votes": 1,
                "course.header.title": -1
            },
            doc! {
                "votes": -1,
                "course.header.title": -1
            },
            doc! {
                "votes": 1,
                "last_modified": -1,
                "course.header.title": -1
            },
            doc! {
                "votes": -1,
                "last_modified": -1,
                "course.header.title": -1
            },
        ];
        // let listed_indexes: Vec<Document> =
        //     courses2.list_indexes()?.filter_map(Result::ok).collect();
        // for index in indexes {
        //     if !listed_indexes.iter().any(|idx| idx == &index) {
        //         courses2.create_index(index, None)?;
        //     }
        // }
        Ok(())
    }

    fn generate_votes_indexes(votes: &Collection) -> Result<(), mongodb::error::Error> {
        let indexes = vec![doc! {
            "account_id": 1,
            "course_id": 1,
        }];
        // let listed_indexes: Vec<Document> = votes.list_indexes()?.filter_map(Result::ok).collect();
        // for index in indexes {
        //     if !listed_indexes.iter().any(|idx| idx == &index) {
        //         votes.create_index(index, None)?;
        //     }
        // }
        Ok(())
    }

    pub async fn get_courses(&self, query: Vec<Document>) -> Result<Cursor, mongodb::error::Error> {
        self.courses.aggregate(query, None).await
    }

    pub async fn get_courses2(
        &self,
        query: Vec<Document>,
    ) -> Result<Cursor, mongodb::error::Error> {
        self.courses2.aggregate(query, None).await
    }

    pub async fn get_missing_migrations(
        &self,
        migrations_to_run: Vec<String>,
    ) -> Result<Vec<String>, mongodb::error::Error> {
        let doc = self
            .meta
            .find_one(
                Some(doc! {
                    "migrations": {
                        "$exists": true
                    }
                }),
                None,
            )
            .await?
            .unwrap_or(doc! {
                "migrations": []
            });

        let migrations: Vec<_> = if let Bson::Array(array) = doc.get("migrations").unwrap() {
            array.iter().map(|bson| bson.to_string()).collect()
        } else {
            vec![]
        };

        Ok(migrations_to_run
            .into_iter()
            .filter(|m| !migrations.contains(&format!("\"{}\"", m)))
            .collect())
    }

    pub async fn migration_completed(
        &self,
        migration: String,
    ) -> Result<(), mongodb::error::Error> {
        let filter = doc! {
            "migrations": {
                "$exists": true
            }
        };
        let update = doc! {
            "$push": {
                "migrations": migration
            }
        };
        self.meta
            .update_one(
                filter,
                update,
                Some(UpdateOptions::builder().upsert(Some(true)).build()),
            )
            .await?;
        Ok(())
    }

    pub async fn fill_lsh_index(&self) -> Result<Cursor, mongodb::error::Error> {
        self.courses2
            .find(
                None,
                Some(
                    FindOptions::builder()
                        .projection(Some(doc! {
                            "hash": 1
                        }))
                        .build(),
                ),
            )
            .await
    }

    pub async fn put_course2(
        &self,
        doc_meta: Document,
        course: &mut smmdb_lib::Course2,
        thumb: Bson,
        thumb_encrypted: Bson,
    ) -> Result<ObjectId, mongodb::error::Error> {
        let insert_res = self.courses2.insert_one(doc_meta, None).await?;
        let inserted_id = insert_res.inserted_id;
        let inserted_id = inserted_id.as_object_id().unwrap();
        course.set_smmdb_id(inserted_id.to_string()).unwrap();
        let mut course_data = course.get_course_data().to_vec();
        smmdb_lib::Course2::encrypt(&mut course_data);
        let doc = doc! {
            "_id": inserted_id.clone(),
            "data_encrypted": Bson::Binary(Binary { subtype: BinarySubtype::Generic, bytes: course_data }),
            "thumb": thumb,
            "thumb_encrypted": thumb_encrypted,
        };
        self.course2_data.insert_one(doc, None).await?;
        Ok(inserted_id.clone())
    }

    pub async fn get_course2(
        &self,
        doc: Document,
        projection: Document,
    ) -> Result<Option<Document>, mongodb::error::Error> {
        self.course2_data
            .find_one(
                Some(doc),
                Some(
                    FindOneOptions::builder()
                        .projection(Some(projection))
                        .build(),
                ),
            )
            .await
    }

    pub async fn update_course2_thumbnail(
        &self,
        course_id: ObjectId,
        size: String,
        data: Vec<u8>,
    ) -> Result<(), mongodb::error::Error> {
        let data = Bson::Binary(Binary {
            subtype: BinarySubtype::Generic,
            bytes: data,
        });
        let filter = doc! {
            "_id": course_id
        };
        let update = doc! {
            "$set": {
                size: data
            }
        };
        self.course2_data.update_one(filter, update, None).await?;
        Ok(())
    }

    pub async fn update_course2(
        &self,
        filter: Document,
        update: Document,
    ) -> Result<(), mongodb::error::Error> {
        self.courses2.update_one(filter, update, None).await?;
        Ok(())
    }

    pub async fn delete_course2(
        &self,
        course_id: String,
        doc: Document,
    ) -> Result<(), DatabaseError> {
        self.courses2.delete_one(doc.clone(), None).await?;
        let res = self.course2_data.delete_one(doc, None).await?;
        if res.deleted_count == 0 {
            Err(DatabaseError::DeleteFailed)
        } else {
            Ok(())
        }
    }

    pub async fn vote_course2(
        &self,
        filter: Document,
        update: Document,
    ) -> Result<(), mongodb::error::Error> {
        self.votes
            .update_one(
                filter,
                update,
                Some(UpdateOptions::builder().upsert(Some(true)).build()),
            )
            .await?;
        Ok(())
    }

    pub async fn unvote_course2(&self, filter: Document) -> Result<(), mongodb::error::Error> {
        self.votes.delete_one(filter, None).await?;
        Ok(())
    }

    pub async fn get_votes_course2(
        &self,
        filter: Document,
        projection: Document,
    ) -> Result<Cursor, mongodb::error::Error> {
        self.votes
            .find(
                Some(filter),
                Some(FindOptions::builder().projection(Some(projection)).build()),
            )
            .await
    }

    pub async fn get_vote_for_account(
        &self,
        account_id: &ObjectId,
        course_id: &ObjectId,
    ) -> Result<i32, mongodb::error::Error> {
        let filter = doc! {
            "account_id": account_id,
            "course_id": course_id
        };
        let projection = doc! {
            "value": true
        };
        let res = self
            .votes
            .find_one(
                Some(filter),
                Some(
                    FindOneOptions::builder()
                        .projection(Some(projection))
                        .build(),
                ),
            )
            .await?
            .map(|doc| doc.get_i32("value").unwrap_or_default())
            .unwrap_or_default();
        Ok(res)
    }

    pub async fn find_courses2(&self, doc: Document) -> Result<Cursor, mongodb::error::Error> {
        self.courses2.find(Some(doc), None).await
    }

    pub async fn update_courses2(
        &self,
        filter: Document,
        update: Document,
    ) -> Result<UpdateResult, mongodb::error::Error> {
        self.courses2.update_one(filter, update, None).await
    }

    pub async fn find_account(
        &self,
        filter: Document,
    ) -> Result<Option<Document>, mongodb::error::Error> {
        self.accounts.find_one(Some(filter), None).await
    }

    pub async fn get_accounts(
        &self,
        account_ids: Vec<Bson>,
    ) -> Result<Cursor, mongodb::error::Error> {
        self.accounts
            .find(
                Some(doc! {
                    "_id": {
                        "$in": account_ids
                    }
                }),
                None,
            )
            .await
    }

    pub async fn insert_account(
        &self,
        account: Document,
    ) -> Result<InsertOneResult, mongodb::error::Error> {
        self.accounts.insert_one(account, None).await
    }

    pub async fn update_account(
        &self,
        filter: Document,
        update: Document,
    ) -> Result<UpdateResult, mongodb::error::Error> {
        self.accounts.update_one(filter, update, None).await
    }

    pub async fn delete_account_session(
        &self,
        account_id: &ObjectId,
    ) -> Result<(), mongodb::error::Error> {
        let filter = doc! {
            "_id": account_id.clone()
        };
        let update = doc! {
            "$unset": {
                "session": ""
            }
        };
        self.accounts.update_one(filter, update, None).await?;
        Ok(())
    }

    pub async fn add_course2_data_br(
        &self,
        course_id: ObjectId,
        course: Document,
    ) -> Result<Vec<u8>, DatabaseError> {
        use std::io::prelude::*;

        let bson_course = course.get("data_encrypted").unwrap();
        if let Bson::Binary(Binary {
            bytes: mut course_data,
            ..
        }) = bson_course.clone()
        {
            smmdb_lib::Course2::decrypt(&mut course_data)?;

            let mut data_br = vec![];
            let mut params = CompressParams::new();
            params.quality(11);
            BrotliEncoder::from_params(&course_data[..], &params).read_to_end(&mut data_br)?;

            let filter = doc! {
                "_id": course_id,
            };
            let update = doc! {
                "$set": {
                    "data_br": Bson::Binary(Binary { subtype: BinarySubtype::Generic, bytes: data_br.clone() }),
                }
            };
            self.course2_data
                .update_one(filter, update, None)
                .await
                .unwrap();
            Ok(data_br)
        } else {
            todo!()
        }
    }

    pub async fn add_course2_data_protobuf_br(
        &self,
        course_id: ObjectId,
        course: Document,
    ) -> Result<Vec<u8>, DatabaseError> {
        use std::io::prelude::*;

        let bson_course = course.get("data_encrypted").unwrap();
        if let Bson::Binary(Binary {
            bytes: mut course_data,
            ..
        }) = bson_course.clone()
        {
            let course = smmdb_lib::Course2::from_switch_files(&mut course_data, None, true)?;
            let course_data = course.get_proto();

            let mut data_br = vec![];
            let mut params = CompressParams::new();
            params.quality(11);
            BrotliEncoder::from_params(&course_data[..], &params).read_to_end(&mut data_br)?;

            let filter = doc! {
                "_id": course_id,
            };
            let update = doc! {
                "$set": {
                    "data_protobuf_br": Bson::Binary(Binary { subtype: BinarySubtype::Generic, bytes: data_br.clone() }),
                }
            };
            self.course2_data
                .update_one(filter, update, None)
                .await
                .unwrap();
            Ok(data_br)
        } else {
            todo!()
        }
    }
}
