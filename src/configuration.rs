//! src/configuration.rs

#[derive(serde::Deserialize)]
pub struct DatabaseSettings {
    pub port: u16,
    pub host: String,
    pub username: String,
    pub password: String,
    pub database_name: String,
}

#[derive(serde::Deserialize)]
pub struct Settings {
    pub application_port: u16,
    pub database: DatabaseSettings,
}
