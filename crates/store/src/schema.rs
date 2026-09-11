use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

const CURRENT_SCHEMA_VERSION: i64 = 3;

const CURRENT_SCHEMA: &str = r"
    CREATE TABLE connection_profiles (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL UNIQUE,
        uri TEXT NOT NULL,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        uri_nonce BLOB,
        uri_ciphertext BLOB
    );
    CREATE INDEX idx_connection_profiles_name
        ON connection_profiles(name);

    CREATE TABLE settings (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        value_nonce BLOB,
        value_ciphertext BLOB
    );
    CREATE INDEX idx_settings_updated_at
        ON settings(updated_at);

    CREATE TABLE store_metadata (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
";

pub(crate) fn initialize(connection: &mut Connection) -> Result<()> {
    match schema_version(connection)? {
        CURRENT_SCHEMA_VERSION => return Ok(()),
        0 => {}
        version => anyhow::bail!(
            "unsupported store schema version {version}; expected {CURRENT_SCHEMA_VERSION}"
        ),
    }

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .context("failed to start store schema initialization")?;
    match schema_version(&transaction)? {
        CURRENT_SCHEMA_VERSION => {
            return transaction
                .commit()
                .context("failed to finish schema initialization")
        }
        0 if schema_is_empty(&transaction)? => {}
        version => anyhow::bail!(
            "unsupported store schema version {version}; expected {CURRENT_SCHEMA_VERSION}"
        ),
    }
    transaction
        .execute_batch(CURRENT_SCHEMA)
        .context("failed to create current store schema")?;
    transaction
        .pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)
        .context("failed to set current store schema version")?;
    transaction
        .commit()
        .context("failed to commit store schema initialization")
}

fn schema_version(connection: &Connection) -> Result<i64> {
    connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .context("failed to read store schema version")
}

fn schema_is_empty(connection: &Connection) -> Result<bool> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_schema
             WHERE name NOT LIKE 'sqlite_%'
             LIMIT 1",
            [],
            |_| Ok(()),
        )
        .optional()
        .context("failed to inspect store schema")
        .map(|object| object.is_none())
}
