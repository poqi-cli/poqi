use crate::{
    match_score, CompletionContext, CompletionItem, CompletionKind, CompletionService, ContextHints,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpdateState {
    Keyword,
    TableChosen,
    SetKeyword,
    Assignments,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UpdateContext {
    state: UpdateState,
    has_value: bool,
}

pub(crate) fn suggest(
    service: &CompletionService,
    ctx: super::StatementContext<'_>,
    candidates: &mut Vec<CompletionItem>,
) -> bool {
    let update_ctx = detect_state(ctx);
    match ctx.completion_context {
        CompletionContext::Table { ref schema_hint } => {
            service.collect_table_context(ctx.prefix, schema_hint.as_deref(), candidates);
            true
        }
        CompletionContext::FromSource => {
            if matches!(update_ctx.state, UpdateState::TableChosen)
                && match_score(ctx.prefix, "SET").is_some()
            {
                candidates.push(CompletionItem {
                    label: "SET".to_string(),
                    insert_text: "SET".to_string(),
                    detail: "clause".to_string(),
                    kind: CompletionKind::Keyword,
                    score: 0,
                });
            }
            true
        }
        CompletionContext::UpdateSet => {
            collect_set(service, ctx.prefix, ctx.hints, update_ctx, candidates);
            true
        }
        CompletionContext::Literal => {
            // Inside WHERE predicates (e.g. `WHERE city = `) we only want literal
            // suggestions like NOW()/NULL/TRUE, not another WHERE or assignment
            // snippets. Detect a trailing ` where ` before the cursor and switch
            // to literal-only completions in that case.
            let slice = ctx
                .buffer
                .get(..ctx.cursor.min(ctx.buffer.len()))
                .unwrap_or("")
                .to_ascii_lowercase();
            if slice.contains(" where ") {
                CompletionService::collect_token_set(
                    candidates,
                    ctx.prefix,
                    crate::VALUE_KEYWORDS,
                    "literal",
                );
                return true;
            }
            if matches!(update_ctx.state, UpdateState::Assignments) {
                collect_set(service, ctx.prefix, ctx.hints, update_ctx, candidates);
                return true;
            }
            false
        }
        CompletionContext::Where => {
            service.collect_where(ctx.prefix, ctx.hints, candidates);
            true
        }
        CompletionContext::Returning => {
            service.collect_returning(ctx.prefix, ctx.hints, candidates);
            true
        }
        _ => false,
    }
}

fn detect_state(ctx: super::StatementContext<'_>) -> UpdateContext {
    let update_tokens = {
        let slice = super::tokens_since_keyword(ctx.scan.tokens.as_slice(), "update");
        if slice.is_empty() {
            ctx.scan.tokens.as_slice()
        } else {
            slice
        }
    };
    let has_table = ctx.hints.scan_table_hint.is_some() || update_tokens.len() >= 2;
    if !has_table {
        return UpdateContext {
            state: UpdateState::Keyword,
            has_value: false,
        };
    }
    let slice = ctx
        .buffer
        .get(..ctx.cursor.min(ctx.buffer.len()))
        .unwrap_or("")
        .to_ascii_lowercase();
    if !slice.contains(" set") {
        return UpdateContext {
            state: UpdateState::TableChosen,
            has_value: false,
        };
    }
    let set_tail = slice.split(" set ").nth(1).unwrap_or("");
    let has_assignment = set_tail
        .split_whitespace()
        .next()
        .is_some_and(|word| word.chars().any(|ch| ch.is_ascii_alphabetic() || ch == '_'));
    let has_value = set_tail
        .rsplit_once('=')
        .is_some_and(|(_, rhs)| !rhs.trim().is_empty());
    let state = if has_assignment {
        UpdateState::Assignments
    } else {
        UpdateState::SetKeyword
    };
    UpdateContext { state, has_value }
}

fn collect_set(
    service: &CompletionService,
    prefix: &str,
    hints: ContextHints<'_>,
    update_ctx: UpdateContext,
    candidates: &mut Vec<CompletionItem>,
) {
    let hint = CompletionService::merge_table_hint(hints, None);
    // Encourage direct assignments by surfacing snippets over raw columns when waiting for values.
    if !update_ctx.has_value {
        service.collect_predicate_snippets(candidates, prefix, hint);
        CompletionService::collect_token_set(candidates, prefix, crate::VALUE_KEYWORDS, "literal");
    }
    if matches!(update_ctx.state, UpdateState::Assignments) {
        candidates.push(CompletionItem {
            label: "WHERE".to_string(),
            insert_text: "WHERE".to_string(),
            detail: "clause".to_string(),
            kind: CompletionKind::Keyword,
            score: 0,
        });
    }
}
