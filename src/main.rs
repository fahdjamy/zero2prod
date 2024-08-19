use std::net::TcpListener;

use sqlx::PgPool;

use zero2prod::configuration::get_configuration;
use zero2prod::startup::run;

// this is a binary crate because it contains a main function
#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    // Panic if we can't read configuration
    let configuration = get_configuration().expect("Failed to read configuration.");

    let connection_pool = PgPool::connect(&configuration.database.connection_string())
        .await
        .expect("Failed to connect to the DB");

    let address = format!("127.0.0.1:{}", configuration.application.port);
    let listener = TcpListener::bind(address).expect(
        format!(
            "failed to bind on port {:?}",
            configuration.application.port
        )
        .as_str(),
    );
    run(listener, connection_pool)?.await
}
