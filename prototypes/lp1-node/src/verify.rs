//! Fixture-driven checks shared by the CLI and the integration tests. Each check recomputes values
//! with this crate's own implementation and compares them with the saved public fixtures.

use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::path::Path;

use serde_json::Value;

use crate::asert::asert;
use crate::chain::FixtureChain;
use crate::fixed::U256;
use crate::fixtures::{self, as_str, as_u64, Profile};
use crate::hashes::{self, keccak256, sha256};
use crate::header::{self, Header};
use crate::hex;
use crate::rlp;
use crate::window::{self, NetConfig, Outcome, Scripted};

pub struct Node {
    pub profile: Profile,
    pub cfg: NetConfig,
    pub chain: FixtureChain,
}

/// Loads the profile (bound to the literal genesis) and, unless `empty`, the validated chain.
pub fn load_node(dir: &Path, empty: bool) -> Result<Node, String> {
    let profile = fixtures::load_profile(&fixtures::read_json(dir, "profile.json")?)?;
    let cfg = NetConfig::from_profile(&profile);
    let chain = if empty {
        FixtureChain::empty()
    } else {
        let raw = fixtures::load_chain_bytes(&fixtures::read_json(dir, "chain.json")?)?;
        FixtureChain::load(&cfg, raw)?
    };
    Ok(Node { profile, cfg, chain })
}

pub fn profile_json(p: &Profile) -> String {
    let fs: Vec<String> = p.fork_schedule.iter().map(|(v, s)| format!("[{v},{s}]")).collect();
    format!(
        "{{\"check\":\"profile\",\"ok\":true,\"label\":{},\"syntheticNonBootable\":true,\"chainId\":{},\"genesisHash\":\"{}\",\"genesisPreBytes\":{},\"forkSchedule\":[{}],\"cp\":{{\"g_ts\":{},\"target_g\":\"{}\",\"T_blk\":{},\"tau\":{},\"nonceMode\":{},\"c\":{},\"D_att\":{},\"dFbWait\":{},\"m\":{},\"GAS_LIMIT\":{},\"baseFee0\":\"{}\",\"H_END\":{}}},\"m0Entries\":{},\"notChecked\":\"CP bounds beyond width, gsOrder, gsSys, ParamGate, allocation, system code\"}}",
        crate::json::quote(&p.label),
        p.chain_id,
        hex::encode(&p.genesis_hash),
        p.genesis_pre.len(),
        fs.join(","),
        p.cp.g_ts,
        p.cp.target_g.to_dec_string(),
        p.cp.t_blk,
        p.cp.tau,
        p.cp.nonce_mode,
        p.cp.c,
        p.cp.d_att,
        p.cp.d_fb_wait,
        p.cp.m,
        p.cp.gas_limit,
        p.cp.base_fee0.to_dec_string(),
        p.cp.h_end,
        p.identity.m0_entries
    )
}

pub struct CaseReport {
    pub id: String,
    pub outcome: Outcome,
    pub counters_json: String,
    pub mismatches: Vec<String>,
}

impl CaseReport {
    pub fn pass(&self) -> bool {
        self.mismatches.is_empty()
    }

    pub fn to_json(&self) -> String {
        let mm: Vec<String> = self.mismatches.iter().map(|m| crate::json::quote(m)).collect();
        format!(
            "{{\"case\":{},\"pass\":{},\"outcome\":{},\"counters\":{},\"mismatches\":[{}]}}",
            crate::json::quote(&self.id),
            self.pass(),
            self.outcome.to_json(),
            self.counters_json,
            mm.join(",")
        )
    }
}

fn lookup<'a>(v: &'a Value, dotted: &str) -> Option<&'a Value> {
    let mut cur = v;
    for part in dotted.split('.') {
        cur = cur.get(part)?;
    }
    Some(cur)
}

