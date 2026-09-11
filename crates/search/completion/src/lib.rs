#![warn(clippy::all, clippy::pedantic)]

mod analyzer;
mod collectors;
mod metadata;
mod parser;
mod scoring;
mod service;
mod start_keywords;
mod tokens;
mod types;
mod validator;

pub use service::CompletionService;
pub use tokens::KEYWORDS;
pub use types::{CompletionBatch, CompletionContext, CompletionItem, CompletionKind};

pub(crate) use scoring::{is_table_identifier, match_score};
pub(crate) use tokens::{
    AGG_FUNCS, ALTER_TABLE_ACTIONS, CLAUSE_TOKENS, DATA_SOURCE_TOKENS, JOIN_TYPES,
    SELECT_LIST_TOKENS, VALUE_KEYWORDS,
};
pub(crate) use types::ContextHints;

#[cfg(test)]
mod tests;
