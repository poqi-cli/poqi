use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use poqi_store::{settings, SettingsStore, Store};

use crate::{AppConfig, ConfigError, ConnectionConfig};

const CONFIG_FILE_NAME: &str = "config.toml";

impl AppConfig {
    /// Resolve the directory where the CLI stores configuration files.
    ///
    /// # Errors
    /// Returns an error when an appropriate configuration directory cannot be determined.
    pub fn config_dir() -> Result<PathBuf> {
        if let Some(dir) = env::var_os("POQI_CONFIG_DIR") {
            return Ok(PathBuf::from(dir));
        }

        let dirs = ProjectDirs::from("com", "poqi", "poqi").context(ConfigError::NoConfigDir)?;
        Ok(dirs.config_dir().to_path_buf())
    }

    /// Load the persisted configuration file, or return the built-in defaults.
    ///
    /// # Errors
    /// Returns an error if the configuration file cannot be read or parsed.
    pub fn load_or_default() -> Result<Self> {
        let store = Store::default();
        store.init()?;
        Self::load_or_default_with_store(&store)
    }

    /// Load configuration values backed by the provided store.
    ///
    /// The initial TOML file is imported exactly once. Before the import marker exists,
    /// already-persisted `SQLite` values take precedence over TOML. Once marked,
    /// `SQLite` is the sole runtime authority.
    ///
    /// # Errors
    /// Returns an error if the store cannot be read or the on-disk config is invalid.
    pub fn load_or_default_with_store(store: &Store) -> Result<Self> {
        let settings = store.settings();
        let import_complete = settings.config_toml_import_complete()?;
        let config_path = Self::config_dir()?.join(CONFIG_FILE_NAME);
        let mut config = if import_complete {
            Self::default()
        } else {
            Self::load_from_disk(&config_path)?.unwrap_or_default()
        };
        let unimported_primary = (!import_complete)
            .then(|| config.db.primary.clone())
            .flatten();

        if import_complete {
            ensure_config_defaults(&settings, &config)?;
        }
        hydrate_config_from_settings(&settings, &mut config)?;

        if !import_complete {
            ensure_primary_matches(unimported_primary.as_ref(), config.db.primary.as_ref())?;
            persist_config_settings(&settings, &config)?;
            settings.set_config_toml_import_complete()?;
        }

        scrub_primary_from_active_toml(&config_path, config.db.primary.as_ref())?;
        Ok(config)
    }

    /// Persist the configuration to `SQLite` and mirror it to TOML.
    ///
    /// # Errors
    /// Returns an error when the configuration directory cannot be created or the file cannot be written.
    pub fn save(&self) -> Result<()> {
        let store = Store::default();
        self.save_with_store(&store)
    }

    pub(crate) fn save_with_store(&self, store: &Store) -> Result<()> {
        let dir = Self::config_dir()?;
        let path = dir.join(CONFIG_FILE_NAME);
        ensure_active_toml_primary_matches(&path, self.db.primary.as_ref())?;

        store.init()?;
        let settings = store.settings();
        persist_config_settings(&settings, self)?;
        settings.set_config_toml_import_complete()?;
        fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create config directory {}", dir.display()))?;
        let mut mirror = self.clone();
        mirror.db.primary = None;
        let data = toml::to_string_pretty(&mirror)?;
        fs::write(&path, data)
            .with_context(|| format!("failed to write config to {}", path.display()))?;
        Ok(())
    }
}

fn ensure_active_toml_primary_matches(
    path: &Path,
    authoritative_primary: Option<&ConnectionConfig>,
) -> Result<()> {
    if !path
        .try_exists()
        .with_context(|| format!("failed to inspect config at {}", path.display()))?
    {
        return Ok(());
    }
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read config from {}", path.display()))?;
    let primary = primary_from_toml(&contents, path)?;
    ensure_primary_matches(primary.as_ref(), authoritative_primary)
}

impl AppConfig {
    fn load_from_disk(path: &Path) -> Result<Option<Self>> {
        if !path.exists() {
            return Ok(None);
        }
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read config from {}", path.display()))?;
        let config: AppConfig = toml::from_str(&contents)
            .with_context(|| format!("failed to parse config at {}", path.display()))?;
        Ok(Some(config))
    }
}

fn scrub_primary_from_active_toml(
    path: &Path,
    authoritative_primary: Option<&ConnectionConfig>,
) -> Result<()> {
    if !path
        .try_exists()
        .with_context(|| format!("failed to inspect config at {}", path.display()))?
    {
        return Ok(());
    }

    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read config from {}", path.display()))?;
    let primary = primary_from_toml(&contents, path)?;
    let Some(primary) = primary.as_ref() else {
        return Ok(());
    };
    ensure_primary_matches(Some(primary), authoritative_primary)?;

    let scrubbed = remove_primary_from_toml(&contents, path)?;
    fs::write(path, scrubbed)
        .with_context(|| format!("failed to remove db.primary from {}", path.display()))
}

