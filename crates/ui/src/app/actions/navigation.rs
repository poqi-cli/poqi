use crate::input::{Action, FocusPanel, MoveDirection};

use super::{App, UiLayer};

impl App {
    /// Handle focus + cursor movement actions for both selector layers.
    pub(super) fn handle_navigation_action(&mut self, action: Action) {
        match action {
            Action::FocusNextPanel => {
                if self.layer == UiLayer::PanelSelect {
                    self.move_selection(true);
                } else {
                    let next = self.focus.next_horizontal();
                    self.focus_window(next);
                    self.status.info(format!("Focus -> {:?}", self.focus));
                }
            }
            Action::FocusPrevPanel => {
                if self.layer == UiLayer::PanelSelect {
                    self.move_selection(false);
                } else {
                    let prev = self.focus.prev_horizontal();
                    self.focus_window(prev);
                    self.status.info(format!("Focus -> {:?}", self.focus));
                }
            }
            Action::ExtendUp => self.apply_move(MoveDirection::Up, 1, true),
            Action::ExtendDown => self.apply_move(MoveDirection::Down, 1, true),
            Action::ExtendLeft => self.apply_move(MoveDirection::Left, 1, true),
            Action::ExtendRight => self.apply_move(MoveDirection::Right, 1, true),
            Action::PageUp => self.on_page(-1),
            Action::PageDown => self.on_page(1),
            Action::JumpRowStart => self.on_jump_row_start(),
            Action::JumpRowEnd => self.on_jump_row_end(),
            Action::JumpColStart => self.on_jump_col_start(),
            Action::JumpColEnd => self.on_jump_col_end(),
            _ => {}
        }
    }

    pub(super) fn apply_move(&mut self, dir: MoveDirection, step: usize, extend: bool) {
        let vertical = dir.vertical_delta();
        if vertical != 0 {
            self.on_move_vertical(vertical, extend, step);
            return;
        }
        let horizontal = dir.horizontal_delta();
        if horizontal != 0 {
            self.on_move_horizontal(horizontal, extend, step);
        }
    }

    fn on_move_vertical(&mut self, delta: isize, extend: bool, step: usize) {
        let step_delta = isize::try_from(step).unwrap_or(isize::MAX);
        match self.focus {
            FocusPanel::Schema => {
                for _ in 0..step {
                    if delta < 0 {
                        self.schema.move_up();
                    } else {
                        self.schema.move_down();
                    }
                }
                self.auto_fetch_on_table_selection();
            }
            FocusPanel::Editor => self.move_editor_cursor_vertical(delta, step, extend),
            FocusPanel::SemanticSearch | FocusPanel::Status => {}
            FocusPanel::Results => {
                self.results.move_vertical(delta * step_delta, extend);
            }
        }
    }

    fn on_move_horizontal(&mut self, delta: isize, extend: bool, step: usize) {
        let step_delta = isize::try_from(step).unwrap_or(isize::MAX);
        match self.focus {
            FocusPanel::Schema => {
                if delta < 0 {
                    if self.schema.go_back() {
                        let selected = self.schema.selected_table_name();
                        self.table_detail.follow_selection(selected.as_ref());
                    }
                    return;
                }
                let target = self.focus.next();
                if target != self.focus {
                    self.enter_window_layer_with_cursor(target);
                }
            }
            FocusPanel::SemanticSearch => {
                let target = if delta < 0 {
                    self.focus.prev()
                } else {
                    self.focus.next()
                };
                if target != self.focus {
                    self.enter_window_layer_with_cursor(target);
                }
            }
            FocusPanel::Status => {
                let target = if delta < 0 {
                    self.focus.prev()
                } else {
                    self.focus.next()
                };
                self.focus_window(target);
                self.status.info(format!("Focus -> {:?}", self.focus));
            }
            FocusPanel::Editor => self.move_editor_cursor_horizontal(delta, step, extend),
            FocusPanel::Results => {
                self.results.move_horizontal(delta * step_delta, extend);
            }
        }
    }

    fn on_page(&mut self, direction: isize) {
        match self.focus {
            FocusPanel::Schema => {
                if direction < 0 {
                    self.schema.page_up();
                } else {
                    self.schema.page_down();
                }
                self.auto_fetch_on_table_selection();
            }
            FocusPanel::Editor => {
                let Some(area) = self.editor_text_area else {
                    return;
                };
                if area.height == 0 {
                    return;
                }
                let rows = usize::from(area.height.max(1));
                if rows == 0 {
                    return;
                }
                let page = rows.saturating_sub(1).max(1);
                let step = isize::try_from(page).unwrap_or(isize::MAX);
                let delta = if direction < 0 { -step } else { step };
                self.editor.clear_selection();
                self.shift_editor_cursor_by_rows(delta);
                self.refresh_editor_after_cursor_move();
            }
            FocusPanel::SemanticSearch | FocusPanel::Status => {}
            FocusPanel::Results => {
                self.results.move_vertical(direction * 5, false);
            }
        }
    }

    fn on_jump_row_start(&mut self) {
        match self.focus {
            FocusPanel::Schema => {
                self.schema.jump_start();
                self.auto_fetch_on_table_selection();
            }
            FocusPanel::Editor | FocusPanel::SemanticSearch | FocusPanel::Status => {}
            FocusPanel::Results => self.results.jump_row_start(),
        }
    }

    fn on_jump_row_end(&mut self) {
        match self.focus {
            FocusPanel::Schema => {
                self.schema.jump_end();
                self.auto_fetch_on_table_selection();
            }
            FocusPanel::Editor | FocusPanel::SemanticSearch | FocusPanel::Status => {}
            FocusPanel::Results => self.results.jump_row_end(),
        }
    }

    fn on_jump_col_start(&mut self) {
        if matches!(self.focus, FocusPanel::Results) {
            self.results.jump_col_start();
        }
    }

    fn on_jump_col_end(&mut self) {
        if matches!(self.focus, FocusPanel::Results) {
            self.results.jump_col_end();
        }
    }
}
