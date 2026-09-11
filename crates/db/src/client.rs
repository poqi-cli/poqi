use crate::cancellation::CancellationGuard;
use deadpool_postgres::Client as PoolClient;
use std::ops::{Deref, DerefMut};

#[derive(Debug)]
pub struct PooledClient {
    cancellation: Option<CancellationGuard>,
    inner: Option<PoolClient>,
}

impl PooledClient {
    pub(crate) fn new(inner: PoolClient) -> Self {
        Self {
            cancellation: None,
            inner: Some(inner),
        }
    }

    pub(crate) fn with_cancellation(inner: PoolClient, cancellation: CancellationGuard) -> Self {
        Self {
            cancellation: Some(cancellation),
            inner: Some(inner),
        }
    }
}

impl Drop for PooledClient {
    fn drop(&mut self) {
        let canceled = self
            .cancellation
            .as_ref()
            .is_some_and(CancellationGuard::retire);
        if canceled {
            if let Some(client) = self.inner.take() {
                drop(PoolClient::take(client));
            }
        }
    }
}

impl Deref for PooledClient {
    type Target = PoolClient;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().expect("pooled client is available")
    }
}

impl DerefMut for PooledClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut().expect("pooled client is available")
    }
}
