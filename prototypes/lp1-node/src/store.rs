//! LP2: durable candidate-header journal (local experimental storage, NOT a protocol format).
//!
//! # Scope
//! Records are header *candidates* admitted after strict decoding and the scoped H-pre checks of
//! `Window::link` (NetID, items 2-9, height, parent, H_END) with the ceiling `U256::MAX` (the RP
//! `viewTargetCeil` is a client rule, not admission) and no wall clock. Nothing here is H-full,
//! executed, canonical, live or consensus: status always reports `candidate`, `awaitingHFull`,
//! `executedHeight = 0`, `consensus = false`. The store is linear: it only accepts the next height
//! on its own tip. A conflicting or sibling candidate is refused as `notLinear` (unsupported by this
//! store), which is never a global block-invalidity verdict and is not cached anywhere.
//!
//! # Journal format, version 1 (experimental; may change without migration)
//! One file `<store dir>/candidates.lp2j`. All integers big-endian.
//!
//! Metadata, exactly `META_BYTES` = 128 bytes:
//! ```text
//!   0..8    magic        "PCLP2J01"
//!   8..12   version      u32 = 1
//!   12..16  bodyLen      u32 = 72
//!   16..48  profileSha256   SHA-256 of the exact profile.json bytes the store was created with
//!   48..80  genesisHash     from the profile (keccak of the literal genesis preimage)
//!   80..88  chainId      u64
//!   88..120 metaChecksum SHA-256(bytes 0..88)
//!   120..128 marker      "LP2META!"
//! ```
//! Record `i` (i = 1, 2, ...; `REC_FIXED` = 120 bytes plus the header):
//! ```text
//!   0..4    magic        "LP2R"
//!   4..12   seq          u64 = i = header height
//!   12..16  len          u32, 1..=3067 (HDR_MAX)
//!   16..20  prefixGuard  first 4 bytes of SHA-256(prevChecksum || bytes 0..16)
//!   20..52  prevHash     blockHash of record i-1, or genesisHash for i = 1
//!   52..84  blockHash    keccak(tid || be64(nonce) || shareRoot) of this header
//!   84..84+len           header: the exact strict RLP item
//!   +32     checksum     SHA-256(prevChecksum || bytes 0..84+len), prevChecksum = metaChecksum for i = 1
//!   +4      marker       "CMT!"  (commit marker)
//! ```
//! Records are chained twice (prevHash and the checksum chain), so reordering, duplication,
//! substitution and any byte flip are detected. The prefix guard protects the length field as soon
//! as 20 bytes of a record exist, so a corrupted length in the final record is reported as
//! corruption instead of being mistaken for a torn tail (and silently dropped by recovery).
//! Every record is fully re-validated on every open (no trusted cached metadata).
//!
//! # Bounds (checked before allocation)
//! At most `MAX_RECORDS` records, `MAX_FILE_BYTES` file bytes, `MAX_BATCH` headers per append,
//! `header::HDR_MAX` header bytes, `MAX_PROFILE_BYTES` profile bytes.
//!
//! # Open classification
//! * fewer than 128 bytes: `metadataIncomplete` (never an empty successful store);
//! * any metadata field/checksum/marker wrong: `metadataCorrupt`;
//! * metadata valid but profile SHA / genesis / chainId differ from the supplied profile: `profileMismatch`;
//! * a *complete* record (all its bytes present) with any wrong field, invalid header, wrong hash,
//!   checksum or marker: `corrupt` (never skipped);
//! * a final, incomplete record any of whose present bytes already disagree with what can be derived
//!   at that point: `corrupt`. This includes magic, seq, prevHash and marker prefixes, and also
//!   partial digests (LP2-I02): once the first 16 record bytes exist, every present byte of the
//!   prefix guard is compared with the recomputed guard; once the complete header exists, every
//!   present byte of the checksum is compared with the recomputed checksum, even when fewer than
//!   32 checksum bytes are present. (The blockHash field cannot be checked before the header is
//!   complete, because it is derived from the header.)
//! * otherwise a final record that is a strict, consistent prefix of a record: torn tail. Readers
//!   report the verified prefix and `recoveryRequired`; writers refuse to open.
//!
//! # Writer / reader contract
//! * Writer (`Writer::open`, `init`): Windows only. The journal is opened with
//!   `OpenOptionsExt::share_mode(0)`, so no other handle (reader or writer, any process) can open it
//!   while the writer lives; the OS releases it when the process exits or crashes. No lock files.
//!   On other platforms `Writer::open` returns `unsupportedPlatform` (no portable lock is claimed);
//!   read-only operations still work there but report `exclusion: none`.
//! * Reader (`read_status`, `recover` source): opens read-only with share mode READ, so it never
//!   observes a concurrently written tail; it fails with `busy` while a writer holds the store.
//! * Append: the whole batch is decoded and validated (in order, against the simulated tip) before
//!   any byte is written. Then each record is written with `write_all` followed by `sync_all`, and
//!   only after both succeed does the in-memory tip advance. Atomicity is per record, NOT per
//!   batch: a crash mid-batch leaves a durable prefix of the batch (plus possibly a torn record).
//! * Any write or sync error poisons the writer: every later append fails with `poisoned` and the
//!   store must be reopened (which then reports a clean or torn tail). A record whose write
//!   succeeded but whose sync failed is not reported as committed by that writer; if its bytes
//!   reached the disk, a later open validates and lists it like any other record.
//! * Recovery never modifies the source. It copies the verified prefix into a journal inside a
//!   NEW destination directory (created with `create_dir`, file with `create_new`), syncs it,
//!   reopens it with full validation, and re-checks that the source SHA-256 is unchanged.
//!   A corrupt (as opposed to torn) source is not recovered.
//! * Directory entries are not fsynced (std offers no portable directory sync on Windows); the
//!   journal file itself is synced. This is a documented limitation of the experimental store.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::fixed::U256;
use crate::fixtures;
use crate::hashes::{sha256, H256};
use crate::header::{self, Header};
use crate::hex;
use crate::json;
use crate::window::{NetConfig, Parent, Window};

