# Phase 7 — Developer Guide: Concurrent Queries, Multi-Index & Production Polish

## Project Structure

```
mini-elasticsearch/
├── Cargo.toml                  # clap, tracing, toml added
├── config.toml                 # Default TOML configuration
├── benches/
│   └── search_bench.rs         # Criterion benchmarks
├── src/
│   ├── lib.rs                  # Library entry point
│   ├── config.rs               # Config loading from TOML (NEW)
│   ├── analyzer.rs             # Positional text analyzer
│   ├── index.rs                # Positional InvertedIndex
│   ├── scorer.rs               # BM25 relevance scoring
│   ├── engine.rs               # EngineManager for multi-index & persistence
│   ├── fuzzy.rs                # Levenshtein distance matching
│   ├── storage/                # Segment storage, WAL, merge
│   ├── server.rs               # Axum HTTP API with multi-index routes
│   └── main.rs                 # CLI with clap subcommands (NEW)
├── tests/
│   ├── multi_index_tests.rs    # Multi-index isolation & route tests (NEW)
│   └── ...
└── docs/
    ├── phase1_guide.md
    ├── phase2_guide.md
    ├── phase3_guide.md
    ├── phase4_guide.md
    ├── phase5_guide.md
    ├── phase6_guide.md
    └── phase7_guide.md          # ← You are here
```

---

## What Changed from Phase 6

| Component | Phase 6 | Phase 7 |
|---|---|---|
| CLI | Minimal async main | Comprehensive CLI using `clap` (5 subcommands) |
| Configuration | Hardcoded parameters | Declarative `config.toml` loaded on startup |
| Observability | `println!` | Structured logging via `tracing` & `tracing-subscriber` |
| Multi-Index | Single global index | `EngineManager` supporting arbitrary named indexes |
| Concurrency | Single engine lock | Per-index `Arc<RwLock<SearchEngine>>` with fast read locks |

---

## Architecture & Features

### 1. Multi-Index Management (`EngineManager`)

`EngineManager` manages isolated `SearchEngine` instances organized into separate subdirectories:

```text
data/
└── indexes/
    ├── default/
    │   ├── segments/
    │   └── wal/
    ├── books/
    │   ├── segments/
    │   └── wal/
    └── movies/
        ├── segments/
        └── wal/
```

- Each index maintains its own independent term dictionary, postings lists, document store, and WAL.
- Reads and writes to `books` do not lock or contend with queries on `movies`.
- Deleting an index removes its persisted directory cleanly from disk.

### 2. HTTP Routes for Multi-Index

| Method | Endpoint | Description |
|---|---|---|
| `GET` | `/indexes` | List all available indexes |
| `PUT` | `/indexes/{name}` | Create a new named index |
| `DELETE` | `/indexes/{name}` | Delete a named index and its data |
| `POST` | `/indexes/{name}/documents` | Index a document into `{name}` |
| `GET` | `/indexes/{name}/documents/{id}` | Retrieve a document from `{name}` |
| `DELETE` | `/indexes/{name}/documents/{id}` | Delete a document from `{name}` |
| `GET` | `/indexes/{name}/search?q=...` | Search `{name}` with BM25 |
| `GET` | `/indexes/{name}/_stats` | Retrieve stats for `{name}` |
| `POST` | `/indexes/{name}/_flush` | Flush memory buffer to segment |
| `POST` | `/indexes/{name}/_compact` | Merge segments into one |

*(Note: Root endpoints `/documents`, `/search`, `/_stats`, etc. transparently route to the `"default"` index for full backward compatibility).*

### 3. CLI Subcommands (`mini-es`)

The command-line interface provides complete administrative control:

```bash
# 1. Start HTTP Server
cargo run -- serve --port 3000

# 2. Bulk index documents from JSON or JSONL file
cargo run -- index --file docs.jsonl --index books

# 3. Query from terminal
cargo run -- search "ogre bride" --index default --limit 5

# 4. View index statistics
cargo run -- stats --index books

# 5. Force segment compaction
cargo run -- compact --index books
```

### 4. Structured Logging (`tracing`)

Configured with `RUST_LOG` environment filtering:

```bash
RUST_LOG=debug cargo run -- serve
```

Logs emit structured spans and key-value fields:
- `tracing::info!(index = %name, "Created new index")`
- `tracing::info!(segment = ?path, "Segment flushed to disk")`
- `tracing::debug!(query = %query, hits = count, "Search query executed")`

### 5. Configuration (`config.toml`)

```toml
[server]
host = "0.0.0.0"
port = 3000

[index]
data_dir = "data"
flush_threshold = 1000
merge_max_segments = 10
default_result_limit = 10

[scoring]
bm25_k1 = 1.2
bm25_b = 0.75
```

---

## Verification & Testing

### Running All Tests
```bash
cargo test
```

### Multi-Index REST Verification
```bash
# Create books index
curl -X PUT http://localhost:3000/indexes/books

# Index into books
curl -X POST http://localhost:3000/indexes/books/documents \
  -H "Content-Type: application/json" \
  -d '{"id":"1","title":"Dune","body":"The spice must flow"}'

# Search books
curl "http://localhost:3000/indexes/books/search?q=spice"

# List all indexes
curl http://localhost:3000/indexes
# → {"indexes":["books","default"]}
```
