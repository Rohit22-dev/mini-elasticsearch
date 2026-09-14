use mini_elasticsearch::engine::SearchEngine;

fn build_test_engine() -> SearchEngine {
    let mut engine = SearchEngine::new();
    engine.add_document("1", "Ogre Story", "The ogre kidnapped the bride");
    engine.add_document("2", "Dragon Tale", "The dragon burned the village");
    engine.add_document("3", "Escape Story", "The bride escaped from the ogre");
    engine
}

// --- Basic search behavior ---

#[test]
fn test_search_single_term_returns_correct_docs() {
    let engine = build_test_engine();
    let results = engine.search("ogre");
    let doc_ids: Vec<&str> = results.iter().map(|r| r.doc_id.as_str()).collect();
    assert!(doc_ids.contains(&"1"));
    assert!(doc_ids.contains(&"3"));
    assert_eq!(doc_ids.len(), 2);
}

#[test]
fn test_search_results_have_positive_scores() {
    let engine = build_test_engine();
    let results = engine.search("ogre");
    for result in &results {
        assert!(result.score > 0.0, "Score should be positive, got {}", result.score);
    }
}

#[test]
fn test_search_results_sorted_by_score_descending() {
    let engine = build_test_engine();
    let results = engine.search("ogre");
    for i in 1..results.len() {
        assert!(
            results[i - 1].score >= results[i].score,
            "Results should be sorted descending: {} >= {}",
            results[i - 1].score,
            results[i].score
        );
    }
}

#[test]
fn test_search_results_include_titles() {
    let engine = build_test_engine();
    let results = engine.search("ogre");
    for result in &results {
        assert!(!result.title.is_empty(), "Title should not be empty");
    }
    // Check specific titles
    let titles: Vec<&str> = results.iter().map(|r| r.title.as_str()).collect();
    assert!(titles.contains(&"Ogre Story") || titles.contains(&"Escape Story"));
}

// --- AND search ---

#[test]
fn test_and_search_multi_term() {
    let engine = build_test_engine();
    let results = engine.search("ogre bride");
    let doc_ids: Vec<&str> = results.iter().map(|r| r.doc_id.as_str()).collect();
    assert!(doc_ids.contains(&"1"));
    assert!(doc_ids.contains(&"3"));
    assert_eq!(doc_ids.len(), 2);
}

#[test]
fn test_and_search_no_match() {
    let engine = build_test_engine();
    let results = engine.search("ogre dragon");
    assert!(results.is_empty());
}

// --- OR search ---

#[test]
fn test_or_search() {
    let engine = build_test_engine();
    let results = engine.search("ogre OR dragon");
    let doc_ids: Vec<&str> = results.iter().map(|r| r.doc_id.as_str()).collect();
    assert!(doc_ids.contains(&"1"));
    assert!(doc_ids.contains(&"2"));
    assert!(doc_ids.contains(&"3"));
    assert_eq!(doc_ids.len(), 3);
}

#[test]
fn test_or_search_with_nonexistent_term() {
    let engine = build_test_engine();
    let results = engine.search("ogre OR wizard");
    let doc_ids: Vec<&str> = results.iter().map(|r| r.doc_id.as_str()).collect();
    assert!(doc_ids.contains(&"1"));
    assert!(doc_ids.contains(&"3"));
}

// --- Edge cases ---

#[test]
fn test_search_no_results() {
    let engine = build_test_engine();
    let results = engine.search("wizard");
    assert!(results.is_empty());
}

#[test]
fn test_empty_query() {
    let engine = build_test_engine();
    let results = engine.search("");
    assert!(results.is_empty());
}

#[test]
fn test_get_document() {
    let engine = build_test_engine();
    let doc = engine.get_document("1").unwrap();
    assert_eq!(doc.title, "Ogre Story");
    assert_eq!(doc.body, "The ogre kidnapped the bride");
    assert!(engine.get_document("999").is_none());
}

#[test]
fn test_doc_count() {
    let engine = build_test_engine();
    assert_eq!(engine.doc_count(), 3);
}

#[test]
fn test_case_insensitive() {
    let engine = build_test_engine();
    let results = engine.search("OGRE");
    let doc_ids: Vec<&str> = results.iter().map(|r| r.doc_id.as_str()).collect();
    assert!(doc_ids.contains(&"1"));
    assert!(doc_ids.contains(&"3"));
}

// --- Delete ---

#[test]
fn test_delete_document() {
    let mut engine = SearchEngine::new();
    engine.add_document("1", "Title", "The ogre kidnapped the bride");
    engine.add_document("2", "Title", "The dragon burned the village");
    assert!(engine.delete_document("1"));
    assert_eq!(engine.doc_count(), 1);
    assert!(engine.get_document("1").is_none());
    let results = engine.search("ogre");
    assert!(results.is_empty());
}

