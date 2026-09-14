use mini_elasticsearch::fuzzy::{find_fuzzy_matches, levenshtein};

// --- Levenshtein distance ---

#[test]
fn test_levenshtein_identical() {
    assert_eq!(levenshtein("ogre", "ogre"), 0);
}

#[test]
fn test_levenshtein_empty_strings() {
    assert_eq!(levenshtein("", ""), 0);
    assert_eq!(levenshtein("abc", ""), 3);
    assert_eq!(levenshtein("", "abc"), 3);
}

#[test]
fn test_levenshtein_insertion() {
    assert_eq!(levenshtein("ogr", "ogre"), 1); // insert 'e'
}

#[test]
fn test_levenshtein_deletion() {
    assert_eq!(levenshtein("ogre", "ogr"), 1); // delete 'e'
}

#[test]
fn test_levenshtein_substitution() {
    assert_eq!(levenshtein("ogre", "ogra"), 1); // substitute 'e' → 'a'
}

#[test]
fn test_levenshtein_multiple_edits() {
    assert_eq!(levenshtein("kitten", "sitting"), 3);
}

#[test]
fn test_levenshtein_completely_different() {
    assert_eq!(levenshtein("abc", "xyz"), 3);
}

#[test]
fn test_levenshtein_case_sensitive() {
    // Levenshtein is case-sensitive; the caller should lowercase first
    assert_eq!(levenshtein("Ogre", "ogre"), 1);
}

// --- Fuzzy matching ---

#[test]
fn test_find_fuzzy_matches_distance_1() {
    let terms = vec!["ogre", "ogres", "dragon", "ore", "agree"];
    let matches = find_fuzzy_matches("ogre", &terms, 1);
    assert!(matches.contains(&"ogre"));   // distance 0
    assert!(matches.contains(&"ogres"));  // distance 1
    assert!(matches.contains(&"ore"));    // distance 1
    assert!(!matches.contains(&"dragon"));
    assert!(!matches.contains(&"agree"));
}

#[test]
fn test_find_fuzzy_matches_distance_0() {
    let terms = vec!["ogre", "ogres", "dragon"];
    let matches = find_fuzzy_matches("ogre", &terms, 0);
    assert_eq!(matches, vec!["ogre"]); // only exact match
}

#[test]
fn test_find_fuzzy_matches_distance_2() {
    let terms = vec!["ogre", "ogres", "ogrish", "dragon"];
    let matches = find_fuzzy_matches("ogr", &terms, 2);
    assert!(matches.contains(&"ogre"));   // distance 1
    assert!(matches.contains(&"ogres"));  // distance 2
    assert!(!matches.contains(&"ogrish")); // distance 3
}

#[test]
fn test_find_fuzzy_matches_no_matches() {
    let terms = vec!["dragon", "village", "castle"];
    let matches = find_fuzzy_matches("xyz", &terms, 1);
    assert!(matches.is_empty());
}

#[test]
fn test_find_fuzzy_matches_empty_terms() {
    let terms: Vec<&str> = vec![];
    let matches = find_fuzzy_matches("ogre", &terms, 1);
    assert!(matches.is_empty());
}
