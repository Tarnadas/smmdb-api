mod delete;
pub mod download;
mod get;
pub mod meta;
mod post;
mod put;
pub mod thumbnail;
mod vote;

pub use delete::*;
pub use get::*;
pub use post::*;
pub use put::*;

use actix_web::dev;
use paperclip::actix::{web, Mountable};

pub fn service() -> impl dev::HttpServiceFactory + Mountable {
    web::scope("/courses2")
        .service(
            web::resource("")
                .route(web::get().to(get::get_courses))
                .route(web::put().to(put::put_courses)),
        )
        .service(web::resource("/analyze").route(web::post().to(post::post_analyze_courses)))
        .service(web::resource("/{course_id}").route(web::delete().to(delete::delete_course)))
        .service(
            web::resource("/download/{course_id}").route(web::get().to(download::download_course)),
        )
        .service(
            web::resource("/thumbnail/{course_id}").route(web::get().to(thumbnail::get_thumbnail)),
        )
        .service(web::resource("/meta/{course_id}").route(web::post().to(meta::post_meta)))
        .service(web::resource("/vote/{course_id}").route(web::post().to(vote::vote_course)))
}
