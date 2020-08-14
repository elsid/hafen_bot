use actix_web::{HttpResponse, web};
use actix_web::dev::Server;

use crate::bot::protocol::Message;

pub fn run_server() -> std::io::Result<Server> {
    use actix_web::{middleware, App, HttpServer};

    Ok(HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default()) // <- limit size of the payload (global configuration)
            .service(web::resource("/ping").route(web::get().to(ping)))
            .default_service(web::resource("").to(HttpResponse::NotFound))
    })
        .bind("127.0.0.1:8080")?
        .run())
}

async fn ping() -> HttpResponse {
    HttpResponse::Ok().json(&Message::Ok)
}
