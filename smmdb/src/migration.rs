#![allow(dead_code)]

use super::Database;

use crate::{course2::Course2, database::DatabaseError};

use brotli2::{read::BrotliEncoder, CompressParams};
use bson::{ordered::OrderedDocument, spec::BinarySubtype, Bson};
use flate2::read::GzDecoder;
use mongodb::coll::Collection;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use rayon::prelude::*;
use std::{
    convert::TryInto,
    sync::{Arc, Mutex},
};

pub struct Migration;

impl Migration {
    pub fn run(database: &Database) {
        Migration::generate_api_keys(&database.accounts);
        Migration::migrate_bad_courses2(database);
        Migration::migrate_course2_data(database);
        // Migration::migrate_course2_data_protobuf(database);
        // Migration::migrate_course2_data_br(database);
    }

    fn generate_api_keys(coll: &Collection) {
        let accounts: Vec<OrderedDocument> = coll
            .find(None, None)
            .unwrap()
            .map(|item| item.unwrap())
            .collect();
        println!("Fixing missing apiKeys...");
        let mut fixed_count = 0u16;
        for account in accounts {
            let apikey = account.get_str("apikey");
            if apikey.is_err() || apikey == Ok("") {
                let apikey: String = thread_rng().sample_iter(&Alphanumeric).take(30).collect();
                let filter = doc! {
                    "_id" => account.get_object_id("_id").unwrap().to_owned()
                };
                let update = doc! {
                    "$set" => {
                        "apikey" => apikey
                    }
                };
                coll.update_one(filter, update, None).unwrap();
                fixed_count += 1;
            }
        }
        println!("Fixed {} accounts", fixed_count);
    }

    fn migrate_bad_courses2(database: &Database) {
        println!("Fixing old SMM2 course formats...");
        let fixed_count = Arc::new(Mutex::new(0u32));
        Migration::get_courses2_result(database, vec![])
            .unwrap()
            .into_par_iter()
            .filter_map(Result::err)
            .filter_map(|err| match err {
                DatabaseError::Course2ConvertError(doc, err) => Some((doc, err)),
                _ => None,
            })
            .for_each(|(doc, _err)| {
                Migration::fix_course2(database, doc.get("_id").unwrap()).unwrap();
                let count = *fixed_count.lock().unwrap() + 1;
                *fixed_count.lock().unwrap() = count;
            });
        println!("Fixed {} SMM2 courses", fixed_count.lock().unwrap());
    }

    fn fix_course2(database: &Database, course_id: &Bson) -> Result<(), mongodb::error::Error> {
        use std::io::prelude::*;

        let doc = database
            .course2_data
            .find_one(
                Some(doc! {
                    "_id" => course_id
                }),
                None,
            )?
            .unwrap();
        let bson = doc.get("data_gz").unwrap();
        if let Bson::Binary(_, data) = bson {
            let mut gz = GzDecoder::new(&data[..]);
            let mut course_data = vec![];
            gz.read_to_end(&mut course_data)?;

            let course = smmdb_lib::Course2::from_switch_files(course_data, None, false).unwrap();
            let course_meta = serde_json::to_value(course.get_course()).unwrap();
            if let Bson::Document(doc_meta) = Bson::from(course_meta) {
                let filter = doc! {
                    "_id" => course_id.clone(),
                };
                let update = doc! {
                    "$set" => {
                        "course" => doc_meta,
                    }
                };
                database.courses2.update_one(filter, update, None).unwrap();
            }
        }
        Ok(())
    }

    fn migrate_course2_data(database: &Database) {
        println!("Converting SMM2 course data...");
        let fixed_count = Arc::new(Mutex::new(0u32));
        Migration::get_courses2_result(database, vec![])
            .unwrap()
            .into_par_iter()
            .filter_map(Result::ok)
            .for_each(|course| {
                if Migration::fix_course2_data(database, course).unwrap() {
                    let count = *fixed_count.lock().unwrap() + 1;
                    *fixed_count.lock().unwrap() = count;
                }
            });
        println!("Converted {} SMM2 course data", fixed_count.lock().unwrap());
    }

