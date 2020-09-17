mod collections;

use collections::Collections;

use brotli2::{read::BrotliEncoder, CompressParams};
use bson::{oid::ObjectId, ordered::OrderedDocument, spec::BinarySubtype, Bson};
use mongodb::{
    coll::{
        options::{FindOptions, UpdateOptions},
        results::{InsertOneResult, UpdateResult},
        Collection,
    },
    cursor::Cursor,
    db::ThreadedDatabase,
    Client, ThreadedClient,
};
use std::env;

mod error;

pub use error::*;

pub struct Database {
    courses: Collection,
    course_data: Collection,
    // TODO non pub
    pub courses2: Collection,
    pub course2_data: Collection,
    pub accounts: Collection,
    votes: Collection,
}

impl Database {
    pub fn new() -> Self {
        let host = match env::var("DOCKER") {
            Ok(val) => match val.as_ref() {
                "true" | "1" => "mongodb".to_string(), // get_gateway_ip(),
                _ => "localhost".to_string(),
            },
            Err(_) => "localhost".to_string(),
        };
        println!("Connecting to MongoDB at mongodb://{}:27017", host);
        let client = Client::with_uri(&format!("mongodb://{}:27017", host))
            .expect("Failed to connect to MongoDB");
        let courses = client.db("admin").collection(Collections::Courses.as_str());
        let course_data = client
            .db("admin")
            .collection(Collections::CourseData.as_str());
        let courses2 = client
            .db("admin")
            .collection(Collections::Courses2.as_str());
        let course2_data = client
            .db("admin")
            .collection(Collections::Course2Data.as_str());
        let accounts = client
            .db("admin")
            .collection(Collections::Accounts.as_str());
        let votes = client.db("admin").collection(Collections::Votes.as_str());

        if let Err(err) = Database::generate_course2_indexes(&courses2) {
            println!("{}", err);
        }

        if let Err(err) = Database::generate_votes_indexes(&votes) {
            println!("{}", err);
        }

        Database {
            courses,
            course_data,
            courses2,
            course2_data,
            accounts,
            votes,
        }
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
        ];
        let listed_indexes: Vec<OrderedDocument> = courses2
            .list_indexes()?
            .map(|item| -> Result<OrderedDocument, mongodb::Error> { Ok(item?) })
            .filter_map(Result::ok)
            .collect();
        for index in indexes {
            if listed_indexes.iter().find(|idx| idx == &&index).is_none() {
                courses2.create_index(index, None)?;
            }
        }
        Ok(())
    }

    fn generate_votes_indexes(votes: &Collection) -> Result<(), mongodb::error::Error> {
        let indexes = vec![doc! {
            "account_id": 1,
            "course_id": 1,
        }];
        let listed_indexes: Vec<OrderedDocument> = votes
            .list_indexes()?
            .map(|item| -> Result<OrderedDocument, mongodb::Error> { Ok(item?) })
            .filter_map(Result::ok)
            .collect();
        for index in indexes {
            if listed_indexes.iter().find(|idx| idx == &&index).is_none() {
                votes.create_index(index, None)?;
            }
        }
        Ok(())
    }

    pub fn get_courses(&self, query: Vec<OrderedDocument>) -> Result<Cursor, mongodb::Error> {
        self.courses.aggregate(query, None)
    }

    pub fn get_courses2(&self, query: Vec<OrderedDocument>) -> Result<Cursor, mongodb::Error> {
        self.courses2.aggregate(query, None)
    }

    pub fn fill_lsh_index(&self) -> Result<Cursor, mongodb::Error> {
        self.courses2.find(
            None,
            Some(FindOptions {
                projection: Some(doc! {
                    "hash" => 1
                }),
                ..Default::default()
            }),
        )
    }

    pub fn put_course2(
        &self,
        doc_meta: OrderedDocument,
        data: Bson,
        thumb: Bson,
        thumb_encrypted: Bson,
    ) -> Result<ObjectId, mongodb::error::Error> {
        let insert_res = self.courses2.insert_one(doc_meta, None)?;
        let inserted_id = insert_res
            .inserted_id
            .ok_or_else(|| mongodb::Error::ResponseError("inserted_id not given".to_string()))?;
        let doc = doc! {
            "_id" => inserted_id.clone(),
            "data_encrypted" => data,
            "thumb" => thumb,
            "thumb_encrypted" => thumb_encrypted,
        };
        self.course2_data.insert_one(doc, None)?;
        Ok(inserted_id.as_object_id().unwrap().clone())
    }

    pub fn get_course2(
        &self,
        doc: OrderedDocument,
        projection: OrderedDocument,
    ) -> Result<Option<OrderedDocument>, mongodb::error::Error> {
        self.course2_data.find_one(
            Some(doc),
            Some(FindOptions {
                projection: Some(projection),
                ..Default::default()
            }),
        )
    }

    pub fn update_course2_thumbnail(
        &self,
        course_id: ObjectId,
        size: String,
        data: Vec<u8>,
    ) -> Result<(), mongodb::error::Error> {
        let data = Bson::Binary(BinarySubtype::Generic, data);
        let filter = doc! {
            "_id" => course_id
        };
        let update = doc! {
            "$set" => {
                size => data
            }
        };
        self.course2_data.update_one(filter, update, None)?;
        Ok(())
    }

    pub fn update_course2(
        &self,
        filter: OrderedDocument,
        update: OrderedDocument,
    ) -> Result<(), mongodb::error::Error> {
        self.courses2.update_one(filter, update, None)?;
        Ok(())
    }

    pub fn delete_course2(
        &self,
        course_id: String,
        doc: OrderedDocument,
    ) -> Result<(), mongodb::error::Error> {
        self.courses2.delete_one(doc.clone(), None)?;
        let res = self.course2_data.delete_one(doc, None)?;
        if res.deleted_count == 0 {
            Err(mongodb::Error::ArgumentError(course_id))
        } else {
            Ok(())
        }
    }

    pub fn vote_course2(
        &self,
        filter: OrderedDocument,
        update: OrderedDocument,
    ) -> Result<(), mongodb::error::Error> {
        self.votes.update_one(
            filter,
            update,
            Some(UpdateOptions {
                upsert: Some(true),
                ..UpdateOptions::default()
            }),
        )?;
        Ok(())
    }

    pub fn unvote_course2(&self, filter: OrderedDocument) -> Result<(), mongodb::error::Error> {
        self.votes.delete_one(filter, None)?;
        Ok(())
    }

    pub fn get_votes_course2(
        &self,
        filter: OrderedDocument,
        projection: OrderedDocument,
    ) -> Result<Cursor, mongodb::Error> {
        self.votes.find(
            Some(filter),
            Some(FindOptions {
                projection: Some(projection),
                ..FindOptions::default()
            }),
        )
    }

    pub fn find_courses2(&self, doc: OrderedDocument) -> Result<Cursor, mongodb::Error> {
        self.courses2.find(Some(doc), None)
    }

    pub fn update_courses2(
        &self,
        filter: OrderedDocument,
        update: OrderedDocument,
    ) -> Result<UpdateResult, mongodb::Error> {
        self.courses2.update_one(filter, update, None)
    }

    pub fn find_account(
        &self,
        filter: OrderedDocument,
    ) -> Result<Option<OrderedDocument>, mongodb::Error> {
        self.accounts.find_one(Some(filter), None)
    }

    pub fn get_accounts(&self, account_ids: Vec<Bson>) -> Result<Cursor, mongodb::Error> {
        self.accounts.find(
            Some(doc! {
                "_id": {
                    "$in": account_ids
                }
            }),
            None,
        )
    }

    pub fn insert_account(
        &self,
        account: OrderedDocument,
    ) -> Result<InsertOneResult, mongodb::Error> {
        self.accounts.insert_one(account, None)
    }

    pub fn update_account(
        &self,
        filter: OrderedDocument,
        update: OrderedDocument,
    ) -> Result<UpdateResult, mongodb::Error> {
        self.accounts.update_one(filter, update, None)
    }

    pub fn delete_account_session(
        &self,
        account_id: &ObjectId,
    ) -> Result<(), mongodb::error::Error> {
        let filter = doc! {
            "_id" => account_id.clone()
        };
        let update = doc! {
            "$unset" => {
                "session" => ""
            }
        };
        self.accounts.update_one(filter, update, None)?;
        Ok(())
    }

    pub fn add_course2_data_br(
        &self,
        course_id: ObjectId,
        course: OrderedDocument,
    ) -> Result<Vec<u8>, mongodb::error::Error> {
        use std::io::prelude::*;

        let bson_course = course.get("data_encrypted").unwrap();
        if let Bson::Binary(_, mut course_data) = bson_course.clone() {
            smmdb_lib::Course2::decrypt(&mut course_data);

            let mut data_br = vec![];
            let mut params = CompressParams::new();
            params.quality(11);
            BrotliEncoder::from_params(&course_data[..], &params).read_to_end(&mut data_br)?;

            let filter = doc! {
                "_id" => course_id,
            };
            let update = doc! {
                "$set" => {
                    "data_br" => Bson::Binary(BinarySubtype::Generic, data_br.clone()),
                }
            };
            self.course2_data.update_one(filter, update, None).unwrap();
            Ok(data_br)
        } else {
            todo!()
        }
    }

    pub fn add_course2_data_protobuf_br(
        &self,
        course_id: ObjectId,
        course: OrderedDocument,
    ) -> Result<Vec<u8>, mongodb::error::Error> {
        use std::io::prelude::*;

        let bson_course = course.get("data_encrypted").unwrap();
        if let Bson::Binary(_, course_data) = bson_course.clone() {
            let course =
                smmdb_lib::Course2::from_switch_files(course_data.clone(), None, true).unwrap();
            let course_data = course.into_proto();

            let mut data_br = vec![];
            let mut params = CompressParams::new();
            params.quality(11);
            BrotliEncoder::from_params(&course_data[..], &params).read_to_end(&mut data_br)?;

            let filter = doc! {
                "_id" => course_id,
            };
            let update = doc! {
                "$set" => {
                    "data_protobuf_br" => Bson::Binary(BinarySubtype::Generic, data_br.clone()),
                }
            };
            self.course2_data.update_one(filter, update, None).unwrap();
            Ok(data_br)
        } else {
            todo!()
        }
    }
}
