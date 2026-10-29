//! RP header window (browser.md:35-56) and header-linking items, ported from
//! m1-draft-0.27/tools/v1_ref_027.py `check_window` / `Window.window_header`.
//!
//! Order per window header: netid, viewFuture, item 2 (height, parent / genesis, H_END), item 3
//! (template signature), item 4 (stamp window, fallback stamp), item 5 (ASERT), viewTargetCeil,
//! item 6 (nonce < n_max), item 7 (powHash <= target), item 8 (winner signature recovers),
//! item 9 (shares). Items 10-17 (body, state, execution, membership) are NOT evaluated.

use std::collections::HashSet;

use crate::asert::asert;
use crate::fixed::{work, U256, U512};
use crate::fixtures::{Cp, Profile};
use crate::hashes::{self, H256};
use crate::header::{self, Header};
use crate::rlp;

pub const VIEW_SLACK_S: u64 = 4;
pub const PHI_VIEW_S: u64 = 10;
pub const KVIEW: u64 = 12;
pub const ITEMS_NOT_EVALUATED: &str = "10-17";

#[derive(Clone, Debug)]
pub struct NetConfig {
    pub chain_id: u64,
    pub genesis_hash: [u8; 32],
    pub fork_schedule: Vec<(u64, u64)>,
    pub cp: Cp,
}

impl NetConfig {
    pub fn from_profile(p: &Profile) -> NetConfig {
        NetConfig { chain_id: p.chain_id, genesis_hash: p.genesis_hash, fork_schedule: p.fork_schedule.clone(), cp: p.cp.clone() }
    }

    /// Last [version, startHeight] with startHeight <= h.
    pub fn active_version(&self, h: u64) -> Option<u64> {
        let mut v = None;
        for (ver, start) in &self.fork_schedule {
            if *start <= h {
                v = Some(*ver);
            }
        }
        v
    }

