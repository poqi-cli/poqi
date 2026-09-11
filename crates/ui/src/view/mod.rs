mod editor;
mod layout;
mod results;
mod schema;
mod semantic;
mod settings;
mod status;
mod table_detail;
pub mod text_input;

use crate::{
    app::{App, UiLayer},
    input::FocusPanel,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding},
    Frame,
};

impl App {
    pub fn draw(&mut self, frame: &mut Frame<'_>) {
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(2)])
            .split(frame.area());

        let (schema_rect, editor_rect, semantic_rect, results_rect) = match self.zoomed_panel {
            Some(FocusPanel::Schema) => {
                let full = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(100)])
                    .split(vertical[0]);
                (full[0], Rect::default(), Rect::default(), Rect::default())
            }
            Some(FocusPanel::Editor) => {
                let full = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(100)])
                    .split(vertical[0]);
                (Rect::default(), full[0], Rect::default(), Rect::default())
            }
            Some(FocusPanel::SemanticSearch) => {
                let full = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(100)])
                    .split(vertical[0]);
                (Rect::default(), Rect::default(), full[0], Rect::default())
            }
            Some(FocusPanel::Results) => {
                let full = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(100)])
                    .split(vertical[0]);
                (Rect::default(), Rect::default(), Rect::default(), full[0])
            }
            _ => {
                let columns = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(26), Constraint::Percentage(74)])
                    .split(vertical[0]);
                let right = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
                    .split(columns[1]);
                let editors = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(66), Constraint::Percentage(34)])
                    .split(right[0]);
                (columns[0], editors[0], editors[1], right[1])
            }
        };

        self.schema_area = Some(schema_rect);
        self.schema_scrollbar = None;
        self.editor_area = Some(editor_rect);
        self.editor_text_area = None;
        self.editor_cursor = None;
        self.semantic_area = Some(semantic_rect);
        self.semantic_text_area = None;
        self.results_area = Some(results_rect);
        self.status_area = None;
        self.settings_hitbox = None;

        if schema_rect.width > 0 && schema_rect.height > 0 {
            self.draw_schema(frame, schema_rect);
        }
        if editor_rect.width > 0 && editor_rect.height > 0 {
            self.draw_editor(frame, editor_rect);
        }
        if semantic_rect.width > 0 && semantic_rect.height > 0 {
            self.draw_semantic(frame, semantic_rect);
        }
        if results_rect.width > 0 && results_rect.height > 0 {
            self.draw_results(frame, results_rect);
        } else {
            self.results_hitbox = None;
        }
        if self.table_detail.is_open() {
            if let Some(area) = self.schema_area {
                if area.width > 0 && area.height > 0 {
                    self.draw_table_detail(frame, area);
                }
            }
        }

        self.draw_status(frame, vertical[1]);

        self.draw_status_overlay(frame, frame.area());
        if self.settings_view.is_some() {
            self.draw_settings(frame, frame.area());
        }

        // Draw the editor completion popup last so it can extend over all panels,
        // including the status bar and settings overlays.
        if self.editor.completions_visible() {
            if let Some((cursor_x, cursor_y)) = self.editor_cursor {
                self.draw_completion_popup(frame, frame.area(), cursor_x, cursor_y);
            }
        }
    }

    pub(super) fn block_with_title(&self, title: &str, panel: FocusPanel) -> Block<'_> {
        let border_style = self.border_style_for(panel);
        let title_style = self.panel_title_style_for(panel);
        let accent = self.panel_accent_style_for(panel);
        let title_line = Line::from(vec![
            Span::styled("● ", accent),
            Span::styled(title.to_string(), title_style),
        ]);
        Block::default()
            .title(title_line)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .style(self.panel_background_for(panel))
            .padding(Padding::new(1, 1, 0, 0))
    }

    fn border_style_for(&self, panel: FocusPanel) -> Style {
        if matches!(self.layer, UiLayer::PanelFocused) && self.focus == panel {
            self.theme.border_focused
        } else if matches!(self.layer, UiLayer::PanelSelect) && self.panel_cursor == panel {
            self.theme.border_cursor
        } else {
            self.theme.border
        }
    }

    fn panel_background_for(&self, panel: FocusPanel) -> Style {
        if matches!(self.layer, UiLayer::PanelFocused) && self.focus == panel {
            self.theme.panel_background_focused
        } else if matches!(self.layer, UiLayer::PanelSelect) && self.panel_cursor == panel {
            self.theme.panel_background_selected
        } else {
            self.theme.panel_background
        }
    }

    fn panel_title_style_for(&self, panel: FocusPanel) -> Style {
        if matches!(self.layer, UiLayer::PanelFocused) && self.focus == panel {
            self.theme.panel_title_focused
        } else if matches!(self.layer, UiLayer::PanelSelect) && self.panel_cursor == panel {
            self.theme.panel_title_selected
        } else {
            self.theme.panel_title
        }
    }

    fn panel_accent_style_for(&self, panel: FocusPanel) -> Style {
        match panel {
            FocusPanel::Schema => self.theme.accent_tertiary,
            FocusPanel::Editor | FocusPanel::Results => self.theme.accent_primary,
            FocusPanel::SemanticSearch | FocusPanel::Status => self.theme.accent_secondary,
        }
    }
}
