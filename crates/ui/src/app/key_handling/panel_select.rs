use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::input::{Action, FocusPanel};

use super::{helpers::is_shift_held, App};

impl App {
    /// Handle keyboard input in `PanelSelect` mode (window selector overlay).
    pub(super) fn handle_window_select_key(&mut self, event: KeyEvent) -> bool {
        let fast_step = self.ui_settings.fast_scroll_step();
        if event.kind != KeyEventKind::Press {
            return false;
        }

        match event.code {
            KeyCode::Down if event.modifiers.contains(KeyModifiers::SHIFT) => {
                self.move_selection_vertical_by(true, fast_step);
            }
            KeyCode::Up if event.modifiers.contains(KeyModifiers::SHIFT) => {
                self.move_selection_vertical_by(false, fast_step);
            }
            KeyCode::Down => self.move_selection_vertical(true),
            KeyCode::Up => self.move_selection_vertical(false),
            KeyCode::Right if event.modifiers.contains(KeyModifiers::SHIFT) => {
                self.move_selection_horizontal_by(true, fast_step);
            }
            KeyCode::Left if event.modifiers.contains(KeyModifiers::SHIFT) => {
                self.move_selection_horizontal_by(false, fast_step);
            }
            KeyCode::Right | KeyCode::Tab => self.move_selection(true),
            KeyCode::Left | KeyCode::BackTab => self.move_selection(false),
            KeyCode::Enter => {
                if matches!(self.panel_cursor, FocusPanel::Status) {
                    self.open_settings_modal();
                } else {
                    self.focus_selected_panel();
                }
            }
            KeyCode::Char(ch) if is_shift_held(&event) => {
                self.handle_panel_select_char(ch, true);
            }
            KeyCode::Char(ch) if event.modifiers.is_empty() => {
                self.handle_panel_select_char(ch, false);
            }
            KeyCode::Esc => return self.handle_action(Action::BackToProfiles),
            _ => {}
        }
        false
    }

    /// Handle WASD/F hotkeys while the selector overlay is active.
    pub(super) fn handle_panel_select_char(&mut self, ch: char, fast: bool) -> bool {
        let step = self.ui_settings.fast_scroll_step();
        match ch.to_ascii_lowercase() {
            'w' => {
                if fast {
                    self.move_selection_vertical_by(false, step);
                } else {
                    self.move_selection_vertical(false);
                }
                true
            }
            's' => {
                if fast {
                    self.move_selection_vertical_by(true, step);
                } else {
                    self.move_selection_vertical(true);
                }
                true
            }
            'a' => {
                if fast {
                    self.move_selection_horizontal_by(false, step);
                } else {
                    self.move_selection(false);
                }
                true
            }
            'd' => {
                if fast {
                    self.move_selection_horizontal_by(true, step);
                } else {
                    self.move_selection(true);
                }
                true
            }
            'f' if !fast => {
                if matches!(self.panel_cursor, FocusPanel::Status) {
                    self.open_settings_modal();
                } else {
                    self.focus_selected_panel();
                }
                true
            }
            _ => false,
        }
    }
}
