use super::requests::PostResultsAction;
use super::{
    ActiveSemanticJob, App, MutationContext, ResultsHitColumn, ResultsHitbox, ScrollbarContext,
    SemanticRuntime, SettingsHitbox, UiLayer,
};
use crate::{
    engine_worker::{EngineCommand, EngineRequestKind, EngineResponse},
    input::{Action, FocusPanel},
    semantic_worker::{SemanticResponse, SemanticStats},
    state::schema::SchemaItem,
    terminal_capabilities::TerminalCapabilities,
    UiRuntimeSettings,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use poqi_catalog::{
    CatalogSnapshot, ColumnDefault, ColumnMeta, Nullability, QualifiedRelation, RelationKind,
    TableId, TableMeta,
};
use poqi_config::AppConfig;
use poqi_engine::{
    CrudAction, QueryResult, ResultMetadata, ResultOrigin, RowIdentity, RunSqlRefresh,
};
use poqi_search_semantic::SearchOptions;
use poqi_store::SemanticRuntimePreference;
use ratatui::layout::Rect;
use std::{
    collections::HashMap,
    fs,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};
use tokio::sync::mpsc;

fn next_store_path() -> std::path::PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let mut path = std::env::temp_dir();
    path.push(format!(
        "poqi-ui-test-store-{}.sqlite",
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

fn row_identity(ctid: &str) -> RowIdentity {
    RowIdentity {
        relation_oid: 1,
        xmin: "1".into(),
        primary_key: Vec::new(),
        table_oid: 1,
        ctid: ctid.to_string(),
    }
}

fn relation(schema: &str, name: &str) -> QualifiedRelation {
    QualifiedRelation::in_schema(schema, name)
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

fn test_app(config: &AppConfig) -> App {
    let (command_tx, _command_rx) = mpsc::unbounded_channel();
    let (_response_tx, response_rx) = mpsc::unbounded_channel();
    let mut schemas = HashMap::new();
    schemas.insert("public".to_string(), vec!["demo".to_string()]);
    let catalog = sample_catalog();
    let ui_settings = UiRuntimeSettings::default();
    App::new_with_capabilities(
        config,
        ui_settings,
        test_store(),
        schemas,
        catalog,
        command_tx,
        response_rx,
        disabled_semantic_runtime(),
        TerminalCapabilities::testing(true),
    )
}

fn test_app_with_channels(config: &AppConfig) -> (App, mpsc::UnboundedReceiver<EngineCommand>) {
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let (_response_tx, response_rx) = mpsc::unbounded_channel();
    let mut schemas = HashMap::new();
    schemas.insert("public".to_string(), vec!["demo".to_string()]);
    let catalog = sample_catalog();
    let ui_settings = UiRuntimeSettings::default();
    (
        App::new_with_capabilities(
            config,
            ui_settings,
            test_store(),
            schemas,
            catalog,
            command_tx,
            response_rx,
            disabled_semantic_runtime(),
            TerminalCapabilities::testing(true),
        ),
        command_rx,
    )
}

fn test_app_with_channels_and_responses(
    config: &AppConfig,
) -> (
    App,
    mpsc::UnboundedReceiver<EngineCommand>,
    mpsc::UnboundedSender<EngineResponse>,
) {
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let (response_tx, response_rx) = mpsc::unbounded_channel();
    let mut schemas = HashMap::new();
    schemas.insert("public".to_string(), vec!["demo".to_string()]);
    let catalog = sample_catalog();
    let ui_settings = UiRuntimeSettings::default();
    (
        App::new_with_capabilities(
            config,
            ui_settings,
            test_store(),
            schemas,
            catalog,
            command_tx,
            response_rx,
            disabled_semantic_runtime(),
            TerminalCapabilities::testing(true),
        ),
        command_rx,
        response_tx,
    )
}

fn seed_editable_results(app: &mut App) {
    let limit = app.ui_settings.select_top_limit();
    let metadata = ResultMetadata {
        source_table: Some(relation("public", "demo")),
        row_identities: vec![Some(row_identity("0")), Some(row_identity("1"))],
        source_columns: vec![Some("col".into())],
        column_types: vec!["text".into()],
        null_cells: Vec::new(),
        origin: ResultOrigin::SelectTop {
            table: relation("public", "demo"),
            limit,
        },
    };
    app.results.set_data(
        vec!["col".into()],
        vec![vec!["alpha".into()], vec!["bravo".into()]],
        metadata,
    );
}

#[test]
fn typing_q_in_editor_inserts_character() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Editor);
    let before = app.editor.lines[0].clone();
    let event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    app.handle_key(event);
    assert!(app.editor.lines[0].ends_with('q'));
    assert_eq!(app.editor.lines[0].len(), before.len() + 1);
}

#[test]
fn sql_preview_truncates_unicode_on_character_boundary() {
    let sql = format!("SELECT '{}ä';", "a".repeat(39));
    let preview = App::preview_sql(&sql);
    assert_eq!(preview.chars().count(), 49);
    assert!(preview.ends_with('…'));
}

#[test]
fn newer_sql_cancels_and_fences_older_semantic_result() {
    let (semantic_tx, semantic_rx) = mpsc::unbounded_channel();
    let (mut app, mut engine_commands) = test_app_with_channels(&AppConfig::default());
    app.semantic_rx = Some(semantic_rx);
    let cancel = Arc::new(AtomicBool::new(false));
    app.active_semantic = Some(ActiveSemanticJob {
        id: 41,
        table: Some("public.demo".to_string()),
        cancel_flag: Arc::clone(&cancel),
    });
    app.results.rows = vec![vec!["newer SQL".to_string()]];

    app.editor.set_text("SELECT 2;");
    app.run_query();
    assert!(cancel.load(Ordering::Relaxed));
    assert!(matches!(
        engine_commands.try_recv(),
        Ok(EngineCommand::RunSql { .. })
    ));

    semantic_tx
        .send(SemanticResponse::Success {
            request_id: 41,
            result: Box::new(QueryResult::empty()),
            stats: SemanticStats {
                matched_rows: 0,
                top_score: 0.0,
                fallback_notice: None,
                backend_label: "test".to_string(),
                elapsed_ms: 1,
                cache_hits: 0,
                cache_misses: 0,
            },
        })
        .expect("semantic response");
    app.poll_semantic();

    assert_eq!(app.results.rows, vec![vec!["newer SQL".to_string()]]);
}

#[test]
fn stale_mutation_success_is_disclosed_without_overwriting_newer_results() {
    let mut app = test_app(&AppConfig::default());
    app.results.rows = vec![vec!["newer result".to_string()]];
    app.start_request(
        52,
        EngineRequestKind::RunSql,
        "newer query".to_string(),
        None,
        false,
    );

    app.handle_engine_response(EngineResponse::Success {
        request_id: 51,
        kind: EngineRequestKind::UpdateCell,
        result: Box::new(QueryResult::empty()),
    });

    assert_eq!(app.requests.active().map(|request| request.id), Some(52));
    assert_eq!(app.results.rows, vec![vec!["newer result".to_string()]]);
    assert!(app.status.message.contains("Previous row update"));
    assert!(app.status.message.contains("completed successfully"));
}

#[test]
fn confirmed_old_cancellation_preserves_new_request_without_popup() {
    let mut app = test_app(&AppConfig::default());
    app.start_request(
        62,
        EngineRequestKind::SelectTop,
        "newer preview".into(),
        None,
        false,
    );
    app.handle_engine_response(EngineResponse::Canceled {
        request_id: 61,
        kind: EngineRequestKind::RunSql,
        message: "canceling statement due to user request".into(),
    });
    assert_eq!(app.requests.active().map(|request| request.id), Some(62));
    assert!(app.status.overlay().is_none());
    assert!(!app
        .catalog_refresh
        .ready_to_dispatch(std::time::Instant::now()));
}

#[test]
fn active_ambiguous_ddl_error_queues_catalog_reconciliation() {
    let mut app = test_app(&AppConfig::default());
    app.start_request(
        61,
        EngineRequestKind::RunSql,
        "ALTER TABLE".into(),
        None,
        true,
    );
    app.handle_engine_response(EngineResponse::Error {
        request_id: 61,
        kind: EngineRequestKind::RunSql,
        message: "database operation outcome is unknown".into(),
    });
    assert!(!app.requests.is_active());
    assert!(app.status.overlay().is_some());
    assert!(app
        .catalog_refresh
        .ready_to_dispatch(std::time::Instant::now()));
}

#[test]
fn stale_sql_error_preserves_original_outcome_in_popup() {
    let mut app = test_app(&AppConfig::default());
    app.start_request(
        62,
        EngineRequestKind::SelectTop,
        "newer preview".to_string(),
        None,
        false,
    );
    let original = "database connection was lost; the operation outcome is unknown";

    app.handle_engine_response(EngineResponse::Error {
        request_id: 61,
        kind: EngineRequestKind::RunSql,
        message: original.to_string(),
    });

    assert_eq!(app.requests.active().map(|request| request.id), Some(62));
    let popup = app.status.overlay().expect("stale SQL outcome popup");
    assert!(popup.message.contains("Previous SQL request"));
    assert!(popup.message.contains(original));
    assert!(app
        .catalog_refresh
        .ready_to_dispatch(std::time::Instant::now()));
}

#[test]
fn stale_sql_success_queues_conservative_catalog_refresh() {
    let mut app = test_app(&AppConfig::default());
    app.results.rows = vec![vec!["newer result".to_string()]];
    app.start_request(
        72,
        EngineRequestKind::SelectTop,
        "newer preview".to_string(),
        None,
        false,
    );

    app.handle_engine_response(EngineResponse::Success {
        request_id: 71,
        kind: EngineRequestKind::RunSql,
        result: Box::new(QueryResult::empty()),
    });

    assert_eq!(app.results.rows, vec![vec!["newer result".to_string()]]);
    assert!(app.status.message.contains("Previous SQL request"));
    assert!(app
        .catalog_refresh
        .ready_to_dispatch(std::time::Instant::now()));
}

#[test]
fn selecting_new_table_preempts_obsolete_preview() {
    let (mut app, mut commands, responses) =
        test_app_with_channels_and_responses(&AppConfig::default());
    app.schema.items.push(SchemaItem::Table {
        schema: "public".to_string(),
        name: "widgets".to_string(),
        is_last_in_schema: true,
    });
    app.catalog.tables.push(TableMeta {
        id: TableId("public.widgets".to_string()),
        name: "widgets".to_string(),
        schema: "public".to_string(),
        relation_kind: RelationKind::Table,
    });

    app.auto_fetch_on_table_selection();
    let first_id = match commands.try_recv().expect("first preview") {
        EngineCommand::Crud {
            request_id,
            kind: EngineRequestKind::SelectTop,
            ..
        } => request_id,
        other => panic!("unexpected first command: {other:?}"),
    };
    app.schema.select_index(2);
    app.auto_fetch_on_table_selection();
    let second_id = match commands.try_recv().expect("replacement preview") {
        EngineCommand::Crud {
            request_id,
            kind: EngineRequestKind::SelectTop,
            ..
        } => request_id,
        other => panic!("unexpected replacement command: {other:?}"),
    };
    assert_ne!(first_id, second_id);
    assert_eq!(
        app.schema.last_fetched_table,
        Some(relation("public", "widgets"))
    );

    responses
        .send(EngineResponse::Success {
            request_id: first_id,
            kind: EngineRequestKind::SelectTop,
            result: Box::new(QueryResult::empty()),
        })
        .expect("stale response");
    app.poll_engine();
    assert_eq!(
        app.requests.active().map(|request| request.id),
        Some(second_id)
    );

    responses
        .send(EngineResponse::Success {
            request_id: second_id,
            kind: EngineRequestKind::SelectTop,
            result: Box::new(QueryResult::empty()),
        })
        .expect("current response");
    app.poll_engine();
    assert!(!app.requests.is_active());
}

#[test]
fn deceptive_relation_requires_manual_preview_and_keeps_exact_identity() {
    let (mut app, mut commands) = test_app_with_channels(&AppConfig::default());
    let name = "demo\u{202E}hidden";
    app.schema.items.push(SchemaItem::Table {
        schema: "public".into(),
        name: name.into(),
        is_last_in_schema: true,
    });
    app.catalog.tables.push(TableMeta {
        id: TableId("deceptive fixture".into()),
        name: name.into(),
        schema: "public".into(),
        relation_kind: RelationKind::Table,
    });
    app.schema.select_index(app.schema.items.len() - 1);
    app.auto_fetch_on_table_selection();
    assert!(commands.try_recv().is_err());
    assert!(app.status.message.contains("\\u{202E}"));
    assert!(!app.status.message.contains('\u{202E}'));
    app.handle_action(Action::SelectTop);
    match commands.try_recv().expect("manual preview") {
        EngineCommand::Crud {
            action: CrudAction::SelectTop { table, .. },
            ..
        } => {
            assert_eq!(table, relation("public", name));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn dotted_relation_selection_preserves_components_in_preview_request() {
    let (mut app, mut commands) = test_app_with_channels(&AppConfig::default());
    app.schema.items.push(SchemaItem::Table {
        schema: "schema.with.dot".to_string(),
        name: "table.with.dot".to_string(),
        is_last_in_schema: true,
    });
    app.catalog.tables.push(TableMeta {
        id: TableId("complex fixture".to_string()),
        name: "table.with.dot".to_string(),
        schema: "schema.with.dot".to_string(),
        relation_kind: RelationKind::Table,
    });
    app.schema.select_index(app.schema.items.len() - 1);

    app.select_top();

    assert!(app
        .editor
        .text()
        .starts_with("SELECT * FROM \"schema.with.dot\".\"table.with.dot\" LIMIT "));
    let EngineCommand::Crud {
        action: CrudAction::SelectTop { table, .. },
        ..
    } = commands.try_recv().expect("complex relation preview")
    else {
        panic!("expected SelectTop CRUD command");
    };
    assert_eq!(table, relation("schema.with.dot", "table.with.dot"));
}

#[test]
fn quit_signals_semantic_cancellation_before_teardown() {
    let mut app = test_app(&AppConfig::default());
    let cancel = Arc::new(AtomicBool::new(false));
    app.active_semantic = Some(ActiveSemanticJob {
        id: 7,
        table: Some("public.demo".to_string()),
        cancel_flag: Arc::clone(&cancel),
    });

    assert!(app.handle_action(Action::Quit));
    assert!(cancel.load(Ordering::Relaxed));
}

#[tokio::test(flavor = "current_thread")]
async fn semantic_runtime_off_to_auto_starts_initialization_immediately() {
    let mut config = AppConfig::default();
    config.search.model_dir = next_store_path().with_extension("semantic-models");
    let mut app = test_app(&config);
    let handles = Arc::clone(&app.semantic_handles);
    app.open_settings_modal();
    let runtime_index = app
        .settings_view
        .as_ref()
        .expect("settings view")
        .rows()
        .iter()
        .position(|row| {
            matches!(
                row,
                crate::state::settings::SettingsRow::Field(field)
                    if field.label().starts_with("Semantic runtime")
            )
        })
        .expect("semantic runtime setting");
    let view = app.settings_view.as_mut().expect("settings view");
    let crate::state::settings::SettingsRow::Field(runtime_field) = &view.rows()[runtime_index]
    else {
        panic!("runtime row should be a field");
    };
    assert_eq!(runtime_field.label(), "Semantic runtime");
    view.selected = runtime_index;
    view.nudge_current(true);

    app.save_settings();

    assert_eq!(
        app.store
            .settings()
            .semantic_runtime_preference()
            .expect("stored runtime"),
        Some(SemanticRuntimePreference::Auto)
    );
    assert!(app.status.message.contains("initialization started"));
    assert!(app.semantic_tx.is_none());
    assert!(app.semantic_events.is_some());
    assert!(app.semantic_disabled_reason.is_none());
    assert_eq!(app.semantic_coordinator.entry_count(), 1);
    assert!(handles.lock().is_empty());
}

#[test]
fn disabling_semantic_runtime_applies_immediately() {
    let mut config = AppConfig::default();
    config.search.semantic_runtime_preference = SemanticRuntimePreference::Auto;
    let mut app = test_app(&config);
    app.open_settings_modal();
    let view = app.settings_view.as_mut().expect("settings view");
    let runtime_index = view
        .rows()
        .iter()
        .position(|row| {
            matches!(
                row,
                crate::state::settings::SettingsRow::Field(field)
                    if field.label() == "Semantic runtime"
            )
        })
        .expect("semantic runtime setting");
    view.selected = runtime_index;
    view.nudge_current(false);

    app.save_settings();

    assert_eq!(
        app.config.search.semantic_runtime_preference,
        SemanticRuntimePreference::Off
    );
    assert!(app.status.message.contains("semantic search is off"));
    assert!(app.semantic_events.is_none());
    assert!(app.semantic_tx.is_none());
    assert!(app
        .semantic_disabled_reason
        .as_deref()
        .is_some_and(|reason| reason.contains("Semantic search is off")));
}

#[tokio::test(flavor = "current_thread")]
async fn prior_backend_requires_restart_after_off() {
    let mut app = test_app(&AppConfig::default());
    let mut prior_config = AppConfig::default();
    prior_config.search.semantic_runtime_preference = SemanticRuntimePreference::Cpu;
    prior_config.search.model_dir = next_store_path().with_extension("prior-semantic-models");
    app.semantic_coordinator.attach(
        prior_config.clone(),
        prior_config.search.model_dir.clone(),
        SemanticRuntimePreference::Cpu,
        "test bootstrap".to_string(),
        Arc::clone(&app.semantic_handles),
        mpsc::unbounded_channel().0,
    );
    app.open_settings_modal();
    let view = app.settings_view.as_mut().expect("settings view");
    let runtime_index = view
        .rows()
        .iter()
        .position(|row| {
            matches!(
                row,
                crate::state::settings::SettingsRow::Field(field)
                    if field.label() == "Semantic runtime"
            )
        })
        .expect("semantic runtime setting");
    view.selected = runtime_index;
    view.nudge_current(true);
    view.nudge_current(true);

    app.save_settings();

    assert_eq!(
        app.config.search.semantic_runtime_preference,
        SemanticRuntimePreference::Gpu
    );
    assert!(app.status.message.contains("restart poqi"));
    assert!(app.semantic_events.is_none());
    assert_eq!(app.semantic_coordinator.entry_count(), 1);
}

#[test]
fn wasd_and_arrows_drive_window_cursor() {
    let mut app = test_app(&AppConfig::default());
    assert_eq!(app.panel_cursor, FocusPanel::Schema);
    app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    assert_eq!(app.panel_cursor, FocusPanel::Editor);
    app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
    assert_eq!(app.panel_cursor, FocusPanel::SemanticSearch);
    app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
    assert_eq!(app.panel_cursor, FocusPanel::Results);
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    assert_eq!(app.panel_cursor, FocusPanel::Status);
}

#[test]
fn vertical_nav_reaches_status_panel() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Results);
    app.enter_window_layer();
    app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
    assert_eq!(app.panel_cursor, FocusPanel::Status);
    app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
    assert_eq!(app.panel_cursor, FocusPanel::Results);
}

#[test]
fn vertical_nav_from_results_reaches_editor() {
    let mut app = test_app(&AppConfig::default());
    app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
    assert_eq!(app.panel_cursor, FocusPanel::Results);
    app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
    assert_eq!(app.panel_cursor, FocusPanel::Editor);
}

#[test]
fn semantic_panel_does_not_move_right() {
    let mut app = test_app(&AppConfig::default());
    app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
    assert_eq!(app.panel_cursor, FocusPanel::SemanticSearch);
    app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
    assert_eq!(app.panel_cursor, FocusPanel::SemanticSearch);
}

#[test]
fn wasd_moves_results_focus() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Results);
    let metadata = ResultMetadata {
        source_table: None,
        row_identities: vec![None, None],
        source_columns: vec![None],
        column_types: vec!["text".to_string()],
        null_cells: Vec::new(),
        origin: ResultOrigin::Unknown,
    };
    app.results.set_data(
        vec!["col".to_string()],
        vec![vec!["first".to_string()], vec!["second".to_string()]],
        metadata,
    );
    app.results.set_focus(1, 0, false);
    app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
    assert_eq!(app.results.focus_cell, (0, 0));
    app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
    assert_eq!(app.results.focus_cell, (1, 0));
}

#[test]
fn status_enter_opens_settings_modal() {
    let mut app = test_app(&AppConfig::default());
    app.layer = UiLayer::PanelSelect;
    app.panel_cursor = FocusPanel::Status;
    assert!(app.settings_view.is_none());
    let event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    app.handle_key(event);
    assert!(matches!(app.layer, UiLayer::Settings));
    assert!(app.settings_view.is_some());
}

#[test]
fn status_f_opens_settings_modal() {
    let mut app = test_app(&AppConfig::default());
    app.layer = UiLayer::PanelSelect;
    app.panel_cursor = FocusPanel::Status;
    assert!(app.settings_view.is_none());
    let event = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE);
    app.handle_key(event);
    assert!(matches!(app.layer, UiLayer::Settings));
    assert!(app.settings_view.is_some());
}

