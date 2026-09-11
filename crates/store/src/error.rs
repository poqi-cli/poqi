use thiserror::Error;

/// Errors that can occur while interacting with the local persistence store.
#[derive(Debug, Error)]
pub enum StoreError {
    /// The provided connection string could not be parsed.
    #[error("invalid connection URI {uri}")]
    InvalidConnectionUri {
        /// The URI that failed validation.
        uri: String,
        /// The underlying parse error.
        #[source]
        source: url::ParseError,
    },
}
