use std::time::Instant;

use crate::engine_worker::EngineRequestKind;

#[derive(Debug, Clone)]
pub(crate) enum PostResultsAction {
    ApplyCellEdit {
        row: usize,
        column: usize,
        value: Option<String>,
    },
    DeleteRow {
        row: usize,
    },
    HydrateRow {
        row: usize,
    },
    Semantic {
        query: String,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct ActiveRequest {
    pub(crate) id: u64,
    pub(crate) kind: EngineRequestKind,
    pub(crate) label: String,
    pub(crate) started_at: Instant,
    pub(crate) follow_up: Option<PostResultsAction>,
    pub(crate) refresh_catalog_on_success: bool,
}

impl ActiveRequest {
    pub(crate) fn new(
        id: u64,
        kind: EngineRequestKind,
        label: String,
        follow_up: Option<PostResultsAction>,
        refresh_catalog_on_success: bool,
    ) -> Self {
        Self {
            id,
            kind,
            label,
            started_at: Instant::now(),
            follow_up,
            refresh_catalog_on_success,
        }
    }
}

/// Tracks the currently running engine request plus the next identifier to use.
#[derive(Debug)]
pub(crate) struct RequestTracker {
    next_id: u64,
    active: Option<ActiveRequest>,
}

impl RequestTracker {
    pub(crate) fn new() -> Self {
        Self {
            next_id: 1,
            active: None,
        }
    }

    pub(super) fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        if self.next_id == 0 {
            self.next_id = 1;
        }
        id
    }

    pub(crate) fn start(&mut self, request: ActiveRequest) {
        self.active = Some(request);
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub(crate) fn active(&self) -> Option<&ActiveRequest> {
        self.active.as_ref()
    }

    pub(crate) fn take_if(
        &mut self,
        predicate: impl FnOnce(&ActiveRequest) -> bool,
    ) -> Option<ActiveRequest> {
        if self.active.as_ref().is_some_and(predicate) {
            self.active.take()
        } else {
            None
        }
    }
}
