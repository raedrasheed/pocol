//! LP2 candidate journal, in-process: P1 lifecycle, P2 validation-before-mutation and linearity,
//! P3 corruption / bounds / binding, P4 torn tails and recovery, writer poisoning. All stores live
//! in fresh temporary directories created and removed by these tests only.

mod common;

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use lp1_node::fixed::U256;
use lp1_node::hashes;
use lp1_node::header::{self, Header};
use lp1_node::store::{self, Binding, JournalIo, Scan, StoreError, Tail, Writer};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A fresh temporary directory, removed on drop. Stores are created as NEW subdirectories of it.
struct Tmp(PathBuf);

impl Tmp {
    fn new(tag: &str) -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let p = std::env::temp_dir().join(format!("lp2-test-{tag}-{}-{n}-{nanos}", std::process::id()));
        fs::create_dir_all(&p).unwrap();
        Tmp(p)
    }
    fn sub(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn binding() -> Binding {
    Binding::from_fixtures_dir(&common::dir()).unwrap()
}

fn chain() -> Vec<Vec<u8>> {
    lp1_node::fixtures::load_chain_bytes(&common::json("chain.json")).unwrap()
}

fn range(c: &[Vec<u8>], from: usize, to: usize) -> Vec<Vec<u8>> {
    c[from - 1..to].to_vec()
}

fn journal_bytes(dir: &Path) -> Vec<u8> {
    fs::read(store::journal_path(dir)).unwrap()
}

/// Test-only: writes raw journal bytes into a NEW store directory (no writer involved).
fn raw_store(dir: &Path, bytes: &[u8]) {
    fs::create_dir(dir).unwrap();
    fs::write(store::journal_path(dir), bytes).unwrap();
}

fn kind_of(r: &Result<store::Status, StoreError>) -> String {
    match r {
        Ok(s) if s.tail == Tail::Clean => format!("clean:{}", s.height),
        Ok(s) => format!("torn:{}", s.height),
        Err(e) => e.kind().to_string(),
    }
}

/// A store holding fixture headers 1..=n, built through the real writer.
fn built_store(tmp: &Tmp, name: &str, n: usize) -> PathBuf {
    let b = binding();
    let d = tmp.sub(name);
    store::init(&d, &b).unwrap();
    if n > 0 {
        let mut w = Writer::open(&d, &b).unwrap();
        w.append(&range(&chain(), 1, n)).unwrap();
    }
    d
}

fn scan_of(bytes: &[u8]) -> Result<Scan, StoreError> {
    store::scan(bytes, &binding())
}

#[test]
fn p1_init_status_and_never_overwrite() {
    let tmp = Tmp::new("init");
    let b = binding();
    let d = tmp.sub("s");
    let s = store::init(&d, &b).unwrap();
    assert_eq!((s.height, s.file_len, s.tail), (0, store::META_BYTES as u64, Tail::Clean));
    assert_eq!(s.tip_hash, b.cfg.genesis_hash);
    let j = s.to_json();
    for want in
        ["\"candidateHeight\":0", "\"executedHeight\":0", "\"consensus\":false", "\"awaiting\":\"H-full\"", "\"status\":\"candidate\"", "\"canonical\":false"]
    {
        assert!(j.contains(want), "{want} in {j}");
    }
    let before = journal_bytes(&d);
    assert_eq!(store::init(&d, &b).unwrap_err().kind(), "exists");
    assert_eq!(journal_bytes(&d), before);
    let empty = tmp.sub("empty");
    fs::create_dir(&empty).unwrap();
    assert_eq!(store::init(&empty, &b).unwrap_err().kind(), "exists");
    assert_eq!(store::read_status(&empty, &b).unwrap_err().kind(), "noStore");
    assert_eq!(store::read_status(&tmp.sub("missing"), &b).unwrap_err().kind(), "noStore");
}

#[test]
fn p1_append_batches_across_reopen_match_fixture_chain() {
    let tmp = Tmp::new("append");
    let b = binding();
    let c = chain();
    let d = tmp.sub("s");
    store::init(&d, &b).unwrap();
    {
        let mut w = Writer::open(&d, &b).unwrap();
        let r = w.append(&range(&c, 1, 7)).unwrap();
        assert_eq!(r.appended, (1..=7).collect::<Vec<u64>>());
    }
    {
        let mut w = Writer::open(&d, &b).unwrap();
        assert_eq!(w.height(), 7);
        let r = w.append(&range(&c, 8, 20)).unwrap();
        assert_eq!((r.appended.len(), r.height), (13, 20));
    }
    let s = store::read_status(&d, &b).unwrap();
    let node = common::node();
    assert_eq!((s.height, s.tail), (20, Tail::Clean));
    assert_eq!(Some(s.tip_hash), node.chain.block_hash(20));
    let bytes = journal_bytes(&d);
    let sc = scan_of(&bytes).unwrap();
    for (i, r) in sc.records.iter().enumerate() {
        assert_eq!(Some(r.block_hash), node.chain.block_hash(i as u64 + 1), "record {}", i + 1);
        let at = r.offset as usize + store::REC_HEADER_AT;
        assert_eq!(&bytes[at..at + r.len as usize], &c[i][..], "exact header bytes {}", i + 1);
    }
    // Reopen again: identical bytes and status.
    let s2 = store::read_status(&d, &b).unwrap();
    assert_eq!(s2.journal_sha256, s.journal_sha256);
    assert_eq!(Writer::open(&d, &b).unwrap().height(), 20);
}

#[test]
fn p2_idempotent_tip_and_linear_store_rules() {
    let tmp = Tmp::new("linear");
    let b = binding();
    let c = chain();
    let d = built_store(&tmp, "s", 20);
    let sha = store::read_status(&d, &b).unwrap().journal_sha256;
    {
        let mut w = Writer::open(&d, &b).unwrap();
        let r = w.append(&[c[19].clone()]).unwrap();
        assert_eq!((r.appended.len(), r.idempotent, r.height), (0, 1, 20));
        match w.append(&[c[18].clone()]).unwrap_err() {
            StoreError::NotLinear { height: 19, tip: 20, .. } => {}
            e => panic!("{e:?}"),
        }
        assert_eq!(w.append(&[c[4].clone()]).unwrap_err().kind(), "notLinear");
        // A sibling of the tip (same height, different valid-looking bytes) is not linear here.
        let mut sib = header::decode(&c[19]).unwrap();
        sib.nonce ^= 1;
        assert_eq!(w.append(&[sib.encode()]).unwrap_err().kind(), "notLinear");
    }
    assert_eq!(store::read_status(&d, &b).unwrap().journal_sha256, sha);
    // Gap on a short store; duplicate inside one batch.
    let d1 = built_store(&tmp, "one", 1);
    let mut w = Writer::open(&d1, &b).unwrap();
    assert_eq!(w.append(&[c[2].clone()]).unwrap_err().kind(), "notLinear");
    let r = w.append(&[c[1].clone(), c[1].clone(), c[2].clone()]).unwrap();
    assert_eq!((r.appended.clone(), r.idempotent), (vec![2, 3], 1));
}

fn mutated(raw: &[u8], f: &dyn Fn(&mut Header)) -> Vec<u8> {
    let mut h = header::decode(raw).unwrap();
    f(&mut h);
    h.encode()
}

#[test]
fn p2_invalid_append_leaves_state_unchanged() {
    let tmp = Tmp::new("invalid");
    let b = binding();
    let c = chain();
    let d = built_store(&tmp, "s", 7);
    let sha = store::read_status(&d, &b).unwrap().journal_sha256;
    let h8 = header::decode(&c[7]).unwrap();
    let tid = hashes::sha256(&h8.ut_raw);
    let pow_fail = (h8.nonce + 1..).find(|k| U256::from_be_slice(&hashes::sha256(&hashes::tid_nonce_preimage(&tid, *k))).unwrap() > h8.target).unwrap();
    let cases: Vec<(Vec<u8>, &str)> = vec![
        (mutated(&c[7], &|x| x.ts += 1), "3"),
        (mutated(&c[7], &|x| x.sig[10] ^= 1), "3"),
        (mutated(&c[7], &|x| x.parent_hash[0] ^= 1), "2"),
        (mutated(&c[7], &|x| x.chain_id += 1), "netChain"),
        (mutated(&c[7], &|x| x.genesis_hash[0] ^= 1), "netGenesis"),
        (mutated(&c[7], &|x| x.protocol_version = 2), "netVersion"),
        (mutated(&c[7], &|x| x.nonce = pow_fail), "7"),
        (mutated(&c[7], &|x| x.winner_sig[64] = 2), "8"),
        (mutated(&c[7], &|x| x.shares = vec![x.nonce]), "9"),
        (mutated(&c[7], &|x| x.shares = (0..257).collect()), "1"),
        (vec![0xc0], "1"),
        (c[7][..c[7].len() - 1].to_vec(), "1"),
    ];
    {
        let mut w = Writer::open(&d, &b).unwrap();
        for (raw, rule) in &cases {
            match w.append(&[raw.clone()]) {
                Err(StoreError::Rejected { rule: got, .. }) => assert_eq!(&got, rule),
                other => panic!("want rule {rule}, got {other:?}"),
            }
            assert_eq!(w.height(), 7);
        }
        // Whole batch validated before any write (LP2-I01): a valid H8 followed by a genuinely
        // invalid next-height H9 (winner signature v = 2, item 8). The H9 rejection is reported at
        // index 1 / height 9, and H8 is not written either.
        let bad_h9 = mutated(&c[8], &|x| x.winner_sig[64] = 2);
        assert_eq!(header::decode(&bad_h9).unwrap().h, 9);
        match w.append(&[c[7].clone(), bad_h9]) {
            Err(StoreError::Rejected { index: 1, height: Some(9), rule, .. }) => assert_eq!(rule, "8"),
            other => panic!("want Rejected index 1 height 9 rule 8, got {other:?}"),
        }
        assert_eq!(w.height(), 7);
        // A different rule at the batch's second element: H9 whose parent hash does not link to H8.
        let unlinked_h9 = mutated(&c[8], &|x| x.parent_hash[31] ^= 1);
        match w.append(&[c[7].clone(), unlinked_h9]) {
            Err(StoreError::Rejected { index: 1, height: Some(9), rule, .. }) => assert_eq!(rule, "2"),
            other => panic!("want Rejected index 1 height 9 rule 2, got {other:?}"),
        }
        assert_eq!(w.height(), 7);
        // Separately: a conflicting sibling of the batch's own simulated tip is notLinear, not a
        // validity verdict, and also leaves nothing written.
        match w.append(&[c[7].clone(), cases[0].0.clone()]) {
            Err(StoreError::NotLinear { index: 1, height: 8, tip: 8, .. }) => {}
            other => panic!("want NotLinear index 1 height 8 tip 8, got {other:?}"),
        }
        assert_eq!(w.height(), 7);
        // Bounds before decoding/allocation.
        assert_eq!(w.append(&[vec![0u8; store::MAX_HEADER_BYTES + 1]]).unwrap_err().kind(), "bound");
        assert_eq!(w.append(&vec![c[7].clone(); store::MAX_BATCH + 1]).unwrap_err().kind(), "bound");
        assert_eq!(w.height(), 7);
        assert!(!w.is_poisoned());
    }
    let after = store::read_status(&d, &b).unwrap();
    assert_eq!((after.height, after.journal_sha256), (7, sha));
    // A rejected append does not block a later valid one.
    let mut w = Writer::open(&d, &b).unwrap();
    assert_eq!(w.append(&[c[7].clone()]).unwrap().height, 8);
}

#[test]
fn p3_profile_binding_is_exact() {
    let tmp = Tmp::new("binding");
    let d = built_store(&tmp, "s", 3);
    let mut other = fs::read(common::dir().join("profile.json")).unwrap();
    other.extend_from_slice(b"\n");
    let b2 = Binding::from_profile_bytes(&other).unwrap();
    assert_eq!(b2.cfg.genesis_hash, binding().cfg.genesis_hash);
    assert_eq!(store::read_status(&d, &b2).unwrap_err(), StoreError::ProfileMismatch("profileSha256"));
    assert_eq!(Writer::open(&d, &b2).err().map(|e| e.kind()), Some("profileMismatch"));
    // A store created under the other profile bytes is refused under the original ones.
    let d2 = tmp.sub("other");
    store::init(&d2, &b2).unwrap();
    assert_eq!(store::read_status(&d2, &binding()).unwrap_err().kind(), "profileMismatch");
}

#[test]
fn p3_metadata_incomplete_and_corrupt() {
    let tmp = Tmp::new("meta");
    let d = built_store(&tmp, "s", 2);
    let good = journal_bytes(&d);
    for cut in 0..store::META_BYTES {
        match scan_of(&good[..cut]) {
            Err(StoreError::MetadataIncomplete { have }) => assert_eq!(have, cut as u64),
            other => panic!("cut {cut}: {other:?}"),
        }
    }
    for i in 0..store::META_BYTES {
        let mut m = good.clone();
        m[i] ^= 0x01;
        assert_eq!(scan_of(&m).err().map(|e| e.kind()), Some("metadataCorrupt"), "meta byte {i}");
    }
    // Through the file path as well: an empty journal is never an empty successful store.
    let e = tmp.sub("emptyfile");
    raw_store(&e, &[]);
    assert_eq!(kind_of(&store::read_status(&e, &binding())), "metadataIncomplete");
}

fn swap_ranges(b: &[u8], a: (usize, usize), c: (usize, usize)) -> Vec<u8> {
    // a precedes c and they are adjacent.
    let mut v = b[..a.0].to_vec();
    v.extend_from_slice(&b[c.0..c.1]);
    v.extend_from_slice(&b[a.0..a.1]);
    v.extend_from_slice(&b[c.1..]);
    v
}

#[test]
fn p3_record_corruption_reorder_duplicate_and_bounds() {
    let tmp = Tmp::new("records");
    let d = built_store(&tmp, "s", 6);
    let good = journal_bytes(&d);
    let sc = scan_of(&good).unwrap();
    let span = |i: usize| -> (usize, usize) {
        let r = sc.records[i];
        (r.offset as usize, r.offset as usize + store::REC_FIXED + r.len as usize)
    };
    let corrupt_at = |m: &[u8]| -> Option<u64> {
        match scan_of(m) {
            Err(StoreError::Corrupt { seq, .. }) => Some(seq),
            _ => None,
        }
    };
    // Byte flips in every field of record 5 (and the final record 6).
    for rec in [4usize, 5] {
        let (s, e) = span(rec);
        let len = sc.records[rec].len as usize;
        let at = store::REC_HEADER_AT;
        for (name, off) in [
            ("magic", 1),
            ("seq", 11),
            ("len", 15),
            ("len-high", 12),
            ("guard", 17),
            ("prevHash", 30),
            ("blockHash", 60),
            ("header", at + len / 2),
            ("checksum", at + len + 3),
            ("marker", at + len + 33),
        ] {
            let mut m = good.clone();
            m[s + off] ^= 0x40;
            assert_eq!(corrupt_at(&m), Some(rec as u64 + 1), "record {} field {name}", rec + 1);
            assert!(s + off < e);
        }
    }
    // Oversized and zero length declarations.
    for v in [0u32, store::MAX_HEADER_BYTES as u32 + 1, u32::MAX] {
        let (s, _) = span(5);
        let mut m = good.clone();
        m[s + 12..s + 16].copy_from_slice(&v.to_be_bytes());
        assert_eq!(corrupt_at(&m), Some(6), "declared length {v}");
    }
    // Reordered, duplicated and missing records.
    assert_eq!(corrupt_at(&swap_ranges(&good, span(2), span(3))), Some(3));
    let mut dup = good[..span(3).1].to_vec();
    dup.extend_from_slice(&good[span(3).0..span(3).1]);
    dup.extend_from_slice(&good[span(3).1..]);
    assert_eq!(corrupt_at(&dup), Some(5));
    let mut gap = good[..span(3).0].to_vec();
    gap.extend_from_slice(&good[span(3).1..]);
    assert_eq!(corrupt_at(&gap), Some(4));
    // Trailing garbage after a clean journal is not a record.
    let mut junk = good.clone();
    junk.extend_from_slice(b"XYZ");
    assert_eq!(corrupt_at(&junk), Some(7));
    // File-size bound is checked before reading.
    let big = tmp.sub("big");
    fs::create_dir(&big).unwrap();
    {
        let f = File::create(store::journal_path(&big)).unwrap();
        f.set_len(store::MAX_FILE_BYTES + 1).unwrap();
    }
    assert_eq!(kind_of(&store::read_status(&big, &binding())), "bound");
    // Corrupt sources are not recovered and no destination is created.
    let cd = tmp.sub("corrupt");
    let mut m = good.clone();
    m[span(1).0 + 60] ^= 1;
    raw_store(&cd, &m);
    let dest = tmp.sub("corrupt-dest");
    assert_eq!(store::recover(&cd, &dest, &binding()).unwrap_err().kind(), "corrupt");
    assert!(!dest.exists());
    assert_eq!(journal_bytes(&cd), m);
}

#[test]
fn p4_every_truncation_of_the_final_record() {
    let tmp = Tmp::new("torn");
    let d = built_store(&tmp, "s", 2);
    let good = journal_bytes(&d);
    let sc = scan_of(&good).unwrap();
    let last = sc.records[1];
    let start = last.offset as usize;
    let total = store::REC_FIXED + last.len as usize;
    assert_eq!(start + total, good.len());
    for cut in 0..total {
        let m = &good[..start + cut];
        let s = scan_of(m).unwrap_or_else(|e| panic!("cut {cut}: {e:?}"));
        assert_eq!(s.height(), 1, "cut {cut}");
        if cut == 0 {
            assert_eq!(s.tail, Tail::Clean);
        } else {
            assert_eq!(s.tail, Tail::Torn { offset: start as u64, bytes: cut as u64 }, "cut {cut}");
            assert_eq!(s.verified_end, start as u64);
        }
    }
    // Through files: writer refuses, recovery copies the prefix into a NEW store, source untouched.
    for cut in [1usize, 17, 19, 20, store::REC_HEADER_AT + 1, total - 20, total - 1] {
        let src = tmp.sub(&format!("torn-{cut}"));
        raw_store(&src, &good[..start + cut]);
        let before = journal_bytes(&src);
        let b = binding();
        match Writer::open(&src, &b) {
            Err(StoreError::RecoveryRequired { verified_height: 1, verified_bytes, file_bytes }) => {
                assert_eq!((verified_bytes, file_bytes), (start as u64, (start + cut) as u64));
            }
            Err(e) => panic!("{e:?}"),
            Ok(_) => panic!("writer opened a torn store"),
        }
        assert!(store::read_status(&src, &b).unwrap().recovery_required());
        let dest = tmp.sub(&format!("rec-{cut}"));
        let rep = store::recover(&src, &dest, &b).unwrap();
        assert_eq!((rep.height, rep.discarded_tail_bytes), (1, cut as u64));
        assert_eq!(journal_bytes(&src), before);
        assert_eq!(journal_bytes(&dest), good[..start].to_vec());
        let ds = store::read_status(&dest, &b).unwrap();
        assert_eq!((ds.height, ds.tail), (1, Tail::Clean));
        // The recovered store accepts the next candidate; the destination must not pre-exist.
        Writer::open(&dest, &b).unwrap().append(&[chain()[1].clone()]).unwrap();
        assert_eq!(store::recover(&src, &dest, &b).unwrap_err().kind(), "exists");
    }
}

/// LP2-I02: partial prefix guard and partial checksum bytes are compared with the recomputed
/// digest. Every consistent partial digest is a torn tail (covered for every offset above and
/// re-asserted here); every flip of any present digest byte is corruption of the final record.
#[test]
fn p4_partial_guard_and_checksum_bytes_are_verified() {
    let tmp = Tmp::new("partial");
    let d = built_store(&tmp, "s", 2);
    let good = journal_bytes(&d);
    let sc = scan_of(&good).unwrap();
    let last = sc.records[1];
    let start = last.offset as usize;
    let len = last.len as usize;
    let guard_at = store::REC_GUARD_AT;
    let ck_at = store::REC_HEADER_AT + len;
    let classify = |m: &[u8]| -> String {
        match scan_of(m) {
            Ok(s) if s.tail == Tail::Clean => format!("clean:{}", s.height()),
            Ok(s) => format!("torn:{}", s.height()),
            Err(StoreError::Corrupt { seq, reason, .. }) => format!("corrupt:{seq}:{reason}"),
            Err(e) => e.kind().to_string(),
        }
    };
    // Partial guard: 1..=3 of its 4 bytes present.
    for present in 1..4usize {
        let cut = start + guard_at + present;
        assert_eq!(classify(&good[..cut]), "torn:1", "valid guard prefix of {present} bytes");
        for j in 0..present {
            let mut m = good[..cut].to_vec();
            m[start + guard_at + j] ^= 0x01;
            assert_eq!(classify(&m), "corrupt:2:prefix guard (magic/seq/len)", "guard byte {j} of {present} present");
        }
    }
    // Partial checksum: 1..=31 of its 32 bytes present (header complete).
    for present in 1..32usize {
        let cut = start + ck_at + present;
        assert_eq!(classify(&good[..cut]), "torn:1", "valid checksum prefix of {present} bytes");
        for j in 0..present {
            let mut m = good[..cut].to_vec();
            m[start + ck_at + j] ^= 0x80;
            assert_eq!(classify(&m), "corrupt:2:checksum", "checksum byte {j} of {present} present");
        }
    }
    // The two shapes of the reviewer's CLI probes, through files: corrupt (status/recover refuse),
    // never recoveryRequired, and the evidence file is left untouched.
    let b = binding();
    for (name, cut, at) in [("partialGuard", start + guard_at + 3, start + guard_at + 1), ("partialChecksum", start + ck_at + 1, start + ck_at)] {
        let mut m = good[..cut].to_vec();
        m[at] ^= 0x5a;
        let src = tmp.sub(name);
        raw_store(&src, &m);
        assert_eq!(kind_of(&store::read_status(&src, &b)), "corrupt", "{name}");
        assert_eq!(Writer::open(&src, &b).err().map(|e| e.kind()), Some("corrupt"), "{name}");
        let dest = tmp.sub(&format!("{name}-dest"));
        assert_eq!(store::recover(&src, &dest, &b).unwrap_err().kind(), "corrupt", "{name}");
        assert!(!dest.exists());
        assert_eq!(journal_bytes(&src), m);
    }
    // Complete-record checksum corruption (all 32 bytes present) remains corruption too.
    let mut m = good.clone();
    m[start + ck_at + 31] ^= 0x01;
    assert_eq!(classify(&m), "corrupt:2:checksum");
}

#[test]
fn p3_deterministic_corruption_corpus_never_clean() {
    let tmp = Tmp::new("corpus");
    let d = built_store(&tmp, "s", 2);
    let good = journal_bytes(&d);
    let mut rng = common::Rng(0x4c50_3200_0037_0001);
    let mut kinds = std::collections::BTreeMap::new();
    for trial in 0..1500 {
        let mut m = good.clone();
        let flips = 1 + rng.below(3);
        for _ in 0..flips {
            let i = rng.below(m.len() as u64) as usize;
            m[i] ^= 1 + (rng.next() % 255) as u8;
        }
        if m == good {
            continue;
        }
        let r = scan_of(&m);
        if let Ok(s) = &r {
            assert!(s.tail != Tail::Clean, "trial {trial}: corrupted journal scanned clean at height {}", s.height());
        }
        *kinds.entry(r.err().map(|e| e.kind()).unwrap_or("torn")).or_insert(0u32) += 1;
    }
    println!("{{\"corpus\":\"journal-flips\",\"kinds\":{kinds:?}}}");
}

/// Test-only `JournalIo` that fails a chosen write (optionally writing half first) or sync.
struct FailIo {
    f: File,
    writes: usize,
    syncs: usize,
    fail_write: Option<(usize, bool)>,
    fail_sync: Option<usize>,
}

impl JournalIo for FailIo {
    fn write_all(&mut self, b: &[u8]) -> io::Result<()> {
        let n = self.writes;
        self.writes += 1;
        if let Some((k, half)) = self.fail_write {
            if k == n {
                if half {
                    io::Write::write_all(&mut self.f, &b[..b.len() / 2])?;
                }
                return Err(io::Error::new(io::ErrorKind::Other, "injected write failure"));
            }
        }
        io::Write::write_all(&mut self.f, b)
    }
    fn sync_all(&mut self) -> io::Result<()> {
        let n = self.syncs;
        self.syncs += 1;
        if self.fail_sync == Some(n) {
            return Err(io::Error::new(io::ErrorKind::Other, "injected sync failure"));
        }
        self.f.sync_all()
    }
}

fn open_failing(d: &Path, fail_write: Option<(usize, bool)>, fail_sync: Option<usize>) -> Writer {
    Writer::open_with(d, &binding(), move |f| -> Box<dyn JournalIo> { Box::new(FailIo { f, writes: 0, syncs: 0, fail_write, fail_sync }) }).unwrap()
}

#[test]
fn write_and_sync_failures_poison_the_writer() {
    let tmp = Tmp::new("poison");
    let b = binding();
    let c = chain();
    // Half-written second record of a batch: 1 committed, writer poisoned, reopen reports torn.
    let d = built_store(&tmp, "half", 3);
    {
        let mut w = open_failing(&d, Some((1, true)), None);
        match w.append(&range(&c, 4, 6)) {
            Err(StoreError::WriteFailed { committed: 1, .. }) => {}
            other => panic!("{other:?}"),
        }
        assert!(w.is_poisoned());
        assert_eq!(w.height(), 4);
        assert_eq!(w.append(&[c[4].clone()]).unwrap_err(), StoreError::Poisoned);
    }
    let s = store::read_status(&d, &b).unwrap();
    assert_eq!((s.height, s.recovery_required()), (4, true));
    assert_eq!(Writer::open(&d, &b).err().map(|e| e.kind()), Some("recoveryRequired"));
    let rec = store::recover(&d, &tmp.sub("half-rec"), &b).unwrap();
    assert_eq!(rec.height, 4);
    // Failure before any byte: clean reopen at the old height.
    let d2 = built_store(&tmp, "zero", 3);
    {
        let mut w = open_failing(&d2, Some((0, false)), None);
        assert_eq!(w.append(&range(&c, 4, 5)).unwrap_err().kind(), "writeFailed");
        assert_eq!(w.height(), 3);
    }
    let s2 = store::read_status(&d2, &b).unwrap();
    assert_eq!((s2.height, s2.tail), (3, Tail::Clean));
    // Sync failure: not reported committed by this writer; the complete record bytes may still be
    // found and fully validated on reopen (documented contract).
    let d3 = built_store(&tmp, "sync", 3);
    {
        let mut w = open_failing(&d3, None, Some(0));
        match w.append(&range(&c, 4, 5)) {
            Err(StoreError::WriteFailed { committed: 0, .. }) => {}
            other => panic!("{other:?}"),
        }
        assert_eq!(w.height(), 3);
        assert!(w.is_poisoned());
    }
    let s3 = store::read_status(&d3, &b).unwrap();
    assert_eq!((s3.height, s3.tail), (4, Tail::Clean));
}

#[cfg(windows)]
#[test]
fn exclusive_writer_in_process() {
    let tmp = Tmp::new("excl");
    let b = binding();
    let d = built_store(&tmp, "s", 2);
    let w = Writer::open(&d, &b).unwrap();
    assert_eq!(Writer::open(&d, &b).err().map(|e| e.kind()), Some("busy"));
    assert_eq!(kind_of(&store::read_status(&d, &b)), "busy");
    assert_eq!(store::recover(&d, &tmp.sub("dest"), &b).unwrap_err().kind(), "busy");
    assert!(!tmp.sub("dest").exists());
    drop(w);
    assert_eq!(kind_of(&store::read_status(&d, &b)), "clean:2");
    // A live reader (share READ) also keeps writers out while it reads.
    let reader = {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new().read(true).share_mode(1).open(store::journal_path(&d)).unwrap()
    };
    assert_eq!(Writer::open(&d, &b).err().map(|e| e.kind()), Some("busy"));
    drop(reader);
    assert!(Writer::open(&d, &b).is_ok());
}

#[cfg(not(windows))]
#[test]
fn writer_is_explicitly_unsupported_off_windows() {
    let tmp = Tmp::new("unsupported");
    assert_eq!(store::init(&tmp.sub("s"), &binding()).unwrap_err().kind(), "unsupportedPlatform");
}
