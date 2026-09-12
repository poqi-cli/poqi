use std::fs;

use rusqlite::Connection;
use tempfile::TempDir;

use super::{
    settings::{KEY_DB_PRIMARY, KEY_UI_THEME},
    NewConnectionProfile, Store,
};
use crate::test_support::{TestStore, TEST_KEY};

fn test_store(path: std::path::PathBuf) -> Store {
    Store::new_with_test_key(Some(path), TEST_KEY)
}

#[test]
fn settings_round_trip_theme() {
    let harness = TestStore::new();
    harness.store().init().expect("init store");
    let settings = harness.store().settings();
    assert!(settings.theme().expect("read setting").is_none());

    settings.set_theme("dark").expect("set theme");
    let theme = settings.theme().expect("read theme");
    assert_eq!(theme.as_deref(), Some("dark"));
}

#[test]
fn settings_update_overwrites_value() {
    let harness = TestStore::new();
    harness.store().init().expect("init store");
    let settings = harness.store().settings();

    settings
        .set_fast_scroll_step(4)
        .expect("initial fast scroll");
    settings
        .set_fast_scroll_step(12)
        .expect("updated fast scroll");

    let step = settings
        .fast_scroll_step()
        .expect("fetch fast scroll")
        .expect("value should exist");
    assert_eq!(step, 12);
}

#[test]
fn optional_threshold_round_trip() {
    let harness = TestStore::new();
    harness.store().init().expect("init store");
    let settings = harness.store().settings();

    assert!(settings
        .semantic_score_threshold()
        .expect("initial read")
        .is_none());

    settings
        .set_semantic_score_threshold(Some(0.42))
        .expect("set threshold");
    assert_eq!(
        settings.semantic_score_threshold().expect("read threshold"),
        Some(0.42)
    );

    settings
        .set_semantic_score_threshold(None)
        .expect("clear threshold");
    assert!(settings
        .semantic_score_threshold()
        .expect("read cleared threshold")
        .is_none());
}

#[test]
fn ensure_setting_preserves_existing_value() {
    let harness = TestStore::new();
    harness.store().init().expect("init store");
    let settings = harness.store().settings();

    settings
        .ensure_setting(KEY_UI_THEME, &"dark")
        .expect("seed theme");
    settings
        .ensure_setting(KEY_UI_THEME, &"light")
        .expect("ensure should skip existing value");

    let theme = settings.theme().expect("fetch theme");
    assert_eq!(theme.as_deref(), Some("dark"));
}

#[test]
fn blank_database_initializes_directly_at_current_encrypted_schema() {
    let harness = TestStore::new();
    harness.store().init().expect("initialize store");
    let raw = Connection::open(harness.db_path()).expect("open initialized store");

    let version: i64 = raw
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version");
    assert_eq!(version, 3);
    let columns: Vec<String> = raw
        .prepare("SELECT name FROM pragma_table_info('connection_profiles') ORDER BY cid")
        .expect("prepare column query")
        .query_map([], |row| row.get(0))
        .expect("query columns")
        .collect::<rusqlite::Result<_>>()
        .expect("read columns");
    assert_eq!(
        columns,
        [
            "id",
            "name",
            "uri",
            "created_at",
            "updated_at",
            "uri_nonce",
            "uri_ciphertext"
        ]
    );
    let encryption_version: String = raw
        .query_row(
            "SELECT value FROM store_metadata WHERE key = 'encryption.version'",
            [],
            |row| row.get(0),
        )
        .expect("read encryption version");
    assert_eq!(encryption_version, "1");
}

#[test]
fn concurrent_blank_store_initialization_is_atomic() {
    use std::sync::{Arc, Barrier};

    let temp = TempDir::new().expect("temp store directory");
    let path = temp.path().join("concurrent.sqlite");
    let barrier = Arc::new(Barrier::new(3));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let store = test_store(path.clone());
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                store.init()
            })
        })
        .collect();
    barrier.wait();
    for handle in handles {
        handle
            .join()
            .expect("initializer panicked")
            .expect("initialize shared store");
    }

    let raw = Connection::open(path).expect("open initialized store");
    let version: i64 = raw
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read schema version");
    assert_eq!(version, 3);
    let identities: i64 = raw
        .query_row(
            "SELECT count(*) FROM store_metadata WHERE key = 'store.id'",
            [],
            |row| row.get(0),
        )
        .expect("count store identities");
    assert_eq!(identities, 1);
}

