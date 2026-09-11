mod actions;
mod catalog_refresh;
mod editor;
mod focus;
mod key_handling;
mod mouse;
mod requests;
mod results_hitbox;
mod results_mutation;
mod scrolling;
mod semantic;
mod settings;
mod text_nav;

use self::requests::RequestTracker;
use catalog_refresh::CatalogRefresh;
use editor::EditorOverlay;
use parking_lot::Mutex;
use poqi_catalog::{schema_table_map, CatalogSnapshot};
use poqi_config::AppConfig;
use poqi_search_completion::CompletionService;
use poqi_search_semantic::SearchOptions;
use poqi_store::Store;
use ratatui::layout::Rect;
use std::{
    collections::HashMap,
    sync::{atomic::AtomicBool, Arc},
};
use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};

use crate::{
    engine_worker::{EngineCommand, EngineRequestKind, EngineResponse},
    input::FocusPanel,
    keymap::KeymapEngine,
    semantic_bootstrap::{SemanticBootstrapCoordinator, SemanticBootstrapEvent},
    semantic_worker::{SemanticCommand, SemanticResponse},
    state::{
        editor_state::EditorState, editor_syntax::SyntaxHighlighter, results_state::ResultsState,
        schema::SchemaState, semantic::SemanticSearchState, settings::SettingsView,
        status::StatusBar, table_detail::TableDetailState,
    },
    terminal_capabilities::{GlyphSet, TerminalCapabilities},
    theme::Theme,
    ExitReason, UiRuntimeSettings,
};

#[cfg(test)]
mod tests;

pub(super) use results_hitbox::{ResultsHitColumn, ResultsHitbox};
pub(super) use results_mutation::MutationContext;
use results_mutation::PendingFocus;
pub(super) use scrolling::{ScrollDrag, ScrollTarget, ScrollbarAxis, ScrollbarContext};

