use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use rusqlite::Connection;

use crate::{
    connection::ConnectionProfileStore, crypto::KeyManager, encryption, schema,
    settings::SettingsStore,
};

const STORE_DIRECTORY: &str = "store";
const STORE_FILE_NAME: &str = "poqi.sqlite";

#[derive(Debug, Clone)]
pub struct Store {
    pub path: Option<PathBuf>,
    pub(crate) keys: Arc<KeyManager>,
}

impl Default for Store {
    fn default() -> Self {
        Self::new(None)
    }
}

impl Store {
    #[must_use]
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            path,
            keys: Arc::new(KeyManager::native()),
        }
    }

    /// Creates a store backed by a deterministic test encryption key.
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn new_with_test_key(path: Option<PathBuf>, key: [u8; 32]) -> Self {
        Self {
            path,
            keys: Arc::new(KeyManager::fixed(key)),
        }
    }

    /// Creates a store whose test secure-storage provider always fails.
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn new_with_unavailable_test_key(path: Option<PathBuf>) -> Self {
        Self {
            path,
            keys: Arc::new(KeyManager::unavailable()),
        }
    }

    /// Creates a store whose test secure-storage provider has no existing key.
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn new_with_missing_test_key(path: Option<PathBuf>) -> Self {
        Self {
            path,
            keys: Arc::new(KeyManager::missing()),
        }
    }

    /// Initialize the backing store, creating any required on-disk resources.
    ///
    /// # Errors
    /// Returns an error if the store cannot be created or has an unsupported schema.
    pub fn init(&self) -> Result<()> {
        let _ = self.open_connection()?;
        Ok(())
    }

    /// Access the connection profile repository.
    #[must_use]
    pub fn connection_profiles(&self) -> ConnectionProfileStore<'_> {
        ConnectionProfileStore::new(self)
    }

    /// Access the settings repository.
    #[must_use]
    pub fn settings(&self) -> SettingsStore<'_> {
        SettingsStore::new(self)
    }

    pub(crate) fn open_connection(&self) -> Result<Connection> {
        let path = self.resolve_path()?;
        if let Some(parent) = path.parent() {
            create_dir_all(parent)?;
        }
        let mut connection = Connection::open(&path)
            .with_context(|| format!("failed to open store at {}", path.display()))?;
        schema::initialize(&mut connection).context("failed to initialize store schema")?;
        encryption::initialize(self, &mut connection)
            .context("failed to initialize encrypted connection storage")?;
        Ok(connection)
    }

    fn resolve_path(&self) -> Result<PathBuf> {
        if let Some(path) = &self.path {
            return Ok(path.clone());
        }
        let dirs = ProjectDirs::from("com", "poqi", "poqi")
            .context("could not determine data directory for store")?;
        Ok(dirs
            .data_local_dir()
            .join(STORE_DIRECTORY)
            .join(STORE_FILE_NAME))
    }
}

fn create_dir_all(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .with_context(|| format!("failed to create store directory {}", path.display()))
}
