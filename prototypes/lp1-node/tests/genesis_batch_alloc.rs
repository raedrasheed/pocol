//! Bounded memory of `genesis_batch::run`, the reader behind `lp1-node genesis-decode`, measured with
//! a counting global allocator on every platform. Runs without libtest so that only this thread
//! allocates. Each case prepares its input first and measures only `run`: the peak of heap bytes in
//! use above the level before the call. Inputs above the tool limit are streamed by a generator and
//! never stored. Set LP3_ALLOC_MAX_VERSION=1 to add the slow full-size gsVersion case (release build
//! recommended).

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{self, BufReader, Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

use lp1_node::genesis_batch::{self, Summary, MAX_LINE_BYTES};
use lp1_node::genesis_spec::{self as gs, MAX_SPEC_BYTES};
use lp1_node::hex;

struct Counting;

static CUR: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn grew(n: usize) {
    let c = CUR.fetch_add(n, SeqCst) + n;
    PEAK.fetch_max(c, SeqCst);
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            grew(l.size());
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l);
        CUR.fetch_sub(l.size(), SeqCst);
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        let q = System.realloc(p, l, new);
        if !q.is_null() {
            if new >= l.size() {
                grew(new - l.size());
            } else {
                CUR.fetch_sub(l.size() - new, SeqCst);
            }
        }
        q
    }
}

#[global_allocator]
static A: Counting = Counting;

/// Buffers `run` reserves once.
const BUFFERS: usize = MAX_LINE_BYTES + MAX_SPEC_BYTES;

/// Keeps the first bytes of each row and counts rows; never grows past its preallocated buffer.
struct Rows {
    head: Vec<u8>,
    rows: usize,
    at_line_start: bool,
    kept: usize,
}

impl Rows {
    fn new() -> Rows {
        Rows { head: Vec::with_capacity(64 * 1024), rows: 0, at_line_start: true, kept: 0 }
    }
}

impl Write for Rows {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for b in buf {
            if self.at_line_start {
                self.rows += 1;
                self.kept = 0;
                self.at_line_start = false;
            }
            if self.kept < 120 && self.head.len() < self.head.capacity() {
                self.head.push(*b);
                self.kept += 1;
            }
            if *b == b'\n' {
                self.at_line_start = true;
                if self.kept == 120 && self.head.len() < self.head.capacity() {
                    self.head.push(b'\n');
                }
            }
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// `count` copies of `line`, or one line of `len` repeated `byte`s, generated on demand.
struct Gen {
    line: Vec<u8>,
    count: u64,
    pos: usize,
}

impl Read for Gen {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let mut n = 0;
        while n < out.len() && self.count > 0 {
            let take = (self.line.len() - self.pos).min(out.len() - n);
            out[n..n + take].copy_from_slice(&self.line[self.pos..self.pos + take]);
            n += take;
            self.pos += take;
            if self.pos == self.line.len() {
                self.pos = 0;
                self.count -= 1;
            }
        }
        Ok(n)
    }
}

struct Huge {
    left: u64,
    byte: u8,
}

impl Read for Huge {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.left == 0 {
            return Ok(0);
        }
        let n = (out.len() as u64).min(self.left) as usize;
        for b in &mut out[..n] {
            *b = self.byte;
        }
        self.left -= n as u64;
        if self.left == 0 {
            out[n - 1] = b'\n';
        }
        Ok(n)
    }
}

fn measure(input: &mut dyn io::BufRead, rows: &mut Rows) -> (Summary, usize) {
    let base = CUR.load(SeqCst);
    PEAK.store(base, SeqCst);
    let s = genesis_batch::run(input, rows).expect("run");
    (s, PEAK.load(SeqCst) - base)
}

fn list_header(n: usize) -> Vec<u8> {
    if n < 56 {
        return vec![0xc0 + n as u8];
    }
    let be = (n as u64).to_be_bytes();
    let first = be.iter().position(|x| *x != 0).unwrap();
    let mut h = vec![0xf7 + (8 - first) as u8];
    h.extend_from_slice(&be[first..]);
    h
}

fn hex_line(b: &[u8]) -> Vec<u8> {
    let mut l = hex::encode(b).into_bytes();
    l.push(b'\n');
    l
}

/// The deepest nesting of lists around `80` that fits in MAX_SPEC_BYTES.
fn max_deep() -> Vec<u8> {
    let mut heads = Vec::new();
    let mut n = 1usize;
    loop {
        let h = list_header(n);
        if n + h.len() > MAX_SPEC_BYTES {
            break;
        }
        n += h.len();
        heads.push(h);
    }
    let mut out = Vec::with_capacity(n);
    for h in heads.iter().rev() {
        out.extend_from_slice(h);
    }
    out.push(0x80);
    out
}

fn gsv1() -> Vec<u8> {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../development/m1/m1-draft-0.21/vectors/v3-gsv1.json");
    let doc: serde_json::Value = serde_json::from_slice(&std::fs::read(p).expect("accepted GSV1 vector")).unwrap();
    let mut out = Vec::new();
    for seg in doc["flatSegments"].as_array().unwrap() {
        match seg {
            serde_json::Value::String(s) => out.extend(hex::decode(s).unwrap()),
            o => out.extend(hex::decode(o["repeat"].as_str().unwrap()).unwrap().repeat(o["count"].as_u64().unwrap() as usize)),
        }
    }
    out
}