pub const JOURNAL_FILE: &str = "candidates.lp2j";
pub const META_MAGIC: [u8; 8] = *b"PCLP2J01";
pub const FORMAT_VERSION: u32 = 1;
pub const META_BODY_LEN: u32 = 72;
pub const META_MARKER: [u8; 8] = *b"LP2META!";
pub const META_BYTES: usize = 128;
pub const REC_MAGIC: [u8; 4] = *b"LP2R";
pub const REC_MARKER: [u8; 4] = *b"CMT!";
/// Record bytes excluding the header.
pub const REC_FIXED: usize = 120;
/// Offset of the prefix guard inside a record.
pub const REC_GUARD_AT: usize = 16;
/// Offset of the header inside a record.
pub const REC_HEADER_AT: usize = 84;
pub const MAX_HEADER_BYTES: usize = header::HDR_MAX;
pub const MAX_RECORD_BYTES: usize = REC_FIXED + MAX_HEADER_BYTES;
pub const MAX_RECORDS: u64 = 1024;
pub const MAX_FILE_BYTES: u64 = META_BYTES as u64 + MAX_RECORDS * MAX_RECORD_BYTES as u64;
pub const MAX_BATCH: usize = 256;
pub const MAX_PROFILE_BYTES: u64 = 65536;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreError {
    /// init/recover target already exists; nothing is overwritten.
    Exists(String),
    NoStore(String),
    /// Another handle (writer, or reader while opening a writer) holds the journal.
    Busy(String),
    UnsupportedPlatform(String),
    Io(String),
    Bound(String),
    Profile(String),
    MetadataIncomplete {
        have: u64,
    },
    MetadataCorrupt(&'static str),
    ProfileMismatch(&'static str),
    Corrupt {
        seq: u64,
        offset: u64,
        reason: String,
    },
    RecoveryRequired {
        verified_height: u64,
        verified_bytes: u64,
        file_bytes: u64,
    },
    /// Append input failed validation; nothing was written.
    Rejected {
        index: usize,
        height: Option<u64>,
        rule: String,
        detail: String,
    },
    /// Append input is not the next height on this linear store's tip; nothing was written.
    NotLinear {
        index: usize,
        height: u64,
        tip: u64,
        reason: &'static str,
    },
    /// A write or sync failed; `committed` records of this batch were durably committed before it.
    WriteFailed {
        committed: u64,
        detail: String,
    },
    Poisoned,
}

impl StoreError {
    pub fn kind(&self) -> &'static str {
        match self {
            StoreError::Exists(_) => "exists",
            StoreError::NoStore(_) => "noStore",
            StoreError::Busy(_) => "busy",
            StoreError::UnsupportedPlatform(_) => "unsupportedPlatform",
            StoreError::Io(_) => "io",
            StoreError::Bound(_) => "bound",
            StoreError::Profile(_) => "profile",
            StoreError::MetadataIncomplete { .. } => "metadataIncomplete",
            StoreError::MetadataCorrupt(_) => "metadataCorrupt",
            StoreError::ProfileMismatch(_) => "profileMismatch",
            StoreError::Corrupt { .. } => "corrupt",
            StoreError::RecoveryRequired { .. } => "recoveryRequired",
            StoreError::Rejected { .. } => "rejected",
            StoreError::NotLinear { .. } => "notLinear",
            StoreError::WriteFailed { .. } => "writeFailed",
            StoreError::Poisoned => "poisoned",
        }
    }

    pub fn to_json(&self) -> String {
        let detail = match self {
            StoreError::Exists(s)
            | StoreError::NoStore(s)
            | StoreError::Busy(s)
            | StoreError::UnsupportedPlatform(s)
            | StoreError::Io(s)
            | StoreError::Bound(s)
            | StoreError::Profile(s) => format!("\"detail\":{}", json::quote(s)),
            StoreError::MetadataIncomplete { have } => format!("\"haveBytes\":{have}"),
            StoreError::MetadataCorrupt(s) | StoreError::ProfileMismatch(s) => format!("\"detail\":{}", json::quote(s)),
            StoreError::Corrupt { seq, offset, reason } => format!("\"seq\":{seq},\"offset\":{offset},\"detail\":{}", json::quote(reason)),
            StoreError::RecoveryRequired { verified_height, verified_bytes, file_bytes } => {
                format!("\"verifiedHeight\":{verified_height},\"verifiedBytes\":{verified_bytes},\"fileBytes\":{file_bytes}")
            }
            StoreError::Rejected { index, height, rule, detail } => format!(
                "\"index\":{index},\"height\":{},\"rule\":{},\"detail\":{}",
                height.map(|h| h.to_string()).unwrap_or_else(|| "null".into()),
                json::quote(rule),
                json::quote(detail)
            ),
            StoreError::NotLinear { index, height, tip, reason } => {
                format!("\"index\":{index},\"height\":{height},\"tip\":{tip},\"detail\":{},\"verdict\":\"unsupportedByLinearStore\"", json::quote(reason))
            }
            StoreError::WriteFailed { committed, detail } => {
                format!("\"committedInBatch\":{committed},\"detail\":{},\"reopenRequired\":true", json::quote(detail))
            }
            StoreError::Poisoned => "\"reopenRequired\":true".to_string(),
        };
        format!("{{\"error\":{{\"kind\":\"{}\",{detail}}}}}", self.kind())
    }
}

fn io_err(e: io::Error, what: &str) -> StoreError {
    StoreError::Io(format!("{what}: {e}"))
}

/// Exact profile binding: SHA-256 of the supplied profile bytes plus the validated network config.
#[derive(Clone, Debug)]
pub struct Binding {
    pub profile_sha256: H256,
    pub cfg: NetConfig,
}

impl Binding {
    pub fn from_profile_bytes(bytes: &[u8]) -> Result<Binding, StoreError> {
        if bytes.len() as u64 > MAX_PROFILE_BYTES {
            return Err(StoreError::Bound("profile exceeds 65536 bytes".into()));
        }
        let v: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| StoreError::Profile(e.to_string()))?;
        let p = fixtures::load_profile(&v).map_err(StoreError::Profile)?;
        Ok(Binding { profile_sha256: sha256(bytes), cfg: NetConfig::from_profile(&p) })
    }

    pub fn from_fixtures_dir(dir: &Path) -> Result<Binding, StoreError> {
        let p = dir.join("profile.json");
        let len = fs::metadata(&p).map_err(|e| io_err(e, "profile.json"))?.len();
        if len > MAX_PROFILE_BYTES {
            return Err(StoreError::Bound("profile exceeds 65536 bytes".into()));
        }
        let bytes = fs::read(&p).map_err(|e| io_err(e, "profile.json"))?;
        Binding::from_profile_bytes(&bytes)
    }

    fn meta_bytes(&self) -> [u8; META_BYTES] {
        let mut m = [0u8; META_BYTES];
        m[0..8].copy_from_slice(&META_MAGIC);
        m[8..12].copy_from_slice(&FORMAT_VERSION.to_be_bytes());
        m[12..16].copy_from_slice(&META_BODY_LEN.to_be_bytes());
        m[16..48].copy_from_slice(&self.profile_sha256);
        m[48..80].copy_from_slice(&self.cfg.genesis_hash);
        m[80..88].copy_from_slice(&self.cfg.chain_id.to_be_bytes());
        let ck = sha256(&m[0..88]);
        m[88..120].copy_from_slice(&ck);
        m[120..128].copy_from_slice(&META_MARKER);
        m
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecInfo {
    pub seq: u64,
    pub offset: u64,
    pub len: u32,
    pub block_hash: H256,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tail {
    Clean,
    Torn { offset: u64, bytes: u64 },
}

/// Result of a full validating scan of journal bytes.
#[derive(Clone, Debug)]
pub struct Scan {
    pub records: Vec<RecInfo>,
    pub verified_end: u64,
    pub file_len: u64,
    pub tail: Tail,
    pub tip: Option<Header>,
    pub tip_raw: Vec<u8>,
    pub last_checksum: H256,
    pub journal_sha256: H256,
}

impl Scan {
    pub fn height(&self) -> u64 {
        self.records.len() as u64
    }

    pub fn tip_hash(&self, genesis: &H256) -> H256 {
        self.records.last().map(|r| r.block_hash).unwrap_or(*genesis)
    }
}

fn be_u32(b: &[u8]) -> u32 {
    let mut a = [0u8; 4];
    a.copy_from_slice(&b[..4]);
    u32::from_be_bytes(a)
}

fn be_u64(b: &[u8]) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[..8]);
    u64::from_be_bytes(a)
}

/// Strictly decodes `raw` and links it to `tip` (or the genesis parent) with the scoped H-pre
/// checks. Returns the header and its block hash, or (rule, detail).
pub fn validate_candidate(cfg: &NetConfig, tip: Option<&Header>, raw: &[u8]) -> Result<(Header, H256), (String, String)> {
    if raw.len() > MAX_HEADER_BYTES {
        return Err(("1".into(), "size".into()));
    }
    let hd = header::decode(raw).map_err(|e| ("1".to_string(), e.detail.to_string()))?;
    let mut w = Window::new(cfg, true);
    let (hs, xi, p) = match tip {
        None => (vec![hd], 0usize, Parent::Genesis),
        Some(t) => (vec![t.clone(), hd], 1usize, Parent::Idx(0)),
    };
    w.link(&hs, xi, p, None, &U256::MAX).map_err(|e| (e.rule.to_string(), e.detail.to_string()))?;
    let bh = w.block_hash(&hs, xi);
    let hd = hs.into_iter().nth(xi).ok_or_else(|| ("1".to_string(), "internal".to_string()))?;
    Ok((hd, bh))
}

/// First 4 bytes of SHA-256(prevChecksum || record bytes 0..16).
fn prefix_guard(prev_ck: &H256, first16: &[u8]) -> [u8; 4] {
    let mut pre = [0u8; 48];
    pre[..32].copy_from_slice(prev_ck);
    pre[32..].copy_from_slice(&first16[..16]);
    let h = sha256(&pre);
    [h[0], h[1], h[2], h[3]]
}

/// SHA-256(prevChecksum || record bytes 0..84+len).
fn record_checksum(prev_ck: &H256, through_header: &[u8]) -> H256 {
    let mut pre = Vec::with_capacity(32 + through_header.len());
    pre.extend_from_slice(prev_ck);
    pre.extend_from_slice(through_header);
    sha256(&pre)
}

/// Builds record `seq`; returns (record bytes, its checksum).
fn build_record(seq: u64, prev_hash: &H256, block_hash: &H256, raw: &[u8], prev_ck: &H256) -> (Vec<u8>, H256) {
    let mut r = Vec::with_capacity(REC_FIXED + raw.len());
    r.extend_from_slice(&REC_MAGIC);
    r.extend_from_slice(&seq.to_be_bytes());
    r.extend_from_slice(&(raw.len() as u32).to_be_bytes());
    let g = prefix_guard(prev_ck, &r[..16]);
    r.extend_from_slice(&g);
    r.extend_from_slice(prev_hash);
    r.extend_from_slice(block_hash);
    r.extend_from_slice(raw);
    let ck = record_checksum(prev_ck, &r);
    r.extend_from_slice(&ck);
    r.extend_from_slice(&REC_MARKER);
    (r, ck)
}

fn corrupt(seq: u64, offset: usize, reason: &str) -> StoreError {
    StoreError::Corrupt { seq, offset: offset as u64, reason: reason.to_string() }
}

/// Compares the present prefix of a field with its expected value.
fn prefix_matches(present: &[u8], expected: &[u8]) -> bool {
    let n = present.len().min(expected.len());
    present[..n] == expected[..n]
}

/// Full validating scan. Never trusts anything that is not re-derived from the bytes and binding.
pub fn scan(bytes: &[u8], binding: &Binding) -> Result<Scan, StoreError> {
    let n = bytes.len();
    if n as u64 > MAX_FILE_BYTES {
        return Err(StoreError::Bound(format!("journal is {n} bytes, limit {MAX_FILE_BYTES}")));
    }
    if n < META_BYTES {
        if !prefix_matches(bytes, &META_MAGIC) {
            return Err(StoreError::MetadataCorrupt("magic"));
        }
        return Err(StoreError::MetadataIncomplete { have: n as u64 });
    }
    let m = &bytes[..META_BYTES];
    if m[0..8] != META_MAGIC {
        return Err(StoreError::MetadataCorrupt("magic"));
    }
    if be_u32(&m[8..12]) != FORMAT_VERSION {
        return Err(StoreError::MetadataCorrupt("version"));
    }
    if be_u32(&m[12..16]) != META_BODY_LEN {
        return Err(StoreError::MetadataCorrupt("bodyLen"));
    }
    let meta_ck = sha256(&m[0..88]);
    if m[88..120] != meta_ck {
        return Err(StoreError::MetadataCorrupt("checksum"));
    }
    if m[120..128] != META_MARKER {
        return Err(StoreError::MetadataCorrupt("marker"));
    }
    if m[16..48] != binding.profile_sha256 {
        return Err(StoreError::ProfileMismatch("profileSha256"));
    }
    if m[48..80] != binding.cfg.genesis_hash {
        return Err(StoreError::ProfileMismatch("genesisHash"));
    }
    if be_u64(&m[80..88]) != binding.cfg.chain_id {
        return Err(StoreError::ProfileMismatch("chainId"));
    }
    let cfg = &binding.cfg;
    let mut o = META_BYTES;
    let mut records: Vec<RecInfo> = Vec::new();
    let mut prev_hash = cfg.genesis_hash;
    let mut prev_ck = meta_ck;
    let mut tip: Option<Header> = None;
    let mut tip_raw: Vec<u8> = Vec::new();
    let mut tail = Tail::Clean;
    while o < n {
        let seq = records.len() as u64 + 1;
        let r = &bytes[o..];
        let rem = r.len();
        if !prefix_matches(r, &REC_MAGIC) {
            return Err(corrupt(seq, o, "record magic"));
        }
        if seq > MAX_RECORDS {
            return Err(corrupt(seq, o, "record count bound"));
        }
        if !prefix_matches(&r[4.min(rem)..rem.min(12)], &seq.to_be_bytes()) {
            return Err(corrupt(seq, o, "sequence"));
        }
        if rem < 16 {
            tail = Tail::Torn { offset: o as u64, bytes: rem as u64 };
            break;
        }
        let len = be_u32(&r[12..16]) as usize;
        if len == 0 || len > MAX_HEADER_BYTES {
            return Err(corrupt(seq, o, "declared header length out of bounds"));
        }
        // The first 16 bytes exist, so the guard is computable: every present guard byte must match,
        // including a partial guard (LP2-I02).
        let guard = prefix_guard(&prev_ck, &r[..16]);
        if !prefix_matches(&r[REC_GUARD_AT..rem.min(REC_GUARD_AT + 4)], &guard) {
            return Err(corrupt(seq, o, "prefix guard (magic/seq/len)"));
        }
        if rem < REC_GUARD_AT + 4 {
            // Consistent partial guard: the length is not yet protected, so this is a torn tail.
            tail = Tail::Torn { offset: o as u64, bytes: rem as u64 };
            break;
        }
        if !prefix_matches(&r[20..rem.min(52)], &prev_hash) {
            return Err(corrupt(seq, o, "prevHash link"));
        }
        let total = REC_FIXED + len;
        let hdr_end = REC_HEADER_AT + len;
        let mut this_hash: Option<(Header, H256)> = None;
        let mut this_ck: Option<H256> = None;
        if rem >= hdr_end {
            let raw = &r[REC_HEADER_AT..hdr_end];
            let (hd, bh) = validate_candidate(cfg, tip.as_ref(), raw).map_err(|(rule, d)| corrupt(seq, o, &format!("header rule {rule} ({d})")))?;
            if hd.h != seq {
                return Err(corrupt(seq, o, "header height differs from sequence"));
            }
            if r[52..84] != bh {
                return Err(corrupt(seq, o, "blockHash"));
            }
            this_hash = Some((hd, bh));
            // The complete checksum preimage exists: every present checksum byte must match, even
            // when fewer than 32 are present (LP2-I02).
            let ck = record_checksum(&prev_ck, &r[..hdr_end]);
            if !prefix_matches(&r[hdr_end..rem.min(hdr_end + 32)], &ck) {
                return Err(corrupt(seq, o, "checksum"));
            }
            this_ck = Some(ck);
        }
        if rem > hdr_end + 32 && !prefix_matches(&r[hdr_end + 32..rem.min(total)], &REC_MARKER) {
            return Err(corrupt(seq, o, "commit marker"));
        }
        if rem < total {
            tail = Tail::Torn { offset: o as u64, bytes: rem as u64 };
            break;
        }
        // Complete record: every field was verified above.
        let (hd, bh) = match this_hash {
            Some(x) => x,
            None => return Err(corrupt(seq, o, "internal: header not verified")),
        };
        prev_ck = match this_ck {
            Some(c) => c,
            None => return Err(corrupt(seq, o, "internal: checksum not verified")),
        };
        records.push(RecInfo { seq, offset: o as u64, len: len as u32, block_hash: bh });
        prev_hash = bh;
        tip_raw = r[REC_HEADER_AT..hdr_end].to_vec();
        tip = Some(hd);
        o += total;
    }
    let verified_end = match tail {
        Tail::Clean => n as u64,
        Tail::Torn { offset, .. } => offset,
    };
    Ok(Scan { records, verified_end, file_len: n as u64, tail, tip, tip_raw, last_checksum: prev_ck, journal_sha256: sha256(bytes) })
}

pub fn journal_path(dir: &Path) -> PathBuf {
    dir.join(JOURNAL_FILE)
}

#[cfg(windows)]
const ERROR_SHARING_VIOLATION: i32 = 32;
#[cfg(windows)]
const ERROR_LOCK_VIOLATION: i32 = 33;

fn map_open_err(e: io::Error, p: &Path) -> StoreError {
    #[cfg(windows)]
    {
        if e.raw_os_error() == Some(ERROR_SHARING_VIOLATION) || e.raw_os_error() == Some(ERROR_LOCK_VIOLATION) {
            return StoreError::Busy(format!("{} is held by another handle", p.display()));
        }
    }
    if e.kind() == io::ErrorKind::NotFound {
        return StoreError::NoStore(format!("{} not found", p.display()));
    }
    io_err(e, &p.display().to_string())
}

/// Writer handle options: read + write, no sharing at all (Windows).
#[cfg(windows)]
fn writer_options(create_new: bool) -> Result<OpenOptions, StoreError> {
    use std::os::windows::fs::OpenOptionsExt;
    let mut o = OpenOptions::new();
    o.read(true).write(true).share_mode(0);
    if create_new {
        o.create_new(true);
    }
    Ok(o)
}

#[cfg(not(windows))]
fn writer_options(_create_new: bool) -> Result<OpenOptions, StoreError> {
    Err(StoreError::UnsupportedPlatform(
        "exclusive single-writer enforcement is implemented only on Windows (share_mode 0); no portable lock is claimed".into(),
    ))
}

/// Reader handle options: read-only, share READ only (denies writers while reading).
fn reader_options() -> OpenOptions {
    let mut o = OpenOptions::new();
    o.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        o.share_mode(1); // FILE_SHARE_READ
    }
    o
}

