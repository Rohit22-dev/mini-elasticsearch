use criterion::{criterion_group, criterion_main, Criterion};
use mini_elasticsearch::engine::SearchEngine;

fn setup_engine_with_docs(count: usize) -> SearchEngine {
    let mut engine = SearchEngine::new();
    for i in 0..count {
        engine.add_document(
            &format!("doc_{}", i),
            &format!("Title of document {}", i),
            &format!(
                "The quick brown fox jumps over the lazy dog in document {} with some words like ogre and dragon and bride",
                i
            ),
        );
    }
    engine
}

fn bench_indexing(c: &mut Criterion) {
    c.bench_function("index_1k_docs", |b| {
        b.iter(|| {
            let mut engine = SearchEngine::new();
            for i in 0..1000 {
                engine.add_document(
                    &format!("doc_{}", i),
                    "Sample Title",
                    "The quick brown fox jumps over the lazy dog",
                );
            }
        });
    });
}

fn bench_search(c: &mut Criterion) {
    let engine = setup_engine_with_docs(1000);

    c.bench_function("search_single_term", |b| {
        b.iter(|| engine.search("ogre"))
    });

    c.bench_function("search_phrase", |b| {
        b.iter(|| engine.search("\"ogre and dragon\""))
    });

    c.bench_function("search_prefix", |b| {
        b.iter(|| engine.search("ogr*"))
    });

    c.bench_function("search_fuzzy", |b| {
        b.iter(|| engine.search("ogr~1"))
    });
}

criterion_group!(benches, bench_indexing, bench_search);
criterion_main!(benches);
