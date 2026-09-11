use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{input::FocusPanel, state::results_state::plain_text_modifiers};

use super::super::App;
use super::helpers::is_ctrl_char;

impl App {
    pub(super) fn handle_semantic_key(&mut self, event: &KeyEvent) -> bool {
        if self.focus != FocusPanel::SemanticSearch {
            return false;
        }
        if matches!(event.code, KeyCode::Esc) {
            return false;
        }
        if is_ctrl_char(event, 'v') {
            self.paste_from_clipboard();
            return true;
        }
        if event.modifiers.contains(KeyModifiers::CONTROL) && !matches!(event.code, KeyCode::Enter)
        {
            return false;
        }
        match event.code {
            KeyCode::Enter => {
                self.trigger_semantic_search();
                return true;
            }
            KeyCode::Tab if event.modifiers.is_empty() => {
                self.semantic.insert_tab();
                self.refresh_semantic_post_edit();
                return true;
            }
            KeyCode::BackTab => {
                self.semantic.outdent();
                self.refresh_semantic_post_edit();
                return true;
            }
            KeyCode::Char(ch) if plain_text_modifiers(event.modifiers) => {
                self.semantic.insert_char(ch);
                self.refresh_semantic_post_edit();
                return true;
            }
            KeyCode::Backspace if event.modifiers.is_empty() => {
                self.semantic.backspace();
                self.refresh_semantic_post_edit();
                return true;
            }
            KeyCode::Up if plain_text_modifiers(event.modifiers) => {
                let extend = event.modifiers.contains(KeyModifiers::SHIFT);
                self.move_semantic_cursor_vertical(-1, 1, extend);
                return true;
            }
            KeyCode::Down if plain_text_modifiers(event.modifiers) => {
                let extend = event.modifiers.contains(KeyModifiers::SHIFT);
                self.move_semantic_cursor_vertical(1, 1, extend);
                return true;
            }
            KeyCode::Left if plain_text_modifiers(event.modifiers) => {
                let extend = event.modifiers.contains(KeyModifiers::SHIFT);
                self.move_semantic_cursor_horizontal(-1, 1, extend);
                return true;
            }
            KeyCode::Right if plain_text_modifiers(event.modifiers) => {
                let extend = event.modifiers.contains(KeyModifiers::SHIFT);
                self.move_semantic_cursor_horizontal(1, 1, extend);
                return true;
            }
            _ => {}
        }
        false
    }
}
