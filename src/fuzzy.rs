/// Fuzzy matching utilities using Levenshtein (edit) distance.
///
/// Levenshtein distance is the minimum number of single-character insertions,
/// deletions, or substitutions needed to transform one string into another.
///
/// # Examples
/// ```
/// use mini_elasticsearch::fuzzy::levenshtein;
/// assert_eq!(levenshtein("ogr", "ogre"), 1);    // insert 'e'
/// assert_eq!(levenshtein("kitten", "sitting"), 3);
/// assert_eq!(levenshtein("same", "same"), 0);
/// ```

/// Default edit distance when '~' is specified without an explicit number.
pub const DEFAULT_MAX_DISTANCE: usize = 1;

/// Compute the Levenshtein (edit) distance between two strings.
///
/// Uses the standard dynamic-programming approach with O(n×m) time and O(n×m)
/// space, where n and m are the lengths of the two strings.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let n = a_chars.len();
    let m = b_chars.len();

    let mut dp = vec![vec![0usize; m + 1]; n + 1];

    for i in 0..=n {
        dp[i][0] = i;
    }
    for j in 0..=m {
        dp[0][j] = j;
    }

    for (i, ca) in a_chars.iter().enumerate() {
        for (j, cb) in b_chars.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            dp[i + 1][j + 1] = (dp[i][j] + cost)
                .min(dp[i + 1][j] + 1)
                .min(dp[i][j + 1] + 1);
        }
    }

    dp[n][m]
}

/// Find all terms from `all_terms` that are within `max_distance` edits of `query_term`.
///
/// This is a brute-force scan — it checks every term in the index. For small
/// to medium indexes this is perfectly adequate. For very large indexes, a
/// Levenshtein automaton or BK-tree would be more efficient.
pub fn find_fuzzy_matches<'a>(
    query_term: &str,
    all_terms: &[&'a str],
    max_distance: usize,
) -> Vec<&'a str> {
    all_terms
        .iter()
        .filter(|term| levenshtein(query_term, term) <= max_distance)
        .copied()
        .collect()
}
