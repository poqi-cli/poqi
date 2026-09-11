use crate::analyzer::{
    analyze_query_scope, current_token, detect_context, previous_non_whitespace, scan_sql,
    QueryScope,
};
use crate::collectors::{self, CandidateCollector};
use crate::metadata::{ColumnEntry, CompletionMetadata};
use crate::parser::{analyze_query, ParsedQuery};
use crate::scoring::literal_value_present;
use crate::start_keywords::{
    detect_start_keyword, dispatch_start_keyword, StartKeyword, StatementContext,
};
use crate::types::{CompletionBatch, CompletionContext, CompletionItem, ContextHints};
use crate::validator::PgQueryValidator;
use poqi_catalog::CatalogSnapshot;
use smallvec::SmallVec;
use std::collections::HashSet;

#[derive(Debug)]
pub struct CompletionService {
    /// Cached snapshot of schemas/tables/columns used for quick lookups.
    metadata: CompletionMetadata,
    /// Validates risky insertions (clauses/snippets) via `pg_query` before surfacing them.
    validator: PgQueryValidator,
}

impl CompletionService {
    #[must_use]
    pub fn new(snapshot: CatalogSnapshot) -> Self {
        Self {
            metadata: CompletionMetadata::new(snapshot),
            validator: PgQueryValidator::new(),
        }
    }

    pub fn update_catalog(&mut self, snapshot: CatalogSnapshot) {
        self.metadata = CompletionMetadata::new(snapshot);
    }

    #[must_use]
    pub fn suggest(&self, buffer: &str, cursor: usize) -> CompletionBatch {
        let collector = CandidateCollector::new(&self.metadata);
        let scan = scan_sql(buffer, cursor);
        if scan.suppressed {
            return CompletionBatch::default();
        }
        let start_keyword = detect_start_keyword(&scan.tokens);
        let token = current_token(buffer, cursor);
        let token_is_empty = token.current.trim().is_empty();
        let prev_non_ws = previous_non_whitespace(buffer, token.start_offset);
        if token_is_empty && prev_non_ws == Some(';') {
            return CompletionBatch::default();
        }
        let query_scope = analyze_query_scope(buffer, cursor);
        let context = detect_context(buffer, cursor, &token, &scan, &query_scope);
        let prefix = token.current.to_ascii_lowercase();
        let literal_filled = literal_value_present(buffer, token.start_offset);

        match context {
            CompletionContext::Literal
                if literal_filled
                    && !matches!(
                        start_keyword,
                        Some(StartKeyword::Update | StartKeyword::InsertInto | StartKeyword::With)
                    ) =>
            {
                return CompletionBatch::default();
            }
            CompletionContext::Literal
                if !(matches!(start_keyword, Some(StartKeyword::With))
                    || (matches!(start_keyword, Some(StartKeyword::Update)) && token_is_empty)) =>
            {
                return CompletionBatch::default();
            }
            CompletionContext::General if prefix.is_empty() => {
                return CompletionBatch::default();
            }
            _ => {}
        }

        let parse_info = analyze_query(buffer);
        let scan_table_hint = collector.table_hint_from_scan(&scan);
        let mut candidates: Vec<CompletionItem> = Vec::new();
        let last_token = scan.tokens.last().map(String::as_str);
        let hints = ContextHints {
            parser_table_hint: parse_info.as_ref().and_then(ParsedQuery::primary_table),
            scan_table_hint: scan_table_hint.as_deref(),
            token_is_empty,
            last_token,
            scope: &query_scope,
        };
        let statement_context = StatementContext {
            buffer,
            cursor,
            token_start: token.start_offset,
            completion_context: &context,
            prefix: &prefix,
            hints,
            scan: &scan,
            scope: &query_scope,
        };
        self.collect_by_context(collector, statement_context, start_keyword, &mut candidates);

        candidates.sort_by(|a, b| a.score.cmp(&b.score).then_with(|| a.label.cmp(&b.label)));
        let mut seen_labels = HashSet::new();
        candidates.retain(|candidate| seen_labels.insert(candidate.label.clone()));
        candidates.truncate(24);
        let candidates = self
            .validator
            .filter(buffer, cursor, token.start_offset, candidates);
        CompletionBatch {
            items: SmallVec::from_vec(candidates),
        }
    }

    fn collect_by_context(
        &self,
        collector: CandidateCollector<'_>,
        statement_context: StatementContext<'_>,
        start_keyword: Option<StartKeyword>,
        candidates: &mut Vec<CompletionItem>,
    ) {
        if let Some(keyword) = start_keyword {
            if dispatch_start_keyword(self, keyword, statement_context, candidates) {
                return;
            }
        }

        collector.collect_by_context(statement_context, candidates);
    }

