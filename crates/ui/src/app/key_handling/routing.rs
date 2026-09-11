use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::input::{Action, FocusPanel};

use super::{
    helpers::{is_ctrl_char, is_shift_char},
    App, UiLayer,
};

impl App {
    /// Central keyboard dispatcher; returns `true` when the app should exit.
    pub fn handle_key(&mut self, event: KeyEvent) -> bool {
        if event.kind == KeyEventKind::Repeat {
            return false;
        }

        if event.code == KeyCode::Esc && event.modifiers.contains(KeyModifiers::SHIFT) {
            return self.handle_action(Action::Quit);
        }

        if is_ctrl_char(&event, 'c') {
            if matches!(self.focus, FocusPanel::Editor | FocusPanel::SemanticSearch)
                && matches!(self.layer, UiLayer::PanelFocused)
            {
                self.copy_editor_buffer();
                return false;
            }
            return self.handle_action(Action::Quit);
        }

        if self.layer == UiLayer::PanelSelect {
            return self.handle_window_select_key(event);
        }

        if self.layer == UiLayer::Settings {
            return self.handle_settings_key(event);
        }

        if event.kind == KeyEventKind::Press && is_shift_char(&event, 'z') {
            return self.handle_action(Action::ToggleZoom);
        }

        if self.focus == FocusPanel::Editor && event.kind == KeyEventKind::Press {
            if let Some(result) = self.handle_editor_key(&event) {
                return result;
            }
        }

        if self.focus == FocusPanel::SemanticSearch
            && event.kind == KeyEventKind::Press
            && self.handle_semantic_key(&event)
        {
            return false;
        }

        if self.focus == FocusPanel::Results
            && self.results.edit_session().is_some()
            && self.handle_results_key(event)
        {
            return false;
        }

        if let Some(action) = self.keymap.resolve(&event, self.focus) {
            return self.handle_action(action);
        }

        if self.focus == FocusPanel::Results && self.handle_results_key(event) {
            return false;
        }

        false
    }
}
