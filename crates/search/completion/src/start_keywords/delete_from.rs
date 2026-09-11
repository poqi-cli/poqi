use crate::{match_score, CompletionContext, CompletionItem, CompletionKind, CompletionService};

pub(crate) fn suggest(
    service: &CompletionService,
    ctx: super::StatementContext<'_>,
    candidates: &mut Vec<CompletionItem>,
) -> bool {
    match ctx.completion_context {
        CompletionContext::Table { ref schema_hint } => {
            service.collect_table_context(ctx.prefix, schema_hint.as_deref(), candidates);
            true
        }
        CompletionContext::FromSource => {
            if has_table(ctx) && match_score(ctx.prefix, "WHERE").is_some() {
                candidates.push(CompletionItem {
                    label: "WHERE".to_string(),
                    insert_text: "WHERE".to_string(),
                    detail: "clause".to_string(),
                    kind: CompletionKind::Keyword,
                    score: 0,
                });
            }
            true
        }
        CompletionContext::Where => {
            service.collect_where(ctx.prefix, ctx.hints, candidates);
            true
        }
        _ => false,
    }
}

fn has_table(ctx: super::StatementContext<'_>) -> bool {
    if ctx.hints.scan_table_hint.is_some() {
        return true;
    }
    let delete_tokens = {
        let slice = super::tokens_since_keyword(ctx.scan.tokens.as_slice(), "delete");
        if slice.is_empty() {
            ctx.scan.tokens.as_slice()
        } else {
            slice
        }
    };
    delete_tokens
        .iter()
        .position(|token| token == "from")
        .is_some_and(|idx| delete_tokens.len() > idx + 1)
}
