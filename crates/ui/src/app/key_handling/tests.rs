use super::{
    helpers::{is_completion_commit_char, is_ctrl_char, is_shift_char, should_run_enter},
    *,
};
use crate::{
    app::SemanticRuntime, input::FocusPanel, terminal_capabilities::TerminalCapabilities,
    UiRuntimeSettings,
};
use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use poqi_catalog::{
    CatalogSnapshot, ColumnDefault, ColumnMeta, Nullability, RelationKind, TableId, TableMeta,
};
use poqi_config::AppConfig;
use poqi_search_semantic::SearchOptions;
use std::{
    collections::HashMap,
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};
use tokio::sync::mpsc;

fn disabled_semantic_runtime() -> SemanticRuntime {
    SemanticRuntime {
        tx: None,
        rx: None,
        options: SearchOptions {
            batch_size: 64,
            top_k: 50,
            threshold: None,
            dim: 768,
        },
        title_column: None,
        disabled_reason: Some("semantic search disabled in tests".to_string()),
        events: None,
        initial_loading_label: None,
        initial_ready_summary: None,
    }
}

fn sample_catalog() -> CatalogSnapshot {
    CatalogSnapshot {
        tables: vec![TableMeta {
            id: TableId("public.demo".to_string()),
            name: "demo".to_string(),
            schema: "public".to_string(),
            relation_kind: RelationKind::Table,
        }],
        columns: vec![ColumnMeta {
            schema: "public".to_string(),
            table: "demo".to_string(),
            name: "id".to_string(),
            data_type: "integer".to_string(),
            ordinal_position: 1,
            nullability: Nullability::NotNull,
            character_maximum_length: None,
            numeric_precision: None,
            numeric_scale: None,
            is_primary_key: true,
            default_kind: ColumnDefault::None,
            is_foreign_key: false,
        }],
        foreign_keys: Vec::new(),
    }
}

fn build_app() -> App {
    let (command_tx, _command_rx) = mpsc::unbounded_channel();
    let (_response_tx, response_rx) = mpsc::unbounded_channel();
    let mut schemas = HashMap::new();
    schemas.insert("public".to_string(), vec!["demo".to_string()]);
    let ui_settings = UiRuntimeSettings::default();
    App::new_with_capabilities(
        &AppConfig::default(),
        ui_settings,
        test_store(),
        schemas,
        sample_catalog(),
        command_tx,
        response_rx,
        disabled_semantic_runtime(),
        TerminalCapabilities::testing(true),
    )
}

fn next_store_path() -> std::path::PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let mut path = std::env::temp_dir();
    path.push(format!(
        "poqi-ui-keymap-test-store-{}.sqlite",
        NEXT_ID.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = fs::remove_file(&path);
    path
}

fn test_store() -> poqi_store::Store {
    let path = next_store_path();
    let store = poqi_store::Store::new_with_test_key(Some(path), [7; 32]);
    store.init().expect("init store");
    store
}

#[test]
fn ctrl_char_is_case_insensitive() {
    let event = KeyEvent::new_with_kind_and_state(
        KeyCode::Char('C'),
        KeyModifiers::CONTROL,
        KeyEventKind::Press,
        KeyEventState::empty(),
    );
    assert!(is_ctrl_char(&event, 'c'));
}

#[test]
fn shift_char_detects_caps_lock_combos() {
    let event = KeyEvent::new_with_kind_and_state(
        KeyCode::Char('z'),
        KeyModifiers::empty(),
        KeyEventKind::Press,
        KeyEventState::CAPS_LOCK,
    );
    assert!(is_shift_char(&event, 'z'));
}

#[test]
fn shift_char_respects_caps_lock_without_shift() {
    let event = KeyEvent::new_with_kind_and_state(
        KeyCode::Char('Z'),
        KeyModifiers::empty(),
        KeyEventKind::Press,
        KeyEventState::CAPS_LOCK,
    );
    assert!(!is_shift_char(&event, 'z'));
}

#[test]
fn shift_enter_runs_query() {
    let event = KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::SHIFT, KeyEventKind::Press);
    assert!(should_run_enter(&event));
}

#[test]
fn plain_enter_does_not_run_query() {
    let event = KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::empty(), KeyEventKind::Press);
    assert!(!should_run_enter(&event));
}

