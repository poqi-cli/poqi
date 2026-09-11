use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{input::Action, state::results_state::plain_text_modifiers};

use super::{
    helpers::{is_completion_commit_char, is_ctrl_char, should_run_enter},
    App,
};

impl App {
    /// Handle editor-specific keystrokes before falling back to the keymap.
    pub(super) fn handle_editor_key(&mut self, event: &KeyEvent) -> Option<bool> {
        if is_ctrl_char(event, 'v') {
            self.paste_from_clipboard();
            return Some(false);
        }
        let popup_active = self.editor.completions_visible();
        if popup_active && event.modifiers.is_empty() {
            match event.code {
                KeyCode::Up => {
                    self.editor.select_previous_completion();
                    return Some(false);
                }
                KeyCode::Down => {
                    self.editor.select_next_completion();
                    return Some(false);
                }
                KeyCode::Esc => {
                    // Close the completion popup without changing focus.
                    self.editor.clear_completions();
                    return Some(false);
                }
                _ => {}
            }
        }
        if event.code == KeyCode::Enter {
            if should_run_enter(event) {
                return Some(self.handle_action(Action::Run));
            }
            if self.editor.accept_selected_completion() {
                self.refresh_editor_completions();
                return Some(false);
            }
            self.editor.newline();
            self.refresh_editor_post_edit();
            return Some(false);
        }
        match event.code {
            KeyCode::Tab if event.modifiers.is_empty() => {
                self.editor.insert_tab();
                self.refresh_editor_post_edit();
                Some(false)
            }
            KeyCode::BackTab => {
                self.editor.outdent();
                self.refresh_editor_post_edit();
                Some(false)
            }
            KeyCode::Char(ch) if plain_text_modifiers(event.modifiers) => {
                if is_completion_commit_char(ch) {
                    let _ = self.editor.maybe_commit_completion();
                }
                self.editor.insert_char(ch);
                self.refresh_editor_post_edit();
                Some(false)
            }
            KeyCode::Backspace if event.modifiers.is_empty() => {
                self.editor.backspace();
                self.refresh_editor_post_edit();
                Some(false)
            }
            KeyCode::Up if plain_text_modifiers(event.modifiers) => {
                let extend = event.modifiers.contains(KeyModifiers::SHIFT);
                self.move_editor_cursor_vertical(-1, 1, extend);
                Some(false)
            }
            KeyCode::Down if plain_text_modifiers(event.modifiers) => {
                let extend = event.modifiers.contains(KeyModifiers::SHIFT);
                self.move_editor_cursor_vertical(1, 1, extend);
                Some(false)
            }
            KeyCode::Left if plain_text_modifiers(event.modifiers) => {
                let extend = event.modifiers.contains(KeyModifiers::SHIFT);
                self.move_editor_cursor_horizontal(-1, 1, extend);
                Some(false)
            }
            KeyCode::Right if plain_text_modifiers(event.modifiers) => {
                let extend = event.modifiers.contains(KeyModifiers::SHIFT);
                self.move_editor_cursor_horizontal(1, 1, extend);
                Some(false)
            }
            _ => None,
        }
    }

    /// Refresh completion state + scroll after mutating the editor buffer.
    pub(super) fn refresh_editor_post_edit(&mut self) {
        self.refresh_editor_completions();
        self.clamp_editor_scroll_to_cursor();
    }
}