    /// ceilTarget = min(2^256 - 1, target_g * 2^VIEW_SLACK_S).
    pub fn ceil_target(&self) -> U256 {
        U512::mul_u256_u64(&self.cp.target_g, 1 << VIEW_SLACK_S).clamp_u256()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShaCat {
    TemplateId,
    PowHash,
    ShareHash,
}

#[derive(Default, Debug)]
pub struct Counters {
    pub decodes: u64,
    pub template_ids: u64,
    pub asert: u64,
    pub pow_hash: u64,
    pub share_hash: u64,
    pub ecrecover: u64,
    pub max_asert_bits: u32,
    pub sha_template: u64,
    pub sha_pow: u64,
    pub sha_share: u64,
    pub preimages: HashSet<Vec<u8>>,
    pub slept_ms: u64,
    pub events: Vec<(&'static str, u64)>,
}

impl Counters {
    pub fn sha256(&mut self, cat: ShaCat, data: &[u8]) -> H256 {
        match cat {
            ShaCat::TemplateId => self.sha_template += 1,
            ShaCat::PowHash => self.sha_pow += 1,
            ShaCat::ShareHash => self.sha_share += 1,
        }
        if !self.preimages.contains(data) {
            self.preimages.insert(data.to_vec());
        }
        hashes::sha256(data)
    }

    pub fn sha_total(&self) -> u64 {
        self.sha_template + self.sha_pow + self.sha_share
    }

    /// Same key set and names as the 0.27 transcript counters.
    pub fn to_json(&self) -> String {
        format!(
            "{{\"asert\":{},\"ecrecover\":{},\"headerDecodes\":{},\"maxAsertBits\":{},\"powHash\":{},\"sha256ByCategory\":{{\"powHash\":{},\"shareHash\":{},\"templateId\":{}}},\"sha256Total\":{},\"sha256UniquePreimages\":{},\"shareHash\":{},\"sleptMs\":{},\"templateIds\":{}}}",
            self.asert,
            self.ecrecover,
            self.decodes,
            self.max_asert_bits,
            self.pow_hash,
            self.sha_pow,
            self.sha_share,
            self.sha_template,
            self.sha_total(),
            self.preimages.len(),
            self.share_hash,
            self.slept_ms,
            self.template_ids
        )
    }

    /// (key, value) pairs in the transcript's flattened form, for comparisons.
    pub fn flat(&self) -> Vec<(&'static str, u64)> {
        vec![
            ("asert", self.asert),
            ("ecrecover", self.ecrecover),
            ("headerDecodes", self.decodes),
            ("maxAsertBits", self.max_asert_bits as u64),
            ("powHash", self.pow_hash),
            ("sha256ByCategory.powHash", self.sha_pow),
            ("sha256ByCategory.shareHash", self.sha_share),
            ("sha256ByCategory.templateId", self.sha_template),
            ("sha256Total", self.sha_total()),
            ("sha256UniquePreimages", self.preimages.len() as u64),
            ("shareHash", self.share_hash),
            ("sleptMs", self.slept_ms),
            ("templateIds", self.template_ids),
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleFail {
    pub rule: &'static str,
    pub detail: &'static str,
}

fn rf<T>(rule: &'static str, detail: &'static str) -> Result<T, RuleFail> {
    Err(RuleFail { rule, detail })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Parent {
    Genesis,
    Idx(usize),
}

/// One snapshot's checking state. The TemplateID cache is per snapshot (per header index).
pub struct Window<'c> {
    cfg: &'c NetConfig,
    cache: bool,
    tids: Vec<Option<H256>>,
    pub ctr: Counters,
}

impl<'c> Window<'c> {
    pub fn new(cfg: &'c NetConfig, cache: bool) -> Window<'c> {
        Window { cfg, cache, tids: Vec::new(), ctr: Counters::default() }
    }

    fn ev(&mut self, name: &'static str, h: u64) {
        self.ctr.events.push((name, h));
    }

    /// TemplateID = SHA256(RLP(UT)) (consensus.md:59).
    pub fn tid(&mut self, hs: &[Header], i: usize) -> H256 {
        if self.tids.len() < hs.len() {
            self.tids.resize(hs.len(), None);
        }
        if self.cache {
            if let Some(t) = self.tids[i] {
                return t;
            }
        }
        self.ctr.template_ids += 1;
        let t = self.ctr.sha256(ShaCat::TemplateId, &hs[i].ut_raw);
        self.tids[i] = Some(t);
        t
    }

    pub fn block_hash(&mut self, hs: &[Header], i: usize) -> H256 {
        let t = self.tid(hs, i);
        hashes::block_hash(&t, hs[i].nonce, &hashes::share_root(&hs[i].share_list_raw))
    }

    pub fn netid(&mut self, x: &Header) -> Result<(), RuleFail> {
        self.ev("netid", x.h);
        if x.chain_id != self.cfg.chain_id {
            return rf("netChain", "chainId");
        }
        if x.genesis_hash != self.cfg.genesis_hash {
            return rf("netGenesis", "genesisHash");
        }
        if Some(x.protocol_version as u64) != self.cfg.active_version(x.h) {
            return rf("netVersion", "protocolVersion");
        }
        Ok(())
    }

    /// `clock = None` is the load-time chain validation, which has no wall clock.
    pub fn future(&mut self, x: &Header, clock: Option<u64>) -> Result<(), RuleFail> {
        if let Some(c) = clock {
            self.ev("viewFuture", x.h);
            if x.ts as u128 > c as u128 + PHI_VIEW_S as u128 {
                return rf("viewFuture", "ts");
            }
        }
        Ok(())
    }

    pub fn link(&mut self, hs: &[Header], xi: usize, p: Parent, clock: Option<u64>, ceil: &U256) -> Result<(), RuleFail> {
        let cp = self.cfg.cp.clone();
        let x = &hs[xi];
        self.netid(x)?;
        self.future(x, clock)?;
        self.ev("item2", x.h);
        let (p_h, p_ts) = match p {
            Parent::Genesis => (0u64, cp.g_ts),
            Parent::Idx(j) => (hs[j].h, hs[j].ts),
        };
        if p_h.checked_add(1) != Some(x.h) {
            return rf("2", "height");
        }
        match p {
            Parent::Genesis => {
                if x.parent_hash != self.cfg.genesis_hash {
                    return rf("viewGenesis", "parentHash");
                }
            }
            Parent::Idx(j) => {
                if x.parent_hash != self.block_hash(hs, j) {
                    return rf("2", "parentHash");
                }
            }
        }
        if x.h > cp.h_end {
            return rf("2", "H_END (10a)");
        }
        self.ev("item3", x.h);
        let signed = x.is_signed();
        if signed {
            if x.proposer.is_empty() || x.sig.is_empty() || x.a != 0 {
                return rf("3", "form");
            }
            let t = self.tid(hs, xi);
            self.ctr.ecrecover += 1;
            match hashes::recover_address(&hashes::sig_msg(&t), &x.sig) {
                Ok(a) if a[..] == x.proposer[..] => {}
                _ => return rf("3", "ecrecover"),
            }
        }
        self.ev("item4", x.h);
        let a = x.a as u128;
        let lo = p_ts as u128 + a * cp.d_att as u128 + 1;
        let hi = p_ts as u128 + (a + 1) * cp.d_att as u128;
        let ts = x.ts as u128;
        if hi >= 1u128 << 63 || ts < lo || ts > hi {
            return rf("4", "window");
        }
        if !signed {
            let want = if x.a == 0 { lo + cp.d_fb_wait as u128 } else { lo };
            if ts != want {
                return rf("4", "fallback stamp");
            }
        }
        self.ev("asert", x.h);
        self.ctr.asert += 1;
        let out = asert(&cp.target_g, cp.g_ts as i128, cp.t_blk as i128, cp.tau as i128, p_ts as i128, p_h as i128);
        if let Some(o) = out {
            self.ctr.max_asert_bits = self.ctr.max_asert_bits.max(o.max_bits);
        }
        match out {
            Some(o) if o.target == x.target => {}
            _ => return rf("5", "asert"),
        }
        self.ev("viewTargetCeil", x.h);
        if x.target > *ceil {
            return rf("viewTargetCeil", "target");
        }
        let n_max: u128 = if cp.nonce_mode == 1 {
            1u128 << 64
        } else {
            let v = U512::mul_u256_u64(&work(&x.target), cp.c);
            if v.bits() > 64 {
                1u128 << 64
            } else {
                v.0[0] as u128
            }
        };
        self.ev("item6", x.h);
        if x.nonce as u128 >= n_max {
            return rf("6", "nonce");
        }
        self.ev("powHash", x.h);
        self.ctr.pow_hash += 1;
        let t = self.tid(hs, xi);
        let ph = self.ctr.sha256(ShaCat::PowHash, &hashes::tid_nonce_preimage(&t, x.nonce));
        if U256::from_be_slice(&ph).unwrap_or(U256::MAX) > x.target {
            return rf("7", "powHash");
        }
        self.ev("item8", x.h);
        let sr = hashes::share_root(&x.share_list_raw);
        self.ctr.ecrecover += 1;
        if hashes::recover_address(&hashes::win_msg(&t, x.nonce, &sr), &x.winner_sig).is_err() {
            return rf("8", "winnerSig");
        }
        self.ev("item9", x.h);
        let t_share = U512::mul_u256_u64(&x.target, cp.m).clamp_u256();
        let mut prev: Option<u64> = None;
        for n in &x.shares {
            let n = *n;
            if prev.map_or(false, |q| n <= q) || n as u128 >= n_max || n == x.nonce {
                return rf("9", "order/range");
            }
            prev = Some(n);
            self.ctr.share_hash += 1;
            let sh = self.ctr.sha256(ShaCat::ShareHash, &hashes::tid_nonce_preimage(&t, n));
            if U256::from_be_slice(&sh).unwrap_or(U256::MAX) > t_share {
                return rf("9", "T_share");
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub b: u64,
    pub from: u64,
    pub count: u64,
    pub n: u64,
    pub has_ref: bool,
    pub anchored_genesis: bool,
}

pub fn plan(h: u64) -> Plan {
    let b = h.saturating_sub(KVIEW).max(1);
    let from = (b - 1).max(1);
    Plan { b, from, count: h - from + 1, n: h - b + 1, has_ref: b > 1, anchored_genesis: b == 1 }
}

fn count_phrase(n: u64) -> String {
    match n {
        1 => "رأس واحد".to_string(),
        2 => "رأسين".to_string(),
        3..=10 => format!("{n} رؤوس"),
        _ => format!("{n} رأسًا"),
    }
}

pub fn message(n: u64, anchored_genesis: bool, min_work: &U256) -> String {
    let mut m = format!(
        "RP: مرتبط بالعمل والهوية لـ{}{}، بعمل محتسب لا يقل عن {} لكل رأس (work(ceilTarget))، دون تحقق تنفيذ أو عضوية",
        count_phrase(n),
        if anchored_genesis { " حتى genesis" } else { "" },
        min_work.to_dec_string()
    );
    if *min_work == U256::ONE {
        m.push_str("، حد العمل غير فعال لهذه الشبكة");
    }
    m
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Ok { n: u64, anchored_genesis: bool, min_work: U256, message: String },
    Fail { rule: &'static str, at: Option<u64>, detail: &'static str },
}

impl Outcome {
    pub fn to_json(&self) -> String {
        match self {
            Outcome::Ok { n, anchored_genesis, min_work, .. } => {
                format!("{{\"ok\":true,\"n\":{n},\"anchoredGenesis\":{anchored_genesis},\"minWork\":{},\"frame\":true}}", min_work.to_dec_string())
            }
            Outcome::Fail { rule, at, detail } => {
                let at = match at {
                    Some(h) => h.to_string(),
                    None => "null".to_string(),
                };
                format!("{{\"ok\":false,\"rule\":\"{rule}\",\"at\":{at},\"detail\":\"{detail}\",\"frame\":false}}")
            }
        }
    }
}

/// Where the snapshot comes from: scripted fixture replies or a JSON-RPC endpoint.
pub trait ViewSource {
    /// None: the reply was not a canonical quantity (viewIncomplete).
    fn block_number(&mut self) -> Option<u64>;
    /// The raw bytes of the result's RLP list, or an error description (viewIncomplete).
    fn headers(&mut self, from: u64, count: u64) -> Result<Vec<u8>, String>;
}

pub struct Scripted {
    pub height: u64,
    pub headers: Vec<u8>,
    pub calls: Vec<(u64, u64)>,
}

impl ViewSource for Scripted {
    fn block_number(&mut self) -> Option<u64> {
        Some(self.height)
    }
    fn headers(&mut self, from: u64, count: u64) -> Result<Vec<u8>, String> {
        self.calls.push((from, count));
        Ok(self.headers.clone())
    }
}

fn wfail(rule: &'static str, at: Option<u64>, detail: &'static str) -> Outcome {
    Outcome::Fail { rule, at, detail }
}

/// check_window (browser.md:35-56). Never panics on reply content.
pub fn check_window(cfg: &NetConfig, src: &mut dyn ViewSource, clock: u64, cache: bool) -> (Outcome, Counters) {
    let mut w = Window::new(cfg, cache);
    let h = match src.block_number() {
        Some(h) => h,
        None => return (wfail("viewIncomplete", None, "eth_blockNumber"), w.ctr),
    };
    if h == 0 {
        return (wfail("viewNoBlocks", Some(0), "no headers requested"), w.ctr);
    }
    let pl = plan(h);
    let raw = match src.headers(pl.from, pl.count) {
        Ok(r) => r,
        Err(_) => return (wfail("viewIncomplete", None, "pocol_getHeaders"), w.ctr),
    };
    let outer = match rlp::decode(&raw) {
        Ok(o) => o,
        Err(_) => return (wfail("viewIncomplete", None, "reply is not an RLP list"), w.ctr),
    };
    let items = match outer.list() {
        Some(v) if v.len() as u64 == pl.count => v,
        _ => return (wfail("viewIncomplete", None, "item count"), w.ctr),
    };
    let mut hdrs: Vec<Header> = Vec::with_capacity(items.len());
    for (i, it) in items.iter().enumerate() {
        let at = pl.from + i as u64;
        w.ctr.decodes += 1;
        w.ev("decode", at);
        let hd = match header::parse_item(it) {
            Ok(hd) => hd,
            Err(e) => return (wfail("1", Some(at), e.detail), w.ctr),
        };
        if hd.h != at {
            return (wfail("viewIncomplete", Some(at), "height"), w.ctr);
        }
        hdrs.push(hd);
    }
    let ceil = cfg.ceil_target();
    let (mut parent, start) = if pl.has_ref {
        let r = &hdrs[0];
        if let Err(e) = w.netid(r).and_then(|_| w.future(r, Some(clock))) {
            return (wfail(e.rule, Some(r.h), e.detail), w.ctr);
        }
        (Parent::Idx(0), 1)
    } else {
        (Parent::Genesis, 0)
    };
    for xi in start..hdrs.len() {
        if let Err(e) = w.link(&hdrs, xi, parent, Some(clock), &ceil) {
            return (wfail(e.rule, Some(hdrs[xi].h), e.detail), w.ctr);
        }
        parent = Parent::Idx(xi);
    }
    let mw = work(&ceil);
    let msg = message(pl.n, pl.anchored_genesis, &mw);
    (Outcome::Ok { n: pl.n, anchored_genesis: pl.anchored_genesis, min_work: mw, message: msg }, w.ctr)
}

/// Load-time validation of a full chain H1..Hn from the genesis parent (no wall clock).
pub fn validate_chain(cfg: &NetConfig, hdrs: &[Header]) -> Result<Counters, (u64, RuleFail)> {
    let mut w = Window::new(cfg, true);
    let ceil = cfg.ceil_target();
    let mut parent = Parent::Genesis;
    for xi in 0..hdrs.len() {
        w.link(hdrs, xi, parent, None, &ceil).map_err(|e| (hdrs[xi].h, e))?;
        parent = Parent::Idx(xi);
    }
    Ok(w.ctr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_table() {
        assert_eq!(plan(1), Plan { b: 1, from: 1, count: 1, n: 1, has_ref: false, anchored_genesis: true });
        assert_eq!(plan(13), Plan { b: 1, from: 1, count: 13, n: 13, has_ref: false, anchored_genesis: true });
        assert_eq!(plan(14), Plan { b: 2, from: 1, count: 14, n: 13, has_ref: true, anchored_genesis: false });
        assert_eq!(plan(20), Plan { b: 8, from: 7, count: 14, n: 13, has_ref: true, anchored_genesis: false });
        assert_eq!(plan(u64::MAX).count, 14);
    }

    #[test]
    fn messages() {
        let m = message(13, false, &U256::from_u64(4095));
        assert!(m.contains("13 رأسًا") && m.contains("4095") && !m.contains("genesis"));
        assert!(message(3, true, &U256::from_u64(4095)).contains("3 رؤوس حتى genesis"));
        assert!(message(1, true, &U256::ONE).ends_with("حد العمل غير فعال لهذه الشبكة"));
    }
}
