use ratatui::{layout::Rect, widgets::Paragraph, Frame};

use crate::{app::App, input::FocusPanel};

use super::{
    hitbox::build_results_hitbox,
    layout::{column_separator_width, estimate_horizontal_viewport_columns},
    scrollbars::HorizontalOverflowHints,
};

impl App {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn draw_results(&mut self, frame: &mut Frame<'_>, area: Rect) {
        // Render the results grid plus header rows and keep scroll/cursor state in sync.
        let row_count = self.results.row_count();
        let col_count = self.results.headers.len();
        let block_title = format!(
            "Results ({} {}, {} {})",
            row_count,
            if row_count == 1 { "row" } else { "rows" },
            col_count,
            if col_count == 1 { "col" } else { "cols" }
        );
        let block = self.block_with_title(&block_title, FocusPanel::Results);
        let inner_area = block.inner(area);
        frame.render_widget(block, area);
        self.results_hitbox = None;

        if inner_area.width == 0 || inner_area.height == 0 {
            return;
        }

        self.results_vertical_scrollbar = None;
        self.results_horizontal_scrollbar = None;

        let header_height = u16::from(!self.results.headers.is_empty());
        let mut body_height = inner_area.height.saturating_sub(header_height);
        let mut horizontal_reserved_rows = 0u16;

        let selection = self.results.selection_bounds();
        let focus_cell = self.results.focus_cell;
        let start_row = self.results.scroll_row();

        let separator_width = column_separator_width(super::COLUMN_SEPARATOR);
        let column_widths = self.measure_column_widths();
        if column_widths.is_empty() {
            return;
        }
        let viewport_columns_hint =
            estimate_horizontal_viewport_columns(inner_area.width, separator_width, &column_widths);
        self.results
            .ensure_focus_col_visible(inner_area.width, separator_width, &column_widths);
        let column_layout = self.column_layout(inner_area.width, separator_width, &column_widths);
        if column_layout.columns.is_empty() {
            return;
        }
        let scrolled_horizontally = self.results.scroll_col() > 0;
        let has_right_overflow = !column_layout.fully_visible;
        let needs_horizontal_scrollbar =
            (scrolled_horizontally || has_right_overflow) && body_height > 0;
        if needs_horizontal_scrollbar && inner_area.height > 1 {
            horizontal_reserved_rows = 1;
        }

        let horizontal_reserved = horizontal_reserved_rows > 0;
        let content_area = if horizontal_reserved {
            Rect {
                x: inner_area.x,
                y: inner_area.y,
                width: inner_area.width,
                height: inner_area.height.saturating_sub(horizontal_reserved_rows),
            }
        } else {
            inner_area
        };
        body_height = content_area.height.saturating_sub(header_height);
        let visible_rows_for_scroll = usize::from(body_height.max(1));
        self.results.ensure_focus_visible(visible_rows_for_scroll);
        self.results_hitbox = Some(build_results_hitbox(
            content_area,
            header_height,
            start_row,
            &column_layout,
        ));

        let mut lines = Vec::new();
        if header_height > 0 {
            if let Some(header_line) = self.results_header_line(&column_layout, inner_area.width) {
                lines.push(header_line);
            }
        }

        let visible_rows = usize::from(body_height);
        if visible_rows > 0 {
            lines.extend(self.results_data_lines(
                &column_layout,
                start_row,
                visible_rows,
                selection,
                focus_cell,
                inner_area.width,
            ));
        }

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, content_area);

        let viewport_rows = usize::from(body_height.max(1));
        self.results.sync_scrollbar_thumb(viewport_rows);
        let has_vertical_scrollbar = body_height > 0 && self.results.row_count() > viewport_rows;
        if has_vertical_scrollbar {
            self.draw_results_scrollbar(
                frame,
                content_area,
                header_height,
                body_height,
                viewport_rows,
            );
        }
        if needs_horizontal_scrollbar && horizontal_reserved {
            let viewport_columns = viewport_columns_hint.max(1);
            self.results.sync_horizontal_scrollbar(viewport_columns);
            let overflow_hints = HorizontalOverflowHints {
                left: scrolled_horizontally,
                right: has_right_overflow,
            };
            self.draw_results_horizontal_scrollbar(
                frame,
                content_area,
                has_vertical_scrollbar,
                viewport_columns,
                overflow_hints,
                self.results_horizontal_scrollbar_enabled(),
            );
        }

        self.position_results_cursor(
            frame,
            content_area,
            header_height,
            start_row,
            focus_cell,
            &column_layout,
        );
    }
}
