use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxKind {
    Keyword,
    Identifier,
    Literal,
    Number,
    Comment,
    Operator,
    Whitespace,
}

#[derive(Debug, Clone)]
pub struct SyntaxSpan {
    pub kind: SyntaxKind,
    pub text: String,
}

#[derive(Debug)]
pub struct SyntaxHighlighter {
    keywords: HashSet<String>,
}

impl SyntaxHighlighter {
    #[must_use]
    pub fn new() -> Self {
        let keywords = syntax_keywords()
            .iter()
            .map(|kw| kw.to_ascii_uppercase())
            .collect();
        Self { keywords }
    }

    #[must_use]
    pub fn highlight(&self, text: &str) -> Vec<SyntaxSpan> {
        if text.is_empty() {
            return Vec::new();
        }
        let mut spans = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let mut idx = 0;
        while idx < chars.len() {
            let ch = chars[idx];
            if ch == '-' && idx + 1 < chars.len() && chars[idx + 1] == '-' {
                let comment: String = chars[idx..].iter().collect();
                spans.push(SyntaxSpan {
                    kind: SyntaxKind::Comment,
                    text: comment,
                });
                break;
            }
            if ch == '\'' {
                let start = idx;
                idx += 1;
                while idx < chars.len() {
                    let next = chars[idx];
                    idx += 1;
                    if next == '\'' {
                        break;
                    }
                }
                let literal: String = chars[start..idx].iter().collect();
                spans.push(SyntaxSpan {
                    kind: SyntaxKind::Literal,
                    text: literal,
                });
                continue;
            }
            if ch == '"' {
                let start = idx;
                idx += 1;
                while idx < chars.len() {
                    let next = chars[idx];
                    idx += 1;
                    if next == '"' {
                        break;
                    }
                }
                let quoted: String = chars[start..idx].iter().collect();
                spans.push(SyntaxSpan {
                    kind: SyntaxKind::Identifier,
                    text: quoted,
                });
                continue;
            }
            if ch.is_ascii_digit() {
                let start = idx;
                idx += 1;
                while idx < chars.len() && chars[idx].is_ascii_digit() {
                    idx += 1;
                }
                let number: String = chars[start..idx].iter().collect();
                spans.push(SyntaxSpan {
                    kind: SyntaxKind::Number,
                    text: number,
                });
                continue;
            }
            if is_identifier_char(ch) {
                let start = idx;
                idx += 1;
                while idx < chars.len() && is_identifier_char(chars[idx]) {
                    idx += 1;
                }
                let token: String = chars[start..idx].iter().collect();
                let upper = token.to_ascii_uppercase();
                let kind = if self.keywords.contains(&upper) {
                    SyntaxKind::Keyword
                } else {
                    SyntaxKind::Identifier
                };
                spans.push(SyntaxSpan { kind, text: token });
                continue;
            }
            if ch.is_whitespace() {
                let start = idx;
                idx += 1;
                while idx < chars.len() && chars[idx].is_whitespace() {
                    idx += 1;
                }
                let whitespace: String = chars[start..idx].iter().collect();
                spans.push(SyntaxSpan {
                    kind: SyntaxKind::Whitespace,
                    text: whitespace,
                });
                continue;
            }
            spans.push(SyntaxSpan {
                kind: SyntaxKind::Operator,
                text: ch.to_string(),
            });
            idx += 1;
        }
        spans
    }
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '$'
}

const fn syntax_keywords() -> &'static [&'static str] {
    poqi_search_completion::KEYWORDS
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}