#[test]
fn test_delete_nonexistent() {
    let engine = build_test_engine();
    // Intentionally shadow with mut — we just need a mutable engine for delete
    let mut engine = engine;
    assert!(!engine.delete_document("999"));
}

// --- Stats ---

#[test]
fn test_stats() {
    let engine = build_test_engine();
    let stats = engine.stats();
    assert_eq!(stats.total_docs, 3);
    assert!(stats.total_terms > 0);
    assert!(stats.avg_doc_length > 0.0);
}

// --- BM25 scoring properties ---

#[test]
fn test_rare_term_scores_higher_than_common_term() {
    let engine = build_test_engine();
    let rare_results = engine.search("kidnapped");
    let common_results = engine.search("the");

    let rare_score = rare_results.iter().find(|r| r.doc_id == "1").unwrap().score;
    let common_score = common_results.iter().find(|r| r.doc_id == "1").unwrap().score;

    assert!(
        rare_score > common_score,
        "Rare term should score higher: {} > {}",
        rare_score,
        common_score
    );
}

#[test]
fn test_higher_tf_scores_higher() {
    let mut engine = SearchEngine::new();
    engine.add_document("1", "", "ogre");
    engine.add_document("2", "", "ogre ogre ogre");
    engine.add_document("3", "", "dragon village");

    let results = engine.search("ogre");
    assert_eq!(results[0].doc_id, "2", "Doc with more 'ogre' should rank first");
    assert!(results[0].score > results[1].score);
}

#[test]
fn test_shorter_doc_scores_higher_with_same_tf() {
    let mut engine = SearchEngine::new();
    engine.add_document("1", "", "ogre");
    engine.add_document("2", "", "ogre filler filler filler filler filler");
    engine.add_document("3", "", "dragon village");

    let results = engine.search("ogre");
    assert_eq!(results[0].doc_id, "1", "Shorter doc with same tf should rank first");
    assert!(results[0].score > results[1].score);
}

#[test]
fn test_deliverable_example() {
    let mut engine = SearchEngine::new();
    engine.add_document("1", "Ogre Story", "The ogre kidnapped the bride");
    engine.add_document("2", "Dragon Tale", "The dragon burned the village");
    engine.add_document("3", "Escape Story", "The bride escaped from the ogre");

    let results = engine.search("ogre");
    assert_eq!(results.len(), 2);
    assert!(results[0].score >= results[1].score);
    let doc_ids: Vec<&str> = results.iter().map(|r| r.doc_id.as_str()).collect();
    assert!(doc_ids.contains(&"1"));
    assert!(doc_ids.contains(&"3"));
}

#[test]
fn test_multi_term_query_scores_accumulate() {
    let mut engine = SearchEngine::new();
    engine.add_document("1", "", "ogre bride castle");
    engine.add_document("2", "", "ogre filler filler");

    let results = engine.search("ogre OR bride");
    assert_eq!(results[0].doc_id, "1");
    assert!(results[0].score > results[1].score);
}

// =========================================================================
// Phase 4 — Phrase search
// =========================================================================

#[test]
fn test_phrase_search_match() {
    let engine = build_test_engine();
    // Doc 1 body: "The ogre kidnapped the bride" → title tokens: [ogre, story]
    // Combined: [ogre, story, the, ogre, kidnapped, the, bride]
    // "ogre kidnapped" appears at positions 3,4 in doc 1
    let results = engine.search("\"ogre kidnapped\"");
    let doc_ids: Vec<&str> = results.iter().map(|r| r.doc_id.as_str()).collect();
    assert!(doc_ids.contains(&"1"), "Doc 1 should match 'ogre kidnapped'");
    assert!(!doc_ids.contains(&"3"), "Doc 3 should not match — terms not adjacent");
}

#[test]
fn test_phrase_search_no_match() {
    let engine = build_test_engine();
    // "ogre bride" are not adjacent in any doc
    // Doc 1: [ogre, story, the, ogre, kidnapped, the, bride] — ogre at 3, bride at 6
    // Doc 3: [escape, story, the, bride, escaped, from, the, ogre] — bride at 3, ogre at 7
    let results = engine.search("\"ogre bride\"");
    assert!(results.is_empty(), "No doc has 'ogre' directly followed by 'bride'");
}

#[test]
fn test_phrase_search_single_term() {
    let engine = build_test_engine();
    // Single-term phrase behaves like normal search
    let results = engine.search("\"ogre\"");
    assert_eq!(results.len(), 2);
}

