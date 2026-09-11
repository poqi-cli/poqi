use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Padding},
    Frame,
};

use poqi_catalog::{display_identifier, ColumnDefault, Nullability};

use crate::{
    app::App,
    state::{schema::SchemaItem, table_detail::TableColumnDetail},
    theme::Theme,
};

const PK_BADGE_WIDTH: u16 = 4;
const NULL_BADGE_WIDTH: u16 = 10;
const FK_BADGE_WIDTH: u16 = 4;
const DEFAULT_BADGE_WIDTH: u16 = 5;
const BADGE_PREFIX_SPACES: u16 = 2;
const BADGE_GAP: u16 = 1;
const BADGE_TRAILING_SPACE: u16 = 1;
const BADGE_BLOCK_WIDTH: u16 = BADGE_PREFIX_SPACES
    + PK_BADGE_WIDTH
    + BADGE_GAP
    + NULL_BADGE_WIDTH
    + BADGE_GAP
    + FK_BADGE_WIDTH
    + BADGE_GAP
    + DEFAULT_BADGE_WIDTH
    + BADGE_TRAILING_SPACE;

impl App {
    pub(super) fn draw_table_detail(&self, frame: &mut Frame<'_>, area: Rect) {
        let Some(detail) = self.table_detail.current() else {
            return;
        };
        if area.width < 10 || area.height < 4 {
            return;
        }

        // Anchor the popup beneath the selected table row within the schema list.
        let Some(selected_idx) = self.schema.selected_item().and_then(|item| {
            matches!(item, SchemaItem::Table { .. }).then_some(self.schema.selected)
        }) else {
            return;
        };
        let row_offset = selected_idx.saturating_sub(self.schema.scroll_offset());
        let screen = frame.area();
        let screen_top = screen.y;
        let screen_bottom = screen.y.saturating_add(screen.height.saturating_sub(1));
        let content_top = area.y.saturating_add(1);
        let row_y = content_top.saturating_add(u16::try_from(row_offset).unwrap_or(u16::MAX));

        let interior_width = screen.width.saturating_sub(2);
        let min_width = 26.min(interior_width);
        let max_width = interior_width.min(72);
        let panel_width = max_width.max(min_width);
        let desired_height =
            u16::try_from(detail.columns.len().saturating_add(2)).unwrap_or(u16::MAX);
        let max_height = screen.height.saturating_sub(2);
        if max_height == 0 {
            return;
        }
        let mut panel_height = desired_height.min(max_height).max(3);

        let space_below = screen_bottom.saturating_sub(row_y);
        let space_above = row_y.saturating_sub(screen_top);

        let panel_y = if space_below >= panel_height {
            row_y.saturating_add(1)
        } else if space_above >= panel_height {
            row_y.saturating_sub(panel_height)
        } else {
            panel_height =
                panel_height.min(space_above.saturating_add(space_below).saturating_add(1));
            screen_top
        };

        let popup_area = Rect {
            x: screen.x,
            y: panel_y,
            width: panel_width,
            height: panel_height,
        };

        frame.render_widget(Clear, popup_area);

        let title = format!("Columns — {}", detail.table);
        let name_column_width = detail
            .columns
            .iter()
            .map(|col| display_identifier(&col.name).chars().count())
            .max()
            .unwrap_or(0)
            .clamp(8, 26);
        let badge_block_width: u16 = BADGE_BLOCK_WIDTH;
        let type_width = popup_area
            .width
            .saturating_sub(4 + u16::try_from(name_column_width).unwrap_or(0) + badge_block_width)
            .max(12);

        let items: Vec<ListItem<'_>> = detail
            .columns
            .iter()
            .map(|column| {
                column_item(
                    &self.theme,
                    column,
                    name_column_width,
                    usize::from(type_width),
                )
            })
            .collect();

        let block = Block::default()
            .title(Line::from(vec![
                Span::styled("● ", self.theme.accent_tertiary),
                Span::styled(title, self.theme.panel_title_focused),
            ]))
            .borders(Borders::ALL)
            .border_style(self.theme.border_focused)
            .style(self.theme.panel_background_focused)
            .padding(Padding::horizontal(1));
        let list = List::new(items).block(block);
        frame.render_widget(list, popup_area);
    }
}

