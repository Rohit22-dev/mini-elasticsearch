# Phase 3 — Developer Guide

## Project Structure

```
mini-elasticsearch/
├── Cargo.toml          # Project manifest & dependencies (axum, tokio, serde added)
├── src/
│   ├── lib.rs          # Library root — module declarations
│   ├── analyzer.rs     # Text tokenizer & normalizer (unchanged)
│   ├── index.rs        # Inverted index — String DocId, Document struct, delete support
│   ├── scorer.rs       # BM25 relevance scoring (unchanged from Phase 2)
│   ├── engine.rs       # SearchEngine facade — title+body, delete, stats
│   ├── server.rs       # HTTP API with axum — 5 REST endpoints (NEW)
│   └── main.rs         # Async main — starts HTTP server on port 3000
├── tests/
│   ├── analyzer_tests.rs  # Analyzer tests (unchanged)
│   ├── index_tests.rs     # Index tests — String IDs, Document, delete
│   ├── engine_tests.rs    # Engine tests — new API signatures
│   ├── scorer_tests.rs    # Scorer tests (unchanged from Phase 2)
│   └── server_tests.rs    # HTTP integration tests (NEW)
└── docs/
    ├── mini_elasticsearch_rust.md   # Full project design doc
    ├── phase1_guide.md              # Phase 1 developer guide
    ├── phase2_guide.md              # Phase 2 developer guide
    └── phase3_guide.md              # ← You are here
```

---

## What Changed from Phase 2

| Component | Phase 2 | Phase 3 |
|---|---|---|
| DocId type | `u64` | `String` (supports API IDs like "abc-123") |
| Document storage | `HashMap<DocId, String>` (raw text) | `HashMap<DocId, Document>` (id + title + body) |
| Indexing API | `add_document(id, text)` | `add_document(id, title, body)` |
| Delete support | None | `delete_document(id) → bool` |
| Stats | None | `stats() → IndexStats` |
| SearchResult | `{ doc_id, score }` | `{ doc_id, title, score }` |
| HTTP server | None | 5 REST endpoints via axum |
| Dependencies | Zero | axum, tokio, serde, serde_json |

---

## What Each File Does

### `Cargo.toml`

Phase 3 introduces **4 external dependencies** for HTTP and serialization:

```toml
[dependencies]
axum = "0.8"                              # HTTP framework
tokio = { version = "1", features = ["full"] }  # Async runtime
serde = { version = "1", features = ["derive"] } # JSON serialization
serde_json = "1"                          # JSON parsing

[dev-dependencies]
tower = { version = "0.5", features = ["util"] }  # ServiceExt for testing
http-body-util = "0.1"                             # BodyExt for test response reading
```

---

### `src/lib.rs`

Declares five public modules (one new from Phase 2):

```rust
pub mod analyzer;
pub mod engine;
pub mod index;
pub mod scorer;
pub mod server;   // ← NEW in Phase 3
```

---

### `src/analyzer.rs` — Text Tokenizer & Normalizer

**Unchanged from Phase 1.** Splits text on non-alphanumeric boundaries, filters empty strings, lowercases tokens.

---

### `src/scorer.rs` — BM25 Scorer

**Unchanged from Phase 2.** Computes IDF and BM25 scores with configurable k1/b parameters.

---

### `src/index.rs` — Inverted Index (Modified)

**Key changes from Phase 2:**

1. **`DocId` is now `String`** — Supports API-provided IDs like `"abc-123"`.
2. **New `Document` struct** — Stores `id`, `title`, and `body` separately.
3. **`add_document(id, title, body)`** — Indexes the combined `title + body` text.
4. **`delete_document(id) → bool`** — Public method to remove a document and its postings.
5. **`term_count()`** — Returns the number of unique terms (for stats endpoint).

**Data structures:**
```
Document:      { id: String, title: String, body: String }
index:         HashMap<String, Vec<Posting>>   — term → postings
documents:     HashMap<DocId, Document>        — doc_id → stored document
metadata:      IndexMetadata                   — corpus-level stats
```

**Tests (18):** Postings, frequencies, metadata, delete (document, postings, metadata cleanup), term_count, title+body both indexed.

---

### `src/engine.rs` — SearchEngine Facade (Modified)

**Key changes from Phase 2:**

1. **`add_document(id, title, body)`** — Three-argument version.
2. **`delete_document(id) → bool`** — Delegates to index.
3. **`SearchResult` now includes `title`**.
4. **`stats() → IndexStats`** — Returns `total_docs`, `total_terms`, `avg_doc_length`.

**Tests (21):** All Phase 2 tests adapted for String IDs, plus delete tests, stats test, title-in-results test.

---

### `src/server.rs` — HTTP API (New)

**Purpose:** Wraps the search engine in an HTTP server using axum, with JSON request/response schemas.

**Shared state:**
```rust
type SharedEngine = Arc<RwLock<SearchEngine>>;
```

- `Arc` = thread-safe reference counting (shared ownership across async tasks).
- `RwLock` = multiple concurrent readers OR one exclusive writer.

**API endpoints:**

