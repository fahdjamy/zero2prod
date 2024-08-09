use std::net::TcpListener;

use actix_web::dev::Server;
use actix_web::{web, App, HttpRequest, HttpServer, Responder};

use crate::routes::check_health;
use crate::routes::subscribe;

async fn greet(req: HttpRequest) -> impl Responder {
    let name = req.match_info().get("name").unwrap_or("world");
    format!("Hi, {}", name)
}

pub fn run(listener: TcpListener) -> Result<Server, std::io::Error> {
    let server = HttpServer::new(|| {
        App::new()
            .route("/health_check", web::get().to(check_health))
            .route("/subscriptions", web::post().to(subscribe))
            .route("/{name}", web::get().to(greet))
    })
    .listen(listener)?
    .run();
    Ok(server)
}
