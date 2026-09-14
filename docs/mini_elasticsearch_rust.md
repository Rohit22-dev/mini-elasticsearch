# Mini Elasticsearch — Rust

Build a small but real search engine from scratch. Not a toy — a system that tokenizes text, builds an inverted index, scores documents with BM25, persists data to disk, and serves concurrent queries over HTTP.

---

## Why This Project?

| What you'll learn | How it maps to the real world |
|---|---|
| Text tokenization & normalization | Every search engine, NLP pipeline, LLM tokenizer |
| Inverted index data structures | Elasticsearch, Lucene, Solr, Meilisearch |
| TF-IDF and BM25 scoring | Google, Bing, any ranked retrieval system |
| Segment-based storage & merge | LSM trees (RocksDB, LevelDB, Cassandra) |
| Memory-mapped I/O | High-perf databases, OS page cache |
| Async Rust / concurrency | Any production Rust service |

---

## High-Level Architecture

```text
                         ┌──────────────────────┐
                         │      HTTP API         │
                         │  (axum / actix-web)   │
                         └──────────┬───────────┘
                                    │
                    ┌───────────────┼───────────────┐
                    ▼               ▼               ▼
             POST /documents   GET /search    DELETE /documents/:id
                    │               │               │
                    ▼               ▼               ▼
                         ┌──────────────────────┐
                         │    Engine Core        │
                         │                      │
                         │  ┌────────────────┐  │
                         │  │  Analyzer       │  │
                         │  │  (tokenize +    │  │
                         │  │   normalize)    │  │
                         │  └───────┬────────┘  │
                         │          │           │
                         │  ┌───────▼────────┐  │
                         │  │ Inverted Index  │  │
                         │  │ term → postings │  │
                         │  └───────┬────────┘  │
                         │          │           │
                         │  ┌───────▼────────┐  │
                         │  │  Scorer         │  │
                         │  │  (BM25)         │  │
                         │  └───────┬────────┘  │
                         │          │           │
                         │  ┌───────▼────────┐  │
                         │  │  Storage        │  │
                         │  │  (segments +    │  │
                         │  │   merge)        │  │
                         │  └────────────────┘  │
                         └──────────────────────┘
```

---

## Project Phases

The project is split into **7 phases**, each building on the last. Every phase produces a working system — you never have a "broken" project mid-way.

---

## Phase 1 — Tokenization & In-Memory Inverted Index

**Goal:** Accept raw text, split it into tokens, build an in-memory inverted index, and answer simple term queries.

### 1.1 Rust Setup & Basics

If this is your first Rust project, start here:

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Create the project
cargo init mini-elasticsearch
cd mini-elasticsearch
```

Spend a few hours with these concepts before writing the search engine:

- **Ownership & borrowing** — The single most important Rust concept. Read [Chapter 4 of The Rust Book](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html).
- **Structs & enums** — Your data modeling tools.
- **`String` vs `&str`** — You'll hit this immediately when tokenizing text.
- **`HashMap`** — The backing store for your inverted index.
- **`Vec`** — Your postings lists.
- **`Option` and `Result`** — Rust's error handling.
- **Iterators** — `.split_whitespace()`, `.map()`, `.filter()`, `.collect()`.

### 1.2 Analyzer (Tokenizer + Normalizer)

The analyzer converts raw text into a list of searchable terms.

```text
"The Ogre kidnapped the Bride!" → ["the", "ogre", "kidnapped", "the", "bride"]
```

**Steps:**

1. **Tokenize** — Split on whitespace and punctuation.
2. **Lowercase** — "Ogre" → "ogre".
3. **Strip punctuation** — "Bride!" → "bride".
4. **Remove stop words** (later) — drop "the", "a", "is", etc.
5. **Stem** (later) — "kidnapped" → "kidnap".

**Code sketch:**

```rust
pub struct Analyzer;

impl Analyzer {
    pub fn analyze(text: &str) -> Vec<String> {
        text.split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_lowercase())
            .collect()
    }
}
```

**Things to learn while building this:**

- How `&str` slicing works.
- The difference between `char` and byte iteration in Rust.
- Why `.to_lowercase()` returns a `String`, not a `&str`.

### 1.3 Inverted Index

The inverted index maps every term to the list of document IDs that contain it.

```text
"ogre"      → [1, 4, 8, 20]
"bride"     → [2, 4, 10]
"kidnapped" → [1, 4]
```

**Data structure:**

```rust
use std::collections::{HashMap, HashSet};

