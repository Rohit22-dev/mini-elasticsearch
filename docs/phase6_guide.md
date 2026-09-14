# Phase 6 — Developer Guide: Memory-Mapped I/O & Performance

## Project Structure

```
mini-elasticsearch/
├── Cargo.toml                  # memmap2 & criterion added
├── benches/
│   └── search_bench.rs         # Criterion benchmarks (NEW)
├── src/
│   ├── storage/
│   │   └── segment.rs          # read_mmap implementation via memmap2
│   └── ...
├── tests/
└── docs/
    ├── phase5_guide.md
    └── phase6_guide.md          # ← You are here
```

---

## What Changed from Phase 5

| Component | Phase 5 | Phase 6 |
|---|---|---|
| Segment loading | `std::fs::read` into userspace buffer | `memmap2::Mmap` virtual memory mapping |
| I/O overhead | Dual copying (kernel buffer → process heap) | Zero-copy mapped views backed by OS page cache |
| Benchmarks | Ad-hoc tests | Criterion statistical harness (`benches/search_bench.rs`) |
| Large indexes | Limited by available contiguous RAM | Kernel pages segments on-demand |

---

## What is Memory-Mapped I/O (`mmap`)?

Standard file reading copies bytes from the operating system's page cache into user space memory buffers:

```text
Standard read():
Disk ──▶ OS Page Cache (Kernel) ──▶ Process Buffer (Heap) ──▶ Deserializer
```

With `mmap`, the OS creates a virtual memory mapping pointing directly to the file pages:

```text
Memory-Mapped (mmap):
Disk ──▶ OS Page Cache (Kernel) ◀── Virtual Memory Map ◀── Deserializer
```

### Advantages:
1. **Zero-Copy**: The deserializer parses directly from `&[u8]` backed by the page cache without intermediate buffer allocations.
2. **On-Demand Paging**: Only the pages accessed during a search or load need to be brought from disk into physical memory.
3. **OS-Level Eviction**: Under memory pressure, clean mapped pages are dropped by the OS without writing back, and easily reloaded if requested again.

---

## Code Implementation in `src/storage/segment.rs`

```rust
use memmap2::Mmap;

impl SegmentReader {
    /// Read a segment file using memory-mapped I/O.
    pub fn read_mmap(path: &Path) -> io::Result<InvertedIndex> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        Self::parse_from_bytes(&mmap)
    }

    /// Parse raw segment bytes directly from slice.
    pub fn parse_from_bytes(data: &[u8]) -> io::Result<InvertedIndex> {
        // Direct zero-copy slice parsing
        ...
    }
}
```

---

## Criterion Benchmarks (`benches/search_bench.rs`)

We measure query latency across different search modes and indexing throughput:

| Benchmark | What It Measures | Target Latency / Throughput |
|---|---|---|
| `index_1k_docs` | Bulk indexing throughput | ~1,000 docs in < 25 ms |
| `search_single_term` | Simple term lookup & BM25 ranking | < 10 µs |
| `search_phrase` | Positional adjacency validation | < 50 µs |
| `search_prefix` | `BTreeMap::range` prefix expansion & union | < 50 µs |
| `search_fuzzy` | Levenshtein distance brute-force scan | < 500 µs |

### Running the Benchmarks

```bash
# Run all Criterion benchmarks
cargo bench

# Run a specific benchmark
cargo bench -- search_single_term

# Check benchmark compilation without running
cargo bench --no-run
```

HTML performance reports are automatically generated under:
`target/criterion/report/index.html`
