use mini_elasticsearch::scorer::BM25Scorer;

// --- IDF tests ---

#[test]
fn test_idf_basic() {
    let scorer = BM25Scorer::new();
    // A term in 1 out of 10 docs → ln(10/1) = ln(10) ≈ 2.302
    let idf = scorer.idf(1, 10);
    assert!((idf - 10.0_f64.ln()).abs() < 1e-10);
}

#[test]
fn test_idf_all_docs() {
    let scorer = BM25Scorer::new();
    // A term in all 10 docs → ln(10/10) = ln(1) = 0.0
    let idf = scorer.idf(10, 10);
    assert!((idf - 0.0).abs() < 1e-10);
}

#[test]
fn test_idf_half_docs() {
    let scorer = BM25Scorer::new();
    // A term in 5 out of 10 docs → ln(10/5) = ln(2) ≈ 0.693
    let idf = scorer.idf(5, 10);
    assert!((idf - 2.0_f64.ln()).abs() < 1e-10);
}

#[test]
fn test_idf_zero_doc_freq() {
    let scorer = BM25Scorer::new();
    let idf = scorer.idf(0, 10);
    assert_eq!(idf, 0.0);
}

#[test]
fn test_idf_zero_total_docs() {
    let scorer = BM25Scorer::new();
    let idf = scorer.idf(1, 0);
    assert_eq!(idf, 0.0);
}

#[test]
fn test_idf_rare_term_higher_than_common() {
    let scorer = BM25Scorer::new();
    let rare = scorer.idf(1, 100);    // appears in 1 of 100
    let common = scorer.idf(50, 100); // appears in 50 of 100
    assert!(rare > common, "Rare term IDF ({}) should be > common term IDF ({})", rare, common);
}

// --- BM25 score tests ---

#[test]
fn test_bm25_score_positive() {
    let scorer = BM25Scorer::new();
    let idf = scorer.idf(1, 10);
    let score = scorer.score(1, 5, 5.0, idf);
    assert!(score > 0.0);
}

#[test]
fn test_bm25_higher_tf_higher_score() {
    let scorer = BM25Scorer::new();
    let idf = scorer.idf(2, 10);
    let score_tf1 = scorer.score(1, 10, 10.0, idf);
    let score_tf5 = scorer.score(5, 10, 10.0, idf);
    assert!(score_tf5 > score_tf1, "Higher TF should give higher score");
}

#[test]
fn test_bm25_tf_saturation() {
    // BM25 saturates: going from tf=100 to tf=200 should have less impact than tf=1 to tf=2
    let scorer = BM25Scorer::new();
    let idf = scorer.idf(1, 10);

    let score_1 = scorer.score(1, 100, 100.0, idf);
    let score_2 = scorer.score(2, 100, 100.0, idf);
    let score_100 = scorer.score(100, 100, 100.0, idf);
    let score_200 = scorer.score(200, 100, 100.0, idf);

    let delta_low = score_2 - score_1;
    let delta_high = score_200 - score_100;
    assert!(delta_low > delta_high, "TF should saturate: first increment ({}) > later increment ({})", delta_low, delta_high);
}

#[test]
fn test_bm25_shorter_doc_higher_score() {
    // With the same TF, a shorter document should score higher (length normalization)
    let scorer = BM25Scorer::new();
    let idf = scorer.idf(2, 10);
    let short = scorer.score(1, 5, 10.0, idf);   // doc is half avg length
    let long = scorer.score(1, 20, 10.0, idf);    // doc is double avg length
    assert!(short > long, "Shorter doc should score higher: {} > {}", short, long);
}

#[test]
fn test_bm25_avg_length_doc() {
    // A document exactly at average length with tf=1 should produce a predictable score
    let scorer = BM25Scorer::new();
    let idf = scorer.idf(1, 10); // ln(10)
    // When dl = avgdl, the denominator simplifies:
    // tf + k1 * (1 - b + b * 1) = tf + k1 = 1 + 1.2 = 2.2
    // numerator = tf * (k1 + 1) = 1 * 2.2 = 2.2
    // score = idf * 2.2 / 2.2 = idf * 1.0
    let score = scorer.score(1, 10, 10.0, idf);
    assert!((score - idf).abs() < 1e-10, "At avg length with tf=1, score should equal IDF");
}

#[test]
fn test_bm25_custom_params() {
    let default_scorer = BM25Scorer::new();
    let custom_scorer = BM25Scorer::with_params(2.0, 0.5);

    let idf = default_scorer.idf(1, 10);
    // Use tf > 1 and dl ≠ avgdl so k1 and b both matter
    let default_score = default_scorer.score(3, 5, 10.0, idf);
    let custom_score = custom_scorer.score(3, 5, 10.0, idf);

    // Different parameters should produce different scores
    assert!(
        (default_score - custom_score).abs() > 1e-10,
        "Different parameters should produce different scores: {} vs {}",
        default_score,
        custom_score
    );
}

#[test]
fn test_bm25_zero_idf_zero_score() {
    let scorer = BM25Scorer::new();
    let score = scorer.score(5, 10, 10.0, 0.0);
    assert_eq!(score, 0.0, "Zero IDF should produce zero score");
}
