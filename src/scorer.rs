/// BM25 relevance scorer.
///
/// Computes document relevance scores using the Okapi BM25 ranking function,
/// which improves on TF-IDF by adding term frequency saturation and document
/// length normalization.
///
/// # Parameters
///
/// - `k1` — Controls term frequency saturation. Higher values give more weight
///   to repeated terms. Typical value: 1.2.
/// - `b` — Controls document length normalization. 0.0 means no normalization,
///   1.0 means full normalization. Typical value: 0.75.
pub struct BM25Scorer {
    pub k1: f64,
    pub b: f64,
}

impl Default for BM25Scorer {
    fn default() -> Self {
        Self { k1: 1.2, b: 0.75 }
    }
}

impl BM25Scorer {
    /// Create a new scorer with the standard BM25 parameters (k1=1.2, b=0.75).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a scorer with custom parameters.
    pub fn with_params(k1: f64, b: f64) -> Self {
        Self { k1, b }
    }

    /// Compute the Inverse Document Frequency for a term.
    ///
    /// ```text
    /// IDF(term) = ln(total_docs / docs_containing_term)
    /// ```
    ///
    /// A term appearing in every document has IDF ≈ 0 (not useful for ranking).
    /// A rare term has high IDF (very useful for ranking).
    ///
    /// Returns 0.0 if `doc_freq` is 0 to avoid division by zero.
    pub fn idf(&self, doc_freq: u64, total_docs: u64) -> f64 {
        if doc_freq == 0 || total_docs == 0 {
            return 0.0;
        }
        (total_docs as f64 / doc_freq as f64).ln()
    }

    /// Compute the BM25 score for a single term in a single document.
    ///
    /// ```text
    /// BM25(term, doc) = IDF(term) × (tf × (k1 + 1)) / (tf + k1 × (1 - b + b × (dl / avgdl)))
    /// ```
    ///
    /// # Arguments
    ///
    /// - `tf` — Term frequency: how many times the term appears in the document.
    /// - `doc_length` — Total number of tokens in the document.
    /// - `avg_doc_length` — Average document length across the corpus.
    /// - `idf` — Pre-computed IDF value for this term.
    pub fn score(&self, tf: u32, doc_length: u32, avg_doc_length: f64, idf: f64) -> f64 {
        let tf = tf as f64;
        let dl = doc_length as f64;

        let numerator = tf * (self.k1 + 1.0);
        let denominator = tf + self.k1 * (1.0 - self.b + self.b * (dl / avg_doc_length));

        idf * (numerator / denominator)
    }
}
