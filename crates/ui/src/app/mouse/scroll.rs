use crossterm::event::{MouseEvent, MouseEventKind};

use crate::{geometry::point_in_rect, input::FocusPanel};

use super::super::App;

impl App {
    pub(super) fn handle_scroll(&mut self, mouse: MouseEvent) {
        if let Some(panel) = self.panel_under_pointer(mouse) {
            if self.focus != panel {
                self.focus_window(panel);
            }
            match panel {
                FocusPanel::Schema => self.handle_schema_scroll(mouse.kind),
                FocusPanel::Results => self.handle_results_scroll(mouse.kind),
                FocusPanel::Editor => self.handle_editor_scroll(mouse),
                FocusPanel::SemanticSearch => self.handle_semantic_scroll(mouse.kind),
                FocusPanel::Status => {}
            }
        }
    }

    fn handle_schema_scroll(&mut self, kind: MouseEventKind) {
        let viewport = self.schema_viewport_rows.max(1);
        let lines = self.ui_settings.mouse_scroll_lines();
        match kind {
            MouseEventKind::ScrollUp => {
                for _ in 0..lines {
                    self.schema.move_up();
                }
                self.schema.ensure_selected_visible(viewport);
            }
            MouseEventKind::ScrollDown => {
                for _ in 0..lines {
                    self.schema.move_down();
                }
                self.schema.ensure_selected_visible(viewport);
            }
            _ => (),
        }
        self.auto_fetch_on_table_selection();
    }

    fn handle_results_scroll(&mut self, kind: MouseEventKind) {
        let step = isize::try_from(self.ui_settings.mouse_scroll_lines()).unwrap_or(isize::MAX);
        match kind {
            MouseEventKind::ScrollUp => self.results.move_vertical(-step, false),
            MouseEventKind::ScrollDown => self.results.move_vertical(step, false),
            MouseEventKind::ScrollLeft => self.results.move_horizontal(-step, false),
            MouseEventKind::ScrollRight => self.results.move_horizontal(step, false),
            _ => {}
        }
    }

    fn handle_editor_scroll(&mut self, mouse: MouseEvent) {
        if self.editor.completions_visible() {
            if let Some(hitbox) = self.completion_popup_hitbox {
                if point_in_rect(mouse.column, mouse.row, &hitbox.area) {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => self.editor.scroll_completion_popup(-1),
                        MouseEventKind::ScrollDown => self.editor.scroll_completion_popup(1),
                        _ => {}
                    }
                    return;
                }
            }
        }
        let Some(text_area) = self.editor_text_area else {
            return;
        };
        if text_area.width == 0 || text_area.height == 0 {
            return;
        }
        let view_width = usize::from(text_area.width.max(1));
        let view_height = usize::from(text_area.height.max(1));
        let total_rows = self.editor_row_count(view_width);
        if total_rows == 0 {
            return;
        }
        let step = isize::try_from(self.ui_settings.mouse_scroll_lines()).unwrap_or(isize::MAX);
        match mouse.kind {
            MouseEventKind::ScrollUp => self.editor.scroll_by(-step, total_rows, view_height),
            MouseEventKind::ScrollDown => self.editor.scroll_by(step, total_rows, view_height),
            _ => {}
        }
    }

    fn handle_semantic_scroll(&mut self, kind: MouseEventKind) {
        let Some(text_area) = self.semantic_text_area else {
            return;
        };
        if text_area.width == 0 || text_area.height == 0 {
            return;
        }
        let view_width = usize::from(text_area.width.max(1));
        let view_height = usize::from(text_area.height.max(1));
        let (wrapped_rows, _, _) =
            crate::view::text_input::wrapped_content(self.semantic.buffer(), view_width);
        let total_rows = wrapped_rows.len();
        if total_rows == 0 {
            return;
        }
        let step = isize::try_from(self.ui_settings.mouse_scroll_lines()).unwrap_or(isize::MAX);
        match kind {
            MouseEventKind::ScrollUp => self.semantic.scroll_by(-step, total_rows, view_height),
            MouseEventKind::ScrollDown => self.semantic.scroll_by(step, total_rows, view_height),
            _ => {}
        }
    }
}
