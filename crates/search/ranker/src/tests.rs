use super::*;

#[test]
fn rank_returns_empty_results_for_stub() {
    let ranker = HybridRanker::new();
    let results = ranker.rank("orders");
    assert!(results.is_empty());
}
