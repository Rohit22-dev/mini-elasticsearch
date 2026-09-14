# Phase 4 — Developer Guide

## Project Structure

```
mini-elasticsearch/
├── Cargo.toml          # Project manifest & dependencies (unchanged)
├── src/
│   ├── lib.rs          # Library root — module declarations (fuzzy added)
│   ├── analyzer.rs     # Text tokenizer & normalizer (positional analysis added)
│   ├── index.rs        # Inverted index — BTreeMap, positional postings, prefix lookup
│   ├── scorer.rs       # BM25 relevance scoring (unchanged from Phase 2)
│   ├── engine.rs       # SearchEngine facade — phrase, prefix, fuzzy, standard search
│   ├── fuzzy.rs        # Levenshtein distance & fuzzy term matching (NEW)
│   ├── server.rs       # HTTP API with axum — 5 REST endpoints (unchanged)
│   └── main.rs         # Async main — starts HTTP server on port 3000
├── tests/
│   ├── analyzer_tests.rs  # Analyzer tests (unchanged)
│   ├── index_tests.rs     # Index tests — positions, prefix lookup (updated)
│   ├── engine_tests.rs    # Engine tests — phrase, prefix, fuzzy (updated)
│   ├── scorer_tests.rs    # Scorer tests (unchanged from Phase 2)
│   ├── fuzzy_tests.rs     # Levenshtein & fuzzy matching tests (NEW)
│   └── server_tests.rs    # HTTP integration tests — new query types (updated)
└── docs/
    ├── mini_elasticsearch_rust.md   # Full project design doc
    ├── phase1_guide.md              # Phase 1 developer guide
    ├── phase2_guide.md              # Phase 2 developer guide
    ├── phase3_guide.md              # Phase 3 developer guide
    └── phase4_guide.md              # ← You are here
```

---

## What Changed from Phase 3

| Component | Phase 3 | Phase 4 |
|---|---|---|
| Posting struct | `{ doc_id, term_frequency }` | `{ doc_id, term_frequency, positions }` |
| Index backing store | `HashMap<String, Vec<Posting>>` | `BTreeMap<String, Vec<Posting>>` |
| Analyzer | `analyze(text)` only | Added `analyze_with_positions(text, offset)` |
| Query modes | AND / OR only | + Phrase `"..."`, Prefix `*`, Fuzzy `~N` |
| Fuzzy module | None | `levenshtein()`, `find_fuzzy_matches()` |
| Index methods | get_postings, get_document | + `get_terms_with_prefix()`, `all_terms()` |
| Dependencies | axum, tokio, serde, serde_json | Unchanged (no new deps) |
| HTTP API | 5 endpoints | Unchanged (query syntax handled by engine) |

---

## What Each File Does

### `analyzer.rs`

Added `analyze_with_positions(text, offset)` — returns `Vec<(String, u32)>` pairing each token with its 0-based position starting from `offset`. The title is analyzed with offset 0, and the body continues from where the title left off, giving continuous positions across the combined document.

```rust
Analyzer::analyze_with_positions("The ogre kidnapped", 0)
// → [("the", 0), ("ogre", 1), ("kidnapped", 2)]

Analyzer::analyze_with_positions("the bride", 3)
// → [("the", 3), ("bride", 4)]
```

The original `analyze()` method is unchanged for backward compatibility.

---

### `index.rs`

#### Positional Postings

`Posting` now includes a `positions: Vec<u32>` field recording where each term appears in the combined title+body token stream:

```rust
pub struct Posting {
    pub doc_id: DocId,
    pub term_frequency: u32,
    pub positions: Vec<u32>,  // NEW — 0-based token offsets
}
```

#### BTreeMap Index

The term index was switched from `HashMap` to `BTreeMap`:

```rust
index: BTreeMap<String, Vec<Posting>>,
```

This keeps terms in sorted order, enabling efficient prefix range queries without any additional data structures.

#### New Methods

| Method | Purpose |
|---|---|
| `get_terms_with_prefix(prefix)` | Uses `BTreeMap::range()` to find all terms starting with a prefix. O(log n + k) where k is the number of matches. |
| `all_terms()` | Returns all unique terms in the index. Used by fuzzy search for brute-force scanning. |

#### Positional Indexing

`add_document` now uses `analyze_with_positions` to record positions:

```text
Document: title="Ogre Story", body="The ogre kidnapped the bride"

Title tokens:  ogre=0, story=1
Body tokens:   the=2, ogre=3, kidnapped=4, the=5, bride=6

Postings for "ogre": { doc_id: "1", tf: 2, positions: [0, 3] }
```

---

### `fuzzy.rs` (NEW)

Two public functions:

#### `levenshtein(a, b) -> usize`

Standard dynamic-programming edit distance. Counts the minimum number of single-character insertions, deletions, or substitutions to transform `a` into `b`.

```rust
levenshtein("ogr", "ogre")  // → 1 (insert 'e')
levenshtein("ogre", "ogra") // → 1 (substitute 'e' → 'a')
levenshtein("kitten", "sitting") // → 3
```

