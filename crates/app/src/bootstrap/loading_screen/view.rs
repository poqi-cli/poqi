use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use super::state::{LoadingApp, LoadingOutcome};

pub(super) fn render(app: &LoadingApp, frame: &mut Frame) {
    let size = frame.area();
    let bg = Block::default().style(Style::default().bg(Color::Rgb(20, 20, 26)));
    frame.render_widget(bg, size);

    let modal_area = centered_rect(60, 7, size);
    let border_color = match app.outcome {
        LoadingOutcome::Pending => Color::DarkGray,
        LoadingOutcome::Success { .. } => Color::Green,
        LoadingOutcome::Failure { .. } => Color::Red,
    };

    let title = Line::from(app.title.as_str()).style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(title);
    frame.render_widget(block, modal_area);

    let inner = Rect {
        x: modal_area.x + 1,
        y: modal_area.y + 1,
        width: modal_area.width.saturating_sub(2),
        height: modal_area.height.saturating_sub(2),
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(inner);

    let status_style = match app.outcome {
        LoadingOutcome::Pending => Style::default().fg(Color::White),
        LoadingOutcome::Success { .. } => Style::default().fg(Color::Green),
        LoadingOutcome::Failure { .. } => Style::default().fg(Color::Red),
    };

    let status = Paragraph::new(app.status_line().to_string())
        .style(status_style)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    frame.render_widget(status, chunks[0]);

    let hint = Paragraph::new("Esc will cancel once the UI loads.")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(hint, chunks[1]);
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let clamped_percent = percent_x.min(100);
    let desired_width_u32 =
        (u32::from(area.width) * u32::from(clamped_percent)).saturating_div(100);
    let desired_width = u16::try_from(desired_width_u32).unwrap_or(u16::MAX);
    let width = desired_width.max(32).min(area.width);
    let horizontal_margin = area.width.saturating_sub(width) / 2;
    let vertical_margin = area.height.saturating_sub(height) / 2;

    Rect {
        x: area.x + horizontal_margin,
        y: area.y + vertical_margin,
        width,
        height,
    }
}