/// Runs every window case (or one by id). Counters are compared only with the TemplateID cache on,
/// which is how the 0.27 transcripts were recorded.
pub fn run_window_cases(cfg: &NetConfig, cases: &Value, cache: bool, only: Option<&str>) -> Result<Vec<CaseReport>, String> {
    let list = cases.get("cases").and_then(|c| c.as_array()).ok_or("window-cases: no cases array")?;
    let mut out = Vec::new();
    for c in list {
        let id = as_str(c.get("id").ok_or("case id")?, "id")?.to_string();
        if let Some(o) = only {
            if o != id {
                continue;
            }
        }
        let height = as_u64(c.get("height").ok_or("height")?, "height")?;
        let clock = as_u64(c.get("clock").ok_or("clock")?, "clock")?;
        let bytes = hex::decode(as_str(c.get("headers_rlp_hex").ok_or("headers_rlp_hex")?, "headers_rlp_hex")?).ok_or("headers_rlp_hex: not hex")?;
        let mut src = Scripted { height, headers: bytes, calls: Vec::new() };
        let (outcome, ctr) = window::check_window(cfg, &mut src, clock, cache);
        let mut mm = Vec::new();
        let expect = c.get("expect").ok_or("expect")?;
        match (&outcome, expect.get("ok").and_then(|v| v.as_bool())) {
            (Outcome::Ok { n, anchored_genesis, min_work, .. }, Some(true)) => {
                if expect.get("n").and_then(|v| v.as_u64()) != Some(*n) {
                    mm.push(format!("n {n}"));
                }
                if expect.get("anchoredGenesis").and_then(|v| v.as_bool()) != Some(*anchored_genesis) {
                    mm.push("anchoredGenesis".into());
                }
                if expect.get("minWork").and_then(|v| v.as_u64()).map(U256::from_u64) != Some(*min_work) {
                    mm.push("minWork".into());
                }
            }
            (Outcome::Fail { rule, at, .. }, Some(false)) => {
                if expect.get("rule").and_then(|v| v.as_str()) != Some(*rule) {
                    mm.push(format!("rule {rule}"));
                }
                let want_at = expect.get("at").ok_or("expect.at")?;
                let at_ok = match at {
                    Some(h) => want_at.as_u64() == Some(*h),
                    None => want_at.is_null(),
                };
                if !at_ok {
                    mm.push(format!("at {at:?}"));
                }
            }
            _ => mm.push(format!("outcome {}", outcome.to_json())),
        }
        if height > 0 {
            let p = window::plan(height);
            if src.calls != vec![(p.from, p.count)] {
                mm.push(format!("getHeaders calls {:?}", src.calls));
            }
        } else if !src.calls.is_empty() {
            mm.push("headers requested at height 0".into());
        }
        if cache {
            let want = c.get("counters").ok_or("counters")?;
            for (k, v) in ctr.flat() {
                if lookup(want, k).and_then(|x| x.as_u64()) != Some(v) {
                    mm.push(format!("counter {k}={v}"));
                }
            }
        }
        out.push(CaseReport { id, outcome, counters_json: ctr.to_json(), mismatches: mm });
    }
    Ok(out)
}

fn dec_i128(v: &Value, ctx: &str) -> Result<i128, String> {
    let s = as_str(v, ctx)?;
    if s.is_empty() || s.len() > 38 || !s.bytes().all(|c| c.is_ascii_digit()) || (s.len() > 1 && s.starts_with('0')) {
        return Err(format!("{ctx}: not a canonical non-negative decimal"));
    }
    s.parse::<i128>().map_err(|_| format!("{ctx}: out of range"))
}

/// Differential ASERT over oracle rows. Writes one JSON line per row; returns (rows, matching rows).
pub fn run_asert_rows(oracle: &Value, out: &mut dyn Write) -> Result<(u64, u64), String> {
    let rows = oracle.get("rows").and_then(|r| r.as_array()).ok_or("asert oracle: no rows")?;
    let mut total = 0u64;
    let mut matched = 0u64;
    let mut max_shift = 0u32;
    for (i, r) in rows.iter().enumerate() {
        total += 1;
        let tg = U256::from_dec_str(as_str(r.get("targetG").ok_or("targetG")?, "targetG")?).ok_or("targetG")?;
        let gts = dec_i128(r.get("gts").ok_or("gts")?, "gts")?;
        let bt = dec_i128(r.get("blockTime").ok_or("blockTime")?, "blockTime")?;
        let tau = dec_i128(r.get("tau").ok_or("tau")?, "tau")?;
        let h = dec_i128(r.get("h").ok_or("h")?, "h")?;
        let ts = dec_i128(r.get("ts").ok_or("ts")?, "ts")?;
        let want_t = U256::from_dec_str(as_str(r.get("target").ok_or("target")?, "target")?).ok_or("target")?;
        let want_early = match r.get("early") {
            Some(Value::Null) => "null".to_string(),
            Some(Value::String(s)) => format!("\"{s}\""),
            _ => return Err("early".into()),
        };
        let want_shift = as_u64(r.get("maxShiftBits").ok_or("maxShiftBits")?, "maxShiftBits")?;
        let line = match asert(&tg, gts, bt, tau, ts, h) {
            Some(o) => {
                max_shift = max_shift.max(o.shift_bits);
                let ok = o.target == want_t && o.early.as_json() == want_early && o.shift_bits as u64 == want_shift;
                if ok {
                    matched += 1;
                }
                format!(
                    "{{\"i\":{i},\"target\":\"{}\",\"early\":{},\"maxShiftBits\":{},\"maxBits\":{},\"match\":{ok}}}",
                    hex::encode(&o.target.to_be_bytes()),
                    o.early.as_json(),
                    o.shift_bits,
                    o.max_bits
                )
            }
            None => format!("{{\"i\":{i},\"target\":null,\"match\":false}}"),
        };
        writeln!(out, "{line}").map_err(|e| e.to_string())?;
    }
    writeln!(out, "{{\"summary\":\"asert-batch\",\"rows\":{total},\"matched\":{matched},\"maxShiftBits\":{max_shift}}}").map_err(|e| e.to_string())?;
    Ok((total, matched))
}

