//! LP2 P5/P6 with actual processes (Windows; the exclusive writer is Windows-only).
//!
//! The helper is this same test executable re-run with `--exact helper_process_entry --ignored`
//! and LP2_HELPER_* environment variables. Without those variables the helper does nothing, so a
//! plain `cargo test -- --ignored` is harmless. The crash wrapper exists only in this test file;
//! the node binary has no fault options.

#![cfg(windows)]

mod common;

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use lp1_node::store::{self, Binding, JournalIo, StoreError, Tail, Writer};

const READY: &str = "LP2-HELPER-READY";

fn tmp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let p = std::env::temp_dir().join(format!("lp2-proc-{tag}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&p).unwrap();
    p
}

fn binding() -> Binding {
    Binding::from_fixtures_dir(&common::dir()).unwrap()
}

fn chain() -> Vec<Vec<u8>> {
    lp1_node::fixtures::load_chain_bytes(&common::json("chain.json")).unwrap()
}

fn node_bin() -> &'static str {
    env!("CARGO_BIN_EXE_lp1-node")
}

/// Runs the real node binary; returns (exit code, stdout).
fn cli(args: &[&str]) -> (i32, String) {
    let out = Command::new(node_bin()).args(args).arg("--fixtures").arg(common::dir()).output().unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned())
}

