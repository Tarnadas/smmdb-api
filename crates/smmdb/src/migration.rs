use super::Database;

use bson::{oid::ObjectId, spec::BinarySubtype, Binary, Bson, Document};
use flate2::read::GzDecoder;
use futures::{future::BoxFuture, Future, StreamExt};
use mongodb::options::FindOptions;
use parking_lot::Mutex;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use rayon::prelude::*;
use smmdb_common::{Course2, PermGen};
use smmdb_db::DatabaseError;
use std::{
    convert::{TryFrom, TryInto},
    sync::Arc,
};
use zstd::dict;

pub struct Migration {
    name: String,
    run: fn(&'static Database, &'static PermGen) -> BoxFuture<'static, ()>,
}

impl Migration {
    pub async fn migrate(database: &'static Database, perm_gen: &'static PermGen) {
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
            Migration {
                name: "course2_hash_v2".to_string(),
                run: Migration::course2_hash_v2,
            },
            // TODO fix out of memory
            // Migration {
            //     name: "zstd_dictionary".to_string(),
            //     run: Migration::zstd_dictionary,
            // },
        ];

        let migrations_to_run = database
            .get_missing_migrations(migrations.iter().map(|m| m.name.clone()).collect())
            .await
            .unwrap();

        migrations = migrations
            .into_iter()
            .filter(|m| migrations_to_run.contains(&m.name))
            .collect();

