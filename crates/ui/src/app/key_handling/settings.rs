use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::state::settings::FieldKind;

use super::{App, UiLayer};

impl App {
    pub(super) fn handle_settings_key(&mut self, event: KeyEvent) -> bool {
        if event.kind != KeyEventKind::Press {
            return false;
        }
        if !matches!(self.layer, UiLayer::Settings) {
            return false;
        }
        let Some(view) = self.settings_view.as_mut() else {
            self.layer = UiLayer::PanelSelect;
            return false;
        };
        let fast_step = self.ui_settings.fast_scroll_step();
        let mut should_save = false;
        let mut should_cancel = false;
        let mut edit_result: Option<Result<(), String>> = None;

        match event.code {
            KeyCode::Char(ch) if view.editing && is_allowed_char(ch, view.selected_kind()) => {
                view.push_char(ch);
            }
            KeyCode::Backspace if view.editing => view.backspace(),
            KeyCode::Esc => {
                should_cancel = true;
            }
            KeyCode::Up if !view.editing => view.move_selection(false, 1),
            KeyCode::Down if !view.editing => view.move_selection(true, 1),
            KeyCode::Char(ch) if ch.eq_ignore_ascii_case(&'w') && !view.editing => {
                if event.modifiers.contains(KeyModifiers::SHIFT) {
                    view.move_selection(false, fast_step);
                } else {
                    view.move_selection(false, 1);
                }
            }
            KeyCode::Char(ch) if ch.eq_ignore_ascii_case(&'s') && !view.editing => {
                if event.modifiers.contains(KeyModifiers::SHIFT) {
                    view.move_selection(true, fast_step);
                } else {
                    view.move_selection(true, 1);
                }
            }
            KeyCode::PageUp if !view.editing => view.move_selection(false, fast_step),
            KeyCode::PageDown if !view.editing => view.move_selection(true, fast_step),
            KeyCode::Left if !view.editing => view.nudge_current(false),
            KeyCode::Right if !view.editing => view.nudge_current(true),
            KeyCode::Char(ch) if ch.eq_ignore_ascii_case(&'a') && !view.editing => {
                view.nudge_current(false);
            }
            KeyCode::Char(ch) if ch.eq_ignore_ascii_case(&'d') && !view.editing => {
                view.nudge_current(true);
            }
            KeyCode::Enter => {
                if view.editing {
                    edit_result = Some(view.commit_edit());
                } else if view.is_save_selected() {
                    should_save = true;
                } else {
                    view.begin_edit();
                }
            }
            _ => {}
        }

        let _ = view;

        if let Some(Err(message)) = edit_result {
            self.status.popup_error(message);
        }

        if should_cancel {
            self.cancel_settings();
            return false;
        }

        if should_save {
            self.save_settings();
        }
        false
    }
}

fn is_allowed_char(ch: char, kind: Option<FieldKind>) -> bool {
    match kind {
        Some(FieldKind::Number) => ch.is_ascii_digit(),
        Some(FieldKind::Float) => ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+',
        Some(FieldKind::Text) => !ch.is_control(),
        _ => false,
    }
}
