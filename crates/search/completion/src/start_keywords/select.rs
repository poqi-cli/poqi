use crate::{
    analyzer::QueryScope, is_table_identifier, match_score, CompletionContext, CompletionItem,
    CompletionKind, CompletionService, ContextHints, DATA_SOURCE_TOKENS,
};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SelectListState {
    pub(crate) has_items: bool,
    pub(crate) has_star: bool,
}

pub(crate) fn suggest(
    service: &CompletionService,
    ctx: super::StatementContext<'_>,
    candidates: &mut Vec<CompletionItem>,
) -> bool {
    match ctx.completion_context {
        CompletionContext::SelectList => {
            collect_select_list(service, ctx.prefix, ctx.hints, ctx, candidates);
            true
        }
        CompletionContext::FromSource => {
            collect_from_source(service, ctx.prefix, ctx.hints, candidates);
            true
        }
        CompletionContext::JoinType => {
            CompletionService::collect_token_set(candidates, ctx.prefix, crate::JOIN_TYPES, "join");
            true
        }
        CompletionContext::JoinCondition => {
            service.collect_join_condition(ctx.prefix, ctx.hints, candidates);
            true
        }
        CompletionContext::Where => {
            service.collect_where(ctx.prefix, ctx.hints, candidates);
            true
        }
        CompletionContext::GroupBy => {
            service.collect_group_by(ctx.prefix, ctx.hints, candidates);
            true
        }
        CompletionContext::Having => {
            service.collect_having(ctx.prefix, ctx.hints, candidates);
            true
        }
        CompletionContext::Window => {
            service.collect_window(ctx.prefix, ctx.hints, candidates);
            true
        }
        CompletionContext::OrderBy => {
            service.collect_order_by(ctx.prefix, ctx.hints, candidates);
            true
        }
        CompletionContext::Returning => {
            service.collect_returning(ctx.prefix, ctx.hints, candidates);
            true
        }
        CompletionContext::Limit | CompletionContext::Offset | CompletionContext::Literal => true,
        _ => false,
    }
}

fn collect_select_list(
    service: &CompletionService,
    prefix: &str,
    hints: ContextHints<'_>,
    ctx: super::StatementContext<'_>,
    candidates: &mut Vec<CompletionItem>,
) {
    let select_list = select_list_state(&ctx.scan.tokens, ctx.scope);
    if !select_list.has_star {
        CompletionService::collect_token_set(
            candidates,
            prefix,
            crate::SELECT_LIST_TOKENS,
            "select",
        );
    }
    if !select_list.has_star {
        CompletionService::collect_token_set(candidates, prefix, crate::AGG_FUNCS, "aggregate");
    }
    if select_list.has_items && match_score(prefix, "FROM").is_some() {
        candidates.push(CompletionItem {
            label: "FROM".to_string(),
            insert_text: "FROM".to_string(),
            detail: "clause".to_string(),
            kind: CompletionKind::Keyword,
            score: 0,
        });
    }
    if !select_list.has_star {
        if hints.scope.has_relation_source {
            service.collect_columns_in_scope(candidates, prefix, hints.scope);
        } else {
            let hint = CompletionService::merge_table_hint(hints, None);
            service.collect_columns(candidates, prefix, hint);
        }
    }
}

fn collect_from_source(
    service: &CompletionService,
    prefix: &str,
    hints: ContextHints<'_>,
    candidates: &mut Vec<CompletionItem>,
) {
    let completed_source =
        hints.last_token.is_some_and(is_table_identifier) || !hints.scope.relations.is_empty();
    let show_sources = !(hints.token_is_empty && prefix.is_empty() && completed_source);
    if show_sources {
        service.collect_tables(candidates, prefix, None, true);
        CompletionService::collect_token_set(candidates, prefix, DATA_SOURCE_TOKENS, "source");
    }
    CompletionService::collect_token_set(candidates, prefix, crate::CLAUSE_TOKENS, "clause");
}

pub(crate) fn select_list_state(tokens: &[String], scope: &QueryScope) -> SelectListState {
    let mut in_select = false;
    let mut has_ident = false;
    for token in tokens {
        if token == "select" {
            in_select = true;
            continue;
        }
        if !in_select {
            continue;
        }
        if matches!(
            token.as_str(),
            "from" | "where" | "group" | "order" | "limit" | "offset" | "fetch" | "join"
        ) {
            break;
        }
        if token != "distinct" {
            has_ident = true;
            break;
        }
    }

    let has_star = scope.select_list_has_star;

    SelectListState {
        has_items: has_ident || has_star,
        has_star,
    }
}
