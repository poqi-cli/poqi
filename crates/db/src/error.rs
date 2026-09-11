use std::error::Error as StdError;

use deadpool_postgres::PoolError;
use thiserror::Error;
use tokio_postgres::{error::SqlState, Error as PostgresError};

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("database pool has not been initialized")]
    PoolUninitialized,
    #[error("invalid postgres uri: {0}")]
    InvalidUri(#[source] tokio_postgres::Error),
    #[error("failed to build connection pool: {0}")]
    PoolBuild(#[source] deadpool_postgres::BuildError),
    #[error("failed to acquire connection from pool: {0}")]
    Acquire(#[source] deadpool_postgres::PoolError),
    #[error("no connection profile available for reconnection")]
    MissingProfile,
    #[error("failed to read from COPY stream: {0}")]
    CopyOutIo(#[source] std::io::Error),
    #[error("COPY export does not support parameters")]
    CopyOutParametersUnsupported,
    #[error("postgres error: {}", postgres_error_message(.0))]
    Postgres(#[source] tokio_postgres::Error),
    #[error(
        "database connection was lost; the operation outcome is unknown and was not retried: {0}"
    )]
    ConnectionLost(#[source] tokio_postgres::Error),
    #[error("failed to send PostgreSQL cancellation request: {0}")]
    Cancel(#[source] tokio_postgres::Error),
    #[error("database operation was canceled before dispatch")]
    OperationCanceled,
    #[error("failed to load native TLS certificates: {details}")]
    LoadNativeCerts { details: String },
    #[error("failed to read TLS file {path}: {source}")]
    TlsRead {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse TLS certificate from {path}")]
    TlsCertParse { path: String },
    #[error("failed to parse TLS private key from {path}")]
    TlsKeyParse { path: String },
    #[error("TLS client certificate and key must both be provided")]
    TlsClientAuthIncomplete,
    #[error("sslrootcert requires TLS, but sslmode=disable was requested")]
    TlsRootCertWithDisabledTls,
    #[error("unsupported sslmode; use require, verify-full, prefer, or disable")]
    UnsupportedTlsMode,
    #[error("failed to build TLS connector: {0}")]
    TlsBuild(#[source] Box<dyn std::error::Error + Send + Sync>),
}

fn postgres_error_message(error: &PostgresError) -> String {
    error.as_db_error().map_or_else(
        || error.to_string(),
        |database_error| {
            format!(
                "{} (SQLSTATE {})",
                database_error.message(),
                database_error.code().code()
            )
        },
    )
}

impl DatabaseError {
    /// Returns a credential-safe action the user can take to resolve a connection error.
    #[must_use]
    pub fn connection_hint(&self) -> &'static str {
        const GENERIC_CONNECTION_HINT: &str =
            "Check the host, port, network path, and TLS configuration.";

        match self {
            Self::InvalidUri(_) => {
                "Enter a valid PostgreSQL URI, for example postgres://user:password@host:5432/database."
            }
            Self::TlsRead { .. }
            | Self::TlsCertParse { .. }
            | Self::TlsKeyParse { .. }
            | Self::TlsClientAuthIncomplete
            | Self::TlsRootCertWithDisabledTls
            | Self::UnsupportedTlsMode
            | Self::TlsBuild(_)
            | Self::LoadNativeCerts { .. } => {
                "Check the TLS mode and the configured CA, client certificate, and private-key files."
            }
            Self::Acquire(PoolError::Timeout(_)) => {
                "The connection pool timed out; retry after active work finishes or increase the pool or timeout."
            }
            Self::Acquire(PoolError::Backend(error))
            | Self::Postgres(error)
            | Self::ConnectionLost(error)
            | Self::Cancel(error) => postgres_connection_hint(error).unwrap_or(GENERIC_CONNECTION_HINT),
            _ => GENERIC_CONNECTION_HINT,
        }
    }
}

fn postgres_connection_hint(error: &PostgresError) -> Option<&'static str> {
    match error.code() {
        Some(&SqlState::INVALID_PASSWORD | &SqlState::INVALID_AUTHORIZATION_SPECIFICATION) => {
            Some(
                "Check the PostgreSQL user name and password, and confirm the server access rules allow this connection.",
            )
        }
        Some(&SqlState::INVALID_CATALOG_NAME) => {
            Some("Check that the requested PostgreSQL database exists and the user may access it.")
        }
        Some(&SqlState::INSUFFICIENT_PRIVILEGE) => Some(
            "Check that the PostgreSQL user has permission to access this database and its catalog.",
        ),
        _ if postgres_io_error(error).is_some() => Some(
            "Check the host, port, network path, and whether PostgreSQL is accepting connections.",
        ),
        _ => None,
    }
}

fn postgres_io_error(error: &PostgresError) -> Option<&std::io::Error> {
    let mut source = error.source();
    while let Some(current) = source {
        if let Some(io_error) = current.downcast_ref::<std::io::Error>() {
            return Some(io_error);
        }
        source = current.source();
    }
    None
}
