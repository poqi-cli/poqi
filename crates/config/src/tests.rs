use super::*;
use poqi_store::{settings, Store};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tempfile::TempDir;

static ENV_LOCK: Mutex<()> = Mutex::new(());
const TEST_STORE_KEY: [u8; 32] = [0x5a; 32];

#[test]
fn default_round_trip() {
    let config = AppConfig::default();
    let serialized = toml::to_string(&config).expect("serialize");
    let deserialized: AppConfig = toml::from_str(&serialized).expect("deserialize");
    assert_eq!(deserialized.ui.theme, config.ui.theme);
    assert_eq!(
        deserialized.db.defaults.page_size,
        config.db.defaults.page_size
    );
    assert!(serialized.contains("statement_timeout_ms = 30000"));
}

#[test]
fn parses_sample_keymap_structure() {
    let sample = r#"
[keymap]
profile = "default"

[keymap.default.global]
back = "esc"
quit = "shift+esc"

[keymap.default.schema]
move_up = "w"
move_down = "s"
"#;
    let parsed: AppConfig = toml::from_str(sample).expect("parse sample keymap");
    let default_profile = parsed
        .keymap
        .profiles
        .get("default")
        .expect("default profile");
    let schema = default_profile
        .contexts
        .get("schema")
        .expect("schema context");
    assert_eq!(schema.get("move_up").map(String::as_str), Some("w"));
}

#[test]
fn resolves_default_keymap_profile() {
    let config = AppConfig::default();
    let resolved = config
        .keymap
        .active_profile()
        .expect("resolve default profile");
    let schema = resolved
        .contexts
        .get("schema")
        .expect("schema context present");
    assert!(schema
        .iter()
        .any(|binding| binding.action == "move_up" && binding.sequence.combos.len() == 1));
}

#[test]
fn search_defaults_align_with_spec() {
    let config = AppConfig::default();
    assert_eq!(config.search.semantic_dim, 384);
    assert_eq!(config.search.ann_engine, "usearch");
    assert!(!config.search.rebuild_on_start);
    assert_eq!(
        config.search.model_dir,
        std::path::PathBuf::from("models/granite-embedding-97m-multilingual-r2")
    );
    assert_eq!(config.search.semantic_batch_size, 96);
    assert_eq!(config.search.semantic_top_k, 100);
    assert_eq!(config.search.semantic_score_threshold, Some(0.2));
    assert!(config.search.semantic_title_column.is_none());
    assert_eq!(
        config.search.semantic_runtime_preference,
        SemanticRuntimePreference::Off
    );
}

#[test]
fn partial_search_section_preserves_defaults() {
    let sample = r#"
[search]
model_dir = "alt-models"
"#;
    let parsed: AppConfig = toml::from_str(sample).expect("parse partial search config");
    assert_eq!(parsed.search.model_dir, PathBuf::from("alt-models"));
    assert_eq!(parsed.search.semantic_dim, 384);
    assert_eq!(parsed.search.ann_engine, "usearch");
    assert_eq!(parsed.search.semantic_batch_size, 96);
    assert_eq!(parsed.search.semantic_top_k, 100);
    assert_eq!(parsed.search.semantic_score_threshold, Some(0.2));
    assert!(parsed.search.semantic_title_column.is_none());
    assert_eq!(
        parsed.search.semantic_runtime_preference,
        SemanticRuntimePreference::Off
    );
}

#[test]
fn load_or_default_with_store_prefers_db_values() {
    with_temp_config_dir(|_| {
        let temp_store = TempDir::new().expect("temp store dir");
        let store_path = temp_store.path().join("settings.sqlite");
        let store = Store::new(Some(store_path));
        store.init().expect("init store");
        let repo = store.settings();
        repo.set_theme("light").expect("set theme");
        repo.set_semantic_batch_size(24)
            .expect("set semantic batch size");

        let config = AppConfig::load_or_default_with_store(&store).expect("load config");
        assert_eq!(config.ui.theme, "light");
        assert_eq!(config.search.semantic_batch_size, 24);
    });
}

#[test]
fn persisted_off_preference_stays_disabled() {
    with_temp_config_dir(|_| {
        let temp_store = TempDir::new().expect("temp store dir");
        let store = Store::new(Some(temp_store.path().join("settings.sqlite")));
        store.init().expect("init store");
        store
            .settings()
            .set_semantic_runtime_preference(SemanticRuntimePreference::Off)
            .expect("disable semantic runtime");

        let config = AppConfig::load_or_default_with_store(&store).expect("load config");

        assert_eq!(
            config.search.semantic_runtime_preference,
            SemanticRuntimePreference::Off
        );
    });
}

