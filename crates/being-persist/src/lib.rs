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

/// Scan framed bytes into (records, valid-prefix-byte-length), stopping at the first torn/corrupt frame.
/// The byte length is where the durable prefix ends — everything after it is a crash-truncated tail.
fn scan(buf: &[u8]) -> (Vec<Vec<u8>>, usize) {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos + 8 <= buf.len() {
        let len = u32::from_le_bytes(buf[pos..pos + 4].try_into().unwrap()) as usize;
        let sum = u32::from_le_bytes(buf[pos + 4..pos + 8].try_into().unwrap());
        let start = pos + 8;
        let end = start + len;
        if end > buf.len() {
            break; // torn tail: body truncated by a crash mid-append
        }
        let body = &buf[start..end];
        if checksum(body) != sum {
            break; // corrupt/torn frame — the prefix is the durable set
        }
        out.push(body.to_vec());
        pos = end;
    }
    (out, pos)
}

impl DurableLog {
    /// Open (creating if absent) the log at `path` for appending. Existing fully-durable records are
    /// preserved; **a torn tail from a prior crash is truncated on open** so subsequent appends stay
    /// contiguous (otherwise new records would be stranded after the torn bytes and lost on the next
    /// replay). After `open`, the file holds exactly the recoverable prefix.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        // Find the durable-prefix length and drop any torn tail before opening for append.
        let valid_len = match File::open(&path) {
            Ok(mut f) => {
                let mut buf = Vec::new();
                f.read_to_end(&mut buf)?;
                scan(&buf).1 as u64
            }
            Err(_) => 0,
        };
        let file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(&path)?;
        if file.metadata()?.len() > valid_len {
            file.set_len(valid_len)?; // remove the crash-truncated tail
        }
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
        Ok(scan(&buf).0)
    }

    /// Number of fully-durable records currently in the log.
    pub fn len(&self) -> io::Result<usize> {
        Ok(self.replay()?.len())
    }

    pub fn is_empty(&self) -> io::Result<bool> {
        Ok(self.len()? == 0)
    }
}

/// A durable insert-only set of 32-byte ids, backed by a [`DurableLog`]. This is exactly what the M6
/// `ForkLedger` and the M1 `DedupLedger` need to survive a restart: today they dedup by content-address
/// in memory (idempotent within a process); persisting the committed ids here makes at-most-once hold
/// **across** a crash/restart too — reopen replays the log to rebuild the set.
pub struct DurableIdSet {
    log: DurableLog,
    ids: std::collections::BTreeSet<[u8; 32]>,
}

impl DurableIdSet {
    /// Open the set at `path`, rebuilding membership from the durable log (torn tail dropped).
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let log = DurableLog::open(path)?;
        let mut ids = std::collections::BTreeSet::new();
        for rec in log.replay()? {
            if let Ok(arr) = <[u8; 32]>::try_from(rec.as_slice()) {
                ids.insert(arr);
            }
        }
        Ok(Self { log, ids })
    }

    /// Insert `id`, persisting it durably. Returns `true` if newly inserted, `false` if already present
    /// (idempotent — a duplicate insert appends nothing, mirroring the ledgers' at-most-once semantics).
    pub fn insert(&mut self, id: [u8; 32]) -> io::Result<bool> {
        if self.ids.contains(&id) {
            return Ok(false);
        }
        self.log.append(&id)?;
        self.ids.insert(id);
        Ok(true)
    }

    pub fn contains(&self, id: &[u8; 32]) -> bool {
        self.ids.contains(id)
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
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
    fn recovers_a_valid_record_prefix_from_a_crash_at_any_byte_offset() {
        // Exhaustive crash-recovery: build a log, then simulate a crash truncating the file at EVERY
        // possible byte offset. Recovery must always yield a whole-record PREFIX (never a misread or
        // panic), and a post-recovery append must be contiguous and replayable.
        let records: Vec<Vec<u8>> = vec![
            b"a".to_vec(),
            b"bb".to_vec(),
            b"ccc".to_vec(),
            vec![7u8; 40],
        ];
        let src = temp_path("crash_src");
        let _ = std::fs::remove_file(&src);
        {
            let mut log = DurableLog::open(&src).unwrap();
            for r in &records {
                log.append(r).unwrap();
            }
        }
        let full = std::fs::read(&src).unwrap();

        for l in 0..=full.len() {
            let p = temp_path("crash_at");
            let _ = std::fs::remove_file(&p);
            std::fs::write(&p, &full[..l]).unwrap(); // crash truncated the file to l bytes

            // Recovery yields a prefix of the original records — for some k, recovered == records[..k].
            let recovered = DurableLog::open(&p).unwrap().replay().unwrap();
            assert_eq!(
                recovered,
                records[..recovered.len()].to_vec(),
                "crash@{l}: not a valid record prefix"
            );

            // After recovery the torn tail is gone, so a fresh append is contiguous and replayable.
            DurableLog::open(&p).unwrap().append(b"new").unwrap();
            let mut expect = recovered.clone();
            expect.push(b"new".to_vec());
            assert_eq!(
                DurableLog::open(&p).unwrap().replay().unwrap(),
                expect,
                "crash@{l}: post-recovery append lost or corrupted"
            );
            std::fs::remove_file(&p).ok();
        }
        std::fs::remove_file(&src).ok();
    }

    #[test]
    fn append_after_a_torn_tail_does_not_lose_records() {
        // Regression: a crash leaves a torn tail; on reopen it must be truncated so a NEW append is
        // contiguous and replayable (else the new record is stranded after the torn bytes and lost).
        let p = temp_path("recover");
        let _ = std::fs::remove_file(&p);
        {
            let mut log = DurableLog::open(&p).unwrap();
            log.append(b"committed-1").unwrap();
            log.append(b"committed-2").unwrap();
        }
        // Simulate a crash mid-append: a partial frame appended directly.
        {
            let mut raw = OpenOptions::new().append(true).open(&p).unwrap();
            raw.write_all(&(999u32).to_le_bytes()).unwrap();
            raw.write_all(&(0u32).to_le_bytes()).unwrap();
            raw.write_all(b"torn").unwrap();
            raw.sync_data().unwrap();
        }
        // Reopen (truncates the torn tail) and append a fresh record.
        {
            let mut log = DurableLog::open(&p).unwrap();
            assert_eq!(log.len().unwrap(), 2); // torn tail dropped on open
            log.append(b"committed-3").unwrap();
        }
        // The new record is NOT lost — replay sees all three contiguous records.
        let recs = DurableLog::open(&p).unwrap().replay().unwrap();
        assert_eq!(
            recs,
            vec![
                b"committed-1".to_vec(),
                b"committed-2".to_vec(),
                b"committed-3".to_vec()
            ]
        );
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn durable_id_set_survives_restart_and_is_idempotent() {
        let p = temp_path("idset");
        let _ = std::fs::remove_file(&p);
        let a = [1u8; 32];
        let b = [2u8; 32];
        {
            let mut s = DurableIdSet::open(&p).unwrap();
            assert!(s.insert(a).unwrap()); // newly inserted
            assert!(!s.insert(a).unwrap()); // duplicate → no-op (at-most-once)
            assert!(s.insert(b).unwrap());
            assert_eq!(s.len(), 2);
        }
        // "Restart": a fresh handle rebuilds membership from the durable log.
        let s = DurableIdSet::open(&p).unwrap();
        assert!(s.contains(&a) && s.contains(&b));
        assert_eq!(s.len(), 2);
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
