# Phase 2 — Developer Guide

## Project Structure

```
mini-elasticsearch/
├── Cargo.toml          # Project manifest & dependencies
├── src/
│   ├── lib.rs          # Library root — module declarations
│   ├── analyzer.rs     # Text tokenizer & normalizer (unchanged from Phase 1)
│   ├── index.rs        # Inverted index — now with frequency-based postings
│   ├── scorer.rs       # BM25 relevance scoring (NEW)
│   ├── engine.rs       # SearchEngine facade — now returns ranked results
│   └── main.rs         # CLI demo program — shows BM25 scores
├── tests/
│   ├── analyzer_tests.rs  # Analyzer tests (unchanged from Phase 1)
│   ├── index_tests.rs     # Index tests — validates postings & metadata
│   ├── engine_tests.rs    # Engine tests — validates scored search
│   └── scorer_tests.rs    # Scorer tests — validates BM25 math (NEW)
└── docs/
    ├── mini_elasticsearch_rust.md   # Full project design doc
    ├── phase1_guide.md              # Phase 1 developer guide
    └── phase2_guide.md              # ← You are here
```

---

## What Changed from Phase 1

| Component | Phase 1 | Phase 2 |
|---|---|---|
| Postings | `HashSet<DocId>` (presence only) | `Vec<Posting>` (doc_id + term_frequency) |
| Metadata | Just `doc_count` | `IndexMetadata` with doc lengths, total length |
| Search return type | `Vec<DocId>` (unordered) | `Vec<SearchResult>` (doc_id + score, sorted) |
| Scoring | None | BM25 with configurable k1/b |
| New module | — | `scorer.rs` |

---

## What Each File Does

### `Cargo.toml`

Still uses **zero external dependencies**. Phase 2 is entirely built with the standard library.

```toml
[package]
name = "mini-elasticsearch"
version = "0.1.0"
edition = "2024"

[dependencies]
```

---

### `src/lib.rs`

Declares four public modules (one new from Phase 1):

```rust
pub mod analyzer;
pub mod engine;
pub mod index;
pub mod scorer;   // ← NEW in Phase 2
```

---

### `src/analyzer.rs` — Text Tokenizer & Normalizer

**Unchanged from Phase 1.** Splits text on non-alphanumeric boundaries, filters empty strings, lowercases tokens.

---

### `src/index.rs` — Inverted Index (Rewritten)

**Purpose:** Maps every term to a list of postings containing document ID **and term frequency**. Also tracks corpus-level metadata needed by BM25.

**New data structures:**

```
Posting:       { doc_id: DocId, term_frequency: u32 }
IndexMetadata: { total_docs, total_doc_length, doc_lengths: HashMap<DocId, u32> }
index:         HashMap<String, Vec<Posting>>   — term → postings with frequency
documents:     HashMap<DocId, String>          — doc_id → original text
```

**Key functions:**

| Function | What it does |
|---|---|
| `add_document(id, text)` | Analyzes text, counts term frequencies, stores postings & metadata |
| `get_postings(term)` | Returns the postings list for a term (if it exists) |
| `get_metadata()` | Returns corpus metadata (total docs, doc lengths, avg length) |
| `get_document(id)` | Retrieves original text by doc ID |
| `doc_count()` | Returns total number of indexed documents |

**What changed from Phase 1:**
- `add_document` now counts how many times each token appears (term frequency) instead of just recording presence.
- `IndexMetadata` tracks document lengths for BM25 length normalization.
- Document overwrites properly clean up old postings and metadata.
- `search_and` / `search_or` removed — scoring is now handled by the engine.

**Tests (15):** Postings existence, correct doc IDs, term frequency (single & multiple), metadata (doc lengths, total length, average), document overwrite, case insensitivity, empty documents.

---

### `src/scorer.rs` — BM25 Scorer (New)

**Purpose:** Computes document relevance scores using the Okapi BM25 ranking function.

**BM25 formula:**
```
BM25(term, doc) = IDF(term) × (tf × (k1 + 1)) / (tf + k1 × (1 - b + b × (dl / avgdl)))
```

Where:
- `tf` = term frequency in the document
- `dl` = document length (total tokens)
- `avgdl` = average document length across the corpus
- `k1` = 1.2 (controls term frequency saturation — diminishing returns for repeated terms)
- `b` = 0.75 (controls length normalization — penalizes long documents)

**IDF formula:**
```
IDF(term) = ln(total_docs / docs_containing_term)
```

**Key functions:**

| Function | What it does |
|---|---|
| `BM25Scorer::new()` | Creates scorer with default parameters (k1=1.2, b=0.75) |
| `BM25Scorer::with_params(k1, b)` | Creates scorer with custom parameters |
| `idf(doc_freq, total_docs)` | Computes Inverse Document Frequency |
| `score(tf, doc_length, avg_doc_length, idf)` | Computes BM25 score for one term in one document |

**Tests (13):** IDF calculation (basic, all docs, half docs, zero edge cases, rare vs common), BM25 score (positive, higher TF, TF saturation, shorter doc advantage, average length identity, custom params, zero IDF).

---

### `src/engine.rs` — SearchEngine Facade (Rewritten)

**Purpose:** The high-level public API. Wraps `InvertedIndex` and `BM25Scorer`, returns ranked search results.

**New data structure:**
```rust
pub struct SearchResult {
    pub doc_id: DocId,
    pub score: f64,
}
```

