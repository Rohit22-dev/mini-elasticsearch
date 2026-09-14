use std::collections::{BTreeMap, HashMap};

use crate::analyzer::Analyzer;

/// Unique identifier for a document (string-based to support API IDs like "abc-123").
pub type DocId = String;

/// A single posting entry: which document a term appeared in, how many times,
/// and at which positions.
///
/// Positions are 0-based token offsets within the combined title+body text.
/// They are used for phrase search (checking positional adjacency).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Posting {
    pub doc_id: DocId,
    pub term_frequency: u32,
    pub positions: Vec<u32>,
}

/// A stored document with its title and body text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub id: DocId,
    pub title: String,
    pub body: String,
}

/// Metadata about the indexed corpus, needed for BM25 length normalization.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct IndexMetadata {
    /// Total number of documents indexed.
    pub total_docs: u64,
    /// Sum of all document lengths (in tokens).
    pub total_doc_length: u64,
    /// Length (in tokens) of each individual document.
    pub doc_lengths: HashMap<DocId, u32>,
}

impl IndexMetadata {
    /// Create empty metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Average document length across the corpus.
    pub fn avg_doc_length(&self) -> f64 {
        if self.total_docs == 0 {
            return 0.0;
        }
        self.total_doc_length as f64 / self.total_docs as f64
    }
}

/// An in-memory inverted index mapping terms to positional postings lists.
///
/// Each term maps to a `Vec<Posting>` containing the document ID, the number
/// of times the term appears in that document, and the positions where it
/// appears. A `BTreeMap` is used instead of `HashMap` to keep terms sorted,
/// enabling efficient prefix range queries.
pub struct InvertedIndex {
    /// term → list of postings (doc_id + term_frequency + positions)
    ///
    /// `BTreeMap` keeps terms in sorted order, enabling prefix search via `.range()`.
    index: BTreeMap<String, Vec<Posting>>,
    /// doc_id → stored document (title + body)
    documents: HashMap<DocId, Document>,
    /// Corpus-level metadata for BM25 scoring.
    metadata: IndexMetadata,
}

impl Default for InvertedIndex {
    fn default() -> Self {
        Self {
            index: BTreeMap::new(),
            documents: HashMap::new(),
            metadata: IndexMetadata::new(),
        }
    }
}

impl InvertedIndex {
    /// Create a new, empty inverted index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Index a document: analyze the combined title+body text, count term
    /// frequencies, record positions, and store postings.
    ///
    /// If a document with the same ID already exists, it will be overwritten.
    /// The old document's contributions are removed before re-indexing.
    pub fn add_document(&mut self, id: &str, title: &str, body: &str) {
        let doc_id = id.to_string();

        // If updating an existing document, remove old data first
        if self.documents.contains_key(&doc_id) {
            self.remove_document_postings(&doc_id);
        }

        // Store the document
        self.documents.insert(
            doc_id.clone(),
            Document {
                id: doc_id.clone(),
                title: title.to_string(),
                body: body.to_string(),
            },
        );

        // Analyze combined title + body into tokens with positions.
        // Title tokens get positions starting at 0, body tokens continue
        // from where the title left off.
        let title_tokens = Analyzer::analyze_with_positions(title, 0);
        let body_offset = title_tokens.len() as u32;
        let body_tokens = Analyzer::analyze_with_positions(body, body_offset);

        let doc_length = (title_tokens.len() + body_tokens.len()) as u32;

        // Collect term frequencies and positions within this document
        let mut term_data: HashMap<String, (u32, Vec<u32>)> = HashMap::new();
        for (token, pos) in title_tokens.into_iter().chain(body_tokens.into_iter()) {
            let entry = term_data.entry(token).or_insert_with(|| (0, Vec::new()));
            entry.0 += 1;
            entry.1.push(pos);
        }

        // Insert postings
        for (term, (freq, positions)) in term_data {
            self.index
                .entry(term)
                .or_default()
                .push(Posting {
                    doc_id: doc_id.clone(),
                    term_frequency: freq,
                    positions,
                });
        }

        // Update metadata
        self.metadata.total_docs += 1;
        self.metadata.total_doc_length += doc_length as u64;
        self.metadata.doc_lengths.insert(doc_id, doc_length);
    }

    /// Delete a document by ID. Returns `true` if the document existed.
    ///
    /// Removes the document from storage, all its postings from the index,
    /// and updates corpus metadata.
    pub fn delete_document(&mut self, id: &str) -> bool {
        if self.documents.remove(id).is_none() {
            return false;
        }
        self.remove_document_postings(id);
        true
    }

