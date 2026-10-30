//! A0, A2-A6 against the public fixtures (read at run time from <crate>/fixtures).

mod common;

use common::*;
use serde_json::Value;

use lp1_node::chain::FixtureChain;
use lp1_node::fixed::U256;
use lp1_node::hashes::{self, SigFault, N_ORDER};
use lp1_node::header::{self, Header};
use lp1_node::rpc::{Router, RouterSource};
use lp1_node::window::{self, NetConfig, Outcome, Scripted};
use lp1_node::{fixtures, hex, rlp, verify};

const W_CLOCK: u64 = 1_700_000_205;

#[test]
fn a0_provenance_and_no_private_material() {
    let res = fixtures::verify_provenance(&dir()).unwrap();
    assert_eq!(res.len(), 5);
    assert!(res.iter().all(|(_, ok)| *ok), "{res:?}");
    for f in fixtures::FILES {
        let text = String::from_utf8_lossy(&fixtures::read_file(&dir(), f).unwrap()).to_lowercase();
        for marker in ["\"privatekey\"", "\"private_key\"", "\"secret", "mnemonic", "xprv", "\"seedphrase\""] {
            assert!(!text.contains(marker), "{f} contains {marker}");
        }
    }
}

#[test]
fn profile_binds_literal_genesis_and_rejects_mutations() {
    let v = json("profile.json");
    let p = fixtures::load_profile(&v).unwrap();
    assert_eq!(p.chain_id, 777002);
    assert_eq!(p.cp.target_g, U256::pow2(240).unwrap());
    assert_eq!((p.cp.g_ts, p.cp.t_blk, p.cp.tau, p.cp.nonce_mode, p.cp.c), (1_700_000_000, 10, 600, 1, 2));
    assert_eq!((p.cp.d_att, p.cp.d_fb_wait, p.cp.m, p.cp.h_end), (60, 6, 32, 200));
    assert_eq!(p.fork_schedule, vec![(1, 0)]);
    assert_eq!(p.identity.cp.len(), 52);
    assert_eq!(p.genesis_pre.len(), 341);

    let reject = |f: &dyn Fn(&mut Value)| {
        let mut m = v.clone();
        f(&mut m);
        assert!(fixtures::load_profile(&m).is_err(), "mutation accepted: {}", m);
    };
    reject(&|m| m["cp"]["target_g"] = Value::from("2^241"));
    reject(&|m| m["cp"]["target_g"] = Value::from("1766847064778384329583297500742918515827483896875618958121606201292619776.0"));
    reject(&|m| m["cp"]["tau"] = Value::from(601));
    reject(&|m| m["cp"]["H_END"] = Value::from(199));
    reject(&|m| m["cp"]["notAField"] = Value::from(1));
    reject(&|m| m["chain_id"] = Value::from(777003));
    reject(&|m| m["fork_schedule"] = serde_json::json!([[1, 0], [2, 0]]));
    reject(&|m| m["fork_schedule"] = serde_json::json!([]));
    reject(&|m| m["fork_schedule"] = serde_json::json!([[1, 0, 5]]));
    reject(&|m| {
        let s = m["genesis_pre_hex"].as_str().unwrap().to_string();
        let flipped = format!("{}{}", &s[..100], if &s[100..101] == "0" { "1" } else { "0" }) + &s[101..];
        m["genesis_pre_hex"] = Value::from(flipped);
    });
    reject(&|m| {
        let s = m["genesis_hash_hex"].as_str().unwrap().replace('c', "d");
        m["genesis_hash_hex"] = Value::from(s);
    });
}

fn chain_raw() -> Vec<Vec<u8>> {
    fixtures::load_chain_bytes(&json("chain.json")).unwrap()
}

#[test]
fn a2_chain_headers_reencode_exactly() {
    let raw = chain_raw();
    assert_eq!(raw.len(), 20);
    for (i, b) in raw.iter().enumerate() {
        let h = header::decode(b).unwrap();
        assert_eq!(h.h, i as u64 + 1);
        assert_eq!(&h.encode(), b, "typed re-encode H{}", i + 1);
        assert_eq!(&rlp::encode(&rlp::decode(b).unwrap()), b, "generic re-encode H{}", i + 1);
        // Every strict prefix is rejected (truncation), as is one trailing byte.
        for cut in [0usize, 1, 3, b.len() / 2, b.len() - 1] {
            assert!(header::decode(&b[..cut]).is_err(), "prefix {cut} of H{}", i + 1);
        }
        let mut t = b.clone();
        t.push(0);
        assert!(header::decode(&t).is_err());
    }
}

