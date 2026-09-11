#![allow(clippy::missing_errors_doc)]

use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{de::DeserializeOwned, Serialize};

use crate::{encryption, Store};

type StoredSettingRow = (String, Option<Vec<u8>>, Option<Vec<u8>>);

pub const KEY_UI_THEME: &str = "ui.theme";
pub const KEY_UI_REDUCED_MOTION: &str = "ui.reduced_motion";
pub const KEY_KEYMAP_PROFILE: &str = "keymap.profile";
pub const KEY_KEYMAP_PROFILES: &str = "keymap.profiles";
pub const KEY_DB_STATEMENT_TIMEOUT_MS: &str = "db.defaults.statement_timeout_ms";
pub const KEY_DB_PAGE_SIZE: &str = "db.defaults.page_size";
pub const KEY_DB_PRIMARY: &str = "db.primary";
pub const KEY_CONFIG_TOML_IMPORT_COMPLETE: &str = "config.toml_import_complete";
pub const KEY_UI_MAIN_TICK_RATE_MS: &str = "ui.main_tick_rate_ms";
pub const KEY_UI_MENU_TICK_RATE_MS: &str = "ui.menu_tick_rate_ms";
pub const KEY_UI_FAST_SCROLL_STEP: &str = "ui.fast_scroll_step";
pub const KEY_UI_MOUSE_SCROLL_LINES: &str = "ui.mouse_scroll_lines";
pub const KEY_UI_SELECT_TOP_LIMIT: &str = "ui.select_top_limit";
pub const KEY_UI_STATUS_EXPIRE_SECS: &str = "ui.status_expire_secs";
pub const KEY_UI_STATUS_AUTO_CLEAR_SECS: &str = "ui.status_auto_clear_secs";
pub const KEY_UI_MIN_COLUMN_WIDTH: &str = "ui.min_column_width";
pub const KEY_SEARCH_MODEL_DIR: &str = "search.model_dir";
pub const KEY_SEARCH_SEMANTIC_DIM: &str = "search.semantic_dim";
pub const KEY_SEARCH_ANN_ENGINE: &str = "search.ann_engine";
pub const KEY_SEARCH_REBUILD_ON_START: &str = "search.rebuild_on_start";
pub const KEY_SEARCH_SEMANTIC_BATCH_SIZE: &str = "search.semantic_batch_size";
pub const KEY_SEARCH_SEMANTIC_TOP_K: &str = "search.semantic_top_k";
pub const KEY_SEARCH_SEMANTIC_SCORE_THRESHOLD: &str = "search.semantic_score_threshold";
pub const KEY_SEARCH_SEMANTIC_TITLE_COLUMN: &str = "search.semantic_title_column";
pub const KEY_SEARCH_SEMANTIC_RUNTIME_PREFERENCE: &str = "search.semantic_runtime_preference";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SemanticRuntimePreference {
    #[default]
    Off,
    Auto,
    Gpu,
    Cpu,
}

#[derive(Debug)]
pub struct SettingsStore<'a> {
    store: &'a Store,
}

impl<'a> SettingsStore<'a> {
    pub(crate) fn new(store: &'a Store) -> Self {
        Self { store }
    }

    pub fn get_setting<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let connection = self.store.open_connection()?;
        let row: Option<StoredSettingRow> = connection
            .query_row(
                "SELECT value, value_nonce, value_ciphertext
                 FROM settings WHERE key = ?1 LIMIT 1",
                params![key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .with_context(|| format!("failed to fetch setting {key}"))?;
        let Some((value, nonce, ciphertext)) = row else {
            return Ok(None);
        };
        let raw = match (nonce, ciphertext) {
            (None, None) => value,
            (Some(nonce), Some(ciphertext)) if key == KEY_DB_PRIMARY => {
                let context = encryption::read_context(self.store, &connection)?;
                context.decrypt_setting(key, &nonce, &ciphertext)?
            }
            (Some(_), Some(_)) => anyhow::bail!("unexpected encrypted setting {key}"),
            _ => anyhow::bail!("stored setting encryption envelope is incomplete"),
        };
        serde_json::from_str(&raw)
            .with_context(|| format!("failed to deserialize setting {key}"))
            .map(Some)
    }

    pub fn set_setting<T: Serialize + ?Sized>(&self, key: &str, value: &T) -> Result<()> {
        let serialized = serde_json::to_string(value)
            .with_context(|| format!("failed to serialize setting {key}"))?;
        let mut connection = self.store.open_connection()?;
        let now = Utc::now().to_rfc3339();
        if key == KEY_DB_PRIMARY && serialized != "null" {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .context("failed to start encrypted primary connection write")?;
            let context = encryption::write_context(self.store, &transaction)?;
            let encrypted = context.encrypt_setting(key, &serialized)?;
            transaction
                .execute(
                    "INSERT INTO settings
                     (key, value, value_nonce, value_ciphertext, updated_at)
                     VALUES (?1, '', ?2, ?3, ?4)
                     ON CONFLICT(key) DO UPDATE SET value = '',
                       value_nonce = excluded.value_nonce,
                       value_ciphertext = excluded.value_ciphertext,
                       updated_at = excluded.updated_at",
                    params![key, encrypted.nonce, encrypted.ciphertext, now],
                )
                .with_context(|| format!("failed to persist setting {key}"))?;
            transaction
                .commit()
                .context("failed to commit encrypted primary connection write")?;
        } else {
            connection
                .execute(
                    "INSERT INTO settings
                     (key, value, value_nonce, value_ciphertext, updated_at)
                     VALUES (?1, ?2, NULL, NULL, ?3)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value,
                       value_nonce = NULL, value_ciphertext = NULL,
                       updated_at = excluded.updated_at",
                    params![key, serialized, now],
                )
                .with_context(|| format!("failed to persist setting {key}"))?;
        }
        Ok(())
    }

    pub fn ensure_setting<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        if self.setting_exists(key)? {
            return Ok(());
        }
        self.set_setting(key, value)
    }

