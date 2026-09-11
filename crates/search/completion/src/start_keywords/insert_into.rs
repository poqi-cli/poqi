use poqi_catalog::{display_identifier, quote_identifier};

use crate::{
    match_score, CompletionContext, CompletionItem, CompletionKind, CompletionService, ContextHints,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InsertState {
    AfterKeyword,
    AfterTable,
    ColumnList { has_prior_columns: bool },
    AfterColumns,
    Values,
}

pub(crate) fn suggest(
    service: &CompletionService,
    ctx: super::StatementContext<'_>,
    candidates: &mut Vec<CompletionItem>,
) -> bool {
    let state = detect_state(ctx);
    match ctx.completion_context {
        CompletionContext::Table { ref schema_hint } => {
            service.collect_table_context(ctx.prefix, schema_hint.as_deref(), candidates);
            true
        }
        CompletionContext::FromSource => {
            match state {
                InsertState::ColumnList { has_prior_columns } => {
                    collect_insert_columns(
                        service,
                        ctx.prefix,
                        ctx.hints,
                        has_prior_columns,
                        candidates,
                    );
                }
                InsertState::AfterColumns => {
                    if match_score(ctx.prefix, "VALUES").is_some() {
                        candidates.push(CompletionItem {
                            label: "VALUES".to_string(),
                            insert_text: "VALUES".to_string(),
                            detail: "clause".to_string(),
                            kind: CompletionKind::Keyword,
                            score: 0,
                        });
                    }
                }
                InsertState::AfterKeyword | InsertState::AfterTable | InsertState::Values => {}
            }
            true
        }
        CompletionContext::InsertValues => {
            collect_values(ctx, candidates);
            true
        }
        CompletionContext::Returning => {
            service.collect_returning(ctx.prefix, ctx.hints, candidates);
            true
        }
        _ => false,
    }
}

fn detect_state(ctx: super::StatementContext<'_>) -> InsertState {
    if matches!(ctx.completion_context, CompletionContext::InsertValues) {
        return InsertState::Values;
    }
    let insert_tokens = {
        let slice = super::tokens_since_keyword(ctx.scan.tokens.as_slice(), "insert");
        if slice.is_empty() {
            ctx.scan.tokens.as_slice()
        } else {
            slice
        }
    };
    let into_idx = insert_tokens.iter().position(|token| token == "into");
    let has_table = ctx.hints.scan_table_hint.is_some()
        || into_idx.is_some_and(|idx| insert_tokens.len() > idx + 1);
    if !has_table {
        return InsertState::AfterKeyword;
    }
    let slice = ctx
        .buffer
        .get(..ctx.cursor.min(ctx.buffer.len()))
        .unwrap_or("")
        .to_ascii_lowercase();
    let open_paren = slice.contains('(');
    let close_paren = slice.contains(')');
    if open_paren && !close_paren {
        let has_prior_columns = slice
            .split('(')
            .nth(1)
            .and_then(|rest| rest.split(')').next())
            .is_some_and(|segment| segment.chars().any(|ch| ch.is_ascii_alphabetic()));
        return InsertState::ColumnList { has_prior_columns };
    }
    if open_paren && close_paren {
        return InsertState::AfterColumns;
    }
    InsertState::AfterTable
}

fn collect_insert_columns(
    service: &CompletionService,
    prefix: &str,
    hints: ContextHints<'_>,
    _has_prior_columns: bool,
    candidates: &mut Vec<CompletionItem>,
) {
    let table_hint = CompletionService::merge_table_hint(hints, None);
    let columns = service.columns_for_hint(table_hint);
    for column in columns {
        if let Some(score) = match_score(prefix, &column.name_lower) {
            let table_detail = service.table_display_for_column(column);
            candidates.push(CompletionItem {
                label: display_identifier(&column.name),
                insert_text: quote_identifier(&column.name),
                detail: table_detail,
                kind: CompletionKind::Column,
                score: score.saturating_sub(1),
            });
        }
    }
}

fn collect_values(ctx: super::StatementContext<'_>, candidates: &mut Vec<CompletionItem>) {
    let slice = ctx
        .buffer
        .get(..ctx.cursor.min(ctx.buffer.len()))
        .unwrap_or("");
    let lower = slice.to_ascii_lowercase();
    let (header, tail) = if let Some(idx) = lower.rfind("values") {
        (&slice[..idx], &slice[idx + "values".len()..])
    } else {
        (slice, slice)
    };

    let column_count = header
        .split_once('(')
        .and_then(|(_, rest)| rest.split(')').next())
        .map_or(0_usize, |segment| {
            segment
                .split(',')
                .filter(|part| {
                    part.chars()
                        .any(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                })
                .count()
        });

    let has_open_paren = tail.contains('(');
    let has_close_paren = tail.contains(')');

    if !ctx.hints.token_is_empty {
        return;
    }

    if !has_open_paren {
        candidates.push(CompletionItem {
            label: "(".to_string(),
            insert_text: "(".to_string(),
            detail: "ddl".to_string(),
            kind: CompletionKind::Keyword,
            score: 0,
        });
        return;
    }

    if !has_close_paren {
        let values_body = tail
            .split_once('(')
            .map_or("", |(_, rest)| rest.split(')').next().unwrap_or(rest));
        let value_count = values_body
            .split(',')
            .filter(|part| {
                part.chars()
                    .any(|ch| !ch.is_whitespace() && ch != ',' && ch != '(' && ch != ')')
            })
            .count();
        if column_count == 0 || value_count < column_count {
            return;
        }
        candidates.push(CompletionItem {
            label: ");".to_string(),
            insert_text: ");".to_string(),
            detail: "ddl".to_string(),
            kind: CompletionKind::Keyword,
            score: 0,
        });
    }
}