    /// Remove a document's postings and metadata (internal helper).
    fn remove_document_postings(&mut self, id: &str) {
        // Remove from all postings lists
        for postings in self.index.values_mut() {
            postings.retain(|p| p.doc_id != id);
        }
        // Remove empty terms
        self.index.retain(|_, postings| !postings.is_empty());

        // Update metadata
        if let Some(old_length) = self.metadata.doc_lengths.remove(id) {
            self.metadata.total_doc_length -= old_length as u64;
            self.metadata.total_docs -= 1;
        }
    }

    /// Get the postings list for a given term, if it exists.
    pub fn get_postings(&self, term: &str) -> Option<&Vec<Posting>> {
        self.index.get(term)
    }

    /// Get the corpus metadata (total docs, doc lengths, etc.).
    pub fn get_metadata(&self) -> &IndexMetadata {
        &self.metadata
    }

    /// Retrieve a stored document by ID.
    pub fn get_document(&self, id: &str) -> Option<&Document> {
        self.documents.get(id)
    }

    /// Return the total number of indexed documents.
    pub fn doc_count(&self) -> u64 {
        self.metadata.total_docs
    }

    /// Return the total number of unique terms in the index.
    pub fn term_count(&self) -> usize {
        self.index.len()
    }

    /// Return all terms in the index that start with the given prefix.
    ///
    /// Uses `BTreeMap::range` for efficient lookup — only terms in the
    /// matching key range are visited.
    pub fn get_terms_with_prefix(&self, prefix: &str) -> Vec<&str> {
        // Build the exclusive upper bound by incrementing the last byte of the prefix.
        // e.g., "ogr" → range ["ogr", "ogs")
        let mut upper = prefix.to_string();
        if let Some(last) = upper.pop() {
            upper.push(char::from_u32(last as u32 + 1).unwrap_or(char::MAX));
        } else {
            // Empty prefix → return all terms
            return self.index.keys().map(|s| s.as_str()).collect();
        }

        self.index
            .range(prefix.to_string()..upper)
            .map(|(term, _)| term.as_str())
            .collect()
    }

    /// Return all unique terms in the index.
    ///
    /// Used by fuzzy search to brute-force scan for terms within a given
    /// edit distance.
    pub fn all_terms(&self) -> Vec<&str> {
        self.index.keys().map(|s| s.as_str()).collect()
    }

    /// Access the underlying term dictionary and postings.
    pub fn index(&self) -> &BTreeMap<String, Vec<Posting>> {
        &self.index
    }

    /// Access the document store.
    pub fn documents(&self) -> &HashMap<DocId, Document> {
        &self.documents
    }

    /// Construct an index directly from its components (used during deserialization).
    pub fn from_parts(
        index: BTreeMap<String, Vec<Posting>>,
        documents: HashMap<DocId, Document>,
        metadata: IndexMetadata,
    ) -> Self {
        Self {
            index,
            documents,
            metadata,
        }
    }

    /// Merge another inverted index into this one.
    ///
    /// Combines postings lists, documents, and updates corpus metadata.
    pub fn merge_with(&mut self, other: InvertedIndex) {
        for (doc_id, doc) in other.documents {
            // If already present in self, remove old postings to avoid duplicates
            if self.documents.contains_key(&doc_id) {
                self.remove_document_postings(&doc_id);
            }
            self.documents.insert(doc_id, doc);
        }

        for (term, postings) in other.index {
            let entry = self.index.entry(term).or_default();
            for p in postings {
                // If posting for this doc_id already exists in list, replace it
                if let Some(pos) = entry.iter().position(|x| x.doc_id == p.doc_id) {
                    entry[pos] = p;
                } else {
                    entry.push(p);
                }
            }
        }

        // Recalculate metadata from documents
        let mut total_docs = 0u64;
        let mut total_doc_length = 0u64;
        let mut doc_lengths = HashMap::new();

        for (doc_id, doc) in &self.documents {
            let title_tokens = Analyzer::analyze_with_positions(&doc.title, 0);
            let body_offset = title_tokens.len() as u32;
            let body_tokens = Analyzer::analyze_with_positions(&doc.body, body_offset);
            let doc_length = (title_tokens.len() + body_tokens.len()) as u32;

            total_docs += 1;
            total_doc_length += doc_length as u64;
            doc_lengths.insert(doc_id.clone(), doc_length);
        }

        self.metadata = IndexMetadata {
            total_docs,
            total_doc_length,
            doc_lengths,
        };
    }
}

