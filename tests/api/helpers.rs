use fake::faker::internet::en::SafeEmail;
use fake::{Fake, Faker};
use once_cell::sync::Lazy;
use secrecy::Secret;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::net::TcpListener;
use uuid::Uuid;
use zero2prod::configuration::{get_configuration, DatabaseSettings};
use zero2prod::domain::SubscriberEmail;
use zero2prod::email_client::EmailClient;
use zero2prod::startup::run;
use zero2prod::telemetry::{get_subscriber, init_subscriber};

// Ensure that the `tracing` stack is only initialised once using `once_cell`
static TRACING: Lazy<()> = Lazy::new(|| {
    let subscriber_name = "test".into();
    let filter_log_level = "debug".into();
    // We cannot assign the output of `get_subscriber` to a variable based on the
    // value TEST_LOG` because the sink is part of the type returned by
    // `get_subscriber`, therefore they are not the same type. We could work around
    // it, but this is the most straight-forward way of moving forward.

    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name, filter_log_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, filter_log_level, std::io::sink);
        init_subscriber(subscriber);
    };
});

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
}

/// Generate a random subscriber email
fn fake_email() -> SubscriberEmail {
    SubscriberEmail::parse(SafeEmail().fake()).unwrap()
}

pub fn email_client(base_url: String) -> EmailClient {
    let time_out = std::time::Duration::from_millis(200);
    EmailClient::new(base_url, fake_email(), Secret::new(Faker.fake()), time_out)
}

// the function is changed to being asynchronous
pub async fn spawn_app() -> TestApp {
    // The first time `initialize` is invoked the code in `TRACING` is executed.
    // All other invocations will instead skip execution.
    Lazy::force(&TRACING);

    // “Port 0 is special-cased at the OS level:
    //  trying to bind port 0 will trigger an OS scan for an
    //  available port which will then be bound to the application”
    let localhost = "127.0.0.1:0";

    let listener = TcpListener::bind(localhost).expect("Failed to bind random port");
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);

    let mut configuration = get_configuration().expect("failed to read configuration");
    // create a new Database on every call to this function
    configuration.database.database_name = Uuid::new_v4().to_string();

    let conn_pool = configure_database(&configuration.database).await;

    // Build a new email client
    let email_client = email_client(configuration.email_client.base_url);

    let server = run(listener, conn_pool.clone(), email_client).expect("failed to bind address");

    let _ = tokio::spawn(server);

    // return app address to the caller
    TestApp {
        address,
        db_pool: conn_pool,
    }
}

async fn configure_database(config: &DatabaseSettings) -> PgPool {
    // Create database
    // Omitting the db name, we connect to the Postgres instance, not a specific logical database.
    let mut connection = PgConnection::connect_with(&config.without_db())
        .await
        .expect("Failed to connect to Postgres");

    // create a new database based on the database_name in config
    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database.");

    // Migrate database
    let connection_pool = PgPool::connect_with(config.without_db())
        .await
        .expect("Failed to connect to Postgres.");
    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate the database");

    connection_pool
}
