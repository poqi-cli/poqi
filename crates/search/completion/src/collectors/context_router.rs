use super::contexts;
use crate::analyzer::SqlScan;
use crate::metadata::CompletionMetadata;
use crate::start_keywords::StatementContext;
use crate::types::CompletionItem;

/// Bundles metadata-backed completion helpers so the main service stays lean.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CandidateCollector<'a> {
    metadata: &'a CompletionMetadata,
}

impl<'a> CandidateCollector<'a> {
    pub(crate) fn new(metadata: &'a CompletionMetadata) -> Self {
        Self { metadata }
    }

    pub(crate) fn table_hint_from_scan(self, scan: &SqlScan) -> Option<String> {
        if scan.tokens.is_empty() {
            return None;
        }
        for (idx, token) in scan.tokens.iter().enumerate().rev() {
            if idx > 0 {
                let schema = &scan.tokens[idx - 1];
                if self
                    .metadata
                    .find_table(Some(schema.as_str()), token.as_str())
                    .is_some()
                {
                    return Some(format!("{schema}.{token}"));
                }
            }
            if self.metadata.find_table(None, token.as_str()).is_some() {
                return Some(token.clone());
            }
        }
        None
    }

    pub(crate) fn collect_by_context(
        self,
        statement_context: StatementContext<'_>,
        candidates: &mut Vec<CompletionItem>,
    ) {
        contexts::collect_by_context(self.metadata, statement_context, candidates);
    }
}