        for migration in migrations {
            (migration.run)(database, perm_gen);
            Migration::store_migration_as_completed(database, migration).await;
        }
    }

    async fn store_migration_as_completed(database: &Database, migration: Migration) {
        database.migration_completed(migration.name).await.unwrap();
    }

    fn generate_api_keys(database: &'static Database, _: &PermGen) -> BoxFuture<'static, ()> {
        Box::pin(async {
            println!("Fixing missing apiKeys...");
            let mut fixed_count = 0u16;
            while let Some(Ok(account)) = database
                .accounts
                .find(None, None)
                .await
                .unwrap()
                .next()
                .await
            {
                let apikey = account.get_str("apikey");
                if apikey.is_err() || apikey == Ok("") {
                    let apikey: String = thread_rng()
                        .sample_iter(&Alphanumeric)
                        .take(30)
                        .map(char::from)
                        .collect();
                    let filter = doc! {
                        "_id": account.get_object_id("_id").unwrap().to_owned()
                    };
                    let update = doc! {
                        "$set": {
                            "apikey": apikey
                        }
                    };
                    database
                        .accounts
                        .update_one(filter, update, None)
                        .await
                        .unwrap();
                    fixed_count += 1;
                }
            }
            println!("Fixed {} accounts", fixed_count);
        })
    }

    fn migrate_bad_courses2(database: &'static Database, _: &PermGen) -> BoxFuture<'static, ()> {
        Box::pin(async {
            println!("Fixing old SMM2 course formats...");
            let fixed_count = Arc::new(Mutex::new(0u32));
            for (doc, _err) in Migration::get_courses2_result(database, vec![])
                .await
                .unwrap()
                .into_iter()
                .filter_map(Result::err)
                .filter_map(|err| match err {
                    DatabaseError::Course2Convert(doc, err) => Some((doc, err)),
                    _ => None,
                })
            {
                Migration::fix_course2(database, doc.get("_id").unwrap())
                    .await
                    .unwrap();
                let count = *fixed_count.lock() + 1;
                *fixed_count.lock() = count;
            }
            println!("Fixed {} SMM2 courses", fixed_count.lock());
        })
    }

    async fn fix_course2(
        database: &Database,
        course_id: &Bson,
    ) -> Result<(), mongodb::error::Error> {
        use std::io::prelude::*;

        let doc = database
            .course2_data
            .find_one(
                Some(doc! {
                    "_id": course_id
                }),
                None,
            )
            .await?
            .unwrap();
        let bson = doc.get("data_gz").unwrap();
        if let Bson::Binary(Binary { bytes: data, .. }) = bson {
            let mut gz = GzDecoder::new(&data[..]);
            let mut course_data = vec![];
            gz.read_to_end(&mut course_data)?;

            let course =
                smmdb_lib::Course2::from_switch_files(&mut course_data, None, false).unwrap();
            if let Ok(doc_meta) = bson::ser::to_document(course.get_course()) {
                let filter = doc! {
                    "_id": course_id.clone(),
                };
                let update = doc! {
                    "$set": {
                        "course": doc_meta,
                    }
                };
                database
                    .courses2
                    .update_one(filter, update, None)
                    .await
                    .unwrap();
            }
        }
        Ok(())
    }

    fn migrate_course2_data(database: &'static Database, _: &PermGen) -> BoxFuture<'static, ()> {
        Box::pin(async {
            println!("Converting SMM2 course data...");
            let fixed_count = Arc::new(Mutex::new(0u32));
            for course in Migration::get_courses2_result(database, vec![])
                .await
                .unwrap()
                .into_iter()
                .filter_map(Result::ok)
            {
                if Migration::fix_course2_data(database, course).await.unwrap() {
                    let count = *fixed_count.lock() + 1;
                    *fixed_count.lock() = count;
                }
            }
            println!("Converted {} SMM2 course data", fixed_count.lock());
        })
    }

    async fn fix_course2_data(
        database: &Database,
        course: Course2,
    ) -> Result<bool, mongodb::error::Error> {
        use std::io::prelude::*;

        let doc = database
            .course2_data
            .find_one(
                Some(doc! {
                    "_id": course.get_id()
                }),
                None,
            )
            .await?
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
        if let Bson::Binary(Binary { bytes: data, .. }) = bson_course {
            if let Bson::Binary(Binary {
                bytes: mut thumb, ..
            }) = bson_thumb
            {
                let mut gz = GzDecoder::new(&data[..]);
                let mut course_data = vec![];
                gz.read_to_end(&mut course_data)?;
                smmdb_lib::Course2::encrypt(&mut course_data);
                smmdb_lib::Thumbnail2::encrypt(&mut thumb);

                let filter = doc! {
                    "_id": course.get_id().clone(),
                };
                let update = doc! {
                    "$set": {
                        "data_encrypted": Bson::Binary(Binary {
                            subtype: BinarySubtype::Generic,
                            bytes: course_data
                        }),
                        "thumb_encrypted": Bson::Binary(Binary {
                            subtype: BinarySubtype::Generic,
                            bytes: thumb
                        }),
                    },
                    "$unset": {
                        "data_gz": "",
                        "data_br": "",
                    }
                };
                database
                    .course2_data
                    .update_one(filter, update, None)
                    .await
                    .unwrap();
            }
        }
        Ok(true)
    }

    fn add_smmdb_id(database: &'static Database, _: &PermGen) -> BoxFuture<'static, ()> {
        Box::pin(async {
            println!("Adding SMMDB ID to course data...");
            let fixed_count = Arc::new(Mutex::new(0u32));
            let projection = doc! {
                "_id": 1,
                "data_encrypted": 1
            };
            while let Some(doc) = database
                .course2_data
                .find(
                    None,
                    Some(FindOptions::builder().projection(Some(projection)).build()),
                )
                .await
                .unwrap()
                .next()
                .await
            {
                if let Ok(doc) = doc {
                    if let (Bson::Binary(Binary { bytes: data, .. }), Bson::ObjectId(course_id)) = (
                        doc.get("data_encrypted").unwrap().clone(),
                        doc.get("_id").unwrap().clone(),
                    ) {
                        let course_id = course_id.to_string();
                        if Migration::add_smmdb_id_to_course(database, course_id, data)
                            .await
                            .is_ok()
                        {
                            let count = *fixed_count.lock() + 1;
                            *fixed_count.lock() = count;
                        }
                    }
                }
            }

            println!("Added {} SMMDB IDs to course data", fixed_count.lock());
        })
    }

    async fn add_smmdb_id_to_course(
        database: &Database,
        course_id: String,
        mut data: Vec<u8>,
    ) -> Result<(), mongodb::error::Error> {
        let mut course = smmdb_lib::Course2::from_switch_files(&mut data, None, true).unwrap();
        course.set_smmdb_id(course_id.clone()).unwrap();
        let mut course_data = course.get_course_data_mut().to_vec();
        smmdb_lib::Course2::encrypt(&mut course_data);

        let filter = doc! {
            "_id": ObjectId::with_string(&course_id).unwrap(),
        };
        let update = doc! {
            "$set": {
                "data_encrypted": Bson::Binary(Binary {
                    subtype: BinarySubtype::Generic,
                    bytes: course_data.to_vec()
                }),
            }
        };
        database
            .course2_data
            .update_one(filter, update, None)
            .await
            .unwrap();
        Ok(())
    }

    async fn get_courses2_result(
        database: &Database,
        query: Vec<Document>,
    ) -> Result<Vec<Result<Course2, DatabaseError>>, mongodb::error::Error> {
        let mut cursor = database.courses2.aggregate(query, None).await?;

        let mut courses: Vec<Result<Course2, DatabaseError>> = vec![];
        while let Some(item) = cursor.next().await {
            match item {
                Ok(item) => match Course2::try_from(item.clone()) {
                    Ok(course) => {
                        courses.push(Ok(course));
                    }
                    Err(err) => courses.push(Err(DatabaseError::Course2Convert(item, err))),
                },
                Err(err) => courses.push(Err(err.into())),
            }
        }

        Ok(courses)
    }

    fn course2_hash_v2(
        database: &'static Database,
        perm_gen: &'static PermGen,
    ) -> BoxFuture<'static, ()> {
        Box::pin(async {
            println!("Adjusting course2 hashes...");
            let fixed_count = Arc::new(Mutex::new(0u32));
            let projection = doc! {
                "_id": 1,
                "data_encrypted": 1
            };
            let courses: Vec<_> = database
                .course2_data
                .find(
                    None,
                    Some(FindOptions::builder().projection(Some(projection)).build()),
                )
                .await
                .unwrap()
                .collect()
                .await;

            let courses: Vec<_> = courses
                .into_par_iter()
                .filter_map(Result::ok)
                .filter_map(|doc| {
                    if let (Bson::Binary(Binary { bytes: data, .. }), Bson::ObjectId(course_id)) = (
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
                futures::executor::block_on(async {
                    if Migration::adjust_hash(database, perm_gen, course_id, data)
                        .await
                        .is_ok()
                    {
                        let count = *fixed_count.lock() + 1;
                        *fixed_count.lock() = count;
                    }
                });
            });
            println!("Adjusted {} course2 hashes", fixed_count.lock());
        })
    }

    async fn adjust_hash(
        database: &Database,
        perm_gen: &PermGen,
        course_id: String,
        mut data: Vec<u8>,
    ) -> Result<(), mongodb::error::Error> {
        let course = smmdb_lib::Course2::from_switch_files(&mut data, None, true).unwrap();
        let course = Course2::insert(ObjectId::new(), &course, None, perm_gen);

        let filter = doc! {
            "_id": ObjectId::with_string(&course_id).unwrap(),
        };
        let update = doc! {
            "$set": {
                "hash": bson::ser::to_bson(course.get_hash()).unwrap(),
            }
        };
        database
            .courses2
            .update_one(filter, update, None)
            .await
            .unwrap();
        Ok(())
    }

    // #[allow(unused)]
    // fn zstd_dictionary(database: &Database) {
    //     let options = FindOptions::builder()
    //         .projection(doc! {
    //             "data_encrypted": 1,
    //         })
    //         .build();
    //     let cursor = database
    //         .course2_data
    //         .find(None, Some(options))
    //         .await
    //         .unwrap();

    //     let sample_sizes = Arc::new(Mutex::new(vec![]));
    //     let courses = Arc::new(Mutex::new(Vec::new()));
    //     // courses
    //     //     .lock()
    //     //     .try_reserve(376_832 * cursor.size_hint().0)
    //     //     .unwrap();
    //     // courses.append(

    //     // );
    //     cursor.into_iter().par_bridge().for_each(|course| {
    //         let mut course = course.unwrap();
    //         let course = course.get_binary_generic_mut(&"data_encrypted").unwrap();
    //         smmdb_lib::Course2::decrypt(course);

    //         if course.len() != 376_832 {
    //             panic!("sample size must not change");
    //         }
    //         // if let Ok(course) = courses.lock().try_reserve(376_832) {}
    //         // sample_sizes.lock().push(*sample_size.read());
    //         // // dbg!(course.len());
    //         // courses.lock().extend(&course[..])
    //     });

    //     let dict = dict::from_continuous(
    //         // &courses[..],
    //         &Arc::try_unwrap(courses).unwrap().into_inner()[..],
    //         &Arc::try_unwrap(sample_sizes).unwrap().into_inner()[..],
    //         376_832,
    //     )
    //     .unwrap();

    //     todo!();
    // }
}
