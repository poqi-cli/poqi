use crate::{
    app::App,
    view::text_input::{byte_offset_from, wrapped_content},
};

impl App {
    pub(crate) fn editor_hit_test(
        &self,
        view_width: usize,
        view_row: usize,
        view_col: usize,
    ) -> Option<(usize, usize)> {
        if view_width == 0 {
            return None;
        }
        let gutter_width = self.editor_gutter_width();
        let content_width = view_width.saturating_sub(gutter_width);
        let (wrapped_rows, _, _) = wrapped_content(&self.editor, content_width);
        if wrapped_rows.is_empty() {
            return Some((0, 0));
        }
        let content_col = view_col.saturating_sub(gutter_width);
        let max_row = wrapped_rows.len().saturating_sub(1);
        let absolute_row = self.editor.scroll_row.saturating_add(view_row).min(max_row);
        let row = wrapped_rows.get(absolute_row)?;
        let line = self.editor.lines.get(row.source_line)?;
        let target_char = content_col.min(row.char_len);
        let byte_offset = byte_offset_from(line, row.start_byte, target_char);
        Some((row.source_line, byte_offset))
    }

    pub(crate) fn editor_row_count(&self, view_width: usize) -> usize {
        if view_width == 0 {
            return 0;
        }
        let content_width = view_width.saturating_sub(self.editor_gutter_width());
        let (wrapped_rows, _, _) = wrapped_content(&self.editor, content_width);
        wrapped_rows.len()
    }

    pub(crate) fn clamp_editor_scroll_to_cursor(&mut self) {
        let Some(area) = self.editor_text_area else {
            return;
        };
        if area.width == 0 || area.height == 0 {
            return;
        }
        let view_width = usize::from(area.width.max(1));
        let content_width = view_width.saturating_sub(self.editor_gutter_width());
        let view_height = usize::from(area.height.max(1));
        let (wrapped_rows, cursor_visual_row, _) = wrapped_content(&self.editor, content_width);
        self.editor
            .ensure_cursor_visible(cursor_visual_row, view_height, wrapped_rows.len());
    }

    pub(crate) fn shift_editor_cursor_by_rows(&mut self, row_delta: isize) {
        let Some(area) = self.editor_text_area else {
            return;
        };
        if area.width == 0 || area.height == 0 {
            return;
        }
        let view_width = usize::from(area.width.max(1));
        let content_width = view_width.saturating_sub(self.editor_gutter_width());
        let view_height = usize::from(area.height.max(1));
        let (wrapped_rows, cursor_visual_row, cursor_visual_col) =
            wrapped_content(&self.editor, content_width);
        if wrapped_rows.is_empty() {
            return;
        }
        let total_rows = wrapped_rows.len();
        let max_row = isize::try_from(total_rows.saturating_sub(1)).unwrap_or(isize::MAX);
        let mut target_row = isize::try_from(cursor_visual_row).unwrap_or(isize::MAX);
        target_row = target_row.saturating_add(row_delta);
        if target_row < 0 {
            target_row = 0;
        } else if target_row > max_row {
            target_row = max_row;
        }
        let target_row = usize::try_from(target_row.max(0)).unwrap_or(0);
        let row = wrapped_rows
            .get(target_row)
            .or_else(|| wrapped_rows.last())
            .unwrap();
        let Some(line) = self.editor.lines.get(row.source_line) else {
            return;
        };
        let char_pos = cursor_visual_col.min(row.char_len);
        let byte_offset = byte_offset_from(line, row.start_byte, char_pos);
        self.editor.set_cursor(row.source_line, byte_offset);
        self.editor
            .ensure_cursor_visible(target_row, view_height, total_rows);
    }
}