#[test]
fn status_click_opens_settings_modal() {
    let mut app = test_app(&AppConfig::default());
    app.layer = UiLayer::PanelSelect;
    app.status_area = Some(Rect::new(0, 0, 10, 1));
    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 1,
        row: 0,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(mouse);
    assert!(matches!(app.layer, UiLayer::Settings));
    assert!(app.settings_view.is_some());
}

#[test]
fn entering_save_row_persists_fast_scroll_step() {
    let mut app = test_app(&AppConfig::default());
    let baseline = app.ui_settings.fast_scroll_step();
    app.open_settings_modal();
    {
        let view = app.settings_view.as_mut().expect("view");
        view.selected = 1; // fast scroll step entry
        view.nudge_current(true);
        let save_index = view.rows().len() - 1;
        view.selected = save_index;
    }
    let event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    app.handle_key(event);
    assert!(app.settings_view.is_none());
    assert!(matches!(app.layer, UiLayer::PanelSelect));
    assert_eq!(app.ui_settings.fast_scroll_step(), baseline + 1);
    let value = app
        .store
        .settings()
        .fast_scroll_step()
        .expect("read setting")
        .expect("stored value");
    assert_eq!(value, baseline + 1);
}

#[test]
fn mouse_scroll_moves_settings_selection() {
    let mut app = test_app(&AppConfig::default());
    app.open_settings_modal();
    let initial = app.settings_view.as_ref().expect("view").selected;
    let scroll = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 0,
        row: 0,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(scroll);
    let selected = app.settings_view.as_ref().expect("view").selected;
    assert!(selected > initial);
}

