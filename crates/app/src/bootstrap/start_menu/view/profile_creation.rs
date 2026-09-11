use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use super::{
    super::{
        form::{ConnectionMode, CreationField, ProfileForm, TextInput},
        state::App,
    },
    Frame,
};

pub(super) fn draw_profile_creation(app: &mut App, frame: &mut Frame<'_>, form: &ProfileForm) {
    app.form_field_rects.clear();
    app.form_size_ok = frame.area().width >= 60 && frame.area().height >= 24;
    if !app.form_size_ok {
        frame.render_widget(
            Paragraph::new("Resize terminal to at least 60 columns and 24 rows to edit this connection. Your entries are retained. Esc: Back | Ctrl+C: Quit")
                .wrap(Wrap { trim: true }).style(Style::default().fg(Color::Yellow)),
            frame.area(),
        );
        return;
    }
    let area = centered_form_area(frame.area(), form.mode);
    draw_title(frame, area, form.name_locked);
    draw_name(app, frame, form, row(area, 1, area.width, 3));
    match form.mode {
        ConnectionMode::Url => draw_url_fields(app, frame, form, area),
        ConnectionMode::Structured => draw_structured_fields(app, frame, form, area),
    }
    draw_hints(frame, area, form.active_field);
}

fn centered_form_area(size: Rect, mode: ConnectionMode) -> Rect {
    let width = size.width.saturating_sub(4).min(64);
    let height = match mode {
        ConnectionMode::Url => 14,
        ConnectionMode::Structured => 20,
    };
    Rect {
        x: size.x + (size.width.saturating_sub(width)) / 2,
        y: size.y + size.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn draw_title(frame: &mut Frame<'_>, area: Rect, editing: bool) {
    let text = if editing {
        "Edit connection"
    } else {
        "New connection"
    };
    frame.render_widget(
        Paragraph::new(text).alignment(Alignment::Center).style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        row(area, 0, area.width, 1),
    );
}

fn draw_name(app: &mut App, frame: &mut Frame<'_>, form: &ProfileForm, area: Rect) {
    let label = if form.name_locked {
        "Profile name (fixed)"
    } else {
        "Profile name"
    };
    draw_input(
        app,
        frame,
        form,
        InputSpec {
            field: CreationField::Name,
            label,
            input: &form.name,
            display: &form.name.value,
            placeholder: "my_database",
            area,
            locked: form.name_locked,
        },
    );
}

fn draw_url_fields(app: &mut App, frame: &mut Frame<'_>, form: &ProfileForm, area: Rect) {
    draw_input(
        app,
        frame,
        form,
        InputSpec {
            field: CreationField::Uri,
            label: "Connection URL",
            input: &form.uri,
            display: &form.uri.value,
            placeholder: "postgresql://user:password@localhost:5432/database",
            area: row(area, 5, area.width, 3),
            locked: false,
        },
    );
    draw_mode_choice(app, frame, form, area, 8);
    draw_error(frame, form, area, 10);
}

fn draw_structured_fields(app: &mut App, frame: &mut Frame<'_>, form: &ProfileForm, area: Rect) {
    let gap = 2;
    let left_width = area.width.saturating_sub(gap) / 2;
    let right_width = area.width.saturating_sub(gap + left_width);
    let left = Rect {
        width: left_width,
        ..area
    };
    let right = Rect {
        x: area.x + left_width + gap,
        width: right_width,
        ..area
    };
    draw_structured_column(app, frame, form, left, true);
    draw_structured_column(app, frame, form, right, false);
    draw_mode_choice(app, frame, form, area, 14);
    draw_error(frame, form, area, 16);
}

fn draw_structured_column(
    app: &mut App,
    frame: &mut Frame<'_>,
    form: &ProfileForm,
    area: Rect,
    left: bool,
) {
    if left {
        draw_plain_input(
            app,
            frame,
            form,
            (CreationField::Host, "Host", &form.host),
            area,
            5,
        );
        draw_plain_input(
            app,
            frame,
            form,
            (CreationField::User, "User", &form.user),
            area,
            8,
        );
        draw_plain_input(
            app,
            frame,
            form,
            (CreationField::Database, "Database", &form.database),
            area,
            11,
        );
    } else {
        draw_plain_input(
            app,
            frame,
            form,
            (CreationField::Port, "Port", &form.port),
            area,
            5,
        );
        draw_input(
            app,
            frame,
            form,
            InputSpec {
                field: CreationField::Password,
                label: "Password",
                input: &form.password,
                display: &form.password.value,
                placeholder: "optional",
                area: row(area, 8, area.width, 3),
                locked: false,
            },
        );
        draw_choice(
            app,
            frame,
            form,
            CreationField::Tls,
            "TLS mode",
            form.tls.label(),
            row(area, 12, area.width, 1),
        );
    }
}

fn draw_plain_input(
    app: &mut App,
    frame: &mut Frame<'_>,
    form: &ProfileForm,
    descriptor: (CreationField, &str, &TextInput),
    area: Rect,
    y: u16,
) {
    let (field, label, input) = descriptor;
    draw_input(
        app,
        frame,
        form,
        InputSpec {
            field,
            label,
            input,
            display: &input.value,
            placeholder: "",
            area: row(area, y, area.width, 3),
            locked: false,
        },
    );
}

fn draw_mode_choice(app: &mut App, frame: &mut Frame<'_>, form: &ProfileForm, area: Rect, y: u16) {
    draw_choice(
        app,
        frame,
        form,
        CreationField::Mode,
        "Connection details",
        match form.mode {
            ConnectionMode::Url if form.name_locked => "URL (fixed)",
            ConnectionMode::Url => "URL (Enter: separate fields)",
            ConnectionMode::Structured => "Separate fields (Enter: URL)",
        },
        row(area, y, area.width, 1),
    );
}

fn draw_error(frame: &mut Frame<'_>, form: &ProfileForm, area: Rect, y: u16) {
    let Some(message) = form.error.as_deref() else {
        return;
    };
    frame.render_widget(
        Paragraph::new(message)
            .style(Style::default().fg(Color::Red))
            .wrap(Wrap { trim: true }),
        row(area, y, area.width, 3),
    );
}

#[derive(Clone, Copy)]
struct InputSpec<'a> {
    field: CreationField,
    label: &'a str,
    input: &'a TextInput,
    display: &'a str,
    placeholder: &'a str,
    area: Rect,
    locked: bool,
}

fn draw_input(app: &mut App, frame: &mut Frame<'_>, form: &ProfileForm, spec: InputSpec<'_>) {
    app.form_field_rects.push((spec.field, spec.area));
    let active = form.active_field == spec.field;
    let shown = if spec.display.is_empty() {
        spec.placeholder
    } else {
        spec.display
    };
    let style = if spec.display.is_empty() || spec.locked {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };
    let (scroll, cursor_column) = cursor_view(spec.display, spec.input.cursor, spec.area.width);
    let paragraph = Paragraph::new(shown)
        .style(style)
        .scroll((0, scroll))
        .block(input_block(spec.label, active));
    frame.render_widget(paragraph, spec.area);
    if active && !spec.locked {
        frame.set_cursor_position((
            spec.area.x.saturating_add(1).saturating_add(cursor_column),
            spec.area.y.saturating_add(1),
        ));
    }
}

fn cursor_view(display: &str, cursor: usize, area_width: u16) -> (u16, u16) {
    let prefix: String = display.chars().take(cursor).collect();
    let cursor_width = Line::from(prefix).width();
    let content_width = usize::from(area_width.saturating_sub(2)).max(1);
    let scroll = cursor_width.saturating_sub(content_width.saturating_sub(1));
    (
        u16::try_from(scroll).unwrap_or(u16::MAX),
        u16::try_from(cursor_width.saturating_sub(scroll)).unwrap_or(u16::MAX),
    )
}

fn input_block(label: &str, active: bool) -> Block<'_> {
    Block::default()
        .title(label)
        .borders(Borders::ALL)
        .border_style(if active {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        })
}

