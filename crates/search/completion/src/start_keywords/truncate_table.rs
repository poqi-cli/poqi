use crate::analyzer::previous_non_whitespace;
use crate::{CompletionContext, CompletionItem, CompletionService};

pub(crate) fn suggest(
    service: &CompletionService,
    ctx: super::StatementContext<'_>,
    candidates: &mut Vec<CompletionItem>,
) -> bool {
    match ctx.completion_context {
        CompletionContext::Start => {
            let prev_char = previous_non_whitespace(ctx.buffer, ctx.cursor);
            let is_comma = prev_char == Some(',');

            let mut should_suggest_tables = is_comma;
            if !should_suggest_tables {
                if let Some(last) = ctx.hints.last_token {
                    let last_upper = last.to_ascii_uppercase();
                    if last_upper == "TRUNCATE" || last_upper == "TABLE" {
                        should_suggest_tables = true;
                    }
                }
            }

            if should_suggest_tables {
                service.collect_table_context(ctx.prefix, None, candidates);
            }
            true
        }
        CompletionContext::Table { ref schema_hint } => {
            service.collect_table_context(ctx.prefix, schema_hint.as_deref(), candidates);
            true
        }
        _ => false,
    }
}