pub fn exclusion_label() -> &'static str {
    if cfg!(windows) {
        "windows-share-mode-0"
    } else {
        "none-unsupported-platform"
    }
}

/// Reads at most `MAX_FILE_BYTES`; the length is checked before allocating.
fn read_bounded(f: &mut File) -> Result<Vec<u8>, StoreError> {
    let len = f.metadata().map_err(|e| io_err(e, "metadata"))?.len();
    if len > MAX_FILE_BYTES {
        return Err(StoreError::Bound(format!("journal is {len} bytes, limit {MAX_FILE_BYTES}")));
    }
    f.seek(SeekFrom::Start(0)).map_err(|e| io_err(e, "seek"))?;
    let mut v = Vec::with_capacity(len as usize);
    Read::by_ref(f).take(MAX_FILE_BYTES + 1).read_to_end(&mut v).map_err(|e| io_err(e, "read"))?;
    if v.len() as u64 > MAX_FILE_BYTES {
        return Err(StoreError::Bound("journal grew beyond the limit while reading".into()));
    }
    Ok(v)
}

/// Seam between the writer and the journal file. Production uses `File`; tests may wrap the file
/// (which keeps the exclusive handle) to inject write/sync failures or a process abort. The node
/// binary never wraps it and has no fault options.
pub trait JournalIo: Send {
    fn write_all(&mut self, b: &[u8]) -> io::Result<()>;
    fn sync_all(&mut self) -> io::Result<()>;
}

