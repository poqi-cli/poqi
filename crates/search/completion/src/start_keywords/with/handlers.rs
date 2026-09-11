use super::state::{analyze_with_state, CteBodyKind, WithState};
use crate::start_keywords::shared::{keyword_item, push_comma_candidate, push_keyword_match};
use crate::{CompletionContext, CompletionItem, CompletionService};

pub(crate) fn suggest(
    service: &CompletionService,
    ctx: crate::start_keywords::StatementContext<'_>,
    candidates: &mut Vec<CompletionItem>,
) -> bool {
    let state = analyze_with_state(ctx);
    match state {
        WithState::ExpectName => handle_expect_name(),
        WithState::ColumnList => handle_column_list(ctx, candidates),
        WithState::ExpectAs => handle_expect_as(ctx, candidates),
        WithState::ExpectMaterialized { after_not } => {
            handle_expect_materialized(after_not, candidates)
        }
        WithState::ExpectOpenParen => handle_expect_open_paren(ctx, candidates),
        WithState::InsideQuery { body_kind } => {
            handle_inside_query(service, ctx, candidates, body_kind)
        }
        WithState::AfterQuery { has_statement } => {
            handle_after_query(ctx, candidates, has_statement)
        }
    }
}

fn handle_expect_name() -> bool {
    true
}

fn handle_column_list(
    ctx: crate::start_keywords::StatementContext<'_>,
    candidates: &mut Vec<CompletionItem>,
) -> bool {
    push_keyword_match(ctx.prefix, ")", "punctuation", candidates);
    true
}

fn handle_expect_as(
    ctx: crate::start_keywords::StatementContext<'_>,
    candidates: &mut Vec<CompletionItem>,
) -> bool {
    push_keyword_match(ctx.prefix, "AS", "keyword", candidates);
    true
}

fn handle_expect_materialized(after_not: bool, candidates: &mut Vec<CompletionItem>) -> bool {
    candidates.push(keyword_item("MATERIALIZED", "keyword"));
    if !after_not {
        candidates.push(keyword_item("NOT MATERIALIZED", "keyword"));
    }
    true
}

fn handle_expect_open_paren(
    ctx: crate::start_keywords::StatementContext<'_>,
    candidates: &mut Vec<CompletionItem>,
) -> bool {
    push_keyword_match(ctx.prefix, "(", "punctuation", candidates);
    if !ctx.prefix.is_empty() {
        push_keyword_match(ctx.prefix, "MATERIALIZED", "keyword", candidates);
        push_keyword_match(ctx.prefix, "NOT MATERIALIZED", "keyword", candidates);
    }
    true
}

fn handle_inside_query(
    service: &CompletionService,
    ctx: crate::start_keywords::StatementContext<'_>,
    candidates: &mut Vec<CompletionItem>,
    body_kind: Option<CteBodyKind>,
) -> bool {
    if matches!(ctx.completion_context, CompletionContext::Literal) {
        candidates.push(keyword_item(")", "punctuation"));
        return true;
    }
    let handled = match body_kind {
        Some(CteBodyKind::Insert) => {
            crate::start_keywords::insert_into::suggest(service, ctx, candidates)
        }
        Some(CteBodyKind::Update) => {
            crate::start_keywords::update::suggest(service, ctx, candidates)
        }
        Some(CteBodyKind::Delete) => {
            crate::start_keywords::delete_from::suggest(service, ctx, candidates)
        }
        _ => crate::start_keywords::select::suggest(service, ctx, candidates),
    };
    candidates.push(keyword_item(")", "punctuation"));
    handled
}

fn handle_after_query(
    ctx: crate::start_keywords::StatementContext<'_>,
    candidates: &mut Vec<CompletionItem>,
    has_statement: bool,
) -> bool {
    if has_statement {
        return false;
    }
    if !ctx.prefix.is_empty() && push_comma_candidate(ctx.prefix, candidates) {
        return true;
    }
    if ctx.prefix.is_empty() {
        CompletionService::collect_statement_keywords(ctx.prefix, candidates);
        return true;
    }
    false
}