pub type DocId = u64;

pub struct InvertedIndex {
    /// term → set of document IDs containing that term
    index: HashMap<String, HashSet<DocId>>,
    /// doc_id → original document text (for retrieval)
    documents: HashMap<DocId, String>,
    /// total number of documents indexed
    doc_count: u64,
}
```

**Operations to implement:**

| Method | Purpose |
|---|---|
| `add_document(id, text)` | Tokenize text, insert each term → doc_id mapping |
| `search(query) → Vec<DocId>` | Tokenize query, intersect/union postings lists |
| `get_document(id) → Option<&str>` | Retrieve original text by ID |

**Key learning:** This is where you first feel Rust's ownership rules. The index owns the `String` keys, and you'll need to decide whether to clone strings or use references.

### 1.4 Boolean Search

Support `AND` and `OR` queries:

- `ogre bride` → documents containing **both** "ogre" AND "bride" (intersection).
- `ogre OR bride` → documents containing **either** (union).

**Implementation:** Postings lists are `HashSet<DocId>`, so intersection and union are built-in:

```rust
// AND: intersection
let result = set_a.intersection(&set_b).cloned().collect();

// OR: union
let result = set_a.union(&set_b).cloned().collect();
```

### Deliverable — Phase 1

```rust
fn main() {
    let mut engine = SearchEngine::new();
    engine.add_document(1, "The ogre kidnapped the bride");
    engine.add_document(2, "The dragon burned the village");
    engine.add_document(3, "The bride escaped from the ogre");

    let results = engine.search("ogre bride");
    // → [1, 3]  (docs containing BOTH terms)
}
```

You should be able to `cargo run` and see correct results. Write unit tests with `#[test]`.

---

## Phase 2 — Scoring: TF-IDF and BM25

**Goal:** Rank results by relevance instead of returning unordered doc IDs.

### 2.1 Term Frequency (TF)

How often does the term appear in a specific document?

```text
TF(term, doc) = count(term in doc) / total_terms_in_doc
```

**Change required:** Your postings list now needs to store term frequency, not just presence.

```rust
/// A single posting: which document, how many times the term appeared
pub struct Posting {
    pub doc_id: DocId,
    pub term_frequency: u32,
}

// index becomes:
index: HashMap<String, Vec<Posting>>,
```

### 2.2 Inverse Document Frequency (IDF)

How rare is this term across all documents? Rare terms are more informative.

```text
IDF(term) = ln(total_docs / docs_containing_term)
```

A term that appears in every document has IDF ≈ 0 (useless for ranking). A term that appears in only one document has high IDF.

### 2.3 TF-IDF Score

```text
score(term, doc) = TF(term, doc) × IDF(term)
```

For a multi-term query, sum the TF-IDF scores:

```text
score(query, doc) = Σ TF-IDF(term, doc) for each term in query
```

### 2.4 BM25 (the real scoring function)

TF-IDF has a problem: long documents get unfairly boosted because they naturally contain more term occurrences. BM25 fixes this with saturation and length normalization.

```text
BM25(term, doc) = IDF(term) × (tf × (k1 + 1)) / (tf + k1 × (1 - b + b × (dl / avgdl)))
```

Where:
- `tf` = term frequency in the document
- `dl` = document length (total terms)
- `avgdl` = average document length across corpus
- `k1` = 1.2 (controls term frequency saturation)
- `b` = 0.75 (controls length normalization)

**You need to store additional metadata:**

```rust
pub struct IndexMetadata {
    pub total_docs: u64,
    pub total_doc_length: u64,           // sum of all document lengths
    pub doc_lengths: HashMap<DocId, u32>, // length of each document
}

impl IndexMetadata {
    pub fn avg_doc_length(&self) -> f64 {
        self.total_doc_length as f64 / self.total_docs as f64
    }
}
```

### Deliverable — Phase 2

```rust
let results = engine.search("ogre");
// Returns:
// [
//   { doc_id: 3, score: 0.82 },
//   { doc_id: 1, score: 0.71 },
// ]
// Sorted by BM25 score, descending
```

---

## Phase 3 — HTTP API

**Goal:** Wrap the engine in an HTTP server so it behaves like a real search service.