#[test]
fn mouse_click_selects_settings_row_and_begins_editing() {
    let mut app = test_app(&AppConfig::default());
    app.open_settings_modal();
    let action_index = app.settings_view.as_ref().expect("view").rows().len() - 1;
    app.settings_hitbox = Some(SettingsHitbox {
        popup_area: Rect::new(0, 0, 20, 10),
        body_area: Rect::new(0, 0, 20, 8),
        action_area: Rect::new(0, 8, 20, 1),
        body_offset: 0,
        action_index,
    });
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 1,
        row: 1,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(click);
    let view = app.settings_view.as_ref().expect("view");
    assert_eq!(view.selected, 1);
    assert!(view.editing);
}

#[test]
fn mouse_click_save_row_applies_changes() {
    let mut app = test_app(&AppConfig::default());
    let baseline = app.ui_settings.fast_scroll_step();
    app.open_settings_modal();
    {
        let view = app.settings_view.as_mut().expect("view");
        view.selected = 1;
        view.nudge_current(true);
    }
    let action_index = app.settings_view.as_ref().expect("view").rows().len() - 1;
    app.settings_hitbox = Some(SettingsHitbox {
        popup_area: Rect::new(0, 0, 20, 10),
        body_area: Rect::new(0, 0, 20, 8),
        action_area: Rect::new(0, 8, 20, 1),
        body_offset: 0,
        action_index,
    });
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 1,
        row: 8,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(click);
    assert!(app.settings_view.is_none());
    assert!(matches!(app.layer, UiLayer::PanelSelect));
    assert_eq!(app.ui_settings.fast_scroll_step(), baseline + 1);
    let stored = app
        .store
        .settings()
        .fast_scroll_step()
        .expect("read setting")
        .expect("stored value");
    assert_eq!(stored, baseline + 1);
}