fn column_item(
    theme: &Theme,
    column: &TableColumnDetail,
    name_width: usize,
    type_width: usize,
) -> ListItem<'static> {
    ListItem::new(column_line(theme, column, name_width, type_width))
}

fn column_line(
    theme: &Theme,
    column: &TableColumnDetail,
    name_width: usize,
    type_width: usize,
) -> Line<'static> {
    let mut spans = Vec::with_capacity(14);
    let displayed_name = display_identifier(&column.name);
    spans.push(Span::styled(
        format!("{displayed_name:name_width$}"),
        theme.text.add_modifier(Modifier::BOLD),
    ));
    spans.push(space_span(BADGE_PREFIX_SPACES));
    spans.push(primary_key_badge(theme, column.is_primary_key));
    spans.push(space_span(BADGE_GAP));
    spans.push(nullability_badge(theme, &column.nullability));
    spans.push(space_span(BADGE_GAP));
    spans.push(foreign_key_badge(theme, column.is_foreign_key));
    spans.push(space_span(BADGE_GAP));
    spans.push(default_badge(theme, column));
    spans.push(space_span(BADGE_TRAILING_SPACE));
    let type_label = truncated_type_label(column, type_width);
    spans.push(Span::styled("[", theme.text_muted));
    spans.push(Span::styled(
        type_label,
        theme
            .text_muted
            .add_modifier(Modifier::ITALIC)
            .add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::styled("]", theme.text_muted));

    Line::from(spans)
}

fn space_span(width: u16) -> Span<'static> {
    if width == 0 {
        return Span::raw("");
    }
    Span::raw(" ".repeat(usize::from(width)))
}

fn padded_badge(
    label: &str,
    width: u16,
    highlight: bool,
    accent: Style,
    muted: Style,
) -> Span<'static> {
    let content_width = usize::from(width.saturating_sub(2));
    let content = format!("[{label:<content_width$}]");
    let style = if highlight {
        accent.add_modifier(Modifier::BOLD)
    } else {
        muted
    };
    Span::styled(content, style)
}

fn primary_key_badge(theme: &Theme, is_primary_key: bool) -> Span<'static> {
    let label = if is_primary_key { "PK" } else { "" };
    padded_badge(
        label,
        PK_BADGE_WIDTH,
        is_primary_key,
        theme.accent_primary,
        theme.text_muted,
    )
}

fn nullability_badge(theme: &Theme, nullability: &Nullability) -> Span<'static> {
    let is_nullable = matches!(nullability, Nullability::Nullable);
    let label = if is_nullable { "NULL" } else { "NOT NULL" };
    padded_badge(
        label,
        NULL_BADGE_WIDTH,
        !is_nullable,
        theme.accent_secondary,
        theme.text_muted,
    )
}

fn foreign_key_badge(theme: &Theme, is_foreign_key: bool) -> Span<'static> {
    let label = if is_foreign_key { "FK" } else { "" };
    padded_badge(
        label,
        FK_BADGE_WIDTH,
        is_foreign_key,
        theme.accent_tertiary,
        theme.text_muted,
    )
}

fn default_badge(theme: &Theme, column: &TableColumnDetail) -> Span<'static> {
    match column.default_kind {
        ColumnDefault::Generated => padded_badge(
            "GEN",
            DEFAULT_BADGE_WIDTH,
            true,
            theme.accent_primary,
            theme.text_muted,
        ),
        ColumnDefault::Default | ColumnDefault::Identity => padded_badge(
            "DEF",
            DEFAULT_BADGE_WIDTH,
            true,
            theme.accent_primary,
            theme.text_muted,
        ),
        ColumnDefault::None => padded_badge(
            "",
            DEFAULT_BADGE_WIDTH,
            false,
            theme.accent_primary,
            theme.text_muted,
        ),
    }
}

