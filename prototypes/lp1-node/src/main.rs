//! lp1-node command line. Every command reads public fixtures only; nothing signs, sends or
//! connects outward. Exit status 0 means the command's own checks passed.
//!
//! LP2 `store` commands manage a local experimental candidate-header journal; they print exactly
//! one JSON line on stdout. Exit status: 0 success, 1 append rejected or not linear (nothing
//! written), 2 usage / I/O / busy / bound / metadata / mismatch / corrupt error, 3 recovery
//! required. There are no fault-injection options.
//!
//! LP3 `genesis-decode` runs the GenesisSpec v1 decoder over hex lines for differential checks; it
//! streams the file it is given with fixed buffers (limits in `genesis_batch`). Exit status 0: every
//! line decoded or rejected by the decoder and every accepted input re-encoded exactly; 1: some line
//! refused (too large or not hex); 2: I/O or buffer reservation error.

use std::fs::File;
use std::io::{self, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process;

use lp1_node::store::{self, Binding, StoreError, Writer};
use lp1_node::{fixtures, genesis_batch, http, rpc, verify, window};

const USAGE: &str = "lp1-node <command> [options]

commands:
  verify-fixtures   provenance, profile/genesis binding, chain, window cases, ASERT oracle, hash oracle
  profile           print the genesis identity binding of profile.json
  window            run the RP window cases [--case ID] [--no-cache]
  hash              K1-K3, hash-oracle digests and derivations from fixture headers
  asert-batch       differential ASERT rows as JSON lines [--in FILE] [--out FILE]
  genesis-decode    GenesisSpec v1 decode of one hex input per line, JSON lines --in FILE [--out FILE]
                    (streamed; a line above the largest valid GenesisSpec is refused, not decoded)
  serve             read-only JSON-RPC on 127.0.0.1 [--bind 127.0.0.1] [--port N] [--empty-chain]
                    [--max-requests N] [--max-runtime-ms N] [--conn-timeout-ms N]
  store init        --store DIR                      create a NEW candidate journal (never overwrites)
  store append      --store DIR (--range A:B | --hex HEADER_RLP_HEX)
                                                     validate the whole batch, then write record by record
  store status      --store DIR                      read-only validated status (exit 3: recovery required)
  store recover     --store DIR --dest NEW_DIR       copy the verified prefix into a new store

common options:
  --fixtures DIR    fixture directory (default: <crate root>/fixtures)";

struct Args {
    cmd: String,
    opts: Vec<(String, Option<String>)>,
}

const FLAGS: [&str; 2] = ["--no-cache", "--empty-chain"];
const VALUED: [&str; 9] = ["--fixtures", "--case", "--in", "--out", "--bind", "--port", "--max-requests", "--max-runtime-ms", "--conn-timeout-ms"];
const STORE_VALUED: [&str; 5] = ["--fixtures", "--store", "--dest", "--range", "--hex"];

fn parse_opts(it: &mut dyn Iterator<Item = String>, flags: &[&str], valued: &[&str]) -> Result<Vec<(String, Option<String>)>, String> {
    let mut opts = Vec::new();
    while let Some(a) = it.next() {
        if flags.contains(&a.as_str()) {
            opts.push((a, None));
        } else if valued.contains(&a.as_str()) {
            let v = it.next().ok_or_else(|| format!("{a} needs a value"))?;
            opts.push((a, Some(v)));
        } else {
            return Err(format!("unknown option {a}\n{USAGE}"));
        }
    }
    Ok(opts)
}

fn parse_args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().ok_or_else(|| USAGE.to_string())?;
    let opts = parse_opts(&mut it, &FLAGS, &VALUED)?;
    Ok(Args { cmd, opts })
}

impl Args {
    fn get(&self, k: &str) -> Option<&str> {
        self.opts.iter().rev().find(|(n, _)| n == k).and_then(|(_, v)| v.as_deref())
    }
    fn flag(&self, k: &str) -> bool {
        self.opts.iter().any(|(n, _)| n == k)
    }
    fn num(&self, k: &str) -> Result<Option<u64>, String> {
        match self.get(k) {
            None => Ok(None),
            Some(s) => s.parse::<u64>().map(Some).map_err(|_| format!("{k}: not a non-negative integer")),
        }
    }
    fn dir(&self) -> PathBuf {
        match self.get("--fixtures") {
            Some(d) => PathBuf::from(d),
            None => fixtures::default_dir(),
        }
    }
}

