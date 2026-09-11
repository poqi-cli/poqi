use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Wrap},
    Frame,
};

use crate::{
    app::App,
    input::FocusPanel,
    state::{semantic::SemanticStatus, text_input::take_chars},
    view::text_input::{selection_span_for_row, visible_rows, wrapped_content, WrappedRow},
};

impl App {
    pub(super) fn draw_semantic(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let block = self.block_with_title("Semantic Search", FocusPanel::SemanticSearch);
        if area.width == 0 || area.height == 0 {
            return;
        }
        let inner = block.inner(area);
        frame.render_widget(block, area);
        if inner.width == 0 || inner.height == 0 {
            self.semantic_text_area = None;
            return;
        }

        let info_height = inner.height.min(2);
        let text_height = inner.height.saturating_sub(info_height.max(1));
        let label_height = text_height.min(1);
        let input_height = text_height.saturating_sub(label_height).max(1);

        let label_area = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: label_height,
        };
        let input_area = Rect {
            x: inner.x,
            y: inner.y.saturating_add(label_height),
            width: inner.width,
            height: input_height,
        };
        let status_area = Rect {
            x: inner.x,
            y: inner
                .y
                .saturating_add(label_height)
                .saturating_add(input_height),
            width: inner.width,
            height: info_height.max(1),
        };

        if label_area.height > 0 {
            let label = Paragraph::new(Line::from(Span::styled(
                "Natural language query",
                self.theme.text_muted,
            )));
            frame.render_widget(label, label_area);
        }

        self.draw_semantic_text(frame, input_area);
        self.draw_semantic_status(frame, status_area);
    }

    fn draw_semantic_text(&mut self, frame: &mut Frame<'_>, area: Rect) {
        if area.width == 0 || area.height == 0 {
            self.semantic_text_area = None;
            return;
        }
        self.semantic_text_area = Some(area);
        let view_width = usize::from(area.width.max(1));
        let view_height = usize::from(area.height.max(1));
        let (wrapped_rows, cursor_visual_row, cursor_visual_col) =
            wrapped_content(self.semantic.buffer(), view_width);
        let selection = self.semantic.buffer().selection_range();
        let (visible_rows, first_visible_row) = visible_rows(
            &wrapped_rows,
            view_height,
            self.semantic.buffer().scroll_row,
        );

        let mut lines: Vec<Line> = visible_rows
            .iter()
            .map(|row| {
                let span = selection
                    .as_ref()
                    .and_then(|range| selection_span_for_row(self.semantic.buffer(), row, range));
                self.semantic_line(row, span)
            })
            .collect();

        let is_empty = self.semantic.buffer().lines.len() == 1
            && self
                .semantic
                .buffer()
                .lines
                .first()
                .is_none_or(String::is_empty);
        if is_empty {
            lines = vec![Line::from(Span::styled(
                "… waiting for input",
                self.theme.text_muted,
            ))];
        }

        let paragraph = Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);

        if self.focus == FocusPanel::SemanticSearch {
            let cursor_row_in_view = cursor_visual_row.saturating_sub(first_visible_row);
            if let (Ok(cursor_x_offset), Ok(cursor_y_offset)) = (
                u16::try_from(cursor_visual_col),
                u16::try_from(cursor_row_in_view),
            ) {
                let cursor_x = area.x.saturating_add(cursor_x_offset);
                let cursor_y = area.y.saturating_add(cursor_y_offset);
                frame.set_cursor_position((cursor_x, cursor_y));
            }
        }
    }

    fn draw_semantic_status(&self, frame: &mut Frame<'_>, area: Rect) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let status = self.semantic.status();
        let status_span = match status {
            SemanticStatus::Disabled { reason } => Span::styled(reason, self.theme.status_warning),
            SemanticStatus::Idle => Span::styled(
                "Idle - type a prompt and press Enter to run",
                self.theme.text_muted,
            ),
            SemanticStatus::Loading { label } => Span::styled(label, self.theme.accent_primary),
            SemanticStatus::Ready { summary } => Span::styled(summary, self.theme.status_success),
            SemanticStatus::Error { message } => Span::styled(message, self.theme.status_error),
        };
        let hint = self.semantic.peek_hint();
        let hint_style: Style = match status {
            SemanticStatus::Ready { .. } => {
                self.theme.status_success.add_modifier(Modifier::ITALIC)
            }
            _ => self.theme.accent_secondary.add_modifier(Modifier::ITALIC),
        };
        let content = vec![
            Line::from(status_span),
            Line::from(Span::styled(hint, hint_style)),
        ];
        let paragraph = Paragraph::new(content).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
    }

    fn semantic_line(&self, row: &WrappedRow, selection: Option<(usize, usize)>) -> Line<'static> {
        if row.text.is_empty() {
            let style = selection.map_or(self.theme.text, |_| self.theme.editor_selection);
            return Line::from(Span::styled(String::new(), style));
        }
        if let Some((start, end)) = selection {
            let mut spans = Vec::new();
            if start > 0 {
                let prefix = take_chars(&row.text, 0, start);
                if !prefix.is_empty() {
                    spans.push(Span::styled(prefix, self.theme.text));
                }
            }
            let length = end.saturating_sub(start);
            if length > 0 {
                let selected = take_chars(&row.text, start, length);
                if !selected.is_empty() {
                    spans.push(Span::styled(selected, self.theme.editor_selection));
                }
            }
            if end < row.char_len {
                let suffix = take_chars(&row.text, end, row.char_len.saturating_sub(end));
                if !suffix.is_empty() {
                    spans.push(Span::styled(suffix, self.theme.text));
                }
            }
            Line::from(spans)
        } else {
            Line::from(Span::styled(row.text.clone(), self.theme.text))
        }
    }
}
