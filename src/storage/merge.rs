use std::io;
use std::path::{Path, PathBuf};

use crate::index::InvertedIndex;
use crate::storage::segment::{SegmentReader, SegmentWriter};

/// Merge multiple segment files into a single unified segment file.
///
/// 1. Reads all input segments into memory via mmap.
/// 2. Combines postings, documents, and updates corpus metadata.
/// 3. Writes out the combined segment to `output_path`.
/// 4. Deletes the original segment files upon successful write.
pub fn merge_segments(segment_paths: &[PathBuf], output_path: &Path) -> io::Result<()> {
    if segment_paths.is_empty() {
        return Ok(());
    }

    if segment_paths.len() == 1 && segment_paths[0] == output_path {
        // Nothing to merge if only 1 segment and already at output path
        return Ok(());
    }

    let mut merged_index = InvertedIndex::new();

    for path in segment_paths {
        if !path.exists() {
            continue;
        }
        // Try mmap reading first, fall back to standard read
        let seg_index = match SegmentReader::read_mmap(path) {
            Ok(idx) => idx,
            Err(_) => SegmentReader::read(path)?,
        };
        merged_index.merge_with(seg_index);
    }

    // Write to a temporary file first for atomicity
    let temp_output = output_path.with_extension("tmp");
    SegmentWriter::write(&merged_index, &temp_output)?;

    // Atomically rename temp file to output path
    std::fs::rename(&temp_output, output_path)?;

    // Remove the merged source files
    for path in segment_paths {
        if path != output_path && path.exists() {
            let _ = std::fs::remove_file(path);
        }
    }

    Ok(())
}