impl JournalIo for File {
    fn write_all(&mut self, b: &[u8]) -> io::Result<()> {
        Write::write_all(self, b)
    }
    fn sync_all(&mut self) -> io::Result<()> {
        File::sync_all(self)
    }
}

#[derive(Clone, Debug)]
pub struct Status {
    pub height: u64,
    pub tip_hash: H256,
    pub file_len: u64,
    pub verified_end: u64,
    pub tail: Tail,
    pub journal_sha256: H256,
    pub profile_sha256: H256,
    pub genesis_hash: H256,
    pub chain_id: u64,
}

impl Status {
    fn from_scan(s: &Scan, b: &Binding) -> Status {
        Status {
            height: s.height(),
            tip_hash: s.tip_hash(&b.cfg.genesis_hash),
            file_len: s.file_len,
            verified_end: s.verified_end,
            tail: s.tail,
            journal_sha256: s.journal_sha256,
            profile_sha256: b.profile_sha256,
            genesis_hash: b.cfg.genesis_hash,
            chain_id: b.cfg.chain_id,
        }
    }

    pub fn recovery_required(&self) -> bool {
        self.tail != Tail::Clean
    }

    pub fn to_json(&self) -> String {
        let (tail, torn) = match self.tail {
            Tail::Clean => ("clean", 0),
            Tail::Torn { bytes, .. } => ("recoveryRequired", bytes),
        };
        format!(
            "{{\"store\":\"lp2-candidate-journal\",\"format\":{FORMAT_VERSION},\"experimental\":true,\"status\":\"candidate\",\"awaiting\":\"H-full\",\"awaitingHFull\":true,\"candidateHeight\":{},\"executedHeight\":0,\"consensus\":false,\"canonical\":false,\"live\":false,\"tipHash\":\"{}\",\"tail\":\"{tail}\",\"tornBytes\":{torn},\"verifiedBytes\":{},\"fileBytes\":{},\"journalSha256\":\"{}\",\"profileSha256\":\"{}\",\"genesisHash\":\"{}\",\"chainId\":{},\"exclusion\":\"{}\"}}",
            self.height,
            hex::encode(&self.tip_hash),
            self.verified_end,
            self.file_len,
            hex::encode(&self.journal_sha256),
            hex::encode(&self.profile_sha256),
            hex::encode(&self.genesis_hash),
            self.chain_id,
            exclusion_label()
        )
    }
}