#[test]
fn panel_select_char_fast_stops_at_semantic_panel() {
    let mut app = build_app();
    assert_eq!(app.panel_cursor, FocusPanel::Schema);
    assert!(app.handle_panel_select_char('d', true));
    assert_eq!(app.panel_cursor, FocusPanel::SemanticSearch);
}

#[test]
fn panel_select_char_a_from_results_jumps_to_schema() {
    let mut app = build_app();
    app.panel_cursor = FocusPanel::Results;
    assert!(app.handle_panel_select_char('a', false));
    assert_eq!(app.panel_cursor, FocusPanel::Schema);
}

#[test]
fn panel_select_left_arrow_from_results_jumps_to_schema() {
    let mut app = build_app();
    app.layer = UiLayer::PanelSelect;
    app.panel_cursor = FocusPanel::Results;
    let event = KeyEvent::new(KeyCode::Left, KeyModifiers::NONE);
    assert!(!app.handle_window_select_key(event));
    assert_eq!(app.panel_cursor, FocusPanel::Schema);
}

#[test]
fn non_navigation_char_returns_false() {
    let mut app = build_app();
    assert!(!app.handle_panel_select_char('x', false));
}

#[test]
fn semicolon_commits_completion() {
    assert!(is_completion_commit_char(';'));
    assert!(!is_completion_commit_char('a'));
}

#[test]
fn left_arrow_moves_with_popup_and_accepts_at_the_new_cursor() {
    let mut app = build_app();
    app.focus_window(FocusPanel::Editor);
    app.editor.set_text("SELECT");
    app.editor.set_cursor(0, 3);
    app.editor.mark_completions_dirty();
    app.refresh_editor_completions();
    assert!(app.editor.completions_visible());

    app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));

    assert_eq!(app.editor.cursor_position(), (0, 2));
    let select_index = app
        .editor
        .completion_popup_items()
        .0
        .iter()
        .position(|item| item.label == "SELECT")
        .expect("SELECT completion after cursor move");
    app.editor.select_completion_index(select_index);
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(app.editor.text(), "SELECT");
}

#[test]
fn mouse_click_refreshes_from_where_to_select_list_before_accepting() {
    let mut app = build_app();
    app.focus_window(FocusPanel::Editor);
    app.editor.set_text("SELECT     FROM public.demo WHERE ");
    app.editor.move_to_end();
    app.editor.mark_completions_dirty();
    app.refresh_editor_completions();
    assert!(app
        .editor
        .completion_popup_items()
        .0
        .iter()
        .any(|item| item.label == "id = ?"));

    let text_area = ratatui::layout::Rect::new(1, 1, 60, 3);
    app.editor_text_area = Some(text_area);
    let gutter = u16::try_from(app.editor_gutter_width()).expect("gutter width");
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: text_area.x + gutter + u16::try_from("SELECT ".len()).expect("column"),
        row: text_area.y,
        modifiers: KeyModifiers::NONE,
    });

    assert_eq!(app.editor.cursor_position(), (0, "SELECT ".len()));
    let (items, _) = app.editor.completion_popup_items();
    assert!(items.iter().any(|item| item.label == "*"));
    assert!(!items.iter().any(|item| item.label == "id = ?"));
    let id_index = items
        .iter()
        .position(|item| item.label == "id")
        .expect("scoped column at SELECT list");
    app.editor.select_completion_index(id_index);
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(
        app.editor.text(),
        "SELECT \"id\"    FROM public.demo WHERE "
    );
}

#[test]
fn mouse_drag_refreshes_completions_for_the_selection_head() {
    let mut app = build_app();
    app.focus_window(FocusPanel::Editor);
    app.editor.set_text("SELECT target FROM public.demo");
    app.editor.move_to_end();
    let text_area = ratatui::layout::Rect::new(1, 1, 60, 3);
    app.editor_text_area = Some(text_area);
    let gutter = u16::try_from(app.editor_gutter_width()).expect("gutter width");
    let target_start = u16::try_from("SELECT ".len()).expect("target start");
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: text_area.x + gutter + target_start,
        row: text_area.y,
        modifiers: KeyModifiers::NONE,
    });
    assert!(app.editor.completions_visible());

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: text_area.x + gutter + target_start + 3,
        row: text_area.y,
        modifiers: KeyModifiers::NONE,
    });

    assert!(app.editor.selection_range().is_some());
    assert!(!app.editor.completions_visible());
}
