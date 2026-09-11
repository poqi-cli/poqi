use crate::scoring::match_score;

#[test]
fn match_score_respects_prefix() {
    assert_eq!(match_score("sel", "select"), Some(0));
    assert_eq!(match_score("ect", "select"), Some(10));
    assert_eq!(match_score("foo", "select"), None);
}