#[derive(Debug, Clone, Copy)]
pub(crate) struct CompletionPopupHitbox {
    pub(crate) area: Rect,
    pub(crate) content_area: Rect,
    pub(crate) start_index: usize,
    pub(crate) visible_rows: usize,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SettingsHitbox {
    pub(super) popup_area: Rect,
    pub(super) body_area: Rect,
    pub(super) action_area: Rect,
    pub(super) body_offset: usize,
    pub(super) action_index: usize,
}

pub struct App {
    pub(super) config: AppConfig,
    store: Store,
    /// Panel currently receiving keyboard focus and direct input.
    pub focus: FocusPanel,
    /// Highlighted panel when the selector layer is active.
    pub(super) panel_cursor: FocusPanel,
    /// Remembers which top-row panel we last hovered so `W`/`S` hops feel natural.
    pub(super) panel_select_last_top: FocusPanel,
    /// Coarse UI layer state that determines how we interpret inputs.
    pub(super) layer: UiLayer,
    /// Reason for exiting the app
    pub exit_reason: Option<ExitReason>,
    /// Which panel (if any) is currently zoomed to full-screen
    pub(super) zoomed_panel: Option<FocusPanel>,
    pub(super) schema: SchemaState,
    pub(super) table_detail: TableDetailState,
    pub(super) editor: EditorState,
    pub(super) semantic: SemanticSearchState,
    pub(super) results: ResultsState,
    pub(super) status: StatusBar,
    pub(super) theme: Theme,
    pub(super) ui_settings: UiRuntimeSettings,
    terminal_capabilities: TerminalCapabilities,
    pub(super) keymap: KeymapEngine,
    pub(super) completion_service: CompletionService,
    catalog: CatalogSnapshot,
    pub(super) syntax: SyntaxHighlighter,
    catalog_refresh: CatalogRefresh,
    pub(super) engine_tx: UnboundedSender<EngineCommand>,
    pub(super) engine_rx: UnboundedReceiver<EngineResponse>,
    pub(super) semantic_tx: Option<UnboundedSender<SemanticCommand>>,
    pub(super) semantic_rx: Option<UnboundedReceiver<SemanticResponse>>,
    pub(super) semantic_events: Option<UnboundedReceiver<SemanticBootstrapEvent>>,
    pub(super) semantic_coordinator: SemanticBootstrapCoordinator,
    pub(super) semantic_handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
    requests: RequestTracker,
    pub(super) schema_area: Option<Rect>,
    pub(super) schema_scrollbar: Option<ScrollbarContext>,
    pub(super) schema_viewport_rows: usize,
    pub(super) editor_area: Option<Rect>,
    pub(super) editor_text_area: Option<Rect>,
    pub(super) editor_cursor: Option<(u16, u16)>,
    pub(super) completion_popup_hitbox: Option<CompletionPopupHitbox>,
    pub(super) semantic_area: Option<Rect>,
    pub(super) semantic_text_area: Option<Rect>,
    pub(super) results_area: Option<Rect>,
    pub(super) results_hitbox: Option<ResultsHitbox>,
    pub(super) results_vertical_scrollbar: Option<ScrollbarContext>,
    pub(super) results_horizontal_scrollbar: Option<ScrollbarContext>,
    pub(super) scroll_drag: Option<ScrollDrag>,
    pub(super) status_area: Option<Rect>,
    pub(super) editor_mouse_drag_anchor: Option<(usize, usize)>,
    pub(super) semantic_mouse_drag_anchor: Option<(usize, usize)>,
    editor_overlay: Option<EditorOverlay>,
    pending_focus: Option<PendingFocus>,
    semantic_options: SearchOptions,
    semantic_title_column: Option<String>,
    semantic_disabled_reason: Option<String>,
    next_semantic_request_id: u64,
    active_semantic: Option<ActiveSemanticJob>,
    pub(super) settings_view: Option<SettingsView>,
    pub(super) settings_hitbox: Option<SettingsHitbox>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UiLayer {
    PanelSelect,
    PanelFocused,
    Settings,
}

impl App {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: &AppConfig,
        ui_settings: UiRuntimeSettings,
        store: Store,
        schemas: HashMap<String, Vec<String>>,
        catalog: CatalogSnapshot,
        engine_tx: UnboundedSender<EngineCommand>,
        engine_rx: UnboundedReceiver<EngineResponse>,
        semantic_runtime: SemanticRuntime,
    ) -> Self {
        Self::new_with_capabilities(
            config,
            ui_settings,
            store,
            schemas,
            catalog,
            engine_tx,
            engine_rx,
            semantic_runtime,
            TerminalCapabilities::detect(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_capabilities(
        config: &AppConfig,
        ui_settings: UiRuntimeSettings,
        store: Store,
        schemas: HashMap<String, Vec<String>>,
        catalog_snapshot: CatalogSnapshot,
        engine_tx: UnboundedSender<EngineCommand>,
        engine_rx: UnboundedReceiver<EngineResponse>,
        semantic_runtime: SemanticRuntime,
        terminal_capabilities: TerminalCapabilities,
    ) -> Self {
        let config_clone = config.clone();
        let theme = Theme::from_config(&config_clone);
        let keymap = KeymapEngine::from_config(&config_clone, ui_settings.fast_scroll_step())
            .unwrap_or_else(|err| {
                tracing::error!(
                    ?err,
                    "failed to build keymap from config; falling back to defaults"
                );
                KeymapEngine::default_with_step(ui_settings.fast_scroll_step())
            });
        let mut status = StatusBar::new(
            ui_settings.status_auto_clear_duration(),
            ui_settings.status_expire_duration(),
        );
        status.info(
            "Panel select: WASD/arrows to choose, Shift for fast, F to focus, Esc to go back, Ctrl+C to quit",
        );
        let completion_service = CompletionService::new(catalog_snapshot.clone());
        let table_detail = TableDetailState::new(catalog_snapshot.clone());
        let mut semantic_state = SemanticSearchState::new();
        if let Some(reason) = semantic_runtime.disabled_reason.clone() {
            semantic_state.set_disabled(reason);
        } else if let Some(label) = semantic_runtime.initial_loading_label.clone() {
            semantic_state.set_loading(label);
        } else if let Some(summary) = semantic_runtime.initial_ready_summary.clone() {
            semantic_state.set_ready(summary);
        }
        Self {
            config: config_clone,
            store,
            focus: FocusPanel::Schema,
            panel_cursor: FocusPanel::Schema,
            panel_select_last_top: FocusPanel::Schema,
            layer: UiLayer::PanelSelect,
            exit_reason: None,
            zoomed_panel: None,
            schema: SchemaState::new(schemas),
            table_detail,
            editor: EditorState::new(),
            semantic: semantic_state,
            results: ResultsState::new(),
            status,
            theme,
            ui_settings,
            terminal_capabilities,
            keymap,
            completion_service,
            catalog: catalog_snapshot,
            catalog_refresh: CatalogRefresh::new(),
            syntax: SyntaxHighlighter::new(),
            engine_tx,
            engine_rx,
            semantic_tx: semantic_runtime.tx,
            semantic_rx: semantic_runtime.rx,
            semantic_events: semantic_runtime.events,
            semantic_coordinator: SemanticBootstrapCoordinator::new(),
            semantic_handles: Arc::new(Mutex::new(Vec::new())),
            requests: RequestTracker::new(),
            schema_area: None,
            schema_scrollbar: None,
            schema_viewport_rows: 1,
            editor_area: None,
            editor_text_area: None,
            editor_cursor: None,
            completion_popup_hitbox: None,
            semantic_area: None,
            semantic_text_area: None,
            results_area: None,
            results_hitbox: None,
            results_vertical_scrollbar: None,
            results_horizontal_scrollbar: None,
            scroll_drag: None,
            status_area: None,
            editor_mouse_drag_anchor: None,
            semantic_mouse_drag_anchor: None,
            editor_overlay: None,
            pending_focus: None,
            semantic_options: semantic_runtime.options,
            semantic_title_column: semantic_runtime.title_column,
            semantic_disabled_reason: semantic_runtime.disabled_reason,
            next_semantic_request_id: 1,
            active_semantic: None,
            settings_view: None,
            settings_hitbox: None,
        }
    }

    pub(crate) fn results_horizontal_scrollbar_enabled(&self) -> bool {
        self.terminal_capabilities
            .results_horizontal_scrollbar_enabled()
    }

    pub(crate) fn glyphs(&self) -> GlyphSet {
        self.terminal_capabilities.glyphs()
    }

    pub fn on_tick(&mut self) {
        self.status.maybe_clear();
        self.poll_semantic_bootstrap();
        self.poll_engine();
        self.poll_semantic();
        self.poll_catalog_refresh();
    }

    pub(crate) fn signal_shutdown(&mut self) {
        if let Some(active) = self.active_semantic.as_ref() {
            active
                .cancel_flag
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        let _ = self.engine_tx.send(EngineCommand::CancelActive);
    }

    pub(crate) fn install_semantic_lifecycle(
        &mut self,
        coordinator: SemanticBootstrapCoordinator,
        handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
    ) {
        self.semantic_coordinator = coordinator;
        self.semantic_handles = handles;
    }

    fn poll_catalog_refresh(&mut self) {
        let now = std::time::Instant::now();
        if !self.catalog_refresh.ready_to_dispatch(now) || self.requests.is_active() {
            return;
        }

        let request_id = self.next_request_id();
        if self
            .engine_tx
            .send(EngineCommand::RefreshCatalog { request_id })
            .is_ok()
        {
            self.catalog_refresh.on_dispatched(request_id);
            self.start_request(
                request_id,
                EngineRequestKind::CatalogRefresh,
                "Schema catalog".to_string(),
                None,
                false,
            );
            self.status.info("Refreshing catalog…");
        } else {
            self.status.warning("Engine unavailable");
            self.catalog_refresh.on_failure(now);
        }
    }

    fn apply_catalog_snapshot(&mut self, snapshot: &CatalogSnapshot) {
        self.catalog = snapshot.clone();
        self.completion_service.update_catalog(snapshot.clone());
        self.table_detail.replace(snapshot.clone());
        let schemas = schema_table_map(snapshot);
        self.schema.replace(schemas);
        self.table_detail
            .follow_selection(self.schema.selected_table_name().as_ref());
    }
}

#[derive(Debug)]
pub(crate) struct SemanticRuntime {
    pub(crate) tx: Option<UnboundedSender<SemanticCommand>>,
    pub(crate) rx: Option<UnboundedReceiver<SemanticResponse>>,
    pub(crate) options: SearchOptions,
    pub(crate) title_column: Option<String>,
    pub(crate) disabled_reason: Option<String>,
    pub(crate) events: Option<UnboundedReceiver<SemanticBootstrapEvent>>,
    pub(crate) initial_loading_label: Option<String>,
    pub(crate) initial_ready_summary: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct ActiveSemanticJob {
    id: u64,
    table: Option<String>,
    cancel_flag: Arc<AtomicBool>,
}
