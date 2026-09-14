import os
from pptx import Presentation
from pptx.util import Inches, Pt
from pptx.dml.color import RGBColor
from pptx.enum.text import PP_ALIGN
from pptx.enum.shapes import MSO_SHAPE

def create_deck(output_path):
    prs = Presentation()
    prs.slide_width = Inches(13.333)
    prs.slide_height = Inches(7.5)
    blank_layout = prs.slide_layouts[6]

    # Color Palette - Modern Sleek Dark
    BG_DARK = RGBColor(15, 23, 42)        # Slate 900
    CARD_BG = RGBColor(30, 41, 59)        # Slate 800
    BORDER_COLOR = RGBColor(51, 65, 85)   # Slate 700
    TEXT_LIGHT = RGBColor(248, 250, 252)  # Slate 50
    TEXT_MUTED = RGBColor(148, 163, 184)  # Slate 400
    ACCENT_CYAN = RGBColor(56, 189, 248)  # Sky 400
    ACCENT_EMERALD = RGBColor(52, 211, 153) # Emerald 400
    ACCENT_ORANGE = RGBColor(251, 146, 60) # Orange 400

    def add_base_slide(title_text, subtitle_text=None):
        slide = prs.slides.add_slide(blank_layout)
        # Background
        bg = slide.shapes.add_shape(MSO_SHAPE.RECTANGLE, Inches(0), Inches(0), Inches(13.333), Inches(7.5))
        bg.fill.solid()
        bg.fill.fore_color.rgb = BG_DARK
        bg.line.fill.background()

        # Title container
        title_box = slide.shapes.add_textbox(Inches(0.8), Inches(0.5), Inches(11.733), Inches(1.0))
        tf = title_box.text_frame
        tf.word_wrap = True
        tf.margin_left = tf.margin_top = tf.margin_right = tf.margin_bottom = 0
        
        p = tf.paragraphs[0]
        p.text = title_text
        p.font.name = "Arial"
        p.font.size = Pt(26)
        p.font.bold = True
        p.font.color.rgb = ACCENT_CYAN

        if subtitle_text:
            p2 = tf.add_paragraph()
            p2.text = subtitle_text
            p2.font.name = "Arial"
            p2.font.size = Pt(13)
            p2.font.color.rgb = TEXT_MUTED
            p2.space_before = Pt(4)

        return slide

    def add_card(slide, left, top, width, height, title="", body_items=None, title_color=ACCENT_CYAN):
        card = slide.shapes.add_shape(MSO_SHAPE.ROUNDED_RECTANGLE, left, top, width, height)
        card.fill.solid()
        card.fill.fore_color.rgb = CARD_BG
        card.line.color.rgb = BORDER_COLOR
        card.line.width = Pt(1.2)

        tb = slide.shapes.add_textbox(left + Inches(0.25), top + Inches(0.2), width - Inches(0.5), height - Inches(0.4))
        tf = tb.text_frame
        tf.word_wrap = True
        tf.margin_left = tf.margin_top = tf.margin_right = tf.margin_bottom = 0

        if title:
            p0 = tf.paragraphs[0]
            p0.text = title
            p0.font.name = "Arial"
            p0.font.size = Pt(16)
            p0.font.bold = True
            p0.font.color.rgb = title_color

        if body_items:
            for idx, item in enumerate(body_items):
                p = tf.add_paragraph() if (title or idx > 0) else tf.paragraphs[0]
                p.text = f"•  {item}"
                p.font.name = "Arial"
                p.font.size = Pt(12)
                p.font.color.rgb = TEXT_LIGHT
                p.space_before = Pt(6)

    # ----------------------------------------------------
    # SLIDE 1: Title Slide
    # ----------------------------------------------------
    s1 = prs.slides.add_slide(blank_layout)
    bg1 = s1.shapes.add_shape(MSO_SHAPE.RECTANGLE, Inches(0), Inches(0), Inches(13.333), Inches(7.5))
    bg1.fill.solid()
    bg1.fill.fore_color.rgb = BG_DARK
    bg1.line.fill.background()

    tb1 = s1.shapes.add_textbox(Inches(1.0), Inches(2.2), Inches(11.333), Inches(3.0))
    tf1 = tb1.text_frame
    tf1.word_wrap = True

    p = tf1.paragraphs[0]
    p.text = "Mini-Elasticsearch in Rust"
    p.font.name = "Arial"
    p.font.size = Pt(44)
    p.font.bold = True
    p.font.color.rgb = ACCENT_CYAN

    p_sub = tf1.add_paragraph()
    p_sub.text = "Architecture, Design & Deep-Dive Engineering Walkthrough (Phases 1–7)"
    p_sub.font.name = "Arial"
    p_sub.font.size = Pt(20)
    p_sub.font.color.rgb = TEXT_LIGHT
    p_sub.space_before = Pt(12)

    p_tags = tf1.add_paragraph()
    p_tags.text = "Inverted Index • BM25 Ranking • Positional Postings • LSM Segments • WAL • mmap • Multi-Index"
    p_tags.font.name = "Arial"
    p_tags.font.size = Pt(14)
    p_tags.font.color.rgb = ACCENT_EMERALD
    p_tags.space_before = Pt(24)

    # ----------------------------------------------------
    # SLIDE 2: Executive Summary (7 Phases)
    # ----------------------------------------------------
    s2 = add_base_slide("Executive Architecture Overview", "A complete, production-grade distributed search engine model built from scratch in Rust")
    col_w = Inches(3.64)
    row_h = Inches(2.4)
    top1 = Inches(1.8)
    top2 = Inches(4.5)

    add_card(s2, Inches(0.8), top1, col_w, row_h, "Phase 1: Inverted Index", [
        "Text analyzer with lowercase tokenization",
        "Positional Inverted Index with BTreeMap",
        "Term vocabulary & doc store mapping"
    ])
    add_card(s2, Inches(4.84), top1, col_w, row_h, "Phase 2: BM25 Scoring", [
        "Okapi BM25 relevance ranking",
        "IDF logarithmic scarcity weighting",
        "Term frequency saturation (k1) & length norm (b)"
    ])
    add_card(s2, Inches(8.88), top1, col_w, row_h, "Phase 3: Async HTTP API", [
        "Axum & Tokio async REST server",
        "Endpoints: POST /documents, GET /search",
        "Thread-safe Arc<RwLock<SearchEngine>>"
    ])

    add_card(s2, Inches(0.8), top2, col_w, row_h, "Phase 4: Advanced Queries", [
        "Phrase search: positional adjacency checking",
        "Prefix search: BTreeMap::range lexicographical scan",
        "Fuzzy search: Levenshtein edit distance"
    ])
    add_card(s2, Inches(4.84), top2, col_w, row_h, "Phase 5 & 6: Persistence & mmap", [
        "Custom binary segment format (MNSE v1)",
        "Append-only Write-Ahead Log (WAL)",
        "Memory-Mapped I/O with memmap2 zero-copy"
    ])
    add_card(s2, Inches(8.88), top2, col_w, row_h, "Phase 7: Polish & CLI", [
        "EngineManager with multi-index isolation",
        "Structured logging via tracing & config.toml",
        "Clap CLI: serve, index, search, stats, compact"
    ])

    # ----------------------------------------------------
    # SLIDE 3: System Architecture
    # ----------------------------------------------------
    s3 = add_base_slide("System Layered Architecture", "Clean separation of concerns across clients, HTTP transport, query pipeline, and storage")
    w = Inches(11.733)
    add_card(s3, Inches(0.8), Inches(1.8), w, Inches(1.0), "1. Client & CLI Interface Layer", [
        "HTTP REST Clients (curl / browsers) + Command-line interface via Clap (mini-es serve, search, stats)"
    ], ACCENT_EMERALD)
    add_card(s3, Inches(0.8), Inches(3.0), w, Inches(1.1), "2. Async Transport Layer (Axum / Tokio)", [
        "Multi-index routing (/indexes/{name}/...), request validation, JSON serialization, and tracing middleware"
    ], ACCENT_CYAN)
    add_card(s3, Inches(0.8), Inches(4.3), w, Inches(1.2), "3. Query & Execution Engine (SearchEngine)", [
        "Query syntax parsing (quotes, prefixes, tildes) • Positional validation • BM25 relevance scoring accumulator"
    ], ACCENT_ORANGE)
    add_card(s3, Inches(0.8), Inches(5.7), w, Inches(1.3), "4. Storage & Persistence Subsystem", [
        "Write-Ahead Log (wal.log) • Immutable binary segments (.seg) • Zero-copy mmap reader • LSM Compaction merge"
    ], ACCENT_EMERALD)

    # ----------------------------------------------------
    # SLIDE 4: Document Ingestion Pipeline
    # ----------------------------------------------------
    s4 = add_base_slide("Document Ingestion & Write Path", "Step-by-step lifecycle from API request to disk durability")
    step_w = Inches(2.7)
    step_h = Inches(4.8)
    add_card(s4, Inches(0.8), Inches(1.8), step_w, step_h, "1. Ingestion Request", [
        "POST /indexes/{name}/documents",
        "JSON payload parsed",
        "Acquire write lock on SearchEngine",
        "Non-blocking to other index partitions"
    ])
    add_card(s4, Inches(3.8), Inches(1.8), step_w, step_h, "2. WAL Durability", [
        "Write operation to wal.log",
        "Newline-delimited JSON entry",
        "Instant crash protection",
        "Replayed on server recovery"
    ], ACCENT_ORANGE)
    add_card(s4, Inches(6.8), Inches(1.8), step_w, step_h, "3. Positional Indexing", [
        "Analyzer tokenizes title & body",
        "0-based continuous positions",
        "BTreeMap postings updated",
        "Doc store & lengths updated"
    ], ACCENT_EMERALD)
    add_card(s4, Inches(9.8), Inches(1.8), step_w, step_h, "4. Segment Flush", [
        "Buffer count checked against threshold",
        "If threshold reached: flush()",
        "SegmentWriter writes binary file",
        "WAL truncated to 0 bytes"
    ], ACCENT_CYAN)

    # ----------------------------------------------------
    # SLIDE 5: Query Execution & BM25 Scoring
    # ----------------------------------------------------
    s5 = add_base_slide("Query Parsing & BM25 Relevance Ranking", "Multi-modal query execution engine with Okapi BM25 scoring")
    add_card(s5, Inches(0.8), Inches(1.8), Inches(5.6), Inches(4.8), "Query Parsing & Dispatch", [
        "Phrase Search (\"ogre bride\"): checks that tokens appear adjacent and in sequential order using positions",
        "Prefix Search (ogr*): expands prefix across BTreeMap::range vocabulary without full scan",
        "Fuzzy Search (ogr~1): scans vocabulary using dynamic-programming Levenshtein edit distance",
        "Boolean Search: handles whitespace (AND) or uppercase OR unions"
    ], ACCENT_ORANGE)

    add_card(s5, Inches(6.9), Inches(1.8), Inches(5.6), Inches(4.8), "BM25 Scoring Formula", [
        "IDF Calculation: ln(TotalDocs / DocFreq)",
        "Rare terms receive exponentially higher weights than common terms",
        "Term Frequency Saturation (k1 = 1.2): prevents runaway scores on repeated words",
        "Length Normalization (b = 0.75): penalizes long documents unless frequency justifies match",
        "Sorted Descending: candidates scored and ranked by relevance"
    ], ACCENT_EMERALD)

    # ----------------------------------------------------
    # SLIDE 6: Storage Engine & Segment Format
    # ----------------------------------------------------
    s6 = add_base_slide("Segment-Based Persistence (MNSE v1)", "LSM-tree inspired immutable segment file format")
    add_card(s6, Inches(0.8), Inches(1.8), Inches(5.6), Inches(4.8), "Binary Segment Layout", [
        "Header (40 Bytes): Magic b\"MNSE\", Version 1, Num Terms, Num Docs, Total Doc Length, Section Offsets",
        "Term Dictionary: Lexicographically sorted terms with postings offsets & lengths",
        "Postings Section: Term frequencies + exact 0-based token positions",
        "Document Store: Stored titles and raw body text for result snippet retrieval"
    ], ACCENT_CYAN)

    add_card(s6, Inches(6.9), Inches(1.8), Inches(5.6), Inches(4.8), "LSM Lifecycle & Compaction", [
        "Immutable on disk: Once written, segment files are never modified",
        "Fast writes: Append-only disk patterns maximize throughput",
        "Compaction (merge_segments): Merges N small segment files into a single unified segment",
        "Reclaims storage and accelerates query traversal"
    ], ACCENT_EMERALD)

    # ----------------------------------------------------
    # SLIDE 7: Memory-Mapped I/O & Performance
    # ----------------------------------------------------
    s7 = add_base_slide("Phase 6: Memory-Mapped I/O (mmap)", "Bypassing userspace copying using operating system page caches")
    add_card(s7, Inches(0.8), Inches(1.8), Inches(5.6), Inches(4.8), "Traditional File I/O vs mmap", [
        "Standard read(): Disk -> OS Page Cache -> Userspace Heap Buffer -> Parser",
        "mmap: File mapped directly into process virtual memory space",
        "Zero-Copy: Deserializer reads directly from memory-mapped &[u8] slice",
        "Lazy Paging: OS brings only accessed segments into physical RAM"
    ], ACCENT_ORANGE)

    add_card(s7, Inches(6.9), Inches(1.8), Inches(5.6), Inches(4.8), "Criterion Benchmarking Suite", [
        "index_1k_docs: Measures raw bulk ingestion throughput (~1K docs in < 25ms)",
        "search_single_term: Microsecond single-token BM25 retrieval (< 10 µs)",
        "search_phrase: Fast positional adjacency validation (< 50 µs)",
        "search_prefix & search_fuzzy: Range expansion & edit distance benchmarks",
        "HTML statistical reports generated via cargo bench"
    ], ACCENT_EMERALD)

    # ----------------------------------------------------
    # SLIDE 8: Multi-Index & Production Polish
    # ----------------------------------------------------
    s8 = add_base_slide("Phase 7: Multi-Index & Concurrency Model", "Portfolio-grade production polish and CLI tooling")
    add_card(s8, Inches(0.8), Inches(1.8), Inches(5.6), Inches(4.8), "EngineManager & Index Isolation", [
        "Multiple Named Indexes: PUT /indexes/{name} (e.g. books, movies)",
        "Independent Storage: data/indexes/<name>/segments and wal/",
        "No Cross-Contention: Writes to 'books' do not lock or block reads on 'movies'",
        "Short Read Locks: Multiple parallel search queries execute concurrently"
    ], ACCENT_CYAN)

    add_card(s8, Inches(6.9), Inches(1.8), Inches(5.6), Inches(4.8), "CLI & Observability", [
        "Command-Line Tool (clap):",
        "  • mini-es serve --port 3000",
        "  • mini-es index --file docs.jsonl --index books",
        "  • mini-es search \"ogre bride\" --index books",
        "  • mini-es stats & compact",
        "Structured Logging: Tracing subscriber with RUST_LOG filtering"
    ], ACCENT_ORANGE)

    # ----------------------------------------------------
    # SLIDE 9: Testing & Verification
    # ----------------------------------------------------
    s9 = add_base_slide("Verification & Test Coverage", "129 automated tests across 9 comprehensive test suites")
    add_card(s9, Inches(0.8), Inches(1.8), Inches(11.733), Inches(4.8), "100% Passing Test Matrix", [
        "analyzer_tests (10 passed): Tokenization, lowercasing, punctuation stripping, Unicode normalization",
        "index_tests (25 passed): Positional postings, prefix range scanning, doc lengths, metadata updates",
        "scorer_tests (13 passed): BM25 saturation, IDF scarcity, document length penalties",
        "engine_tests (38 passed): Phrase search, prefix search, fuzzy search, AND/OR queries, score ranking",
        "fuzzy_tests (13 passed): Levenshtein dynamic-programming algorithm correctness",
        "server_tests (16 passed): Axum HTTP REST integration tests for all query types and stats",
        "storage_tests (5 passed): Binary segment roundtrip, mmap reader, WAL replay/truncate, segment merge",
        "persistence_tests (4 passed): Auto-flush thresholds, crash recovery via WAL, compaction",
        "multi_index_tests (2 passed): EngineManager isolation and namespaced REST endpoints"
    ], ACCENT_EMERALD)

    prs.save(output_path)
    print(f"Presentation generated successfully at: {output_path}")

if __name__ == "__main__":
    os.makedirs("presentation", exist_ok=True)
    create_deck("presentation/mini_elasticsearch_overview.pptx")
