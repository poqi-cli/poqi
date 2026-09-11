use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use super::layout::centered_rect;
use crate::{
    app::{App, SettingsHitbox},
    state::settings::{SettingsAction, SettingsRow},
    theme::Theme,
};

impl App {
    pub(super) fn draw_settings(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let Some(view) = self.settings_view.clone() else {
            return;
        };

        let popup_area = centered_rect(82, 82, area);
        frame.render_widget(Clear, popup_area);

        let theme = self.theme.clone();
        let block = Block::default()
            .title(Line::styled(
                "Settings",
                self.theme.panel_title_focused.add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(self.theme.border_focused)
            .style(self.theme.panel_background_focused);
        frame.render_widget(block.clone(), popup_area);
        let inner = block.inner(popup_area);

        let action_index = view.rows().len().saturating_sub(1);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(4),
                Constraint::Length(1),
                Constraint::Length(2),
            ])
            .split(inner);

        let mut body_items = Vec::new();
        for (index, row) in view.rows().iter().enumerate().take(action_index) {
            let item = Self::row_item(&theme, &view, index, row);
            body_items.push(item);
        }
        let viewport_rows = usize::from(chunks[0].height.max(1));
        let offset = view.scroll_offset(viewport_rows);
        let mut state = ListState::default().with_offset(offset);
        self.settings_hitbox = Some(SettingsHitbox {
            popup_area,
            body_area: chunks[0],
            action_area: chunks[1],
            body_offset: offset,
            action_index,
        });
        frame.render_stateful_widget(
            List::new(body_items).block(Block::default()),
            chunks[0],
            &mut state,
        );

        let action_item = Self::action_item(&theme, &view, action_index);
        frame.render_widget(
            List::new(vec![action_item]).block(Block::default()),
            chunks[1],
        );

        self.render_instructions(frame, chunks[2]);
    }
}

impl App {
    fn row_item<'a>(
        theme: &'a Theme,
        view: &'a crate::state::settings::SettingsView,
        index: usize,
        row: &'a SettingsRow,
    ) -> ListItem<'a> {
        match row {
            SettingsRow::Field(field) => {
                let mut label = field.label().to_string();
                if field.is_modified() {
                    label.push('*');
                }
                let mut value = field.value_display();
                if view.editing && view.selected == index {
                    let buffer = view.editing_buffer();
                    let buffer = if buffer.is_empty() {
                        "_".to_string()
                    } else {
                        format!("{buffer}_")
                    };
                    value = buffer;
                }
                let mut spans = Vec::with_capacity(4);
                if view.selected == index {
                    spans.push(Span::styled("> ", theme.accent_primary));
                } else {
                    spans.push(Span::raw("  "));
                }
                spans.push(Span::styled(label, theme.text));
                spans.push(Span::raw(": "));
                spans.push(Span::styled(value, theme.accent_secondary));
                let mut item = ListItem::new(Line::from(spans));
                if view.selected == index {
                    let style = theme.panel_background_focused.patch(
                        theme
                            .highlight
                            .bg(Color::Rgb(35, 52, 104))
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    );
                    item = item.style(style);
                }
                item
            }
            SettingsRow::Action(SettingsAction::SaveAndExit) => {
                Self::action_item(theme, view, index)
            }
        }
    }

    fn action_item<'a>(
        theme: &'a Theme,
        view: &'a crate::state::settings::SettingsView,
        index: usize,
    ) -> ListItem<'a> {
        let mut label = "Save & Exit".to_string();
        if view.dirty {
            label.push_str(" (Enter)");
        }
        if view.selected == index {
            label = format!("> {label}");
        }
        let mut item = ListItem::new(Line::from(Span::styled(
            label,
            theme.accent_primary.add_modifier(Modifier::BOLD),
        )));
        if view.selected == index {
            let style = theme.panel_background_focused.patch(
                theme
                    .highlight
                    .bg(Color::Rgb(35, 52, 104))
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            );
            item = item.style(style);
        }
        item
    }

    fn render_instructions(&self, frame: &mut Frame<'_>, area: Rect) {
        let instructions = vec![Line::from(Span::styled(
            "Save & Exit is fixed at the bottom - W/S or up/down to navigate - Enter to save & exit - Esc to cancel",
            self.theme.text_muted.add_modifier(Modifier::ITALIC),
        ))];
        let paragraph = Paragraph::new(instructions)
            .wrap(Wrap { trim: true })
            .block(Block::default());
        frame.render_widget(paragraph, area);
    }
}
