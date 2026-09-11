use crossterm::event::{self, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::TestBackend, Terminal};
use std::{
    collections::{HashMap, VecDeque},
    fs,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
use tokio::sync::mpsc;

use crate::{
    app::{App, SemanticRuntime},
    event_loop::{run_event_loop, EventSource},
    terminal_capabilities::TerminalCapabilities,
    UiRuntimeSettings,
};
use anyhow::Result;
use poqi_catalog::CatalogSnapshot;
use poqi_config::AppConfig;
use poqi_search_semantic::SearchOptions;

fn next_store_path() -> std::path::PathBuf {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let mut path = std::env::temp_dir();
    path.push(format!(
        "poqi-ui-event-test-store-{}.sqlite",
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

struct StubEvents {
    events: VecDeque<event::Event>,
}

impl StubEvents {
    fn new(events: Vec<event::Event>) -> Self {
        Self {
            events: VecDeque::from(events),
        }
    }
}

impl EventSource for StubEvents {
    fn poll(&mut self, _timeout: Duration) -> Result<Option<event::Event>> {
        Ok(self.events.pop_front())
    }
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

fn test_app() -> App {
    let config = AppConfig::default();
    let (command_tx, _command_rx) = mpsc::unbounded_channel();
    let (_response_tx, response_rx) = mpsc::unbounded_channel();
    let mut schemas = HashMap::new();
    schemas.insert("public".to_string(), vec!["demo".to_string()]);
    let catalog = CatalogSnapshot::default();
    let ui_settings = UiRuntimeSettings::default();
    App::new_with_capabilities(
        &config,
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

#[test]
fn event_loop_exits_on_quit_binding() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("terminal");
    let mut app = test_app();
    let quit_event = event::Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::SHIFT));
    let mut events = StubEvents::new(vec![quit_event]);
    run_event_loop(
        &mut terminal,
        &mut app,
        &mut events,
        Duration::from_millis(16),
        Some(Duration::from_mins(1)),
    )
    .expect("run loop");
}