#[test]
fn a6_chain_loads_only_when_fully_valid() {
    let n = node();
    assert_eq!(n.chain.head(), 20);
    let cfg = &n.cfg;
    let raw = chain_raw();
    let mut swapped = raw.clone();
    swapped.swap(2, 3);
    assert!(FixtureChain::load(cfg, swapped).is_err());
    let mut missing = raw.clone();
    missing.remove(0);
    assert!(FixtureChain::load(cfg, missing).is_err());
    let mut bad_sig = raw.clone();
    let mut h5 = header::decode(&bad_sig[4]).unwrap();
    h5.sig[10] ^= 1;
    bad_sig[4] = h5.encode();
    let e = FixtureChain::load(cfg, bad_sig).unwrap_err();
    assert!(e.starts_with("H5: rule 3"), "{e}");
    let mut wrong_net = cfg.clone();
    wrong_net.chain_id += 1;
    assert!(FixtureChain::load(&wrong_net, raw.clone()).unwrap_err().contains("netChain"));
    assert!(FixtureChain::load(cfg, raw).is_ok());
}

#[test]
fn a5_window_cases_match_saved_transcripts() {
    let n = node();
    let cases = json("window-cases.json");
    let reports = verify::run_window_cases(&n.cfg, &cases, true, None).unwrap();
    assert_eq!(reports.len(), 22);
    for r in &reports {
        assert!(r.pass(), "{}", r.to_json());
    }
}

fn scripted(cfg: &NetConfig, height: u64, bytes: Vec<u8>, cache: bool) -> (Outcome, window::Counters) {
    let mut src = Scripted { height, headers: bytes, calls: Vec::new() };
    window::check_window(cfg, &mut src, W_CLOCK, cache)
}

#[test]
fn a5_named_counts_and_order() {
    let n = node();
    let cases = json("window-cases.json");
    let (o, c) = scripted(&n.cfg, 20, case_bytes(&cases, "RW-h20-phase0"), true);
    assert!(matches!(o, Outcome::Ok { n: 13, anchored_genesis: false, .. }), "{o:?}");
    assert_eq!((c.decodes, c.asert, c.template_ids, c.pow_hash, c.ecrecover), (14, 13, 14, 13, 26));
    assert_eq!(c.sha_total(), 27);
    assert_eq!(c.max_asert_bits, 257);
    let order: Vec<&str> = c.events.iter().filter(|(_, h)| *h == 8).map(|(e, _)| *e).collect();
    assert_eq!(order, vec!["decode", "netid", "viewFuture", "item2", "item3", "item4", "asert", "viewTargetCeil", "item6", "powHash", "item8", "item9"]);
    let (o, c) = scripted(&n.cfg, 20, case_bytes(&cases, "SHA-W-phase0"), true);
    assert!(matches!(o, Outcome::Ok { .. }), "{o:?}");
    assert_eq!((c.decodes, c.asert, c.share_hash), (14, 13, 3328));
    assert_eq!(c.sha_total(), 3355);
    // Without the per-snapshot TemplateID cache the same verdict costs more SHA-256 evaluations.
    let (o2, c2) = scripted(&n.cfg, 20, case_bytes(&cases, "SHA-W-phase0"), false);
    assert_eq!(o, o2);
    assert!(c2.sha_total() > 3355 && c2.template_ids > 14);
    // Height 0: no headers requested.
    let (o, c) = scripted(&n.cfg, 0, vec![0xc0], true);
    assert_eq!(o, Outcome::Fail { rule: "viewNoBlocks", at: Some(0), detail: "no headers requested" });
    assert_eq!(c.decodes, 0);
}

#[test]
fn a5_uncached_outcomes_equal_cached() {
    let n = node();
    let cases = json("window-cases.json");
    let a = verify::run_window_cases(&n.cfg, &cases, true, None).unwrap();
    let b = verify::run_window_cases(&n.cfg, &cases, false, None).unwrap();
    for (x, y) in a.iter().zip(b.iter()) {
        assert_eq!(x.outcome, y.outcome, "{}", x.id);
    }
}

