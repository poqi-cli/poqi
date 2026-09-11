use super::columns::{
    collect_columns, collect_columns_in_scope, collect_predicate_snippets,
    collect_predicate_snippets_in_scope, collect_qualified_columns,
};
use super::start::collect_start_keywords;
use super::tables::collect_tables;
use super::token_sets::{collect_keywords, collect_token_set};
use crate::metadata::CompletionMetadata;
use crate::scoring::{is_table_identifier, match_score};
use crate::start_keywords::select::select_list_state;
use crate::start_keywords::StatementContext;
use crate::tokens::{
    AGG_FUNCS, CLAUSE_TOKENS, CMP_OPS, DATA_SOURCE_TOKENS, GROUPING_TOKENS, JOIN_TYPES,
    LOGICAL_OPS, ORDER_MODS, SELECT_LIST_TOKENS, VALUE_KEYWORDS, WINDOW_TOKENS,
};
use crate::types::{CompletionContext, CompletionItem, CompletionKind, ContextHints};

pub(crate) fn collect_by_context(
    metadata: &CompletionMetadata,
    statement_context: StatementContext<'_>,
    candidates: &mut Vec<CompletionItem>,
) {
    match statement_context.completion_context {
        CompletionContext::Start => collect_start_keywords(statement_context, candidates),
        CompletionContext::Table { ref schema_hint } => collect_table_context(
            metadata,
            statement_context.prefix,
            schema_hint.as_deref(),
            candidates,
        ),
        CompletionContext::Column { ref table_hint } => collect_column_context(
            metadata,
            statement_context.prefix,
            table_hint.as_deref(),
            statement_context.hints,
            candidates,
        ),
        CompletionContext::SelectList => {
            collect_select_list_default(metadata, statement_context, candidates);
        }
        CompletionContext::FromSource => collect_from_source_default(
            metadata,
            statement_context.prefix,
            statement_context.hints,
            candidates,
        ),
        CompletionContext::JoinType => {
            collect_token_set(candidates, statement_context.prefix, JOIN_TYPES, "join");
        }
        CompletionContext::JoinCondition => collect_join_condition(
            metadata,
            statement_context.prefix,
            statement_context.hints,
            candidates,
        ),
        CompletionContext::Where => collect_where(
            metadata,
            statement_context.prefix,
            statement_context.hints,
            candidates,
        ),
        CompletionContext::GroupBy => collect_group_by(
            metadata,
            statement_context.prefix,
            statement_context.hints,
            candidates,
        ),
        CompletionContext::Having => collect_having(
            metadata,
            statement_context.prefix,
            statement_context.hints,
            candidates,
        ),
        CompletionContext::Window => collect_window(
            metadata,
            statement_context.prefix,
            statement_context.hints,
            candidates,
        ),
        CompletionContext::OrderBy => collect_order_by(
            metadata,
            statement_context.prefix,
            statement_context.hints,
            candidates,
        ),
        CompletionContext::Limit | CompletionContext::Offset | CompletionContext::Literal => {}
        CompletionContext::Returning => collect_returning(
            metadata,
            statement_context.prefix,
            statement_context.hints,
            candidates,
        ),
        CompletionContext::InsertValues | CompletionContext::UpdateSet => {
            let hint = merge_table_hint(statement_context.hints, None);
            collect_columns(metadata, candidates, statement_context.prefix, hint);
            collect_token_set(
                candidates,
                statement_context.prefix,
                VALUE_KEYWORDS,
                "literal",
            );
        }
        CompletionContext::Keyword | CompletionContext::General => {
            collect_token_set(
                candidates,
                statement_context.prefix,
                CLAUSE_TOKENS,
                "clause",
            );
            collect_keywords(candidates, statement_context.prefix);
            collect_tables(candidates, metadata, statement_context.prefix, None, false);
            collect_columns(metadata, candidates, statement_context.prefix, None);
        }
    }
}

pub(crate) fn collect_table_context(
    metadata: &CompletionMetadata,
    prefix: &str,
    schema_hint: Option<&str>,
    candidates: &mut Vec<CompletionItem>,
) {
    collect_tables(candidates, metadata, prefix, schema_hint, true);
}

pub(crate) fn collect_column_context(
    metadata: &CompletionMetadata,
    prefix: &str,
    table_hint: Option<&str>,
    hints: ContextHints<'_>,
    candidates: &mut Vec<CompletionItem>,
) {
    if let Some(qualifier) = table_hint.filter(|hint| hints.scope.has_qualifier(hint)) {
        collect_qualified_columns(metadata, candidates, prefix, hints.scope, qualifier);
        return;
    }
    if let Some(schema) = table_hint.filter(|hint| metadata.schema_exists(hint)) {
        collect_tables(candidates, metadata, prefix, Some(schema), true);
        return;
    }
    if let Some(qualifier) = table_hint {
        collect_qualified_columns(metadata, candidates, prefix, hints.scope, qualifier);
        return;
    }
    collect_context_columns(metadata, candidates, prefix, hints);
}

