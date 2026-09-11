use anyhow::{Context, Result};
use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use poqi_ui::point_in_rect;

use super::{
    form::{CreationField, ProfileForm},
    state::{App, ProfileSelectionResult, StatusKind, UiState},
};

impl App {
    pub(super) fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        if matches!(key.kind, KeyEventKind::Release | KeyEventKind::Repeat) {
            return Ok(false);
        }
        if matches!(self.state, UiState::ProfileCreation(_))
            && !self.form_size_ok
            && key.code != KeyCode::Esc
            && !is_quit(key)
        {
            return Ok(false);
        }
        match &mut self.state {
            UiState::ProfileSelection => self.handle_selection_key(key),
            UiState::ProfileCreation(form) => {
                let action = handle_form_key(key, form, &self.profiles)?;
                match action {
                    FormAction::Continue => Ok(false),
                    FormAction::Back => {
                        self.state = UiState::ProfileSelection;
                        Ok(false)
                    }
                    FormAction::Finish(result) => {
                        self.result = Some(result);
                        Ok(true)
                    }
                }
            }
            UiState::TestDataGeneration { progress, .. } => {
                if progress.is_some() {
                    Ok(false)
                } else {
                    self.handle_test_generation_key(key)
                }
            }
        }
    }

    fn handle_selection_key(&mut self, key: KeyEvent) -> Result<bool> {
        if is_quit(key) {
            return Err(super::super::SelectionQuit.into());
        }
        let total = self.profiles.len() + 2;
        match key.code {
            KeyCode::Esc => self.cancel_selection(),
            KeyCode::Delete => {
                self.delete_selected_profile()?;
                Ok(false)
            }
            KeyCode::Up | KeyCode::Char('w' | 'a') => {
                self.selected = self.selected.checked_sub(1).unwrap_or(total - 1);
                Ok(false)
            }
            KeyCode::Down | KeyCode::Char('s' | 'd') => {
                self.selected = (self.selected + 1) % total;
                Ok(false)
            }
            KeyCode::Char('e' | 'E') => {
                self.edit_selected_profile();
                Ok(false)
            }
            KeyCode::Enter | KeyCode::Char('f') => Ok(self.activate_selected()),
            _ => Ok(false),
        }
    }

    fn cancel_selection(&self) -> Result<bool> {
        if self.from_main_ui {
            Ok(false)
        } else {
            Err(super::super::SelectionCancelled.into())
        }
    }

    fn activate_selected(&mut self) -> bool {
        if self.selected == self.profiles.len() {
            self.state = UiState::ProfileCreation(Box::new(ProfileForm::new()));
            false
        } else if self.selected == self.profiles.len() + 1 {
            self.state = UiState::TestDataGeneration {
                progress: None,
                error: None,
            };
            false
        } else {
            self.result = Some(ProfileSelectionResult::Selected {
                profile: self.profiles[self.selected].clone(),
            });
            true
        }
    }

    fn edit_selected_profile(&mut self) {
        let Some(profile) = self.profiles.get(self.selected) else {
            self.set_status("Select a saved profile to edit", StatusKind::Error);
            return;
        };
        self.state = UiState::ProfileCreation(Box::new(ProfileForm::from_profile(
            profile.name.clone(),
            profile.uri.clone(),
            true,
            None,
        )));
    }

    fn handle_test_generation_key(&mut self, key: KeyEvent) -> Result<bool> {
        if is_quit(key) {
            return Err(super::super::SelectionQuit.into());
        }
        match key.code {
            KeyCode::Esc => {
                self.state = UiState::ProfileSelection;
                Ok(false)
            }
            KeyCode::Enter => {
                self.result = Some(ProfileSelectionResult::GenerateTestData);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub(super) fn handle_paste(&mut self, text: &str) {
        if !self.form_size_ok {
            return;
        }
        if let UiState::ProfileCreation(form) = &mut self.state {
            insert_paste(form, text);
        }
    }

    pub(super) fn handle_mouse(&mut self, mouse: MouseEvent) -> bool {
        if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            return false;
        }
        match &mut self.state {
            UiState::ProfileSelection => self.handle_selection_click(mouse),
            UiState::ProfileCreation(form) => {
                if let Some((field, _)) = self
                    .form_field_rects
                    .iter()
                    .find(|(_, rect)| point_in_rect(mouse.column, mouse.row, rect))
                {
                    form.active_field = *field;
                    if form.active_text_mut().is_none() {
                        form.toggle_active();
                    }
                }
                false
            }
            UiState::TestDataGeneration { .. } => false,
        }
    }

    fn handle_selection_click(&mut self, mouse: MouseEvent) -> bool {
        let Some(list_area) = self.profile_list_area else {
            return false;
        };
        if !point_in_rect(mouse.column, mouse.row, &list_area) {
            return false;
        }
        let clicked = self.profile_list_offset + usize::from(mouse.row - list_area.y);
        if clicked >= self.profiles.len() + 2 {
            return false;
        }
        self.selected = clicked;
        self.activate_selected()
    }

    fn delete_selected_profile(&mut self) -> Result<()> {
        if self.selected >= self.profiles.len() {
            self.set_status("Select a saved profile to delete", StatusKind::Error);
            return Ok(());
        }
        let profile = self
            .profiles
            .get(self.selected)
            .cloned()
            .context("selected profile should exist")?;
        let deleted = self
            .store
            .connection_profiles()
            .delete(&profile.name)
            .context("failed to delete connection profile")?;
        self.profiles.remove(self.selected);
        self.selected = self
            .selected
            .min((self.profiles.len() + 2).saturating_sub(1));
        let message = if deleted {
            format!("Removed profile '{}'", profile.name)
        } else {
            format!("Profile '{}' was already removed", profile.name)
        };
        self.set_status(
            message,
            if deleted {
                StatusKind::Info
            } else {
                StatusKind::Error
            },
        );
        Ok(())
    }

    fn set_status(&mut self, message: impl Into<String>, kind: StatusKind) {
        self.status = Some(super::state::SelectionStatus {
            message: message.into(),
            kind,
        });
    }
}

enum FormAction {
    Continue,
    Back,
    Finish(ProfileSelectionResult),
}

fn handle_form_key(
    key: KeyEvent,
    form: &mut ProfileForm,
    profiles: &[poqi_store::StoredConnectionProfile],
) -> Result<FormAction> {
    if is_quit(key) {
        return Err(super::super::SelectionQuit.into());
    }
    let modifiers = key.modifiers;
    match key.code {
        KeyCode::Esc => return Ok(FormAction::Back),
        KeyCode::Tab | KeyCode::BackTab => {
            form.move_field(
                key.code == KeyCode::BackTab || modifiers.contains(KeyModifiers::SHIFT),
            );
        }
        KeyCode::Up | KeyCode::Down => {
            form.move_field(matches!(key.code, KeyCode::Up));
        }
        KeyCode::Char(' ') if form.active_text_mut().is_none() => {
            form.toggle_active();
        }
        KeyCode::Enter if form.active_field == CreationField::Mode => form.toggle_active(),
        KeyCode::Enter => return Ok(submit_form(form, profiles)),
        KeyCode::Char('a') if modifiers.contains(KeyModifiers::CONTROL) => {
            edit_text(form, super::form::TextInput::clear);
        }
        KeyCode::Char('v') if modifiers.contains(KeyModifiers::CONTROL) => {
            paste_from_clipboard(form);
        }
        KeyCode::Home => edit_text(form, |input| input.cursor = 0),
        KeyCode::End => edit_text(form, |input| input.cursor = input.value.chars().count()),
        KeyCode::Left => edit_text(form, super::form::TextInput::move_left),
        KeyCode::Right => edit_text(form, super::form::TextInput::move_right),
        KeyCode::Backspace => edit_text(form, super::form::TextInput::backspace),
        KeyCode::Delete => edit_text(form, super::form::TextInput::delete),
        KeyCode::Char(character) if !modifiers.contains(KeyModifiers::CONTROL) => {
            edit_text(form, |input| input.insert(&character.to_string()));
        }
        _ => {}
    }
    Ok(FormAction::Continue)
}

fn edit_text(form: &mut ProfileForm, operation: impl FnOnce(&mut super::form::TextInput)) {
    if let Some(input) = form.active_text_mut() {
        operation(input);
        form.error = None;
    }
}

fn insert_paste(form: &mut ProfileForm, text: &str) {
    let pasted = if form.active_field == super::form::CreationField::Password {
        text
    } else {
        text.trim()
    };
    if let Some(input) = form.active_text_mut() {
        input.insert(pasted);
        form.error = None;
    }
}

fn paste_from_clipboard(form: &mut ProfileForm) {
    let result = arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.get_text())
        .map_err(|error| format!("Clipboard read failed: {error}"));
    match result {
        Ok(text) => insert_paste(form, &text),
        Err(error) => form.error = Some(error),
    }
}