struct Snap {
    items: Vec<Vec<u8>>,
}

impl Snap {
    fn rw20() -> Snap {
        let cases = json("window-cases.json");
        let b = case_bytes(&cases, "RW-h20-phase0");
        let o = rlp::decode(&b).unwrap();
        Snap { items: o.list().unwrap().iter().map(|i| i.raw.to_vec()).collect() }
    }
    fn hdr(&self, h: u64) -> Header {
        header::decode(&self.items[(h - 7) as usize]).unwrap()
    }
    fn with(&self, h: u64, f: &dyn Fn(&mut Header)) -> Vec<u8> {
        let mut x = self.hdr(h);
        f(&mut x);
        let mut items = self.items.clone();
        items[(h - 7) as usize] = x.encode();
        outer(&items)
    }
}

fn fail_at(o: &Outcome) -> (&'static str, Option<u64>) {
    match o {
        Outcome::Fail { rule, at, .. } => (*rule, *at),
        Outcome::Ok { .. } => ("ok", None),
    }
}

fn high_s(sig: &mut [u8]) {
    let s = U256::from_be_slice(&sig[32..64]).unwrap();
    let ns = U256::from_be_slice(&N_ORDER).unwrap().checked_sub(&s).unwrap();
    sig[32..64].copy_from_slice(&ns.to_be_bytes());
    sig[64] ^= 1;
}

#[test]
fn a5_mutations_are_rejected() {
    let n = node();
    let cfg = &n.cfg;
    let s = Snap::rw20();
    let run = |b: Vec<u8>| fail_at(&scripted(cfg, 20, b, true).0);
    assert_eq!(run(outer(&s.items)).0, "ok");

    // Signed template fields: any UT change breaks the template signature (item 3).
    assert_eq!(run(s.with(10, &|x| x.ts += 1)), ("3", Some(10)));
    assert_eq!(run(s.with(10, &|x| x.parent_hash[0] ^= 1)), ("2", Some(10)));
    assert_eq!(run(s.with(10, &|x| high_s(&mut x.sig))), ("3", Some(10)));
    assert_eq!(run(s.with(10, &|x| x.sig[64] = 27)), ("3", Some(10)));
    assert_eq!(run(s.with(10, &|x| x.sig.clear())), ("3", Some(10))); // proposer without sig
                                                                      // Winner signature: high-S and bad v are item 8.
    assert_eq!(run(s.with(10, &|x| high_s(&mut x.winner_sig))), ("8", Some(10)));
    assert_eq!(run(s.with(10, &|x| x.winner_sig[64] = 2)), ("8", Some(10)));
    // Nonce: pick one whose powHash exceeds the target (item 7, before item 8).
    let h10 = s.hdr(10);
    let tid = hashes::sha256(&h10.ut_raw);
    let pow_fail = (h10.nonce + 1..).find(|k| U256::from_be_slice(&hashes::sha256(&hashes::tid_nonce_preimage(&tid, *k))).unwrap() > h10.target).unwrap();
    assert_eq!(run(s.with(10, &|x| x.nonce = pow_fail)), ("7", Some(10)));
    // Shares: order, equality with nonce, T_share, and the 256 bound.
    let t_share = lp1_node::fixed::U512::mul_u256_u64(&h10.target, 32).clamp_u256();
    let bad_share =
        (0u64..).find(|k| *k != h10.nonce && U256::from_be_slice(&hashes::sha256(&hashes::tid_nonce_preimage(&tid, *k))).unwrap() > t_share).unwrap();
    assert_eq!(run(s.with(10, &|x| x.shares = vec![bad_share])), ("9", Some(10)));
    assert_eq!(run(s.with(10, &|x| x.shares = vec![h10.nonce])), ("9", Some(10)));
    assert_eq!(run(s.with(10, &|x| x.shares = vec![5, 5])), ("9", Some(10)));
    assert_eq!(run(s.with(10, &|x| x.shares = (0..257).collect())), ("1", Some(10)));
    // Unsigned variants exercise item 4 fallback stamp and item 5 (ASERT) directly.
    let p_ts = s.hdr(9).ts;
    let unsigned = |x: &mut Header| {
        x.proposer.clear();
        x.sig.clear();
    };
    assert_eq!(run(s.with(10, &|x| unsigned(x))), ("4", Some(10)));
    assert_eq!(
        run(s.with(10, &|x| {
            unsigned(x);
            x.ts = p_ts + 1 + 6;
            x.target = U256::from_be_slice(&{
                let mut b = x.target.to_be_bytes();
                b[31] ^= 1;
                b
            })
            .unwrap();
        })),
        ("5", Some(10))
    );
    assert_eq!(run(s.with(10, &|x| x.a = 1)), ("3", Some(10))); // signed with a != 0
                                                                // Structure: missing reference, swapped heights.
    assert_eq!(run(outer(&s.items[1..])), ("viewIncomplete", None));
    let mut sw = s.items.clone();
    sw.swap(2, 3);
    assert_eq!(run(outer(&sw)), ("viewIncomplete", Some(9)));
    // Configuration mutations: H_END, fork schedule, target_g, nonceMode 0.
    let mut c2 = cfg.clone();
    c2.cp.h_end = 15;
    assert_eq!(fail_at(&scripted(&c2, 20, outer(&s.items), true).0), ("2", Some(16)));
    let mut c3 = cfg.clone();
    c3.fork_schedule = vec![(1, 0), (2, 15)];
    assert_eq!(fail_at(&scripted(&c3, 20, outer(&s.items), true).0), ("netVersion", Some(15)));
    let mut c4 = cfg.clone();
    c4.cp.target_g = U256::pow2(239).unwrap();
    assert_eq!(fail_at(&scripted(&c4, 20, outer(&s.items), true).0).0, "5");
    let mut c5 = cfg.clone();
    c5.cp.nonce_mode = 0;
    c5.cp.c = 1;
    let r5 = fail_at(&scripted(&c5, 20, outer(&s.items), true).0);
    assert!(r5.0 == "6" || r5.0 == "ok", "{r5:?}");
    let mut c6 = cfg.clone();
    c6.genesis_hash[0] ^= 1;
    assert_eq!(fail_at(&scripted(&c6, 20, outer(&s.items), true).0), ("netGenesis", Some(7)));
}

