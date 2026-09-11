use super::*;

#[test]
fn search_returns_empty_results_for_stub() {
    let index = FuzzyIndex::new();
    let results = index.search("orders", 5).expect("search should succeed");
    assert!(results.is_empty());
}