#[test]
fn clicking_outside_settings_closes_modal() {
    let mut app = test_app(&AppConfig::default());
    app.open_settings_modal();
    let action_index = app.settings_view.as_ref().expect("view").rows().len() - 1;
    app.settings_hitbox = Some(SettingsHitbox {
        popup_area: Rect::new(2, 2, 10, 6),
        body_area: Rect::new(2, 2, 10, 4),
        action_area: Rect::new(2, 6, 10, 1),
        body_offset: 0,
        action_index,
    });
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 0,
        row: 0,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(click);
    assert!(app.settings_view.is_none());
    assert!(matches!(app.layer, UiLayer::PanelSelect));
}

#[test]
fn results_char_input_requires_backspace_to_edit() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Results);
    seed_editable_results(&mut app);
    let event = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
    app.handle_key(event);
    assert!(app.results.edit_session().is_none());
}

#[test]
fn wasd_characters_edit_instead_of_navigating_during_edit_session() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Results);
    seed_editable_results(&mut app);
    app.results.set_focus(1, 0, false);

    app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    {
        let session = app.results.edit_session().expect("edit session");
        assert_eq!(session.buffer(), "brav");
    }

    app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
    assert_eq!(app.results.focus_cell, (1, 0));
    {
        let session = app.results.edit_session().expect("edit session");
        assert_eq!(session.buffer(), "bravw");
    }

    app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
    {
        let session = app.results.edit_session().expect("edit session");
        assert_eq!(session.buffer(), "bravwr");
    }
}

#[test]
fn enter_starts_results_edit_session() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Results);
    seed_editable_results(&mut app);
    assert!(app.results.edit_session().is_none());
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.results.edit_session().is_some());
}

#[test]
fn aliased_result_edit_preview_uses_physical_source_column() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Results);
    app.results.set_data(
        vec!["display_alias".into()],
        vec![vec!["alpha".into()]],
        ResultMetadata {
            source_table: Some(relation("public", "demo")),
            row_identities: vec![Some(row_identity("0"))],
            source_columns: vec![Some("physical_name".into())],
            column_types: vec!["text".into()],
            null_cells: Vec::new(),
            origin: ResultOrigin::Unknown,
        },
    );

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let preview = app.editor.text();
    assert!(preview.contains("SET \"physical_name\" ="));
    assert!(!preview.contains("SET \"display_alias\" ="));
}

#[test]
fn changed_results_edit_submits_typed_relation_and_value() {
    let (mut app, mut command_rx) = test_app_with_channels(&AppConfig::default());
    app.focus_window(FocusPanel::Results);
    seed_editable_results(&mut app);
    app.results.set_focus(1, 0, false);

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.results.edit_session().is_some());
    app.handle_key(KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE));

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let command = command_rx.try_recv().expect("update command");
    match command {
        EngineCommand::Crud {
            action:
                CrudAction::UpdateCell {
                    table,
                    column,
                    new_value,
                    row_identity,
                    column_type,
                },
            kind,
            ..
        } => {
            assert_eq!(kind, EngineRequestKind::UpdateCell);
            assert_eq!(table, relation("public", "demo"));
            assert_eq!(column, "col");
            assert_eq!(new_value.as_deref(), Some("bravo!"));
            assert_eq!(
                row_identity,
                RowIdentity {
                    relation_oid: 1,
                    xmin: "1".into(),
                    primary_key: Vec::new(),
                    table_oid: 1,
                    ctid: "1".into()
                }
            );
            assert_eq!(column_type.as_deref(), Some("text"));
        }
        other => panic!("unexpected command: {other:?}"),
    }

    assert!(app.results.edit_session().is_none());
}

#[test]
fn unchanged_null_edit_does_not_queue_an_update() {
    let (mut app, mut command_rx) = test_app_with_channels(&AppConfig::default());
    app.focus_window(FocusPanel::Results);
    app.results.set_data(
        vec!["value".into()],
        vec![vec!["NULL".into()]],
        ResultMetadata {
            source_table: Some(relation("public", "nullable_values")),
            row_identities: vec![Some(row_identity("0"))],
            source_columns: vec![Some("value".into())],
            column_types: vec!["text".into()],
            null_cells: vec![vec![true]],
            origin: ResultOrigin::Unknown,
        },
    );

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app
        .results
        .edit_session()
        .is_some_and(crate::state::results_state::EditSession::is_null));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(command_rx.try_recv().is_err());
    assert!(app.results.edit_session().is_none());
}

#[test]
fn ctrl_n_sets_sql_null_while_literal_null_remains_text() {
    let (mut app, mut command_rx) = test_app_with_channels(&AppConfig::default());
    app.focus_window(FocusPanel::Results);
    app.results.set_data(
        vec!["value".into()],
        vec![vec!["NULL".into()]],
        ResultMetadata {
            source_table: Some(relation("public", "nullable_values")),
            row_identities: vec![Some(row_identity("0"))],
            source_columns: vec![Some("value".into())],
            column_types: vec!["text".into()],
            null_cells: vec![vec![false]],
            origin: ResultOrigin::Unknown,
        },
    );

    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app
        .results
        .edit_session()
        .is_some_and(|edit| !edit.is_null()));
    app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
    assert!(app
        .results
        .edit_session()
        .is_some_and(crate::state::results_state::EditSession::is_null));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let command = command_rx.try_recv().expect("NULL update command");
    let EngineCommand::Crud {
        action: CrudAction::UpdateCell { new_value, .. },
        ..
    } = command
    else {
        panic!("unexpected command: {command:?}");
    };
    assert_eq!(new_value, None);
}

