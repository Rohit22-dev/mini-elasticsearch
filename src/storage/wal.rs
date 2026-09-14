use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// An operation recorded in the Write-Ahead Log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum WalRecord {
    Add {
        id: String,
        title: String,
        body: String,
    },
    Delete {
        id: String,
    },
}

/// Append-only Write-Ahead Log for crash resilience.
///
/// Every mutating operation (`add_document`, `delete_document`) is appended to
/// the log file before modifying in-memory state. On startup, uncommitted log
/// entries are replayed. When a segment is flushed, the log is truncated.
pub struct Wal {
    path: PathBuf,
    file: File,
}

impl Wal {
    /// Open or create a WAL file at `path`.
    pub fn open(path: &Path) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(path)?;

        Ok(Self {
            path: path.to_path_buf(),
            file,
        })
    }

    /// Append a document addition entry to the WAL.
    pub fn append_add(&mut self, id: &str, title: &str, body: &str) -> io::Result<()> {
        let record = WalRecord::Add {
            id: id.to_string(),
            title: title.to_string(),
            body: body.to_string(),
        };
        self.append_record(&record)
    }

    /// Append a document deletion entry to the WAL.
    pub fn append_delete(&mut self, id: &str) -> io::Result<()> {
        let record = WalRecord::Delete {
            id: id.to_string(),
        };
        self.append_record(&record)
    }

    fn append_record(&mut self, record: &WalRecord) -> io::Result<()> {
        let json = serde_json::to_string(record)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        writeln!(self.file, "{}", json)?;
        self.file.flush()?;
        Ok(())
    }

    /// Replay all records currently stored in the WAL.
    pub fn replay(&self) -> io::Result<Vec<WalRecord>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut records = Vec::new();

        for line_res in reader.lines() {
            let line = line_res?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let record: WalRecord = serde_json::from_str(trimmed)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            records.push(record);
        }

        Ok(records)
    }

    /// Truncate the WAL to 0 bytes after a segment flush.
    pub fn truncate(&mut self) -> io::Result<()> {
        self.file.set_len(0)?;
        self.file.seek(SeekFrom::Start(0))?;
        self.file.flush()?;
        Ok(())
    }

    /// Return the path to the WAL file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}