fn ensure_primary_matches(
    toml_primary: Option<&ConnectionConfig>,
    authoritative_primary: Option<&ConnectionConfig>,
) -> Result<()> {
    if toml_primary.is_some() && toml_primary != authoritative_primary {
        anyhow::bail!(
            "active config.toml contains a db.primary value that differs from encrypted SQLite; resolve the conflict before retrying"
        );
    }
    Ok(())
}

fn primary_from_toml(contents: &str, path: &Path) -> Result<Option<ConnectionConfig>> {
    let value: toml::Value = toml::from_str(contents)
        .with_context(|| format!("failed to parse config at {}", path.display()))?;
    value
        .get("db")
        .and_then(|db| db.get("primary"))
        .cloned()
        .map(toml::Value::try_into)
        .transpose()
        .with_context(|| format!("failed to parse db.primary at {}", path.display()))
}

fn remove_primary_from_toml(contents: &str, path: &Path) -> Result<String> {
    let mut expected: toml::Value = toml::from_str(contents)
        .with_context(|| format!("failed to parse config at {}", path.display()))?;
    if let Some(db) = expected.get_mut("db").and_then(toml::Value::as_table_mut) {
        db.remove("primary");
    }
    if expected
        .get("db")
        .and_then(toml::Value::as_table)
        .is_some_and(toml::map::Map::is_empty)
    {
        expected
            .as_table_mut()
            .expect("TOML document root must be a table")
            .remove("db");
    }

    if let Some(scrubbed) = strip_canonical_primary_table(contents) {
        if toml::from_str::<toml::Value>(&scrubbed).is_ok_and(|actual| actual == expected) {
            return Ok(scrubbed);
        }
    }

    toml::to_string_pretty(&expected).context("failed to serialize config without db.primary")
}

fn strip_canonical_primary_table(contents: &str) -> Option<String> {
    let mut output = String::with_capacity(contents.len());
    let mut removing = false;
    let mut found = false;

    for line in contents.split_inclusive('\n') {
        if let Some(header) = canonical_table_header(line) {
            removing = header == "db.primary";
            found |= removing;
        }
        if removing {
            if line.contains("\"\"\"") || line.contains("'''") {
                return None;
            }
        } else {
            output.push_str(line);
        }
    }

    found.then_some(output)
}

fn canonical_table_header(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('[') || trimmed.starts_with("[[") {
        return None;
    }
    let end = trimmed.find(']')?;
    let suffix = trimmed[end + 1..].trim();
    if !suffix.is_empty() && !suffix.starts_with('#') {
        return None;
    }
    Some(trimmed[1..end].trim())
}

fn persist_config_settings(settings: &SettingsStore<'_>, config: &AppConfig) -> Result<()> {
    settings.set_theme(&config.ui.theme)?;
    settings.set_reduced_motion(config.ui.reduced_motion)?;
    settings.set_keymap_profile(&config.keymap.profile)?;
    settings.set_setting(settings::KEY_KEYMAP_PROFILES, &config.keymap.profiles)?;
    settings.set_setting(
        settings::KEY_DB_STATEMENT_TIMEOUT_MS,
        &duration_millis(config.db.defaults.statement_timeout)?,
    )?;
    settings.set_setting(settings::KEY_DB_PAGE_SIZE, &config.db.defaults.page_size)?;
    settings.set_setting(settings::KEY_DB_PRIMARY, &config.db.primary)?;
    settings.set_model_dir(config.search.model_dir.clone())?;
    settings.set_semantic_dim(config.search.semantic_dim)?;
    settings.set_ann_engine(&config.search.ann_engine)?;
    settings.set_rebuild_on_start(config.search.rebuild_on_start)?;
    settings.set_semantic_batch_size(config.search.semantic_batch_size)?;
    settings.set_semantic_top_k(config.search.semantic_top_k)?;
    settings.set_semantic_score_threshold(config.search.semantic_score_threshold)?;
    settings.set_semantic_title_column(config.search.semantic_title_column.clone())?;
    settings.set_semantic_runtime_preference(config.search.semantic_runtime_preference)?;
    Ok(())
}

