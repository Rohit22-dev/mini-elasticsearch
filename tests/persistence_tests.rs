use std::fs;
use std::path::PathBuf;

use mini_elasticsearch::engine::SearchEngine;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mini_es_persist_{}_{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_auto_flush_on_threshold() {
    let dir = temp_dir("auto_flush");
    let mut engine = SearchEngine::open(&dir, 2).expect("Failed to open engine");

    engine.add_document("1", "Doc 1", "first test document");
    assert_eq!(engine.segment_count(), 0);

    // Adding 2nd doc hits flush_threshold=2
    engine.add_document("2", "Doc 2", "second test document");
    assert_eq!(engine.segment_count(), 1);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_restart_and_persistence() {
    let dir = temp_dir("restart");

    {
        let mut engine = SearchEngine::open(&dir, 100).expect("Failed to open engine");
        engine.add_document("1", "Ogre Tale", "The ogre kidnapped the bride");
        engine.add_document("2", "Dragon Tale", "The dragon burned the village");
        let flushed = engine.flush().unwrap();
        assert!(flushed.is_some());
        assert_eq!(engine.segment_count(), 1);
    } // Engine dropped here

    // Reopen from disk
    let reopened = SearchEngine::open(&dir, 100).expect("Failed to reopen engine");
    assert_eq!(reopened.doc_count(), 2);
    assert_eq!(reopened.segment_count(), 1);

    let results = reopened.search("ogre");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].doc_id, "1");
    assert!(results[0].score > 0.0);

    let doc = reopened.get_document("2").expect("Doc 2 missing");
    assert_eq!(doc.title, "Dragon Tale");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_wal_crash_recovery() {
    let dir = temp_dir("crash_recovery");

    {
        // High threshold so auto-flush does NOT trigger
        let mut engine = SearchEngine::open(&dir, 1000).expect("Failed to open engine");
        engine.add_document("unflushed_1", "Emergency Backup", "This document was not flushed before crash");
        // Do NOT call flush()! Simulate sudden shutdown / drop.
    }

    // Reopen — should replay from WAL
    let recovered = SearchEngine::open(&dir, 1000).expect("Failed to recover engine");
    assert_eq!(recovered.doc_count(), 1);

    let results = recovered.search("emergency");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].doc_id, "unflushed_1");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_compaction() {
    let dir = temp_dir("compaction");
    let mut engine = SearchEngine::open(&dir, 100).expect("Failed to open engine");

    engine.add_document("1", "Part 1", "first batch");
    engine.flush().unwrap();

    engine.add_document("2", "Part 2", "second batch");
    engine.flush().unwrap();

    engine.add_document("3", "Part 3", "third batch");
    engine.flush().unwrap();

    assert_eq!(engine.segment_count(), 3);
    assert_eq!(engine.stats().segment_count, 3);

    // Run compaction
    engine.compact().expect("Compaction failed");
    assert_eq!(engine.segment_count(), 1);
    assert_eq!(engine.stats().segment_count, 1);

    // Verify all docs are still present and searchable
    assert_eq!(engine.doc_count(), 3);
    assert_eq!(engine.search("batch").len(), 3);

    let _ = fs::remove_dir_all(dir);
}
