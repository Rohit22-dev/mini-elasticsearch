use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::analyzer::Analyzer;
use crate::config::Config;
use crate::fuzzy;
use crate::index::{DocId, InvertedIndex};
use crate::scorer::BM25Scorer;
use crate::storage::merge::merge_segments;
use crate::storage::segment::{SegmentReader, SegmentWriter};
use crate::storage::wal::{Wal, WalRecord};

/// A single search result containing the document ID, title, and BM25 relevance score.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub doc_id: DocId,
    pub title: String,
    pub score: f64,
}

/// Index statistics returned by the `/_stats` endpoint.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexStats {
    pub total_docs: u64,
    pub total_terms: usize,
    pub avg_doc_length: f64,
    pub segment_count: usize,
}

/// The type of query detected by the parser.
#[derive(Debug, PartialEq)]
enum QueryMode {
    /// `"ogre bride"` — documents where the terms appear adjacent and in order.
    Phrase(Vec<String>),
    /// `ogr*` — expand the prefix to all matching terms, union results.
    Prefix(String),
    /// `ogr~1` — find terms within the given edit distance, union results.
    Fuzzy(String, usize),
    /// Standard AND or OR query (unchanged from Phase 3).
    Standard(Vec<String>, bool),
}

/// High-level search engine facade.
///
/// Wraps the inverted index and BM25 scorer, providing a simple API for indexing
/// documents and running ranked queries. Results are sorted by BM25 score (descending).
///
/// Supports persistence via WAL and segment files.
pub struct SearchEngine {
    index: InvertedIndex,
    buffered_index: InvertedIndex,
    scorer: BM25Scorer,
    data_dir: Option<PathBuf>,
    wal: Option<Wal>,
    flush_threshold: usize,
    unflushed_docs: usize,
    segment_count: usize,
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self {
            index: InvertedIndex::new(),
            buffered_index: InvertedIndex::new(),
            scorer: BM25Scorer::new(),
            data_dir: None,
            wal: None,
            flush_threshold: 0,
            unflushed_docs: 0,
            segment_count: 0,
        }
    }
}

impl SearchEngine {
    /// Create a new, in-memory search engine with default BM25 parameters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an in-memory search engine with custom BM25 scorer.
    pub fn with_scorer(scorer: BM25Scorer) -> Self {
        Self {
            scorer,
            ..Self::default()
        }
    }

    /// Open or create a persistent search engine at `data_dir`.
    ///
    /// 1. Discovers and loads existing segment files from `data_dir/segments` via mmap.
    /// 2. Replays uncommitted WAL records from `data_dir/wal/wal.log`.
    pub fn open(data_dir: &Path, flush_threshold: usize) -> io::Result<Self> {
        Self::open_with_scorer(data_dir, flush_threshold, BM25Scorer::new())
    }

    /// Open a persistent search engine with a custom scorer.
    pub fn open_with_scorer(
        data_dir: &Path,
        flush_threshold: usize,
        scorer: BM25Scorer,
    ) -> io::Result<Self> {
        let segments_dir = data_dir.join("segments");
        let wal_dir = data_dir.join("wal");
        fs::create_dir_all(&segments_dir)?;
        fs::create_dir_all(&wal_dir)?;

        let mut index = InvertedIndex::new();
        let mut segment_count = 0;

        // Collect and sort segment files
        let mut segment_files = Vec::new();
        if segments_dir.exists() {
            for entry in fs::read_dir(&segments_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("seg") {
                    segment_files.push(path);
                }
            }
        }
        segment_files.sort();

        // Load segments via mmap
        for seg_path in &segment_files {
            let seg_index = match SegmentReader::read_mmap(seg_path) {
                Ok(idx) => idx,
                Err(_) => SegmentReader::read(seg_path)?,
            };
            index.merge_with(seg_index);
            segment_count += 1;
        }

        // Open WAL
        let wal_path = wal_dir.join("wal.log");
        let wal = Wal::open(&wal_path)?;
        let wal_records = wal.replay()?;

        let mut buffered_index = InvertedIndex::new();
        let mut unflushed_docs = 0;

        for record in wal_records {
            match record {
                WalRecord::Add { id, title, body } => {
                    index.add_document(&id, &title, &body);
                    buffered_index.add_document(&id, &title, &body);
                    unflushed_docs += 1;
                }
                WalRecord::Delete { id } => {
                    index.delete_document(&id);
                    buffered_index.delete_document(&id);
                }
            }
        }

        tracing::info!(
            data_dir = ?data_dir,
            loaded_segments = segment_count,
            recovered_wal_docs = unflushed_docs,
            "Opened persistent search engine"
        );

        Ok(Self {
            index,
            buffered_index,
            scorer,
            data_dir: Some(data_dir.to_path_buf()),
            wal: Some(wal),
            flush_threshold,
            unflushed_docs,
            segment_count,
        })
    }

