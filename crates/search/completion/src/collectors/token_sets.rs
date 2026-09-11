use crate::scoring::{adjusted_token_score, match_score};
use crate::tokens::KEYWORDS;
use crate::types::{CompletionItem, CompletionKind};

pub(crate) fn collect_keywords(items: &mut Vec<CompletionItem>, prefix: &str) {
    for &keyword in KEYWORDS {
        if let Some(score) = match_score(prefix, keyword) {
            items.push(CompletionItem {
                label: keyword.to_string(),
                insert_text: keyword.to_string(),
                detail: "keyword".to_string(),
                kind: CompletionKind::Keyword,
                score,
            });
        }
    }
}

pub(crate) fn collect_token_set(
    items: &mut Vec<CompletionItem>,
    prefix: &str,
    tokens: &[&str],
    detail: &str,
) {
    for &token in tokens {
        if detail == "select" && token == "*" {
            if match_score(prefix, token).is_some() {
                items.push(CompletionItem {
                    label: token.to_string(),
                    insert_text: token.to_string(),
                    detail: detail.to_string(),
                    kind: CompletionKind::Keyword,
                    score: 0,
                });
            }
            continue;
        }
        if let Some(score) = match_score(prefix, token) {
            let score = adjusted_token_score(detail, token, score);
            items.push(CompletionItem {
                label: token.to_string(),
                insert_text: token.to_string(),
                detail: detail.to_string(),
                kind: CompletionKind::Keyword,
                score,
            });
        }
    }
}
