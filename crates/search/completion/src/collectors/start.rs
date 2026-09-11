use super::token_sets::collect_token_set;
use crate::scoring::match_score;
use crate::start_keywords::StatementContext;
use crate::tokens::{START_DDL_KEYWORDS, START_KEYWORDS};
use crate::types::{CompletionItem, CompletionKind};

pub(crate) fn collect_start_keywords(
    statement_context: StatementContext<'_>,
    candidates: &mut Vec<CompletionItem>,
) {
    collect_statement_keywords(statement_context.prefix, candidates);
    collect_start_ddl_keywords(statement_context, candidates);
}

pub(crate) fn collect_statement_keywords(prefix: &str, candidates: &mut Vec<CompletionItem>) {
    collect_token_set(candidates, prefix, START_KEYWORDS, "statement");
}

pub(crate) fn collect_start_ddl_keywords(
    statement_context: StatementContext<'_>,
    candidates: &mut Vec<CompletionItem>,
) {
    const DDL_SCORE_BIAS: u32 = 1_000;
    let mut items: Vec<CompletionItem> = START_DDL_KEYWORDS
        .iter()
        .filter_map(|keyword| {
            match_score(statement_context.prefix, keyword).map(|score| {
                let insert_text = ddl_insert_text(statement_context, keyword);
                CompletionItem {
                    label: (*keyword).to_owned(),
                    insert_text,
                    detail: "ddl".to_string(),
                    kind: CompletionKind::Keyword,
                    score: score.saturating_add(DDL_SCORE_BIAS),
                }
            })
        })
        .collect();
    candidates.append(&mut items);
}

fn ddl_insert_text(statement_context: StatementContext<'_>, keyword: &str) -> String {
    let Some((first_word, remainder)) = keyword.split_once(' ') else {
        return keyword.to_string();
    };
    if start_keyword_prefix_typed(statement_context, first_word) {
        return remainder.to_string();
    }
    keyword.to_string()
}

fn start_keyword_prefix_typed(statement_context: StatementContext<'_>, first_word: &str) -> bool {
    statement_context
        .buffer
        .get(..statement_context.token_start)
        .and_then(|head| head.split_whitespace().last())
        .is_some_and(|word| word.eq_ignore_ascii_case(first_word))
}
