use poqi_store::SemanticRuntimePreference;

use super::fields::{SettingField, SettingKey, SettingsRow};

#[derive(Debug, Clone)]
pub(crate) enum SettingUpdate {
    UiTheme(String),
    UiFastScrollStep(usize),
    UiMouseScrollLines(usize),
    UiSelectTopLimit(u32),
    UiMainTickRate(u64),
    UiMenuTickRate(u64),
    UiStatusExpireSecs(u64),
    UiStatusAutoClearSecs(u64),
    SemanticRuntime(SemanticRuntimePreference),
    SemanticBatchSize(usize),
    SemanticTopK(usize),
    SemanticThreshold(Option<f32>),
    SemanticTitle(Option<String>),
    SemanticDim(usize),
    KeymapProfile(String),
}

/// Translate modified rows into typed updates for app + persistence layers.
pub(crate) fn collect_updates(rows: &[SettingsRow]) -> Vec<SettingUpdate> {
    let mut updates = Vec::new();
    for row in rows {
        let SettingsRow::Field(field) = row else {
            continue;
        };
        if !field.is_modified() {
            continue;
        }
        match field {
            SettingField::Choice(field) => {
                let value = field.current_label();
                match field.key {
                    SettingKey::UiTheme => updates.push(SettingUpdate::UiTheme(value)),
                    SettingKey::SearchSemanticRuntimePreference => {
                        updates.push(SettingUpdate::SemanticRuntime(preference_from_label(
                            &value,
                        )));
                    }
                    SettingKey::KeymapProfile => {
                        updates.push(SettingUpdate::KeymapProfile(value));
                    }
                    _ => {}
                }
            }
            SettingField::Number(field) => match field.key {
                SettingKey::UiFastScrollStep => {
                    updates.push(SettingUpdate::UiFastScrollStep(usize_from_u64(field.value)));
                }
                SettingKey::UiMouseScrollLines => updates.push(SettingUpdate::UiMouseScrollLines(
                    usize_from_u64(field.value),
                )),
                SettingKey::UiSelectTopLimit => {
                    updates.push(SettingUpdate::UiSelectTopLimit(u32_from_u64(field.value)));
                }
                SettingKey::UiMainTickRate => {
                    updates.push(SettingUpdate::UiMainTickRate(field.value));
                }
                SettingKey::UiMenuTickRate => {
                    updates.push(SettingUpdate::UiMenuTickRate(field.value));
                }
                SettingKey::UiStatusExpireSecs => {
                    updates.push(SettingUpdate::UiStatusExpireSecs(field.value));
                }
                SettingKey::UiStatusAutoClearSecs => {
                    updates.push(SettingUpdate::UiStatusAutoClearSecs(field.value));
                }
                SettingKey::SearchSemanticBatchSize => {
                    updates.push(SettingUpdate::SemanticBatchSize(usize_from_u64(
                        field.value,
                    )));
                }
                SettingKey::SearchSemanticTopK => {
                    updates.push(SettingUpdate::SemanticTopK(usize_from_u64(field.value)));
                }
                SettingKey::SearchSemanticDim => {
                    updates.push(SettingUpdate::SemanticDim(usize_from_u64(field.value)));
                }
                _ => {}
            },
            SettingField::Float(field) => {
                if matches!(field.key, SettingKey::SearchSemanticScoreThreshold) {
                    updates.push(SettingUpdate::SemanticThreshold(field.value));
                }
            }
            SettingField::Text(field) => {
                if matches!(field.key, SettingKey::SearchSemanticTitleColumn) {
                    updates.push(SettingUpdate::SemanticTitle(field.value.clone()));
                }
            }
        }
    }
    updates
}

pub(crate) const fn preference_index(value: SemanticRuntimePreference) -> usize {
    match value {
        SemanticRuntimePreference::Off => 0,
        SemanticRuntimePreference::Auto => 1,
        SemanticRuntimePreference::Gpu => 2,
        SemanticRuntimePreference::Cpu => 3,
    }
}

pub(crate) fn preference_from_label(label: &str) -> SemanticRuntimePreference {
    match label {
        "off" => SemanticRuntimePreference::Off,
        "gpu" => SemanticRuntimePreference::Gpu,
        "cpu" => SemanticRuntimePreference::Cpu,
        _ => SemanticRuntimePreference::Auto,
    }
}

fn usize_from_u64(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn u32_from_u64(value: u64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
