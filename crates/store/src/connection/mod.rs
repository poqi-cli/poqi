use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row, TransactionBehavior};

use crate::{encryption, store::Store};

const SELECT_FIELDS: &str =
    "SELECT id, name, uri_nonce, uri_ciphertext, created_at, updated_at FROM connection_profiles";

#[derive(Debug, Clone)]
pub struct NewConnectionProfile {
    pub name: String,
    pub uri: String,
}

impl NewConnectionProfile {
    #[must_use]
    pub fn new(name: impl Into<String>, uri: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            uri: uri.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StoredConnectionProfile {
    pub id: i64,
    pub name: String,
    pub uri: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct ConnectionProfileStore<'a> {
    store: &'a Store,
}

impl<'a> ConnectionProfileStore<'a> {
    pub(crate) fn new(store: &'a Store) -> Self {
        Self { store }
    }

    /// Returns all stored connection profiles ordered by name.
    ///
    /// # Errors
    /// Returns an error if the backing store cannot be queried or decrypted.
    pub fn list(&self) -> Result<Vec<StoredConnectionProfile>> {
        let connection = self.store.open_connection()?;
        let encrypted = list_encrypted(&connection)?;
        if encrypted.is_empty() {
            return Ok(Vec::new());
        }
        let context = encryption::read_context(self.store, &connection)?;
        encrypted
            .into_iter()
            .map(|profile| profile.decrypt(&context))
            .collect()
    }

    /// Fetches a stored connection profile by its unique name.
    ///
    /// # Errors
    /// Returns an error if the backing store cannot be queried or decrypted.
    pub fn get(&self, name: &str) -> Result<Option<StoredConnectionProfile>> {
        let connection = self.store.open_connection()?;
        let Some(encrypted) = fetch_by_name(&connection, name)? else {
            return Ok(None);
        };
        let context = encryption::read_context(self.store, &connection)?;
        encrypted.decrypt(&context).map(Some)
    }

    /// Creates an encrypted profile without replacing an existing name.
    ///
    /// # Errors
    /// Returns an error if the name exists, secure storage is unavailable, or the write fails.
    pub fn insert(&self, profile: &NewConnectionProfile) -> Result<StoredConnectionProfile> {
        self.persist(profile, false)
    }

    /// Inserts or updates an encrypted connection profile.
    ///
    /// # Errors
    /// Returns an error if secure storage is unavailable or the write fails.
    pub fn upsert(&self, profile: &NewConnectionProfile) -> Result<StoredConnectionProfile> {
        self.persist(profile, true)
    }

    /// Deletes a stored connection profile.
    ///
    /// # Errors
    /// Returns an error if the backing store rejects the delete operation.
    pub fn delete(&self, name: &str) -> Result<bool> {
        let connection = self.store.open_connection()?;
        let affected = connection
            .execute(
                "DELETE FROM connection_profiles WHERE name = ?1",
                params![name],
            )
            .context("failed to delete connection profile")?;
        Ok(affected > 0)
    }

    fn persist(
        &self,
        profile: &NewConnectionProfile,
        replace: bool,
    ) -> Result<StoredConnectionProfile> {
        let mut connection = self.store.open_connection()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .context("failed to start encrypted connection write")?;
        let context = encryption::write_context(self.store, &transaction)?;
        let encrypted = context.encrypt_profile(&profile.name, &profile.uri)?;
        let affected = write_profile(&transaction, profile, &encrypted, replace)?;
        if affected != 1 {
            anyhow::bail!("A connection profile with this name already exists");
        }
        let stored = fetch_by_name(&transaction, &profile.name)?
            .context("persisted connection profile could not be read")?
            .decrypt(&context)?;
        transaction
            .commit()
            .context("failed to commit encrypted connection write")?;
        Ok(stored)
    }
}

fn write_profile(
    connection: &Connection,
    profile: &NewConnectionProfile,
    encrypted: &crate::crypto::EncryptedValue,
    replace: bool,
) -> Result<usize> {
    let now = Utc::now();
    let conflict = if replace {
        "DO UPDATE SET uri = '', uri_nonce = excluded.uri_nonce,
         uri_ciphertext = excluded.uri_ciphertext, updated_at = excluded.updated_at"
    } else {
        "DO NOTHING"
    };
    connection
        .execute(
            &format!(
                "INSERT INTO connection_profiles
                 (name, uri, uri_nonce, uri_ciphertext, created_at, updated_at)
                 VALUES (?1, '', ?2, ?3, ?4, ?4) ON CONFLICT(name) {conflict}"
            ),
            params![profile.name, encrypted.nonce, encrypted.ciphertext, now],
        )
        .context("failed to persist encrypted connection profile")
}

fn list_encrypted(connection: &Connection) -> Result<Vec<EncryptedProfile>> {
    let mut statement = connection
        .prepare(&format!("{SELECT_FIELDS} ORDER BY name"))
        .context("failed to prepare connection profile query")?;
    let rows = statement
        .query_map([], map_encrypted_profile)
        .context("failed to iterate connection profiles")?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to read connection profile rows")
}

fn fetch_by_name(connection: &Connection, name: &str) -> Result<Option<EncryptedProfile>> {
    let mut statement = connection
        .prepare(&format!("{SELECT_FIELDS} WHERE name = ?1 LIMIT 1"))
        .context("failed to prepare lookup query")?;
    statement
        .query_row(params![name], map_encrypted_profile)
        .optional()
        .context("failed to fetch connection profile")
}

struct EncryptedProfile {
    id: i64,
    name: String,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl EncryptedProfile {
    fn decrypt(self, context: &encryption::EncryptionContext) -> Result<StoredConnectionProfile> {
        let uri = context.decrypt_profile(&self.name, &self.nonce, &self.ciphertext)?;
        Ok(StoredConnectionProfile {
            id: self.id,
            name: self.name,
            uri,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

fn map_encrypted_profile(row: &Row<'_>) -> rusqlite::Result<EncryptedProfile> {
    Ok(EncryptedProfile {
        id: row.get("id")?,
        name: row.get("name")?,
        nonce: row.get("uri_nonce")?,
        ciphertext: row.get("uri_ciphertext")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

#[cfg(test)]
mod tests;