    pub(crate) fn collect_statement_keywords(prefix: &str, candidates: &mut Vec<CompletionItem>) {
        collectors::collect_statement_keywords(prefix, candidates);
    }

    pub(crate) fn collect_start_ddl_keywords(
        statement_context: StatementContext<'_>,
        candidates: &mut Vec<CompletionItem>,
    ) {
        collectors::collect_start_ddl_keywords(statement_context, candidates);
    }

    pub(crate) fn collect_table_context(
        &self,
        prefix: &str,
        schema_hint: Option<&str>,
        candidates: &mut Vec<CompletionItem>,
    ) {
        collectors::collect_table_context(&self.metadata, prefix, schema_hint, candidates);
    }

    pub(crate) fn merge_table_hint<'a>(
        hints: ContextHints<'a>,
        table_hint: Option<&'a str>,
    ) -> Option<&'a str> {
        collectors::merge_table_hint(hints, table_hint)
    }

    pub(crate) fn collect_join_condition(
        &self,
        prefix: &str,
        hints: ContextHints<'_>,
        candidates: &mut Vec<CompletionItem>,
    ) {
        collectors::collect_join_condition(&self.metadata, prefix, hints, candidates);
    }

    pub(crate) fn collect_where(
        &self,
        prefix: &str,
        hints: ContextHints<'_>,
        candidates: &mut Vec<CompletionItem>,
    ) {
        collectors::collect_where(&self.metadata, prefix, hints, candidates);
    }

    pub(crate) fn collect_group_by(
        &self,
        prefix: &str,
        hints: ContextHints<'_>,
        candidates: &mut Vec<CompletionItem>,
    ) {
        collectors::collect_group_by(&self.metadata, prefix, hints, candidates);
    }

    pub(crate) fn collect_having(
        &self,
        prefix: &str,
        hints: ContextHints<'_>,
        candidates: &mut Vec<CompletionItem>,
    ) {
        collectors::collect_having(&self.metadata, prefix, hints, candidates);
    }

    pub(crate) fn collect_window(
        &self,
        prefix: &str,
        hints: ContextHints<'_>,
        candidates: &mut Vec<CompletionItem>,
    ) {
        collectors::collect_window(&self.metadata, prefix, hints, candidates);
    }

    pub(crate) fn collect_order_by(
        &self,
        prefix: &str,
        hints: ContextHints<'_>,
        candidates: &mut Vec<CompletionItem>,
    ) {
        collectors::collect_order_by(&self.metadata, prefix, hints, candidates);
    }

    pub(crate) fn collect_returning(
        &self,
        prefix: &str,
        hints: ContextHints<'_>,
        candidates: &mut Vec<CompletionItem>,
    ) {
        collectors::collect_returning(&self.metadata, prefix, hints, candidates);
    }

    pub(crate) fn collect_token_set(
        items: &mut Vec<CompletionItem>,
        prefix: &str,
        tokens: &[&str],
        detail: &str,
    ) {
        collectors::collect_token_set(items, prefix, tokens, detail);
    }

    pub(crate) fn collect_tables(
        &self,
        items: &mut Vec<CompletionItem>,
        prefix: &str,
        schema_hint: Option<&str>,
        allow_schema_prefix: bool,
    ) {
        collectors::collect_tables(
            items,
            &self.metadata,
            prefix,
            schema_hint,
            allow_schema_prefix,
        );
    }

    pub(crate) fn collect_columns(
        &self,
        items: &mut Vec<CompletionItem>,
        prefix: &str,
        table_hint: Option<&str>,
    ) {
        collectors::collect_columns(&self.metadata, items, prefix, table_hint);
    }

    pub(crate) fn collect_columns_in_scope(
        &self,
        items: &mut Vec<CompletionItem>,
        prefix: &str,
        scope: &QueryScope,
    ) {
        collectors::collect_columns_in_scope(&self.metadata, items, prefix, scope);
    }

    pub(crate) fn columns_for_hint(&self, table_hint: Option<&str>) -> &[ColumnEntry] {
        collectors::columns_for_hint(&self.metadata, table_hint)
    }

    pub(crate) fn table_display_for_column(&self, column: &ColumnEntry) -> String {
        collectors::table_display_for_column(&self.metadata, column)
    }

    pub(crate) fn collect_predicate_snippets(
        &self,
        items: &mut Vec<CompletionItem>,
        prefix: &str,
        table_hint: Option<&str>,
    ) {
        collectors::collect_predicate_snippets(&self.metadata, items, prefix, table_hint);
    }
}
