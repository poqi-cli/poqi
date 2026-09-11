use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use clap::Parser;
use poqi_config::AppConfig;
use poqi_store::Store;

use crate::{
    bootstrap,
    cli::{apply_cli_overrides, Cli},
};

struct TempStore {
    store: Store,
    dir: PathBuf,
}

impl TempStore {
    fn new() -> Self {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "poqi-app-test-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&dir).expect("create temp store directory");
        let store_path = dir.join("store.sqlite");
        Self {
            store: Store::new_with_test_key(Some(store_path), [7; 32]),
            dir,
        }
    }

    fn store(&self) -> &Store {
        &self.store
    }
}

impl Drop for TempStore {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn parses_cli_flags_without_overrides() {
    let cli = Cli::parse_from(["poqi"]);
    assert!(!cli.show_config);
    assert!(!cli.doctor);
    assert!(cli.keymap.is_none());
}

#[test]
fn applies_cli_overrides_to_config() {
    let cli = Cli::parse_from(["poqi", "--keymap", "default"]);
    let mut config = AppConfig::default();
    apply_cli_overrides(&cli, &mut config);
    assert_eq!(config.keymap.profile, "default");
}

#[test]
fn startup_flags_are_replaced_by_menu_and_settings() {
    for args in [
        vec!["poqi", "--profiles"],
        vec!["poqi", "--semantic-runtime", "auto"],
        vec!["poqi", "--url", "postgres://localhost/app"],
        vec!["poqi", "--profile", "local"],
    ] {
        assert!(Cli::try_parse_from(args).is_err());
    }
}
#[test]
fn explicit_saved_profile_resolves_without_saving_other_connections() {
    let harness = TempStore::new();
    harness.store().init().expect("init store");
    harness
        .store()
        .connection_profiles()
        .upsert(&poqi_store::NewConnectionProfile::new(
            "local",
            "postgres://localhost/app",
        ))
        .unwrap();
    let cli = Cli::parse_from(["poqi", "--check-connection", "--profile", "local"]);
    let choice = bootstrap::resolve_connection_profile(
        &cli,
        &AppConfig::default(),
        harness.store(),
        Duration::from_millis(1),
    )
    .unwrap();
    assert_eq!(choice.profile.name, "local");
    assert_eq!(choice.profile.uri, "postgres://localhost/app");
    assert!(!choice.save);
}

#[test]
fn new_profile_is_saved_only_after_success_and_headless_choice_is_not_saved() {
    let harness = TempStore::new();
    harness.store().init().expect("init store");
    let mut choice = bootstrap::ConnectionChoice::new(
        poqi_db::ConnectionProfile::new("new", "postgres://localhost/app"),
        true,
        "connection form",
    );
    assert!(harness
        .store()
        .connection_profiles()
        .list()
        .unwrap()
        .is_empty());
    choice.persist_after_success(harness.store()).unwrap();
    assert!(!choice.save);
    assert_eq!(
        harness.store().connection_profiles().list().unwrap().len(),
        1
    );
    let mut session = bootstrap::ConnectionChoice::new(
        poqi_db::ConnectionProfile::new("headless", "postgres://localhost/other"),
        false,
        "connection form",
    );
    session.persist_after_success(harness.store()).unwrap();
    assert!(harness
        .store()
        .connection_profiles()
        .get("headless")
        .unwrap()
        .is_none());
}

#[test]
fn connection_cli_rejects_ambiguous_choices_and_invalid_timeout() {
    for args in [
        vec![
            "poqi",
            "--check-connection",
            "--url",
            "postgres://a",
            "--profile",
            "b",
        ],
        vec![
            "poqi",
            "--check-connection",
            "postgres://a",
            "--url",
            "postgres://b",
        ],
        vec!["poqi", "--list-profiles", "--check-connection"],
        vec!["poqi", "--connect-timeout", "0"],
    ] {
        assert!(Cli::try_parse_from(args).is_err());
    }
    assert!(Cli::try_parse_from(["poqi", "--check-connection", "--profile", "local"]).is_ok());
}

#[test]
fn failed_save_retains_pending_choice_and_does_not_overwrite_name_collision() {
    let harness = TempStore::new();
    harness.store().init().unwrap();
    let mut choice = bootstrap::ConnectionChoice::new(
        poqi_db::ConnectionProfile::new("primary", "postgres://new/app"),
        true,
        "connection form",
    );
    let invalid_store = Store::new_with_test_key(Some(harness.dir.clone()), [7; 32]);
    assert!(choice.persist_after_success(&invalid_store).is_err());
    assert!(choice.save);
    harness
        .store()
        .connection_profiles()
        .upsert(&poqi_store::NewConnectionProfile::new(
            "primary",
            "postgres://original/app",
        ))
        .unwrap();
    assert!(choice.persist_after_success(harness.store()).is_err());
    assert!(choice.save);
    assert_eq!(
        harness
            .store()
            .connection_profiles()
            .get("primary")
            .unwrap()
            .unwrap()
            .uri,
        "postgres://original/app"
    );
    choice.editing = true;
    choice.persist_after_success(harness.store()).unwrap();
    assert_eq!(
        harness
            .store()
            .connection_profiles()
            .get("primary")
            .unwrap()
            .unwrap()
            .uri,
        "postgres://new/app"
    );
}

#[test]
fn unavailable_secure_storage_preserves_connection_for_retry() {
    let harness = TempStore::new();
    let unavailable = Store::new_with_unavailable_test_key(harness.store().path.clone());
    let uri = "postgres://user:retry-secret@localhost/app";
    let mut choice = bootstrap::ConnectionChoice::new(
        poqi_db::ConnectionProfile::new("retry", uri),
        true,
        "connection form",
    );
    assert!(choice.persist_after_success(&unavailable).is_err());
    assert!(choice.save);
    assert_eq!(choice.profile.uri, uri);
    assert!(harness
        .store()
        .connection_profiles()
        .list()
        .unwrap()
        .is_empty());
    choice.persist_after_success(harness.store()).unwrap();
    assert!(!choice.save);
    assert_eq!(
        harness
            .store()
            .connection_profiles()
            .get("retry")
            .unwrap()
            .unwrap()
            .uri,
        uri
    );
}
