use super::{
    displayed_value_width, estimate_horizontal_viewport_columns, format_cell_value,
    layout::column_separator_width, text::clip_text_to_width,
};
use crate::{
    app::{App, SemanticRuntime},
    terminal_capabilities::TerminalCapabilities,
    UiRuntimeSettings,
};
use poqi_catalog::CatalogSnapshot;
use poqi_config::AppConfig;
use poqi_engine::{ResultMetadata, ResultOrigin};
use poqi_search_semantic::SearchOptions;
use ratatui::{backend::TestBackend, layout::Rect, Terminal};
use std::{
    collections::HashMap,
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};
use tokio::sync::mpsc;

#[test]
fn format_cell_value_pads_short_text() {
    let text = format_cell_value("abc", 6);
    assert_eq!(text, "abc   ");
}

#[test]
fn format_cell_value_truncates_long_text() {
    let text = format_cell_value("abcdefgh", 5);
    assert_eq!(text, "abcd…");
}

#[test]
fn displayed_value_width_matches_truncation() {
    assert_eq!(displayed_value_width("abcdefgh", 5), 5);
}

#[test]
fn text_helpers_bound_large_ascii_and_combining_only_values() {
    let large_ascii = "a".repeat(1024 * 1024);
    let large_combining = "\u{0301}".repeat(512 * 1024);

    for _ in 0..8 {
        assert_eq!(format_cell_value(&large_ascii, 12), "aaaaaaaaaaa…");
        assert_eq!(displayed_value_width(&large_ascii, 12), 12);
        assert_eq!(clip_text_to_width(&large_ascii, 12), "aaaaaaaaaaa…");

        let formatted_combining = format_cell_value(&large_combining, 12);
        assert!(formatted_combining.ends_with("…           "));
        assert!(formatted_combining.len() <= 4 * 1024 + 16);
        assert_eq!(displayed_value_width(&large_combining, 12), 1);
        let clipped_combining = clip_text_to_width(&large_combining, 12);
        assert!(clipped_combining.ends_with('…'));
        assert!(clipped_combining.len() <= 4 * 1024 + 4);
    }
}

#[test]
fn text_helpers_preserve_ordinary_unicode() {
    let value = "A界e\u{301}";
    assert_eq!(format_cell_value(value, 6), "A界e\u{301}  ");
    assert_eq!(displayed_value_width(value, 6), 4);
    assert_eq!(clip_text_to_width(value, 6), value);
}

#[test]
fn estimated_horizontal_columns_skip_partial_last_column() {
    let widths = vec![8, 8, 8];
    let count = estimate_horizontal_viewport_columns(15, 1, &widths);
    assert_eq!(count, 1);
}

#[test]
fn estimated_horizontal_columns_never_returns_zero_for_visible_area() {
    let widths = vec![30, 30, 30];
    let count = estimate_horizontal_viewport_columns(5, 1, &widths);
    assert_eq!(count, 1);
}

#[test]
fn column_layout_keeps_partial_tail_column_visible() {
    let mut app = build_results_app(true);
    seed_wide_results(&mut app);
    let column_widths = app.measure_column_widths();
    let separator_width = column_separator_width(super::COLUMN_SEPARATOR);
    let tail_width: u16 = 4;
    let viewport_width = column_widths[0] + separator_width + tail_width;

    let layout = app.column_layout(viewport_width, separator_width, &column_widths);

    assert_eq!(layout.columns.len(), 2);
    assert_eq!(layout.columns[0].width, column_widths[0]);
    assert_eq!(layout.columns[1].index, 1);
    assert_eq!(layout.columns[1].width, tail_width);
    assert_eq!(layout.occupied_width, viewport_width);
    assert!(!layout.fully_visible);
}

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

fn next_store_path() -> std::path::PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let mut path = std::env::temp_dir();
    path.push(format!(
        "poqi-ui-results-test-store-{}.sqlite",
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

fn build_results_app(show_scrollbar: bool) -> App {
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
        CatalogSnapshot::default(),
        command_tx,
        response_rx,
        disabled_semantic_runtime(),
        TerminalCapabilities::testing(show_scrollbar),
    )
}

fn seed_wide_results(app: &mut App) {
    let metadata = ResultMetadata {
        source_table: None,
        row_identities: vec![None],
        source_columns: vec![None; 5],
        column_types: vec!["text".into(); 5],
        null_cells: vec![vec![false; 5]],
        origin: ResultOrigin::Unknown,
    };
    let headers = vec![
        "id".into(),
        "name".into(),
        "email".into(),
        "city".into(),
        "country".into(),
    ];
    let row = vec![
        "1".into(),
        "Alice".into(),
        "alice@example.com".into(),
        "Paris".into(),
        "FR".into(),
    ];
    app.results.set_data(headers, vec![row], metadata);
}

fn render_results(app: &mut App, width: u16, height: u16) {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| {
            let area = Rect::new(0, 0, width, height);
            app.draw_results(frame, area);
        })
        .expect("render");
}

#[test]
fn repeated_draws_keep_oversized_cells_bounded() {
    let mut app = build_results_app(false);
    let rows = vec![
        vec!["a".repeat(1024 * 1024)],
        vec!["\u{0301}".repeat(512 * 1024)],
    ];
    app.results.set_data(
        vec!["payload".into()],
        rows,
        ResultMetadata {
            source_table: None,
            row_identities: vec![None, None],
            source_columns: vec![None],
            column_types: vec!["text".into()],
            null_cells: vec![vec![false], vec![false]],
            origin: ResultOrigin::Unknown,
        },
    );

    for _ in 0..8 {
        render_results(&mut app, 40, 7);
    }
    assert_eq!(app.results.current_value().map(str::len), Some(1024 * 1024));
}

#[test]
fn pointerless_terminal_hides_horizontal_scrollbar_track() {
    let mut app = build_results_app(false);
    seed_wide_results(&mut app);
    render_results(&mut app, 40, 6);
    assert!(app.results_horizontal_scrollbar.is_none());
}

#[test]
fn pointer_friendly_terminal_displays_horizontal_scrollbar() {
    let mut app = build_results_app(true);
    seed_wide_results(&mut app);
    render_results(&mut app, 40, 6);
    assert!(app.results_horizontal_scrollbar.is_some());
}
