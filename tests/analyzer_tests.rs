use mini_elasticsearch::analyzer::Analyzer;

#[test]
fn test_basic_tokenization() {
    let tokens = Analyzer::analyze("hello world");
    assert_eq!(tokens, vec!["hello", "world"]);
}

#[test]
fn test_lowercasing() {
    let tokens = Analyzer::analyze("The Ogre KIDNAPPED the Bride");
    assert_eq!(tokens, vec!["the", "ogre", "kidnapped", "the", "bride"]);
}

#[test]
fn test_punctuation_stripping() {
    let tokens = Analyzer::analyze("Hello, World! How's it going?");
    assert_eq!(tokens, vec!["hello", "world", "how", "s", "it", "going"]);
}

#[test]
fn test_multiple_delimiters() {
    let tokens = Analyzer::analyze("one---two...three   four");
    assert_eq!(tokens, vec!["one", "two", "three", "four"]);
}

#[test]
fn test_empty_input() {
    let tokens = Analyzer::analyze("");
    assert!(tokens.is_empty());
}

#[test]
fn test_only_punctuation() {
    let tokens = Analyzer::analyze("!@#$%^&*()");
    assert!(tokens.is_empty());
}

#[test]
fn test_numeric_tokens() {
    let tokens = Analyzer::analyze("chapter 42 is great");
    assert_eq!(tokens, vec!["chapter", "42", "is", "great"]);
}

#[test]
fn test_unicode() {
    let tokens = Analyzer::analyze("Ünïcödé café résumé");
    assert_eq!(tokens, vec!["ünïcödé", "café", "résumé"]);
}

#[test]
fn test_single_word() {
    let tokens = Analyzer::analyze("ogre");
    assert_eq!(tokens, vec!["ogre"]);
}

#[test]
fn test_mixed_alphanumeric() {
    let tokens = Analyzer::analyze("doc123 test456abc");
    assert_eq!(tokens, vec!["doc123", "test456abc"]);
}
