// Completion collectors are split into focused modules so the service stays lean.
mod columns;
mod context_router;
mod contexts;
mod start;
mod tables;
mod token_sets;

pub(crate) use columns::{
    collect_columns, collect_columns_in_scope, collect_predicate_snippets, columns_for_hint,
    table_display_for_column,
};
pub(crate) use context_router::CandidateCollector;
pub(crate) use contexts::{
    collect_group_by, collect_having, collect_join_condition, collect_order_by, collect_returning,
    collect_table_context, collect_where, collect_window, merge_table_hint,
};
pub(crate) use start::{collect_start_ddl_keywords, collect_statement_keywords};
pub(crate) use tables::collect_tables;
pub(crate) use token_sets::collect_token_set;
