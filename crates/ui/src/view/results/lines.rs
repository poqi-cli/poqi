use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

use crate::{
    app::{App, UiLayer},
    input::FocusPanel,
};

use super::{layout::ColumnLayout, text::format_cell_value};

impl App {
    pub(super) fn results_header_line(
        &self,
        layout: &ColumnLayout,
        total_width: u16,
    ) -> Option<Line<'static>> {
        if self.results.headers.is_empty() {
            return None;
        }
        let mut spans = Vec::new();
        let focused = matches!(self.layer, UiLayer::PanelFocused)
            && matches!(self.focus, FocusPanel::Results);
        let style = if focused {
            self.panel_accent_style_for(FocusPanel::Results)
        } else {
            self.theme.panel_title
        }
        .add_modifier(Modifier::BOLD);

        for (visible_idx, column) in layout.columns.iter().enumerate() {
            let header = self.results.display_header(column.index)?;
            spans.push(Span::styled(format_cell_value(header, column.width), style));
            if visible_idx + 1 != layout.columns.len() && layout.separator_width > 0 {
                spans.push(Span::styled(
                    super::COLUMN_SEPARATOR.to_string(),
                    self.theme.text_muted,
                ));
            }
        }
        self.append_trailing_padding(&mut spans, total_width, layout.occupied_width);
        Some(Line::from(spans))
    }

    pub(super) fn results_data_lines(
        &self,
        layout: &ColumnLayout,
        start_row: usize,
        visible_rows: usize,
        selection: Option<((usize, usize), (usize, usize))>,
        focus_cell: (usize, usize),
        total_width: u16,
    ) -> Vec<Line<'static>> {
        if visible_rows == 0 {
            return Vec::new();
        }
        self.results
            .rows
            .iter()
            .enumerate()
            .skip(start_row)
            .take(visible_rows)
            .map(|(row_idx, row)| {
                self.build_row_line(row, row_idx, layout, selection, focus_cell, total_width)
            })
            .collect()
    }

    fn build_row_line(
        &self,
        row: &[String],
        row_idx: usize,
        layout: &ColumnLayout,
        selection: Option<((usize, usize), (usize, usize))>,
        focus_cell: (usize, usize),
        total_width: u16,
    ) -> Line<'static> {
        let mut spans = Vec::new();
        for (visible_idx, column) in layout.columns.iter().enumerate() {
            let mut value = row.get(column.index).map_or("", String::as_str);
            let mut is_null = self
                .results
                .null_cells
                .get(row_idx)
                .and_then(|cells| cells.get(column.index))
                .copied()
                .unwrap_or(false);
            if let Some(session) = self.results.edit_session() {
                if session.row == row_idx && session.column == column.index {
                    value = session.buffer();
                    is_null = session.is_null();
                }
            }
            let in_selection = selection.is_some_and(|((sr, sc), (er, ec))| {
                row_idx >= sr && row_idx <= er && column.index >= sc && column.index <= ec
            });
            let zebra = row_idx.is_multiple_of(2);
            let mut style = if zebra {
                self.theme.text.patch(self.theme.panel_background_selected)
            } else {
                self.theme.text
            };
            if (row_idx, column.index) == focus_cell {
                style = self
                    .theme
                    .grid_focus_row
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::UNDERLINED);
            } else if in_selection {
                style = self.theme.grid_selection;
            }
            if is_null {
                style = style.add_modifier(Modifier::ITALIC | Modifier::DIM);
            }
            spans.push(Span::styled(format_cell_value(value, column.width), style));
            if visible_idx + 1 != layout.columns.len() && layout.separator_width > 0 {
                spans.push(Span::styled(
                    super::COLUMN_SEPARATOR.to_string(),
                    self.theme.text_muted,
                ));
            }
        }
        self.append_trailing_padding(&mut spans, total_width, layout.occupied_width);
        Line::from(spans)
    }

    pub(super) fn append_trailing_padding(
        &self,
        spans: &mut Vec<Span<'static>>,
        total_width: u16,
        occupied_width: u16,
    ) {
        let padding = total_width.saturating_sub(occupied_width);
        if padding == 0 {
            return;
        }
        spans.push(Span::styled(" ".repeat(padding as usize), self.theme.text));
    }
}
