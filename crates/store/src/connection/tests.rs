use super::*;
use crate::test_support::TestStore;

#[test]
fn init_creates_database_file() {
    let harness = TestStore::new();
    harness.store().init().expect("init store");
    assert!(harness.db_path().exists());
}

#[test]
fn password_uri_is_encrypted_and_returned_in_memory() {
    let harness = TestStore::new();
    let repo = harness.store().connection_profiles();
    let profile = NewConnectionProfile::new("primary", "postgres://user:secret@localhost:5432/app");
    let stored = repo
        .upsert(&profile)
        .expect("should store profile with password in URL");
    assert_eq!(stored.name, "primary");
    assert_eq!(stored.uri, "postgres://user:secret@localhost:5432/app");
}

#[test]
fn raw_database_contains_only_ciphertext_for_full_unicode_uri() {
    let harness = TestStore::new();
    let uri = "postgres://käyttäjä:p%40ss%2F秘密@localhost:5432/tietokanta?application_name=poqi%20测试&sslmode=require";
    harness
        .store()
        .connection_profiles()
        .upsert(&NewConnectionProfile::new("ensisijainen-数据库", uri))
        .expect("store encrypted profile");

    let raw = Connection::open(harness.db_path()).expect("open raw database");
    let (reserved_plaintext, nonce, ciphertext): (String, Vec<u8>, Vec<u8>) = raw
        .query_row(
            "SELECT uri, uri_nonce, uri_ciphertext FROM connection_profiles",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read encrypted row");
    assert!(reserved_plaintext.is_empty());
    assert_eq!(nonce.len(), 12);
    assert!(!ciphertext
        .windows(b"p%40ss%2F".len())
        .any(|part| part == b"p%40ss%2F"));
    let bytes = std::fs::read(harness.db_path()).expect("read database bytes");
    assert!(!bytes.windows(uri.len()).any(|part| part == uri.as_bytes()));

    let stored = harness
        .store()
        .connection_profiles()
        .get("ensisijainen-数据库")
        .expect("read encrypted profile")
        .expect("profile exists");
    assert_eq!(stored.uri, uri);
}

#[test]
fn repeated_writes_use_distinct_nonces() {
    let harness = TestStore::new();
    let repo = harness.store().connection_profiles();
    let profile = NewConnectionProfile::new("primary", "postgres://user:secret@localhost/app");
    repo.upsert(&profile).expect("first encrypted write");
    let first: Vec<u8> = Connection::open(harness.db_path())
        .expect("open database")
        .query_row("SELECT uri_nonce FROM connection_profiles", [], |row| {
            row.get(0)
        })
        .expect("read first nonce");
    repo.upsert(&profile).expect("second encrypted write");
    let second: Vec<u8> = Connection::open(harness.db_path())
        .expect("open database")
        .query_row("SELECT uri_nonce FROM connection_profiles", [], |row| {
            row.get(0)
        })
        .expect("read second nonce");
    assert_ne!(first, second);
}

#[test]
fn tampered_ciphertext_fails_closed() {
    let harness = TestStore::new();
    harness
        .store()
        .connection_profiles()
        .upsert(&NewConnectionProfile::new(
            "primary",
            "postgres://user:secret@localhost/app",
        ))
        .expect("store encrypted profile");
    let raw = Connection::open(harness.db_path()).expect("open database");
    raw.execute(
        "UPDATE connection_profiles
         SET uri_ciphertext = CAST(X'00' || substr(uri_ciphertext, 2) AS BLOB)",
        [],
    )
    .expect("tamper with ciphertext");
    let error = harness
        .store()
        .connection_profiles()
        .get("primary")
        .expect_err("tampered ciphertext must fail");
    assert!(format!("{error:#}").contains("failed authentication"));
}

#[test]
fn changing_profile_name_breaks_aad_authentication() {
    let harness = TestStore::new();
    harness
        .store()
        .connection_profiles()
        .upsert(&NewConnectionProfile::new(
            "primary",
            "postgres://user:secret@localhost/app",
        ))
        .expect("store encrypted profile");
    Connection::open(harness.db_path())
        .expect("open database")
        .execute("UPDATE connection_profiles SET name = 'renamed'", [])
        .expect("change authenticated name");
    assert!(harness
        .store()
        .connection_profiles()
        .get("renamed")
        .is_err());
}

#[test]
fn plaintext_downgrade_is_rejected() {
    let harness = TestStore::new();
    harness
        .store()
        .connection_profiles()
        .upsert(&NewConnectionProfile::new(
            "primary",
            "postgres://user:secret@localhost/app",
        ))
        .expect("store encrypted profile");
    Connection::open(harness.db_path())
        .expect("open database")
        .execute(
            "UPDATE connection_profiles
             SET uri = 'postgres://attacker:plaintext@localhost/app',
                 uri_nonce = NULL, uri_ciphertext = NULL",
            [],
        )
        .expect("downgrade encrypted row");
    let error = harness
        .store()
        .connection_profiles()
        .list()
        .expect_err("plaintext downgrade must fail");
    assert!(format!("{error:#}").contains("unsupported plaintext store data"));
}

#[test]
fn can_store_and_retrieve_connection_profile() {
    let harness = TestStore::new();
    let repo = harness.store().connection_profiles();
    let profile = NewConnectionProfile::new("primary", "postgres://user:secret@localhost:5432/app");
    let stored = repo.upsert(&profile).expect("store profile");
    assert_eq!(stored.name, "primary");
    assert_eq!(stored.uri, "postgres://user:secret@localhost:5432/app");

    let fetched = repo.get("primary").expect("fetch").expect("profile exists");
    assert_eq!(fetched.id, stored.id);
    assert_eq!(fetched.uri, stored.uri);
}

#[test]
fn list_returns_sorted_profiles() {
    let harness = TestStore::new();
    let repo = harness.store().connection_profiles();
    repo.upsert(&NewConnectionProfile::new(
        "beta",
        "postgres://beta@localhost/db",
    ))
    .expect("store beta");
    repo.upsert(&NewConnectionProfile::new(
        "alpha",
        "postgres://alpha@localhost/db",
    ))
    .expect("store alpha");

    let names: Vec<String> = repo
        .list()
        .expect("list profiles")
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, vec!["alpha".to_string(), "beta".to_string()]);
}

#[test]
fn delete_removes_profile() {
    let harness = TestStore::new();
    let repo = harness.store().connection_profiles();
    repo.upsert(&NewConnectionProfile::new(
        "primary",
        "postgres://user@localhost/db",
    ))
    .expect("store profile");
    assert!(repo.delete("primary").expect("delete profile"));
    assert!(repo.get("primary").expect("fetch").is_none());
    assert!(!repo.delete("primary").expect("delete non-existent"));
}

#[test]
fn concurrent_new_profiles_never_replace_the_winning_name() {
    use std::sync::{Arc, Barrier};
    let harness = TestStore::new();
    harness.store().init().unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let handles: Vec<_> = ["postgres://first/app", "postgres://second/app"]
        .into_iter()
        .map(|uri| {
            let store = harness.store().clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                store
                    .connection_profiles()
                    .insert(&NewConnectionProfile::new("shared", uri))
            })
        })
        .collect();
    barrier.wait();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    let successes: Vec<_> = results
        .iter()
        .filter_map(|result| result.as_ref().ok())
        .collect();
    assert_eq!(successes.len(), 1);
    let stored = harness
        .store()
        .connection_profiles()
        .get("shared")
        .unwrap()
        .unwrap();
    assert_eq!(stored.uri, successes[0].uri);
    assert_eq!(
        harness.store().connection_profiles().list().unwrap().len(),
        1
    );
}
