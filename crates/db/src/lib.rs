#![warn(clippy::all, clippy::pedantic)]

mod cancellation;
mod client;
mod database;
mod error;
mod profile;
mod tls;

pub use client::PooledClient;
pub use database::{validate_connection_uri, Database};
pub use deadpool_postgres::PoolError;
pub use error::DatabaseError;
pub use profile::ConnectionProfile;
pub use tokio_postgres::{error::SqlState, Error as PostgresError};

#[cfg(test)]
mod tests;
