#![warn(clippy::all, clippy::pedantic)]

mod connection;
mod crypto;
mod encryption;
mod error;
mod models;
mod schema;
pub mod settings;
mod store;

pub use connection::{ConnectionProfileStore, NewConnectionProfile, StoredConnectionProfile};
pub use error::StoreError;
pub use models::{Favorite, RecentEntry, StoredObjectKind};
pub use settings::{SemanticRuntimePreference, SettingsStore};
pub use store::Store;

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod tests;
