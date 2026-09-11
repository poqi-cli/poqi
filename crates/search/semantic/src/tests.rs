use super::{
    cosine_similarity, finalize_ranking, format_query_prompt, format_row_prompt, l2_normalize,
    RankedRow, SearchOptions,
};

#[test]
fn query_text_has_no_model_specific_instruction() {
    let prompt = format_query_prompt("find invoices");
    assert_eq!(prompt, "find invoices");
    assert_eq!(format_query_prompt("  etsi laskut  "), "etsi laskut");
}

#[test]
fn normalize_handles_zero_vector() {
    let mut values = vec![0.0, 0.0, 0.0];
    l2_normalize(&mut values);
    assert_eq!(values, vec![0.0, 0.0, 0.0]);

    let mut values = vec![3.0, 4.0];
    l2_normalize(&mut values);
    let norm = (values[0].powi(2) + values[1].powi(2)).sqrt();
    assert!((norm - 1.0).abs() < 1e-6);
}

#[test]
fn cosine_similarity_matches_dot_product_for_normalized_vectors() {
    let a = vec![0.0, 1.0];
    let b = vec![0.0, 1.0];
    assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);
}

#[test]
fn finalize_ranking_returns_empty_when_no_score_meets_threshold() {
    let rows = vec![
        RankedRow {
            index: 0,
            score: 0.9,
        },
        RankedRow {
            index: 1,
            score: 0.2,
        },
    ];
    let options = SearchOptions {
        batch_size: 4,
        top_k: 1,
        threshold: Some(0.95),
        dim: 768,
    };
    let ranked = finalize_ranking(rows, options);
    assert!(ranked.is_empty());
}

#[test]
fn finalize_ranking_caps_thresholded_rows_at_top_k() {
    let rows = vec![
        RankedRow {
            index: 0,
            score: 0.9,
        },
        RankedRow {
            index: 1,
            score: 0.8,
        },
        RankedRow {
            index: 2,
            score: 0.7,
        },
    ];
    let options = SearchOptions {
        batch_size: 4,
        top_k: 2,
        threshold: Some(0.5),
        dim: 768,
    };
    let ranked = finalize_ranking(rows, options);
    assert_eq!(ranked.len(), 2);
    assert_eq!(ranked[0].index, 0);
    assert_eq!(ranked[1].index, 1);
}

#[test]
fn row_prompt_includes_null_sentinel_and_clips() {
    let long_value = "x".repeat(600);
    let columns = vec![
        ("id", "123", false),
        ("empty", "", false),
        ("literal_null", "NULL", false),
        ("nullish", "NULL", true),
        ("whitespace", "   ", false),
        ("notes", long_value.as_str(), false),
    ];
    let prompt = format_row_prompt(Some("Invoice #42"), &columns);
    assert!(prompt.starts_with("Invoice #42 | id: 123"));
    assert!(!prompt.contains("task:"));
    assert!(!prompt.contains("query:"));
    assert!(prompt.contains("id: 123"));
    assert!(prompt.contains("empty: "));
    assert!(prompt.contains("literal_null: NULL"));
    assert!(prompt.contains("nullish: [NULL]"));
    assert!(prompt.contains("whitespace: "));
    assert!(prompt.contains("notes: x"));
    assert!(prompt.len() < 700);
}
