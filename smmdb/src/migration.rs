use super::Database;

use bson::{oid::ObjectId, ordered::OrderedDocument, spec::BinarySubtype, Bson};
use flate2::read::GzDecoder;
use mongodb::coll::options::FindOptions;
use parking_lot::Mutex;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use rayon::prelude::*;
use smmdb_common::Course2;
use smmdb_db::DatabaseError;
use std::{convert::TryInto, sync::Arc};
use zstd::dict;

pub struct Migration {
    name: String,
    run: fn(&Database),
}

impl Migration {
    pub fn migrate(database: &Database) {
        let mut migrations = vec![
            Migration {
                name: "generate_api_keys".to_string(),
                run: Migration::generate_api_keys,
            },
            Migration {
                name: "bad_courses2".to_string(),
                run: Migration::migrate_bad_courses2,
            },
            Migration {
                name: "course2_data".to_string(),
                run: Migration::migrate_course2_data,
            },
            Migration {
                name: "add_smmdb_id".to_string(),
                run: Migration::add_smmdb_id,
            },
            // TODO fix out of memory
            // Migration {
            //     name: "zstd_dictionary".to_string(),
            //     run: Migration::zstd_dictionary,
            // },
        ];

        let migrations_to_run = database
            .get_missing_migrations(migrations.iter().map(|m| m.name.clone()).collect())
            .unwrap();

        migrations = migrations
            .into_iter()
            .filter(|m| migrations_to_run.contains(&m.name))
            .collect();

        for migration in migrations {
            (migration.run)(database);
            Migration::store_migration_as_completed(database, migration);
        }
    }

    fn store_migration_as_completed(database: &Database, migration: Migration) {
        database.migration_completed(migration.name).unwrap();
    }

    fn generate_api_keys(database: &Database) {
        let accounts: Vec<OrderedDocument> = database
            .accounts
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
                database.accounts.update_one(filter, update, None).unwrap();
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
                let count = *fixed_count.lock() + 1;
                *fixed_count.lock() = count;
            });
        println!("Fixed {} SMM2 courses", fixed_count.lock());
    }

    fn fix_course2(database: &Database, course_id: &Bson) -> Result<(), mongodb::Error> {
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
                    let count = *fixed_count.lock() + 1;
                    *fixed_count.lock() = count;
                }
            });
        println!("Converted {} SMM2 course data", fixed_count.lock());
    }

    fn fix_course2_data(database: &Database, course: Course2) -> Result<bool, mongodb::Error> {
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

    fn add_smmdb_id(database: &Database) {
        println!("Adding SMMDB ID to course data...");
        let fixed_count = Arc::new(Mutex::new(0u32));
        let projection = doc! {
            "_id" => 1,
            "data_encrypted" => 1
        };
        let courses: Vec<_> = database
            .course2_data
            .find(
                None,
                Some(FindOptions {
                    projection: Some(projection),
                    ..Default::default()
                }),
            )
            .unwrap()
            .filter_map(Result::ok)
            .filter_map(|doc| {
                if let (Bson::Binary(_, data), Bson::ObjectId(course_id)) = (
                    doc.get("data_encrypted").unwrap().clone(),
                    doc.get("_id").unwrap().clone(),
                ) {
                    Some((course_id.to_string(), data))
                } else {
                    None
                }
            })
            .collect();

        courses.into_par_iter().for_each(|(course_id, data)| {
            if Migration::add_smmdb_id_to_course(database, course_id, data).is_ok() {
                let count = *fixed_count.lock() + 1;
                *fixed_count.lock() = count;
            }
        });
        println!("Added {} SMMDB IDs to course data", fixed_count.lock());
    }

    fn add_smmdb_id_to_course(
        database: &Database,
        course_id: String,
        data: Vec<u8>,
    ) -> Result<(), mongodb::Error> {
        let mut course = smmdb_lib::Course2::from_switch_files(data, None, true).unwrap();
        course.set_smmdb_id(course_id.clone()).unwrap();
        let mut course_data = course.get_course_data().clone();
        smmdb_lib::Course2::encrypt(&mut course_data);

        let filter = doc! {
            "_id" => ObjectId::with_string(&course_id)?,
        };
        let update = doc! {
            "$set" => {
                "data_encrypted" => Bson::Binary(BinarySubtype::Generic, course_data),
            }
        };
        database
            .course2_data
            .update_one(filter, update, None)
            .unwrap();
        Ok(())
    }

    fn get_courses2_result(
        database: &Database,
        query: Vec<OrderedDocument>,
    ) -> Result<Vec<Result<Course2, DatabaseError>>, mongodb::Error> {
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

    #[allow(unused)]
    fn zstd_dictionary(database: &Database) {
        let options = FindOptions {
            projection: Some(doc! {
                "data_encrypted" => 1,
            }),
            ..FindOptions::default()
        };
        let cursor = database.course2_data.find(None, Some(options)).unwrap();

        let sample_sizes = Arc::new(Mutex::new(vec![]));
        let courses = Arc::new(Mutex::new(Vec::new()));
        // courses
        //     .lock()
        //     .try_reserve(376_832 * cursor.size_hint().0)
        //     .unwrap();
        // courses.append(

        // );
        cursor.into_iter().par_bridge().for_each(|course| {
            let mut course = course.unwrap();
            let course = course.get_binary_generic_mut(&"data_encrypted").unwrap();
            smmdb_lib::Course2::decrypt(course);

            if course.len() != 376_832 {
                panic!("sample size must not change");
            }
            // if let Ok(course) = courses.lock().try_reserve(376_832) {}
            // sample_sizes.lock().push(*sample_size.read());
            // // dbg!(course.len());
            // courses.lock().extend(&course[..])
        });

        let dict = dict::from_continuous(
            // &courses[..],
            &Arc::try_unwrap(courses).unwrap().into_inner()[..],
            &Arc::try_unwrap(sample_sizes).unwrap().into_inner()[..],
            376_832,
        )
        .unwrap();

        todo!();
    }
}
