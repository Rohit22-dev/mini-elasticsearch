use std::fs;
use std::path::PathBuf;

use mini_elasticsearch::index::InvertedIndex;
use mini_elasticsearch::storage::merge::merge_segments;
use mini_elasticsearch::storage::segment::{SegmentReader, SegmentWriter};
use mini_elasticsearch::storage::wal::{Wal, WalRecord};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mini_es_test_{}_{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_segment_write_and_read_roundtrip() {
    let dir = temp_dir("segment_roundtrip");
    let seg_path = dir.join("test.seg");

    let mut index = InvertedIndex::new();
    index.add_document("1", "First Doc", "The ogre kidnapped the bride");
    index.add_document("2", "Second Doc", "The dragon burned the village");

    SegmentWriter::write(&index, &seg_path).expect("Failed to write segment");
    assert!(seg_path.exists());

    let loaded = SegmentReader::read(&seg_path).expect("Failed to read segment");

    assert_eq!(loaded.doc_count(), 2);
    assert_eq!(loaded.get_document("1").unwrap().title, "First Doc");
    assert_eq!(loaded.get_document("2").unwrap().title, "Second Doc");

    let ogre_postings = loaded.get_postings("ogre").expect("ogre postings missing");
    assert_eq!(ogre_postings.len(), 1);
    assert_eq!(ogre_postings[0].doc_id, "1");
    assert!(!ogre_postings[0].positions.is_empty());

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_segment_read_mmap() {
    let dir = temp_dir("segment_mmap");
    let seg_path = dir.join("test_mmap.seg");

    let mut index = InvertedIndex::new();
    index.add_document("10", "Mmap Title", "Testing memory mapped segment reading in Rust");
    index.add_document("20", "Fast Search", "Page cache accelerates postings traversal");

    SegmentWriter::write(&index, &seg_path).unwrap();

    let loaded = SegmentReader::read_mmap(&seg_path).expect("Failed to mmap read segment");

    assert_eq!(loaded.doc_count(), 2);
    assert!(loaded.get_postings("memory").is_some());
    assert!(loaded.get_postings("traversal").is_some());

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_segment_invalid_magic() {
    let dir = temp_dir("segment_invalid");
    let bad_path = dir.join("bad.seg");
    fs::write(&bad_path, b"BADMAGICDATA123456789012345678901234567890").unwrap();

    let res = SegmentReader::read(&bad_path);
    assert!(res.is_err());

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_wal_append_replay_truncate() {
    let dir = temp_dir("wal_test");
    let wal_path = dir.join("wal.log");

    let mut wal = Wal::open(&wal_path).expect("Failed to open WAL");
    wal.append_add("1", "Doc 1", "Body 1").unwrap();
    wal.append_add("2", "Doc 2", "Body 2").unwrap();
    wal.append_delete("1").unwrap();

    let records = wal.replay().unwrap();
    assert_eq!(records.len(), 3);
    assert_eq!(
        records[0],
        WalRecord::Add {
            id: "1".into(),
            title: "Doc 1".into(),
            body: "Body 1".into(),
        }
    );
    assert_eq!(
        records[1],
        WalRecord::Add {
            id: "2".into(),
            title: "Doc 2".into(),
            body: "Body 2".into(),
        }
    );
    assert_eq!(records[2], WalRecord::Delete { id: "1".into() });

    // Test truncate
    wal.truncate().unwrap();
    let records_after = wal.replay().unwrap();
    assert!(records_after.is_empty());

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn test_merge_segments() {
    let dir = temp_dir("merge_test");
    let seg1_path = dir.join("seg1.seg");
    let seg2_path = dir.join("seg2.seg");
    let out_path = dir.join("merged.seg");

    let mut idx1 = InvertedIndex::new();
    idx1.add_document("1", "Title 1", "alpha beta");
    SegmentWriter::write(&idx1, &seg1_path).unwrap();

    let mut idx2 = InvertedIndex::new();
    idx2.add_document("2", "Title 2", "beta gamma");
    SegmentWriter::write(&idx2, &seg2_path).unwrap();

    merge_segments(&[seg1_path.clone(), seg2_path.clone()], &out_path).unwrap();

    assert!(out_path.exists());
    // Source files should be removed after successful merge
    assert!(!seg1_path.exists());
    assert!(!seg2_path.exists());

    let merged = SegmentReader::read_mmap(&out_path).unwrap();
    assert_eq!(merged.doc_count(), 2);
    assert!(merged.get_postings("alpha").is_some());
    assert!(merged.get_postings("gamma").is_some());

    let beta_postings = merged.get_postings("beta").unwrap();
    assert_eq!(beta_postings.len(), 2);

    let _ = fs::remove_dir_all(dir);
}