#[test]
fn persisted_auto_preference_is_not_rewritten() {
    with_temp_config_dir(|_| {
        let temp_store = TempDir::new().expect("temp store dir");
        let store = Store::new(Some(temp_store.path().join("settings.sqlite")));
        store.init().expect("init store");
        let settings = store.settings();
        settings
            .set_config_toml_import_complete()
            .expect("mark import complete");
        settings
            .set_semantic_runtime_preference(SemanticRuntimePreference::Auto)
            .expect("select automatic runtime");

        let config = AppConfig::load_or_default_with_store(&store).expect("load config");

        assert_eq!(
            config.search.semantic_runtime_preference,
            SemanticRuntimePreference::Auto
        );
    });
}

#[test]
fn load_or_default_with_store_seeds_from_config_file() {
    with_temp_config_dir(|dir| {
        let file = dir.join("config.toml");
        fs::write(
            &file,
            b"[ui]\ntheme = \"midnight\"\n[search]\nsemantic_batch_size = 7\n",
        )
        .expect("write config file");

        let temp_store = TempDir::new().expect("temp store dir");
        let store_path = temp_store.path().join("settings.sqlite");
        let store = Store::new(Some(store_path));
        store.init().expect("init store");

        let config = AppConfig::load_or_default_with_store(&store).expect("load config");
        assert_eq!(config.ui.theme, "midnight");
        assert_eq!(config.search.semantic_batch_size, 7);

        let stored_theme = store
            .settings()
            .theme()
            .expect("read theme")
            .expect("theme stored");
        assert_eq!(stored_theme, "midnight");
        assert!(store
            .settings()
            .config_toml_import_complete()
            .expect("read import marker"));
    });
}

#[test]
fn runtime_setting_rows_do_not_suppress_toml_import() {
    with_temp_config_dir(|dir| {
        fs::write(
            dir.join("config.toml"),
            b"[ui]\ntheme = \"midnight\"\n[db.defaults]\npage_size = 37\n",
        )
        .expect("write config file");

        let temp_store = TempDir::new().expect("temp store dir");
        let store = Store::new(Some(temp_store.path().join("settings.sqlite")));
        store.init().expect("init store");
        store
            .settings()
            .set_main_tick_rate_ms(11)
            .expect("seed unrelated runtime setting");

        let config = AppConfig::load_or_default_with_store(&store).expect("load config");
        assert_eq!(config.ui.theme, "midnight");
        assert_eq!(config.db.defaults.page_size, 37);
    });
}

#[test]
fn complete_config_survives_two_store_loads() {
    with_temp_config_dir(|dir| {
        let source = r#"
[ui]
theme = "midnight"
reduced_motion = true

[keymap]
profile = "vim"

[keymap.vim.global]
back = "ctrl+g"

[db.defaults]
statement_timeout_ms = 1234
page_size = 37

[db.primary]
name = "custom"
uri = "postgres://user:secret@example.test/app"
max_pool_size = 5
connect_timeout_ms = 2500

[search]
semantic_dim = 111
ann_engine = "custom-ann"
rebuild_on_start = true
model_dir = "models/custom"
semantic_batch_size = 7
semantic_top_k = 9
semantic_score_threshold = 0.42
semantic_title_column = "title"
semantic_runtime_preference = "cpu"
"#;
        fs::write(dir.join("config.toml"), source).expect("write config file");
        let expected: AppConfig = toml::from_str(source).expect("parse expected config");

        let temp_store = TempDir::new().expect("temp store dir");
        let store = Store::new_with_test_key(
            Some(temp_store.path().join("settings.sqlite")),
            TEST_STORE_KEY,
        );
        store.init().expect("init store");

        let first = AppConfig::load_or_default_with_store(&store).expect("first load");
        assert_eq!(first, expected);

        fs::write(
            dir.join("config.toml"),
            toml::to_string(&AppConfig::default()).unwrap(),
        )
        .expect("replace config mirror");
        let second = AppConfig::load_or_default_with_store(&store).expect("second load");
        assert_eq!(second, expected);
    });
}

#[test]
fn successful_primary_import_scrubs_only_primary_from_active_toml() {
    with_temp_config_dir(|dir| {
        let source = r#"# keep this comment
[ui]
theme = "midnight"

[db.defaults]
statement_timeout_ms = 1234
page_size = 37

[db.primary]
name = "secure"
uri = "postgres://user:secret@example.test/app"
max_pool_size = 5
connect_timeout_ms = 2500

[extension]
custom_value = "preserved"
"#;
        let path = dir.join("config.toml");
        fs::write(&path, source).expect("write config file");
        let expected: AppConfig = toml::from_str(source).expect("parse expected config");
        let temp_store = TempDir::new().expect("temp store dir");
        let store = Store::new_with_test_key(
            Some(temp_store.path().join("settings.sqlite")),
            TEST_STORE_KEY,
        );
        store.init().expect("init store");

        let loaded = AppConfig::load_or_default_with_store(&store).expect("import config");

        assert_eq!(loaded.db, expected.db);
        let stored = store
            .settings()
            .get_setting::<Option<ConnectionConfig>>(settings::KEY_DB_PRIMARY)
            .expect("read primary setting")
            .expect("primary row");
        assert_eq!(stored, expected.db.primary);
        let scrubbed = fs::read_to_string(path).expect("read scrubbed config");
        assert!(!scrubbed.contains("[db.primary]"));
        assert!(!scrubbed.contains("user:secret"));
        assert!(scrubbed.contains("# keep this comment"));
        assert!(scrubbed.contains("statement_timeout_ms = 1234"));
        assert!(scrubbed.contains("[extension]"));
        assert!(scrubbed.contains("custom_value = \"preserved\""));
    });
}