**Query parsing rules (same as Phase 1):**
- Default is **AND** — `"ogre bride"` finds docs with both terms
- If the query contains uppercase `OR` — `"ogre OR bride"` finds docs with either term

**Key difference from Phase 1:** `search()` now returns `Vec<SearchResult>` sorted by BM25 score (descending) instead of `Vec<DocId>`.

**How scoring works:**
1. For each query term, look up its postings list
2. Compute IDF for the term (based on how many docs contain it)
3. For each posting, compute BM25 score using TF, doc length, and avg doc length
4. Accumulate scores across all query terms per document
5. Apply AND/OR filtering (AND = only docs matching all terms; OR = any term)
6. Sort results by score, descending

**Key functions:**

| Function | What it does |
|---|---|
| `SearchEngine::new()` | Creates an empty engine with default BM25 params |
| `add_document(id, text)` | Indexes a document |
| `search(query)` | Parses query, scores with BM25, returns ranked results |
| `get_document(id)` | Retrieves document text |
| `doc_count()` | Returns total indexed docs |

**Tests (17):** Correct doc IDs returned, positive scores, descending sort, AND/OR search, no match, empty query, case insensitivity, rare term scores higher, higher TF scores higher, shorter doc scores higher, multi-term score accumulation, deliverable example.

---

### `src/main.rs` — CLI Demo

**Purpose:** A runnable demo that indexes 5 sample documents and runs several queries to demonstrate BM25 scoring.

---

## How to Check Changes

### View modified files with Git

```bash
# See which files changed
git status

# See a summary of changes (lines added/removed per file)
git diff --stat

# See the full diff
git diff

# See changes for a specific file
git diff src/index.rs
```

### View the new files

```bash
# List all source files
ls -la src/

# Read the new scorer module
cat src/scorer.rs

# Read the updated files
cat src/index.rs
cat src/engine.rs
cat src/main.rs
```

---

## How to Run & Verify

### 1. Run all tests

```bash
cargo test
```

**Expected output:** 57 tests pass (55 unit tests + 2 doc-tests)

```
running 10 tests          (analyzer_tests)   — 10 passed
running 17 tests          (engine_tests)     — 17 passed
running 15 tests          (index_tests)      — 15 passed
running 13 tests          (scorer_tests)     — 13 passed
Doc-tests:                                   —  2 passed

test result: ok. 57 passed; 0 failed; 0 ignored
```

### 2. Run a specific test suite

```bash
# Run only scorer tests
cargo test scorer

# Run only index tests
cargo test index

# Run only engine tests
cargo test engine

# Run a single test by name
cargo test test_deliverable_example
cargo test test_bm25_tf_saturation
```

### 3. Run the demo

```bash
cargo run
```

**Expected output:**

```
=== Mini Elasticsearch — Phase 2 (BM25 Scoring) ===

Indexed 5 documents.

Search: "ogre"
  [ 1] score: 0.9711  "The ogre kidnapped the bride"
  [ 3] score: 0.9035  "The bride escaped from the ogre"

AND search: "ogre bride"
  [ 1] score: 1.5125  "The ogre kidnapped the bride"
  [ 3] score: 1.4073  "The bride escaped from the ogre"

Search: "bride"
  [ 1] score: 0.5414  "The ogre kidnapped the bride"
  [ 3] score: 0.5037  "The bride escaped from the ogre"
  [ 4] score: 0.4422  "The knight rescued the bride from the dragon"

OR search: "ogre OR dragon"
  [ 2] score: 0.9711  "The dragon burned the village"
  [ 1] score: 0.9711  "The ogre kidnapped the bride"
  [ 3] score: 0.9035  "The bride escaped from the ogre"
  [ 4] score: 0.7932  "The knight rescued the bride from the dragon"

Search: "wizard"
  (no matches)
```

**Things to observe:**
- Doc 1 ("The ogre kidnapped the bride") scores higher than doc 3 ("The bride escaped from the ogre") for "ogre" because doc 1 is shorter (5 tokens vs 6 tokens), so BM25's length normalization gives it a slight boost.
- "bride" appears in 3 documents, so its IDF is lower than "ogre" (which appears in only 2). That's why individual "bride" scores are lower than "ogre" scores.
- In the AND search for "ogre bride", scores are the **sum** of both terms' BM25 scores, so they're higher.
- Doc 4 ("The knight rescued the bride from the dragon") is the longest document (8 tokens), so it consistently scores lowest for any given term.

### 4. Build without running

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release
```

### 5. Check for compiler warnings

```bash
cargo clippy
```

---

## Quick Verification Checklist

| Check | Command | Expected |
|---|---|---|
| Project compiles | `cargo build` | No errors |
| All tests pass | `cargo test` | 57/57 pass |
| Demo runs correctly | `cargo run` | Output shows scores |
| No warnings | `cargo clippy` | No warnings |
| Results are scored | `cargo test test_search_results_have_positive_scores` | Pass |
| Results are sorted | `cargo test test_search_results_sorted_by_score_descending` | Pass |
| Rare terms rank higher | `cargo test test_rare_term_scores_higher` | Pass |
| Higher TF ranks higher | `cargo test test_higher_tf_scores_higher` | Pass |
| Length normalization works | `cargo test test_shorter_doc_scores_higher` | Pass |
| BM25 TF saturation | `cargo test test_bm25_tf_saturation` | Pass |
| Deliverable example | `cargo test test_deliverable_example` | Pass |