#[test]
fn esc_in_editor_opens_panel_selector() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Editor);
    assert!(matches!(app.layer, super::UiLayer::PanelFocused));
    let event = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.handle_key(event));
    assert_eq!(app.focus, FocusPanel::Editor);
    assert!(matches!(app.layer, super::UiLayer::PanelSelect));
    assert_eq!(app.panel_cursor, FocusPanel::Editor);
}

#[test]
fn esc_in_schema_opens_panel_selector() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Schema);
    assert!(matches!(app.layer, super::UiLayer::PanelFocused));
    let event = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.handle_key(event));
    // Esc now collapses back within the schema panel instead of entering panel select.
    assert!(matches!(app.layer, super::UiLayer::PanelFocused));
    assert_eq!(app.focus, FocusPanel::Schema);
}

#[test]
fn shift_esc_in_window_layer_quits() {
    let mut app = test_app(&AppConfig::default());
    app.enter_window_layer();
    let event = KeyEvent::new(KeyCode::Esc, KeyModifiers::SHIFT);
    assert!(app.handle_key(event));
}

#[test]
fn shift_esc_quits_from_editor() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Editor);
    let event = KeyEvent::new(KeyCode::Esc, KeyModifiers::SHIFT);
    assert!(app.handle_key(event));
}

#[test]
fn shift_tab_moves_focus_backward() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Results);
    let event = KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE);
    assert!(!app.handle_key(event));
    assert_eq!(app.focus, FocusPanel::Editor);
}

#[test]
fn horizontal_move_from_schema_enters_panel_select() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Schema);
    let event = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
    assert!(!app.handle_key(event));
    assert!(matches!(app.layer, super::UiLayer::PanelSelect));
    assert_eq!(app.focus, FocusPanel::Schema);
    assert_eq!(app.panel_cursor, FocusPanel::Editor);
}

#[test]
fn f_opens_table_detail_without_firing_select_top() {
    let (mut app, mut commands) = test_app_with_channels(&AppConfig::default());
    app.focus_window(FocusPanel::Schema);
    let _ = commands.try_recv(); // ignore auto-fetch triggered on focus
    let event = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE);
    assert!(!app.handle_key(event));
    assert!(app.table_detail.is_open());
    assert!(commands.try_recv().is_err());
}

#[test]
fn shift_f_still_runs_select_top() {
    let (mut app, mut commands) = test_app_with_channels(&AppConfig::default());
    app.focus_window(FocusPanel::Schema);
    let event = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::SHIFT);
    assert!(!app.handle_key(event));
    match commands.try_recv() {
        Ok(EngineCommand::Crud { kind, .. }) => {
            assert_eq!(kind, EngineRequestKind::SelectTop);
        }
        other => panic!("expected select top request, got {other:?}"),
    }
}

#[test]
fn esc_closes_table_detail() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Schema);
    app.handle_action(Action::TableDetail);
    assert!(app.table_detail.is_open());
    let event = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!app.handle_key(event));
    assert!(!app.table_detail.is_open());
    assert_eq!(app.focus, FocusPanel::Schema);
    assert!(matches!(app.layer, super::UiLayer::PanelFocused));
}

#[test]
fn table_detail_toggles_on_repeat_activation() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Schema);
    app.handle_action(Action::TableDetail);
    assert!(app.table_detail.is_open());
    app.handle_action(Action::TableDetail);
    assert!(!app.table_detail.is_open());
}

#[test]
fn mouse_scroll_moves_schema_selection() {
    let mut app = test_app(&AppConfig::default());
    app.schema.items = vec![
        SchemaItem::Schema {
            name: "a".into(),
            expanded: true,
            table_count: 1,
        },
        SchemaItem::Table {
            schema: "a".into(),
            name: "a1".into(),
            is_last_in_schema: true,
        },
        SchemaItem::Schema {
            name: "b".into(),
            expanded: true,
            table_count: 1,
        },
        SchemaItem::Table {
            schema: "b".into(),
            name: "b1".into(),
            is_last_in_schema: true,
        },
    ];
    app.schema.selected = 0;
    app.schema_area = Some(Rect::new(0, 0, 10, 10));

    let scroll_down = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 2,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(scroll_down);
    let expected_down = app
        .ui_settings
        .mouse_scroll_lines()
        .min(app.schema.items.len() - 1);
    assert_eq!(app.schema.selected, expected_down);

    let scroll_up = MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 1,
        row: 2,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(scroll_up);
    assert_eq!(app.schema.selected, 0);
}

#[test]
fn mouse_scroll_moves_results_focus() {
    let mut app = test_app(&AppConfig::default());
    app.results_area = Some(Rect::new(0, 0, 20, 10));
    let rows: Vec<Vec<String>> = (0..8).map(|idx| vec![format!("row-{idx}")]).collect();
    let metadata = ResultMetadata {
        source_table: None,
        row_identities: vec![None; 8],
        source_columns: vec![None],
        column_types: vec!["text".to_string()],
        null_cells: Vec::new(),
        origin: ResultOrigin::Unknown,
    };
    app.results
        .set_data(vec!["col".to_string()], rows, metadata);

    let scroll_down = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 3,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(scroll_down);
    let expected = app
        .ui_settings
        .mouse_scroll_lines()
        .min(app.results.row_count() - 1);
    assert_eq!(app.results.focus_cell.0, expected);

    let scroll_up = MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 1,
        row: 3,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(scroll_up);
    assert_eq!(app.results.focus_cell.0, 0);
}

#[test]
fn clicking_results_respects_scroll_offset() {
    let mut app = test_app(&AppConfig::default());
    app.results_area = Some(Rect::new(0, 0, 20, 10));
    let rows: Vec<Vec<String>> = (0..200).map(|idx| vec![format!("row-{idx}")]).collect();
    let metadata = ResultMetadata {
        source_table: None,
        row_identities: vec![None; 200],
        source_columns: vec![None],
        column_types: vec!["text".to_string()],
        null_cells: Vec::new(),
        origin: ResultOrigin::Unknown,
    };
    app.results
        .set_data(vec!["col".to_string()], rows, metadata);
    app.results.set_scroll_row(50);

    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 2,
        row: 4, // first visible data row after headers in the area
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(mouse);
    assert_eq!(app.results.focus_cell.0, 52);
}

#[test]
fn mouse_scroll_updates_results_scrollbar_state() {
    let mut app = test_app(&AppConfig::default());
    app.results_area = Some(Rect::new(0, 0, 20, 6));
    let rows: Vec<Vec<String>> = (0..60).map(|idx| vec![format!("row-{idx}")]).collect();
    let metadata = ResultMetadata {
        source_table: None,
        row_identities: vec![None; 60],
        source_columns: vec![None],
        column_types: vec!["text".to_string()],
        null_cells: Vec::new(),
        origin: ResultOrigin::Unknown,
    };
    app.results
        .set_data(vec!["col".to_string()], rows, metadata);
    app.focus_window(FocusPanel::Results);

    let header_height = u16::from(!app.results.headers.is_empty());
    let body_height = app
        .results_area
        .map_or(0, |rect| rect.height.saturating_sub(header_height));
    let visible_rows = usize::from(body_height.max(1));

    for _ in 0..3 {
        let scroll_down = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 1,
            row: 3,
            modifiers: KeyModifiers::empty(),
        };
        app.handle_mouse(scroll_down);
        app.results.ensure_focus_visible(visible_rows);
        app.results.sync_scrollbar_thumb(visible_rows);
    }

    assert_eq!(app.results.scroll_row(), 5);
    assert_eq!(
        app.results.vertical_scrollbar_state().get_position(),
        app.results.scroll_row()
    );
}