fn ensure_config_defaults(settings: &SettingsStore<'_>, defaults: &AppConfig) -> Result<()> {
    settings.ensure_setting(settings::KEY_UI_THEME, &defaults.ui.theme)?;
    settings.ensure_setting(settings::KEY_UI_REDUCED_MOTION, &defaults.ui.reduced_motion)?;
    settings.ensure_setting(settings::KEY_KEYMAP_PROFILE, &defaults.keymap.profile)?;
    settings.ensure_setting(settings::KEY_KEYMAP_PROFILES, &defaults.keymap.profiles)?;
    settings.ensure_setting(
        settings::KEY_DB_STATEMENT_TIMEOUT_MS,
        &duration_millis(defaults.db.defaults.statement_timeout)?,
    )?;
    settings.ensure_setting(settings::KEY_DB_PAGE_SIZE, &defaults.db.defaults.page_size)?;
    settings.ensure_setting(settings::KEY_DB_PRIMARY, &defaults.db.primary)?;
    settings.ensure_setting(settings::KEY_SEARCH_MODEL_DIR, &defaults.search.model_dir)?;
    settings.ensure_setting(
        settings::KEY_SEARCH_SEMANTIC_DIM,
        &defaults.search.semantic_dim,
    )?;
    settings.ensure_setting(settings::KEY_SEARCH_ANN_ENGINE, &defaults.search.ann_engine)?;
    settings.ensure_setting(
        settings::KEY_SEARCH_REBUILD_ON_START,
        &defaults.search.rebuild_on_start,
    )?;
    settings.ensure_setting(
        settings::KEY_SEARCH_SEMANTIC_BATCH_SIZE,
        &defaults.search.semantic_batch_size,
    )?;
    settings.ensure_setting(
        settings::KEY_SEARCH_SEMANTIC_TOP_K,
        &defaults.search.semantic_top_k,
    )?;
    settings.ensure_setting(
        settings::KEY_SEARCH_SEMANTIC_SCORE_THRESHOLD,
        &defaults.search.semantic_score_threshold,
    )?;
    settings.ensure_setting(
        settings::KEY_SEARCH_SEMANTIC_TITLE_COLUMN,
        &defaults.search.semantic_title_column,
    )?;
    settings.ensure_setting(
        settings::KEY_SEARCH_SEMANTIC_RUNTIME_PREFERENCE,
        &defaults.search.semantic_runtime_preference,
    )?;
    Ok(())
}

fn hydrate_config_from_settings(
    settings: &SettingsStore<'_>,
    config: &mut AppConfig,
) -> Result<()> {
    if let Some(theme) = settings.theme()? {
        config.ui.theme = theme;
    }
    if let Some(reduced) = settings.reduced_motion()? {
        config.ui.reduced_motion = reduced;
    }
    if let Some(profile) = settings.keymap_profile()? {
        config.keymap.profile = profile;
    }
    if let Some(profiles) = settings.get_setting(settings::KEY_KEYMAP_PROFILES)? {
        config.keymap.profiles = profiles;
    }
    if let Some(timeout_ms) = settings.get_setting(settings::KEY_DB_STATEMENT_TIMEOUT_MS)? {
        config.db.defaults.statement_timeout = Duration::from_millis(timeout_ms);
    }
    if let Some(page_size) = settings.get_setting(settings::KEY_DB_PAGE_SIZE)? {
        config.db.defaults.page_size = page_size;
    }
    if let Some(primary) = settings.get_setting(settings::KEY_DB_PRIMARY)? {
        config.db.primary = primary;
    }
    if let Some(model_dir) = settings.model_dir()? {
        config.search.model_dir = model_dir;
    }
    if let Some(dim) = settings.semantic_dim()? {
        config.search.semantic_dim = dim;
    }
    if let Some(engine) = settings.ann_engine()? {
        config.search.ann_engine = engine;
    }
    if let Some(rebuild) = settings.rebuild_on_start()? {
        config.search.rebuild_on_start = rebuild;
    }
    if let Some(batch) = settings.semantic_batch_size()? {
        config.search.semantic_batch_size = batch;
    }
    if let Some(top_k) = settings.semantic_top_k()? {
        config.search.semantic_top_k = top_k;
    }
    if let Some(threshold) = settings.get_setting(settings::KEY_SEARCH_SEMANTIC_SCORE_THRESHOLD)? {
        config.search.semantic_score_threshold = threshold;
    }
    if let Some(title) = settings.get_setting(settings::KEY_SEARCH_SEMANTIC_TITLE_COLUMN)? {
        config.search.semantic_title_column = title;
    }
    if let Some(preference) = settings.semantic_runtime_preference()? {
        config.search.semantic_runtime_preference = preference;
    }
    Ok(())
}

fn duration_millis(duration: Duration) -> Result<u64> {
    u64::try_from(duration.as_millis()).context("statement timeout exceeds supported milliseconds")
}