fn max_valid() -> Vec<u8> {
    let mut s = gs::decode(&gsv1()).unwrap();
    s.m0_list = (1..=gs::MAX_MEMBERS as u32)
        .map(|i| {
            let mut id = [0x10u8; 20];
            id[16..].copy_from_slice(&i.to_be_bytes());
            gs::Member { id, reward_addr: [0xb1; 20] }
        })
        .collect();
    gs::encode(&s)
}

fn version_line(v: usize) -> Vec<u8> {
    let mut item = Vec::new();
    lp1_node::rlp::encode_bytes(&vec![0xff; v], &mut item);
    let mut b = list_header(item.len());
    b.extend(item);
    hex_line(&b)
}

fn main() {
    let mut report = Vec::new();
    let mut fail = Vec::new();
    let mut case = |name: &str, input: &mut dyn io::BufRead, want_rows: usize, check: &dyn Fn(&Summary, &str) -> bool, extra_bound: usize| {
        let mut rows = Rows::new();
        let (s, peak) = measure(input, &mut rows);
        let head = String::from_utf8_lossy(&rows.head).into_owned();
        let bound = BUFFERS + extra_bound;
        let ok = rows.rows == want_rows && check(&s, &head) && peak <= bound;
        report.push(format!(
            "{{\"case\":\"{name}\",\"ok\":{ok},\"peakHeapBytes\":{peak},\"boundBytes\":{bound},\"perInputAboveBuffers\":{},\"rows\":{}}}",
            peak.saturating_sub(BUFFERS),
            rows.rows
        ));
        if !ok {
            fail.push(format!("{name}: rows {} peak {peak} bound {bound} summary {s:?} head {}", rows.rows, &head[..head.len().min(400)]));
        }
    };

    // 1. A 256 MiB line is refused without being stored, and the next line is still decoded.
    let mut tail = BufReader::new(Huge { left: 256 << 20, byte: b'0' }.chain(&b"c0\n"[..]));
    case(
        "oversizedLine256MiB",
        &mut tail,
        3,
        &|s, h| s.refused == 1 && s.rejected == 1 && h.contains("\"refused\":\"inputTooLarge\",\"lineBytes\":268435455") && h.contains("\"code\":\"gsCount\""),
        64 * 1024,
    );

    // 2. Many lines (about 20 MB): memory does not grow with the stream.
    let g = gsv1();
    let mut many = BufReader::new(Gen { line: hex_line(&g), count: 30_000, pos: 0 });
    case("gsv1Times30000", &mut many, 30_001, &|s, _| s.accepted == 30_000 && s.ok(), 64 * 1024);

    // 3. One byte above the limit (hex of MAX_SPEC_BYTES + 1 bytes) is refused; exactly the limit is decoded.
    let mut over = hex_line(&vec![0x01; MAX_SPEC_BYTES + 1]);
    over.extend(hex_line(&max_valid()));
    case(
        "limitPlusOneThenMaxValid",
        &mut &over[..],
        3,
        &|s, h| s.refused == 1 && s.accepted == 1 && s.reencode_equal && h.contains("inputTooLarge"),
        2 * MAX_SPEC_BYTES,
    );

    // 4. Largest framing / structure inputs.
    let deep = hex_line(&max_deep());
    case("maxDeepNest", &mut &deep[..], 2, &|s, h| s.rejected == 1 && h.contains("\"code\":\"gsStructure\",\"detail\":\"top[0]\""), 3 * MAX_SPEC_BYTES);
    let mut flat_b = list_header(MAX_SPEC_BYTES - 4);
    flat_b.extend(vec![0x01; MAX_SPEC_BYTES - 4]);
    let flat = hex_line(&flat_b);
    case("maxFlatList", &mut &flat[..], 2, &|s, h| s.rejected == 1 && h.contains("\"code\":\"gsStructure\",\"detail\":\"top[2]\""), 64 * 1024);

    // 5. gsVersion detail of a 256 KiB value (and of the full size with LP3_ALLOC_MAX_VERSION=1).
    let v = version_line(256 * 1024);
    case("gsVersion256KiB", &mut &v[..], 2, &|s, h| s.rejected == 1 && h.contains("\"code\":\"gsVersion\""), 10 * 256 * 1024 + 64 * 1024);
    if std::env::var("LP3_ALLOC_MAX_VERSION").ok().as_deref() == Some("1") {
        let v = version_line(MAX_SPEC_BYTES - 8);
        case("gsVersionMax", &mut &v[..], 2, &|s, h| s.rejected == 1 && h.contains("\"code\":\"gsVersion\""), 10 * MAX_SPEC_BYTES);
    }

    for r in &report {
        println!("{r}");
    }
    println!(
        "{{\"test\":\"genesis_batch_alloc\",\"buffersBytes\":{BUFFERS},\"maxSpecBytes\":{MAX_SPEC_BYTES},\"cases\":{},\"failed\":{}}}",
        report.len(),
        fail.len()
    );
    if !fail.is_empty() {
        for f in &fail {
            eprintln!("{f}");
        }
        std::process::exit(1);
    }
}