fn submit_form(
    form: &mut ProfileForm,
    profiles: &[poqi_store::StoredConnectionProfile],
) -> FormAction {
    match form.submit(profiles) {
        Ok(result) => FormAction::Finish(result),
        Err(error) => {
            form.error = Some(error);
            FormAction::Continue
        }
    }
}

fn is_quit(key: KeyEvent) -> bool {
    (matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        && key.modifiers.contains(KeyModifiers::SHIFT))
        || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::{handle_form_key, insert_paste};
    use crate::bootstrap::start_menu::{
        form::{ConnectionMode, CreationField, ProfileForm, TextInput},
        state::{ProfileRecovery, UiState},
    };

    #[test]
    fn enter_on_connection_details_switches_mode_without_submitting() {
        let mut form = ProfileForm::new();
        form.active_field = CreationField::Mode;
        for expected in [ConnectionMode::Structured, ConnectionMode::Url] {
            let action = handle_form_key(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &mut form,
                &[],
            )
            .unwrap();
            assert!(matches!(action, super::FormAction::Continue));
            assert_eq!(form.mode, expected);
            assert!(form.error.is_none());
        }
    }

    #[test]
    fn bracketed_paste_trims_only_outer_whitespace() {
        let mut form = ProfileForm::new();
        form.active_field = CreationField::Uri;
        insert_paste(&mut form, " \r\npostgres://user:p a@host/db \t");
        assert_eq!(form.uri.value, "postgres://user:p a@host/db");
    }

    #[test]
    fn password_paste_preserves_whitespace() {
        let mut form = ProfileForm::new();
        form.active_field = CreationField::Password;
        insert_paste(&mut form, " secret ");
        assert_eq!(form.password.value, " secret ");
    }

    #[test]
    fn failed_profile_prefill_keeps_error_and_uri() {
        let profile =
            poqi_db::ConnectionProfile::new("broken", "postgres://host/db?sslmode=require");
        let store = poqi_store::Store::new_with_test_key(Some(":memory:".into()), [7; 32]);
        let saved = store
            .connection_profiles()
            .upsert(&poqi_store::NewConnectionProfile::new(
                "broken",
                "postgres://old@host/db",
            ))
            .expect("create saved profile");
        let app = super::App::new(
            poqi_store::Store::new_with_test_key(Some("unused.sqlite".into()), [7; 32]),
            vec![saved],
            0,
            false,
            Some(ProfileRecovery {
                profile: &profile,
                message: "Authentication failed; check user and password",
                editing: true,
                draft: None,
            }),
        );
        let UiState::ProfileCreation(form) = app.state else {
            panic!("recovery should open the profile form");
        };
        assert_eq!(form.uri.value, profile.uri);
        assert_eq!(
            form.error.as_deref(),
            Some("Authentication failed; check user and password")
        );
        assert!(form.name_locked);
    }

    #[test]
    fn structured_recovery_retains_draft() {
        let mut draft = ProfileForm::new();
        draft.name = TextInput::new("session");
        draft.mode = ConnectionMode::Structured;
        draft.host = TextInput::new("db.example.com");
        draft.port = TextInput::new("5433");
        draft.user = TextInput::new("operator");
        draft.password = TextInput::new(" secret ");
        draft.database = TextInput::new("warehouse");
        draft.active_field = CreationField::Password;
        let profile = poqi_db::ConnectionProfile::new(
            "session",
            "postgresql://operator:%20secret%20@db.example.com:5433/warehouse",
        );
        let app = super::App::new(
            poqi_store::Store::new_with_test_key(Some("unused.sqlite".into()), [7; 32]),
            Vec::new(),
            0,
            false,
            Some(ProfileRecovery {
                profile: &profile,
                message: "Authentication failed",
                editing: false,
                draft: Some(&draft),
            }),
        );
        let UiState::ProfileCreation(form) = app.state else {
            panic!("recovery should open the profile form");
        };
        assert_eq!(form.mode, ConnectionMode::Structured);
        assert_eq!(form.password.value, " secret ");
        assert_eq!(form.host.value, "db.example.com");
        assert_eq!(form.active_field, CreationField::Password);
        assert!(!form.is_editing());
    }

    #[test]
    fn external_matching_name_stays_unlocked_and_cannot_overwrite() {
        let store = poqi_store::Store::new_with_test_key(Some(":memory:".into()), [7; 32]);
        let saved = store
            .connection_profiles()
            .upsert(&poqi_store::NewConnectionProfile::new(
                "primary",
                "postgres://saved@host/db",
            ))
            .expect("create saved profile");
        let profile = poqi_db::ConnectionProfile::new(
            "primary",
            "postgres://external@host/db?sslmode=disable",
        );
        let app = super::App::new(
            poqi_store::Store::new_with_test_key(Some("unused.sqlite".into()), [7; 32]),
            vec![saved],
            0,
            false,
            Some(ProfileRecovery {
                profile: &profile,
                message: "Connection failed",
                editing: false,
                draft: None,
            }),
        );
        let UiState::ProfileCreation(form) = app.state else {
            panic!("recovery should open the profile form");
        };
        assert!(!form.is_editing());
        assert_eq!(
            form.submit(&app.profiles)
                .expect_err("saved-name collision should fail"),
            "A saved profile named 'primary' already exists"
        );
    }

    #[test]
    fn ctrl_a_clears_active_field() {
        let mut form = ProfileForm::new();
        form.name = TextInput::new("temporary");
        handle_form_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            &mut form,
            &[],
        )
        .expect("key handling should succeed");
        assert!(form.name.value.is_empty());
        assert_eq!(form.name.cursor, 0);
    }
}
