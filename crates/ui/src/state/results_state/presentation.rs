use poqi_catalog::display_identifier;
use unicode_width::UnicodeWidthStr;

pub(crate) const MAX_DISPLAY_COLUMN_WIDTH: u16 = 256;
pub(crate) const MAX_DISPLAY_FIELD_BYTES: usize = 4 * 1024;

pub(super) fn build_presentation_cache(
    headers: &[String],
    rows: &[Vec<String>],
) -> (Vec<String>, Vec<u16>) {
    let display_headers: Vec<String> = headers
        .iter()
        .map(|header| bounded_identifier_display(header))
        .collect();
    let mut widths: Vec<u16> = display_headers
        .iter()
        .map(|header| measured_column_width(header))
        .collect();

    for row in rows {
        for (column, value) in row.iter().enumerate().take(widths.len()) {
            widths[column] = widths[column].max(measured_column_width(value));
        }
    }

    (display_headers, widths)
}

pub(super) fn measured_column_width(value: &str) -> u16 {
    let width = bounded_value_width(value).saturating_add(2);
    u16::try_from(width.min(usize::from(MAX_DISPLAY_COLUMN_WIDTH)))
        .unwrap_or(MAX_DISPLAY_COLUMN_WIDTH)
}

pub(crate) fn bounded_prefix(value: &str) -> (&str, bool) {
    if value.len() <= MAX_DISPLAY_FIELD_BYTES {
        return (value, false);
    }

    let mut end = MAX_DISPLAY_FIELD_BYTES;
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    (&value[..end], true)
}

fn bounded_value_width(value: &str) -> usize {
    let (prefix, truncated) = bounded_prefix(value);
    UnicodeWidthStr::width(prefix).saturating_add(usize::from(truncated))
}

fn bounded_identifier_display(value: &str) -> String {
    let (prefix, truncated) = bounded_prefix(value);
    let mut displayed = display_identifier(prefix);
    if truncated {
        displayed.push('…');
    }
    displayed
}
