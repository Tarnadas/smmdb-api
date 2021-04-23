#[macro_use]
extern crate bson;

#[macro_use]
extern crate failure;

mod config;
mod migration;
mod routes;
mod server;
mod session;

use migration::Migration;
use server::Server;

use smmdb_db::Database;
use std::io;

#[actix_web::main]
async fn main() -> io::Result<()> {
    println!("Starting...");
    std::env::set_var("RUST_BACKTRACE", "1");
    use std::sync::Arc;

    let database = Database::new();
    Migration::migrate(&database);
    Server::start(Arc::new(database)).unwrap().await
}