#[test]
fn version_zero_nonempty_schema_is_rejected_without_mutation() {
    let temp = TempDir::new().expect("temp store directory");
    let path = temp.path().join("nonempty-v0.sqlite");
    let raw = Connection::open(&path).expect("open nonempty store");
    raw.execute_batch(
        "CREATE TABLE retained_data (
            id INTEGER PRIMARY KEY,
            value TEXT NOT NULL
         );
         INSERT INTO retained_data (id, value) VALUES (1, 'keep me');",
    )
    .expect("create unrelated version-zero schema");
    let schema_before: String = raw
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'retained_data'",
            [],
            |row| row.get(0),
        )
        .expect("read original schema");
    drop(raw);

    let error = test_store(path.clone())
        .init()
        .expect_err("nonempty version-zero schema must be rejected");
    assert!(format!("{error:#}").contains("unsupported store schema version 0"));

    let unchanged = Connection::open(path).expect("reopen rejected store");
    let stored_version: i64 = unchanged
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("read retained schema version");
    assert_eq!(stored_version, 0);
    let schema_after: String = unchanged
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'retained_data'",
            [],
            |row| row.get(0),
        )
        .expect("read retained schema");
    assert_eq!(schema_after, schema_before);
    let value: String = unchanged
        .query_row("SELECT value FROM retained_data WHERE id = 1", [], |row| {
            row.get(0)
        })
        .expect("read retained data");
    assert_eq!(value, "keep me");
    let store_objects: i64 = unchanged
        .query_row(
            "SELECT count(*) FROM sqlite_schema
             WHERE name IN ('connection_profiles', 'settings', 'store_metadata')",
            [],
            |row| row.get(0),
        )
        .expect("inspect rejected schema");
    assert_eq!(store_objects, 0);
}

#[test]
fn old_plaintext_schema_versions_are_rejected_without_mutation() {
    for version in [1_i64, 2] {
        let temp = TempDir::new().expect("temp store directory");
        let path = temp.path().join(format!("v{version}.sqlite"));
        let raw = Connection::open(&path).expect("open old store");
        raw.execute_batch(
            "CREATE TABLE connection_profiles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                uri TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
             );
             INSERT INTO connection_profiles (name, uri, created_at, updated_at)
             VALUES ('old', 'postgres://user:plaintext@localhost/app', 'now', 'now');",
        )
        .expect("create old plaintext schema");
        if version == 2 {
            raw.execute_batch(
                "CREATE TABLE settings (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                 );",
            )
            .expect("create old settings schema");
        }
        raw.pragma_update(None, "user_version", version)
            .expect("set old schema version");
        drop(raw);

        let error = test_store(path.clone())
            .init()
            .expect_err("old plaintext schema must be rejected");
        assert!(
            format!("{error:#}").contains(&format!("unsupported store schema version {version}"))
        );

        let unchanged = Connection::open(&path).expect("reopen rejected store");
        let uri: String = unchanged
            .query_row("SELECT uri FROM connection_profiles", [], |row| row.get(0))
            .expect("read retained plaintext");
        assert_eq!(uri, "postgres://user:plaintext@localhost/app");
        let stored_version: i64 = unchanged
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("read retained schema version");
        assert_eq!(stored_version, version);
        let metadata_tables: i64 = unchanged
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE type = 'table' AND name = 'store_metadata'",
                [],
                |row| row.get(0),
            )
            .expect("inspect rejected schema");
        assert_eq!(metadata_tables, 0);
    }
}

