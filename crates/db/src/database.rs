use std::{future::Future, str::FromStr, sync::Arc};

use crate::{
    cancellation::CancellationRegistry,
    client::PooledClient,
    error::DatabaseError,
    profile::ConnectionProfile,
    tls::{build_tls_connector, effective_ssl_mode, strip_tls_params, TlsChoice},
};
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod, Runtime};
use parking_lot::RwLock;
use tokio_postgres::{error::SqlState, Config as PgConfig, NoTls};

mod copy;
mod statements;

const DEFAULT_POOL_SIZE: usize = 8;

#[derive(Debug, Clone, Default)]
pub struct Database {
    connection: Arc<RwLock<Option<ConnectionResources>>>,
    profile: Arc<RwLock<Option<ConnectionProfile>>>,
    cancellation: CancellationRegistry,
}

#[derive(Debug, Clone)]
struct ConnectionResources {
    pool: Pool,
    tls: TlsChoice,
}

struct PreparedConnection {
    config: PgConfig,
    tls: TlsChoice,
}

/// Validates a `PostgreSQL` connection URI and its referenced TLS configuration without
/// attempting a network connection.
///
/// # Errors
/// Returns [`DatabaseError`] when the URI cannot be parsed or its TLS files/configuration
/// cannot be used.
pub fn validate_connection_uri(uri: &str) -> Result<(), DatabaseError> {
    prepare_connection(uri, None, None).map(|_| ())
}

impl Database {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Establishes a connection pool for the provided profile and performs an initial
    /// healthcheck before storing it in the shared state.
    ///
    /// # Errors
    /// Returns [`DatabaseError`] if the pool cannot be created or the healthcheck fails.
    pub async fn connect(&self, profile: &ConnectionProfile) -> Result<(), DatabaseError> {
        let connection = Self::create_connection(profile)?;
        Self::healthcheck(&connection.pool).await?;
        self.set_connection(connection);
        self.store_profile(profile);
        Ok(())
    }

    /// Executes a healthcheck against the active connection pool to ensure connectivity.
    ///
    /// # Errors
    /// Returns [`DatabaseError`] if the pool is unavailable or the healthcheck query fails.
    pub async fn ping(&self) -> Result<(), DatabaseError> {
        let pool = self.pool()?;
        Self::healthcheck(&pool).await?;
        Ok(())
    }

    /// Cache the active profile for future reconnect attempts.
    fn store_profile(&self, profile: &ConnectionProfile) {
        let mut stored = self.profile.write();
        *stored = Some(profile.clone());
    }

    /// Replace the shared pool and its matching TLS cancellation connector.
    fn set_connection(&self, connection: ConnectionResources) {
        let mut slot = self.connection.write();
        *slot = Some(connection);
    }

    /// Acquires a single connection from the pool, wrapping it in a [`PooledClient`].
    ///
    /// # Errors
    /// Returns [`DatabaseError`] if the pool is unavailable or a client cannot be acquired.
    pub async fn acquire(&self) -> Result<PooledClient, DatabaseError> {
        let mut attempts = 0;
        loop {
            let pool = self.pool()?;
            let client = pool.get().await.map_err(DatabaseError::Acquire)?;
            if attempts == 0 && client.is_closed() {
                drop(pool);
                self.reconnect().await?;
                attempts += 1;
                continue;
            }
            return Ok(PooledClient::new(client));
        }
    }

