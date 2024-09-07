use std::net::TcpListener;

use actix_web::dev::Server;
use actix_web::web::Data;
use actix_web::{web, App, HttpRequest, HttpServer, Responder};
use sqlx::PgPool;
use tracing_actix_web::TracingLogger;

use crate::email_client::EmailClient;
use crate::routes::check_health;
use crate::routes::subscribe;

async fn greet(req: HttpRequest) -> impl Responder {
    let name = req.match_info().get("name").unwrap_or("world");
    format!("Hi, {}", name)
}

pub fn run(
    listener: TcpListener,
    connection_pool: PgPool,
    email_client: EmailClient,
) -> Result<Server, std::io::Error> {
    // Wrap the connection in a smart pointer (an ARC) https://doc.rust-lang.org/std/sync/struct.Arc.html
    // Wrap the pool using web::Data, which boils down to an Arc smart pointer
    let db_pool = Data::new(connection_pool);
    // wrap EmailClient in actix_web::web::Data (an Arc pointer) and pass a pointer to
    // app_data every time we need to build an App - like we are doing with PgPool
    let email_client = Data::new(email_client);

    // Capture `connection` from the surrounding environment
    let server = HttpServer::new(move || {
        App::new()
            // middlewares are added using the method `wrap` in `App`
            // will automatically add a requestId
            .wrap(TracingLogger::default())
            .route("/health_check", web::get().to(check_health))
            .route("/subscriptions", web::post().to(subscribe))
            .route("/{name}", web::get().to(greet))
            .app_data(db_pool.clone())
            .app_data(email_client.clone())
    })
    .listen(listener)?
    .run();
    Ok(server)
}
