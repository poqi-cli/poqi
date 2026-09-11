use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use uuid::Uuid;

use crate::{
    crypto::{self, EncryptedValue},
    settings::KEY_DB_PRIMARY,
    Store,
};

const STORE_ID_KEY: &str = "store.id";
const ENCRYPTION_VERSION_KEY: &str = "encryption.version";
const ENCRYPTION_VERSION: &str = "1";
const KEY_INITIALIZED_KEY: &str = "encryption.key_initialized";
const AAD_VERSION: &str = "poqi-store-v1";

pub(crate) struct EncryptionContext {
    store_id: String,
    key: [u8; 32],
}

impl EncryptionContext {
    pub(crate) fn encrypt_profile(&self, name: &str, uri: &str) -> Result<EncryptedValue> {
        crypto::encrypt(
            &self.key,
            uri.as_bytes(),
            &profile_aad(&self.store_id, name),
        )
    }

    pub(crate) fn decrypt_profile(
        &self,
        name: &str,
        nonce: &[u8],
        ciphertext: &[u8],
    ) -> Result<String> {
        decrypt_text(
            &self.key,
            nonce,
            ciphertext,
            &profile_aad(&self.store_id, name),
        )
    }

    pub(crate) fn encrypt_setting(&self, key: &str, value: &str) -> Result<EncryptedValue> {
        crypto::encrypt(
            &self.key,
            value.as_bytes(),
            &setting_aad(&self.store_id, key),
        )
    }

    pub(crate) fn decrypt_setting(
        &self,
        key: &str,
        nonce: &[u8],
        ciphertext: &[u8],
    ) -> Result<String> {
        decrypt_text(
            &self.key,
            nonce,
            ciphertext,
            &setting_aad(&self.store_id, key),
        )
    }
}

pub(crate) fn initialize(store: &Store, connection: &mut Connection) -> Result<()> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .context("failed to start encrypted store validation")?;
    let state = inspect_sensitive_state(&transaction)?;
    let completed = encryption_completed(&transaction)?;
    let key_initialized = metadata_flag(&transaction, KEY_INITIALIZED_KEY)?;
    anyhow::ensure!(
        !state.has_plaintext,
        "unsupported plaintext store data; only encrypted poqi stores are supported"
    );
    anyhow::ensure!(
        completed || !state.has_encrypted,
        "connection encryption metadata is missing from an encrypted store"
    );
    anyhow::ensure!(
        !state.has_encrypted || key_initialized,
        "connection encryption key metadata is missing from an encrypted store"
    );
    let store_id = ensure_store_id(&transaction)?;
    if !state.has_sensitive_data() {
        mark_encryption_complete(&transaction)?;
        transaction
            .commit()
            .context("failed to save store identity")?;
        return Ok(());
    }

    let key = store.keys.load(&store_id)?.context(
        "saved connections cannot be decrypted because their native secure-storage key is missing",
    )?;
    let context = EncryptionContext { store_id, key };
    validate_encrypted_rows(&transaction, &context)?;
    transaction
        .commit()
        .context("failed to finish encrypted store validation")
}

pub(crate) fn write_context(
    store: &Store,
    transaction: &Transaction<'_>,
) -> Result<EncryptionContext> {
    let store_id = ensure_store_id(transaction)?;
    let key = if let Some(key) = store.keys.load(&store_id)? {
        key
    } else {
        anyhow::ensure!(
            !inspect_sensitive_state(transaction)?.has_sensitive_data(),
            "saved connections cannot be encrypted because their native secure-storage key is missing"
        );
        store.keys.load_or_create(&store_id)?
    };
    set_metadata_flag(transaction, KEY_INITIALIZED_KEY)?;
    Ok(EncryptionContext { store_id, key })
}

pub(crate) fn read_context(store: &Store, connection: &Connection) -> Result<EncryptionContext> {
    let store_id = read_store_id(connection)?;
    let key = store.keys.load(&store_id)?.context(
        "saved connections cannot be decrypted because their native secure-storage key is missing",
    )?;
    Ok(EncryptionContext { store_id, key })
}

fn inspect_sensitive_state(transaction: &Transaction<'_>) -> Result<SensitiveState> {
    let profile_state = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM connection_profiles WHERE uri_nonce IS NULL AND uri_ciphertext IS NULL),
                    EXISTS(SELECT 1 FROM connection_profiles WHERE uri_nonce IS NOT NULL OR uri_ciphertext IS NOT NULL)",
            [],
            |row| Ok((row.get::<_, bool>(0)?, row.get::<_, bool>(1)?)),
        )
        .context("failed to inspect connection encryption state")?;
    let primary_state = transaction
        .query_row(
            "SELECT value, value_nonce, value_ciphertext FROM settings WHERE key = ?1",
            params![KEY_DB_PRIMARY],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<Vec<u8>>>(1)?,
                    row.get::<_, Option<Vec<u8>>>(2)?,
                ))
            },
        )
        .optional()
        .context("failed to inspect primary connection encryption state")?;
    let (primary_plaintext, primary_encrypted) = match primary_state {
        None => (false, false),
        Some((value, None, None)) if value == "null" => (false, false),
        Some((_, None, None)) => (true, false),
        Some((_, Some(_), Some(_))) => (false, true),
        Some(_) => anyhow::bail!("stored primary connection encryption envelope is incomplete"),
    };
    Ok(SensitiveState {
        has_plaintext: profile_state.0 || primary_plaintext,
        has_encrypted: profile_state.1 || primary_encrypted,
    })
}