#[test]
fn primary_scrub_preserves_following_unknown_array_of_tables() {
    with_temp_config_dir(|dir| {
        let source = r#"[db.primary]
uri = "postgres://user:secret@example.test/app"

[[extensions]]
name = "first"

[[extensions]]
name = "second"
"#;
        let path = dir.join("config.toml");
        fs::write(&path, source).expect("write config file");
        let temp_store = TempDir::new().expect("temp store dir");
        let store = Store::new_with_test_key(
            Some(temp_store.path().join("settings.sqlite")),
            TEST_STORE_KEY,
        );
        store.init().expect("init store");

        AppConfig::load_or_default_with_store(&store).expect("import config");

        let scrubbed = fs::read_to_string(path).expect("read scrubbed config");
        let value: toml::Value = toml::from_str(&scrubbed).expect("parse scrubbed config");
        assert!(value.get("db").and_then(|db| db.get("primary")).is_none());
        let extensions = value
            .get("extensions")
            .and_then(toml::Value::as_array)
            .expect("preserved extensions array");
        assert_eq!(extensions.len(), 2);
        assert_eq!(
            extensions[0].get("name").and_then(toml::Value::as_str),
            Some("first")
        );
        assert_eq!(
            extensions[1].get("name").and_then(toml::Value::as_str),
            Some("second")
        );
    });
}

#[test]
fn failed_secure_primary_import_leaves_active_toml_untouched() {
    with_temp_config_dir(|dir| {
        let source =
            b"# original\n[db.primary]\nuri = \"postgres://user:secret@example.test/app\"\n";
        let path = dir.join("config.toml");
        fs::write(&path, source).expect("write config file");
        let temp_store = TempDir::new().expect("temp store dir");
        let store =
            Store::new_with_unavailable_test_key(Some(temp_store.path().join("settings.sqlite")));
        store.init().expect("init store");

        AppConfig::load_or_default_with_store(&store)
            .expect_err("unavailable encryption key must fail import");

        assert_eq!(fs::read(path).expect("read original config"), source);
        assert!(!store
            .settings()
            .config_toml_import_complete()
            .expect("read import marker"));
    });
}

#[test]
fn completed_import_scrubs_matching_toml_primary_on_later_load() {
    with_temp_config_dir(|dir| {
        let primary = sample_primary("stored");
        let source = format!(
            "# retained\n[db.primary]\nname = \"stored\"\nuri = \"{}\"\n\n[extension]\nvalue = 7\n",
            primary.uri
        );
        let path = dir.join("config.toml");
        fs::write(&path, source).expect("write stale config");
        let temp_store = TempDir::new().expect("temp store dir");
        let store = Store::new_with_test_key(
            Some(temp_store.path().join("settings.sqlite")),
            TEST_STORE_KEY,
        );
        store.init().expect("init store");
        let repo = store.settings();
        repo.set_setting(settings::KEY_DB_PRIMARY, &Some(primary.clone()))
            .expect("seed encrypted primary");
        repo.set_config_toml_import_complete()
            .expect("mark import complete");
        let loaded = AppConfig::load_or_default_with_store(&store).expect("load config");

        assert_eq!(loaded.db.primary.as_ref(), Some(&primary));
        let scrubbed = fs::read_to_string(path).expect("read scrubbed config");
        assert!(!scrubbed.contains("[db.primary]"));
        assert!(!scrubbed.contains("user:stored"));
        assert!(scrubbed.contains("# retained"));
        assert!(scrubbed.contains("[extension]"));
    });
}

