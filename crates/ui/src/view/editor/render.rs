use ratatui::{
    layout::Rect,
    style::Modifier,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::{
    app::{App, CompletionPopupHitbox},
    input::FocusPanel,
    view::text_input::{visible_rows as text_input_visible_rows, wrapped_content},
};

use super::gutter::{editor_gutter_span, GUTTER_MARGIN_WIDTH};

impl App {
    pub(crate) fn draw_editor(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let inner_area = {
            let tmp_block = self.block_with_title("Editor", FocusPanel::Editor);
            tmp_block.inner(area)
        };

        self.completion_popup_hitbox = None;

        if inner_area.width == 0 || inner_area.height == 0 {
            let block = self.block_with_title("Editor", FocusPanel::Editor);
            let paragraph = Paragraph::new(Text::default()).block(block);
            frame.render_widget(paragraph, area);
            self.editor_text_area = None;
            return;
        }

        let view_width = usize::from(inner_area.width.max(1));
        let view_height = usize::from(inner_area.height.max(1));
        let gutter_width = self.editor_gutter_width();
        let content_width = view_width.saturating_sub(gutter_width);
        let (wrapped_rows, cursor_visual_row, cursor_visual_col) =
            wrapped_content(&self.editor, content_width);
        let selection = self.editor.selection_range();
        let (visible_rows, first_visible_row) =
            text_input_visible_rows(&wrapped_rows, view_height, self.editor.scroll_row);

        let gutter_active = self
            .theme
            .text
            .add_modifier(Modifier::BOLD)
            .patch(self.theme.panel_background);
        let gutter_inactive = self.theme.text_muted;
        let number_field_width = gutter_width.saturating_sub(GUTTER_MARGIN_WIDTH).max(1);
        let lines: Vec<Line> = visible_rows
            .iter()
            .map(|row| {
                let span = selection
                    .as_ref()
                    .and_then(|range| self.selection_span_for_row(row, range));
                let mut content = self.highlight_line(row, span);
                let gutter_span = editor_gutter_span(
                    row,
                    gutter_width,
                    number_field_width,
                    gutter_inactive,
                    gutter_active,
                    row.visual_index == cursor_visual_row,
                );
                let mut combined = Vec::with_capacity(content.spans.len() + 1);
                combined.push(gutter_span);
                combined.append(&mut content.spans);
                Line::from(combined)
            })
            .collect();

        let block = self.block_with_title("Editor", FocusPanel::Editor);
        let paragraph = Paragraph::new(Text::from(lines)).block(block);
        frame.render_widget(paragraph, area);

        if self.focus == FocusPanel::Editor {
            let cursor_row_in_view = cursor_visual_row.saturating_sub(first_visible_row);
            if let (Ok(cursor_x_offset), Ok(cursor_y_offset), Ok(gutter_offset)) = (
                u16::try_from(cursor_visual_col),
                u16::try_from(cursor_row_in_view),
                u16::try_from(gutter_width),
            ) {
                let cursor_x = inner_area
                    .x
                    .saturating_add(gutter_offset)
                    .saturating_add(cursor_x_offset);
                let cursor_y = inner_area.y.saturating_add(cursor_y_offset);
                frame.set_cursor_position((cursor_x, cursor_y));
                self.editor_cursor = Some((cursor_x, cursor_y));
            }
        }

        self.editor_text_area = Some(inner_area);
    }

    pub(crate) fn draw_completion_popup(
        &mut self,
        frame: &mut Frame<'_>,
        inner_area: Rect,
        cursor_x: u16,
        cursor_y: u16,
    ) {
        self.completion_popup_hitbox = None;
        let max_content_width = usize::from(inner_area.width.saturating_sub(2));
        let max_content_height = usize::from(inner_area.height.saturating_sub(2));
        if max_content_width == 0 || max_content_height == 0 {
            return;
        }
        self.editor.set_completion_viewport_rows(max_content_height);
        let (items, selected_idx) = self.editor.completion_popup_items();
        if items.is_empty() {
            return;
        }
        let (mut start, mut end) = self.editor.completion_visible_range();
        if end <= start {
            end = (start + 1).min(items.len());
        }
        start = start.min(items.len().saturating_sub(1));
        end = end.min(items.len());
        let mut visible_rows = end.saturating_sub(start);
        if visible_rows == 0 {
            visible_rows = 1;
        }
        visible_rows = visible_rows.min(max_content_height).max(1);
        let label_width = items
            .iter()
            .map(|item| item.label.len())
            .max()
            .unwrap_or(0)
            .min(64);
        let detail_width = items
            .iter()
            .map(|item| item.detail.len())
            .max()
            .unwrap_or(0)
            .min(32);
        let popup_char_width = label_width.saturating_add(detail_width).saturating_add(2);
        let popup_content_width = popup_char_width.min(max_content_width).max(1);
        let popup_content_height = visible_rows;
        let popup_width =
            u16::try_from(popup_content_width.saturating_add(2)).unwrap_or(inner_area.width);
        let popup_height =
            u16::try_from(popup_content_height.saturating_add(2)).unwrap_or(inner_area.height);
        // Offset the popup horizontally so the cursor column stays visible.
        let mut popup_x = cursor_x.saturating_add(1);
        let inner_right = inner_area.x.saturating_add(inner_area.width);
        if popup_x.saturating_add(popup_width) > inner_right {
            popup_x = inner_right.saturating_sub(popup_width);
        }
        let mut popup_y = cursor_y.saturating_add(1);
        let inner_bottom = inner_area.y.saturating_add(inner_area.height);
        if popup_y.saturating_add(popup_height) > inner_bottom {
            popup_y = cursor_y.saturating_sub(popup_height).max(inner_area.y);
        }
        let popup_area = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width,
            height: popup_height.min(inner_area.height),
        };
        let content_area = Rect {
            x: popup_area.x.saturating_add(1),
            y: popup_area.y.saturating_add(1),
            width: popup_area.width.saturating_sub(2),
            height: popup_area.height.saturating_sub(2),
        };
        let mut lines = Vec::new();
        let label_field_width = popup_content_width
            .saturating_sub(2)
            .min(label_width)
            .max(1);
        let slice_end = start.saturating_add(visible_rows).min(items.len());
        for (idx, item) in items[start..slice_end].iter().enumerate() {
            let absolute_idx = start + idx;
            let style = if selected_idx == Some(absolute_idx) {
                self.theme.highlight
            } else {
                self.theme.text_muted
            };
            let text = format!(
                "{:<width$}  {}",
                item.label,
                item.detail,
                width = label_field_width
            );
            lines.push(Line::from(Span::styled(text, style)));
        }
        let block = Block::default()
            .borders(Borders::ALL)
            .style(self.theme.border);
        let paragraph = Paragraph::new(Text::from(lines)).block(block);
        frame.render_widget(paragraph, popup_area);
        self.completion_popup_hitbox = Some(CompletionPopupHitbox {
            area: popup_area,
            content_area,
            start_index: start,
            visible_rows,
        });
    }
}
