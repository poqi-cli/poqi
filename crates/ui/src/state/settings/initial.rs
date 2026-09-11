use anyhow::Result;
use poqi_config::AppConfig;
use poqi_store::{SemanticRuntimePreference, SettingsStore};
use std::time::Duration;

use crate::{theme::Theme, UiRuntimeSettings};

use super::{
    fields::{
        ChoiceField, FloatField, NumberField, SettingField, SettingKey, SettingsAction,
        SettingsRow, TextField,
    },
    updates::preference_index,
};

/// Captures initial values from the store + config so the rows can be built once.
pub(crate) struct InitialSettings {
    theme_options: Vec<String>,
    theme_selected: usize,
    fast_scroll_step: u64,
    mouse_scroll_lines: u64,
    select_top_limit: u64,
    main_tick_rate_ms: u64,
    menu_tick_rate_ms: u64,
    status_expire_secs: u64,
    status_auto_clear_secs: u64,
    semantic_pref: SemanticRuntimePreference,
    semantic_batch_size: u64,
    semantic_top_k: u64,
    semantic_threshold: Option<f32>,
    semantic_title_column: Option<String>,
    semantic_dim: u64,
    keymap_profiles: Vec<String>,
    keymap_selected: usize,
}

impl InitialSettings {
    pub(crate) fn load(
        repo: &SettingsStore<'_>,
        config: &AppConfig,
        ui_settings: UiRuntimeSettings,
    ) -> Result<Self> {
        let theme = repo.theme()?.unwrap_or_else(|| config.ui.theme.clone());
        let fast_scroll = repo.fast_scroll_step()?.map_or_else(
            || u64_from_usize(ui_settings.fast_scroll_step()),
            u64_from_usize,
        );
        let mouse_scroll = repo.mouse_scroll_lines()?.map_or_else(
            || u64_from_usize(ui_settings.mouse_scroll_lines()),
            u64_from_usize,
        );
        let select_top = repo
            .select_top_limit()?
            .map_or_else(|| u64::from(ui_settings.select_top_limit()), u64::from);
        let main_tick = repo
            .main_tick_rate_ms()?
            .unwrap_or_else(|| duration_millis_to_u64(ui_settings.main_tick_rate()));
        let menu_tick = repo
            .menu_tick_rate_ms()?
            .unwrap_or_else(|| duration_millis_to_u64(ui_settings.menu_tick_rate()));
        let status_expire = repo
            .status_expire_secs()?
            .unwrap_or_else(|| ui_settings.status_expire_duration().as_secs().max(1));
        let status_auto_clear = repo
            .status_auto_clear_secs()?
            .unwrap_or_else(|| ui_settings.status_auto_clear_duration().as_secs().max(1));
        let semantic_pref = repo
            .semantic_runtime_preference()?
            .unwrap_or(config.search.semantic_runtime_preference);
        let semantic_batch = repo.semantic_batch_size()?.map_or_else(
            || u64_from_usize(config.search.semantic_batch_size),
            u64_from_usize,
        );
        let semantic_top_k = repo.semantic_top_k()?.map_or_else(
            || u64_from_usize(config.search.semantic_top_k),
            u64_from_usize,
        );
        let semantic_threshold = repo.semantic_score_threshold()?;
        let semantic_title = repo.semantic_title_column()?;
        let semantic_dim = repo.semantic_dim()?.map_or_else(
            || u64_from_usize(config.search.semantic_dim),
            u64_from_usize,
        );
        let keymap_profile = repo
            .keymap_profile()?
            .unwrap_or_else(|| config.keymap.profile.clone());
        let mut keymap_profiles = config.keymap.profiles.keys().cloned().collect::<Vec<_>>();
        if !keymap_profiles
            .iter()
            .any(|profile| profile == &keymap_profile)
        {
            keymap_profiles.push(keymap_profile.clone());
        }
        keymap_profiles.sort();
        let keymap_selected = keymap_profiles
            .iter()
            .position(|profile| profile == &keymap_profile)
            .unwrap_or(0);
        let mut theme_options = Theme::names()
            .iter()
            .map(|name| (*name).to_string())
            .collect::<Vec<_>>();
        if !theme_options.iter().any(|option| option == &theme) {
            theme_options.push(theme.clone());
        }
        let theme_selected = theme_options
            .iter()
            .position(|option| option == &theme)
            .unwrap_or(0);

        Ok(Self {
            theme_options,
            theme_selected,
            fast_scroll_step: fast_scroll,
            mouse_scroll_lines: mouse_scroll,
            select_top_limit: select_top,
            main_tick_rate_ms: main_tick,
            menu_tick_rate_ms: menu_tick,
            status_expire_secs: status_expire,
            status_auto_clear_secs: status_auto_clear,
            semantic_pref,
            semantic_batch_size: semantic_batch,
            semantic_top_k,
            semantic_threshold,
            semantic_title_column: semantic_title,
            semantic_dim,
            keymap_profiles,
            keymap_selected,
        })
    }