    fn fix_course2_data(
        database: &Database,
        course: Course2,
    ) -> Result<bool, mongodb::error::Error> {
        use std::io::prelude::*;

        let doc = database
            .course2_data
            .find_one(
                Some(doc! {
                    "_id" => course.get_id()
                }),
                None,
            )?
            .unwrap();
        let bson_course = doc.get("data_gz");
        if bson_course.is_none() {
            return Ok(false);
        }
        let bson_course = bson_course.unwrap();
        let bson_thumb = doc.get("thumb");
        if bson_thumb.is_none() {
            return Ok(false);
        }
        let bson_thumb = bson_thumb.unwrap().clone();
        if let Bson::Binary(_, data) = bson_course {
            if let Bson::Binary(_, mut thumb) = bson_thumb {
                let mut gz = GzDecoder::new(&data[..]);
                let mut course_data = vec![];
                gz.read_to_end(&mut course_data)?;
                smmdb_lib::Course2::encrypt(&mut course_data);
                smmdb_lib::Thumbnail2::encrypt(&mut thumb);

                let filter = doc! {
                    "_id" => course.get_id().clone(),
                };
                let update = doc! {
                    "$set" => {
                        "data_encrypted" => Bson::Binary(BinarySubtype::Generic, course_data),
                        "thumb_encrypted" => Bson::Binary(BinarySubtype::Generic, thumb.clone()),
                    },
                    "$unset" => {
                        "data_gz" => "",
                        "data_br" => "",
                    }
                };
                database
                    .course2_data
                    .update_one(filter, update, None)
                    .unwrap();
            }
        }
        Ok(true)
    }

    fn migrate_course2_data_protobuf(database: &Database) {
        println!("Adding SMM2 protobuf data...");
        let fixed_count = Arc::new(Mutex::new(0u32));
        Migration::get_courses2_result(database, vec![])
            .unwrap()
            .into_par_iter()
            .filter_map(Result::ok)
            .for_each(|course| {
                if Migration::add_course2_data_protobuf(database, course).unwrap() {
                    let count = *fixed_count.lock().unwrap() + 1;
                    *fixed_count.lock().unwrap() = count;
                }
            });
        println!("Added {} SMM2 protobuf data", fixed_count.lock().unwrap());
    }

    fn add_course2_data_protobuf(
        database: &Database,
        course: Course2,
    ) -> Result<bool, mongodb::error::Error> {
        use std::io::prelude::*;

        let doc = database
            .course2_data
            .find_one(
                Some(doc! {
                    "_id" => course.get_id()
                }),
                None,
            )?
            .unwrap();
        if doc.get("data_protobuf_br").is_some() {
            return Ok(false);
        }
        let bson_course = doc.get("data_encrypted");
        if bson_course.is_none() {
            return Ok(false);
        }
        let bson_course = bson_course.unwrap();
        let course_id = course.get_id().clone();
        if let Bson::Binary(_, course_data) = bson_course {
            let course =
                smmdb_lib::Course2::from_switch_files(course_data.clone(), None, true).unwrap();
            let course_proto = course.into_proto();

            let mut data_br = vec![];
            let mut params = CompressParams::new();
            params.quality(11);
            BrotliEncoder::from_params(&course_proto[..], &params).read_to_end(&mut data_br)?;

            let filter = doc! {
                "_id" => course_id,
            };
            let update = doc! {
                "$set" => {
                    "data_protobuf_br" => Bson::Binary(BinarySubtype::Generic, data_br),
                }
            };
            database
                .course2_data
                .update_one(filter, update, None)
                .unwrap();
        }
        Ok(true)
    }

    fn migrate_course2_data_br(database: &Database) {
        println!("Adding SMM2 decrypted brotli data...");
        let fixed_count = Arc::new(Mutex::new(0u32));
        Migration::get_courses2_result(database, vec![])
            .unwrap()
            .into_par_iter()
            .filter_map(Result::ok)
            .for_each(|course| {
                if Migration::add_course2_data_br(database, course).unwrap() {
                    let count = *fixed_count.lock().unwrap() + 1;
                    *fixed_count.lock().unwrap() = count;
                }
            });
        println!(
            "Added {} SMM2 decrypted brotli data",
            fixed_count.lock().unwrap()
        );
    }

    fn add_course2_data_br(
        database: &Database,
        course: Course2,
    ) -> Result<bool, mongodb::error::Error> {
        use std::io::prelude::*;

        let doc = database
            .course2_data
            .find_one(
                Some(doc! {
                    "_id" => course.get_id()
                }),
                None,
            )?
            .unwrap();
        if doc.get("data_br").is_some() {
            return Ok(false);
        }
        let bson_course = doc.get("data_encrypted");
        if bson_course.is_none() {
            return Ok(false);
        }
        let bson_course = bson_course.unwrap();
        let course_id = course.get_id().clone();
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
                    "data_br" => Bson::Binary(BinarySubtype::Generic, data_br),
                }
            };
            database
                .course2_data
                .update_one(filter, update, None)
                .unwrap();
        }
        Ok(true)
    }

    fn get_courses2_result(
        database: &Database,
        query: Vec<OrderedDocument>,
    ) -> Result<Vec<Result<Course2, DatabaseError>>, mongodb::error::Error> {
        let cursor = database.courses2.aggregate(query, None)?;

        let courses: Vec<Result<Course2, DatabaseError>> = cursor
            .map(|item| -> Result<Course2, DatabaseError> {
                let item = item?;
                let course: Course2 = item
                    .clone()
                    .try_into()
                    .map_err(|err| DatabaseError::Course2ConvertError(item, err))?;
                Ok(course)
            })
            .collect();

        Ok(courses)
    }
}
