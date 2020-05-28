use super::Database;

use crate::{database::DatabaseError};

use bson::{ordered::OrderedDocument, Bson};
use mongodb::coll::Collection;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use rayon::prelude::*;
use std::sync::{Arc, Mutex};

pub struct Migration;

impl Migration {
    pub fn run(database: &Database) {
        Migration::generate_api_keys(&database.accounts);
        Migration::migrate_bad_courses2(database);
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
        database
            .get_courses2_result(vec![])
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
        use compression::prelude::*;

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
            let course_data = data
                .clone()
                .decode(&mut GZipDecoder::new())
                .collect::<Result<Vec<u8>, _>>()
                .unwrap();
            let course =
                cemu_smm::Course2::from_switch_files(&course_data[..], None, false).unwrap();
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
}
