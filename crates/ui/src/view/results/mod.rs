mod cursor;
mod hitbox;
mod layout;
mod lines;
mod render;
mod scrollbars;
mod text;

pub(super) const COLUMN_SEPARATOR: &str = "│";

#[cfg(test)]
pub(super) use layout::estimate_horizontal_viewport_columns;
#[cfg(test)]
pub(super) use text::{displayed_value_width, format_cell_value};

#[cfg(test)]
mod tests;