#[test]
fn scrollbar_drag_tracks_grabbed_offset() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Results);
    app.results_area = Some(Rect::new(0, 0, 30, 20));
    let rows: Vec<Vec<String>> = (0..120).map(|idx| vec![format!("row-{idx}")]).collect();
    let metadata = ResultMetadata {
        source_table: None,
        row_identities: vec![None; 120],
        source_columns: vec![None],
        column_types: vec!["text".to_string()],
        null_cells: Vec::new(),
        origin: ResultOrigin::Unknown,
    };
    app.results
        .set_data(vec!["col".to_string()], rows, metadata);
    let initial_scroll = 40;
    app.results.set_scroll_row(initial_scroll);

    let context = ScrollbarContext {
        track: Rect::new(25, 2, 1, 10),
        viewport_items: 5,
        max_scroll: app.results.row_count().saturating_sub(5),
    };
    app.results_vertical_scrollbar = Some(context.clone());

    let mouse_down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 25,
        row: 2,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(mouse_down);
    let delta = app.results.scroll_row().abs_diff(initial_scroll);
    assert!(
        delta <= 2,
        "initial grab should stay near the current scroll (Δ={delta})"
    );
    assert!(app.scroll_drag.is_some());

    let drag_bottom = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 25,
        row: 11,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(drag_bottom);
    let expected_top = app.results.row_count().saturating_sub(5);
    assert_eq!(app.results.scroll_row(), expected_top);

    let drag_top = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 25,
        row: 2,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(drag_top);
    assert_eq!(app.results.scroll_row(), 0);
}

#[test]
fn scrollbar_drag_reaches_end_when_grabbed_low() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Results);
    app.results_area = Some(Rect::new(0, 0, 30, 20));
    let rows: Vec<Vec<String>> = (0..150).map(|idx| vec![format!("row-{idx}")]).collect();
    let metadata = ResultMetadata {
        source_table: None,
        row_identities: vec![None; 150],
        source_columns: vec![None],
        column_types: vec!["text".to_string()],
        null_cells: Vec::new(),
        origin: ResultOrigin::Unknown,
    };
    app.results
        .set_data(vec!["col".to_string()], rows, metadata);

    let viewport = 5;
    let context = ScrollbarContext {
        track: Rect::new(25, 2, 1, 10),
        viewport_items: viewport,
        max_scroll: app.results.row_count().saturating_sub(viewport),
    };
    let expected_top = context.max_scroll;
    app.results_vertical_scrollbar = Some(context.clone());

    let mouse_down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: context.track.x,
        row: context.track.y + context.track.height - 2,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(mouse_down);
    assert!(app.scroll_drag.is_some());

    let drag_bottom = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: context.track.x,
        row: context.track.y + context.track.height - 1,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(drag_bottom);
    assert_eq!(app.results.scroll_row(), expected_top);
}

#[test]
fn horizontal_scrollbar_drag_reaches_end_when_grabbed_right() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Results);
    app.results_area = Some(Rect::new(0, 0, 40, 15));
    let column_count = 10;
    let headers: Vec<String> = (0..column_count).map(|idx| format!("c{idx}")).collect();
    let row: Vec<String> = (0..column_count).map(|idx| format!("v{idx}")).collect();
    let rows = vec![row];
    let metadata = ResultMetadata {
        source_table: None,
        row_identities: vec![None; rows.len()],
        source_columns: vec![None; column_count],
        column_types: vec!["text".to_string(); column_count],
        null_cells: Vec::new(),
        origin: ResultOrigin::Unknown,
    };
    app.results.set_data(headers, rows, metadata);

    let viewport_cols = 3;
    let track = Rect::new(5, 12, 12, 1);
    let max_scroll = column_count.saturating_sub(viewport_cols);
    let context = ScrollbarContext {
        track,
        viewport_items: viewport_cols,
        max_scroll,
    };
    app.results_horizontal_scrollbar = Some(context.clone());

    let mouse_down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: context.track.x + context.track.width - 2,
        row: context.track.y,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(mouse_down);
    assert!(app.scroll_drag.is_some());

    let drag_right = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: context.track.x + context.track.width - 1,
        row: context.track.y,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(drag_right);
    assert_eq!(app.results.scroll_col(), max_scroll);
}

#[test]
fn clicking_results_updates_column_when_hitbox_present() {
    let mut app = test_app(&AppConfig::default());
    app.results_area = Some(Rect::new(0, 0, 20, 10));
    let metadata = ResultMetadata {
        source_table: None,
        row_identities: vec![None, None],
        source_columns: vec![None; 3],
        column_types: vec!["int4".to_string(), "text".to_string(), "text".to_string()],
        null_cells: Vec::new(),
        origin: ResultOrigin::Unknown,
    };
    app.results.set_data(
        vec!["id".to_string(), "name".to_string(), "email".to_string()],
        vec![
            vec![
                "1".to_string(),
                "Alice".to_string(),
                "a@example.com".to_string(),
            ],
            vec![
                "2".to_string(),
                "Bob".to_string(),
                "b@example.com".to_string(),
            ],
        ],
        metadata,
    );
    app.results_hitbox = Some(ResultsHitbox {
        inner_area: Rect::new(1, 1, 18, 8),
        header_height: 1,
        start_row: 0,
        separator_width: 1,
        columns: vec![
            ResultsHitColumn {
                index: 0,
                start: 0,
                width: 5,
            },
            ResultsHitColumn {
                index: 1,
                start: 6,
                width: 5,
            },
            ResultsHitColumn {
                index: 2,
                start: 12,
                width: 5,
            },
        ],
    });

    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 1 + 7, // in the second column
        row: 3,        // second visible data row
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(mouse);
    assert_eq!(app.results.focus_cell, (1, 1));
}

#[test]
fn refresh_after_select_top_reissues_select_top() {
    let (mut app, mut command_rx) = test_app_with_channels(&AppConfig::default());
    let metadata = ResultMetadata {
        source_table: Some(relation("public", "demo")),
        row_identities: vec![Some(row_identity("0"))],
        source_columns: vec![Some("id".into())],
        column_types: vec!["int4".into()],
        null_cells: Vec::new(),
        origin: ResultOrigin::SelectTop {
            table: relation("public", "demo"),
            limit: app.ui_settings.select_top_limit(),
        },
    };
    app.results
        .set_data(vec!["id".into()], vec![vec!["1".into()]], metadata);
    let context = Some(MutationContext {
        table: relation("public", "demo"),
        row_identity: Some(row_identity("0")),
        row: Some(0),
        column: Some(0),
    });
    app.refresh_results_after_mutation(context);
    let command = command_rx.try_recv().expect("command");
    match command {
        EngineCommand::Crud {
            action: CrudAction::SelectTop { table, limit, .. },
            ..
        } => {
            assert_eq!(table, relation("public", "demo"));
            assert_eq!(limit, app.ui_settings.select_top_limit());
        }
        other => panic!("unexpected command: {other:?}"),
    }
    assert!(app.pending_focus.is_some());
}