#[test]
fn a3_signatures_and_hash_oracle() {
    let n = node();
    let raw = chain_raw();
    let h1 = header::decode(&raw[0]).unwrap();
    let tid = hashes::sha256(&h1.ut_raw);
    let msg = hashes::sig_msg(&tid);
    assert_eq!(hashes::recover_address(&msg, &h1.sig).unwrap(), n.profile.signer);
    assert_eq!(h1.proposer, n.profile.signer.to_vec());
    let mut hs = h1.sig.clone();
    high_s(&mut hs);
    assert_eq!(hashes::recover_address(&msg, &hs), Err(SigFault::HighS));
    let mut bv = h1.sig.clone();
    bv[64] = 27;
    assert_eq!(hashes::recover_address(&msg, &bv), Err(SigFault::BadV));
    let mut fv = h1.sig.clone();
    fv[64] ^= 1;
    assert_ne!(hashes::recover_address(&msg, &fv).ok(), Some(n.profile.signer));
    let mut other = msg;
    other[0] ^= 1;
    assert_ne!(hashes::recover_address(&other, &h1.sig).ok(), Some(n.profile.signer));
    assert_eq!(hashes::recover_address(&msg, &h1.sig[..64]), Err(SigFault::Length));
    let sr = hashes::share_root(&h1.share_list_raw);
    assert!(hashes::recover_address(&hashes::win_msg(&tid, h1.nonce, &sr), &h1.winner_sig).is_ok());

    let rep = verify::run_hash_checks(&json("hash-oracle.json"), &json("window-cases.json")).unwrap();
    assert!(rep.pass(), "{}", rep.to_json());
    assert_eq!(rep.entries, 3677);
    assert_eq!(rep.derived.get("v1.shareHash"), Some(&(3587, 3587)));
}

#[test]
fn a4_asert_oracle_rows() {
    let oracle = json("asert-oracle.json");
    let mut sink = Vec::new();
    let (rows, matched) = verify::run_asert_rows(&oracle, &mut sink).unwrap();
    assert_eq!((rows, matched), (1027, 1027));
    let text = String::from_utf8(sink).unwrap();
    assert!(text.lines().last().unwrap().contains("\"maxShiftBits\":511"));
}

