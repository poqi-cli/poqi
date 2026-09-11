use std::{
    fmt,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use parking_lot::{Mutex, RwLock};
use tokio_postgres::{CancelToken, NoTls};

use crate::{tls::TlsChoice, DatabaseError};

#[derive(Clone)]
pub(crate) struct CancellationRegistry {
    next_generation: Arc<AtomicU64>,
    active: Arc<RwLock<Option<Arc<OperationState>>>>,
}

impl Default for CancellationRegistry {
    fn default() -> Self {
        Self {
            next_generation: Arc::new(AtomicU64::new(1)),
            active: Arc::new(RwLock::new(None)),
        }
    }
}

impl fmt::Debug for CancellationRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CancellationRegistry")
            .field("has_active", &self.active.read().is_some())
            .finish_non_exhaustive()
    }
}

impl CancellationRegistry {
    pub(crate) fn begin(&self) -> CancellationGuard {
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        let state = Arc::new(OperationState {
            generation,
            status: Mutex::new(OperationStatus::default()),
        });
        *self.active.write() = Some(state.clone());
        CancellationGuard {
            registry: self.clone(),
            state,
        }
    }

    pub(crate) async fn cancel_current(&self) -> Result<bool, DatabaseError> {
        let token = {
            // Keep the registry read lock until this generation is marked. A completing
            // client takes the write lock before returning itself to the pool, so either
            // cancellation taints it or retirement makes it unavailable here.
            let active = self.active.read();
            let Some(active) = active.as_ref() else {
                return Ok(false);
            };
            let mut status = active.status.lock();
            if status.retired {
                return Ok(false);
            }
            status.canceled = true;
            status.token.clone()
        };

        let Some(token) = token else {
            // The operation is still waiting for a pooled client. Its cancellation guard
            // prevents the SQL closure from being dispatched after acquisition.
            return Ok(true);
        };
        match token.tls {
            TlsChoice::NoTls => token.token.cancel_query(NoTls).await,
            TlsChoice::Rustls(connector) => token.token.cancel_query(connector).await,
        }
        .map_err(DatabaseError::Cancel)?;
        Ok(true)
    }

    fn retire(&self, state: &OperationState) -> bool {
        let mut active = self.active.write();
        let mut status = state.status.lock();
        status.retired = true;
        let canceled = status.canceled;
        if active
            .as_ref()
            .is_some_and(|entry| entry.generation == state.generation)
        {
            *active = None;
        }
        canceled
    }
}

struct OperationState {
    generation: u64,
    status: Mutex<OperationStatus>,
}

#[derive(Default)]
struct OperationStatus {
    canceled: bool,
    retired: bool,
    token: Option<RegisteredToken>,
}

#[derive(Clone)]
struct RegisteredToken {
    token: CancelToken,
    tls: TlsChoice,
}

pub(crate) struct CancellationGuard {
    registry: CancellationRegistry,
    state: Arc<OperationState>,
}

impl fmt::Debug for CancellationGuard {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CancellationGuard")
            .field("generation", &self.state.generation)
            .finish_non_exhaustive()
    }
}

impl CancellationGuard {
    pub(crate) fn activate(&self, token: CancelToken, tls: TlsChoice) -> bool {
        let mut status = self.state.status.lock();
        if status.retired || status.canceled {
            return false;
        }
        status.token = Some(RegisteredToken { token, tls });
        true
    }

    pub(crate) fn retire(&self) -> bool {
        self.registry.retire(&self.state)
    }
}

impl Drop for CancellationGuard {
    fn drop(&mut self) {
        self.retire();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn retirement_before_cancel_makes_generation_unavailable() {
        let registry = CancellationRegistry::default();
        let guard = registry.begin();

        assert!(!guard.retire());
        assert!(!registry.cancel_current().await.expect("idle cancellation"));
    }

    #[tokio::test]
    async fn cancellation_before_retirement_taints_generation() {
        let registry = CancellationRegistry::default();
        let guard = registry.begin();

        assert!(registry
            .cancel_current()
            .await
            .expect("pending cancellation"));
        assert!(guard.retire());
        assert!(!registry
            .cancel_current()
            .await
            .expect("retired cancellation"));
    }

    #[test]
    fn newer_generation_does_not_cancel_older_pending_operation() {
        let registry = CancellationRegistry::default();
        let first = registry.begin();
        let second = registry.begin();

        assert!(!first.retire());
        assert!(!second.retire());
    }
}
