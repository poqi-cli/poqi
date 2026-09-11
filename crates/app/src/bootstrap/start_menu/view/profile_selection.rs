use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use super::{
    super::state::{App, StatusKind},
    util::truncate_text,
    Frame,
};

pub(super) fn draw_profile_selection(app: &mut App, frame: &mut Frame<'_>) {
    let size = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(size);

    let mut items: Vec<ListItem> = app
        .profiles
        .iter()
        .enumerate()
        .map(|(i, profile)| {
            let style = if i == app.selected {
                Style::default()
                    .bg(Color::Cyan)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let display_text = profile_display_text(&profile.name, &profile.uri);
            ListItem::new(display_text).style(style)
        })
        .collect();

    let new_profile_style = if app.selected == app.profiles.len() {
        Style::default()
            .bg(Color::Magenta)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    items.push(ListItem::new("+ Create New Profile").style(new_profile_style));

    let test_db_style = if app.selected == app.profiles.len() + 1 {
        Style::default()
            .bg(Color::Green)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    items.push(ListItem::new("Generate Test Database (Docker)").style(test_db_style));

    let list_height = u16::try_from(items.len()).unwrap_or(u16::MAX);
    let list_area = centered_area(chunks[0], list_height, 3, 4, 32);
    app.profile_list_area = Some(list_area);

    let title = Paragraph::new("SELECT CONNECTION PROFILE")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center);

    frame.render_widget(
        title,
        Rect {
            x: list_area.x,
            y: list_area.y.saturating_sub(2),
            width: list_area.width,
            height: 1,
        },
    );

    let list = List::new(items).block(Block::default().borders(Borders::NONE));
    let mut list_state = ListState::default().with_selected(Some(app.selected));
    *list_state.offset_mut() = app.profile_list_offset;
    frame.render_stateful_widget(list, list_area, &mut list_state);
    app.profile_list_offset = list_state.offset();

    let mut hint_lines: Vec<Line> = Vec::new();

    if let Some(status) = &app.status {
        let color = match status.kind {
            StatusKind::Info => Color::Cyan,
            StatusKind::Error => Color::Red,
        };
        hint_lines.push(Line::from(Span::styled(
            status.message.clone(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )));
    }

    hint_lines.push(Line::from(Span::styled(
        "W/A/Up: Up  |  S/D/Down: Down  |  Enter/F: Select  |  E: Edit  |  Delete: Remove  |  Esc: Cancel  |  Shift+Esc: Quit",
        Style::default().fg(Color::DarkGray),
    )));

    let hints = Paragraph::new(hint_lines)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(hints, chunks[1]);
}

fn profile_display_text(name: &str, uri: &str) -> String {
    let safe_uri = crate::diagnostics::redact_uri(uri);
    format!("{name} - {}", truncate_text(&safe_uri, 80))
}

fn centered_area(area: Rect, list_height: u16, num: u16, den: u16, min_width: u16) -> Rect {
    let width = area
        .width
        .saturating_mul(num)
        .checked_div(den)
        .unwrap_or(area.width)
        .max(min_width)
        .min(area.width);
    let margin = (area.width.saturating_sub(width)) / 2;
    let height = list_height.min(area.height.saturating_sub(2)).max(1);
    let vertical_margin = (area.height.saturating_sub(height)) / 3;
    Rect {
        x: area.x + margin,
        y: area.y + vertical_margin + 1,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::profile_display_text;

    #[test]
    fn long_profile_list_scrolls_and_mouse_uses_visible_offset() {
        use super::super::super::state::{App, ProfileSelectionResult};
        use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        use ratatui::{backend::TestBackend, Terminal};
        let profiles = (0..35)
            .map(|index| poqi_store::StoredConnectionProfile {
                id: index,
                name: format!("profile-{index}"),
                uri: "postgres://user@localhost/app".into(),
                created_at: std::time::SystemTime::UNIX_EPOCH.into(),
                updated_at: std::time::SystemTime::UNIX_EPOCH.into(),
            })
            .collect();
        let mut app = App::new(
            poqi_store::Store::new_with_test_key(Some("unused.sqlite".into()), [7; 32]),
            profiles,
            35,
            false,
            None,
        );
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        let screen: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(screen.contains("+ Create New Profile"));
        assert!(app.profile_list_offset > 0);
        let expected = format!("profile-{}", app.profile_list_offset);
        let area = app.profile_list_area.unwrap();
        assert!(app.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x,
            row: area.y,
            modifiers: KeyModifiers::NONE
        }));
        let Some(ProfileSelectionResult::Selected { profile }) = app.result else {
            panic!("visible row should select a profile");
        };
        assert_eq!(profile.name, expected);
    }

    #[test]
    fn passive_profile_label_redacts_password() {
        let label = profile_display_text(
            "production",
            "postgres://user:top-secret@example.com/app?sslpassword=cert-secret",
        );

        assert!(label.contains("production - postgres://user:***@example.com/app"));
        assert!(!label.contains("top-secret"));
        assert!(!label.contains("cert-secret"));
    }

    #[test]
    fn passive_profile_label_hides_malformed_uri_credentials() {
        let label =
            profile_display_text("broken", "postgres://user:top-secret@localhost:invalid/app");

        assert_eq!(
            label,
            "broken - <invalid connection URI; credentials hidden>"
        );
        assert!(!label.contains("top-secret"));
    }
}
