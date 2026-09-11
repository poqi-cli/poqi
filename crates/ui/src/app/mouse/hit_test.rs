use crossterm::event::MouseEvent;

use crate::{geometry::point_in_rect, input::FocusPanel};

use super::super::App;

impl App {
    pub(super) fn panel_under_pointer(&self, mouse: MouseEvent) -> Option<FocusPanel> {
        self.detect_clicked_panel(mouse.column, mouse.row)
    }

    pub(super) fn detect_clicked_panel(&self, column: u16, row: u16) -> Option<FocusPanel> {
        if let Some(schema_area) = self.schema_area {
            if point_in_rect(column, row, &schema_area) {
                return Some(FocusPanel::Schema);
            }
        }

        if let Some(editor_text_area) = self.editor_text_area {
            if point_in_rect(column, row, &editor_text_area) {
                return Some(FocusPanel::Editor);
            }
        }

        if let Some(editor_area) = self.editor_area {
            if point_in_rect(column, row, &editor_area) {
                return Some(FocusPanel::Editor);
            }
        }

        if let Some(semantic_area) = self.semantic_area {
            if point_in_rect(column, row, &semantic_area) {
                return Some(FocusPanel::SemanticSearch);
            }
        }

        if let Some(results_area) = self.results_area {
            if point_in_rect(column, row, &results_area) {
                return Some(FocusPanel::Results);
            }
        }

        if let Some(status_area) = self.status_area {
            if point_in_rect(column, row, &status_area) {
                return Some(FocusPanel::Status);
            }
        }

        None
    }

    pub(super) fn clamp_editor_hit(&self, line_idx: usize, byte_offset: usize) -> (usize, usize) {
        let row = line_idx.min(self.editor.lines.len().saturating_sub(1));
        let col = self
            .editor
            .lines
            .get(row)
            .map_or(0, |line| byte_offset.min(line.len()));
        (row, col)
    }
}