fn result_of(resp: &str) -> String {
    let v: Value = serde_json::from_str(resp).unwrap();
    v["result"].as_str().unwrap_or_else(|| panic!("{resp}")).to_string()
}

#[test]
fn a6_rpc_over_validated_chain() {
    let n = node();
    let r = Router::new(n.profile.chain_id, &n.chain);
    let before = n.chain.state_digest();
    let call = |m: &str, p: &str| r.handle(format!("{{\"jsonrpc\":\"2.0\",\"id\":9,\"method\":\"{m}\",\"params\":{p}}}").as_bytes());
    assert_eq!(result_of(&call("eth_chainId", "[]")), "0xbdb2a");
    assert_eq!(result_of(&call("eth_blockNumber", "[]")), "0x14");
    let raw = chain_raw();
    for (from, count) in [(1u64, 20u64), (7, 14), (20, 1), (1, 1)] {
        let res = result_of(&call("pocol_getHeaders", &format!("[\"0x{from:x}\",\"0x{count:x}\"]")));
        let b = hex::decode(&res[2..]).unwrap();
        let o = rlp::decode(&b).unwrap();
        let got: Vec<&[u8]> = o.list().unwrap().iter().map(|i| i.raw).collect();
        let want: Vec<&[u8]> = raw[(from - 1) as usize..(from - 1 + count) as usize].iter().map(|v| &v[..]).collect();
        assert_eq!(got, want, "from {from} count {count}");
    }
    let err = |p: &str| call("pocol_getHeaders", p);
    assert!(err("[\"0x15\",\"0x1\"]").contains("fromAboveHead"));
    assert!(err("[\"0x14\",\"0x2\"]").contains("beyondHead"));
    assert!(err("[\"0x1\",\"0x200\"]").contains("beyondHead"));
    assert!(err("[\"0x1\",\"0x201\"]").contains("countRange"));
    assert!(err("[\"0xffffffffffffffff\",\"0x200\"]").contains("fromAboveHead"));
    assert!(err("[\"0x14\",\"0xffffffffffffffff\"]").contains("countRange"));
    assert!(err("[\"0x0\",\"0x1\"]").contains("fromZero"));
    assert!(err("[\"0x1\",\"0x0\"]").contains("countRange"));
    assert!(err("[\"0x10000000000000000\",\"0x1\"]").contains("\"params[0]\""));
    for m in [
        "eth_sendRawTransaction",
        "eth_sendTransaction",
        "eth_accounts",
        "eth_requestAccounts",
        "eth_sign",
        "personal_sign",
        "eth_signTypedData_v4",
        "wallet_switchEthereumChain",
        "eth_getBalance",
        "pocol_submitBlock",
    ] {
        assert!(call(m, "[\"0x00\"]").contains("\"code\":-32601"), "{m}");
    }
    assert_eq!(n.chain.state_digest(), before);
    let echoed = r.handle(br#"{"jsonrpc":"2.0","id":4294967295,"method":"eth_blockNumber"}"#);
    assert_eq!(echoed, r#"{"jsonrpc":"2.0","id":4294967295,"result":"0x14"}"#);
}

#[test]
fn a6_window_check_over_own_rpc_matches_transcript() {
    let n = node();
    let r = Router::new(n.profile.chain_id, &n.chain);
    let mut src = RouterSource::new(&r);
    let (o, c) = window::check_window(&n.cfg, &mut src, W_CLOCK, true);
    assert!(matches!(o, Outcome::Ok { n: 13, anchored_genesis: false, .. }), "{o:?}");
    let cases = json("window-cases.json");
    let want = &case(&cases, "RW-h20-phase0")["counters"];
    for (k, v) in c.flat() {
        let mut cur = want;
        for part in k.split('.') {
            cur = &cur[part];
        }
        assert_eq!(cur.as_u64(), Some(v), "{k}");
    }
    let empty = FixtureChain::empty();
    let re = Router::new(n.profile.chain_id, &empty);
    let (o, _) = window::check_window(&n.cfg, &mut RouterSource::new(&re), W_CLOCK, true);
    assert_eq!(fail_at(&o), ("viewNoBlocks", Some(0)));
}