fn run() -> Result<bool, String> {
    let a = parse_args()?;
    let dir = a.dir();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    match a.cmd.as_str() {
        "verify-fixtures" => verify::verify_all(&dir, &mut out),
        "profile" => {
            let node = verify::load_node(&dir, true)?;
            writeln!(out, "{}", verify::profile_json(&node.profile)).map_err(|e| e.to_string())?;
            Ok(true)
        }
        "window" => {
            let node = verify::load_node(&dir, true)?;
            let cases = fixtures::read_json(&dir, "window-cases.json")?;
            let cache = !a.flag("--no-cache");
            let reports = verify::run_window_cases(&node.cfg, &cases, cache, a.get("--case"))?;
            if reports.is_empty() {
                return Err("no matching case".into());
            }
            for r in &reports {
                writeln!(out, "{}", r.to_json()).map_err(|e| e.to_string())?;
            }
            let pass = reports.iter().filter(|r| r.pass()).count();
            writeln!(
                out,
                "{{\"check\":\"window\",\"cache\":{cache},\"cases\":{},\"pass\":{pass},\"itemsNotEvaluated\":\"{}\"}}",
                reports.len(),
                window::ITEMS_NOT_EVALUATED
            )
            .map_err(|e| e.to_string())?;
            Ok(pass == reports.len())
        }
        "hash" => {
            let rep = verify::run_hash_checks(&fixtures::read_json(&dir, "hash-oracle.json")?, &fixtures::read_json(&dir, "window-cases.json")?)?;
            writeln!(out, "{}", rep.to_json()).map_err(|e| e.to_string())?;
            Ok(rep.pass())
        }
        "asert-batch" => {
            let oracle = match a.get("--in") {
                Some(p) => {
                    let b = std::fs::read(p).map_err(|e| format!("read {p}: {e}"))?;
                    serde_json::from_slice(&b).map_err(|e| format!("{p}: {e}"))?
                }
                None => fixtures::read_json(&dir, "asert-oracle.json")?,
            };
            let (rows, matched) = match a.get("--out") {
                Some(p) => {
                    let f = File::create(p).map_err(|e| format!("create {p}: {e}"))?;
                    let mut w = BufWriter::new(f);
                    let r = verify::run_asert_rows(&oracle, &mut w)?;
                    w.flush().map_err(|e| e.to_string())?;
                    r
                }
                None => verify::run_asert_rows(&oracle, &mut out)?,
            };
            Ok(rows == matched)
        }
        "genesis-decode" => {
            let p = a.get("--in").ok_or("genesis-decode needs --in FILE")?;
            let mut input = BufReader::new(File::open(p).map_err(|e| format!("open {p}: {e}"))?);
            let summary = match a.get("--out") {
                Some(o) => {
                    let f = File::create(o).map_err(|e| format!("create {o}: {e}"))?;
                    let mut w = BufWriter::new(f);
                    let r = genesis_batch::run(&mut input, &mut w)?;
                    w.flush().map_err(|e| e.to_string())?;
                    r
                }
                None => genesis_batch::run(&mut input, &mut out)?,
            };
            Ok(summary.ok())
        }
        "serve" => serve(&a, &dir, &mut out),
        "help" | "--help" | "-h" => {
            writeln!(out, "{USAGE}").map_err(|e| e.to_string())?;
            Ok(true)
        }
        other => Err(format!("unknown command {other}\n{USAGE}")),
    }
}

fn serve(a: &Args, dir: &std::path::Path, out: &mut dyn Write) -> Result<bool, String> {
    http::check_bind(a.get("--bind").unwrap_or("127.0.0.1"))?;
    let port = match a.num("--port")? {
        None => 0u16,
        Some(p) if p <= u16::MAX as u64 => p as u16,
        Some(_) => return Err("--port out of range".into()),
    };
    let cfg = http::HttpConfig {
        port,
        max_requests: a.num("--max-requests")?,
        max_runtime_ms: a.num("--max-runtime-ms")?,
        conn_timeout_ms: a.num("--conn-timeout-ms")?.unwrap_or(5000),
    };
    // The chain is validated completely before the listener exists.
    let node = verify::load_node(dir, a.flag("--empty-chain"))?;
    let router = rpc::Router::new(node.profile.chain_id, &node.chain);
    let listener = http::bind(&cfg).map_err(|e| format!("bind: {e}"))?;
    let local = listener.local_addr().map_err(|e| e.to_string())?;
    if !local.ip().is_loopback() {
        return Err("listener is not on loopback".into());
    }
    let opt = |v: Option<u64>| v.map(|x| x.to_string()).unwrap_or_else(|| "null".into());
    writeln!(
        out,
        "{{\"event\":\"ready\",\"transport\":\"lp1-loopback-http-serial\",\"addr\":\"127.0.0.1\",\"port\":{},\"head\":{},\"chainId\":\"{}\",\"maxRequests\":{},\"maxRuntimeMs\":{},\"connTimeoutMs\":{},\"m7\":false}}",
        local.port(),
        router.head(),
        rpc::qty(node.profile.chain_id),
        opt(cfg.max_requests),
        opt(cfg.max_runtime_ms),
        cfg.conn_timeout_ms
    )
    .map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())?;
    let stderr = io::stderr();
    let mut log = stderr.lock();
    let (served, why) = http::serve(&router, &listener, &cfg, &mut log).map_err(|e| e.to_string())?;
    writeln!(out, "{{\"event\":\"shutdown\",\"served\":{served},\"reason\":\"{why}\"}}").map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())?;
    Ok(true)
}

