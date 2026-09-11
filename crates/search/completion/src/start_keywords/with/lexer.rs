#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WithToken<'a> {
    Word(&'a str),
    OpenParen,
    CloseParen,
    Comma,
}

impl<'a> WithToken<'a> {
    pub(crate) fn word(self) -> Option<&'a str> {
        if let Self::Word(word) = self {
            Some(word)
        } else {
            None
        }
    }

    pub(crate) fn is_word(self) -> bool {
        matches!(self, Self::Word(_))
    }

    pub(crate) fn eq_word(self, keyword: &str) -> bool {
        self.word()
            .is_some_and(|word| word.eq_ignore_ascii_case(keyword))
    }

    pub(crate) fn starts_with(self, keyword: &str) -> bool {
        self.word().is_some_and(|word| {
            let lower = word.to_ascii_lowercase();
            keyword.starts_with(&lower) && lower.len() < keyword.len()
        })
    }
}

pub(crate) struct WithLexer<'a> {
    input: &'a str,
    bytes: &'a [u8],
    idx: usize,
}

impl<'a> WithLexer<'a> {
    pub(crate) fn new(input: &'a str) -> Self {
        Self {
            input,
            bytes: input.as_bytes(),
            idx: 0,
        }
    }

    fn next_token(&mut self) -> Option<WithToken<'a>> {
        while self.idx < self.bytes.len() {
            let ch = self.bytes[self.idx] as char;
            match ch {
                '(' => {
                    self.idx += 1;
                    return Some(WithToken::OpenParen);
                }
                ')' => {
                    self.idx += 1;
                    return Some(WithToken::CloseParen);
                }
                ',' => {
                    self.idx += 1;
                    return Some(WithToken::Comma);
                }
                '-' if self.peek_char(1) == Some('-') => {
                    self.skip_line_comment();
                }
                '/' if self.peek_char(1) == Some('*') => {
                    self.skip_block_comment();
                }
                '\'' => {
                    self.skip_single_quote();
                }
                '$' if self.skip_dollar_quote() => {}
                '$' => {
                    self.idx += 1;
                }
                '"' => {
                    return Some(self.consume_quoted_identifier());
                }
                _ => {
                    if ch.is_ascii_whitespace() {
                        self.idx += 1;
                        continue;
                    }
                    if is_identifier_start(ch) {
                        return Some(self.consume_word());
                    }
                    self.idx += 1;
                }
            }
        }
        None
    }

    fn consume_word(&mut self) -> WithToken<'a> {
        let start = self.idx;
        self.idx += 1;
        while self.idx < self.bytes.len() {
            let ch = self.bytes[self.idx] as char;
            if !is_identifier_char(ch) {
                break;
            }
            self.idx += 1;
        }
        WithToken::Word(&self.input[start..self.idx])
    }

    fn consume_quoted_identifier(&mut self) -> WithToken<'a> {
        let start = self.idx;
        self.idx += 1;
        while self.idx < self.bytes.len() {
            if self.bytes[self.idx] == b'"' {
                self.idx += 1;
                if self.idx < self.bytes.len() && self.bytes[self.idx] == b'"' {
                    self.idx += 1;
                    continue;
                }
                break;
            }
            self.idx += 1;
        }
        WithToken::Word(&self.input[start..self.idx])
    }

    fn skip_line_comment(&mut self) {
        self.idx += 2;
        while self.idx < self.bytes.len() {
            if self.bytes[self.idx] as char == '\n' {
                self.idx += 1;
                break;
            }
            self.idx += 1;
        }
    }

    fn skip_block_comment(&mut self) {
        self.idx += 2;
        let mut depth = 1;
        while self.idx < self.bytes.len() && depth > 0 {
            if self.peek_char(0) == Some('/') && self.peek_char(1) == Some('*') {
                depth += 1;
                self.idx += 2;
                continue;
            }
            if self.peek_char(0) == Some('*') && self.peek_char(1) == Some('/') {
                depth -= 1;
                self.idx += 2;
                continue;
            }
            self.idx += 1;
        }
    }

    fn skip_single_quote(&mut self) {
        self.idx += 1;
        while self.idx < self.bytes.len() {
            let ch = self.bytes[self.idx] as char;
            self.idx += 1;
            if ch == '\'' {
                if self.idx < self.bytes.len() && self.bytes[self.idx] as char == '\'' {
                    self.idx += 1;
                    continue;
                }
                break;
            }
        }
    }

    fn skip_dollar_quote(&mut self) -> bool {
        if let Some((delim, len)) = parse_dollar_delimiter(self.input, self.idx) {
            self.idx += len;
            while self.idx < self.input.len() {
                if self.bytes[self.idx..].starts_with(delim.as_bytes()) {
                    self.idx += len;
                    break;
                }
                self.idx += 1;
            }
            return true;
        }
        false
    }

    fn peek_char(&self, offset: usize) -> Option<char> {
        self.bytes.get(self.idx + offset).map(|byte| *byte as char)
    }
}

impl<'a> Iterator for WithLexer<'a> {
    type Item = WithToken<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_token()
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

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '$'
}

fn is_identifier_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}
