use crate::routes::{courses, courses2, index, login, logout};
use crate::session::Auth;

use actix_cors::Cors;
use actix_session::CookieSession;
use actix_web::{
    client::Client,
    dev::Server as ActixServer,
    middleware::{Compress, Logger},
    App, HttpServer,
};
use paperclip::{
    actix::{web, OpenApiExt},
    v2::models::{DefaultApiRaw, Info, Tag},
};
use smmdb_common::PermGen;
use smmdb_db::Database;
use std::{io, sync::Arc};

mod data;

pub use data::*;

pub struct Server;

impl Server {
    pub fn start(database: Arc<Database>, perm_gen: PermGen) -> Result<ActixServer, io::Error> {
        println!("Starting SMMDB API server");
        std::env::set_var("RUST_LOG", "actix_web=debug");
        env_logger::init();
        let data = Arc::new(Data::new(database, perm_gen));

        Ok(HttpServer::new(move || {
            let spec = DefaultApiRaw {
                tags: vec![Tag {
                    name: "SMM1".to_string(),
                    description: Some("Super Mario Maker 1 API".to_string()),
                    external_docs: None,
                }, Tag {
                    name: "SMM2".to_string(),
                    description: Some("Super Mario Maker 2 API".to_string()),
                    external_docs: None,
                }, Tag {
                    name: "Auth".to_string(),
                    description: Some("Authorization handling".to_string()),
                    external_docs: None,
                }],
                info: Info {
                    title: "SMMDB API".into(),
                    description: Some("
[![GitHub](https://img.shields.io/github/stars/Tarnadas/smmdb?label=Github%20Website&style=flat)](https://github.com/Tarnadas/smmdb)
[![GitHub](https://img.shields.io/github/stars/Tarnadas/smmdb-api?label=Github%20API&style=flat)](https://github.com/Tarnadas/smmdb-api)
[![Discord](https://img.shields.io/discord/168893527357521920?label=Discord&logo=discord&color=7289da)](https://discord.gg/SPZsgSe)
[![Twitter](https://img.shields.io/twitter/follow/marior_dev?style=flat&logo=twitter&label=follow&color=00acee)](https://twitter.com/marior_dev)

A cross console/emulator sharing platform for Super Mario Maker courses to rule them all.

## [Website](https://smmdb.net)

## [Cemu SMMDB](https://github.com/tarnadas/cemu-smmdb)

[![GitHub All Releases](https://img.shields.io/github/downloads/Tarnadas/cemu-smmdb/total)](https://github.com/Tarnadas/cemu-smmdb/releases)
[![GitHub Releases](https://img.shields.io/github/downloads/Tarnadas/cemu-smmdb/latest/total)](https://github.com/Tarnadas/cemu-smmdb/releases/latest)

A save file editor for Super Mario Maker.

## [SMMDB Client](https://github.com/tarnadas/smmdb-client)

[![Test](https://github.com/Tarnadas/smmdb-client/workflows/Test/badge.svg)](https://github.com/Tarnadas/smmdb-client/actions)
[![GitHub All Releases](https://img.shields.io/github/downloads/Tarnadas/smmdb-client/total)](https://github.com/Tarnadas/smmdb-client/releases)
[![GitHub Releases](https://img.shields.io/github/downloads/Tarnadas/smmdb-client/latest/total)](https://github.com/Tarnadas/smmdb-client/releases/latest)

Save file editor for Super Mario Maker 2.

It will automatically detect your Yuzu and Ryujinx save folder, but you can also manually select any SMM2 save file on your system.

This software lets you download courses from SMMDB.
For planned features, please visit the [Github issue page](https://github.com/Tarnadas/smmdb-client/issues).

### Install

You can download Windows, Linux and MacOS binaries in the [Github release section](https://github.com/Tarnadas/smmdb-client/releases)

#### via Cargo

You can install SMMDB Client via Cargo:

`cargo install --git https://github.com/Tarnadas/smmdb-client.git`

It is recommended to install Cargo via [Rustup](https://rustup.rs/)

#### via Chocolatey (Windows Only)

`choco install smmdb-client`

Chocolatey install instructions/docs [Chocolatey.org](https://chocolatey.org/install)
".into()),
                    ..Default::default()
                },
                ..Default::default()
            };

            App::new()
                .wrap_api_with_spec(spec)
                .data(data.clone())
                .data(Client::default())
                .service(courses::service())
                .service(courses2::service())
                .service(login::service())
                .service(logout::service())
                .service(web::resource("/").route(web::get().to(index)))
                .with_json_spec_at("/api/spec")
                .wrap(Auth)
                .wrap(
                    CookieSession::signed(&[0; 32])
                        .name("smmdb")
                        .path("/")
                        .max_age(3600 * 24 * 7)
                        .secure(false),
                )
                .wrap(Cors::permissive())
                .wrap(Compress::default())
                .wrap(Logger::default())
                .build()
        })
        .bind("0.0.0.0:3030")?
        .workers(num_cpus::get())
        .run())
    }
}