#[test]
fn refresh_after_simple_run_sql_reruns_query() {
    let (mut app, mut command_rx) = test_app_with_channels(&AppConfig::default());
    let metadata = ResultMetadata {
        source_table: Some(relation("public", "demo")),
        row_identities: vec![Some(row_identity("0"))],
        source_columns: vec![Some("id".into())],
        column_types: vec!["int4".into()],
        null_cells: Vec::new(),
        origin: ResultOrigin::RunSql {
            sql: "select * from public.demo".into(),
            refresh: RunSqlRefresh::SimpleSingleTable {
                table: relation("public", "demo"),
            },
        },
    };
    app.results
        .set_data(vec!["id".into()], vec![vec!["1".into()]], metadata);
    let context = Some(MutationContext {
        table: relation("public", "demo"),
        row_identity: Some(row_identity("0")),
        row: Some(0),
        column: Some(0),
    });
    app.refresh_results_after_mutation(context);
    let command = command_rx.try_recv().expect("command");
    match command {
        EngineCommand::RunSql { sql, kind, .. } => {
            assert_eq!(kind, EngineRequestKind::RunSql);
            assert_eq!(sql, "select * from public.demo");
        }
        other => panic!("unexpected command: {other:?}"),
    }
    assert!(app.pending_focus.is_some());
}

#[test]
fn refresh_after_complex_query_requests_row_refresh() {
    let (mut app, mut command_rx) = test_app_with_channels(&AppConfig::default());
    let metadata = ResultMetadata {
        source_table: Some(relation("public", "demo")),
        row_identities: vec![Some(row_identity("0"))],
        source_columns: vec![Some("id".into())],
        column_types: vec!["int4".into()],
        null_cells: Vec::new(),
        origin: ResultOrigin::RunSql {
            sql: "select id from public.demo where id > 0".into(),
            refresh: RunSqlRefresh::ComplexSingleTable {
                table: relation("public", "demo"),
            },
        },
    };
    app.results
        .set_data(vec!["id".into()], vec![vec!["1".into()]], metadata);
    let context = Some(MutationContext {
        table: relation("public", "demo"),
        row_identity: Some(row_identity("0")),
        row: Some(0),
        column: Some(0),
    });
    app.refresh_results_after_mutation(context);
    let command = command_rx.try_recv().expect("command");
    match command {
        EngineCommand::Crud {
            action:
                CrudAction::RefreshRow {
                    table,
                    row_identity: identity,
                },
            kind,
            ..
        } => {
            assert_eq!(kind, EngineRequestKind::RowRefresh);
            assert_eq!(table, relation("public", "demo"));
            assert_eq!(identity, row_identity("0"));
        }
        other => panic!("unexpected command: {other:?}"),
    }
    assert!(app.pending_focus.is_none());
}

#[test]
fn mutation_refresh_follow_up_response_is_processed() {
    let (mut app, mut command_rx, response_tx) =
        test_app_with_channels_and_responses(&AppConfig::default());
    let metadata = ResultMetadata {
        source_table: Some(relation("public", "demo")),
        row_identities: vec![Some(row_identity("0"))],
        source_columns: vec![Some("id".into())],
        column_types: vec!["int4".into()],
        null_cells: Vec::new(),
        origin: ResultOrigin::SelectTop {
            table: relation("public", "demo"),
            limit: app.ui_settings.select_top_limit(),
        },
    };
    app.results
        .set_data(vec!["id".into()], vec![vec!["1".into()]], metadata);

    let request_id = app.next_request_id();
    app.start_request(
        request_id,
        EngineRequestKind::UpdateCell,
        "public.demo.id".into(),
        Some(PostResultsAction::ApplyCellEdit {
            row: 0,
            column: 0,
            value: Some("2".into()),
        }),
        false,
    );

    let update_result = QueryResult {
        columns: Vec::new(),
        rows: Vec::new(),
        metadata: ResultMetadata {
            source_table: Some(relation("public", "demo")),
            row_identities: vec![Some(row_identity("5"))],
            source_columns: Vec::new(),
            column_types: Vec::new(),
            null_cells: Vec::new(),
            origin: ResultOrigin::Unknown,
        },
    };
    response_tx
        .send(EngineResponse::Success {
            request_id,
            kind: EngineRequestKind::UpdateCell,
            result: Box::new(update_result),
        })
        .unwrap();
    app.poll_engine();

    let refresh_request_id = match command_rx.try_recv().expect("refresh command") {
        EngineCommand::Crud {
            request_id,
            action: CrudAction::SelectTop { .. },
            kind: EngineRequestKind::SelectTop,
        } => request_id,
        other => panic!("unexpected command: {other:?}"),
    };

    let refresh_result = QueryResult {
        columns: vec!["id".into()],
        rows: vec![vec!["99".into()]],
        metadata: ResultMetadata {
            source_table: Some(relation("public", "demo")),
            row_identities: vec![Some(row_identity("6"))],
            source_columns: vec![Some("id".into())],
            column_types: vec!["int4".into()],
            null_cells: Vec::new(),
            origin: ResultOrigin::SelectTop {
                table: relation("public", "demo"),
                limit: app.ui_settings.select_top_limit(),
            },
        },
    };
    response_tx
        .send(EngineResponse::Success {
            request_id: refresh_request_id,
            kind: EngineRequestKind::SelectTop,
            result: Box::new(refresh_result),
        })
        .unwrap();
    app.poll_engine();

    assert_eq!(app.results.rows, vec![vec![String::from("99")]]);
}

#[test]
fn column_rename_success_dispatches_catalog_refresh() {
    assert_success_dispatches_catalog_refresh(
        "ALTER TABLE public.demo RENAME COLUMN old_name TO new_name;",
    );
}

#[test]
fn select_into_success_dispatches_catalog_refresh() {
    assert_success_dispatches_catalog_refresh("SELECT 1 AS id INTO public.new_table;");
}

fn assert_success_dispatches_catalog_refresh(sql: &str) {
    let (mut app, mut command_rx, response_tx) =
        test_app_with_channels_and_responses(&AppConfig::default());
    app.editor.set_text(sql);
    app.run_query();
    let EngineCommand::RunSql { request_id, .. } = command_rx.try_recv().unwrap() else {
        panic!("expected SQL request");
    };
    response_tx
        .send(EngineResponse::Success {
            request_id,
            kind: EngineRequestKind::RunSql,
            result: Box::new(QueryResult::empty()),
        })
        .unwrap();
    app.poll_engine();
    app.on_tick();
    assert!(matches!(
        command_rx.try_recv(),
        Ok(EngineCommand::RefreshCatalog { .. })
    ));
}

#[test]
fn ddl_success_triggers_catalog_refresh() {
    let (mut app, mut command_rx, response_tx) =
        test_app_with_channels_and_responses(&AppConfig::default());
    app.editor.set_text("DROP TABLE IF EXISTS public.demo;");
    app.run_query();
    let ddl_request_id = match command_rx.try_recv().expect("send run-sql command") {
        EngineCommand::RunSql {
            request_id,
            kind,
            sql,
        } => {
            assert_eq!(kind, EngineRequestKind::RunSql);
            assert!(sql.to_ascii_lowercase().contains("drop table"));
            request_id
        }
        other => panic!("unexpected command: {other:?}"),
    };

    response_tx
        .send(EngineResponse::Success {
            request_id: ddl_request_id,
            kind: EngineRequestKind::RunSql,
            result: Box::new(QueryResult::empty()),
        })
        .unwrap();
    app.poll_engine();
    app.on_tick();

    let refresh_request_id = match command_rx.try_recv().expect("catalog refresh command") {
        EngineCommand::RefreshCatalog { request_id } => request_id,
        other => panic!("unexpected command: {other:?}"),
    };

    let mut refreshed_catalog = sample_catalog();
    refreshed_catalog.tables.push(TableMeta {
        id: TableId("public.widgets".into()),
        name: "widgets".into(),
        schema: "public".into(),
        relation_kind: RelationKind::Table,
    });
    refreshed_catalog.columns.push(ColumnMeta {
        schema: "public".into(),
        table: "widgets".into(),
        name: "id".into(),
        data_type: "integer".into(),
        ordinal_position: 1,
        nullability: Nullability::NotNull,
        character_maximum_length: None,
        numeric_precision: None,
        numeric_scale: None,
        is_primary_key: true,
        default_kind: ColumnDefault::None,
        is_foreign_key: false,
    });
    response_tx
        .send(EngineResponse::CatalogRefreshed {
            request_id: refresh_request_id,
            snapshot: refreshed_catalog,
        })
        .unwrap();
    app.poll_engine();

    let tables: Vec<_> = app
        .schema
        .items
        .iter()
        .filter_map(|item| match item {
            SchemaItem::Table { name, .. } => Some(name.as_str()),
            SchemaItem::Schema { .. } => None,
        })
        .collect();
    assert!(tables.contains(&"widgets"));
    assert!(
        app.table_detail
            .open_for(&relation("public", "widgets"))
            .is_some(),
        "table detail updated after refresh"
    );
}