pub struct HashReport {
    pub entries: u64,
    pub digest_ok: u64,
    pub derived: BTreeMap<String, (u64, u64)>,
    pub known_answers_ok: bool,
    pub failures: Vec<String>,
}

impl HashReport {
    pub fn pass(&self) -> bool {
        self.known_answers_ok && self.digest_ok == self.entries && self.derived.values().all(|(m, t)| m == t) && self.failures.is_empty()
    }

    pub fn to_json(&self) -> String {
        let d: Vec<String> = self.derived.iter().map(|(g, (m, t))| format!("{}:{{\"derived\":{m},\"entries\":{t}}}", crate::json::quote(g))).collect();
        let f: Vec<String> = self.failures.iter().take(20).map(|x| crate::json::quote(x)).collect();
        format!(
            "{{\"check\":\"hash\",\"ok\":{},\"knownAnswersK1K3\":{},\"entries\":{},\"digestMatches\":{},\"groups\":{{{}}},\"failures\":[{}]}}",
            self.pass(),
            self.known_answers_ok,
            self.entries,
            self.digest_ok,
            d.join(","),
            f.join(",")
        )
    }
}

fn headers_of_cases(cases: &Value) -> Vec<Header> {
    let mut out = Vec::new();
    let list = match cases.get("cases").and_then(|c| c.as_array()) {
        Some(l) => l,
        None => return out,
    };
    for c in list {
        let b = match c.get("headers_rlp_hex").and_then(|v| v.as_str()).and_then(hex::decode) {
            Some(b) => b,
            None => continue,
        };
        if let Ok(outer) = rlp::decode(&b) {
            if let Some(items) = outer.list() {
                for it in items {
                    if let Ok(h) = header::parse_item(it) {
                        out.push(h);
                    }
                }
            }
        }
    }
    out
}

