use ratatui::{
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use super::layout::centered_rect;
use crate::{
    app::{App, UiLayer},
    input::FocusPanel,
    state::status::StatusLevel,
};

impl App {
    pub(super) fn draw_status(&mut self, frame: &mut Frame<'_>, area: Rect) {
        self.status_area = Some(area);
        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(self.border_style_for(FocusPanel::Status))
            .style(self.panel_background_for(FocusPanel::Status));
        let status_selected = matches!(self.layer, UiLayer::PanelSelect)
            && matches!(self.panel_cursor, FocusPanel::Status);
        let status_style = if matches!(self.layer, UiLayer::PanelFocused)
            && matches!(self.focus, FocusPanel::Status)
        {
            self.theme.highlight
        } else if status_selected {
            self.theme.border_focused
        } else {
            self.theme.text_muted
        };
        let mut segments = vec![Span::raw("  "), Span::styled("Settings", status_style)];
        segments.push(Span::raw("  |  "));
        segments.push(Span::styled(
            self.status.message.clone(),
            *self.status.style(&self.theme),
        ));
        let status_line = Line::from(segments);
        let paragraph = Paragraph::new(status_line).block(block);
        frame.render_widget(paragraph, area);
    }

    pub(super) fn draw_status_overlay(&self, frame: &mut Frame<'_>, area: Rect) {
        let Some(overlay) = self.status.overlay() else {
            return;
        };

        let popup_area = centered_rect(46, 30, area);
        frame.render_widget(Clear, popup_area);

        let (title, title_style) = match overlay.level {
            StatusLevel::Info => ("Info", self.theme.status_info),
            StatusLevel::Warning => ("Warning", self.theme.status_warning),
            StatusLevel::Success => ("Success", self.theme.status_success),
            StatusLevel::Error => ("Error", self.theme.status_error),
        };

        let block = Block::default()
            .title(Line::styled(
                title,
                title_style.add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(self.theme.border_focused)
            .style(self.theme.panel_background_focused);

        let hint = Span::styled(
            "This message will dismiss shortly.",
            self.theme.text_muted.add_modifier(Modifier::ITALIC),
        );

        let content = vec![
            Line::from(Span::styled(overlay.message.clone(), self.theme.text)),
            Line::from(""),
            Line::from(hint),
        ];

        let paragraph = Paragraph::new(content)
            .block(block)
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, popup_area);
    }
}