#[test]
fn mouse_scroll_adjusts_editor_scroll_without_moving_cursor() {
    let mut app = test_app(&AppConfig::default());
    app.editor_area = Some(Rect::new(0, 0, 40, 10));
    app.editor_text_area = Some(Rect::new(1, 1, 38, 8));
    app.editor.lines = (0..12).map(|idx| format!("line-{idx}")).collect();
    app.focus_window(FocusPanel::Editor);
    app.editor.cursor_row = 0;
    app.editor.cursor_col = 0;
    let viewport = app.editor_text_area.expect("viewport");
    let view_width = usize::from(viewport.width.max(1));
    let view_height = usize::from(viewport.height.max(1));
    let max_scroll = app.editor_row_count(view_width).saturating_sub(view_height);

    let scroll_down = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 2,
        row: 2,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(scroll_down);
    let expected = app.ui_settings.mouse_scroll_lines().min(max_scroll);
    assert_eq!(app.editor.scroll_row, expected);
    assert_eq!(app.editor.cursor_row, 0);

    let scroll_up = MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 2,
        row: 2,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(scroll_up);
    assert_eq!(app.editor.scroll_row, 0);
    assert_eq!(app.editor.cursor_row, 0);
}

#[test]
fn clamp_editor_scroll_keeps_cursor_visible_after_insert() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Editor);
    app.editor_text_area = Some(Rect::new(0, 0, 20, 3));
    app.editor.lines = vec!["start".to_string()];
    app.editor.cursor_row = 0;
    app.editor.cursor_col = app.editor.lines[0].len();

    app.editor.insert_text("\nline1\nline2\nline3\nline4");
    app.clamp_editor_scroll_to_cursor();

    assert_eq!(app.editor.cursor_row, app.editor.lines.len() - 1);
    let expected = (app.editor.cursor_row + 1).saturating_sub(3);
    assert_eq!(app.editor.scroll_row, expected);
}

#[test]
fn shift_arrow_extends_selection_in_editor() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Editor);
    app.editor.lines = vec!["abcd".to_string()];
    app.editor.set_cursor(0, 3);
    let event = KeyEvent::new(KeyCode::Left, KeyModifiers::SHIFT);
    app.handle_key(event);
    let range = app.editor.selection_range().expect("selection");
    assert_eq!(range.start, (0, 2));
    assert_eq!(range.end, (0, 3));
}

#[test]
fn plain_arrow_clears_selection() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Editor);
    app.editor.lines = vec!["abcd".to_string()];
    app.editor.set_cursor(0, 2);
    app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::SHIFT));
    assert!(app.editor.selection_range().is_some());
    app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    assert!(app.editor.selection_range().is_none());
    let (row, col) = app.editor.cursor_position();
    assert_eq!((row, col), (0, 0));
}

#[test]
fn mouse_click_clears_editor_selection() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Editor);
    app.editor_area = Some(Rect::new(0, 0, 10, 5));
    let text_area = Rect::new(1, 1, 20, 3);
    app.editor_text_area = Some(text_area);
    app.editor.lines = vec!["abcd".to_string()];
    app.editor.set_selection_for_test((0, 0), (0, 2));
    let gutter = u16::try_from(app.editor_gutter_width()).unwrap_or(u16::MAX);
    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: text_area.x + gutter + 2,
        row: text_area.y,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(mouse);
    assert!(app.editor.selection_range().is_none());
    let (row, col) = app.editor.cursor_position();
    assert_eq!((row, col), (0, 2));
}

#[test]
fn shift_click_extends_editor_selection() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Editor);
    app.editor_area = Some(Rect::new(0, 0, 10, 5));
    let text_area = Rect::new(1, 1, 20, 3);
    app.editor_text_area = Some(text_area);
    app.editor.lines = vec!["abcdef".to_string()];
    app.editor.set_cursor(0, 1);
    let gutter = u16::try_from(app.editor_gutter_width()).unwrap_or(u16::MAX);
    app.clamp_editor_scroll_to_cursor();
    app.editor.clear_selection();
    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: text_area.x + gutter + 4,
        row: text_area.y,
        modifiers: KeyModifiers::SHIFT,
    };
    app.handle_mouse(mouse);
    let range = app.editor.selection_range().expect("selection");
    assert_eq!(range.start, (0, 1));
    assert_eq!(range.end, (0, 4));
}

#[test]
fn copy_payload_prefers_selection_text() {
    let mut app = test_app(&AppConfig::default());
    app.editor.lines = vec!["abcdef".to_string()];
    app.editor.set_selection_for_test((0, 1), (0, 4));
    let (text, message) = app.editor_copy_payload().expect("selection payload");
    assert_eq!(text, "bcd");
    assert_eq!(message, "Copied selection to clipboard");
    app.editor.clear_selection();
    let (full, message) = app.editor_copy_payload().expect("buffer payload");
    assert_eq!(full, "abcdef");
    assert_eq!(message, "Copied editor text to clipboard");
}

#[test]
fn mouse_drag_selects_text() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::Editor);
    app.editor_area = Some(Rect::new(0, 0, 20, 6));
    app.editor_text_area = Some(Rect::new(1, 1, 18, 4));
    app.editor.lines = vec!["abcdef".to_string()];
    let text_area = app.editor_text_area.unwrap();
    let gutter = u16::try_from(app.editor_gutter_width()).unwrap_or(u16::MAX);

    let down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: text_area.x + gutter + 1,
        row: text_area.y,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(down);

    let drag = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: text_area.x + gutter + 4,
        row: text_area.y,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(drag);

    let range = app.editor.selection_range().expect("drag selection");
    assert_eq!(range.start, (0, 1));
    assert_eq!(range.end, (0, 4));

    let up = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: text_area.x + gutter + 4,
        row: text_area.y,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(up);
    assert!(app.editor_mouse_drag_anchor.is_none());
}

#[test]
fn mouse_drag_selects_semantic_text() {
    let mut app = test_app(&AppConfig::default());
    app.focus_window(FocusPanel::SemanticSearch);
    app.semantic_area = Some(Rect::new(0, 0, 20, 5));
    app.semantic_text_area = Some(Rect::new(0, 0, 20, 5));
    app.semantic.set_cursor(0, 0);
    app.semantic.insert_text("abcde");

    let mouse_down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 0,
        row: 0,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(mouse_down);

    let drag = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 3,
        row: 0,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(drag);

    assert_eq!(app.semantic.selected_text(), Some("abc".to_string()));

    let mouse_up = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 3,
        row: 0,
        modifiers: KeyModifiers::empty(),
    };
    app.handle_mouse(mouse_up);
    assert!(app.semantic_mouse_drag_anchor.is_none());
}