### 3.1 Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
axum = "0.8"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
```

### 3.2 API Design

| Endpoint | Method | Purpose |
|---|---|---|
| `/documents` | POST | Index a new document |
| `/documents/:id` | GET | Retrieve a document by ID |
| `/documents/:id` | DELETE | Delete a document |
| `/search?q=...` | GET | Search and return ranked results |
| `/_stats` | GET | Return index statistics |

### 3.3 Request/Response Schemas

**Index a document:**

```http
POST /documents
Content-Type: application/json

{
  "id": "abc-123",
  "title": "The Ogre's Castle",
  "body": "The ogre kidnapped the bride and took her to the castle."
}
```

Response:

```json
{
  "status": "indexed",
  "id": "abc-123"
}
```

**Search:**

```http
GET /search?q=ogre+bride&limit=10
```

Response:

```json
{
  "query": "ogre bride",
  "total_hits": 2,
  "hits": [
    { "id": "abc-123", "score": 0.82, "title": "The Ogre's Castle" },
    { "id": "def-456", "score": 0.71, "title": "The Bride's Escape" }
  ],
  "took_ms": 3
}
```

### 3.4 Shared State

The search engine must be shared across async request handlers. In Rust, this means wrapping it in an `Arc<RwLock<...>>`:

```rust
use std::sync::{Arc, RwLock};

type SharedEngine = Arc<RwLock<SearchEngine>>;
```

- `Arc` = thread-safe reference counting (shared ownership across async tasks).
- `RwLock` = multiple readers OR one writer at a time.

This is where Rust's concurrency model really shines — the compiler prevents data races at compile time.

### Deliverable — Phase 3

```bash
# Index a document
curl -X POST http://localhost:3000/documents \
  -H "Content-Type: application/json" \
  -d '{"id": "1", "title": "Ogre Story", "body": "The ogre kidnapped the bride"}'

# Search
curl "http://localhost:3000/search?q=ogre"
```

---

## Phase 4 — Advanced Query Features

**Goal:** Support phrase search, prefix search, and fuzzy search.

### 4.1 Phrase Search

`"ogre bride"` (quoted) should only match documents where "ogre" appears directly before "bride".

**Requires positional information.** Extend your posting to include positions:

```rust
pub struct Posting {
    pub doc_id: DocId,
    pub term_frequency: u32,
    pub positions: Vec<u32>, // positions where the term appears
}
```

During indexing:

```text
"The ogre kidnapped the bride"
 pos:  0    1       2      3    4

"ogre" → { doc_id: 1, positions: [1] }
"bride" → { doc_id: 1, positions: [4] }
```

Phrase matching: for each pair of adjacent query terms, check that `position[term_i+1] = position[term_i] + 1`.

### 4.2 Prefix Search

`ogr*` matches "ogre", "ogres", "ogrish".

**Implementation options:**

1. **Brute force:** Iterate all terms, check `term.starts_with(prefix)`. Simple, O(n).
2. **Trie:** Build a trie alongside the inverted index. O(prefix length) lookup.
3. **Sorted terms + binary search:** Keep terms in a `BTreeMap`, use `range()` to find all terms starting with the prefix.

Start with option 3 — it's idiomatic Rust:

```rust
use std::collections::BTreeMap;

let index: BTreeMap<String, Vec<Posting>> = BTreeMap::new();

// Prefix search for "ogr"
let results: Vec<_> = index
    .range("ogr".to_string().."ogs".to_string())
    .flat_map(|(_, postings)| postings)
    .collect();
```

### 4.3 Fuzzy Search

`ogr` matches "ogre" (edit distance 1).

**Levenshtein distance:** The minimum number of single-character insertions, deletions, or substitutions needed to transform one string into another.

```text
distance("ogr", "ogre") = 1   (insert 'e')
distance("ogr", "ore")  = 1   (substitute 'g' → 'e')
distance("ogr", "cat")  = 3   (all different)
```

**Implementation:** Build a Levenshtein automaton or brute-force check all terms within a given edit distance. Start brute-force, optimize later.

```rust
pub fn levenshtein(a: &str, b: &str) -> usize {
    let n = a.len();
    let m = b.len();
    let mut dp = vec![vec![0usize; m + 1]; n + 1];

    for i in 0..=n { dp[i][0] = i; }
    for j in 0..=m { dp[0][j] = j; }

    for (i, ca) in a.chars().enumerate() {
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            dp[i + 1][j + 1] = (dp[i][j] + cost)
                .min(dp[i + 1][j] + 1)
                .min(dp[i][j + 1] + 1);
        }
    }

    dp[n][m]
}
```

### Deliverable — Phase 4

```bash
# Phrase search
curl "http://localhost:3000/search?q=%22ogre+bride%22"

