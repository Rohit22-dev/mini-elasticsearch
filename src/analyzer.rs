/// Text analyzer that converts raw text into normalized, searchable tokens.
///
/// The analysis pipeline:
/// 1. Split on any non-alphanumeric character boundary
/// 2. Filter out empty fragments (from consecutive delimiters)
/// 3. Lowercase every token
pub struct Analyzer;

impl Analyzer {
    /// Analyze the given text and return a list of normalized tokens.
    ///
    /// # Examples
    /// ```
    /// use mini_elasticsearch::analyzer::Analyzer;
    /// let tokens = Analyzer::analyze("The Ogre kidnapped the Bride!");
    /// assert_eq!(tokens, vec!["the", "ogre", "kidnapped", "the", "bride"]);
    /// ```
    pub fn analyze(text: &str) -> Vec<String> {
        text.split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_lowercase())
            .collect()
    }

    /// Analyze text and return tokens paired with their 0-based positions.
    ///
    /// Each token is assigned a sequential position starting from `offset`.
    /// This is used during indexing to record where terms appear in a document,
    /// enabling phrase search (positional matching).
    ///
    /// # Examples
    /// ```
    /// use mini_elasticsearch::analyzer::Analyzer;
    /// let tokens = Analyzer::analyze_with_positions("The Ogre kidnapped the Bride!", 0);
    /// assert_eq!(tokens, vec![
    ///     ("the".to_string(), 0),
    ///     ("ogre".to_string(), 1),
    ///     ("kidnapped".to_string(), 2),
    ///     ("the".to_string(), 3),
    ///     ("bride".to_string(), 4),
    /// ]);
    /// ```
    pub fn analyze_with_positions(text: &str, offset: u32) -> Vec<(String, u32)> {
        text.split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .enumerate()
            .map(|(i, s)| (s.to_lowercase(), offset + i as u32))
            .collect()
    }
}
