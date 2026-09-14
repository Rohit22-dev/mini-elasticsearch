use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use memmap2::Mmap;

use crate::index::{Document, IndexMetadata, InvertedIndex, Posting};

pub const MAGIC_BYTES: &[u8; 4] = b"MNSE";
pub const FORMAT_VERSION: u32 = 1;

/// Serializes an in-memory `InvertedIndex` into an immutable binary segment file on disk.
pub struct SegmentWriter;

impl SegmentWriter {
    /// Write the inverted index to a segment file at `path`.
    pub fn write(index: &InvertedIndex, path: &Path) -> io::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        let terms = index.index();
        let docs = index.documents();
        let metadata = index.get_metadata();

        // 1. Pre-encode postings data to compute exact offsets
        let mut postings_bytes = Vec::new();
        // (term, postings_offset_in_postings_section, postings_len)
        let mut term_offsets = Vec::with_capacity(terms.len());

        for (term, postings) in terms {
            let offset_start = postings_bytes.len();
            postings_bytes.extend_from_slice(&(postings.len() as u32).to_le_bytes());

            for p in postings {
                let id_bytes = p.doc_id.as_bytes();
                postings_bytes.extend_from_slice(&(id_bytes.len() as u16).to_le_bytes());
                postings_bytes.extend_from_slice(id_bytes);
                postings_bytes.extend_from_slice(&p.term_frequency.to_le_bytes());
                postings_bytes.extend_from_slice(&(p.positions.len() as u32).to_le_bytes());
                for pos in &p.positions {
                    postings_bytes.extend_from_slice(&pos.to_le_bytes());
                }
            }
            let len = postings_bytes.len() - offset_start;
            term_offsets.push((term, offset_start, len as u32));
        }

        // 2. Pre-encode document store
        let mut doc_store_bytes = Vec::new();
        for doc in docs.values() {
            let id_bytes = doc.id.as_bytes();
            let title_bytes = doc.title.as_bytes();
            let body_bytes = doc.body.as_bytes();
            let doc_length = metadata.doc_lengths.get(&doc.id).copied().unwrap_or(0);

            doc_store_bytes.extend_from_slice(&(id_bytes.len() as u16).to_le_bytes());
            doc_store_bytes.extend_from_slice(id_bytes);
            doc_store_bytes.extend_from_slice(&doc_length.to_le_bytes());
            doc_store_bytes.extend_from_slice(&(title_bytes.len() as u16).to_le_bytes());
            doc_store_bytes.extend_from_slice(title_bytes);
            doc_store_bytes.extend_from_slice(&(body_bytes.len() as u32).to_le_bytes());
            doc_store_bytes.extend_from_slice(body_bytes);
        }

        // 3. Compute section offsets
        // Header size:
        // magic(4) + version(4) + num_terms(4) + num_docs(4) + total_doc_length(8)
        // + postings_section_offset(8) + doc_store_section_offset(8) = 40 bytes
        let header_size = 40u64;

        let mut term_dict_size = 0u64;
        for (term, _, _) in &term_offsets {
            // term_len(2) + term_bytes + postings_offset(8) + postings_len(4)
            term_dict_size += 2 + term.as_bytes().len() as u64 + 8 + 4;
        }

        let postings_section_offset = header_size + term_dict_size;
        let doc_store_section_offset = postings_section_offset + postings_bytes.len() as u64;

        // 4. Write Header
        writer.write_all(MAGIC_BYTES)?;
        writer.write_all(&FORMAT_VERSION.to_le_bytes())?;
        writer.write_all(&(terms.len() as u32).to_le_bytes())?;
        writer.write_all(&(docs.len() as u32).to_le_bytes())?;
        writer.write_all(&metadata.total_doc_length.to_le_bytes())?;
        writer.write_all(&postings_section_offset.to_le_bytes())?;
        writer.write_all(&doc_store_section_offset.to_le_bytes())?;

        // 5. Write Term Dictionary
        for (term, rel_offset, len) in term_offsets {
            let term_bytes = term.as_bytes();
            let abs_offset = postings_section_offset + rel_offset as u64;
            writer.write_all(&(term_bytes.len() as u16).to_le_bytes())?;
            writer.write_all(term_bytes)?;
            writer.write_all(&abs_offset.to_le_bytes())?;
            writer.write_all(&len.to_le_bytes())?;
        }

        // 6. Write Postings Section
        writer.write_all(&postings_bytes)?;

        // 7. Write Document Store Section
        writer.write_all(&doc_store_bytes)?;

        writer.flush()?;
        Ok(())
    }
}

/// Reads a binary segment file from disk, reconstructing an `InvertedIndex`.
pub struct SegmentReader;

impl SegmentReader {
    /// Read a segment file into memory using standard file I/O.
    pub fn read(path: &Path) -> io::Result<InvertedIndex> {
        let bytes = std::fs::read(path)?;
        Self::parse_from_bytes(&bytes)
    }