# Prefix search
curl "http://localhost:3000/search?q=ogr*"

# Fuzzy search
curl "http://localhost:3000/search?q=ogr~1"
```

---

## Phase 5 — Persistence & Segment Storage

**Goal:** Data survives restarts. Introduce a segment-based storage model similar to Lucene.

### 5.1 Why Segments?

Writing to a single file on every index operation is slow. Instead:

1. Buffer documents in memory.
2. When the buffer reaches a threshold (e.g., 1000 docs), **flush** it to disk as an immutable **segment**.
3. Searches query all segments and merge results.
4. A background process **merges** small segments into larger ones.

```text
Memory Buffer
    │
    │  flush (every N docs)
    ▼
┌──────────┐ ┌──────────┐ ┌──────────┐
│ Segment 0│ │ Segment 1│ │ Segment 2│  ← immutable files on disk
└──────────┘ └──────────┘ └──────────┘
    │              │              │
    └──────────────┼──────────────┘
                   │  merge
                   ▼
            ┌──────────────┐
            │  Segment 0'  │  ← merged, larger segment
            └──────────────┘
```

### 5.2 Segment File Format

Design a binary format. A simple one:

```text
┌─────────────────────────────────┐
│           Header                │
│  magic bytes: "MNSE"            │
│  version: u32                   │
│  num_terms: u32                 │
│  num_docs: u32                  │
├─────────────────────────────────┤
│         Term Dictionary         │
│  For each term:                 │
│    term_length: u16             │
│    term_bytes: [u8]             │
│    postings_offset: u64         │
│    postings_length: u32         │
├─────────────────────────────────┤
│         Postings Lists          │
│  For each term:                 │
│    num_postings: u32            │
│    For each posting:            │
│      doc_id: u64                │
│      term_freq: u32             │
│      num_positions: u32         │
│      positions: [u32]           │
├─────────────────────────────────┤
│         Document Store          │
│  For each document:             │
│    doc_id: u64                  │
│    doc_length: u32              │
│    title_length: u16            │
│    title_bytes: [u8]            │
│    body_length: u32             │
│    body_bytes: [u8]             │
└─────────────────────────────────┘
```

### 5.3 Write-Ahead Log (WAL)

To prevent data loss between flushes, write every incoming document to an append-only WAL file:

```text
[timestamp][doc_id_len][doc_id][doc_len][document_json]\n
```

On startup:
1. Load all committed segments.
2. Replay the WAL to recover any unflushed documents.
3. Truncate the WAL after the next flush.

### 5.4 Segment Merge

Merging is critical for performance — too many small segments slow down queries.

**Tiered merge policy** (simplified):
- Group segments by size tier (e.g., <1MB, <10MB, <100MB).
- When a tier has more than N segments, merge them into one.

**Key learning:** This is the same merge strategy used by LSM-tree databases (LevelDB, RocksDB, Cassandra). Building it here gives you deep intuition for how those systems work.

### Deliverable — Phase 5

```bash
# Index 10,000 documents
for i in $(seq 1 10000); do
  curl -s -X POST http://localhost:3000/documents \
    -H "Content-Type: application/json" \
    -d "{\"id\": \"$i\", \"title\": \"doc $i\", \"body\": \"some searchable text $i\"}"
done

# Restart the server — data should still be there
cargo run

# Verify
curl "http://localhost:3000/_stats"
# → { "total_docs": 10000, "segments": 10, ... }
```

---

## Phase 6 — Memory-Mapped I/O & Performance

**Goal:** Use `mmap` to let the OS manage which segments are in memory, enabling the engine to handle indexes larger than RAM.

### 6.1 What is mmap?

Instead of `read()`-ing file bytes into a buffer you allocated, `mmap` maps the file directly into your process's virtual address space. The OS loads pages on demand and caches them via the page cache.

```rust
use memmap2::Mmap;
use std::fs::File;

let file = File::open("segment_0.idx")?;
let mmap = unsafe { Mmap::map(&file)? };