#### `find_fuzzy_matches(query_term, all_terms, max_distance) -> Vec<&str>`

Brute-force scan: checks every term in the index and returns those within `max_distance` edits of `query_term`. For small-to-medium indexes this is perfectly adequate.

---

### `engine.rs`

#### Query Mode Detection

`parse_query` now returns a `QueryMode` enum instead of `(Vec<String>, bool)`:

```rust
enum QueryMode {
    Phrase(Vec<String>),          // "ogre bride"
    Prefix(String),               // ogr*
    Fuzzy(String, usize),         // ogr~1
    Standard(Vec<String>, bool),  // ogre bride / ogre OR bride
}
```

Detection rules (checked in order):

1. **Quoted string** → Phrase: `"ogre kidnapped"` strips quotes, analyzes inner text
2. **Trailing `*`** → Prefix: `ogr*` extracts the prefix
3. **Contains `~`** → Fuzzy: `ogr~1` extracts term and distance (default: 1)
4. **Everything else** → Standard AND/OR (same as Phase 3)

#### Phrase Search

For each document containing all phrase terms, checks positional adjacency:

```text
Query: "ogre kidnapped"
Terms: [ogre, kidnapped]

Doc 1 positions:
  ogre → [0, 3]
  kidnapped → [4]

Check: Is there a start position p where ogre=p and kidnapped=p+1?
  p=3: ogre at 3 ✓, kidnapped at 4 = 3+1 ✓ → MATCH

Doc 3 positions:
  ogre → [7]
  kidnapped → not present → NO MATCH
```

Scoring: Sum of BM25 scores for each phrase term.

#### Prefix Search

1. Call `index.get_terms_with_prefix(prefix)` to expand the prefix
2. Union all postings from matching terms
3. Score each document with BM25 (scores accumulate across matching terms)

#### Fuzzy Search

1. Call `index.all_terms()` to get all terms
2. Call `fuzzy::find_fuzzy_matches(term, all_terms, max_distance)` to find candidates
3. Union postings from matching terms
4. Score with BM25

---

### `server.rs` (Unchanged)

No changes needed. The `GET /search?q=...` endpoint passes the raw query string to `engine.search()`, which handles all query syntax detection internally.

---

## Query Syntax Reference

| Syntax | Example | Behavior |
|---|---|---|
| Plain terms | `ogre bride` | AND — docs must contain both terms |
| OR operator | `ogre OR bride` | OR — docs containing either term |
| Phrase (quoted) | `"ogre kidnapped"` | Docs where terms appear adjacent, in order |
| Prefix (trailing `*`) | `ogr*` | Expand to all terms starting with `ogr` |
| Fuzzy (trailing `~N`) | `ogr~1` | Terms within edit distance 1 of `ogr` |
| Fuzzy (default) | `ogr~` | Same as `ogr~1` (default distance = 1) |
| Fuzzy (exact) | `ogre~0` | Only exact matches (same as plain search) |

---

## Examples

```bash
# Start the server
cargo run

# Index test documents
curl -X POST http://localhost:3000/documents \
  -H "Content-Type: application/json" \
  -d '{"id": "1", "title": "Ogre Story", "body": "The ogre kidnapped the bride"}'

curl -X POST http://localhost:3000/documents \
  -H "Content-Type: application/json" \
  -d '{"id": "2", "title": "Dragon Tale", "body": "The dragon burned the village"}'

curl -X POST http://localhost:3000/documents \
  -H "Content-Type: application/json" \
  -d '{"id": "3", "title": "Escape Story", "body": "The bride escaped from the ogre"}'

# Phrase search — only doc 1 has "ogre kidnapped" adjacent
curl "http://localhost:3000/search?q=%22ogre+kidnapped%22"

# Prefix search — matches "ogre" in docs 1 and 3
curl "http://localhost:3000/search?q=ogr*"

# Fuzzy search — "ogr" is edit distance 1 from "ogre"
curl "http://localhost:3000/search?q=ogr~1"

# Standard AND search
curl "http://localhost:3000/search?q=ogre+bride"

# Standard OR search
curl "http://localhost:3000/search?q=ogre+OR+dragon"
```

---

## Test Summary

Run all tests:

```bash
cargo test
```

| Test file | Count | What's tested |
|---|---|---|
| `analyzer_tests.rs` | 10 | Tokenization, casing, punctuation, Unicode |
| `index_tests.rs` | 25 | Postings, metadata, delete, positions, prefix lookup |
| `engine_tests.rs` | 38 | AND/OR, BM25 properties, phrase, prefix, fuzzy search |
| `fuzzy_tests.rs` | 13 | Levenshtein distance, fuzzy matching |
| `scorer_tests.rs` | 13 | IDF, BM25 score properties |
| `server_tests.rs` | 16 | HTTP endpoints, phrase/prefix/fuzzy via HTTP |
| Doc-tests | 4 | Inline examples in analyzer, engine, fuzzy |
| **Total** | **119** | |
