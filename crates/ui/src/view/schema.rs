use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
    Frame,
};

use poqi_catalog::display_identifier;

use crate::{
    app::{App, ScrollbarAxis, ScrollbarContext},
    input::FocusPanel,
    state::schema::SchemaItem,
};

impl App {
    pub(super) fn draw_schema(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let title = if let Some(schema) = self.schema.current_schema() {
            format!("Tables ({})", display_identifier(schema))
        } else {
            "Schemas".to_string()
        };

        let content_area = self
            .block_with_title(&title, FocusPanel::Schema)
            .inner(area);
        if content_area.width == 0 || content_area.height == 0 {
            let mut state = self.schema_list_state();
            frame.render_stateful_widget(
                List::new(Vec::<ListItem>::new())
                    .block(self.block_with_title(&title, FocusPanel::Schema)),
                area,
                &mut state,
            );
            self.schema_scrollbar = None;
            return;
        }

        let viewport_rows = usize::from(content_area.height.max(1));
        self.schema.ensure_selected_visible(viewport_rows);
        let block = self.block_with_title(&title, FocusPanel::Schema);

        let accent = self.panel_accent_style_for(FocusPanel::Schema);
        let items = self.schema_list_items(accent);

        let list = List::new(items).block(block);
        let mut state = self.schema_list_state();
        frame.render_stateful_widget(list, area, &mut state);

        self.schema_scrollbar = self.schema_scrollbar_context(content_area, viewport_rows);
        if let Some(ctx) = &self.schema_scrollbar {
            Self::render_scrollbar_with_context(
                frame,
                ctx,
                ScrollbarAxis::Vertical,
                self.schema.scroll_offset(),
                self.theme.panel_background_selected,
                self.theme.accent_secondary,
            );
        }

        self.schema_viewport_rows = viewport_rows;
    }

    fn schema_list_state(&self) -> ListState {
        ListState::default().with_offset(self.schema.scroll_offset())
    }

    fn schema_list_items(&self, accent: Style) -> Vec<ListItem<'_>> {
        let glyphs = self.glyphs();
        self.schema
            .items
            .iter()
            .enumerate()
            .map(|(idx, item)| match item {
                SchemaItem::Schema {
                    name,
                    expanded,
                    table_count,
                } => {
                    let mut style = if self.schema.selected == idx {
                        self.theme
                            .accent_secondary
                            .add_modifier(Modifier::BOLD)
                            .add_modifier(Modifier::UNDERLINED)
                    } else {
                        self.theme.text
                    };
                    if self.schema.selected != idx
                        && self
                            .schema
                            .last_fetched_table
                            .as_ref()
                            .is_some_and(|last| last.schema.as_deref() == Some(name))
                    {
                        style = style.add_modifier(Modifier::DIM);
                    }
                    let caret = if *table_count == 0 {
                        "  "
                    } else if *expanded {
                        "v "
                    } else {
                        "> "
                    };
                    let mut spans = Vec::with_capacity(3);
                    spans.push(Span::raw(caret));
                    if glyphs.schema_icon.is_empty() {
                        spans.push(Span::raw(" "));
                    } else {
                        spans.push(Span::styled(format!("{} ", glyphs.schema_icon), accent));
                    }
                    spans.push(Span::styled(display_identifier(name), style));
                    let line = Line::from(spans);
                    ListItem::new(line)
                }
                SchemaItem::Table {
                    schema,
                    name,
                    is_last_in_schema,
                } => {
                    let mut style = if self.schema.selected == idx {
                        self.theme
                            .accent_secondary
                            .add_modifier(Modifier::BOLD)
                            .add_modifier(Modifier::UNDERLINED)
                    } else {
                        self.theme.text
                    };
                    if self.schema.selected != idx
                        && self.schema.last_fetched_table.as_ref().is_some_and(|last| {
                            last == &poqi_catalog::QualifiedRelation::in_schema(schema, name)
                        })
                    {
                        style = style.add_modifier(Modifier::DIM);
                    }
                    let connector = if *is_last_in_schema {
                        glyphs.elbow
                    } else {
                        glyphs.branch
                    };
                    let mut spans = Vec::with_capacity(4);
                    spans.push(Span::raw(glyphs.space));
                    spans.push(Span::styled(connector.to_string(), accent));
                    spans.push(Span::raw(" "));
                    if !glyphs.table_icon.is_empty() {
                        spans.push(Span::styled(format!("{} ", glyphs.table_icon), accent));
                    }
                    spans.push(Span::styled(display_identifier(name), style));
                    let line = Line::from(spans);
                    ListItem::new(line)
                }
            })
            .collect()
    }

    fn schema_scrollbar_context(
        &self,
        content_area: Rect,
        viewport_rows: usize,
    ) -> Option<ScrollbarContext> {
        if viewport_rows >= self.schema.len() || content_area.height == 0 {
            return None;
        }
        let track = Rect {
            x: content_area
                .x
                .saturating_add(content_area.width.saturating_sub(1)),
            y: content_area.y,
            width: 1,
            height: content_area.height,
        };
        if track.height == 0 {
            return None;
        }
        let max_scroll = self.schema.len().saturating_sub(viewport_rows);
        Some(ScrollbarContext {
            track,
            viewport_items: viewport_rows,
            max_scroll,
        })
    }
}
