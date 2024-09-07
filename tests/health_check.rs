use std::net::TcpListener;

use once_cell::sync::Lazy;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;

use zero2prod::configuration::{get_configuration, DatabaseSettings};
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

// the function is changed to being asynchronous
async fn spawn_app() -> TestApp {
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
    let sender_email = configuration
        .email_client
        .sender()
        .expect("Invalid sender email address.");
    let email_client = EmailClient::email_client(
        configuration.email_client.base_url,
        sender_email,
        configuration.email_client.authorization_token,
    );

    let server = run(listener, conn_pool.clone(), email_client).expect("failed to bind address");

    let _ = tokio::spawn(server);

    // return app address to the caller
    TestApp {
        address,
        db_pool: conn_pool,
    }
}

// `tokio::test` is the testing equivalent of `tokio::main`.
// It also spares you from having to specify the `#[test]` attribute.
//
// You can inspect what code gets generated using
// `cargo expand --test health_check` (<- name of the test file)
#[tokio::test]
async fn health_check_works() {
    // Arrange
    let app = spawn_app().await;

    // we need to bring in request
    // to perform HTTP requests against our app
    let client = reqwest::Client::new();

    // Act
    let response = client
        .get(&format!("{}/health_check", &app.address))
        .send()
        .await
        .expect("Failed to execute request");

    // Assert
    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[tokio::test]
#[ignore]
async fn subscribe_returns_200_for_valid_data() {
    // Arrange
    let app = spawn_app().await;
    let client = reqwest::Client::new();

    // Action
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    let response = client
        .post(&format!("{}/subscriptions", &app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");

    // Assert
    assert_eq!(200, response.status().as_u16());

    let saved = sqlx::query!("SELECT email, name FROM subscriptions",)
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscription.");

    assert_eq!(saved.name, "le guin");
    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
}

#[tokio::test]
async fn subscribe_returns_a_400_when_data_is_missing() {
    // Arrange
    let app = spawn_app().await;

    let client = reqwest::Client::new();
    let test_cases = vec![
        ("name=le%20guin", "missing the email"),
        ("email=ursula_le_guin%40gmail.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    for (invalid_body, err_msg) in test_cases {
        // Action
        let response = client
            .post(&format!("{}/subscriptions", &app.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request.");

        // Assert
        assert_eq!(
            400,
            response.status().as_u16(),
            // Additional customised error message on test failure
            "The API did not fail with 400 Bad Request when the payload was {}.",
            err_msg
        );
    }
}
#[tokio::test]
async fn subscribe_returns_a_400_when_data_present_but_invalid() {
    // Arrange
    let app = spawn_app().await;

    let client = reqwest::Client::new();

    let test_cases = vec![
        ("name=&email=ursula_le_guin%40gmail.com", "empty name"),
        ("name=Ursula&email=", "empty email"),
        ("name=Ursula&email=definitely-not-an-email", "invalid email"),
    ];

    for (invalid_body, err_msg) in test_cases {
        // Action
        let response = client
            .post(&format!("{}/subscriptions", &app.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request.");

        // Assert
        assert_eq!(
            400,
            response.status().as_u16(),
            // Additional customised error message on test failure
            "The API did not fail with 400 Bad Request when the payload was {}.",
            err_msg
        );
    }
}

pub async fn configure_database(config: &DatabaseSettings) -> PgPool {
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
