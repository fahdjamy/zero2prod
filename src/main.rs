use std::net::TcpListener;

use secrecy::ExposeSecret;
use sqlx::PgPool;

use zero2prod::configuration::get_configuration;
use zero2prod::startup::run;
use zero2prod::telemetry::{get_subscriber, init_subscriber};

// this is a binary crate because it contains a main function
#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    // Panic if we can't read configuration
    let configuration = get_configuration().expect("Failed to read configuration.");

    let connection_pool =
        PgPool::connect_lazy(&configuration.database.connection_string().expose_secret())
            .await
            .expect("Failed to connect to the DB");

    let address = format!(
        "{}:{}",
        configuration.application.host, configuration.application.port
    );
    let listener = TcpListener::bind(address).expect(
        format!(
            "failed to bind on port {:?}",
            configuration.application.port
        )
        .as_str(),
    );
    run(listener, connection_pool)?.await
}