pub(crate) fn collect_select_list_default(
    metadata: &CompletionMetadata,
    statement_context: StatementContext<'_>,
    candidates: &mut Vec<CompletionItem>,
) {
    let select_list = select_list_state(&statement_context.scan.tokens, statement_context.scope);
    if !select_list.has_star {
        collect_token_set(
            candidates,
            statement_context.prefix,
            SELECT_LIST_TOKENS,
            "select",
        );
    }
    if !select_list.has_star {
        collect_token_set(candidates, statement_context.prefix, AGG_FUNCS, "aggregate");
    }
    if select_list.has_items && match_score(statement_context.prefix, "FROM").is_some() {
        candidates.push(CompletionItem {
            label: "FROM".to_string(),
            insert_text: "FROM".to_string(),
            detail: "clause".to_string(),
            kind: CompletionKind::Keyword,
            score: 0,
        });
    }
    if !select_list.has_star {
        collect_context_columns(
            metadata,
            candidates,
            statement_context.prefix,
            statement_context.hints,
        );
    }
}

pub(crate) fn collect_from_source_default(
    metadata: &CompletionMetadata,
    prefix: &str,
    hints: ContextHints<'_>,
    candidates: &mut Vec<CompletionItem>,
) {
    let completed_source =
        hints.last_token.is_some_and(is_table_identifier) || !hints.scope.relations.is_empty();
    let show_sources = !(hints.token_is_empty && prefix.is_empty() && completed_source);
    if show_sources {
        collect_tables(candidates, metadata, prefix, None, true);
        collect_token_set(candidates, prefix, DATA_SOURCE_TOKENS, "source");
    }
    collect_token_set(candidates, prefix, CLAUSE_TOKENS, "clause");
}

pub(crate) fn collect_join_condition(
    metadata: &CompletionMetadata,
    prefix: &str,
    hints: ContextHints<'_>,
    candidates: &mut Vec<CompletionItem>,
) {
    collect_context_columns(metadata, candidates, prefix, hints);
    collect_token_set(candidates, prefix, CMP_OPS, "comparison");
    collect_token_set(candidates, prefix, LOGICAL_OPS, "logical");
    collect_token_set(candidates, prefix, VALUE_KEYWORDS, "literal");
}

pub(crate) fn collect_where(
    metadata: &CompletionMetadata,
    prefix: &str,
    hints: ContextHints<'_>,
    candidates: &mut Vec<CompletionItem>,
) {
    collect_context_predicates(metadata, candidates, prefix, hints);
    collect_context_columns(metadata, candidates, prefix, hints);
    collect_token_set(candidates, prefix, CMP_OPS, "comparison");
    collect_token_set(candidates, prefix, LOGICAL_OPS, "logical");
    collect_token_set(candidates, prefix, VALUE_KEYWORDS, "literal");
}

pub(crate) fn collect_group_by(
    metadata: &CompletionMetadata,
    prefix: &str,
    hints: ContextHints<'_>,
    candidates: &mut Vec<CompletionItem>,
) {
    collect_context_columns(metadata, candidates, prefix, hints);
    collect_token_set(candidates, prefix, GROUPING_TOKENS, "grouping");
}

pub(crate) fn collect_having(
    metadata: &CompletionMetadata,
    prefix: &str,
    hints: ContextHints<'_>,
    candidates: &mut Vec<CompletionItem>,
) {
    collect_context_columns(metadata, candidates, prefix, hints);
    collect_token_set(candidates, prefix, AGG_FUNCS, "aggregate");
    collect_token_set(candidates, prefix, CMP_OPS, "comparison");
    collect_token_set(candidates, prefix, LOGICAL_OPS, "logical");
}

pub(crate) fn collect_window(
    metadata: &CompletionMetadata,
    prefix: &str,
    hints: ContextHints<'_>,
    candidates: &mut Vec<CompletionItem>,
) {
    collect_context_columns(metadata, candidates, prefix, hints);
    collect_token_set(candidates, prefix, WINDOW_TOKENS, "window");
}

pub(crate) fn collect_order_by(
    metadata: &CompletionMetadata,
    prefix: &str,
    hints: ContextHints<'_>,
    candidates: &mut Vec<CompletionItem>,
) {
    collect_context_columns(metadata, candidates, prefix, hints);
    collect_token_set(candidates, prefix, ORDER_MODS, "ordering");
}

pub(crate) fn collect_returning(
    metadata: &CompletionMetadata,
    prefix: &str,
    hints: ContextHints<'_>,
    candidates: &mut Vec<CompletionItem>,
) {
    collect_context_columns(metadata, candidates, prefix, hints);
}

fn collect_context_columns(
    metadata: &CompletionMetadata,
    candidates: &mut Vec<CompletionItem>,
    prefix: &str,
    hints: ContextHints<'_>,
) {
    if hints.scope.has_relation_source {
        collect_columns_in_scope(metadata, candidates, prefix, hints.scope);
    } else {
        let hint = merge_table_hint(hints, None);
        collect_columns(metadata, candidates, prefix, hint);
    }
}

fn collect_context_predicates(
    metadata: &CompletionMetadata,
    candidates: &mut Vec<CompletionItem>,
    prefix: &str,
    hints: ContextHints<'_>,
) {
    if hints.scope.has_relation_source {
        collect_predicate_snippets_in_scope(metadata, candidates, prefix, hints.scope);
    } else {
        let hint = merge_table_hint(hints, None);
        collect_predicate_snippets(metadata, candidates, prefix, hint);
    }
}

pub(crate) fn merge_table_hint<'a>(
    hints: ContextHints<'a>,
    table_hint: Option<&'a str>,
) -> Option<&'a str> {
    table_hint
        .or(hints.parser_table_hint)
        .or(hints.scan_table_hint)
}
