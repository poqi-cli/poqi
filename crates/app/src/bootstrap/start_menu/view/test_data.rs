use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use super::{util::truncate_text, Frame};

pub(super) fn draw_test_data_generation(
    frame: &mut Frame<'_>,
    progress: Option<&str>,
    error: Option<&str>,
) {
    let size = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(size);

    let title = Paragraph::new("GENERATE TEST DATABASE")
        .alignment(Alignment::Center)
        .style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(
        title,
        Rect {
            x: size.x,
            y: size.height.saturating_sub(20) / 2,
            width: size.width,
            height: 1,
        },
    );

    // Center the info card so status and help text stay readable while progress updates.
    let info_area = Rect {
        x: size.width / 6,
        y: size.height.saturating_sub(14) / 2,
        width: size.width * 2 / 3,
        height: 10,
    };

    let info_text = if let Some(err) = error {
        truncate_text(err, 500)
    } else if let Some(prog) = progress {
        truncate_text(prog, 500)
    } else {
        "This will start a PostgreSQL container in Docker with test data:\n\n\
         - 3 schemas with 100 tables each\n\
         - schema1: 100 rows/table\n\
         - schema2: 1,000 rows/table\n\
         - schema3: 10,000 rows/table\n\n\
         Uses a dedicated localhost port range (starting 55,432) with an \
         'unless-stopped' restart policy so saved URIs stay valid after a reboot. \
         Each new generation picks the next free port for side-by-side datasets."
            .to_string()
    };

    let info_style = if error.is_some() {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else if progress.is_some() {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::White)
    };

    let info = Paragraph::new(info_text)
        .style(info_style)
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(if error.is_some() {
                    Color::Red
                } else {
                    Color::Green
                })),
        );

    frame.render_widget(info, info_area);

    let hints = if progress.is_some() {
        Paragraph::new("Generating... Please wait")
    } else {
        Paragraph::new("Enter: Start  |  Esc: Back  |  Shift+Esc: Quit")
    };

    frame.render_widget(
        hints
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray)),
        chunks[1],
    );
}
