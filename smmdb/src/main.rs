#[macro_use]
extern crate bson;

#[macro_use]
extern crate failure;

mod config;
mod course;
mod course2;
mod migration;
mod minhash;
mod routes;
mod server;
mod session;
mod vote;

pub use course::Course;
pub use course2::Course2;
pub use vote::Vote;

use migration::Migration;
use server::Server;

use smmdb_db::Database;
use std::io;

#[actix_rt::main]
async fn main() -> io::Result<()> {
    println!("Starting...");
    std::env::set_var("RUST_BACKTRACE", "1");
    use std::sync::Arc;

    let database = Database::new();
    Migration::run(&database);
    Server::start(Arc::new(database)).unwrap().await
}