// Now `mmap` is a &[u8] backed by the file
let magic = &mmap[0..4]; // reads directly from the file via page cache
```

Add to `Cargo.toml`:

```toml
memmap2 = "0.9"
```

### 6.2 What to mmap

- **Term dictionary** — Frequently accessed during search.
- **Postings lists** — Read on every query that hits a segment.
- **Document store** — Only accessed when returning results (lazy loading).

### 6.3 Benchmarking

Use `criterion` for benchmarks:

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }
```

Write benchmarks for:

| Benchmark | What it measures |
|---|---|
| `index_1k_docs` | Throughput of indexing |
| `search_single_term` | Latency of a simple query |
| `search_phrase` | Latency of phrase matching |
| `search_fuzzy` | Latency of fuzzy matching |

```rust
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_search(c: &mut Criterion) {
    let engine = setup_engine_with_10k_docs();
    c.bench_function("search_single_term", |b| {
        b.iter(|| engine.search("ogre"))
    });
}

criterion_group!(benches, bench_search);
criterion_main!(benches);
```

### Deliverable — Phase 6

- Segments are memory-mapped.
- You have benchmark numbers before and after mmap.
- The engine can index and search 100K+ documents efficiently.

---

## Phase 7 — Concurrent Queries & Production Polish

**Goal:** Handle multiple queries simultaneously, add observability, and polish for a portfolio-ready project.

### 7.1 Concurrency Model

Replace `RwLock` with a more granular approach:

```text
                    ┌───────────────────────┐
                    │    Query Executor      │
                    │                        │
  Query ──────────▶ │  1. Acquire read lock  │
                    │  2. Snapshot segments   │
                    │  3. Release lock        │
                    │  4. Search segments     │  ← no lock held here
                    │  5. Merge & rank        │
                    │  6. Return results      │
                    └───────────────────────┘
```

The key insight: acquire the segment list under a short lock, then search the immutable segments **without holding any lock**. Writes only need to lock when modifying the segment list.

### 7.2 Index Aliases & Multi-Index

Allow multiple named indexes:

```bash
# Create an index
curl -X PUT http://localhost:3000/indexes/books

# Index into it
curl -X POST http://localhost:3000/indexes/books/documents \
  -d '{"id": "1", "title": "Dune", "body": "..."}'

# Search across indexes
curl "http://localhost:3000/indexes/books/search?q=spice"
```

### 7.3 Observability

Add structured logging and metrics:

```toml
tracing = "0.1"
tracing-subscriber = "0.3"
```

Log key events:
- Document indexed (doc_id, index_time_ms)
- Segment flushed (num_docs, segment_size_bytes)
- Segment merged (input_segments, output_size)
- Query executed (query, num_results, took_ms)

### 7.4 Configuration

Use a TOML config file:

```toml
[server]
host = "0.0.0.0"
port = 3000

[index]
flush_threshold = 1000           # docs before flush
merge_max_segments_per_tier = 10
default_result_limit = 10

[scoring]
bm25_k1 = 1.2
bm25_b = 0.75
```

### 7.5 CLI

Add a CLI for admin operations:

```bash
mini-es serve                     # start the server
mini-es index --file data.jsonl   # bulk index from file
mini-es search "ogre bride"      # search from terminal
mini-es stats                     # show index stats
mini-es compact                   # force segment merge
```

Use `clap` for argument parsing:

```toml
clap = { version = "4", features = ["derive"] }
```

### Deliverable — Phase 7

A complete, portfolio-ready search engine with:
- HTTP API
- BM25 ranking
- Phrase, prefix, and fuzzy search
- Persistent segment storage
- Memory-mapped I/O
- Concurrent queries
- Structured logging
- CLI tooling

---

## Complete Dependency List

```toml
[package]
name = "mini-elasticsearch"
version = "0.1.0"
edition = "2024"

[dependencies]
axum = "0.8"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
memmap2 = "0.9"
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = "0.3"
toml = "0.8"

[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }
```

---

## Project Directory Structure