#[test]
fn test_phrase_search_adjacent_in_body() {
    let mut engine = SearchEngine::new();
    engine.add_document("1", "", "the quick brown fox");
    engine.add_document("2", "", "quick fox brown jumps");

    let results = engine.search("\"quick brown\"");
    let doc_ids: Vec<&str> = results.iter().map(|r| r.doc_id.as_str()).collect();
    assert!(doc_ids.contains(&"1"), "Doc 1 has 'quick brown' adjacent");
    assert!(!doc_ids.contains(&"2"), "Doc 2 has 'quick' then 'fox' then 'brown' — not adjacent");
}

#[test]
fn test_phrase_search_term_not_in_index() {
    let engine = build_test_engine();
    let results = engine.search("\"wizard castle\"");
    assert!(results.is_empty());
}

#[test]
fn test_phrase_search_has_scores() {
    let engine = build_test_engine();
    let results = engine.search("\"ogre kidnapped\"");
    for r in &results {
        assert!(r.score > 0.0, "Phrase results should have positive scores");
    }
}

// =========================================================================
// Phase 4 — Prefix search
// =========================================================================

#[test]
fn test_prefix_search_matches() {
    let engine = build_test_engine();
    // "ogr*" should match "ogre" in docs 1 and 3
    let results = engine.search("ogr*");
    let doc_ids: Vec<&str> = results.iter().map(|r| r.doc_id.as_str()).collect();
    assert!(doc_ids.contains(&"1"));
    assert!(doc_ids.contains(&"3"));
}

#[test]
fn test_prefix_search_expands_multiple_terms() {
    let mut engine = SearchEngine::new();
    engine.add_document("1", "", "ogre castle");
    engine.add_document("2", "", "ogres village");
    engine.add_document("3", "", "dragon burned");

    let results = engine.search("ogr*");
    let doc_ids: Vec<&str> = results.iter().map(|r| r.doc_id.as_str()).collect();
    assert!(doc_ids.contains(&"1")); // "ogre" matches
    assert!(doc_ids.contains(&"2")); // "ogres" matches
    assert!(!doc_ids.contains(&"3")); // no "ogr*" terms
}

#[test]
fn test_prefix_search_no_match() {
    let engine = build_test_engine();
    let results = engine.search("xyz*");
    assert!(results.is_empty());
}

#[test]
fn test_prefix_search_exact_match() {
    let engine = build_test_engine();
    // "ogre*" should still match "ogre"
    let results = engine.search("ogre*");
    assert!(!results.is_empty());
}

#[test]
fn test_prefix_search_has_scores() {
    let engine = build_test_engine();
    let results = engine.search("ogr*");
    for r in &results {
        assert!(r.score > 0.0, "Prefix results should have positive scores");
    }
}

// =========================================================================
// Phase 4 — Fuzzy search
// =========================================================================

#[test]
fn test_fuzzy_search_distance_1() {
    let engine = build_test_engine();
    // "ogr" is distance 1 from "ogre"
    let results = engine.search("ogr~1");
    let doc_ids: Vec<&str> = results.iter().map(|r| r.doc_id.as_str()).collect();
    assert!(doc_ids.contains(&"1"));
    assert!(doc_ids.contains(&"3"));
}

#[test]
fn test_fuzzy_search_default_distance() {
    let engine = build_test_engine();
    // "ogr~" without a number defaults to distance 1
    let results = engine.search("ogr~");
    let doc_ids: Vec<&str> = results.iter().map(|r| r.doc_id.as_str()).collect();
    assert!(doc_ids.contains(&"1"));
    assert!(doc_ids.contains(&"3"));
}

#[test]
fn test_fuzzy_search_exact_match() {
    let engine = build_test_engine();
    // "ogre~0" should only match the exact term
    let results = engine.search("ogre~0");
    let doc_ids: Vec<&str> = results.iter().map(|r| r.doc_id.as_str()).collect();
    assert!(doc_ids.contains(&"1"));
    assert!(doc_ids.contains(&"3"));
}

#[test]
fn test_fuzzy_search_no_match() {
    let engine = build_test_engine();
    // "xyzzy~1" — nothing within distance 1
    let results = engine.search("xyzzy~1");
    assert!(results.is_empty());
}

#[test]
fn test_fuzzy_search_wider_distance() {
    let mut engine = SearchEngine::new();
    engine.add_document("1", "", "ogre castle");
    engine.add_document("2", "", "agree village");

    // "ogre" to "agree" is distance 2 (o→a, insert second 'e'... actually "ogre" vs "agree")
    // Let's just verify distance 2 expands more
    let results_1 = engine.search("ogre~1");
    let results_2 = engine.search("ogre~2");
    assert!(results_2.len() >= results_1.len());
}

#[test]
fn test_fuzzy_search_has_scores() {
    let engine = build_test_engine();
    let results = engine.search("ogr~1");
    for r in &results {
        assert!(r.score > 0.0, "Fuzzy results should have positive scores");
    }
}
