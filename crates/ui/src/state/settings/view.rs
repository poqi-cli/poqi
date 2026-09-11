use anyhow::Result;
use poqi_config::AppConfig;
use poqi_store::Store;

use crate::UiRuntimeSettings;

use super::{
    fields::{FieldKind, SettingField, SettingsAction, SettingsRow},
    initial::InitialSettings,
    updates::{collect_updates, SettingUpdate},
};

/// In-memory state for the Settings screen, including selection and edits.
#[derive(Debug, Clone)]
pub(crate) struct SettingsView {
    rows: Vec<SettingsRow>,
    pub(crate) selected: usize,
    pub(crate) editing: bool,
    editing_buffer: String,
    pub(crate) dirty: bool,
}

impl SettingsView {
    pub(crate) fn new(
        store: &Store,
        config: &AppConfig,
        ui_settings: UiRuntimeSettings,
    ) -> Result<Self> {
        let repo = store.settings();
        let initial = InitialSettings::load(&repo, config, ui_settings)?;
        let rows = initial.build_rows();
        let mut view = Self {
            rows,
            selected: 0,
            editing: false,
            editing_buffer: String::new(),
            dirty: false,
        };
        view.selected = view.default_selection();
        view.refresh_dirty();
        Ok(view)
    }

    pub(crate) fn rows(&self) -> &[SettingsRow] {
        &self.rows
    }

    pub(crate) fn move_selection(&mut self, forward: bool, steps: usize) {
        if steps == 0 || self.rows.is_empty() {
            return;
        }
        let last = self.rows.len() - 1;
        for _ in 0..steps {
            if forward {
                self.selected = if self.selected == last {
                    0
                } else {
                    self.selected + 1
                };
            } else {
                self.selected = if self.selected == 0 {
                    last
                } else {
                    self.selected - 1
                };
            }
        }
    }

    pub(crate) fn selected_kind(&self) -> Option<FieldKind> {
        match self.rows.get(self.selected) {
            Some(SettingsRow::Field(field)) => Some(field.kind()),
            _ => None,
        }
    }

    pub(crate) fn is_save_selected(&self) -> bool {
        matches!(
            self.rows.get(self.selected),
            Some(SettingsRow::Action(SettingsAction::SaveAndExit))
        )
    }

    pub(crate) fn scroll_offset(&self, viewport_rows: usize) -> usize {
        if viewport_rows == 0 {
            return 0;
        }
        let body_len = self.rows.len().saturating_sub(1);
        if body_len == 0 {
            return 0;
        }
        let viewport = viewport_rows.max(1);
        let max_offset = body_len.saturating_sub(viewport);
        let selected = self.selected.min(self.rows.len().saturating_sub(1));
        if selected >= body_len {
            return max_offset;
        }
        let offset = selected.saturating_add(1).saturating_sub(viewport);
        offset.min(max_offset)
    }

    pub(crate) fn nudge_current(&mut self, forward: bool) {
        let Some(field) = self.selected_field_mut() else {
            return;
        };
        field.nudge(forward);
        self.refresh_dirty();
    }

    pub(crate) fn begin_edit(&mut self) -> bool {
        let Some(row) = self.rows.get(self.selected) else {
            return false;
        };
        let SettingsRow::Field(field) = row else {
            return false;
        };
        if !field.is_editable() {
            return false;
        }
        let snapshot = field.clone();
        self.editing = true;
        self.editing_buffer = match snapshot {
            SettingField::Number(field) => field.value.to_string(),
            SettingField::Float(field) => field
                .value
                .map(|value| format!("{value:.2}"))
                .unwrap_or_default(),
            SettingField::Text(field) => field.value.clone().unwrap_or_default(),
            SettingField::Choice(_) => unreachable!(),
        };
        true
    }

    pub(crate) fn push_char(&mut self, ch: char) {
        if self.editing {
            self.editing_buffer.push(ch);
        }
    }

    pub(crate) fn backspace(&mut self) {
        if self.editing {
            self.editing_buffer.pop();
        }
    }

    pub(crate) fn editing_buffer(&self) -> &str {
        &self.editing_buffer
    }

    pub(crate) fn cancel_edit(&mut self) {
        self.editing = false;
        self.editing_buffer.clear();
    }

    pub(crate) fn commit_edit(&mut self) -> Result<(), String> {
        let input = self.editing_buffer.clone();
        let Some(field) = self.selected_field_mut() else {
            self.cancel_edit();
            return Ok(());
        };
        match field {
            SettingField::Number(field) => {
                let parsed: u64 = input
                    .trim()
                    .parse()
                    .map_err(|_| "Enter a positive integer".to_string())?;
                if parsed < field.min {
                    return Err(format!("Minimum value is {}", field.min));
                }
                field.value = parsed;
            }
            SettingField::Float(field) => {
                if input.trim().is_empty() {
                    field.value = None;
                } else {
                    let parsed: f32 = input
                        .trim()
                        .parse()
                        .map_err(|_| "Enter a number between -1.0 and 1.0".to_string())?;
                    if !(field.min..=field.max).contains(&parsed) {
                        return Err("Threshold must stay between -1 and 1".to_string());
                    }
                    field.value = Some(parsed);
                }
            }
            SettingField::Text(field) => {
                let value = input.trim();
                field.value = if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                };
            }
            SettingField::Choice(_) => {}
        }
        self.editing = false;
        self.editing_buffer.clear();
        self.refresh_dirty();
        Ok(())
    }

    pub(crate) fn pending_updates(&self) -> Vec<SettingUpdate> {
        collect_updates(&self.rows)
    }

    fn selected_field_mut(&mut self) -> Option<&mut SettingField> {
        match self.rows.get_mut(self.selected) {
            Some(SettingsRow::Field(field)) => Some(field),
            _ => None,
        }
    }

    fn default_selection(&self) -> usize {
        self.rows
            .iter()
            .position(|row| matches!(row, SettingsRow::Field(_)))
            .unwrap_or(0)
    }

    fn refresh_dirty(&mut self) {
        self.dirty = self.rows.iter().any(SettingsRow::is_modified);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_store() -> (Store, TempDir) {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("store.sqlite");
        let store = Store::new_with_test_key(Some(path), [7; 32]);
        store.init().expect("init store");
        (store, dir)
    }

    #[test]
    fn navigation_moves_selection() {
        let (store, _dir) = test_store();
        let config = AppConfig::default();
        let ui_settings = UiRuntimeSettings::default();
        let mut view = SettingsView::new(&store, &config, ui_settings).expect("view");
        assert_eq!(view.selected, 0);
        view.move_selection(true, 2);
        assert_eq!(view.selected, 2);
        view.move_selection(false, 1);
        assert_eq!(view.selected, 1);
    }

    #[test]
    fn navigation_wraps_from_top_to_save_action() {
        let (store, _dir) = test_store();
        let config = AppConfig::default();
        let ui_settings = UiRuntimeSettings::default();
        let mut view = SettingsView::new(&store, &config, ui_settings).expect("view");
        let last_index = view.rows.len() - 1;
        view.selected = view.default_selection();
        view.move_selection(false, 1);
        assert_eq!(view.selected, last_index);
    }

    #[test]
    fn editing_number_updates_dirty_flag() {
        let (store, _dir) = test_store();
        let config = AppConfig::default();
        let ui_settings = UiRuntimeSettings::default();
        let mut view = SettingsView::new(&store, &config, ui_settings).expect("view");
        assert!(!view.dirty);
        view.selected = 1; // first numeric row (fast scroll)
        view.nudge_current(true);
        assert!(view.dirty);
    }
}
