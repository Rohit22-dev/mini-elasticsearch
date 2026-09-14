# Phase 5 — Developer Guide: Persistence & Segment Storage

## Project Structure

```
mini-elasticsearch/
├── Cargo.toml                  # Project manifest
├── config.toml                 # Engine configuration file
├── src/
│   ├── lib.rs                  # Library root — storage module exposed
│   ├── analyzer.rs             # Positional text analyzer
│   ├── index.rs                # InvertedIndex — accessors & merge_with added
│   ├── scorer.rs               # BM25 relevance scoring
│   ├── engine.rs               # SearchEngine facade with persistence & WAL recovery
│   ├── fuzzy.rs                # Levenshtein distance & fuzzy matching
│   ├── server.rs               # HTTP API — /_flush and /_compact added
│   ├── storage/
│   │   ├── mod.rs              # Storage module root
│   │   ├── segment.rs          # Binary segment writer & reader (magic MNSE)
│   │   ├── wal.rs              # Write-Ahead Log (append-only JSONL)
│   │   └── merge.rs            # Segment compaction and merge
│   └── main.rs                 # CLI entry point
├── tests/
│   ├── analyzer_tests.rs
│   ├── index_tests.rs
│   ├── engine_tests.rs
│   ├── scorer_tests.rs
│   ├── fuzzy_tests.rs
│   ├── server_tests.rs
│   ├── storage_tests.rs        # NEW — Segment roundtrip, WAL, and merge tests
│   └── persistence_tests.rs    # NEW — Auto-flush, crash recovery, compaction tests
└── docs/
    ├── phase1_guide.md
    ├── phase2_guide.md
    ├── phase3_guide.md
    ├── phase4_guide.md
    └── phase5_guide.md          # ← You are here
```

---

## What Changed from Phase 4

| Component | Phase 4 | Phase 5 |
|---|---|---|
| InvertedIndex | Purely in-memory | Added `index()`, `documents()`, `from_parts()`, `merge_with()` |
| Storage format | None | Binary segment format with magic `MNSE` v1 |
| Crash resilience | None (data lost on kill) | Append-only Write-Ahead Log (`wal.log`) |
| Segment merging | None | `merge_segments` combines multiple segments into one |
| Engine persistence | None | `SearchEngine::open(data_dir, threshold)`, `flush()`, `compact()` |
| HTTP API | Core CRUD + Search | Added `POST /_flush`, `POST /_compact`, `segment_count` in `/_stats` |

---

## Architecture & Data Flow

```text
Incoming Write (add_document)
       │
       ├──▶ 1. Append to Write-Ahead Log (data/wal/wal.log) [fsync/append]
       │
       ├──▶ 2. Index in memory (immediately searchable)
       │
       └──▶ 3. Check buffer threshold
                 │
                 ▼ (if unflushed_docs >= flush_threshold)
             flush() ──▶ Write immutable Segment (data/segments/segment_*.seg)
                     ──▶ Truncate WAL
```

```text
Crash Recovery on Startup:
1. Scan `data/segments/*.seg`
2. Load each segment via mmap and merge into in-memory InvertedIndex
3. Replay uncommitted operations from `data/wal/wal.log`
4. Search engine is fully restored!
```

---

## Binary Segment File Format

The custom binary format is structured in 4 distinct sections:

```text
┌────────────────────────────────────────────────────────┐
│                        Header                          │
│  magic bytes: "MNSE" (4 bytes)                         │
│  version: u32 (1)                                      │
│  num_terms: u32                                        │
│  num_docs: u32                                         │
│  total_doc_length: u64                                 │
│  postings_section_offset: u64                          │
│  doc_store_section_offset: u64                         │
├────────────────────────────────────────────────────────┤
│                   Term Dictionary                      │
│  For each term:                                        │
│    term_len: u16                                       │
│    term_bytes: [u8]                                    │
│    postings_offset: u64                                │
│    postings_len: u32                                   │
├────────────────────────────────────────────────────────┤
│                   Postings Section                     │
│  For each term:                                        │
│    num_postings: u32                                   │
│    For each posting:                                   │
│      doc_id_len: u16                                   │
│      doc_id_bytes: [u8]                                │
│      term_freq: u32                                    │
│      num_positions: u32                                │
│      positions: [u32]                                  │
├────────────────────────────────────────────────────────┤
│                 Document Store Section                 │
│  For each document:                                    │
│    doc_id_len: u16                                     │
│    doc_id_bytes: [u8]                                  │
│    doc_length: u32                                     │
│    title_len: u16                                      │
│    title_bytes: [u8]                                   │
│    body_len: u32                                       │
│    body_bytes: [u8]                                    │
└────────────────────────────────────────────────────────┘
```

---

## What Each File Does

### `src/storage/segment.rs`
- **`SegmentWriter::write(index: &InvertedIndex, path: &Path)`**:
  Pre-computes postings and document store byte buffers, generates term dictionary offsets, and writes the complete immutable segment to disk.
- **`SegmentReader::read(path: &Path) -> io::Result<InvertedIndex>`**:
  Standard file read and deserializer that validates magic bytes `b"MNSE"` and reconstructs the `InvertedIndex`.
- **`SegmentReader::read_mmap(path: &Path) -> io::Result<InvertedIndex>`**:
  Maps the segment file using `memmap2::Mmap` and parses directly from memory-mapped slices.

### `src/storage/wal.rs`
- **`Wal::open(path: &Path)`**:
  Opens or creates `wal.log` with append-only mode.
- **`append_add(id, title, body)`** and **`append_delete(id)`**:
  Writes newline-delimited JSON records and flushes to disk.
- **`replay() -> io::Result<Vec<WalRecord>>`**:
  Reads and deserializes all records for startup recovery.
- **`truncate()`**:
  Clears the log file back to 0 bytes once a segment has been safely flushed to disk.

### `src/storage/merge.rs`
- **`merge_segments(segment_paths: &[PathBuf], output_path: &Path)`**:
  Loads multiple segment files, combines them via `InvertedIndex::merge_with`, writes the new unified segment, and safely cleans up the source segments.

### `src/engine.rs`
- **`SearchEngine::open(data_dir, flush_threshold)`**:
  Loads persisted segments from disk, replays the WAL, and maintains an active write buffer.
- **`flush()`**:
  Writes the current write buffer into a new segment file `segment_<timestamp>_<id>.seg` and truncates the WAL.
- **`compact()`**:
  Consolidates multiple segment files into one.

### `src/server.rs`
- `POST /_flush` — Manually triggers an index flush.
- `POST /_compact` — Manually triggers segment compaction.
- `GET /_stats` — Includes `segment_count` in response.

---

## Deliverable Verification

### Running Automated Tests
```bash
cargo test --test storage_tests
cargo test --test persistence_tests
```

### Persistence Manual Walkthrough
```bash
# 1. Start server
cargo run -- serve &

# 2. Index documents
curl -X POST http://localhost:3000/documents \
  -H "Content-Type: application/json" \
  -d '{"id":"1","title":"Dune","body":"The spice must flow"}'

# 3. Manually flush to segment
curl -X POST http://localhost:3000/_flush

# 4. Check stats
curl http://localhost:3000/_stats
# → {"total_docs":1,"total_terms":4,"avg_doc_length":5.0,"segment_count":1}

# 5. Kill server and restart
kill %1
cargo run -- serve &

# 6. Verify data survived restart
curl "http://localhost:3000/search?q=spice"
# → returns Dune document!
```