struct SensitiveState {
    has_plaintext: bool,
    has_encrypted: bool,
}

impl SensitiveState {
    fn has_sensitive_data(&self) -> bool {
        self.has_plaintext || self.has_encrypted
    }
}

fn validate_encrypted_rows(
    transaction: &Transaction<'_>,
    context: &EncryptionContext,
) -> Result<()> {
    let mut statement = transaction
        .prepare(
            "SELECT name, uri, uri_nonce, uri_ciphertext FROM connection_profiles
             WHERE uri_nonce IS NOT NULL OR uri_ciphertext IS NOT NULL",
        )
        .context("failed to prepare encrypted connection validation")?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<Vec<u8>>>(2)?,
                row.get::<_, Option<Vec<u8>>>(3)?,
            ))
        })
        .context("failed to query encrypted connections")?;
    for row in rows {
        let (name, reserved_plaintext, nonce, ciphertext) =
            row.context("failed to read encrypted connection")?;
        anyhow::ensure!(
            reserved_plaintext.is_empty(),
            "encrypted connection retained plaintext data"
        );
        let (Some(nonce), Some(ciphertext)) = (nonce, ciphertext) else {
            anyhow::bail!("stored connection encryption envelope is incomplete");
        };
        context.decrypt_profile(&name, &nonce, &ciphertext)?;
    }
    validate_encrypted_primary(transaction, context)
}

fn validate_encrypted_primary(
    transaction: &Transaction<'_>,
    context: &EncryptionContext,
) -> Result<()> {
    let row = transaction
        .query_row(
            "SELECT value, value_nonce, value_ciphertext FROM settings
             WHERE key = ?1 AND (value_nonce IS NOT NULL OR value_ciphertext IS NOT NULL)",
            params![KEY_DB_PRIMARY],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<Vec<u8>>>(1)?,
                    row.get::<_, Option<Vec<u8>>>(2)?,
                ))
            },
        )
        .optional()
        .context("failed to read encrypted primary connection")?;
    if let Some((reserved_plaintext, nonce, ciphertext)) = row {
        anyhow::ensure!(
            reserved_plaintext.is_empty(),
            "encrypted primary connection retained plaintext data"
        );
        let (Some(nonce), Some(ciphertext)) = (nonce, ciphertext) else {
            anyhow::bail!("stored primary connection encryption envelope is incomplete");
        };
        context.decrypt_setting(KEY_DB_PRIMARY, &nonce, &ciphertext)?;
    }
    Ok(())
}

fn ensure_store_id(connection: &Connection) -> Result<String> {
    let existing: Option<String> = connection
        .query_row(
            "SELECT value FROM store_metadata WHERE key = ?1",
            params![STORE_ID_KEY],
            |row| row.get(0),
        )
        .optional()
        .context("failed to read store identity")?;
    if let Some(store_id) = existing {
        return Ok(store_id);
    }
    let store_id = Uuid::new_v4().to_string();
    connection
        .execute(
            "INSERT OR IGNORE INTO store_metadata (key, value) VALUES (?1, ?2)",
            params![STORE_ID_KEY, store_id],
        )
        .context("failed to create store identity")?;
    read_store_id(connection)
}

fn encryption_completed(connection: &Connection) -> Result<bool> {
    let version: Option<String> = connection
        .query_row(
            "SELECT value FROM store_metadata WHERE key = ?1",
            params![ENCRYPTION_VERSION_KEY],
            |row| row.get(0),
        )
        .optional()
        .context("failed to read connection encryption version")?;
    match version.as_deref() {
        None => Ok(false),
        Some(ENCRYPTION_VERSION) => Ok(true),
        Some(version) => anyhow::bail!("unsupported connection encryption version {version}"),
    }
}

fn mark_encryption_complete(connection: &Connection) -> Result<()> {
    connection
        .execute(
            "INSERT INTO store_metadata (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![ENCRYPTION_VERSION_KEY, ENCRYPTION_VERSION],
        )
        .context("failed to save connection encryption version")?;
    Ok(())
}

fn read_store_id(connection: &Connection) -> Result<String> {
    connection
        .query_row(
            "SELECT value FROM store_metadata WHERE key = ?1",
            params![STORE_ID_KEY],
            |row| row.get(0),
        )
        .context("store identity is missing")
}

fn metadata_flag(connection: &Connection, key: &str) -> Result<bool> {
    connection
        .query_row(
            "SELECT value = '1' FROM store_metadata WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .context("failed to read encrypted store metadata")
        .map(|value| value.unwrap_or(false))
}

fn set_metadata_flag(connection: &Connection, key: &str) -> Result<()> {
    connection
        .execute(
            "INSERT INTO store_metadata (key, value) VALUES (?1, '1')
             ON CONFLICT(key) DO UPDATE SET value = '1'",
            params![key],
        )
        .context("failed to update encrypted store metadata")?;
    Ok(())
}

fn profile_aad(store_id: &str, name: &str) -> Vec<u8> {
    format!("{AAD_VERSION}\0{store_id}\0profile\0{name}").into_bytes()
}

fn setting_aad(store_id: &str, key: &str) -> Vec<u8> {
    format!("{AAD_VERSION}\0{store_id}\0setting\0{key}").into_bytes()
}

fn decrypt_text(key: &[u8; 32], nonce: &[u8], ciphertext: &[u8], aad: &[u8]) -> Result<String> {
    let plaintext = crypto::decrypt(key, nonce, ciphertext, aad)?;
    String::from_utf8(plaintext).context("decrypted connection data is not valid UTF-8")
}
