#![warn(clippy::all, clippy::pedantic)]

mod app;
mod constants;
mod download_progress;
mod engine_worker;
mod event_loop;
mod geometry;
mod input;
mod keymap;
mod runtime;
mod semantic_bootstrap;
mod semantic_runtime;
mod semantic_worker;
mod state;
mod terminal_capabilities;
mod terminal_utils;
mod theme;
mod tick_timer;
mod view;

pub use constants::{
    UiRuntimeSettings, UiRuntimeSettingsParts, DEFAULT_FAST_SCROLL_STEP, DEFAULT_MAIN_TICK_RATE_MS,
    DEFAULT_MENU_TICK_RATE_MS, DEFAULT_MIN_COLUMN_WIDTH, DEFAULT_MOUSE_SCROLL_LINES,
    DEFAULT_SELECT_TOP_LIMIT, DEFAULT_STATUS_AUTO_CLEAR_SECS, DEFAULT_STATUS_EXPIRE_SECS,
};
pub use geometry::point_in_rect;
pub use semantic_bootstrap::SemanticBootstrapCoordinator;
pub use terminal_utils::{setup_terminal, shutdown_terminal, TerminalSession};
pub use tick_timer::TickTimer;

/// Reason for exiting the UI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    /// Normal quit
    Quit,
    /// User wants to return to profile selector
    BackToProfiles,
}

use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc};

use anyhow::{anyhow, Result};
use app::App;
use engine_worker::spawn_engine_worker;
use event_loop::{run_event_loop, CrosstermEventSource};
use poqi_catalog::CatalogSnapshot;
use poqi_config::AppConfig;
use poqi_engine::Engine;
use poqi_store::Store;
use semantic_runtime::{init_semantic_runtime, SemanticBootstrap};
use tokio::{sync::mpsc, task::JoinHandle};

/// Run the TUI using the supplied configuration and capability detection.
///
/// # Errors
/// Returns an error if the terminal cannot be configured or if the engine worker reports a failure.
#[allow(clippy::implicit_hasher)]
pub async fn run_app(
    config: &AppConfig,
    ui_settings: UiRuntimeSettings,
    store: Store,
    engine: Arc<dyn Engine>,
    schemas: HashMap<String, Vec<String>>,
    catalog_snapshot: CatalogSnapshot,
    semantic_coordinator: &SemanticBootstrapCoordinator,
) -> Result<ExitReason> {
    let mut terminal = terminal_utils::TerminalSession::start()?;

    let (mut app, mut events, worker_handle, semantic_handles) = bootstrap_app(
        config,
        ui_settings,
        store,
        engine,
        schemas,
        catalog_snapshot,
        semantic_coordinator,
    );

    let initial_draw = terminal
        .terminal_mut()
        .draw(|frame| app.draw(frame))
        .map(|_| ())
        .map_err(anyhow::Error::from);

    let run_result = initial_draw.and_then(|()| {
        run_event_loop(
            terminal.terminal_mut(),
            &mut app,
            &mut events,
            ui_settings.main_tick_rate(),
            None,
        )
    });

    let exit_reason = app.exit_reason.unwrap_or(ExitReason::Quit);
    app.signal_shutdown();
    drop(app);

    let terminal_result = terminal.restore();
    let worker_result = wait_for_worker(worker_handle).await;
    let semantic_worker_result = wait_for_semantic_workers(semantic_handles).await;

    terminal_result?;
    run_result?;
    worker_result?;
    semantic_worker_result?;
    Ok(exit_reason)
}

async fn wait_for_worker(handle: JoinHandle<()>) -> Result<()> {
    match handle.await {
        Ok(()) => Ok(()),
        Err(err) if err.is_cancelled() => Ok(()),
        Err(err) => Err(anyhow!(err)),
    }
}

async fn wait_for_semantic_workers(handles: Arc<Mutex<Vec<JoinHandle<()>>>>) -> Result<()> {
    let workers = {
        let mut guard = handles.lock();
        guard.drain(..).collect::<Vec<_>>()
    };
    for handle in &workers {
        handle.abort();
    }
    for handle in workers {
        wait_for_worker(handle).await?;
    }
    Ok(())
}

type BootstrapAppBundle = (
    App,
    CrosstermEventSource,
    JoinHandle<()>,
    Arc<Mutex<Vec<JoinHandle<()>>>>,
);

fn bootstrap_app(
    config: &AppConfig,
    ui_settings: UiRuntimeSettings,
    store: Store,
    engine: Arc<dyn Engine>,
    schemas: HashMap<String, Vec<String>>,
    catalog_snapshot: CatalogSnapshot,
    semantic_coordinator: &SemanticBootstrapCoordinator,
) -> BootstrapAppBundle {
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let (response_tx, response_rx) = mpsc::unbounded_channel();
    let worker_handle = spawn_engine_worker(engine, command_rx, response_tx);
    let semantic_bootstrap: SemanticBootstrap = init_semantic_runtime(config, semantic_coordinator);
    let semantic_handles = semantic_bootstrap.handles.clone();
    let mut app = App::new(
        config,
        ui_settings,
        store,
        schemas,
        catalog_snapshot,
        command_tx,
        response_rx,
        semantic_bootstrap.runtime,
    );
    app.install_semantic_lifecycle(semantic_coordinator.clone(), Arc::clone(&semantic_handles));
    (app, CrosstermEventSource, worker_handle, semantic_handles)
}

#[cfg(test)]
mod tests;
