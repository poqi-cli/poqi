use super::shared::push_comma_candidate;
use crate::{CompletionContext, CompletionItem, CompletionService};

pub(crate) fn suggest(
    service: &CompletionService,
    ctx: super::StatementContext<'_>,
    candidates: &mut Vec<CompletionItem>,
) -> bool {
    if !matches!(
        ctx.completion_context,
        CompletionContext::Start | CompletionContext::Table { .. }
    ) {
        return false;
    }
    collect_drop_targets(service, ctx, schema_hint(&ctx), candidates);
    true
}

fn collect_drop_targets(
    service: &CompletionService,
    ctx: super::StatementContext<'_>,
    schema_hint: Option<&str>,
    candidates: &mut Vec<CompletionItem>,
) {
    let state = DropTableState::new(ctx);
    if state.allow_if_exists {
        CompletionService::collect_token_set(candidates, ctx.prefix, &["IF EXISTS"], "ddl");
    }
    if state.allow_comma {
        push_comma_candidate(ctx.prefix, candidates);
    }
    if state.allow_tables {
        service.collect_tables(candidates, ctx.prefix, schema_hint, true);
    }
}

fn schema_hint<'a>(ctx: &'a super::StatementContext<'a>) -> Option<&'a str> {
    match ctx.completion_context {
        CompletionContext::Table { ref schema_hint } => schema_hint.as_deref(),
        _ => None,
    }
}

/// Tracks what the user has typed after `DROP TABLE` so we only surface valid next tokens.
#[derive(Debug, Clone, Copy)]
struct DropTableState {
    allow_if_exists: bool,
    allow_tables: bool,
    allow_comma: bool,
}

impl DropTableState {
    fn new(ctx: super::StatementContext<'_>) -> Self {
        let cursor = ctx.cursor.min(ctx.buffer.len());
        let slice = ctx.buffer.get(..cursor).unwrap_or("");
        let lower = slice.to_ascii_lowercase();
        let drop_idx = lower.rfind("drop table");
        let tokens_since_drop = super::tokens_since_keyword(ctx.scan.tokens.as_slice(), "drop");
        let has_if_exists = tokens_since_drop.iter().any(|token| token == "exists");

        let mut remainder = drop_idx.map_or("", |idx| &lower[idx + "drop table".len()..]);
        remainder = remainder.trim_start();
        if remainder.starts_with("if exists") {
            remainder = remainder["if exists".len()..].trim_start();
        }

        // Track already-typed targets to decide whether to suggest commas or additional table names.
        let any_targets = remainder
            .split(',')
            .any(|segment| !segment.trim().is_empty());
        let trimmed = remainder.trim_end();
        let ends_with_comma = trimmed.ends_with(',');
        let editing_segment = !ctx.hints.token_is_empty || !ctx.prefix.is_empty();

        let allow_if_exists = !has_if_exists && !any_targets;
        let allow_comma = any_targets && !editing_segment && !ends_with_comma;
        let allow_tables = !any_targets || editing_segment || ends_with_comma;

        Self {
            allow_if_exists,
            allow_tables,
            allow_comma,
        }
    }
}