#[test]
fn plaintext_rows_in_current_schema_are_rejected_without_mutation() {
    let temp = TempDir::new().expect("temp store directory");
    let path = temp.path().join("plaintext-v3.sqlite");
    let mut raw = Connection::open(&path).expect("open current plaintext store");
    crate::schema::initialize(&mut raw).expect("create current schema");
    raw.execute(
        "INSERT INTO connection_profiles (name, uri, created_at, updated_at)
         VALUES ('old', 'postgres://user:plaintext@localhost/app', 'now', 'now')",
        [],
    )
    .expect("insert plaintext row");
    drop(raw);

    let error = test_store(path.clone())
        .init()
        .expect_err("plaintext current-schema row must be rejected");
    assert!(format!("{error:#}").contains("unsupported plaintext store data"));

    let unchanged = Connection::open(path).expect("reopen rejected store");
    let (uri, nonce, ciphertext): (String, Option<Vec<u8>>, Option<Vec<u8>>) = unchanged
        .query_row(
            "SELECT uri, uri_nonce, uri_ciphertext FROM connection_profiles",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read retained plaintext row");
    assert_eq!(uri, "postgres://user:plaintext@localhost/app");
    assert!(nonce.is_none());
    assert!(ciphertext.is_none());
    let metadata_rows: i64 = unchanged
        .query_row("SELECT count(*) FROM store_metadata", [], |row| row.get(0))
        .expect("inspect rejected metadata");
    assert_eq!(metadata_rows, 0);
}

#[test]
fn current_encrypted_store_reopens_with_same_identity_and_aad() {
    let temp = TempDir::new().expect("temp store directory");
    let path = temp.path().join("current.sqlite");
    let uri = "postgres://user:secret@example.test/app";
    test_store(path.clone())
        .connection_profiles()
        .upsert(&NewConnectionProfile::new("primary", uri))
        .expect("write encrypted profile");
    let raw = Connection::open(&path).expect("open encrypted store");
    let before: (String, Vec<u8>) = raw
        .query_row(
            "SELECT (SELECT value FROM store_metadata WHERE key = 'store.id'), uri_ciphertext
             FROM connection_profiles WHERE name = 'primary'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read encrypted identity");
    drop(raw);

    let reopened = test_store(path.clone());
    reopened.init().expect("reopen current encrypted store");
    assert_eq!(
        reopened
            .connection_profiles()
            .get("primary")
            .expect("read reopened profile")
            .expect("profile exists")
            .uri,
        uri
    );
    let after: (String, Vec<u8>) = Connection::open(path)
        .expect("reopen raw store")
        .query_row(
            "SELECT (SELECT value FROM store_metadata WHERE key = 'store.id'), uri_ciphertext
             FROM connection_profiles WHERE name = 'primary'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read preserved encrypted identity");
    assert_eq!(after, before);
}

#[test]
fn primary_setting_is_encrypted_except_literal_null() {
    let harness = TestStore::new();
    let primary = serde_json::json!({
        "uri": "postgres://user:primary-secret@localhost/app",
        "name": "primary"
    });
    harness
        .store()
        .settings()
        .set_setting(KEY_DB_PRIMARY, &primary)
        .expect("store encrypted primary");
    let raw = Connection::open(harness.db_path()).expect("open raw database");
    let (value, nonce, ciphertext): (String, Option<Vec<u8>>, Option<Vec<u8>>) = raw
        .query_row(
            "SELECT value, value_nonce, value_ciphertext FROM settings WHERE key = ?1",
            [KEY_DB_PRIMARY],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read raw primary");
    assert!(value.is_empty());
    assert_eq!(nonce.expect("nonce").len(), 12);
    assert!(ciphertext.is_some());
    assert_eq!(
        harness
            .store()
            .settings()
            .get_setting::<serde_json::Value>(KEY_DB_PRIMARY)
            .expect("decrypt primary"),
        Some(primary)
    );

    harness
        .store()
        .settings()
        .set_setting(KEY_DB_PRIMARY, &serde_json::Value::Null)
        .expect("clear primary");
    let cleared: (String, Option<Vec<u8>>, Option<Vec<u8>>) = raw
        .query_row(
            "SELECT value, value_nonce, value_ciphertext FROM settings WHERE key = ?1",
            [KEY_DB_PRIMARY],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read cleared primary");
    assert_eq!(cleared, ("null".to_string(), None, None));
}

#[test]
fn missing_initialized_key_fails_without_creating_a_replacement() {
    let harness = TestStore::new();
    harness
        .store()
        .connection_profiles()
        .upsert(&NewConnectionProfile::new(
            "primary",
            "postgres://user:secret@localhost/app",
        ))
        .expect("store profile");
    let missing = Store::new_with_missing_test_key(Some(harness.db_path()));
    let error = missing
        .connection_profiles()
        .list()
        .expect_err("missing key must fail");
    let message = format!("{error:#}");
    assert!(message.contains("secure-storage key is missing"));
    assert!(!message.contains("attempted key creation"));
}

#[test]
fn unavailable_secure_storage_fails_closed_without_mutation() {
    let harness = TestStore::new();
    let uri = "postgres://user:secret@localhost/app";
    harness
        .store()
        .connection_profiles()
        .upsert(&NewConnectionProfile::new("primary", uri))
        .expect("store profile");

    let unavailable = Store::new_with_unavailable_test_key(Some(harness.db_path()));
    let error = unavailable
        .connection_profiles()
        .list()
        .expect_err("unavailable secure storage must fail");
    assert!(format!("{error:#}").contains("test secure storage is unavailable"));
    assert_eq!(
        harness
            .store()
            .connection_profiles()
            .get("primary")
            .expect("read unchanged profile")
            .expect("profile exists")
            .uri,
        uri
    );
}

#[test]
fn empty_store_can_attempt_key_creation_after_key_loss() {
    let harness = TestStore::new();
    let profiles = harness.store().connection_profiles();
    profiles
        .upsert(&NewConnectionProfile::new(
            "old",
            "postgres://localhost/old",
        ))
        .expect("store old profile");
    profiles.delete("old").expect("delete old profile");
    let missing = Store::new_with_missing_test_key(Some(harness.db_path()));
    let error = missing
        .connection_profiles()
        .insert(&NewConnectionProfile::new(
            "new",
            "postgres://localhost/new",
        ))
        .expect_err("missing test provider rejects key creation");
    assert!(format!("{error:#}").contains("attempted key creation"));
    assert!(harness
        .store()
        .connection_profiles()
        .list()
        .expect("list empty profiles")
        .is_empty());
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires an unlocked native operating-system credential store"]
async fn native_keyring_round_trip_smoke() {
    struct NativeKeyCleanup(String);
    impl Drop for NativeKeyCleanup {
        fn drop(&mut self) {
            let _ = crate::crypto::delete_native_key_for_test(&self.0);
        }
    }

    let temp = TempDir::new().expect("temp native-keyring store");
    let path = temp.path().join("native-keyring.sqlite");
    let store = Store::new(Some(path.clone()));
    store.init().expect("initialize native-keyring store");
    let store_id: String = Connection::open(&path)
        .expect("open native-keyring database")
        .query_row(
            "SELECT value FROM store_metadata WHERE key = 'store.id'",
            [],
            |row| row.get(0),
        )
        .expect("read store identity");
    let _cleanup = NativeKeyCleanup(store_id.clone());
    let uri = "postgres://native-user:native-smoke-secret@localhost/native";
    store
        .connection_profiles()
        .upsert(&NewConnectionProfile::new("native-smoke", uri))
        .expect("write with native keyring");
    drop(store);

    let reopened = Store::new(Some(path.clone()));
    assert_eq!(
        reopened
            .connection_profiles()
            .get("native-smoke")
            .expect("read with native keyring")
            .expect("native profile exists")
            .uri,
        uri
    );
    let raw_uri: String = Connection::open(&path)
        .expect("open raw native database")
        .query_row("SELECT uri FROM connection_profiles", [], |row| row.get(0))
        .expect("read raw native profile");
    assert!(raw_uri.is_empty());
    let bytes = fs::read(&path).expect("read native-keyring database");
    assert!(!bytes
        .windows(b"native-smoke-secret".len())
        .any(|part| part == b"native-smoke-secret"));

    crate::crypto::delete_native_key_for_test(&store_id)
        .expect("remove key while encrypted data remains");
    let missing = Store::new(Some(path));
    let error = missing
        .connection_profiles()
        .list()
        .expect_err("encrypted store must fail closed when its native key is missing");
    assert!(format!("{error:#}").contains("secure-storage key is missing"));
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires an unlocked native operating-system credential store"]
async fn native_keyring_empty_store_recovers_after_key_loss() {
    struct NativeKeyCleanup(String);
    impl Drop for NativeKeyCleanup {
        fn drop(&mut self) {
            let _ = crate::crypto::delete_native_key_for_test(&self.0);
        }
    }

    let temp = TempDir::new().expect("temp native-keyring store");
    let path = temp.path().join("native-keyring-recovery.sqlite");
    let store = Store::new(Some(path.clone()));
    let uri = "postgres://native-user:native-smoke-secret@localhost/native";
    store
        .connection_profiles()
        .insert(&NewConnectionProfile::new("temporary", uri))
        .expect("create native key");
    let store_id: String = Connection::open(&path)
        .expect("open native-keyring database")
        .query_row(
            "SELECT value FROM store_metadata WHERE key = 'store.id'",
            [],
            |row| row.get(0),
        )
        .expect("read store identity");
    let _cleanup = NativeKeyCleanup(store_id.clone());
    store
        .connection_profiles()
        .delete("temporary")
        .expect("delete final profile");
    drop(store);
    crate::crypto::delete_native_key_for_test(&store_id)
        .expect("simulate lost key for empty store");
    let recovered = Store::new(Some(path.clone()));
    recovered
        .connection_profiles()
        .insert(&NewConnectionProfile::new("recovered", uri))
        .expect("create a new key for an empty store");
    drop(recovered);
    assert_eq!(
        Store::new(Some(path))
            .connection_profiles()
            .get("recovered")
            .expect("decrypt after empty-store recovery")
            .expect("recovered profile")
            .uri,
        uri
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires an intentionally locked Linux Secret Service session"]
fn linux_locked_keyring_fails_closed() {
    let temp = TempDir::new().expect("temp locked-keyring store");
    let path = temp.path().join("locked-keyring.sqlite");
    let store = Store::new(Some(path.clone()));
    let error = store
        .connection_profiles()
        .insert(&NewConnectionProfile::new(
            "locked",
            "postgres://user:must-not-persist@localhost/app",
        ))
        .expect_err("locked native credential store must reject persistence");
    assert!(format!("{error:#}").contains("native secure storage"));

    let raw = Connection::open(path).expect("open rejected store");
    let rows: i64 = raw
        .query_row("SELECT count(*) FROM connection_profiles", [], |row| {
            row.get(0)
        })
        .expect("count rejected profiles");
    assert_eq!(rows, 0);
}

#[cfg(target_os = "linux")]
fn linux_keyring_compatibility_store_path() -> std::path::PathBuf {
    std::env::var_os("POQI_KEYRING_COMPAT_STORE")
        .map(std::path::PathBuf::from)
        .expect("POQI_KEYRING_COMPAT_STORE must name the shared compatibility database")
}

#[cfg(target_os = "linux")]
fn store_id(path: &std::path::Path) -> String {
    Connection::open(path)
        .expect("open compatibility database")
        .query_row(
            "SELECT value FROM store_metadata WHERE key = 'store.id'",
            [],
            |row| row.get(0),
        )
        .expect("read compatibility store identity")
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "run by the isolated Linux keyring compatibility harness"]
fn linux_keyring_compatibility_seed_with_selected_backend() {
    let path = linux_keyring_compatibility_store_path();
    let legacy_uri = "postgres://legacy:legacy-secret@localhost/app";
    Store::new_with_test_key(Some(path.clone()), TEST_KEY)
        .connection_profiles()
        .insert(&NewConnectionProfile::new("legacy", legacy_uri))
        .expect("write legacy fixture with deterministic key");
    crate::crypto::store_native_key_for_test(&store_id(&path), &TEST_KEY)
        .expect("store fixture key with selected native backend");
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "run by the isolated Linux keyring compatibility harness"]
fn linux_keyring_compatibility_read_and_write_with_selected_backend() {
    let path = linux_keyring_compatibility_store_path();
    let store = Store::new(Some(path));
    assert_eq!(
        store
            .connection_profiles()
            .get("legacy")
            .expect("read legacy profile")
            .expect("legacy profile exists")
            .uri,
        "postgres://legacy:legacy-secret@localhost/app"
    );
    store
        .connection_profiles()
        .insert(&NewConnectionProfile::new(
            "async",
            "postgres://async:async-secret@localhost/app",
        ))
        .expect("write profile with selected native backend");
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "run by the isolated Linux keyring compatibility harness"]
fn linux_keyring_compatibility_verify_with_selected_backend() {
    let path = linux_keyring_compatibility_store_path();
    let store = Store::new(Some(path.clone()));
    let profiles = store
        .connection_profiles()
        .list()
        .expect("read compatibility profiles");
    assert_eq!(profiles.len(), 2);
    assert!(profiles.iter().any(|profile| {
        profile.name == "async" && profile.uri == "postgres://async:async-secret@localhost/app"
    }));
    crate::crypto::delete_native_key_for_test(&store_id(&path))
        .expect("delete compatibility test key");
}
