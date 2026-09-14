# Phase 1 — Developer Guide

## Project Structure

```
mini-elasticsearch/
├── Cargo.toml          # Project manifest & dependencies
├── src/
│   ├── lib.rs          # Library root — module declarations
│   ├── analyzer.rs     # Text tokenizer & normalizer
│   ├── index.rs        # In-memory inverted index
│   ├── engine.rs       # SearchEngine facade (public API)
│   └── main.rs         # CLI demo program
└── docs/
    ├── mini_elasticsearch_rust.md   # Full project design doc
    └── phase1_guide.md              # ← You are here
```

---

## What Each File Does

### `Cargo.toml`

The Rust project manifest. Phase 1 uses **zero external dependencies** — everything is built with the standard library.

```toml
[package]
name = "mini-elasticsearch"
version = "0.1.0"
edition = "2024"

[dependencies]
```

---

### `src/lib.rs`

The library entry point. Declares the three public modules so they can be used by `main.rs` and by external consumers (e.g., tests, future phases).

```rust
pub mod analyzer;
pub mod engine;
pub mod index;
```

---

### `src/analyzer.rs` — Text Tokenizer & Normalizer

**Purpose:** Converts raw text into a list of normalized, searchable tokens.

**Pipeline:**
1. **Split** on any non-alphanumeric character (`!`, `.`, `,`, spaces, etc.)
2. **Filter** out empty strings (from consecutive delimiters like `---`)
3. **Lowercase** every token

**Example:**
```
Input:  "The Ogre kidnapped the Bride!"
Output: ["the", "ogre", "kidnapped", "the", "bride"]
```

**Key function:**
```rust
Analyzer::analyze(text: &str) -> Vec<String>
```

**Tests (10):** Basic tokenization, lowercasing, punctuation stripping, unicode, empty input, numeric tokens, mixed alphanumeric, multiple delimiters.

---

### `src/index.rs` — In-Memory Inverted Index

**Purpose:** Maps every term to the set of document IDs that contain it. Also stores original document text for retrieval.

**Data structures:**
```
index:     HashMap<String, HashSet<DocId>>   — term → {doc_id, doc_id, ...}
documents: HashMap<DocId, String>            — doc_id → original text
doc_count: u64                               — total documents indexed
```

**Key functions:**

| Function | What it does |
|---|---|
| `add_document(id, text)` | Analyzes text, inserts each term → doc_id mapping |
| `search_and(terms)` | Returns docs containing ALL terms (intersection) |
| `search_or(terms)` | Returns docs containing ANY term (union) |
| `get_document(id)` | Retrieves original text by doc ID |
| `doc_count()` | Returns total number of indexed documents |

**How boolean search works:**
- **AND** — Start with the postings set of the first term, intersect with each subsequent term's set. If any term is missing, return empty.
- **OR** — Union all postings sets together.

**Tests (11):** Single/multi-term AND, single/multi-term OR, no overlap, nonexistent term, document retrieval, empty query, duplicate doc IDs, case insensitivity.

---

### `src/engine.rs` — SearchEngine Facade

**Purpose:** The high-level public API. Wraps `InvertedIndex` and adds query parsing.

**Query parsing rules:**
- Default is **AND** — `"ogre bride"` finds docs with both terms
- If the query contains uppercase `OR` — `"ogre OR bride"` finds docs with either term

**Key functions:**

| Function | What it does |
|---|---|
| `SearchEngine::new()` | Creates an empty engine |
| `add_document(id, text)` | Indexes a document |
| `search(query)` | Parses query, runs AND or OR search |
| `get_document(id)` | Retrieves document text |
| `doc_count()` | Returns total indexed docs |

**Tests (12):** AND/OR search, case insensitivity, empty query, nonexistent terms, the exact deliverable example from the design doc.

---

### `src/main.rs` — CLI Demo

**Purpose:** A runnable demo that indexes 5 sample documents and runs several queries to demonstrate AND and OR search.

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
git diff src/analyzer.rs
```

### View the new files

```bash
# List all source files
ls -la src/

# Read any file
cat src/analyzer.rs
cat src/index.rs
cat src/engine.rs
cat src/lib.rs
cat src/main.rs
```

---

## How to Run & Verify

### 1. Run all tests

```bash
cargo test
```

**Expected output:** 35 tests pass (33 unit tests + 2 doc-tests)

```
running 33 tests
test analyzer::tests::test_basic_tokenization ... ok
test analyzer::tests::test_lowercasing ... ok
test analyzer::tests::test_punctuation_stripping ... ok
...
test engine::tests::test_deliverable_example ... ok
...
test index::tests::test_multi_term_and_search ... ok
...

test result: ok. 33 passed; 0 failed; 0 ignored

   Doc-tests mini_elasticsearch
test result: ok. 2 passed; 0 failed; 0 ignored
```

### 2. Run a specific test

```bash
# Run only analyzer tests
cargo test analyzer

# Run only index tests
cargo test index

# Run only engine tests
cargo test engine

# Run a single test by name
cargo test test_deliverable_example
```

### 3. Run the demo

```bash
cargo run
```

**Expected output:**

```
=== Mini Elasticsearch — Phase 1 ===

Indexed 5 documents.

AND search: "ogre bride"
  → doc IDs: [1, 3]
  [1] The ogre kidnapped the bride
  [3] The bride escaped from the ogre

AND search: "dragon village"
  → doc IDs: [2]
  [2] The dragon burned the village

OR search: "ogre OR dragon"
  → doc IDs: [1, 2, 3, 4]
  [1] The ogre kidnapped the bride
  [2] The dragon burned the village
  [3] The bride escaped from the ogre
  [4] The knight rescued the bride from the dragon

AND search: "wizard"
  → doc IDs: [] (no matches)
```

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
| All tests pass | `cargo test` | 35/35 pass |
| Demo runs correctly | `cargo run` | Output matches above |
| No warnings | `cargo clippy` | No warnings |
| AND search works | `cargo test test_and_search` | Pass |
| OR search works | `cargo test test_or_search` | Pass |
| Deliverable example | `cargo test test_deliverable_example` | Pass |
