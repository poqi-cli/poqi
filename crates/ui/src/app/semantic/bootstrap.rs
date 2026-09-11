use crate::{app::App, semantic_bootstrap::SemanticBootstrapEvent};
use tokio::sync::mpsc::error::TryRecvError;

impl App {
    pub(crate) fn poll_semantic_bootstrap(&mut self) {
        loop {
            let Some(events) = self.semantic_events.as_mut() else {
                return;
            };
            match events.try_recv() {
                Ok(event) => match event {
                    SemanticBootstrapEvent::Status(message) => {
                        self.semantic.set_loading(message);
                    }
                    SemanticBootstrapEvent::Ready { tx, rx, backend } => {
                        self.semantic_tx = Some(tx);
                        self.semantic_rx = Some(rx);
                        self.semantic_events = None;
                        self.semantic_disabled_reason = None;
                        let summary = format!(
                            "Semantic model ready ({} runtime) - type a query and press Enter",
                            backend.label()
                        );
                        self.semantic.set_ready(summary.clone());
                        self.status
                            .success(format!("Semantic model ready via {}", backend.label()));
                    }
                    SemanticBootstrapEvent::Failed(message) => {
                        self.semantic_events = None;
                        self.semantic_tx = None;
                        self.semantic_rx = None;
                        self.semantic_disabled_reason = Some(message.clone());
                        self.semantic.set_disabled(message.clone());
                        self.status.error(message);
                    }
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.semantic_events = None;
                    if self.semantic_tx.is_none() {
                        let reason = "Semantic bootstrap task stopped unexpectedly".to_string();
                        self.semantic_disabled_reason = Some(reason.clone());
                        self.semantic.set_disabled(reason);
                    }
                    break;
                }
            }
        }
    }
}
