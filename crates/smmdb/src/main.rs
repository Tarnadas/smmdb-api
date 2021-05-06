#[macro_use]
extern crate bson;

mod config;
mod migration;
mod routes;
mod server;
mod session;

use migration::Migration;
use server::Server;

use smmdb_common::PermGen;
use smmdb_db::Database;
use std::io;

#[actix_web::main]
async fn main() -> io::Result<()> {
    println!("Starting...");
    std::env::set_var("RUST_BACKTRACE", "1");
    use std::sync::Arc;

    let database = Database::new();
    let perm_gen = PermGen::new(128);
    Migration::migrate(&database, &perm_gen);
    Server::start(Arc::new(database), perm_gen).unwrap().await
}