```text
mini-elasticsearch/
├── Cargo.toml
├── config.toml
├── README.md
├── src/
│   ├── main.rs              # Entry point, CLI parsing
│   ├── lib.rs               # Public engine API
│   ├── analyzer/
│   │   ├── mod.rs            # Analyzer trait
│   │   ├── tokenizer.rs      # Whitespace/punctuation tokenizer
│   │   ├── normalizer.rs     # Lowercase, strip accents
│   │   └── stop_words.rs     # Stop word filter
│   ├── index/
│   │   ├── mod.rs            # InvertedIndex struct
│   │   ├── postings.rs       # Posting, PostingsList
│   │   └── term_dict.rs      # Term dictionary (BTreeMap)
│   ├── scoring/
│   │   ├── mod.rs            # Scorer trait
│   │   ├── tfidf.rs          # TF-IDF implementation
│   │   └── bm25.rs           # BM25 implementation
│   ├── query/
│   │   ├── mod.rs            # Query parser
│   │   ├── boolean.rs        # AND / OR queries
│   │   ├── phrase.rs         # Phrase queries
│   │   ├── prefix.rs         # Prefix queries
│   │   └── fuzzy.rs          # Fuzzy queries (Levenshtein)
│   ├── storage/
│   │   ├── mod.rs            # Storage manager
│   │   ├── segment.rs        # Segment reader/writer
│   │   ├── wal.rs            # Write-ahead log
│   │   ├── merge.rs          # Segment merge policy
│   │   └── mmap.rs           # Memory-mapped segment reader
│   ├── server/
│   │   ├── mod.rs            # Axum app setup
│   │   ├── handlers.rs       # Route handlers
│   │   └── models.rs         # Request/Response types (serde)
│   └── config.rs             # Configuration loading
├── benches/
│   └── search_bench.rs       # Criterion benchmarks
├── tests/
│   ├── integration_test.rs   # Full API integration tests
│   └── fixtures/
│       └── sample_docs.json  # Test data
└── data/                     # Runtime data directory
    ├── segments/
    └── wal/
```

---

## Step-by-Step Checklist

### Phase 1 — Core (Week 1–2)
- [ ] Set up Rust project with `cargo init`
- [ ] Read Rust Book chapters 1–6 (ownership, structs, enums, collections)
- [ ] Implement `Analyzer::analyze()` — tokenize and lowercase
- [ ] Write unit tests for the analyzer
- [ ] Implement `InvertedIndex` with `HashMap<String, HashSet<DocId>>`
- [ ] Implement `add_document()` and `search()`
- [ ] Add boolean AND/OR query support
- [ ] Write unit tests for indexing and search

### Phase 2 — Scoring (Week 3)
- [ ] Refactor postings list to store term frequency
- [ ] Add document length tracking
- [ ] Implement TF-IDF scorer
- [ ] Implement BM25 scorer
- [ ] Return results sorted by score
- [ ] Write unit tests comparing TF-IDF and BM25 results
- [ ] Test with a real dataset (e.g., Wikipedia abstracts)

### Phase 3 — HTTP API (Week 3–4)
- [ ] Add axum, tokio, serde dependencies
- [ ] Create `POST /documents` handler
- [ ] Create `GET /search?q=` handler
- [ ] Create `GET /documents/:id` handler
- [ ] Create `DELETE /documents/:id` handler
- [ ] Create `GET /_stats` handler
- [ ] Add error handling (proper HTTP status codes)
- [ ] Wrap engine in `Arc<RwLock<...>>`
- [ ] Write integration tests using `reqwest`

### Phase 4 — Advanced Queries (Week 4–5)
- [ ] Extend postings with position information
- [ ] Implement phrase search
- [ ] Implement prefix search with `BTreeMap::range()`
- [ ] Implement Levenshtein distance function
- [ ] Implement fuzzy search with configurable max edit distance
- [ ] Add query parser to detect phrase/prefix/fuzzy syntax
- [ ] Write tests for all query types

### Phase 5 — Persistence (Week 5–7)
- [ ] Design segment binary format
- [ ] Implement segment writer (flush in-memory index to disk)
- [ ] Implement segment reader (load segment from disk)
- [ ] Add WAL (write-ahead log) for crash recovery
- [ ] Implement flush trigger (every N documents)
- [ ] Search across multiple segments, merge results
- [ ] Implement segment merge (combine small segments)
- [ ] Add startup recovery: load segments + replay WAL
- [ ] Write tests: index docs → kill process → restart → verify data

### Phase 6 — Performance (Week 7–8)
- [ ] Replace file reads with `memmap2`
- [ ] Add `criterion` benchmarks
- [ ] Benchmark before and after mmap
- [ ] Profile with `cargo flamegraph`
- [ ] Optimize hot paths identified by profiling
- [ ] Test with 100K+ documents