    /// Read a segment file using memory-mapped I/O (`mmap`).
    pub fn read_mmap(path: &Path) -> io::Result<InvertedIndex> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        Self::parse_from_bytes(&mmap)
    }

    /// Parse raw segment bytes into an `InvertedIndex`.
    pub fn parse_from_bytes(data: &[u8]) -> io::Result<InvertedIndex> {
        if data.len() < 40 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Segment file too short for header",
            ));
        }

        if &data[0..4] != MAGIC_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid segment magic bytes",
            ));
        }

        let version = u32::from_le_bytes(data[4..8].try_into().unwrap());
        if version != FORMAT_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported segment format version: {}", version),
            ));
        }

        let num_terms = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
        let num_docs = u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;
        let total_doc_length = u64::from_le_bytes(data[16..24].try_into().unwrap());
        let _postings_section_offset = u64::from_le_bytes(data[24..32].try_into().unwrap()) as usize;
        let doc_store_section_offset = u64::from_le_bytes(data[32..40].try_into().unwrap()) as usize;

        let mut offset = 40;

        // Parse Term Dictionary: entries with (term, postings_offset, postings_len)
        let mut term_entries = Vec::with_capacity(num_terms);
        for _ in 0..num_terms {
            if offset + 2 > data.len() {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected EOF in term dict"));
            }
            let term_len = u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap()) as usize;
            offset += 2;

            if offset + term_len + 12 > data.len() {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected EOF in term entry"));
            }
            let term_str = std::str::from_utf8(&data[offset..offset + term_len])
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
                .to_string();
            offset += term_len;

            let postings_offset = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap()) as usize;
            offset += 8;
            let postings_len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
            offset += 4;

            term_entries.push((term_str, postings_offset, postings_len));
        }

        // Parse Postings for each term
        let mut index = BTreeMap::new();
        for (term, p_offset, _p_len) in term_entries {
            let mut cur = p_offset;
            if cur + 4 > data.len() {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected EOF in postings header"));
            }
            let num_postings = u32::from_le_bytes(data[cur..cur + 4].try_into().unwrap()) as usize;
            cur += 4;

            let mut postings = Vec::with_capacity(num_postings);
            for _ in 0..num_postings {
                if cur + 2 > data.len() {
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected EOF in posting doc_id_len"));
                }
                let id_len = u16::from_le_bytes(data[cur..cur + 2].try_into().unwrap()) as usize;
                cur += 2;

                if cur + id_len + 8 > data.len() {
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected EOF in posting header"));
                }
                let doc_id = std::str::from_utf8(&data[cur..cur + id_len])
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
                    .to_string();
                cur += id_len;

                let term_frequency = u32::from_le_bytes(data[cur..cur + 4].try_into().unwrap());
                cur += 4;
                let num_positions = u32::from_le_bytes(data[cur..cur + 4].try_into().unwrap()) as usize;
                cur += 4;

                if cur + num_positions * 4 > data.len() {
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected EOF in posting positions"));
                }
                let mut positions = Vec::with_capacity(num_positions);
                for _ in 0..num_positions {
                    let pos = u32::from_le_bytes(data[cur..cur + 4].try_into().unwrap());
                    cur += 4;
                    positions.push(pos);
                }

                postings.push(Posting {
                    doc_id,
                    term_frequency,
                    positions,
                });
            }

            index.insert(term, postings);
        }

        // Parse Document Store
        let mut documents = HashMap::with_capacity(num_docs);
        let mut doc_lengths = HashMap::with_capacity(num_docs);
        let mut cur = doc_store_section_offset;

        for _ in 0..num_docs {
            if cur + 2 > data.len() {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected EOF in doc store id_len"));
            }
            let id_len = u16::from_le_bytes(data[cur..cur + 2].try_into().unwrap()) as usize;
            cur += 2;

            if cur + id_len + 4 + 2 > data.len() {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected EOF in doc store header"));
            }
            let doc_id = std::str::from_utf8(&data[cur..cur + id_len])
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
                .to_string();
            cur += id_len;

            let doc_length = u32::from_le_bytes(data[cur..cur + 4].try_into().unwrap());
            cur += 4;

            let title_len = u16::from_le_bytes(data[cur..cur + 2].try_into().unwrap()) as usize;
            cur += 2;

            if cur + title_len + 4 > data.len() {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected EOF in doc title"));
            }
            let title = std::str::from_utf8(&data[cur..cur + title_len])
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
                .to_string();
            cur += title_len;

            let body_len = u32::from_le_bytes(data[cur..cur + 4].try_into().unwrap()) as usize;
            cur += 4;

            if cur + body_len > data.len() {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected EOF in doc body"));
            }
            let body = std::str::from_utf8(&data[cur..cur + body_len])
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
                .to_string();
            cur += body_len;

            doc_lengths.insert(doc_id.clone(), doc_length);
            documents.insert(
                doc_id.clone(),
                Document {
                    id: doc_id,
                    title,
                    body,
                },
            );
        }

        let metadata = IndexMetadata {
            total_docs: num_docs as u64,
            total_doc_length,
            doc_lengths,
        };

        Ok(InvertedIndex::from_parts(index, documents, metadata))
    }
}
