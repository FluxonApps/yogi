//! `being-persist` — a durable, crash-safe **append-only log** (build-spec §5 persistence; the
//! SQLite+fsync the journal/ledgers deferred). Pure `std`: no external crates (local-only ethos), no
//! model — safe in the automated loop.
//!
//! Today the journal, fork ledger, and dedup ledger are in-memory, so "crash-recoverable" means
//! idempotent replay within a process. This gives the missing half: records survive a process restart.
//! Each record is framed `len(u32 LE) ++ blake3-free checksum(u32 LE) ++ bytes`; [`DurableLog::replay`]
//! reads records until EOF **or the first torn/corrupt record** (a crash mid-`append` leaves a partial
//! tail, which replay stops at rather than misreading) — so the recovered prefix is always the set of
//! fully-fsynced records.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// A tiny non-cryptographic checksum (FNV-1a, 32-bit) to detect a torn/corrupt record on replay. (Not
/// a security boundary — the journal's blake3 hash-chain + signatures are that; this only catches a
/// crash-truncated tail.)
fn checksum(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for &b in bytes {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// A durable append-only log backed by one file. Single-writer (matches the single-writer-per-DID
/// journal discipline).
pub struct DurableLog {
    path: PathBuf,
    file: File,
}

impl DurableLog {
    /// Open (creating if absent) the log at `path` for appending. Existing records are preserved; new
    /// appends go after them.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(&path)?;
        Ok(Self { path, file })
    }

    /// Append one record and **fsync** it to disk before returning — so once `append` returns, the
    /// record survives a crash. Frame: `len ++ checksum ++ bytes`.
    pub fn append(&mut self, record: &[u8]) -> io::Result<()> {
        let mut framed = Vec::with_capacity(record.len() + 8);
        framed.extend_from_slice(&(record.len() as u32).to_le_bytes());
        framed.extend_from_slice(&checksum(record).to_le_bytes());
        framed.extend_from_slice(record);
        self.file.write_all(&framed)?;
        self.file.sync_data()?; // durability point
        Ok(())
    }

    /// Replay every fully-written record in order. Stops cleanly at EOF or at the first record whose
    /// frame is incomplete or whose checksum fails (a crash-truncated tail) — never misreads past it.
    pub fn replay(&self) -> io::Result<Vec<Vec<u8>>> {
        let mut f = File::open(&self.path)?;
        f.seek(SeekFrom::Start(0))?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;

        let mut out = Vec::new();
        let mut pos = 0usize;
        while pos + 8 <= buf.len() {
            let len = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap()) as usize;
            let sum = u32::from_le_bytes(buf[pos + 4..pos + 8].try_into().unwrap());
            let start = pos + 8;
            let end = start + len;
            if end > buf.len() {
                break; // torn tail: record body truncated by a crash mid-append
            }
            let body = &buf[start..end];
            if checksum(body) != sum {
                break; // corrupt/torn record — stop; the prefix is the durable set
            }
            out.push(body.to_vec());
            pos = end;
        }
        Ok(out)
    }

    /// Number of fully-durable records currently in the log.
    pub fn len(&self) -> io::Result<usize> {
        Ok(self.replay()?.len())
    }

    pub fn is_empty(&self) -> io::Result<bool> {
        Ok(self.len()? == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    // Unique temp path per test (no external tempfile dep), so parallel tests don't collide.
    fn temp_path(tag: &str) -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        std::env::temp_dir().join(format!("being_persist_{tag}_{pid}_{n}.log"))
    }

    #[test]
    fn append_then_replay_roundtrips_in_order() {
        let p = temp_path("roundtrip");
        let _ = std::fs::remove_file(&p);
        {
            let mut log = DurableLog::open(&p).unwrap();
            log.append(b"alpha").unwrap();
            log.append(b"beta").unwrap();
            log.append(b"gamma").unwrap();
        }
        let log = DurableLog::open(&p).unwrap();
        let recs = log.replay().unwrap();
        assert_eq!(
            recs,
            vec![b"alpha".to_vec(), b"beta".to_vec(), b"gamma".to_vec()]
        );
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn records_survive_reopen_and_appends_continue() {
        let p = temp_path("reopen");
        let _ = std::fs::remove_file(&p);
        DurableLog::open(&p).unwrap().append(b"first").unwrap();
        // simulate "restart": a fresh handle sees the prior record and appends after it
        let mut log = DurableLog::open(&p).unwrap();
        assert_eq!(log.len().unwrap(), 1);
        log.append(b"second").unwrap();
        assert_eq!(
            DurableLog::open(&p).unwrap().replay().unwrap(),
            vec![b"first".to_vec(), b"second".to_vec()]
        );
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn torn_tail_from_a_crash_is_dropped_not_misread() {
        let p = temp_path("torn");
        let _ = std::fs::remove_file(&p);
        {
            let mut log = DurableLog::open(&p).unwrap();
            log.append(b"committed-1").unwrap();
            log.append(b"committed-2").unwrap();
        }
        // Simulate a crash mid-append: append a partial frame (a length header claiming more bytes
        // than follow) directly to the file.
        {
            let mut raw = OpenOptions::new().append(true).open(&p).unwrap();
            raw.write_all(&(999u32).to_le_bytes()).unwrap(); // claims 999 bytes
            raw.write_all(&(0u32).to_le_bytes()).unwrap(); // checksum
            raw.write_all(b"only-a-few").unwrap(); // …but far fewer follow
            raw.sync_data().unwrap();
        }
        // Replay recovers exactly the two committed records and stops at the torn tail.
        let recs = DurableLog::open(&p).unwrap().replay().unwrap();
        assert_eq!(recs, vec![b"committed-1".to_vec(), b"committed-2".to_vec()]);
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn empty_log_replays_to_nothing() {
        let p = temp_path("empty");
        let _ = std::fs::remove_file(&p);
        let log = DurableLog::open(&p).unwrap();
        assert!(log.is_empty().unwrap());
        assert_eq!(log.replay().unwrap().len(), 0);
        std::fs::remove_file(&p).ok();
    }
}
