//! src/configuration.rs

use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use secrecy::{ExposeSecret, Secret};
use serde_aux::field_attributes::deserialize_number_from_string;
use sqlx::postgres::{PgConnectOptions, PgSslMode};
use sqlx::ConnectOptions;

#[derive(serde::Deserialize, Clone)]
pub struct Settings {
    pub redis_url: Secret<String>,
    pub database: DatabaseSettings,
    pub application: ApplicationSettings,
    pub email_client: EmailClientSettings,
}

#[derive(serde::Deserialize, Clone)]
pub struct EmailClientSettings {
    pub base_url: String,
    pub sender_email: String,
    timeout_milliseconds: u64,
    pub authorization_token: Secret<String>,
}

impl EmailClientSettings {
    pub fn sender(&self) -> Result<SubscriberEmail, String> {
        SubscriberEmail::parse(self.sender_email.clone())
    }

    pub fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.timeout_milliseconds)
    }

    pub fn client(self) -> EmailClient {
        let sender_email = self.sender().expect("Invalid sender email address");
        let timeout = self.timeout();
        EmailClient::new(
            self.base_url,
            sender_email,
            self.authorization_token,
            timeout,
        )
    }
}

#[derive(serde::Deserialize, Clone)]
pub struct DatabaseSettings {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub username: String,
    // determines of db connection needs to be secure or not
    pub require_ssl: bool,
    pub database_name: String,
    pub password: Secret<String>,
}

impl DatabaseSettings {
    pub fn connection_with_db(&self) -> PgConnectOptions {
        self.without_db()
            .database(&self.database_name)
            // lower logs from INFO to TRACE.
            .log_statements(tracing_log::log::LevelFilter::Trace)
    }

    // helps us connect to the database instance rather than a specific db
    pub fn without_db(&self) -> PgConnectOptions {
        // deciding whether to strictly require an encrypted connection or
        // allow falling back to an unencrypted one if necessary.
        let ssl_mode = if self.require_ssl {
            PgSslMode::Require
        } else {
            // Try an encrypted connection, fallback to unencrypted if it fails
            // First try an SSL connection; if that fails, try a non-SSL connection.
            // This is the default if no other mode is specified
            PgSslMode::Prefer
        };
        PgConnectOptions::new()
            .port(self.port)
            .host(&self.host)
            .ssl_mode(ssl_mode)
            .username(&self.username)
            .password(self.password.expose_secret())
    }
}

#[derive(serde::Deserialize, Clone)]
pub struct ApplicationSettings {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub base_url: String,
    pub hmac_secret: Secret<String>,
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let base_path = std::env::current_dir().expect("failed to determine current dir");

    let configuration_dir = base_path.join("configuration");

    // Detect the running environment.
    // Default to `local` if unspecified.
    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT.");

    let environment_filename = format!("{}.yaml", environment.as_str());

    // Initialise our configuration reader
    let settings = config::Config::builder()
        // Add configuration values from a file named `configuration.yaml`.
        .add_source(config::File::from(configuration_dir.join("base.yaml")))
        .add_source(config::File::from(
            configuration_dir.join(environment_filename),
        ))
        // Add in settings from environment variables (with a prefix of APP and
        // '__' as separator)
        // E.g. `APP_APPLICATION__PORT=8001 would set `Settings.application.port`
        .add_source(
            config::Environment::with_prefix("APP")
                .prefix_separator("_")
                .separator("__"),
        )
        .build()?;

    // Try to convert the configuration values it read into
    // our Settings type
    settings.try_deserialize::<Settings>()
}

/// The possible runtime environment for our application.
pub enum Environment {
    Local,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "{} is not a supported environment. Use either `local` or `production`.",
                other
            )),
        }
    }
}