/// Creates a NEW store directory and journal with metadata only. Refuses any existing directory.
pub fn init(dir: &Path, binding: &Binding) -> Result<Status, StoreError> {
    let opts = writer_options(true)?;
    if dir.exists() {
        return Err(StoreError::Exists(format!("{} already exists; stores are never overwritten", dir.display())));
    }
    fs::create_dir(dir).map_err(|e| {
        if e.kind() == io::ErrorKind::AlreadyExists {
            StoreError::Exists(format!("{} already exists", dir.display()))
        } else {
            io_err(e, "create store directory")
        }
    })?;
    let p = journal_path(dir);
    let mut f = opts.open(&p).map_err(|e| map_open_err(e, &p))?;
    let meta = binding.meta_bytes();
    Write::write_all(&mut f, &meta).map_err(|e| io_err(e, "write metadata"))?;
    f.sync_all().map_err(|e| io_err(e, "sync metadata"))?;
    let bytes = read_bounded(&mut f)?;
    let s = scan(&bytes, binding)?;
    Ok(Status::from_scan(&s, binding))
}

/// Read-only validated status (reader contract). A torn tail is reported, not an error.
pub fn read_status(dir: &Path, binding: &Binding) -> Result<Status, StoreError> {
    let (_bytes, s) = read_scan(dir, binding)?;
    Ok(Status::from_scan(&s, binding))
}

