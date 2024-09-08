use std::net::TcpListener;

use sqlx::PgPool;

use zero2prod::configuration::get_configuration;
use zero2prod::email_client::EmailClient;
use zero2prod::startup::run;
use zero2prod::telemetry::{get_subscriber, init_subscriber};

// this is a binary crate because it contains a main function
#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    // Panic if we can't read configuration
    let configuration = get_configuration().expect("Failed to read configuration.");

    // `connect_lazy_with` instead of `connect_lazy`
    let connection_pool = PgPool::connect_lazy_with(configuration.database.connection_with_db());

    let sender_mail = configuration
        .email_client
        .sender()
        .expect("Invalid sender email configured");

    let time_out = configuration.email_client.timeout();
    let base_url = configuration.email_client.base_url;
    let auth_token = configuration.email_client.authorization_token;
    let email_client = EmailClient::new(base_url, sender_mail, auth_token, time_out);

    let address = format!(
        "{}:{}",
        configuration.application.host, configuration.application.port
    );

    let listener = TcpListener::bind(address).unwrap_or_else(|_| {
        panic!(
            "failed to bind on port {:?}",
            configuration.application.port
        )
    });
    run(listener, connection_pool, email_client)?.await
}