    /// Index a document with the given ID, title, and body.
    pub fn add_document(&mut self, id: &str, title: &str, body: &str) {
        if let Some(wal) = &mut self.wal {
            if let Err(e) = wal.append_add(id, title, body) {
                tracing::error!(error = %e, "Failed to write to WAL");
            }
        }

        self.index.add_document(id, title, body);

        if self.data_dir.is_some() {
            self.buffered_index.add_document(id, title, body);
            self.unflushed_docs += 1;

            if self.flush_threshold > 0 && self.unflushed_docs >= self.flush_threshold {
                let _ = self.flush();
            }
        }

        tracing::debug!(doc_id = %id, "Document indexed");
    }

    /// Delete a document by ID. Returns `true` if the document existed.
    pub fn delete_document(&mut self, id: &str) -> bool {
        if let Some(wal) = &mut self.wal {
            if let Err(e) = wal.append_delete(id) {
                tracing::error!(error = %e, "Failed to append delete to WAL");
            }
        }

        if self.data_dir.is_some() {
            self.buffered_index.delete_document(id);
        }

        let deleted = self.index.delete_document(id);
        if deleted {
            tracing::debug!(doc_id = %id, "Document deleted");
        }
        deleted
    }

    /// Flush the current in-memory buffer to a new segment file and truncate WAL.
    pub fn flush(&mut self) -> io::Result<Option<PathBuf>> {
        let data_dir = match &self.data_dir {
            Some(dir) => dir.clone(),
            None => return Ok(None),
        };

        if self.buffered_index.doc_count() == 0 {
            return Ok(None);
        }

        let segments_dir = data_dir.join("segments");
        fs::create_dir_all(&segments_dir)?;

        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let filename = format!("segment_{}_{}.seg", ts, self.segment_count + 1);
        let path = segments_dir.join(filename);

        SegmentWriter::write(&self.buffered_index, &path)?;

        self.buffered_index = InvertedIndex::new();
        self.unflushed_docs = 0;
        self.segment_count += 1;

        if let Some(wal) = &mut self.wal {
            wal.truncate()?;
        }

        tracing::info!(segment = ?path, "Segment flushed to disk");
        Ok(Some(path))
    }

    /// Force-merge all segment files into a single consolidated segment.
    pub fn compact(&mut self) -> io::Result<()> {
        let data_dir = match &self.data_dir {
            Some(dir) => dir.clone(),
            None => return Ok(()),
        };

        // First flush any unflushed buffered docs
        if self.buffered_index.doc_count() > 0 {
            self.flush()?;
        }

        let segments_dir = data_dir.join("segments");
        if !segments_dir.exists() {
            return Ok(());
        }

        let mut segment_files = Vec::new();
        for entry in fs::read_dir(&segments_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("seg") {
                segment_files.push(path);
            }
        }

        if segment_files.len() > 1 {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let out_path = segments_dir.join(format!("segment_compacted_{}.seg", ts));

            merge_segments(&segment_files, &out_path)?;
            self.segment_count = 1;
            tracing::info!(compacted_segment = ?out_path, "Compacted all segments into one");
        }

        Ok(())
    }

    /// Search for documents matching the query, ranked by BM25 score.
    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let results = match Self::parse_query(query) {
            QueryMode::Phrase(terms) => self.phrase_search(&terms),
            QueryMode::Prefix(prefix) => self.prefix_search(&prefix),
            QueryMode::Fuzzy(term, max_dist) => self.fuzzy_search(&term, max_dist),
            QueryMode::Standard(terms, is_or) => self.standard_search(&terms, is_or),
        };