    pub fn is_empty(&self) -> Result<bool> {
        let connection = self.store.open_connection()?;
        let found: Option<i64> = connection
            .query_row("SELECT 1 FROM settings LIMIT 1", [], |row| row.get(0))
            .optional()
            .context("failed to inspect settings state")?;
        Ok(found.is_none())
    }

    fn setting_exists(&self, key: &str) -> Result<bool> {
        let connection = self.store.open_connection()?;
        let found: Option<i64> = connection
            .query_row(
                "SELECT 1 FROM settings WHERE key = ?1 LIMIT 1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .with_context(|| format!("failed to check setting existence for {key}"))?;
        Ok(found.is_some())
    }

    pub fn theme(&self) -> Result<Option<String>> {
        self.get_setting(KEY_UI_THEME)
    }

    pub fn set_theme(&self, theme: impl AsRef<str>) -> Result<()> {
        self.set_setting(KEY_UI_THEME, theme.as_ref())
    }

    pub fn reduced_motion(&self) -> Result<Option<bool>> {
        self.get_setting(KEY_UI_REDUCED_MOTION)
    }

    pub fn set_reduced_motion(&self, value: bool) -> Result<()> {
        self.set_setting(KEY_UI_REDUCED_MOTION, &value)
    }

    pub fn keymap_profile(&self) -> Result<Option<String>> {
        self.get_setting(KEY_KEYMAP_PROFILE)
    }

    pub fn set_keymap_profile(&self, value: impl AsRef<str>) -> Result<()> {
        self.set_setting(KEY_KEYMAP_PROFILE, value.as_ref())
    }

    pub fn config_toml_import_complete(&self) -> Result<bool> {
        Ok(self
            .get_setting(KEY_CONFIG_TOML_IMPORT_COMPLETE)?
            .unwrap_or(false))
    }

    pub fn set_config_toml_import_complete(&self) -> Result<()> {
        self.set_setting(KEY_CONFIG_TOML_IMPORT_COMPLETE, &true)
    }

    pub fn semantic_runtime_preference(&self) -> Result<Option<SemanticRuntimePreference>> {
        self.get_setting(KEY_SEARCH_SEMANTIC_RUNTIME_PREFERENCE)
    }

    pub fn set_semantic_runtime_preference(&self, value: SemanticRuntimePreference) -> Result<()> {
        self.set_setting(KEY_SEARCH_SEMANTIC_RUNTIME_PREFERENCE, &value)
    }

    pub fn semantic_batch_size(&self) -> Result<Option<usize>> {
        self.get_setting(KEY_SEARCH_SEMANTIC_BATCH_SIZE)
    }

    pub fn set_semantic_batch_size(&self, value: usize) -> Result<()> {
        self.set_setting(KEY_SEARCH_SEMANTIC_BATCH_SIZE, &value)
    }

    pub fn semantic_top_k(&self) -> Result<Option<usize>> {
        self.get_setting(KEY_SEARCH_SEMANTIC_TOP_K)
    }

    pub fn set_semantic_top_k(&self, value: usize) -> Result<()> {
        self.set_setting(KEY_SEARCH_SEMANTIC_TOP_K, &value)
    }

    pub fn semantic_score_threshold(&self) -> Result<Option<f32>> {
        let stored: Option<Option<f32>> = self.get_setting(KEY_SEARCH_SEMANTIC_SCORE_THRESHOLD)?;
        Ok(stored.unwrap_or(None))
    }

    pub fn set_semantic_score_threshold(&self, value: Option<f32>) -> Result<()> {
        self.set_setting(KEY_SEARCH_SEMANTIC_SCORE_THRESHOLD, &value)
    }

    pub fn semantic_title_column(&self) -> Result<Option<String>> {
        let stored: Option<Option<String>> = self.get_setting(KEY_SEARCH_SEMANTIC_TITLE_COLUMN)?;
        Ok(stored.unwrap_or(None))
    }

    pub fn set_semantic_title_column(&self, value: Option<String>) -> Result<()> {
        let normalized = value;
        self.set_setting(KEY_SEARCH_SEMANTIC_TITLE_COLUMN, &normalized)
    }

    pub fn model_dir(&self) -> Result<Option<PathBuf>> {
        self.get_setting(KEY_SEARCH_MODEL_DIR)
    }

    pub fn set_model_dir(&self, value: impl Into<PathBuf>) -> Result<()> {
        let path: PathBuf = value.into();
        self.set_setting(KEY_SEARCH_MODEL_DIR, &path)
    }

    pub fn semantic_dim(&self) -> Result<Option<usize>> {
        self.get_setting(KEY_SEARCH_SEMANTIC_DIM)
    }

    pub fn set_semantic_dim(&self, value: usize) -> Result<()> {
        self.set_setting(KEY_SEARCH_SEMANTIC_DIM, &value)
    }

    pub fn ann_engine(&self) -> Result<Option<String>> {
        self.get_setting(KEY_SEARCH_ANN_ENGINE)
    }

    pub fn set_ann_engine(&self, value: impl AsRef<str>) -> Result<()> {
        self.set_setting(KEY_SEARCH_ANN_ENGINE, value.as_ref())
    }

    pub fn rebuild_on_start(&self) -> Result<Option<bool>> {
        self.get_setting(KEY_SEARCH_REBUILD_ON_START)
    }

    pub fn set_rebuild_on_start(&self, value: bool) -> Result<()> {
        self.set_setting(KEY_SEARCH_REBUILD_ON_START, &value)
    }

    pub fn main_tick_rate_ms(&self) -> Result<Option<u64>> {
        self.get_setting(KEY_UI_MAIN_TICK_RATE_MS)
    }

    pub fn set_main_tick_rate_ms(&self, value: u64) -> Result<()> {
        self.set_setting(KEY_UI_MAIN_TICK_RATE_MS, &value)
    }

    pub fn menu_tick_rate_ms(&self) -> Result<Option<u64>> {
        self.get_setting(KEY_UI_MENU_TICK_RATE_MS)
    }

    pub fn set_menu_tick_rate_ms(&self, value: u64) -> Result<()> {
        self.set_setting(KEY_UI_MENU_TICK_RATE_MS, &value)
    }

    pub fn fast_scroll_step(&self) -> Result<Option<usize>> {
        self.get_setting(KEY_UI_FAST_SCROLL_STEP)
    }

    pub fn set_fast_scroll_step(&self, value: usize) -> Result<()> {
        self.set_setting(KEY_UI_FAST_SCROLL_STEP, &value)
    }

    pub fn mouse_scroll_lines(&self) -> Result<Option<usize>> {
        self.get_setting(KEY_UI_MOUSE_SCROLL_LINES)
    }

    pub fn set_mouse_scroll_lines(&self, value: usize) -> Result<()> {
        self.set_setting(KEY_UI_MOUSE_SCROLL_LINES, &value)
    }

    pub fn select_top_limit(&self) -> Result<Option<u32>> {
        self.get_setting(KEY_UI_SELECT_TOP_LIMIT)
    }

    pub fn set_select_top_limit(&self, value: u32) -> Result<()> {
        self.set_setting(KEY_UI_SELECT_TOP_LIMIT, &value)
    }

    pub fn status_expire_secs(&self) -> Result<Option<u64>> {
        self.get_setting(KEY_UI_STATUS_EXPIRE_SECS)
    }

    pub fn set_status_expire_secs(&self, value: u64) -> Result<()> {
        self.set_setting(KEY_UI_STATUS_EXPIRE_SECS, &value)
    }

    pub fn status_auto_clear_secs(&self) -> Result<Option<u64>> {
        self.get_setting(KEY_UI_STATUS_AUTO_CLEAR_SECS)
    }

    pub fn set_status_auto_clear_secs(&self, value: u64) -> Result<()> {
        self.set_setting(KEY_UI_STATUS_AUTO_CLEAR_SECS, &value)
    }

    pub fn min_column_width(&self) -> Result<Option<u16>> {
        self.get_setting(KEY_UI_MIN_COLUMN_WIDTH)
    }

    pub fn set_min_column_width(&self, value: u16) -> Result<()> {
        self.set_setting(KEY_UI_MIN_COLUMN_WIDTH, &value)
    }
}