fn read_scan(dir: &Path, binding: &Binding) -> Result<(Vec<u8>, Scan), StoreError> {
    let p = journal_path(dir);
    let mut f = reader_options().open(&p).map_err(|e| map_open_err(e, &p))?;
    let bytes = read_bounded(&mut f)?;
    let s = scan(&bytes, binding)?;
    Ok((bytes, s))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppendReport {
    pub appended: Vec<u64>,
    pub idempotent: u64,
    pub height: u64,
}

impl AppendReport {
    pub fn to_json(&self) -> String {
        let a: Vec<String> = self.appended.iter().map(|h| h.to_string()).collect();
        format!(
            "{{\"appended\":[{}],\"idempotentTipReplays\":{},\"candidateHeight\":{},\"executedHeight\":0,\"consensus\":false,\"status\":\"candidate\"}}",
            a.join(","),
            self.idempotent,
            self.height
        )
    }
}

/// Exclusive writer (Windows). Holds the journal handle for its whole life.
pub struct Writer {
    io: Box<dyn JournalIo>,
    binding: Binding,
    tip: Option<Header>,
    tip_raw: Vec<u8>,
    tip_hash: H256,
    height: u64,
    last_ck: H256,
    end: u64,
    poisoned: bool,
}

impl Writer {
    /// Opens an existing store for appending. Refuses busy, mismatched, corrupt or torn stores.
    pub fn open(dir: &Path, binding: &Binding) -> Result<Writer, StoreError> {
        Writer::open_with(dir, binding, |f| -> Box<dyn JournalIo> { Box::new(f) })
    }

    /// As `open`, with a wrapper around the exclusive file handle (test seam; see `JournalIo`).
    pub fn open_with<F: FnOnce(File) -> Box<dyn JournalIo>>(dir: &Path, binding: &Binding, wrap: F) -> Result<Writer, StoreError> {
        let opts = writer_options(false)?;
        let p = journal_path(dir);
        let mut f = opts.open(&p).map_err(|e| map_open_err(e, &p))?;
        let bytes = read_bounded(&mut f)?;
        let s = scan(&bytes, binding)?;
        if let Tail::Torn { .. } = s.tail {
            return Err(StoreError::RecoveryRequired { verified_height: s.height(), verified_bytes: s.verified_end, file_bytes: s.file_len });
        }
        f.seek(SeekFrom::Start(s.file_len)).map_err(|e| io_err(e, "seek end"))?;
        Ok(Writer {
            io: wrap(f),
            binding: binding.clone(),
            tip_hash: s.tip_hash(&binding.cfg.genesis_hash),
            height: s.height(),
            tip: s.tip,
            tip_raw: s.tip_raw,
            last_ck: s.last_checksum,
            end: s.file_len,
            poisoned: false,
        })
    }

    pub fn height(&self) -> u64 {
        self.height
    }

    pub fn tip_hash(&self) -> H256 {
        self.tip_hash
    }

    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    /// Committed file length known to this writer.
    pub fn committed_bytes(&self) -> u64 {
        self.end
    }

    /// Validates the whole batch, then writes and syncs record by record (per-record atomicity).
    pub fn append(&mut self, batch: &[Vec<u8>]) -> Result<AppendReport, StoreError> {
        if self.poisoned {
            return Err(StoreError::Poisoned);
        }
        if batch.len() > MAX_BATCH {
            return Err(StoreError::Bound(format!("batch of {} headers exceeds {MAX_BATCH}", batch.len())));
        }
        // Phase 1: validate everything against a simulated tip; no I/O.
        let cfg = self.binding.cfg.clone();
        let mut sim_tip = self.tip.clone();
        let mut sim_raw = self.tip_raw.clone();
        let mut sim_hash = self.tip_hash;
        let mut sim_height = self.height;
        let mut sim_ck = self.last_ck;
        let mut planned: Vec<(u64, Vec<u8>, H256, Header, Vec<u8>, H256)> = Vec::new();
        let mut idempotent = 0u64;
        for (index, raw) in batch.iter().enumerate() {
            if raw.len() > MAX_HEADER_BYTES {
                return Err(StoreError::Bound(format!("header {index} is {} bytes, limit {MAX_HEADER_BYTES}", raw.len())));
            }
            let hd = header::decode(raw).map_err(|e| StoreError::Rejected { index, height: None, rule: "1".into(), detail: e.detail.into() })?;
            if sim_height > 0 && hd.h == sim_height && *raw == sim_raw {
                idempotent += 1;
                continue;
            }
            if Some(hd.h) != sim_height.checked_add(1) {
                let reason = if hd.h <= sim_height {
                    "height already stored with different bytes (conflicting or sibling candidate)"
                } else {
                    "gap: not the next height"
                };
                return Err(StoreError::NotLinear { index, height: hd.h, tip: sim_height, reason });
            }
            if hd.h > MAX_RECORDS {
                return Err(StoreError::Bound(format!("record count limit {MAX_RECORDS}")));
            }
            let (hd, bh) =
                validate_candidate(&cfg, sim_tip.as_ref(), raw).map_err(|(rule, detail)| StoreError::Rejected { index, height: Some(hd.h), rule, detail })?;
            let (rec, ck) = build_record(hd.h, &sim_hash, &bh, raw, &sim_ck);
            sim_height = hd.h;
            sim_hash = bh;
            sim_ck = ck;
            sim_raw = raw.clone();
            sim_tip = Some(hd.clone());
            planned.push((hd.h, rec, bh, hd, raw.clone(), ck));
        }
        if self.end + planned.iter().map(|p| p.1.len() as u64).sum::<u64>() > MAX_FILE_BYTES {
            return Err(StoreError::Bound("journal file size limit".into()));
        }
        // Phase 2: write + sync each record; advance in-memory state only after both succeed.
        let mut report = AppendReport { appended: Vec::new(), idempotent, height: self.height };
        for (h, rec, bh, hd, raw, ck) in planned {
            if let Err(e) = self.io.write_all(&rec) {
                self.poisoned = true;
                return Err(StoreError::WriteFailed { committed: report.appended.len() as u64, detail: format!("write: {e}") });
            }
            if let Err(e) = self.io.sync_all() {
                self.poisoned = true;
                return Err(StoreError::WriteFailed { committed: report.appended.len() as u64, detail: format!("sync: {e}") });
            }
            self.end += rec.len() as u64;
            self.height = h;
            self.tip_hash = bh;
            self.last_ck = ck;
            self.tip = Some(hd);
            self.tip_raw = raw;
            report.appended.push(h);
            report.height = h;
        }
        report.height = self.height;
        Ok(report)
    }
}

#[derive(Clone, Debug)]
pub struct RecoverReport {
    pub source_sha256: H256,
    pub dest_sha256: H256,
    pub height: u64,
    pub discarded_tail_bytes: u64,
    pub dest_status: Status,
}

impl RecoverReport {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"recovered\":true,\"sourceSha256\":\"{}\",\"sourceUnchanged\":true,\"destSha256\":\"{}\",\"candidateHeight\":{},\"discardedTailBytes\":{},\"destStatus\":{}}}",
            hex::encode(&self.source_sha256),
            hex::encode(&self.dest_sha256),
            self.height,
            self.discarded_tail_bytes,
            self.dest_status.to_json()
        )
    }
}