        tracing::debug!(query = %query, hits = results.len(), "Search query executed");
        results
    }

    /// Retrieve a document's title and body by its ID.
    pub fn get_document(&self, id: &str) -> Option<&crate::index::Document> {
        self.index.get_document(id)
    }

    /// Return the total number of indexed documents.
    pub fn doc_count(&self) -> u64 {
        self.index.doc_count()
    }

    /// Return the number of committed segment files on disk.
    pub fn segment_count(&self) -> usize {
        self.segment_count
    }

    /// Return index statistics.
    pub fn stats(&self) -> IndexStats {
        let metadata = self.index.get_metadata();
        IndexStats {
            total_docs: metadata.total_docs,
            total_terms: self.index.term_count(),
            avg_doc_length: metadata.avg_doc_length(),
            segment_count: self.segment_count,
        }
    }

    // -----------------------------------------------------------------------
    // Query parsing
    // -----------------------------------------------------------------------

    /// Parse a query string into a `QueryMode`.
    fn parse_query(query: &str) -> QueryMode {
        let trimmed = query.trim();

        if trimmed.is_empty() {
            return QueryMode::Standard(Vec::new(), false);
        }

        // 1. Phrase query: starts and ends with double quotes
        if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
            let inner = &trimmed[1..trimmed.len() - 1];
            let terms = Analyzer::analyze(inner);
            return QueryMode::Phrase(terms);
        }

        // 2. Prefix query: single token ending with '*'
        if trimmed.ends_with('*') && !trimmed.contains(char::is_whitespace) {
            let prefix = trimmed[..trimmed.len() - 1].to_lowercase();
            if !prefix.is_empty() {
                return QueryMode::Prefix(prefix);
            }
        }

        // 3. Fuzzy query: single token ending with '~' or '~N'
        if let Some(tilde_pos) = trimmed.rfind('~') {
            let before = &trimmed[..tilde_pos];
            let after = &trimmed[tilde_pos + 1..];

            if !before.is_empty()
                && !before.contains(char::is_whitespace)
                && !after.contains(char::is_whitespace)
            {
                let distance = if after.is_empty() {
                    fuzzy::DEFAULT_MAX_DISTANCE
                } else if let Ok(d) = after.parse::<usize>() {
                    d
                } else {
                    return Self::parse_standard(trimmed);
                };

                let term = before.to_lowercase();
                return QueryMode::Fuzzy(term, distance);
            }
        }

        // 4. Standard AND / OR query
        Self::parse_standard(trimmed)
    }

    /// Parse a standard query with optional uppercase `OR`.
    fn parse_standard(trimmed: &str) -> QueryMode {
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        let has_or = words.contains(&"OR");

        if has_or {
            let segments: Vec<&str> = trimmed.split("OR").collect();
            let mut terms = Vec::new();
            for segment in segments {
                let segment_terms = Analyzer::analyze(segment.trim());
                terms.extend(segment_terms);
            }
            QueryMode::Standard(terms, true)
        } else {
            let terms = Analyzer::analyze(trimmed);
            QueryMode::Standard(terms, false)
        }
    }

    // -----------------------------------------------------------------------
    // Query execution strategies
    // -----------------------------------------------------------------------

    /// Execute a phrase query: find documents where all terms appear adjacent
    /// and in the specified order.
    fn phrase_search(&self, terms: &[String]) -> Vec<SearchResult> {
        if terms.is_empty() {
            return Vec::new();
        }

        if terms.len() == 1 {
            return self.standard_search(terms, false);
        }

        // Get postings for all terms
        let mut term_postings = Vec::with_capacity(terms.len());
        for term in terms {
            match self.index.get_postings(term) {
                Some(postings) => term_postings.push(postings),
                None => return Vec::new(), // Term not in index → phrase cannot match
            }
        }

        // Find candidate documents that contain ALL terms (set intersection)
        let mut candidate_docs: Option<HashSet<&str>> = None;
        for postings in &term_postings {
            let doc_ids: HashSet<&str> = postings.iter().map(|p| p.doc_id.as_str()).collect();
            candidate_docs = Some(match candidate_docs {
                Some(prev) => prev.intersection(&doc_ids).copied().collect(),
                None => doc_ids,
            });
        }

        let candidates = match candidate_docs {
            Some(docs) if !docs.is_empty() => docs,
            _ => return Vec::new(),
        };

        // For each candidate document, check positional adjacency
        let mut matching_doc_ids = Vec::new();

        for doc_id in &candidates {
            let mut doc_positions: Vec<&[u32]> = Vec::with_capacity(terms.len());
            for postings in &term_postings {
                if let Some(p) = postings.iter().find(|p| p.doc_id == *doc_id) {
                    doc_positions.push(&p.positions);
                }
            }

            if doc_positions.len() != terms.len() {
                continue;
            }

            // Check if there is any starting position for term 0 such that
            // term 1 is at pos+1, term 2 is at pos+2, etc.
            let first_positions = doc_positions[0];
            let mut phrase_matched = false;

            for &start_pos in first_positions {
                let mut sequence_holds = true;
                for (i, positions) in doc_positions.iter().enumerate().skip(1) {
                    let expected_pos = start_pos + i as u32;
                    if !positions.contains(&expected_pos) {
                        sequence_holds = false;
                        break;
                    }
                }
                if sequence_holds {
                    phrase_matched = true;
                    break;
                }
            }

            if phrase_matched {
                matching_doc_ids.push(doc_id.to_string());
            }
        }

        // Score the matching documents using BM25 across the phrase terms
        self.score_documents(&matching_doc_ids, terms)
    }

    /// Execute a prefix query: expand prefix to all matching terms, union results.
    fn prefix_search(&self, prefix: &str) -> Vec<SearchResult> {
        let matching_terms = self.index.get_terms_with_prefix(prefix);

        if matching_terms.is_empty() {
            return Vec::new();
        }

        let terms: Vec<String> = matching_terms.into_iter().map(|s| s.to_string()).collect();
        self.standard_search(&terms, true)
    }

    /// Execute a fuzzy query: find terms within edit distance, union results.
    fn fuzzy_search(&self, term: &str, max_distance: usize) -> Vec<SearchResult> {
        let all_terms = self.index.all_terms();
        let matched_terms = fuzzy::find_fuzzy_matches(term, &all_terms, max_distance);

        if matched_terms.is_empty() {
            return Vec::new();
        }

        let terms: Vec<String> = matched_terms.into_iter().map(|s| s.to_string()).collect();
        self.standard_search(&terms, true)
    }

    /// Execute a standard AND or OR query across the given terms.
    fn standard_search(&self, terms: &[String], is_or: bool) -> Vec<SearchResult> {
        if terms.is_empty() {
            return Vec::new();
        }

        let candidate_doc_ids = if is_or {
            self.or_doc_ids(terms)
        } else {
            self.and_doc_ids(terms)
        };

        let doc_id_vec: Vec<String> = candidate_doc_ids.into_iter().collect();
        self.score_documents(&doc_id_vec, terms)
    }

    /// Find doc IDs matching ALL terms (AND).
    fn and_doc_ids(&self, terms: &[String]) -> HashSet<DocId> {
        let mut candidate_doc_ids: Option<HashSet<DocId>> = None;

        for term in terms {
            let doc_ids_for_term: HashSet<DocId> = match self.index.get_postings(term) {
                Some(postings) => postings.iter().map(|p| p.doc_id.clone()).collect(),
                None => HashSet::new(),
            };

            candidate_doc_ids = Some(match candidate_doc_ids {
                Some(accumulated) => accumulated
                    .intersection(&doc_ids_for_term)
                    .cloned()
                    .collect(),
                None => doc_ids_for_term,
            });
        }

        candidate_doc_ids.unwrap_or_default()
    }

    /// Find doc IDs matching ANY term (OR).
    fn or_doc_ids(&self, terms: &[String]) -> HashSet<DocId> {
        let mut candidate_doc_ids: HashSet<DocId> = HashSet::new();

        for term in terms {
            if let Some(postings) = self.index.get_postings(term) {
                for p in postings {
                    candidate_doc_ids.insert(p.doc_id.clone());
                }
            }
        }

        candidate_doc_ids
    }

    /// Score candidate documents against a set of query terms using BM25.
    fn score_documents(&self, doc_ids: &[DocId], terms: &[String]) -> Vec<SearchResult> {
        let metadata = self.index.get_metadata();
        let total_docs = metadata.total_docs;
        let avg_doc_length = metadata.avg_doc_length();

        let mut doc_scores: HashMap<DocId, f64> = HashMap::new();

        for term in terms {
            let postings = match self.index.get_postings(term) {
                Some(p) => p,
                None => continue,
            };

            let doc_freq = postings.len() as u64;
            let idf = self.scorer.idf(doc_freq, total_docs);

            for posting in postings {
                if doc_ids.contains(&posting.doc_id) {
                    let doc_len = metadata
                        .doc_lengths
                        .get(&posting.doc_id)
                        .copied()
                        .unwrap_or(0);

                    let term_score = self.scorer.score(
                        posting.term_frequency,
                        doc_len,
                        avg_doc_length,
                        idf,
                    );

                    *doc_scores.entry(posting.doc_id.clone()).or_insert(0.0) += term_score;
                }
            }
        }

        let mut results: Vec<SearchResult> = doc_scores
            .into_iter()
            .map(|(doc_id, score)| {
                let title = self
                    .index
                    .get_document(&doc_id)
                    .map(|d| d.title.clone())
                    .unwrap_or_default();
                SearchResult {
                    doc_id,
                    title,
                    score,
                }
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }
}

/// Manages multiple named search engines for multi-index support.
pub struct EngineManager {
    base_data_dir: PathBuf,
    config: Config,
    indexes: HashMap<String, Arc<RwLock<SearchEngine>>>,
}

impl EngineManager {
    /// Initialize the manager with a base data directory and config.
    pub fn new(base_data_dir: PathBuf, config: Config) -> io::Result<Self> {
        let mut manager = Self {
            base_data_dir,
            config,
            indexes: HashMap::new(),
        };
        manager.load_existing_indexes()?;
        Ok(manager)
    }

    fn load_existing_indexes(&mut self) -> io::Result<()> {
        let indexes_dir = self.base_data_dir.join("indexes");
        if indexes_dir.exists() {
            for entry in fs::read_dir(&indexes_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let engine = SearchEngine::open_with_scorer(
                        &entry.path(),
                        self.config.index.flush_threshold,
                        BM25Scorer::with_params(self.config.scoring.bm25_k1, self.config.scoring.bm25_b),
                    )?;
                    self.indexes.insert(name, Arc::new(RwLock::new(engine)));
                }
            }
        }

        // Always ensure default index exists
        if !self.indexes.contains_key("default") {
            let default_dir = self.base_data_dir.join("indexes").join("default");
            let engine = SearchEngine::open_with_scorer(
                &default_dir,
                self.config.index.flush_threshold,
                BM25Scorer::with_params(self.config.scoring.bm25_k1, self.config.scoring.bm25_b),
            )?;
            self.indexes.insert("default".to_string(), Arc::new(RwLock::new(engine)));
        }

        Ok(())
    }

    /// Get a shared reference to a named engine.
    pub fn get(&self, name: &str) -> Option<Arc<RwLock<SearchEngine>>> {
        self.indexes.get(name).cloned()
    }

    /// Get an existing engine or create a new persistent named index.
    pub fn get_or_create(&mut self, name: &str) -> io::Result<Arc<RwLock<SearchEngine>>> {
        if let Some(engine) = self.indexes.get(name) {
            return Ok(Arc::clone(engine));
        }

        let dir = self.base_data_dir.join("indexes").join(name);
        let engine = SearchEngine::open_with_scorer(
            &dir,
            self.config.index.flush_threshold,
            BM25Scorer::with_params(self.config.scoring.bm25_k1, self.config.scoring.bm25_b),
        )?;
        let shared = Arc::new(RwLock::new(engine));
        self.indexes.insert(name.to_string(), Arc::clone(&shared));
        tracing::info!(index = %name, "Created new index");
        Ok(shared)
    }

    /// List all currently available index names.
    pub fn list_indexes(&self) -> Vec<String> {
        let mut names: Vec<String> = self.indexes.keys().cloned().collect();
        names.sort();
        names
    }

    /// Delete a named index from disk and memory.
    pub fn delete_index(&mut self, name: &str) -> io::Result<bool> {
        if name == "default" {
            // Re-initialize default index rather than removing the entry
            if let Some(engine) = self.indexes.get("default") {
                let default_dir = self.base_data_dir.join("indexes").join("default");
                if default_dir.exists() {
                    let _ = fs::remove_dir_all(&default_dir);
                }
                let mut engine = engine.write().unwrap();
                *engine = SearchEngine::open_with_scorer(
                    &default_dir,
                    self.config.index.flush_threshold,
                    BM25Scorer::with_params(self.config.scoring.bm25_k1, self.config.scoring.bm25_b),
                )?;
            }
            return Ok(true);
        }

        if self.indexes.remove(name).is_some() {
            let dir = self.base_data_dir.join("indexes").join(name);
            if dir.exists() {
                fs::remove_dir_all(&dir)?;
            }
            tracing::info!(index = %name, "Deleted index");
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
