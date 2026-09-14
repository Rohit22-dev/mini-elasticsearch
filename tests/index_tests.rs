use mini_elasticsearch::index::InvertedIndex;

fn build_test_index() -> InvertedIndex {
    let mut idx = InvertedIndex::new();
    idx.add_document("1", "Doc One", "The ogre kidnapped the bride");
    idx.add_document("2", "Doc Two", "The dragon burned the village");
    idx.add_document("3", "Doc Three", "The bride escaped from the ogre");
    idx
}

#[test]
fn test_doc_count() {
    let idx = build_test_index();
    assert_eq!(idx.doc_count(), 3);
}

#[test]
fn test_postings_exist_for_indexed_term() {
    let idx = build_test_index();
    let postings = idx.get_postings("ogre");
    assert!(postings.is_some());
    let postings = postings.unwrap();
    assert_eq!(postings.len(), 2); // docs 1 and 3
}

#[test]
fn test_postings_contain_correct_doc_ids() {
    let idx = build_test_index();
    let postings = idx.get_postings("ogre").unwrap();
    let mut doc_ids: Vec<&str> = postings.iter().map(|p| p.doc_id.as_str()).collect();
    doc_ids.sort();
    assert_eq!(doc_ids, vec!["1", "3"]);
}

#[test]
fn test_term_frequency_single_occurrence() {
    let idx = build_test_index();
    let postings = idx.get_postings("ogre").unwrap();
    for posting in postings {
        assert_eq!(posting.term_frequency, 1);
    }
}

#[test]
fn test_term_frequency_multiple_occurrences() {
    let mut idx = InvertedIndex::new();
    idx.add_document("1", "", "ogre ogre ogre");
    let postings = idx.get_postings("ogre").unwrap();
    assert_eq!(postings.len(), 1);
    assert_eq!(postings[0].term_frequency, 3);
}

#[test]
fn test_nonexistent_term_returns_none() {
    let idx = build_test_index();
    assert!(idx.get_postings("wizard").is_none());
}

#[test]
fn test_get_document() {
    let idx = build_test_index();
    let doc = idx.get_document("1").unwrap();
    assert_eq!(doc.title, "Doc One");
    assert_eq!(doc.body, "The ogre kidnapped the bride");
    assert!(idx.get_document("999").is_none());
}

#[test]
fn test_metadata_doc_lengths() {
    let idx = build_test_index();
    let meta = idx.get_metadata();
    // "Doc One" + "The ogre kidnapped the bride" → "doc one the ogre kidnapped the bride" = 7 tokens
    assert_eq!(meta.doc_lengths.get("1"), Some(&7));
}

#[test]
fn test_metadata_total_doc_length() {
    let mut idx = InvertedIndex::new();
    idx.add_document("1", "", "one two three"); // 3 tokens
    idx.add_document("2", "", "four five");      // 2 tokens
    let meta = idx.get_metadata();
    assert_eq!(meta.total_doc_length, 5);
}

#[test]
fn test_metadata_avg_doc_length() {
    let mut idx = InvertedIndex::new();
    idx.add_document("1", "", "one two three"); // 3 tokens
    idx.add_document("2", "", "four five");      // 2 tokens
    let meta = idx.get_metadata();
    let avg = meta.avg_doc_length();
    assert!((avg - 2.5).abs() < 1e-10);
}

#[test]
fn test_duplicate_doc_id_overwrites() {
    let mut idx = InvertedIndex::new();
    idx.add_document("1", "First", "first version");
    idx.add_document("1", "Second", "second version");
    assert_eq!(idx.doc_count(), 1);
    let doc = idx.get_document("1").unwrap();
    assert_eq!(doc.title, "Second");
    assert_eq!(doc.body, "second version");
}

#[test]
fn test_duplicate_doc_id_updates_metadata() {
    let mut idx = InvertedIndex::new();
    idx.add_document("1", "", "hello world"); // 2 tokens
    idx.add_document("1", "", "a b c d");     // 4 tokens
    let meta = idx.get_metadata();
    assert_eq!(meta.total_docs, 1);
    assert_eq!(meta.total_doc_length, 4);
    assert_eq!(meta.doc_lengths.get("1"), Some(&4));
}

