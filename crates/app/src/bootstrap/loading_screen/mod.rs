mod state;
mod view;

use std::{
    future::Future,
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread,
    time::Duration,
};

use anyhow::{Context, Result};
use poqi_ui::TerminalSession;
use state::LoadingApp;

/// Configuration for the loading overlay.
#[derive(Clone, Debug)]
pub struct LoadingScreenConfig {
    pub title: String,
    pub initial_detail: String,
    pub success_detail: Option<String>,
    pub failure_detail: Option<String>,
    pub linger: Duration,
}

impl LoadingScreenConfig {
    #[must_use]
    pub fn new(title: impl Into<String>, initial_detail: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            initial_detail: initial_detail.into(),
            success_detail: None,
            failure_detail: None,
            linger: Duration::from_millis(200),
        }
    }

    #[must_use]
    pub fn with_success_detail(mut self, detail: impl Into<String>) -> Self {
        self.success_detail = Some(detail.into());
        self
    }
}

#[derive(Clone)]
pub struct LoadingHandle {
    detail_tx: Sender<String>,
}

impl LoadingHandle {
    fn new(detail_tx: Sender<String>) -> Self {
        Self { detail_tx }
    }

    pub fn set_detail(&self, detail: impl Into<String>) {
        let _ = self.detail_tx.send(detail.into());
    }
}

enum CompletionSignal {
    Success { detail: Option<String> },
    Failure { detail: String },
}

struct ViewConfig {
    title: String,
    initial_detail: String,
    linger: Duration,
}

/// Run a Ratatui loading screen while the provided future resolves.
pub async fn run_loading_screen<F, Fut, T>(
    config: LoadingScreenConfig,
    tick_rate: Duration,
    task: F,
) -> Result<T>
where
    F: FnOnce(LoadingHandle) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let (detail_tx, detail_rx) = mpsc::channel::<String>();
    let (completion_tx, completion_rx) = mpsc::channel::<CompletionSignal>();

    let handle = LoadingHandle::new(detail_tx);

    let view_config = ViewConfig {
        title: config.title.clone(),
        initial_detail: config.initial_detail.clone(),
        linger: config.linger,
    };

    let spinner_thread = thread::Builder::new()
        .name("poqi-loading-screen".to_string())
        .spawn(move || spinner_loop(view_config, tick_rate, &detail_rx, &completion_rx))
        .context("failed to start loading screen")?;

    let result = task(handle.clone()).await;

    let completion_signal = match &result {
        Ok(_) => CompletionSignal::Success {
            detail: config.success_detail.clone(),
        },
        Err(err) => CompletionSignal::Failure {
            detail: config
                .failure_detail
                .clone()
                .unwrap_or_else(|| err.to_string()),
        },
    };

    let _ = completion_tx.send(completion_signal);
    drop(handle);

    let thread_result = spinner_thread
        .join()
        .map_err(|_| anyhow::anyhow!("loading screen thread panicked"))?;
    thread_result?;

    result
}

fn spinner_loop(
    config: ViewConfig,
    tick_rate: Duration,
    detail_rx: &Receiver<String>,
    completion_rx: &Receiver<CompletionSignal>,
) -> Result<()> {
    let mut terminal = TerminalSession::start()?;
    let mut app = LoadingApp::new(config.title, config.initial_detail);

    let run_result = (|| -> Result<()> {
        loop {
            drain_detail_updates(&mut app, detail_rx);

            match completion_rx.try_recv() {
                Ok(CompletionSignal::Success { detail }) => {
                    app.mark_success(detail);
                    terminal.terminal_mut().draw(|f| app.draw(f))?;
                    thread::sleep(config.linger);
                    break Ok(());
                }
                Ok(CompletionSignal::Failure { detail }) => {
                    app.mark_failure(detail);
                    terminal.terminal_mut().draw(|f| app.draw(f))?;
                    thread::sleep(config.linger);
                    break Ok(());
                }
                Err(TryRecvError::Disconnected) => break Ok(()),
                Err(TryRecvError::Empty) => {
                    terminal.terminal_mut().draw(|f| app.draw(f))?;
                    app.advance_spinner();
                    thread::sleep(tick_rate);
                }
            }
        }
    })();

    let shutdown_result = terminal.restore();

    run_result?;
    shutdown_result
}

fn drain_detail_updates(app: &mut LoadingApp, detail_rx: &Receiver<String>) {
    while let Ok(detail) = detail_rx.try_recv() {
        if !detail.is_empty() {
            app.update_detail(detail);
        }
    }
}