    /// Executes an asynchronous operation with a pooled client, returning its result.
    ///
    /// # Errors
    /// Returns [`DatabaseError`] if the pool is unavailable, a client cannot be acquired,
    /// or the provided operation fails.
    pub async fn with_client<F, Fut, T>(&self, operation: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(PooledClient) -> Fut,
        Fut: Future<Output = Result<T, tokio_postgres::Error>>,
    {
        let cancellation = self.cancellation.begin();
        let connection = self.connection()?;
        let client = connection
            .pool
            .get()
            .await
            .map_err(DatabaseError::Acquire)?;
        if !cancellation.activate(client.cancel_token(), connection.tls) {
            // Clear the canceled generation before the raw client can return to the pool.
            cancellation.retire();
            return Err(DatabaseError::OperationCanceled);
        }
        let pooled = PooledClient::with_cancellation(client, cancellation);

        match operation(pooled).await {
            Ok(value) => Ok(value),
            Err(error) if Self::should_reconnect(&error) => {
                // The server may have completed a dispatched operation before the connection
                // failed. Return its result immediately and let the pool recycle the closed
                // client for a later request; never replay this closure.
                Err(DatabaseError::ConnectionLost(error))
            }
            Err(error) => Err(DatabaseError::Postgres(error)),
        }
    }

    /// Sends `PostgreSQL`'s out-of-band cancellation request for the active pooled operation.
    ///
    /// Returns `false` if no operation currently owns a pooled client.
    ///
    /// # Errors
    /// Returns [`DatabaseError`] if a cancellation connection cannot be established.
    pub async fn cancel_current(&self) -> Result<bool, DatabaseError> {
        self.cancellation.cancel_current().await
    }

    fn create_connection(
        profile: &ConnectionProfile,
    ) -> Result<ConnectionResources, DatabaseError> {
        let PreparedConnection { config, tls } =
            prepare_connection(&profile.uri, Some(&profile.name), profile.connect_timeout)?;

        let manager = match tls.clone() {
            TlsChoice::NoTls => Manager::from_config(
                config,
                NoTls,
                ManagerConfig {
                    recycling_method: RecyclingMethod::Fast,
                },
            ),
            TlsChoice::Rustls(connector) => Manager::from_config(
                config,
                connector,
                ManagerConfig {
                    recycling_method: RecyclingMethod::Fast,
                },
            ),
        };
        let pool = Pool::builder(manager)
            .max_size(profile.max_pool_size.unwrap_or(DEFAULT_POOL_SIZE))
            .runtime(Runtime::Tokio1)
            .build()
            .map_err(DatabaseError::PoolBuild)?;
        Ok(ConnectionResources { pool, tls })
    }

    async fn healthcheck(pool: &Pool) -> Result<(), DatabaseError> {
        let client = pool.get().await.map_err(DatabaseError::Acquire)?;
        client
            .batch_execute("SELECT 1")
            .await
            .map_err(DatabaseError::Postgres)?;
        Ok(())
    }

    fn pool(&self) -> Result<Pool, DatabaseError> {
        Ok(self.connection()?.pool)
    }

    fn connection(&self) -> Result<ConnectionResources, DatabaseError> {
        self.connection
            .read()
            .clone()
            .ok_or(DatabaseError::PoolUninitialized)
    }

    fn profile(&self) -> Result<ConnectionProfile, DatabaseError> {
        self.profile
            .read()
            .clone()
            .ok_or(DatabaseError::MissingProfile)
    }

    async fn reconnect(&self) -> Result<(), DatabaseError> {
        let profile = self.profile()?;
        let connection = Self::create_connection(&profile)?;
        Self::healthcheck(&connection.pool).await?;
        self.set_connection(connection);
        Ok(())
    }

    fn should_reconnect(error: &tokio_postgres::Error) -> bool {
        if error.is_closed() {
            return true;
        }

        error.code().is_some_and(|code| {
            matches!(
                code,
                &SqlState::ADMIN_SHUTDOWN
                    | &SqlState::CRASH_SHUTDOWN
                    | &SqlState::CONNECTION_EXCEPTION
                    | &SqlState::CONNECTION_FAILURE
                    | &SqlState::CONNECTION_DOES_NOT_EXIST
            )
        })
    }
}

fn prepare_connection(
    uri: &str,
    application_name: Option<&str>,
    connect_timeout: Option<std::time::Duration>,
) -> Result<PreparedConnection, DatabaseError> {
    let (sanitized_uri, tls_files, url_ssl_mode) = strip_tls_params(uri)?;
    let config_input = match url_ssl_mode {
        Some(_) => sanitized_uri,
        None => format!("sslmode=require {sanitized_uri}"),
    };
    let mut config = PgConfig::from_str(&config_input).map_err(DatabaseError::InvalidUri)?;
    if let Some(application_name) = application_name {
        config.application_name(application_name);
    }
    if let Some(connect_timeout) = connect_timeout {
        config.connect_timeout(connect_timeout);
    }

    let ssl_mode = effective_ssl_mode(url_ssl_mode.unwrap_or(config.get_ssl_mode()), &tls_files)?;
    config.ssl_mode(ssl_mode);
    let tls = build_tls_connector(ssl_mode, &tls_files)?;

    Ok(PreparedConnection { config, tls })
}