| Endpoint | Method | Purpose | Success | Error |
|---|---|---|---|---|
| `/documents` | POST | Index a new document | 201 Created | — |
| `/documents/:id` | GET | Retrieve a document by ID | 200 OK | 404 |
| `/documents/:id` | DELETE | Delete a document | 200 OK | 404 |
| `/search?q=...&limit=10` | GET | Search with ranked results | 200 OK | — |
| `/_stats` | GET | Return index statistics | 200 OK | — |

**Request/Response schemas:**

Index a document:
```json
// POST /documents
// Request:
{ "id": "abc-123", "title": "The Ogre's Castle", "body": "The ogre kidnapped..." }
// Response (201):
{ "status": "indexed", "id": "abc-123" }
```

Search:
```json
// GET /search?q=ogre+bride&limit=10
// Response:
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

Get document:
```json
// GET /documents/abc-123
// Response (200):
{ "id": "abc-123", "title": "The Ogre's Castle", "body": "The ogre kidnapped..." }
// Response (404): empty
```

Delete document:
```json
// DELETE /documents/abc-123
// Response (200):
{ "status": "deleted", "id": "abc-123" }
// Response (404): empty
```

Stats:
```json
// GET /_stats
// Response:
{ "total_docs": 100, "total_terms": 523, "avg_doc_length": 45.2 }
```

**Tests (10):** All 5 endpoints tested individually, plus limit parameter, not-found cases, and a full index→search→delete workflow test.

---

### `src/main.rs` — HTTP Server Entry Point

**Purpose:** Starts the async HTTP server on `0.0.0.0:3000` using `#[tokio::main]`.

---

## How to Check Changes

### View modified files with Git

```bash
# See which files changed
git status

# See a summary of changes
git diff --stat

# See the full diff
git diff
```

### View the new files

```bash
# List all source files
ls -la src/

# Read the new server module
cat src/server.rs
```

---

## How to Run & Verify

### 1. Run all tests

```bash
cargo test
```

**Expected output:** 74 tests pass (10 analyzer + 21 engine + 18 index + 13 scorer + 10 server + 2 doc-tests)

```
running 10 tests          (analyzer_tests)   — 10 passed
running 21 tests          (engine_tests)     — 21 passed
running 18 tests          (index_tests)      — 18 passed
running 13 tests          (scorer_tests)     — 13 passed
running 10 tests          (server_tests)     — 10 passed
Doc-tests:                                   —  2 passed

test result: ok. 74 passed; 0 failed; 0 ignored
```

### 2. Run a specific test suite

```bash
# Run only server tests
cargo test server

# Run only index tests
cargo test index

# Run a single test by name
cargo test test_index_then_search_then_delete
```

### 3. Start the server

```bash
cargo run
```

**Expected output:**
```
=== Mini Elasticsearch — Phase 3 (HTTP API) ===

Mini Elasticsearch listening on 0.0.0.0:3000
```

### 4. Test with curl (in another terminal)

```bash
# Index documents
curl -s -X POST http://localhost:3000/documents \
  -H "Content-Type: application/json" \
  -d '{"id": "1", "title": "Ogre Story", "body": "The ogre kidnapped the bride"}' | jq .

# Expected: { "status": "indexed", "id": "1" }

curl -s -X POST http://localhost:3000/documents \
  -H "Content-Type: application/json" \
  -d '{"id": "2", "title": "Dragon Tale", "body": "The dragon burned the village"}' | jq .

curl -s -X POST http://localhost:3000/documents \
  -H "Content-Type: application/json" \
  -d '{"id": "3", "title": "Escape Story", "body": "The bride escaped from the ogre"}' | jq .

# Search
curl -s "http://localhost:3000/search?q=ogre" | jq .
# Expected: 2 hits with scores, sorted descending

# Search with limit
curl -s "http://localhost:3000/search?q=ogre&limit=1" | jq .
# Expected: total_hits=2 but only 1 hit returned

# Get a document
curl -s http://localhost:3000/documents/1 | jq .
# Expected: { "id": "1", "title": "Ogre Story", "body": "..." }

# Stats
curl -s http://localhost:3000/_stats | jq .
# Expected: { "total_docs": 3, "total_terms": ..., "avg_doc_length": ... }

# Delete a document
curl -s -X DELETE http://localhost:3000/documents/1 | jq .
# Expected: { "status": "deleted", "id": "1" }

# Verify deletion
curl -s -o /dev/null -w "%{http_code}" http://localhost:3000/documents/1
# Expected: 404
```

### 5. Build without running

```bash
cargo build
cargo build --release
```

### 6. Check for compiler warnings

```bash
cargo clippy
```

---

## Quick Verification Checklist

| Check | Command | Expected |
|---|---|---|
| Project compiles | `cargo build` | No errors |
| All tests pass | `cargo test` | 74/74 pass |
| No warnings | `cargo clippy` | No warnings |
| Server starts | `cargo run` | Listening on 0.0.0.0:3000 |
| POST /documents | `curl -X POST ...` | 201, `{"status":"indexed"}` |
| GET /search?q=ogre | `curl ...` | 200, ranked hits with scores |
| GET /documents/:id | `curl ...` | 200, document JSON |
| DELETE /documents/:id | `curl -X DELETE ...` | 200, `{"status":"deleted"}` |
| GET /_stats | `curl ...` | 200, stats JSON |
| 404 on missing doc | `curl /documents/999` | 404 |