// ------------------------------------------------------------------ LP2 store commands

fn store_exit(e: &StoreError) -> i32 {
    match e {
        StoreError::Rejected { .. } | StoreError::NotLinear { .. } => 1,
        StoreError::RecoveryRequired { .. } => 3,
        _ => 2,
    }
}

fn usage_err(s: &str) -> StoreError {
    StoreError::Io(format!("usage: {s}"))
}

/// Builds the append batch from exactly one of --range A:B (fixture chain heights) or --hex.
fn store_batch(a: &Args, fixtures_dir: &Path) -> Result<Vec<Vec<u8>>, StoreError> {
    match (a.get("--range"), a.get("--hex")) {
        (Some(r), None) => {
            let (x, y) = r.split_once(':').ok_or_else(|| usage_err("--range A:B"))?;
            let from: usize = x.parse().map_err(|_| usage_err("--range A:B"))?;
            let to: usize = y.parse().map_err(|_| usage_err("--range A:B"))?;
            if from < 1 || to < from {
                return Err(usage_err("--range needs 1 <= A <= B"));
            }
            if to - from + 1 > store::MAX_BATCH {
                return Err(StoreError::Bound(format!("batch exceeds {}", store::MAX_BATCH)));
            }
            let chain = fixtures::read_json(fixtures_dir, "chain.json").and_then(|v| fixtures::load_chain_bytes(&v)).map_err(StoreError::Io)?;
            if to > chain.len() {
                return Err(usage_err("--range beyond the fixture chain"));
            }
            Ok(chain[from - 1..to].to_vec())
        }
        (None, Some(h)) => {
            let h = h.strip_prefix("0x").unwrap_or(h);
            if h.len() > 2 * store::MAX_HEADER_BYTES {
                return Err(StoreError::Bound(format!("header exceeds {} bytes", store::MAX_HEADER_BYTES)));
            }
            let b = lp1_node::hex::decode(h).ok_or_else(|| usage_err("--hex must be even-length hex"))?;
            Ok(vec![b])
        }
        _ => Err(usage_err("append needs exactly one of --range A:B or --hex HEX")),
    }
}

fn store_cmd(sub: &str, a: &Args) -> Result<String, StoreError> {
    let fixtures_dir = a.dir();
    let dir = PathBuf::from(a.get("--store").ok_or_else(|| usage_err("--store DIR is required"))?);
    let binding = Binding::from_fixtures_dir(&fixtures_dir)?;
    match sub {
        "init" => Ok(store::init(&dir, &binding)?.to_json()),
        "status" => {
            let s = store::read_status(&dir, &binding)?;
            if s.recovery_required() {
                // Report the verified prefix, then signal recovery with exit status 3.
                println!("{}", s.to_json());
                return Err(StoreError::RecoveryRequired { verified_height: s.height, verified_bytes: s.verified_end, file_bytes: s.file_len });
            }
            Ok(s.to_json())
        }
        "append" => {
            let batch = store_batch(a, &fixtures_dir)?;
            let mut w = Writer::open(&dir, &binding)?;
            let rep = w.append(&batch)?;
            Ok(rep.to_json())
        }
        "recover" => {
            let dest = PathBuf::from(a.get("--dest").ok_or_else(|| usage_err("--dest NEW_DIR is required"))?);
            Ok(store::recover(&dir, &dest, &binding)?.to_json())
        }
        other => Err(usage_err(&format!("unknown store command {other}"))),
    }
}

fn store_main(args: Vec<String>) -> i32 {
    let sub = match args.get(0) {
        Some(s) => s.clone(),
        None => {
            println!("{}", usage_err("store init|append|status|recover").to_json());
            return 2;
        }
    };
    let mut it = args.into_iter().skip(1);
    let opts = match parse_opts(&mut it, &[], &STORE_VALUED) {
        Ok(o) => o,
        Err(e) => {
            println!("{}", StoreError::Io(e).to_json());
            return 2;
        }
    };
    let a = Args { cmd: "store".into(), opts };
    match store_cmd(&sub, &a) {
        Ok(line) => {
            println!("{line}");
            0
        }
        Err(e) => {
            if !matches!(e, StoreError::RecoveryRequired { .. }) || sub != "status" {
                println!("{}", e.to_json());
            }
            store_exit(&e)
        }
    }
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.first().map(|s| s.as_str()) == Some("store") {
        let code = store_main(argv.into_iter().skip(1).collect());
        let _ = io::stdout().flush();
        process::exit(code);
    }
    match run() {
        Ok(true) => {}
        Ok(false) => process::exit(1),
        Err(e) => {
            eprintln!("{{\"error\":{}}}", lp1_node::json::quote(&e));
            process::exit(2);
        }
    }
}
