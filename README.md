# Mini Elasticsearch (Rust)

A lightweight, high-performance search engine built from scratch in Rust. Not a toy—it features an inverted index with positional tracking, Okapi BM25 relevance ranking, LSM-tree inspired segment persistence with memory-mapped I/O (mmap), Write-Ahead Logging (WAL) for crash recovery, multi-index support, a command-line interface, and an asynchronous HTTP REST API powered by Tokio and Axum.

---

## Key Features

- **Text Analysis & Tokenization**: Alphanumeric tokenization, case-folding normalization, and 0-based token position tracking.
- **Positional Inverted Index**: Sorted `BTreeMap` term index mapping terms to postings with document IDs, term frequencies, and positional offsets.
- **Okapi BM25 Scoring**: Industry-standard ranked retrieval incorporating term frequency saturation ($k_1$) and document length normalization ($b$).
- **Rich Query Semantics**:
  - **Single & Multi-term Queries**: Default `AND` semantics and explicit `OR` support.
  - **Phrase Search**: Quoted queries (e.g., `"ogre bride"`) verified via token adjacency in positional postings.
  - **Prefix Search**: Trailing wildcard queries (e.g., `ogr*`) leveraging `BTreeMap` range queries.
  - **Fuzzy Search**: Levenshtein edit distance queries (e.g., `ogr~` or `sitting~2`).
- **Storage Engine & Persistence**:
  - **Write-Ahead Log (WAL)**: Append-only disk logging ensuring crash resilience before memory mutations.
  - **Binary Segment Files (`.seg`)**: Immutable on-disk structures featuring magic bytes (`MNSE`), binary term dictionaries, postings blocks, and document stores.
  - **Zero-Copy Memory-Mapped I/O**: High-performance segment reading using OS page cache via `memmap2`.
  - **Compaction & Merge**: Automatic and manual flushing of in-memory buffers and segment merging to reclaim space and prune deletions.
- **Multi-Index Support**: Isolated index management via `EngineManager` with separate storage directories and lifecycle control.
- **HTTP REST API**: Fully asynchronous REST interface with endpoints for ingestion, search, stats, index management, and maintenance.
- **CLI & Utilities**: Command-line tool for running the server, batch indexing (JSON / JSONL), command-line querying, compaction, and inspection.

---

## Architecture Overview

```text
                               ┌─────────────────────────┐
                               │     HTTP REST API       │
                               │     (Axum / Tokio)      │
                               └────────────┬────────────┘
                                            │
                     ┌──────────────────────┼──────────────────────┐
                     ▼                      ▼                      ▼
              POST /documents          GET /search          DELETE /documents/:id
                     │                      │                      │
                     └──────────────────────┼──────────────────────┘
                                            │
                                            ▼
                               ┌─────────────────────────┐
                               │      EngineManager      │
                               │  (Multi-Index Router)   │
                               └────────────┬────────────┘
                                            │
                                            ▼
                               ┌─────────────────────────┐
                               │      SearchEngine       │
                               │                         │
                               │   ┌──────────────────┐  │
                               │   │     Analyzer     │  │
                               │   └────────┬─────────┘  │
                               │            │            │
                               │   ┌────────▼─────────┐  │
                               │   │  Inverted Index  │  │
                               │   │ (BTreeMap+Posns) │  │
                               │   └────────┬─────────┘  │
                               │            │            │
                               │   ┌────────▼─────────┐  │
                               │   │   BM25 Scorer    │  │
                               │   └────────┬─────────┘  │
                               │            │            │
                               │   ┌────────▼─────────┐  │
                               │   │  Storage Layer   │  │
                               │   │  WAL + Segments  │  │
                               │   └──────────────────┘  │
                               └─────────────────────────┘
                                            │
                     ┌──────────────────────┴──────────────────────┐
                     ▼                                             ▼
          ┌─────────────────────┐                       ┌─────────────────────┐
          │   Write-Ahead Log   │                       │ Immutable Segments  │
          │     (wal.log)       │                       │  (.seg via mmap)    │
          └─────────────────────┘                       └─────────────────────┘
```

---

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/) (2024 edition or newer; 1.85+ recommended)
- `cargo` package manager

### Build and Test

Clone the repository and build the binary:

```bash
# Build release binary
cargo build --release

# Run all unit, integration, and doc tests (129+ tests)
cargo test

# Run microbenchmarks (using Criterion)
cargo bench
```

---

## Configuration

Settings can be customized via `config.toml` in the working directory or specified with the `--config` flag:

```toml
[server]
host = "0.0.0.0"
port = 3000

[index]
data_dir = "data"               # Base directory for segments and WAL
flush_threshold = 1000          # Auto-flush in-memory buffer after N documents
merge_max_segments = 10         # Maximum segments before compaction triggers
default_result_limit = 10       # Default hit count for searches

[scoring]
bm25_k1 = 1.2                   # Term frequency saturation
bm25_b = 0.75                   # Document length normalization
```

---