fn truncated_type_label(column: &TableColumnDetail, max_width: usize) -> String {
    let length = if let Some(len) = column.max_length {
        Some(len.to_string())
    } else if let Some(precision) = column.numeric_precision {
        let scale = column
            .numeric_scale
            .map_or_else(String::new, |scale| format!(",{scale}"));
        Some(format!("{precision}{scale}"))
    } else {
        None
    };

    let suffix = length.map(|len| format!("({len})")).unwrap_or_default();
    let base = if suffix.is_empty() {
        column.data_type.clone()
    } else {
        format!("{}{}", column.data_type, suffix)
    };
    let base = display_identifier(&base);
    if base.chars().count() <= max_width {
        return base;
    }
    let mut truncated = base
        .chars()
        .take(max_width.saturating_sub(1))
        .collect::<String>();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;
    use poqi_config::AppConfig;

    #[test]
    fn type_format_includes_precision_and_scale() {
        let column = TableColumnDetail {
            name: "price".into(),
            data_type: "numeric".into(),
            ordinal_position: 1,
            max_length: None,
            numeric_precision: Some(10),
            numeric_scale: Some(2),
            nullability: Nullability::NotNull,
            default_kind: ColumnDefault::None,
            is_primary_key: false,
            is_foreign_key: false,
        };
        assert_eq!(truncated_type_label(&column, 20), "numeric(10,2)");
    }

    #[test]
    fn column_line_includes_pk_and_not_null_badges() {
        let theme = Theme::from_config(&AppConfig::default());
        let column = TableColumnDetail {
            name: "id".into(),
            data_type: "uuid".into(),
            ordinal_position: 1,
            max_length: None,
            numeric_precision: None,
            numeric_scale: None,
            nullability: Nullability::NotNull,
            default_kind: ColumnDefault::None,
            is_primary_key: true,
            is_foreign_key: false,
        };
        let line = column_line(&theme, &column, 4, 12);
        let text: String = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert!(text.contains("[PK]"));
        assert!(text.contains("[NOT NULL]"));
        assert!(text.contains("[uuid]"));
    }

    #[test]
    fn column_line_marks_nullable_foreign_keys() {
        let theme = Theme::from_config(&AppConfig::default());
        let column = TableColumnDetail {
            name: "widget_id".into(),
            data_type: "integer".into(),
            ordinal_position: 2,
            max_length: None,
            numeric_precision: None,
            numeric_scale: None,
            nullability: Nullability::Nullable,
            default_kind: ColumnDefault::None,
            is_primary_key: false,
            is_foreign_key: true,
        };
        let line = column_line(&theme, &column, 10, 12);
        let text: String = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert!(text.contains("[NULL"));
        assert!(text.contains("[FK]"));
    }

    #[test]
    fn column_line_renders_default_and_generated_flags() {
        let theme = Theme::from_config(&AppConfig::default());
        let defaulted = TableColumnDetail {
            name: "created_at".into(),
            data_type: "timestamp".into(),
            ordinal_position: 3,
            max_length: None,
            numeric_precision: None,
            numeric_scale: None,
            nullability: Nullability::NotNull,
            default_kind: ColumnDefault::Default,
            is_primary_key: false,
            is_foreign_key: false,
        };
        let line = column_line(&theme, &defaulted, 10, 12);
        let text: String = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert!(text.contains("[DEF]"));

        let generated = TableColumnDetail {
            name: "doubled".into(),
            data_type: "integer".into(),
            ordinal_position: 4,
            max_length: None,
            numeric_precision: None,
            numeric_scale: None,
            nullability: Nullability::NotNull,
            default_kind: ColumnDefault::Generated,
            is_primary_key: false,
            is_foreign_key: false,
        };
        let generated_line = column_line(&theme, &generated, 10, 12);
        let generated_text: String = generated_line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert!(generated_text.contains("[GEN]"));
    }

    #[test]
    fn column_line_escapes_deceptive_metadata() {
        let theme = Theme::from_config(&AppConfig::default());
        let column = TableColumnDetail {
            name: "line\nitem".into(),
            data_type: "custom\u{202E}type".into(),
            ordinal_position: 1,
            max_length: None,
            numeric_precision: None,
            numeric_scale: None,
            nullability: Nullability::Nullable,
            default_kind: ColumnDefault::None,
            is_primary_key: false,
            is_foreign_key: false,
        };

        let text: String = column_line(&theme, &column, 20, 30)
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert!(text.contains("line\\u{000A}item"));
        assert!(text.contains("custom\\u{202E}type"));
        assert!(!text.contains('\n'));
        assert!(!text.contains('\u{202E}'));
    }
}
