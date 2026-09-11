use anyhow::{Context, Result};
use std::time::Duration;

use super::{App, FocusPanel, UiLayer};
use crate::{
    keymap::KeymapEngine,
    semantic_runtime::init_semantic_runtime_with_handles,
    state::settings::{SettingUpdate, SettingsView},
    theme::Theme,
    UiRuntimeSettings, UiRuntimeSettingsParts,
};
use poqi_search_semantic::SearchOptions;
use poqi_store::SemanticRuntimePreference;
use std::sync::Arc;

impl App {
    pub(super) fn open_settings_modal(&mut self) {
        if self.settings_view.is_some() {
            return;
        }
        match SettingsView::new(&self.store, &self.config, self.ui_settings) {
            Ok(view) => {
                self.settings_view = Some(view);
                self.layer = UiLayer::Settings;
                self.status.info(
                    "Settings: W/S or arrows to navigate · Enter to edit/save · Esc to cancel",
                );
            }
            Err(err) => self
                .status
                .error(format!("Failed to load settings: {err:#}")),
        }
    }

    pub(super) fn cancel_settings(&mut self) {
        self.settings_view = None;
        self.layer = UiLayer::PanelSelect;
        self.panel_cursor = FocusPanel::Status;
        self.status.info("Settings canceled");
    }

    pub(super) fn save_settings(&mut self) {
        let Some(view) = self.settings_view.as_ref() else {
            return;
        };
        let updates = view.pending_updates();
        let previous_runtime = self.config.search.semantic_runtime_preference;
        let requested_runtime = updates.iter().find_map(|update| match update {
            SettingUpdate::SemanticRuntime(value) => Some(*value),
            _ => None,
        });
        let repo = self.store.settings();
        if let Err(err) = persist_updates(&repo, &updates) {
            self.status
                .error(format!("Failed to save settings: {err:#}"));
            return;
        }
        self.apply_setting_updates(&updates);
        self.settings_view = None;
        self.layer = UiLayer::PanelSelect;
        self.panel_cursor = FocusPanel::Status;
        if updates.is_empty() {
            self.status.info("Settings unchanged");
        } else if let Some(requested) = requested_runtime {
            self.finish_semantic_runtime_update(previous_runtime, requested);
        } else {
            self.status.success("Settings saved");
        }
    }

    fn finish_semantic_runtime_update(
        &mut self,
        previous: SemanticRuntimePreference,
        requested: SemanticRuntimePreference,
    ) {
        if requested == SemanticRuntimePreference::Off {
            self.disable_semantic_runtime();
            self.status
                .success("Settings saved; semantic search is off");
        } else if previous == SemanticRuntimePreference::Off
            && !self
                .semantic_coordinator
                .has_different_enabled_preference(requested)
        {
            match self.start_semantic_runtime() {
                Ok(()) => self
                    .status
                    .success("Settings saved; semantic initialization started"),
                Err(reason) => self.status.error(format!(
                    "Settings saved; semantic initialization failed: {reason}"
                )),
            }
        } else {
            self.status
                .success("Settings saved; restart poqi to switch the semantic backend");
        }
    }

    fn start_semantic_runtime(&mut self) -> std::result::Result<(), String> {
        let bootstrap = init_semantic_runtime_with_handles(
            &self.config,
            &self.semantic_coordinator,
            Arc::clone(&self.semantic_handles),
        );
        let runtime = bootstrap.runtime;
        let failure = runtime.disabled_reason.clone();
        self.apply_semantic_runtime(runtime);
        failure.map_or(Ok(()), Err)
    }