#[test]
fn test_case_insensitive_postings() {
    let mut idx = InvertedIndex::new();
    idx.add_document("1", "Title", "The Ogre KIDNAPPED the Bride");
    let postings = idx.get_postings("ogre");
    assert!(postings.is_some());
    assert_eq!(postings.unwrap()[0].doc_id, "1");
}

#[test]
fn test_delete_document() {
    let mut idx = build_test_index();
    assert!(idx.delete_document("1"));
    assert_eq!(idx.doc_count(), 2);
    assert!(idx.get_document("1").is_none());
    // "ogre" should now only be in doc 3
    let postings = idx.get_postings("ogre").unwrap();
    assert_eq!(postings.len(), 1);
    assert_eq!(postings[0].doc_id, "3");
}

#[test]
fn test_delete_nonexistent_document() {
    let mut idx = build_test_index();
    assert!(!idx.delete_document("999"));
    assert_eq!(idx.doc_count(), 3);
}

#[test]
fn test_delete_updates_metadata() {
    let mut idx = build_test_index();
    let old_total = idx.get_metadata().total_doc_length;
    let doc1_len = *idx.get_metadata().doc_lengths.get("1").unwrap() as u64;
    idx.delete_document("1");
    assert_eq!(idx.get_metadata().total_doc_length, old_total - doc1_len);
    assert!(idx.get_metadata().doc_lengths.get("1").is_none());
}

#[test]
fn test_term_count() {
    let mut idx = InvertedIndex::new();
    idx.add_document("1", "", "hello world");
    idx.add_document("2", "", "hello rust");
    assert_eq!(idx.term_count(), 3); // hello, world, rust
}

#[test]
fn test_title_and_body_both_indexed() {
    let mut idx = InvertedIndex::new();
    idx.add_document("1", "castle", "ogre");
    // Both "castle" (from title) and "ogre" (from body) should be searchable
    assert!(idx.get_postings("castle").is_some());
    assert!(idx.get_postings("ogre").is_some());
}

// --- Phase 4: Positional postings ---

#[test]
fn test_postings_have_positions() {
    let mut idx = InvertedIndex::new();
    idx.add_document("1", "", "The ogre kidnapped the bride");
    // "ogre" is at position 1 (0-based: the=0, ogre=1, kidnapped=2, the=3, bride=4)
    let postings = idx.get_postings("ogre").unwrap();
    assert_eq!(postings[0].positions, vec![1]);
}

#[test]
fn test_positions_multiple_occurrences() {
    let mut idx = InvertedIndex::new();
    idx.add_document("1", "", "the ogre met the ogre");
    // "ogre" appears at positions 1 and 4
    let postings = idx.get_postings("ogre").unwrap();
    let mut positions = postings[0].positions.clone();
    positions.sort();
    assert_eq!(positions, vec![1, 4]);
}

#[test]
fn test_positions_title_and_body_continuous() {
    let mut idx = InvertedIndex::new();
    idx.add_document("1", "hello world", "foo bar");
    // Title: hello=0, world=1 | Body: foo=2, bar=3
    let postings = idx.get_postings("foo").unwrap();
    assert_eq!(postings[0].positions, vec![2]);
    let postings = idx.get_postings("hello").unwrap();
    assert_eq!(postings[0].positions, vec![0]);
}

// --- Phase 4: Prefix term lookup ---

#[test]
fn test_get_terms_with_prefix() {
    let mut idx = InvertedIndex::new();
    idx.add_document("1", "", "ogre ogres ogrish dragon");
    let mut terms = idx.get_terms_with_prefix("ogr");
    terms.sort();
    assert_eq!(terms, vec!["ogre", "ogres", "ogrish"]);
}

#[test]
fn test_get_terms_with_prefix_no_match() {
    let idx = build_test_index();
    let terms = idx.get_terms_with_prefix("xyz");
    assert!(terms.is_empty());
}

#[test]
fn test_get_terms_with_prefix_exact() {
    let mut idx = InvertedIndex::new();
    idx.add_document("1", "", "ogre dragon");
    let terms = idx.get_terms_with_prefix("ogre");
    assert_eq!(terms, vec!["ogre"]);
}

// --- Phase 4: All terms ---

#[test]
fn test_all_terms() {
    let mut idx = InvertedIndex::new();
    idx.add_document("1", "", "hello world");
    let mut terms = idx.all_terms();
    terms.sort();
    assert_eq!(terms, vec!["hello", "world"]);
}
