use super::fixtures::completion_service;

#[test]
fn unicode_identifier_does_not_break_completion_scanning() {
    let service = completion_service();
    let sql = "SELECT ä FROM widgets";

    for cursor in (0..=sql.len()).filter(|cursor| sql.is_char_boundary(*cursor)) {
        let _ = service.suggest(sql, cursor);
    }
}

#[test]
fn dollar_quotes_suppress_completion_until_the_closing_delimiter() {
    let service = completion_service();
    for literal in ["$$ä日本語🙂$$", "$tag$ä日本語🙂$tag$"] {
        let sql = format!("SELECT {literal} FROM widgets WHERE ");
        let start = sql.find('ä').expect("literal starts here");
        let end = sql.find(" FROM").expect("literal ends here");
        for cursor in start..end {
            if sql.is_char_boundary(cursor) {
                assert!(service.suggest(&sql, cursor).items.is_empty());
            }
        }
        assert!(!service.suggest(&sql, sql.len()).items.is_empty());
    }
}

#[test]
fn with_clause_lexer_resumes_after_unicode_dollar_quoted_body() {
    let service = completion_service();
    for body in ["$$ä日本語🙂$$", "$tag$ä日本語🙂$tag$"] {
        let sql = format!("WITH c AS (SELECT {body}) SELECT ");
        let batch = service.suggest(&sql, sql.len());
        assert!(!batch.items.is_empty());
    }
}