    fn disable_semantic_runtime(&mut self) {
        if let Some(active) = self.active_semantic.take() {
            active
                .cancel_flag
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.semantic_tx = None;
        self.semantic_rx = None;
        self.semantic_events = None;
        let reason = "Semantic search is off. Enable it in Settings → Semantic runtime.";
        self.semantic_disabled_reason = Some(reason.to_string());
        self.semantic.set_disabled(reason);
    }

    fn apply_semantic_runtime(&mut self, runtime: super::SemanticRuntime) {
        self.semantic_tx = runtime.tx;
        self.semantic_rx = runtime.rx;
        self.semantic_events = runtime.events;
        self.semantic_options = runtime.options;
        self.semantic_title_column = runtime.title_column;
        self.semantic_disabled_reason = runtime.disabled_reason.clone();
        if let Some(reason) = runtime.disabled_reason {
            self.semantic.set_disabled(reason);
        } else if let Some(label) = runtime.initial_loading_label {
            self.semantic.set_loading(label);
        } else if let Some(summary) = runtime.initial_ready_summary {
            self.semantic.set_ready(summary);
        }
    }

    fn apply_setting_updates(&mut self, updates: &[SettingUpdate]) {
        if updates.is_empty() {
            return;
        }

        let mut ui_parts = self.current_ui_settings_parts();
        let mut ui_changed = false;
        let mut theme_dirty = false;
        let mut keymap_dirty = false;
        let mut semantic_dirty = false;

        for update in updates {
            self.apply_single_setting_update(
                update,
                &mut ui_parts,
                &mut ui_changed,
                &mut theme_dirty,
                &mut keymap_dirty,
                &mut semantic_dirty,
            );
        }

        if ui_changed {
            self.ui_settings = UiRuntimeSettings::from_parts(ui_parts);
            self.status.update_durations(
                self.ui_settings.status_auto_clear_duration(),
                self.ui_settings.status_expire_duration(),
            );
        }

        if theme_dirty {
            self.theme = Theme::from_config(&self.config);
        }

        if keymap_dirty {
            self.keymap =
                KeymapEngine::from_config(&self.config, self.ui_settings.fast_scroll_step())
                    .unwrap_or_else(|err| {
                        tracing::error!(
                        ?err,
                        "failed to rebuild keymap after settings update; falling back to defaults"
                    );
                        KeymapEngine::default_with_step(self.ui_settings.fast_scroll_step())
                    });
        }

        if semantic_dirty {
            self.semantic_options = SearchOptions {
                batch_size: self.config.search.semantic_batch_size,
                top_k: self.config.search.semantic_top_k,
                threshold: self.config.search.semantic_score_threshold,
                dim: self.config.search.semantic_dim,
            };
            self.semantic_title_column = self.config.search.semantic_title_column.clone();
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_single_setting_update(
        &mut self,
        update: &SettingUpdate,
        ui_parts: &mut UiRuntimeSettingsParts,
        ui_changed: &mut bool,
        theme_dirty: &mut bool,
        keymap_dirty: &mut bool,
        semantic_dirty: &mut bool,
    ) {
        match update {
            SettingUpdate::UiTheme(value) => {
                self.config.ui.theme.clone_from(value);
                *theme_dirty = true;
            }
            SettingUpdate::UiFastScrollStep(value) => {
                ui_parts.fast_scroll_step = *value;
                *ui_changed = true;
                *keymap_dirty = true;
            }
            SettingUpdate::UiMouseScrollLines(value) => {
                ui_parts.mouse_scroll_lines = *value;
                *ui_changed = true;
            }
            SettingUpdate::UiSelectTopLimit(value) => {
                ui_parts.select_top_limit = *value;
                *ui_changed = true;
            }
            SettingUpdate::UiMainTickRate(value) => {
                ui_parts.main_tick_rate_ms = *value;
                *ui_changed = true;
            }
            SettingUpdate::UiMenuTickRate(value) => {
                ui_parts.menu_tick_rate_ms = *value;
                *ui_changed = true;
            }
            SettingUpdate::UiStatusExpireSecs(value) => {
                ui_parts.status_expire_secs = *value;
                *ui_changed = true;
            }
            SettingUpdate::UiStatusAutoClearSecs(value) => {
                ui_parts.status_auto_clear_secs = *value;
                *ui_changed = true;
            }
            SettingUpdate::SemanticRuntime(value) => {
                self.config.search.semantic_runtime_preference = *value;
            }
            SettingUpdate::SemanticBatchSize(value) => {
                self.config.search.semantic_batch_size = *value;
                *semantic_dirty = true;
            }
            SettingUpdate::SemanticTopK(value) => {
                self.config.search.semantic_top_k = *value;
                *semantic_dirty = true;
            }
            SettingUpdate::SemanticThreshold(value) => {
                self.config.search.semantic_score_threshold = *value;
                *semantic_dirty = true;
            }
            SettingUpdate::SemanticTitle(value) => {
                self.config.search.semantic_title_column.clone_from(value);
                *semantic_dirty = true;
            }
            SettingUpdate::SemanticDim(value) => {
                self.config.search.semantic_dim = *value;
            }
            SettingUpdate::KeymapProfile(value) => {
                self.config.keymap.profile.clone_from(value);
                *keymap_dirty = true;
            }
        }
    }

    fn current_ui_settings_parts(&self) -> UiRuntimeSettingsParts {
        UiRuntimeSettingsParts {
            main_tick_rate_ms: duration_millis_to_u64(self.ui_settings.main_tick_rate()),
            menu_tick_rate_ms: duration_millis_to_u64(self.ui_settings.menu_tick_rate()),
            fast_scroll_step: self.ui_settings.fast_scroll_step(),
            mouse_scroll_lines: self.ui_settings.mouse_scroll_lines(),
            select_top_limit: self.ui_settings.select_top_limit(),
            status_expire_secs: self.ui_settings.status_expire_duration().as_secs().max(1),
            status_auto_clear_secs: self
                .ui_settings
                .status_auto_clear_duration()
                .as_secs()
                .max(1),
            min_column_width: self.ui_settings.min_column_width(),
        }
    }
}

fn persist_updates(repo: &poqi_store::SettingsStore<'_>, updates: &[SettingUpdate]) -> Result<()> {
    for update in updates {
        match update {
            SettingUpdate::UiTheme(value) => repo
                .set_theme(value)
                .context("failed to persist theme setting")?,
            SettingUpdate::UiFastScrollStep(value) => repo
                .set_fast_scroll_step(*value)
                .context("failed to persist fast-scroll step")?,
            SettingUpdate::UiMouseScrollLines(value) => repo
                .set_mouse_scroll_lines(*value)
                .context("failed to persist mouse-scroll lines")?,
            SettingUpdate::UiSelectTopLimit(value) => repo
                .set_select_top_limit(*value)
                .context("failed to persist select-top limit")?,
            SettingUpdate::UiMainTickRate(value) => repo
                .set_main_tick_rate_ms(*value)
                .context("failed to persist main tick rate")?,
            SettingUpdate::UiMenuTickRate(value) => repo
                .set_menu_tick_rate_ms(*value)
                .context("failed to persist menu tick rate")?,
            SettingUpdate::UiStatusExpireSecs(value) => repo
                .set_status_expire_secs(*value)
                .context("failed to persist status expiration time")?,
            SettingUpdate::UiStatusAutoClearSecs(value) => repo
                .set_status_auto_clear_secs(*value)
                .context("failed to persist status auto-clear time")?,
            SettingUpdate::SemanticRuntime(value) => {
                repo.set_semantic_runtime_preference(*value)
                    .context("failed to persist semantic runtime preference")?;
            }
            SettingUpdate::SemanticBatchSize(value) => repo
                .set_semantic_batch_size(*value)
                .context("failed to persist semantic batch size")?,
            SettingUpdate::SemanticTopK(value) => repo
                .set_semantic_top_k(*value)
                .context("failed to persist semantic top-k")?,
            SettingUpdate::SemanticThreshold(value) => repo
                .set_semantic_score_threshold(*value)
                .context("failed to persist semantic score threshold")?,
            SettingUpdate::SemanticTitle(value) => repo
                .set_semantic_title_column(value.clone())
                .context("failed to persist semantic title column")?,
            SettingUpdate::SemanticDim(value) => repo
                .set_semantic_dim(*value)
                .context("failed to persist semantic dimension")?,
            SettingUpdate::KeymapProfile(value) => repo
                .set_keymap_profile(value)
                .context("failed to persist keymap profile")?,
        }
    }
    Ok(())
}

fn duration_millis_to_u64(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
