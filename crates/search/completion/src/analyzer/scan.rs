use super::identifiers::{is_identifier_char, is_identifier_start};

#[derive(Debug, Default)]
pub(crate) struct SqlScan {
    /// Lowercased tokens collected up to the cursor (reset on semicolons).
    pub(crate) tokens: Vec<String>,
    /// Suppresses completions when the cursor is inside strings or comments.
    pub(crate) suppressed: bool,
}

pub(crate) fn scan_sql(buffer: &str, cursor: usize) -> SqlScan {
    if buffer.is_empty() {
        return SqlScan::default();
    }

    let mut upper_bound = cursor.min(buffer.len());
    while !buffer.is_char_boundary(upper_bound) {
        upper_bound -= 1;
    }
    let mut state = ScanState::new(upper_bound);
    let bytes = buffer.as_bytes();

    while state.idx < state.upper_bound {
        if state.skip_comment_or_quote(bytes, buffer) {
            continue;
        }
        if state.consume_token(buffer) {
            continue;
        }
        state.idx += buffer[state.idx..].chars().next().map_or(1, char::len_utf8);
    }

    let suppressed = state.is_suppressed();
    SqlScan {
        tokens: state.tokens,
        suppressed,
    }
}

// Lightweight lexer that tracks nesting so we can suppress completions inside strings/comments.
struct ScanState {
    tokens: Vec<String>,
    idx: usize,
    upper_bound: usize,
    in_single: bool,
    in_double: bool,
    in_line_comment: bool,
    block_depth: usize,
    dollar_delim: Option<String>,
}

impl ScanState {
    fn new(upper_bound: usize) -> Self {
        Self {
            tokens: Vec::new(),
            idx: 0,
            upper_bound,
            in_single: false,
            in_double: false,
            in_line_comment: false,
            block_depth: 0,
            dollar_delim: None,
        }
    }

    fn skip_comment_or_quote(&mut self, bytes: &[u8], buffer: &str) -> bool {
        if self.in_line_comment {
            if bytes[self.idx] as char == '\n' {
                self.in_line_comment = false;
            }
            self.idx += 1;
            return true;
        }
        if self.block_depth > 0 {
            self.consume_block_comment(bytes);
            return true;
        }
        if self.in_single {
            self.consume_single_quote(bytes);
            return true;
        }
        if self.in_double {
            self.consume_double_quote(bytes);
            return true;
        }
        if let Some(delim) = self.dollar_delim.as_ref() {
            if bytes[self.idx..self.upper_bound].starts_with(delim.as_bytes()) {
                self.idx += delim.len();
                self.dollar_delim = None;
            } else {
                self.idx += 1;
            }
            return true;
        }

        let ch = bytes[self.idx] as char;
        if ch == '-' && self.peek(bytes, 1) == Some('-') {
            self.in_line_comment = true;
            self.idx += 2;
            return true;
        }
        if ch == '/' && self.peek(bytes, 1) == Some('*') {
            self.block_depth += 1;
            self.idx += 2;
            return true;
        }
        if ch == '\'' {
            self.in_single = true;
            self.idx += 1;
            return true;
        }
        if ch == '"' {
            self.in_double = true;
            self.idx += 1;
            return true;
        }
        if ch == '$' {
            if let Some((delim, len)) = parse_dollar_delimiter(buffer, self.idx) {
                self.dollar_delim = Some(delim);
                self.idx += len;
                return true;
            }
        }
        false
    }

    fn consume_token(&mut self, buffer: &str) -> bool {
        let Some(ch) = buffer[self.idx..self.upper_bound].chars().next() else {
            return false;
        };
        if ch == ';' {
            self.tokens.clear();
            self.idx += ch.len_utf8();
            return true;
        }
        if ch.is_whitespace() {
            self.idx += ch.len_utf8();
            return true;
        }
        if is_identifier_start(ch) {
            let start = self.idx;
            self.idx += ch.len_utf8();
            while self.idx < self.upper_bound {
                let Some(c) = buffer[self.idx..self.upper_bound].chars().next() else {
                    break;
                };
                if !is_identifier_char(c) {
                    break;
                }
                self.idx += c.len_utf8();
            }
            self.tokens
                .push(buffer[start..self.idx].to_ascii_lowercase());
            return true;
        }
        false
    }

    fn consume_block_comment(&mut self, bytes: &[u8]) {
        if bytes[self.idx] == b'/' && self.peek(bytes, 1) == Some('*') {
            self.block_depth += 1;
            self.idx += 2;
            return;
        }
        if bytes[self.idx] == b'*' && self.peek(bytes, 1) == Some('/') {
            self.block_depth = self.block_depth.saturating_sub(1);
            self.idx += 2;
            return;
        }
        self.idx += 1;
    }

    fn consume_single_quote(&mut self, bytes: &[u8]) {
        if bytes[self.idx] == b'\'' {
            if self.peek(bytes, 1) == Some('\'') {
                self.idx += 2;
                return;
            }
            self.in_single = false;
        }
        self.idx += 1;
    }

    fn consume_double_quote(&mut self, bytes: &[u8]) {
        if bytes[self.idx] == b'"' {
            if self.peek(bytes, 1) == Some('"') {
                self.idx += 2;
                return;
            }
            self.in_double = false;
        }
        self.idx += 1;
    }

    fn peek(&self, bytes: &[u8], offset: usize) -> Option<char> {
        let idx = self.idx + offset;
        (idx < self.upper_bound).then(|| bytes[idx] as char)
    }

    fn is_suppressed(&self) -> bool {
        self.in_single
            || self.in_double
            || self.in_line_comment
            || self.block_depth > 0
            || self.dollar_delim.is_some()
    }
}

fn parse_dollar_delimiter(buffer: &str, start: usize) -> Option<(String, usize)> {
    let mut idx = start + 1;
    let bytes = buffer.as_bytes();
    while idx < buffer.len() {
        let ch = bytes[idx] as char;
        if ch == '$' {
            let delim = buffer[start..=idx].to_string();
            let len = delim.len();
            return Some((delim, len));
        }
        if !is_identifier_char(ch) {
            return None;
        }
        idx += 1;
    }
    None
}