/// Copies the verified prefix of `src` into a journal in the NEW directory `dest`.
pub fn recover(src: &Path, dest: &Path, binding: &Binding) -> Result<RecoverReport, StoreError> {
    if dest.exists() {
        return Err(StoreError::Exists(format!("{} already exists; recovery writes only to a new directory", dest.display())));
    }
    let (bytes, s) = read_scan(src, binding)?;
    let source_sha = s.journal_sha256;
    let discarded = s.file_len - s.verified_end;
    fs::create_dir(dest).map_err(|e| {
        if e.kind() == io::ErrorKind::AlreadyExists {
            StoreError::Exists(format!("{} already exists", dest.display()))
        } else {
            io_err(e, "create destination directory")
        }
    })?;
    let dp = journal_path(dest);
    let mut o = OpenOptions::new();
    o.write(true).create_new(true);
    let mut f = o.open(&dp).map_err(|e| map_open_err(e, &dp))?;
    Write::write_all(&mut f, &bytes[..s.verified_end as usize]).map_err(|e| io_err(e, "write destination"))?;
    f.sync_all().map_err(|e| io_err(e, "sync destination"))?;
    drop(f);
    // Fresh, fully validating reopen of the destination.
    let (dbytes, ds) = read_scan(dest, binding)?;
    if ds.tail != Tail::Clean || ds.height() != s.height() || dbytes[..] != bytes[..s.verified_end as usize] {
        return Err(StoreError::Io("destination did not reopen as the identical clean verified prefix".into()));
    }
    // The source must be untouched.
    let (_b2, s2) = read_scan(src, binding)?;
    if s2.journal_sha256 != source_sha {
        return Err(StoreError::Io("source changed during recovery".into()));
    }
    Ok(RecoverReport {
        source_sha256: source_sha,
        dest_sha256: ds.journal_sha256,
        height: ds.height(),
        discarded_tail_bytes: discarded,
        dest_status: Status::from_scan(&ds, binding),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_constants() {
        assert_eq!(META_BYTES, 8 + 4 + 4 + META_BODY_LEN as usize + 32 + 8);
        assert_eq!(REC_FIXED, 4 + 8 + 4 + 4 + 32 + 32 + 32 + 4);
        assert_eq!(REC_GUARD_AT, 4 + 8 + 4);
        assert_eq!(REC_HEADER_AT, 4 + 8 + 4 + 4 + 32 + 32);
        assert_eq!(MAX_RECORD_BYTES, 3187);
        assert_eq!(MAX_FILE_BYTES, 128 + 1024 * 3187);
    }

    #[test]
    fn record_builder_layout() {
        let (r, ck) = build_record(7, &[1u8; 32], &[2u8; 32], &[0xc0], &[3u8; 32]);
        assert_eq!(r.len(), REC_FIXED + 1);
        assert_eq!(&r[0..4], b"LP2R");
        assert_eq!(be_u64(&r[4..12]), 7);
        assert_eq!(be_u32(&r[12..16]), 1);
        let mut g = vec![3u8; 32];
        g.extend_from_slice(&r[..16]);
        assert_eq!(&r[16..20], &sha256(&g)[..4]);
        assert_eq!(&r[20..52], &[1u8; 32]);
        assert_eq!(&r[52..84], &[2u8; 32]);
        assert_eq!(r[84], 0xc0);
        let mut pre = vec![3u8; 32];
        pre.extend_from_slice(&r[..85]);
        assert_eq!(sha256(&pre), ck);
        assert_eq!(record_checksum(&[3u8; 32], &r[..85]), ck);
        assert_eq!(&r[85..117], &ck);
        assert_eq!(&r[117..], b"CMT!");
    }

    #[test]
    fn prefix_match_semantics() {
        assert!(prefix_matches(&[], &[1, 2, 3]));
        assert!(prefix_matches(&[1, 2], &[1, 2, 3]));
        assert!(!prefix_matches(&[1, 3], &[1, 2, 3]));
        assert!(prefix_matches(&[1, 2, 3, 9], &[1, 2, 3]));
    }

    #[test]
    fn error_json_shapes() {
        let e = StoreError::NotLinear { index: 0, height: 5, tip: 7, reason: "x" };
        assert!(e.to_json().contains("\"kind\":\"notLinear\"") && e.to_json().contains("unsupportedByLinearStore"));
        assert!(StoreError::MetadataIncomplete { have: 3 }.to_json().contains("\"haveBytes\":3"));
        assert!(StoreError::Poisoned.to_json().contains("reopenRequired"));
    }
}
