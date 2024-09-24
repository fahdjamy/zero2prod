//! src/lib.rs
//!
//!

pub use configuration::DatabaseSettings;

pub mod configuration;
pub mod domain;
pub mod email_client;
pub mod routes;
pub mod startup;
pub mod telemetry;
