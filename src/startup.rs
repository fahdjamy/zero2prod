use crate::configuration::{DatabaseSettings, Settings};
use crate::email_client::EmailClient;
use crate::routes::{check_health, confirm, home, login, login_form};
use crate::routes::{publish_newsletter, subscribe};
use actix_session::storage::RedisSessionStore;
use actix_session::SessionMiddleware;
use actix_web::cookie::Key;
use actix_web::dev::Server;
use actix_web::web::Data;
use actix_web::{web, App, HttpServer};
use actix_web_flash_messages::storage::CookieMessageStore;
use actix_web_flash_messages::FlashMessagesFramework;
use secrecy::{ExposeSecret, Secret};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;

/// Read-more about the endpoints
///     POST /subscriptions will:
///
///    Add the subscriber details to the database in the subscriptions table, with status equal to pending_confirmation;
///     generate a (unique) subscription_token;
///     store subscription_token in our database against the subscriber id in a subscription_tokens table;
///     Email the new subscriber a link structured as https://<api-domain>/subscriptions/confirm?token=<subscription_token>;
///     containing a token.
///     return a 200 OK.
///
/// Once they click on the link, a browser tab will open up and a GET request will be fired to our
/// GET /subscriptions/confirm endpoint. The request handler will:
///
///    retrieve subscription_token from the query parameters;
///     retrieve the subscriber id associated with subscription_token from the subscription_tokens table;
///     update the subscriber status from pending_confirmation to active in the subscriptions table;
///     return a 200 OK.

pub struct Application {
    port: u16,
    server: Server,
}

#[derive(Clone)]
pub struct HmacSecret(pub Secret<String>);

impl Application {
    pub async fn build(configuration: Settings) -> Result<Self, anyhow::Error> {
        //`connect_lazy_with` instead of `connect_lazy`
        let connection_pool =
            PgPoolOptions::new().connect_lazy_with(configuration.database.connection_with_db());

        let sender_email = configuration
            .email_client
            .sender()
            .expect("Invalid sender email address.");
        let timeout = configuration.email_client.timeout();
        let email_client = EmailClient::new(
            configuration.email_client.base_url,
            sender_email,
            configuration.email_client.authorization_token,
            timeout,
        );

        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );
        let listener = TcpListener::bind(address)?;

        let port = listener.local_addr()?.port();
        let redis_url = configuration.redis_url;
        let base_url = configuration.application.base_url;
        let hmac_secret = configuration.application.hmac_secret;
        let server = run(
            base_url,
            listener,
            connection_pool,
            email_client,
            redis_url,
            hmac_secret,
        )
        .await?;

        // We "save" the bound port in one of `Application`'s fields
        Ok(Self { port, server })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    // A more expressive name that makes it clear that
    // this function only returns when the application is stopped.
    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

// A wrapper type in order to retrieve the URL
// in the `subscribe` handler.
// Retrieval from the context, in actix-web, is type-based: using
// a raw `String` would expose us to conflicts.
pub struct ApplicationBaseUrl(pub String);

pub async fn run(
    base_url: String,
    listener: TcpListener,
    connection_pool: PgPool,
    email_client: EmailClient,
    redis_url: Secret<String>,
    hmac_secret: Secret<String>,
) -> Result<Server, anyhow::Error> {
    // Wrap the connection in a smart pointer (an ARC) https://doc.rust-lang.org/std/sync/struct.Arc.html
    // Wrap the pool using web::Data, which boils down to an Arc smart pointer
    let db_pool = Data::new(connection_pool);
    // wrap EmailClient in actix_web::web::Data (an Arc pointer) and pass a pointer to
    // app_data every time we need to build an App - like we are doing with PgPool
    let email_client = Data::new(email_client);
    let base_url = Data::new(ApplicationBaseUrl(base_url));

    let secret_key = Key::from(hmac_secret.expose_secret().as_bytes());
    let message_store =
        CookieMessageStore::builder(Key::from(hmac_secret.expose_secret().as_bytes())).build();
    let message_framework = FlashMessagesFramework::builder(message_store).build();
    let redis_store = RedisSessionStore::new(redis_url.expose_secret()).await?;
    // Capture `connection` from the surrounding environment
    let server = HttpServer::new(move || {
        App::new()
            // middlewares are added using the method `wrap` in `App`
            // will automatically add a requestId
            .wrap(TracingLogger::default())
            .wrap(message_framework.clone())
            .wrap(SessionMiddleware::new(
                redis_store.clone(),
                secret_key.clone(),
            ))
            .route("/", web::get().to(home))
            .route("/login", web::post().to(login))
            .route("/login", web::get().to(login_form))
            .route("/subscriptions", web::post().to(subscribe))
            .route("/health_check", web::get().to(check_health))
            .route("/subscriptions/confirm", web::get().to(confirm))
            .route("/newsletters", web::post().to(publish_newsletter))
            .app_data(db_pool.clone())
            .app_data(base_url.clone())
            .app_data(email_client.clone())
            .app_data(Data::new(HmacSecret(hmac_secret.clone())))
    })
    .listen(listener)?
    .run();
    Ok(server)
}

pub fn get_connection_pool(configuration: &DatabaseSettings) -> PgPool {
    PgPoolOptions::new().connect_lazy_with(configuration.connection_with_db())
}