## Command-Line Interface (CLI)

The `mini-es` executable provides subcommands for running services and performing index operations:

### 1. Start the HTTP Server

```bash
# Start on default address (0.0.0.0:3000) or config.toml settings
cargo run --release -- serve

# Specify custom host and port
cargo run --release -- serve --host 127.0.0.1 --port 8080

# Specify custom configuration file
cargo run --release -- --config ./custom-config.toml serve
```

### 2. Bulk Index Documents

Supports both JSON arrays (`[{...}, {...}]`) and Newline-Delimited JSON (`{"id": ...}\n{"id": ...}`):

```bash
# Index documents into the default index
cargo run --release -- index --file sample_docs.json

# Index documents into a named index
cargo run --release -- index --file sample_docs.jsonl --index articles
```

Expected document format:
```json
{
  "id": "doc_1",
  "title": "Rust Programming",
  "body": "Rust is a systems language emphasizing safety and speed."
}
```

### 3. Search via CLI

```bash
# Standard keyword search
cargo run --release -- search "systems language"

# Phrase search (exact ordered adjacency)
cargo run --release -- search "\"systems language\""

# Prefix search
cargo run --release -- search "syst*"

# Fuzzy search (Levenshtein edit distance)
cargo run --release -- search "sistems~1"

# Query a specific index with a custom limit
cargo run --release -- search "safety" --index articles --limit 5
```

### 4. Index Inspection & Compaction

```bash
# View document count, term count, average length, and segment count
cargo run --release -- stats --index articles

# Merge all segment files into a single consolidated segment
cargo run --release -- compact --index articles
```

---

## HTTP REST API

When running `mini-es serve`, the following HTTP endpoints are exposed. The server supports both the **default index** directly and **isolated named indices** under `/indexes/{name}/...`.

### Summary of Endpoints

| Method | Endpoint | Description |
|---|---|---|
| `POST` | `/documents` | Index or update a document (default index) |
| `GET` | `/documents/:id` | Fetch a document by ID (default index) |
| `DELETE` | `/documents/:id` | Delete a document by ID (default index) |
| `GET` | `/search?q={query}&limit={n}` | Execute ranked search query (default index) |
| `GET` | `/_stats` | Retrieve index statistics (default index) |
| `POST` | `/_flush` | Flush in-memory buffer to a new disk segment |
| `POST` | `/_compact` | Compact all segments into one |
| `GET` | `/indexes` | List all existing indices |
| `PUT` | `/indexes/:name` | Create a new index |
| `DELETE` | `/indexes/:name` | Delete an index and its on-disk files |
| `POST` | `/indexes/:name/documents` | Index or update a document in a named index |
| `GET` | `/indexes/:name/documents/:id` | Fetch a document by ID from a named index |
| `DELETE` | `/indexes/:name/documents/:id`| Delete a document from a named index |
| `GET` | `/indexes/:name/search?q=...` | Execute search on a named index |
| `GET` | `/indexes/:name/_stats` | Retrieve statistics for a named index |
| `POST` | `/indexes/:name/_flush` | Flush buffered documents for a named index |
| `POST` | `/indexes/:name/_compact` | Compact segments for a named index |

---

### Examples

#### Index a Document

```bash
curl -X POST http://localhost:3000/documents \
  -H "Content-Type: application/json" \
  -d '{
    "id": "1",
    "title": "The Princess Bride",
    "body": "The dreadful dread pirate Roberts kidnapped the lovely bride."
  }'
```

Response (`201 Created`):
```json
{
  "status": "indexed",
  "id": "1"
}
```

#### Retrieve a Document

```bash
curl http://localhost:3000/documents/1
```

Response (`200 OK`):
```json
{
  "id": "1",
  "title": "The Princess Bride",
  "body": "The dreadful dread pirate Roberts kidnapped the lovely bride."
}
```

#### Execute Searches

##### 1. Standard Ranked Search
```bash
curl "http://localhost:3000/search?q=pirate+bride"
```

##### 2. Exact Phrase Search
```bash
curl "http://localhost:3000/search?q=%22pirate+roberts%22"
```

##### 3. Prefix Search
```bash
curl "http://localhost:3000/search?q=kidnap*"
```

##### 4. Fuzzy Search
```bash
# Default edit distance 1
curl "http://localhost:3000/search?q=pirat~"

# Custom edit distance (e.g. up to 2 edits)
curl "http://localhost:3000/search?q=pirote~2"
```

##### 5. Boolean OR Search
```bash
curl "http://localhost:3000/search?q=ogre+OR+pirate"
```

Response format:
```json
{
  "query": "pirate bride",
  "total_hits": 1,
  "took_ms": 0,
  "hits": [
    {
      "id": "1",
      "title": "The Princess Bride",
      "score": 0.5841
    }
  ]
}
```

#### Index Statistics

```bash
curl http://localhost:3000/_stats
```

Response (`200 OK`):
```json
{
  "total_docs": 1,
  "total_terms": 8,
  "avg_doc_length": 9.0,
  "segment_count": 0
}
```

