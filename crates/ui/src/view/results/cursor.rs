use ratatui::{layout::Rect, Frame};

use crate::{app::App, input::FocusPanel};

use super::{layout::ColumnLayout, text::displayed_value_width};

impl App {
    pub(super) fn position_results_cursor(
        &self,
        frame: &mut Frame<'_>,
        content_area: Rect,
        header_height: u16,
        start_row: usize,
        focus_cell: (usize, usize),
        layout: &ColumnLayout,
    ) {
        if self.focus != FocusPanel::Results {
            return;
        }
        let Some((visible_idx, column)) = layout
            .columns
            .iter()
            .enumerate()
            .find(|(_, col)| col.index == focus_cell.1)
        else {
            return;
        };
        let Some(col_offset) = layout.offsets.get(visible_idx) else {
            return;
        };
        let row_in_view = focus_cell.0.saturating_sub(start_row);
        let Ok(row_offset) = u16::try_from(row_in_view) else {
            return;
        };
        let text_width = self
            .results
            .current_value()
            .map_or(0, |value| displayed_value_width(value, column.width));
        let text_width_u16 = u16::try_from(text_width).unwrap_or(column.width);
        let cursor_x = content_area
            .x
            .saturating_add(*col_offset)
            .saturating_add(text_width_u16);
        let cursor_y = content_area
            .y
            .saturating_add(header_height)
            .saturating_add(row_offset);
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}
