#[macro_use]
extern crate log;

use actix_web::{HttpResponse, web};

use crate::bot::Message;

mod bot;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    use actix_web::{middleware, App, HttpServer};

    env_logger::init();

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default()) // <- limit size of the payload (global configuration)
            .service(web::resource("/ping").route(web::get().to(ping)))
            .default_service(web::resource("").to(HttpResponse::NotFound))
    })
        .bind("127.0.0.1:8080")?
        .run()
        .await
}

async fn ping() -> HttpResponse {
    HttpResponse::Ok().json(&Message::Ok)
}