/// K1-K3, every oracle digest recomputed, and every oracle preimage re-derived from fixture headers
/// with this crate's domain functions (TemplateID, powHash, shareHash, blockHash, sigMsg, winMsg,
/// shareRoot).
pub fn run_hash_checks(oracle: &Value, cases: &Value) -> Result<HashReport, String> {
    let k = [
        ("", "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"),
        ("c0", "1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347"),
        ("80", "56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421"),
    ];
    let known_answers_ok = k.iter().all(|(p, d)| hex::encode(&keccak256(&hex::decode(p).unwrap_or_default())) == *d);
    let mut derived_set: HashSet<(&'static str, Vec<u8>)> = HashSet::new();
    for h in headers_of_cases(cases) {
        let tid = sha256(&h.ut_raw);
        let sr = hashes::share_root(&h.share_list_raw);
        derived_set.insert(("v1.templateId", h.ut_raw.clone()));
        derived_set.insert(("v1.powHash", hashes::tid_nonce_preimage(&tid, h.nonce).to_vec()));
        for n in &h.shares {
            derived_set.insert(("v1.shareHash", hashes::tid_nonce_preimage(&tid, *n).to_vec()));
        }
        derived_set.insert(("v1.blockHash", hashes::block_hash_preimage(&tid, h.nonce, &sr).to_vec()));
        derived_set.insert(("v1.sigMsg", hashes::sig_msg_preimage(&tid)));
        derived_set.insert(("v1.winMsg", hashes::win_msg_preimage(&tid, h.nonce, &sr)));
        derived_set.insert(("v1.shareRoot", h.share_list_raw.clone()));
    }
    let list = oracle.get("list").and_then(|l| l.as_array()).ok_or("hash oracle: no list")?;
    let mut rep = HashReport { entries: 0, digest_ok: 0, derived: BTreeMap::new(), known_answers_ok, failures: Vec::new() };
    for e in list {
        rep.entries += 1;
        let id = e.get("id").and_then(|v| v.as_str()).unwrap_or("?").to_string();
        let group = as_str(e.get("group").ok_or("group")?, "group")?;
        let alg = as_str(e.get("algorithm").ok_or("algorithm")?, "algorithm")?;
        let pre = hex::decode(as_str(e.get("preimageHex").ok_or("preimageHex")?, "preimageHex")?).ok_or("preimageHex")?;
        let want = as_str(e.get("digest").ok_or("digest")?, "digest")?;
        let got = match alg {
            "sha256" => sha256(&pre),
            "keccak256" => keccak256(&pre),
            _ => {
                rep.failures.push(format!("{id}: algorithm {alg}"));
                continue;
            }
        };
        if hex::encode(&got) == want {
            rep.digest_ok += 1;
        } else {
            rep.failures.push(format!("{id}: digest"));
        }
        let slot = rep.derived.entry(group.to_string()).or_insert((0, 0));
        slot.1 += 1;
        let key: Option<&'static str> =
            ["v1.templateId", "v1.powHash", "v1.shareHash", "v1.blockHash", "v1.sigMsg", "v1.winMsg", "v1.shareRoot"].iter().copied().find(|g| *g == group);
        match key {
            Some(g) if derived_set.contains(&(g, pre.clone())) => slot.0 += 1,
            _ => rep.failures.push(format!("{id}: preimage not derived from fixture headers")),
        }
    }
    Ok(rep)
}

/// Provenance + profile + chain + window + ASERT + hash, as one report. Returns overall pass.
pub fn verify_all(dir: &Path, out: &mut dyn Write) -> Result<bool, String> {
    let mut all = true;
    let prov = fixtures::verify_provenance(dir)?;
    let prov_ok = prov.iter().all(|(_, ok)| *ok);
    all &= prov_ok;
    let pl: Vec<String> = prov.iter().map(|(p, ok)| format!("{}:{ok}", crate::json::quote(p))).collect();
    writeln!(out, "{{\"check\":\"provenance\",\"ok\":{prov_ok},\"files\":{{{}}}}}", pl.join(",")).map_err(|e| e.to_string())?;
    let node = load_node(dir, false)?;
    writeln!(out, "{}", profile_json(&node.profile)).map_err(|e| e.to_string())?;
    writeln!(
        out,
        "{{\"check\":\"chain\",\"ok\":true,\"head\":{},\"headBlockHash\":\"{}\"}}",
        node.chain.head(),
        node.chain.block_hash(node.chain.head()).map(|h| hex::encode(&h)).unwrap_or_default()
    )
    .map_err(|e| e.to_string())?;
    let cases = fixtures::read_json(dir, "window-cases.json")?;
    let reports = run_window_cases(&node.cfg, &cases, true, None)?;
    let wpass = reports.iter().filter(|r| r.pass()).count();
    all &= wpass == reports.len();
    for r in &reports {
        writeln!(out, "{}", r.to_json()).map_err(|e| e.to_string())?;
    }
    writeln!(
        out,
        "{{\"check\":\"window\",\"ok\":{},\"cases\":{},\"pass\":{wpass},\"itemsNotEvaluated\":\"{}\"}}",
        wpass == reports.len(),
        reports.len(),
        window::ITEMS_NOT_EVALUATED
    )
    .map_err(|e| e.to_string())?;
    let oracle = fixtures::read_json(dir, "asert-oracle.json")?;
    let mut sink = Vec::new();
    let (rows, matched) = run_asert_rows(&oracle, &mut sink)?;
    let declared = oracle.get("checks").and_then(|v| v.as_u64()).unwrap_or(0);
    let aok = rows == matched && rows == declared;
    all &= aok;
    writeln!(out, "{{\"check\":\"asert\",\"ok\":{aok},\"rows\":{rows},\"matched\":{matched},\"declared\":{declared}}}").map_err(|e| e.to_string())?;
    let hrep = run_hash_checks(&fixtures::read_json(dir, "hash-oracle.json")?, &cases)?;
    all &= hrep.pass();
    writeln!(out, "{}", hrep.to_json()).map_err(|e| e.to_string())?;
    writeln!(out, "{{\"check\":\"summary\",\"ok\":{all}}}").map_err(|e| e.to_string())?;
    Ok(all)
}