fn draw_choice(
    app: &mut App,
    frame: &mut Frame<'_>,
    form: &ProfileForm,
    field: CreationField,
    label: &str,
    value: &str,
    area: Rect,
) {
    app.form_field_rects.push((field, area));
    let active = form.active_field == field;
    frame.render_widget(
        Paragraph::new(format!("{label}: {value}")).style(if active {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        }),
        area,
    );
}

fn draw_hints(frame: &mut Frame<'_>, area: Rect, active: CreationField) {
    let hint = match active {
        CreationField::Mode => "Tab: Next   Enter/Space: Change   Esc: Back",
        CreationField::Tls => "Tab: Next   Space: Change   Enter: Connect   Esc: Back",
        _ => "Tab: Next   Enter: Connect   Esc: Back",
    };
    frame.render_widget(
        Paragraph::new(hint)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray)),
        row(area, area.height - 1, area.width, 1),
    );
}

fn row(area: Rect, y: u16, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x,
        y: area.y.saturating_add(y),
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{backend::TestBackend, Terminal};

    use super::super::super::{
        form::{CreationField, ProfileForm, TextInput},
        state::{App, UiState},
    };

    fn rendered(form: ProfileForm) -> String {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut app = App::new(
            poqi_store::Store::new_with_test_key(Some("unused.sqlite".into()), [7; 32]),
            Vec::new(),
            0,
            false,
            None,
        );
        app.state = UiState::ProfileCreation(Box::new(form));
        terminal.draw(|frame| app.draw(frame)).expect("render form");
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect()
    }

    #[test]
    fn url_password_is_visible_at_80_by_24() {
        let mut form = ProfileForm::new();
        form.name = TextInput::new("production");
        form.uri = TextInput::new("postgres://user:top-secret@db/prod?sslmode=require");
        form.active_field = CreationField::Uri;
        let screen = rendered(form);
        assert!(screen.contains("top-secret"));
        assert!(!screen.contains("Save locally"));
    }

    #[test]
    fn long_url_scrolls_to_the_caret() {
        let mut form = ProfileForm::new();
        form.name = TextInput::new("long");
        form.uri = TextInput::new(format!(
            "postgres://user@host/{}/database",
            "segment".repeat(20)
        ));
        form.active_field = CreationField::Uri;
        let screen = rendered(form);
        assert!(screen.contains("database"));
        assert!(!screen.contains("postgres://user@host"));
    }

    #[test]
    fn small_terminal_keeps_draft_and_blocks_hidden_submission() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        for (width, height) in [(40, 24), (80, 18)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut app = App::new(
                poqi_store::Store::new_with_test_key(Some("unused.sqlite".into()), [7; 32]),
                Vec::new(),
                0,
                false,
                None,
            );
            let mut form = ProfileForm::new();
            form.name = TextInput::new("retained");
            app.state = UiState::ProfileCreation(Box::new(form));
            terminal.draw(|frame| app.draw(frame)).unwrap();
            let screen: String = terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect();
            assert!(screen.contains("Resize terminal"));
            assert!(!app.form_size_ok);
            assert!(!app
                .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
                .unwrap());
            app.handle_paste("discard this paste");
            assert!(app.result.is_none());
            let UiState::ProfileCreation(form) = &app.state else {
                panic!("draft must stay open");
            };
            assert_eq!(form.name.value, "retained");
            app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
                .unwrap();
            assert!(matches!(app.state, UiState::ProfileSelection));
        }
    }
}
