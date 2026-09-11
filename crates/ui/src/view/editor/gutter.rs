use ratatui::{style::Style, text::Span};

use crate::{app::App, view::text_input::WrappedRow};

pub(super) const GUTTER_MARGIN_WIDTH: usize = 2;

impl App {
    pub(crate) fn editor_gutter_width(&self) -> usize {
        const MIN_GUTTER_DIGITS: usize = 3;
        let line_count = self.editor.lines.len().max(1);
        let digits = decimal_digits(line_count).max(MIN_GUTTER_DIGITS);
        digits.saturating_add(GUTTER_MARGIN_WIDTH)
    }
}

pub(super) fn editor_gutter_span(
    row: &WrappedRow,
    gutter_width: usize,
    number_field_width: usize,
    inactive_style: Style,
    active_style: Style,
    is_active_row: bool,
) -> Span<'static> {
    if gutter_width == 0 {
        return Span::from("");
    }
    let number = row.visual_index.saturating_add(1);
    let field_width = number_field_width.max(1);
    let margin = " ".repeat(GUTTER_MARGIN_WIDTH);
    let text = format!("{number:>field_width$}{margin}");
    let style = if is_active_row {
        active_style
    } else {
        inactive_style
    };
    Span::styled(text, style)
}

fn decimal_digits(mut value: usize) -> usize {
    if value == 0 {
        return 1;
    }
    let mut digits = 0usize;
    while value > 0 {
        digits += 1;
        value /= 10;
    }
    digits
}