    pub(crate) fn build_rows(&self) -> Vec<SettingsRow> {
        let mut rows = Vec::new();
        rows.extend(self.ui_rows());
        rows.extend(self.semantic_rows());
        rows.push(SettingsRow::Field(SettingField::Choice(ChoiceField {
            key: SettingKey::KeymapProfile,
            label: "Keymap profile".to_string(),
            options: self.keymap_profiles.clone(),
            selected: self.keymap_selected,
            original_selected: self.keymap_selected,
        })));
        rows.push(SettingsRow::Action(SettingsAction::SaveAndExit));
        rows
    }

    fn ui_rows(&self) -> Vec<SettingsRow> {
        vec![
            SettingsRow::Field(SettingField::Choice(ChoiceField {
                key: SettingKey::UiTheme,
                label: "Theme".to_string(),
                options: self.theme_options.clone(),
                selected: self.theme_selected,
                original_selected: self.theme_selected,
            })),
            SettingsRow::Field(SettingField::Number(NumberField {
                key: SettingKey::UiFastScrollStep,
                label: "Fast scroll step".to_string(),
                value: self.fast_scroll_step,
                original_value: self.fast_scroll_step,
                min: 1,
            })),
            SettingsRow::Field(SettingField::Number(NumberField {
                key: SettingKey::UiMouseScrollLines,
                label: "Mouse scroll lines".to_string(),
                value: self.mouse_scroll_lines,
                original_value: self.mouse_scroll_lines,
                min: 1,
            })),
            SettingsRow::Field(SettingField::Number(NumberField {
                key: SettingKey::UiSelectTopLimit,
                label: "Select-top limit".to_string(),
                value: self.select_top_limit,
                original_value: self.select_top_limit,
                min: 1,
            })),
            SettingsRow::Field(SettingField::Number(NumberField {
                key: SettingKey::UiMainTickRate,
                label: "Main tick (ms)".to_string(),
                value: self.main_tick_rate_ms,
                original_value: self.main_tick_rate_ms,
                min: 1,
            })),
            SettingsRow::Field(SettingField::Number(NumberField {
                key: SettingKey::UiMenuTickRate,
                label: "Menu tick (ms)".to_string(),
                value: self.menu_tick_rate_ms,
                original_value: self.menu_tick_rate_ms,
                min: 1,
            })),
            SettingsRow::Field(SettingField::Number(NumberField {
                key: SettingKey::UiStatusExpireSecs,
                label: "Status expire (s)".to_string(),
                value: self.status_expire_secs,
                original_value: self.status_expire_secs,
                min: 1,
            })),
            SettingsRow::Field(SettingField::Number(NumberField {
                key: SettingKey::UiStatusAutoClearSecs,
                label: "Status auto-clear (s)".to_string(),
                value: self.status_auto_clear_secs,
                original_value: self.status_auto_clear_secs,
                min: 1,
            })),
        ]
    }

    fn semantic_rows(&self) -> Vec<SettingsRow> {
        vec![
            SettingsRow::Field(SettingField::Choice(ChoiceField {
                key: SettingKey::SearchSemanticRuntimePreference,
                label: "Semantic runtime".to_string(),
                options: vec![
                    "off".to_string(),
                    "auto".to_string(),
                    "gpu".to_string(),
                    "cpu".to_string(),
                ],
                selected: preference_index(self.semantic_pref),
                original_selected: preference_index(self.semantic_pref),
            })),
            SettingsRow::Field(SettingField::Number(NumberField {
                key: SettingKey::SearchSemanticBatchSize,
                label: "Semantic batch size".to_string(),
                value: self.semantic_batch_size,
                original_value: self.semantic_batch_size,
                min: 1,
            })),
            SettingsRow::Field(SettingField::Number(NumberField {
                key: SettingKey::SearchSemanticTopK,
                label: "Semantic top-k".to_string(),
                value: self.semantic_top_k,
                original_value: self.semantic_top_k,
                min: 1,
            })),
            SettingsRow::Field(SettingField::Float(FloatField {
                key: SettingKey::SearchSemanticScoreThreshold,
                label: "Semantic score threshold".to_string(),
                value: self.semantic_threshold,
                original_value: self.semantic_threshold,
                min: -1.0,
                max: 1.0,
            })),
            SettingsRow::Field(SettingField::Text(TextField {
                key: SettingKey::SearchSemanticTitleColumn,
                label: "Semantic title column".to_string(),
                value: self.semantic_title_column.clone(),
                original_value: self.semantic_title_column.clone(),
            })),
            SettingsRow::Field(SettingField::Number(NumberField {
                key: SettingKey::SearchSemanticDim,
                label: "Semantic dimension (future ANN)".to_string(),
                value: self.semantic_dim,
                original_value: self.semantic_dim,
                min: 1,
            })),
        ]
    }
}

fn duration_millis_to_u64(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn u64_from_usize(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
