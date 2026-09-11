use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::{geometry::point_in_rect, state::settings::FieldKind};

use super::super::App;

impl App {
    pub(super) fn handle_settings_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                self.handle_settings_scroll(mouse.kind);
            }
            MouseEventKind::Down(MouseButton::Left) => self.handle_settings_click(mouse),
            MouseEventKind::Up(MouseButton::Left) => {
                self.clear_editor_drag_anchor();
                self.clear_semantic_drag_anchor();
                self.scroll_drag = None;
            }
            _ => {}
        }
    }

    fn handle_settings_scroll(&mut self, kind: MouseEventKind) {
        let Some(view) = self.settings_view.as_mut() else {
            return;
        };
        if view.editing {
            return;
        }
        let step = self.ui_settings.mouse_scroll_lines().max(1);
        match kind {
            MouseEventKind::ScrollUp => view.move_selection(false, step),
            MouseEventKind::ScrollDown => view.move_selection(true, step),
            _ => {}
        }
    }

    fn handle_settings_click(&mut self, mouse: MouseEvent) {
        let Some(view) = self.settings_view.as_mut() else {
            return;
        };
        let Some(hitbox) = self.settings_hitbox else {
            self.cancel_settings();
            return;
        };
        if !point_in_rect(mouse.column, mouse.row, &hitbox.popup_area) {
            self.cancel_settings();
            return;
        }

        if point_in_rect(mouse.column, mouse.row, &hitbox.action_area) {
            view.selected = hitbox.action_index.min(view.rows().len().saturating_sub(1));
            if view.editing {
                if let Err(message) = view.commit_edit() {
                    self.status.popup_error(message);
                    return;
                }
            }
            self.save_settings();
            return;
        }

        if !point_in_rect(mouse.column, mouse.row, &hitbox.body_area) {
            return;
        }
        let visible_row = usize::from(mouse.row.saturating_sub(hitbox.body_area.y));
        let target_index = hitbox.body_offset.saturating_add(visible_row);
        let total_fields = view.rows().len().saturating_sub(1);
        if total_fields == 0 || target_index >= total_fields {
            return;
        }
        let previous_selection = view.selected;
        if view.editing && target_index != previous_selection {
            view.cancel_edit();
        }
        view.selected = target_index;

        match view.selected_kind() {
            Some(FieldKind::Choice) => view.nudge_current(true),
            Some(FieldKind::Number | FieldKind::Float | FieldKind::Text) if !view.editing => {
                let _ = view.begin_edit();
            }
            _ => {}
        }
    }
}