fn spawn_helper(mode: &str, store_dir: &Path) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .args(&["helper_process_entry", "--exact", "--ignored", "--nocapture", "--test-threads", "1"])
        .env("LP2_HELPER_MODE", mode)
        .env("LP2_HELPER_STORE", store_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

/// Reads helper stdout until the READY line (or EOF).
fn wait_ready(child: &mut Child) -> bool {
    let out = child.stdout.take().unwrap();
    for line in BufReader::new(out).lines() {
        match line {
            Ok(l) if l.contains(READY) => return true,
            Ok(_) => continue,
            Err(_) => return false,
        }
    }
    false
}

fn wait_exit(child: &mut Child, limit: Duration) -> Option<i32> {
    let start = Instant::now();
    while start.elapsed() < limit {
        if let Ok(Some(st)) = child.try_wait() {
            return Some(st.code().unwrap_or(-1));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = child.kill();
    let _ = child.wait();
    None
}

/// Crash wrapper used only by the helper: the second record write writes half its bytes and the
/// process aborts (no unwinding, no destructors), like a power cut after a partial write.
struct AbortIo {
    f: File,
    writes: usize,
}

impl JournalIo for AbortIo {
    fn write_all(&mut self, b: &[u8]) -> io::Result<()> {
        self.writes += 1;
        if self.writes == 2 {
            io::Write::write_all(&mut self.f, &b[..b.len() / 2])?;
            let _ = self.f.sync_all();
            std::process::abort();
        }
        io::Write::write_all(&mut self.f, b)
    }
    fn sync_all(&mut self) -> io::Result<()> {
        self.f.sync_all()
    }
}

#[test]
#[ignore]
fn helper_process_entry() {
    let mode = match std::env::var("LP2_HELPER_MODE") {
        Ok(m) => m,
        Err(_) => return,
    };
    let dir = PathBuf::from(std::env::var("LP2_HELPER_STORE").unwrap());
    let b = binding();
    match mode.as_str() {
        "hold" => {
            let w = Writer::open(&dir, &b).unwrap();
            println!("{READY} height={}", w.height());
            std::thread::sleep(Duration::from_secs(120));
            drop(w);
        }
        "crash" => {
            let mut w = Writer::open_with(&dir, &b, |f| -> Box<dyn JournalIo> { Box::new(AbortIo { f, writes: 0 }) }).unwrap();
            println!("{READY} height={}", w.height());
            let _ = w.append(&chain()[5..10].to_vec());
            println!("unreachable: append returned");
        }
        _ => {}
    }
}

#[test]
fn concurrent_writer_is_refused_and_killed_writer_releases() {
    let base = tmp_dir("hold");
    let d = base.join("s");
    let b = binding();
    store::init(&d, &b).unwrap();
    Writer::open(&d, &b).unwrap().append(&chain()[0..5].to_vec()).unwrap();
    let before = store::read_status(&d, &b).unwrap();
    let mut child = spawn_helper("hold", &d);
    assert!(wait_ready(&mut child), "helper did not become ready");
    // A second writer, in this process or via the CLI, cannot modify the open store.
    assert_eq!(Writer::open(&d, &b).err().map(|e| e.kind()), Some("busy"));
    let ds = d.to_string_lossy().into_owned();
    let (code, out) = cli(&["store", "append", "--store", &ds, "--range", "6:6"]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("\"kind\":\"busy\""), "{out}");
    let (code, out) = cli(&["store", "status", "--store", &ds]);
    assert_eq!(code, 2, "{out}");
    // Kill the holder: the OS releases the handle; no lock file to clean up.
    child.kill().unwrap();
    let _ = child.wait();
    let after = store::read_status(&d, &b).unwrap();
    assert_eq!((after.height, after.journal_sha256), (before.height, before.journal_sha256));
    let (code, out) = cli(&["store", "append", "--store", &ds, "--range", "6:20"]);
    assert_eq!(code, 0, "{out}");
    let (code, out) = cli(&["store", "status", "--store", &ds]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("\"candidateHeight\":20") && out.contains("\"consensus\":false"), "{out}");
    let entries: Vec<_> = fs::read_dir(&d).unwrap().collect();
    assert_eq!(entries.len(), 1, "only the journal exists in the store directory");
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn crashed_writer_leaves_committed_prefix_and_torn_tail() {
    let base = tmp_dir("crash");
    let d = base.join("s");
    let b = binding();
    store::init(&d, &b).unwrap();
    Writer::open(&d, &b).unwrap().append(&chain()[0..5].to_vec()).unwrap();
    let mut child = spawn_helper("crash", &d);
    assert!(wait_ready(&mut child), "helper did not become ready");
    let code = wait_exit(&mut child, Duration::from_secs(60));
    assert!(matches!(code, Some(c) if c != 0), "helper should abort, got {code:?}");
    // Record 6 was written and synced before the crash; record 7 is torn. The handle is released.
    let s = store::read_status(&d, &b).unwrap();
    assert_eq!(s.height, 6);
    assert!(matches!(s.tail, Tail::Torn { .. }));
    match Writer::open(&d, &b) {
        Err(StoreError::RecoveryRequired { verified_height: 6, .. }) => {}
        Err(e) => panic!("{e:?}"),
        Ok(_) => panic!("writer opened a torn store"),
    }
    let ds = d.to_string_lossy().into_owned();
    let (code, out) = cli(&["store", "status", "--store", &ds]);
    assert_eq!(code, 3, "{out}");
    assert!(out.contains("\"tail\":\"recoveryRequired\"") && out.contains("\"candidateHeight\":6"), "{out}");
    let src_before = fs::read(store::journal_path(&d)).unwrap();
    let dest = base.join("recovered");
    let dests = dest.to_string_lossy().into_owned();
    let (code, out) = cli(&["store", "recover", "--store", &ds, "--dest", &dests]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(fs::read(store::journal_path(&d)).unwrap(), src_before, "source evidence unchanged");
    let (code, out) = cli(&["store", "recover", "--store", &ds, "--dest", &dests]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("\"kind\":\"exists\""), "{out}");
    let (code, out) = cli(&["store", "append", "--store", &dests, "--range", "7:20"]);
    assert_eq!(code, 0, "{out}");
    let rs = store::read_status(&dest, &b).unwrap();
    assert_eq!((rs.height, rs.tail), (20, Tail::Clean));
    let _ = fs::remove_dir_all(&base);
}