#[test]
fn conflicting_toml_primary_is_reported_and_preserved() {
    with_temp_config_dir(|dir| {
        let source =
            b"[db.primary]\nname = \"toml\"\nuri = \"postgres://user:toml@example.test/app\"\n";
        let path = dir.join("config.toml");
        fs::write(&path, source).expect("write conflicting config");
        let temp_store = TempDir::new().expect("temp store dir");
        let store = Store::new_with_test_key(
            Some(temp_store.path().join("settings.sqlite")),
            TEST_STORE_KEY,
        );
        store.init().expect("init store");
        let repo = store.settings();
        repo.set_setting(settings::KEY_DB_PRIMARY, &Some(sample_primary("sqlite")))
            .expect("seed encrypted primary");
        repo.set_config_toml_import_complete()
            .expect("mark import complete");
        let error = AppConfig::load_or_default_with_store(&store)
            .expect_err("conflicting primary must stop loading");

        assert!(error.to_string().contains("differs from encrypted SQLite"));
        assert_eq!(fs::read(path).expect("read conflicting config"), source);
    });
}

#[test]
fn config_save_keeps_primary_in_store_and_omits_it_from_toml_mirror() {
    with_temp_config_dir(|dir| {
        let temp_store = TempDir::new().expect("temp store dir");
        let store = Store::new_with_test_key(
            Some(temp_store.path().join("settings.sqlite")),
            TEST_STORE_KEY,
        );
        let mut config = AppConfig::default();
        config.db.primary = Some(sample_primary("saved"));
        config.db.defaults.page_size = 91;

        config.save_with_store(&store).expect("save config");

        let mirror = fs::read_to_string(dir.join("config.toml")).expect("read config mirror");
        assert!(!mirror.contains("[db.primary]"));
        assert!(!mirror.contains("user:saved"));
        assert!(mirror.contains("page_size = 91"));
        let reloaded = AppConfig::load_or_default_with_store(&store).expect("reload config");
        assert_eq!(reloaded.db, config.db);
    });
}

#[test]
fn config_save_rejects_conflicting_toml_before_changing_store() {
    with_temp_config_dir(|dir| {
        let source =
            b"[db.primary]\nname = \"toml\"\nuri = \"postgres://user:toml@example.test/app\"\n";
        let path = dir.join("config.toml");
        fs::write(&path, source).expect("write conflicting config");
        let temp_store = TempDir::new().expect("temp store dir");
        let store = Store::new_with_test_key(
            Some(temp_store.path().join("settings.sqlite")),
            TEST_STORE_KEY,
        );
        store.init().expect("init store");
        let original = sample_primary("original");
        store
            .settings()
            .set_setting(settings::KEY_DB_PRIMARY, &Some(original.clone()))
            .expect("seed original primary");
        let mut config = AppConfig::default();
        config.db.primary = Some(sample_primary("replacement"));

        let error = config
            .save_with_store(&store)
            .expect_err("conflicting TOML must stop save");

        assert!(error.to_string().contains("differs from encrypted SQLite"));
        assert_eq!(fs::read(path).expect("read conflicting config"), source);
        let stored = store
            .settings()
            .get_setting::<Option<ConnectionConfig>>(settings::KEY_DB_PRIMARY)
            .expect("read primary setting")
            .expect("primary row");
        assert_eq!(stored, Some(original));
    });
}

#[test]
fn failed_secure_save_does_not_replace_existing_toml() {
    with_temp_config_dir(|dir| {
        let source = b"# existing config\n[ui]\ntheme = \"light\"\n";
        let path = dir.join("config.toml");
        fs::write(&path, source).expect("write existing config");
        let temp_store = TempDir::new().expect("temp store dir");
        let store =
            Store::new_with_unavailable_test_key(Some(temp_store.path().join("settings.sqlite")));
        let mut config = AppConfig::default();
        config.db.primary = Some(sample_primary("failed"));

        config
            .save_with_store(&store)
            .expect_err("unavailable encryption key must fail save");

        assert_eq!(fs::read(path).expect("read existing config"), source);
    });
}

#[test]
fn poqi_config_dir_override_is_used() {
    with_temp_config_dir(|dir| {
        assert_eq!(AppConfig::config_dir().expect("resolve config dir"), dir);
    });
}

fn sample_primary(password: &str) -> ConnectionConfig {
    ConnectionConfig {
        name: Some(password.to_owned()),
        uri: format!("postgres://user:{password}@example.test/app"),
        max_pool_size: None,
        connect_timeout: None,
    }
}

fn with_temp_config_dir<F: FnOnce(&Path)>(f: F) {
    let _guard = ENV_LOCK.lock().expect("lock env");
    let temp = TempDir::new().expect("temp config dir");
    let config_path = temp.path().join("poqi-config");
    fs::create_dir_all(&config_path).expect("create config path");
    let previous_poqi = std::env::var_os("POQI_CONFIG_DIR");
    std::env::set_var("POQI_CONFIG_DIR", &config_path);
    f(&config_path);
    restore_env("POQI_CONFIG_DIR", previous_poqi);
}

fn restore_env(name: &str, value: Option<OsString>) {
    if let Some(value) = value {
        std::env::set_var(name, value);
    } else {
        std::env::remove_var(name);
    }
}