### Phase 7 — Polish (Week 8–10)
- [ ] Add structured logging with `tracing`
- [ ] Add config file support (TOML)
- [ ] Build CLI with `clap` (`serve`, `search`, `stats`, `compact`)
- [ ] Add multi-index support
- [ ] Improve concurrency (short locks, immutable segment snapshots)
- [ ] Write a comprehensive README with architecture diagram
- [ ] Add GitHub Actions CI (build, test, lint, format)
- [ ] Add Dockerfile

---

## Concepts to Study Before/During Each Phase

| Phase | Concepts | Resources |
|---|---|---|
| 1 | Rust ownership, borrowing, lifetimes | [The Rust Book Ch. 4](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html) |
| 1 | Inverted index fundamentals | [Stanford IR Book Ch. 1–2](https://nlp.stanford.edu/IR-book/information-retrieval-book.html) |
| 2 | TF-IDF, BM25 scoring | [Stanford IR Book Ch. 6](https://nlp.stanford.edu/IR-book/information-retrieval-book.html), [BM25 Wikipedia](https://en.wikipedia.org/wiki/Okapi_BM25) |
| 3 | Async Rust, tokio runtime | [Tokio Tutorial](https://tokio.rs/tokio/tutorial) |
| 3 | Axum web framework | [Axum Docs](https://docs.rs/axum/latest/axum/) |
| 4 | Edit distance algorithms | [Levenshtein distance](https://en.wikipedia.org/wiki/Levenshtein_distance) |
| 5 | Log-structured merge trees | [LSM Tree Paper](https://www.cs.umb.edu/~poneil/lsmtree.pdf) |
| 5 | Lucene segment architecture | [Lucene in Action](https://www.manning.com/books/lucene-in-action-second-edition) |
| 6 | Memory-mapped I/O, OS page cache | [mmap(2) man page](https://man7.org/linux/man-pages/man2/mmap.2.html) |
| 6 | Benchmarking in Rust | [Criterion.rs Guide](https://bheisler.github.io/criterion.rs/book/) |
| 7 | Concurrency patterns | [Rust Book Ch. 16](https://doc.rust-lang.org/book/ch16-00-concurrency.html) |

---

## Real-World Datasets for Testing

| Dataset | Size | Source |
|---|---|---|
| Wikipedia abstracts | ~600MB | [Hugging Face](https://huggingface.co/datasets/wikipedia) |
| Enron emails | ~1.3GB | [CMU archive](https://www.cs.cmu.edu/~enron/) |
| Project Gutenberg books | Varies | [gutenberg.org](https://www.gutenberg.org/) |
| HackerNews posts | ~2GB | [HN Search API](https://hn.algolia.com/api) |
| arXiv abstracts | ~3GB | [Kaggle](https://www.kaggle.com/datasets/Cornell-University/arxiv) |

Start with something small (Gutenberg books), graduate to larger sets as you optimize.

---

## Stretch Goals (Post-MVP)

Once the core engine is solid, consider these extensions:

- [ ] **Highlighting** — Return matching snippets with `<em>` tags around matches
- [ ] **Faceted search** — Category-based filtering
- [ ] **Field-level search** — Search `title:ogre` vs `body:ogre`
- [ ] **Replication** — Replicate index across nodes
- [ ] **Cluster mode** — Shard index across multiple instances
- [ ] **Query DSL** — JSON-based query language (like Elasticsearch's)
- [ ] **Synonym support** — "car" matches "automobile"
- [ ] **Auto-complete / suggest** — Return completions as user types
- [ ] **Custom analyzers** — Pluggable tokenizers, filters, and normalizers
- [ ] **Compression** — Compress postings lists (varint, PFOR, etc.)

---

## How This Looks on Your Portfolio

> **Mini Elasticsearch** — A search engine built from scratch in Rust
>
> - Full-text search with BM25 ranking, phrase/prefix/fuzzy queries
> - Segment-based persistent storage with write-ahead logging
> - Memory-mapped I/O for indexes larger than available RAM
> - Concurrent query execution over an HTTP API
> - ~5,000 lines of Rust, benchmarked to handle 100K+ documents
>
> **Tech:** Rust, axum, tokio, mmap, custom binary format
>
> This project demonstrates understanding of information retrieval, storage engines, and systems programming.

This is the kind of project that makes a senior engineer or hiring manager pause and actually read your code.
