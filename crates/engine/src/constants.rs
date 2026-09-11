//! Shared constants for the engine crate.

/// Synthetic column alias injected whenever we surface CTID metadata.
pub const CTID_ALIAS: &str = "poqi__ctid";
pub const TABLEOID_ALIAS: &str = "poqi__tableoid";
pub const XMIN_ALIAS: &str = "poqi__xmin";
pub const PRIMARY_KEY_ALIAS_PREFIX: &str = "poqi__primary_key_";