#### Multi-Index Workflow

```bash
# 1. Create a named index
curl -X PUT http://localhost:3000/indexes/wiki

# 2. Ingest into the named index
curl -X POST http://localhost:3000/indexes/wiki/documents \
  -H "Content-Type: application/json" \
  -d '{"id": "doc-101", "title": "Search Engines", "body": "An inverted index is central to search."}'

# 3. Query the named index
curl "http://localhost:3000/indexes/wiki/search?q=inverted+index"

# 4. Flush and compact the named index
curl -X POST http://localhost:3000/indexes/wiki/_flush
curl -X POST http://localhost:3000/indexes/wiki/_compact
```

---

## Storage & Internal Mechanics

### On-Disk Binary Segment Format (`.seg`)

Each flushed segment is an immutable binary file designed for fast sequential parsing and memory-mapped lookups:

```text
┌──────────────────────────────────────────────────────────┐
│ Header (40 bytes)                                        │
│   - Magic Bytes: "MNSE" (4 bytes)                        │
│   - Version: u32 (1)                                     │
│   - Terms Count: u32                                     │
│   - Docs Count: u32                                      │
│   - Total Doc Length: u64                                │
│   - Postings Section Offset: u64                         │
│   - Doc Store Section Offset: u64                        │
├──────────────────────────────────────────────────────────┤
│ Term Dictionary                                          │
│   - term_len: u16, term: [u8; len]                       │
│   - postings_offset: u64, postings_len: u32              │
├──────────────────────────────────────────────────────────┤
│ Postings Section                                         │
│   - postings_count: u32                                  │
│   - per posting: doc_id_len (u16) + doc_id               │
│                  + tf (u32) + pos_count (u32)            │
│                  + positions ([u32; pos_count])          │
├──────────────────────────────────────────────────────────┤
│ Document Store                                           │
│   - per doc: id_len (u16) + id + doc_len (u32)           │
│              + title_len (u16) + title                   │
│              + body_len (u32) + body                     │
└──────────────────────────────────────────────────────────┘
```

### Crash Recovery & WAL
1. Ingest operations (`add_document`, `delete_document`) synchronously append an entry to `wal/wal.log`.
2. When the engine boots or re-opens, it loads all existing segments and replays any uncommitted entries in `wal.log`.
3. Upon calling `flush()` (either explicitly or when `flush_threshold` is met), the in-memory buffer is written to a new segment and `wal.log` is truncated.

### Segment Merging (Compaction)
Calling `compact()` aggregates all segments on disk, combines postings for shared terms, prunes deleted documents, recalculates corpus metadata (`avg_doc_length`, `total_docs`), and outputs a consolidated segment.

---

## Directory Structure

```text
mini-elasticsearch/
├── Cargo.toml                # Project manifests & dependencies
├── config.toml               # Engine and server runtime settings
├── benches/
│   └── search_bench.rs       # Criterion benchmark harness
├── src/
│   ├── lib.rs                # Library root exporting engine components
│   ├── main.rs               # CLI entrypoint (clap parser & subcommands)
│   ├── analyzer.rs           # Tokenizer, normalizer, and positional scanner
│   ├── index.rs              # Inverted index, postings, and metadata
│   ├── scorer.rs             # Okapi BM25 scoring & IDF calculations
│   ├── fuzzy.rs              # Levenshtein distance and fuzzy matching
│   ├── engine.rs             # SearchEngine facade, query parser, and EngineManager
│   ├── server.rs             # Axum HTTP routes and request/response handlers
│   ├── config.rs             # TOML configuration loader and defaults
│   └── storage/
│       ├── mod.rs
│       ├── segment.rs        # Binary segment serialization & mmap reader
│       ├── wal.rs            # Write-Ahead Log implementation
│       └── merge.rs          # Multi-segment merge & compaction
└── tests/
    ├── analyzer_tests.rs     # Analyzer & tokenization test suite
    ├── index_tests.rs        # Inverted index & positional postings tests
    ├── scorer_tests.rs       # BM25 math and ranking behavior tests
    ├── engine_tests.rs       # Query parser, phrase, prefix, and fuzzy tests
    ├── fuzzy_tests.rs        # Levenshtein distance edge-case tests
    ├── persistence_tests.rs  # WAL recovery & auto-flush persistence tests
    ├── storage_tests.rs      # Segment I/O, mmap, and merge tests
    ├── server_tests.rs       # HTTP REST API endpoint integration tests
    └── multi_index_tests.rs  # EngineManager index isolation tests
```

---

## Benchmarks

Benchmarking is managed via Criterion:

```bash
cargo bench
```

Benchmarked operations:
- Ingestion throughput (`index_1k_docs`)
- Single-term search latency
- Phrase search latency with positional checks
- Prefix search range expansion
- Fuzzy search distance evaluations

---

## License

This project is licensed under the [MIT License](LICENSE) or Apache 2.0 at your discretion.
