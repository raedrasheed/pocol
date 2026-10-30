//! Deterministic seeded adversarial corpus (A2 / A6): mutated headers, outer lists, genesis
//! preimages, JSON-RPC bodies and ASERT inputs. Properties: no panic anywhere; anything the strict
//! decoders accept re-encodes to the identical bytes; every RPC reply is a well-formed JSON-RPC
//! envelope; the served state never changes.

mod common;

use common::*;

use lp1_node::asert::asert;
use lp1_node::fixed::U256;
use lp1_node::rpc::{self, Router};
use lp1_node::window::{self, Scripted};
use lp1_node::{genesis, header, json, rlp};

const SEED: u64 = 0x4c50_3101_0034_0001;

/// Applies 1..=4 random edits. Edits can cancel out (for example the same bit flipped twice, or a
/// byte overwritten with its own value), so the result may be byte-identical to `base`; callers that
/// assert "changed input" properties must classify such no-op mutants separately.
fn mutate(rng: &mut Rng, base: &[u8]) -> Vec<u8> {
    let mut v = base.to_vec();
    let ops = 1 + rng.below(4);
    for _ in 0..ops {
        let len = v.len() as u64;
        match rng.below(7) {
            0 if len > 0 => {
                let i = rng.below(len) as usize;
                v[i] ^= 1 << rng.below(8);
            }
            1 if len > 0 => {
                let i = rng.below(len) as usize;
                v[i] = rng.next() as u8;
            }
            2 => v.truncate(rng.below(len + 1) as usize),
            3 => {
                let i = rng.below(len + 1) as usize;
                v.insert(i, rng.next() as u8);
            }
            4 if len > 0 => {
                let i = rng.below(len) as usize;
                v.remove(i);
            }
            5 if len > 1 => {
                let a = rng.below(len) as usize;
                let b = (a + 1 + rng.below(16) as usize).min(v.len());
                let piece = v[a..b].to_vec();
                v.extend_from_slice(&piece);
            }
            _ => {
                // Prefix-byte attack: rewrite the first byte to a random list/string prefix.
                if len > 0 {
                    v[0] = 0x80 + (rng.next() as u8 % 0x80);
                }
            }
        }
    }
    v
}

#[test]
fn header_and_rlp_corpus() {
    let mut rng = Rng(SEED);
    let raw = lp1_node::fixtures::load_chain_bytes(&json("chain.json")).unwrap();
    let mut accepted = 0u32;
    for _ in 0..20_000 {
        let base = &raw[rng.below(raw.len() as u64) as usize];
        let m = mutate(&mut rng, base);
        if let Ok(it) = rlp::decode(&m) {
            assert_eq!(rlp::encode(&it), m, "non-canonical bytes accepted");
        }
        if let Ok(h) = header::decode(&m) {
            accepted += 1;
            assert_eq!(h.encode(), m, "header accepted but re-encodes differently");
        }
    }
    // Pure noise of random length.
    for _ in 0..5_000 {
        let n = rng.below(80) as usize;
        let m: Vec<u8> = (0..n).map(|_| rng.next() as u8).collect();
        if let Ok(it) = rlp::decode(&m) {
            assert_eq!(rlp::encode(&it), m);
        }
        let _ = header::decode(&m);
    }
    println!("{{\"corpus\":\"header\",\"seed\":\"{SEED:#x}\",\"acceptedMutants\":{accepted}}}");
}

#[test]
fn window_corpus_never_panics() {
    let mut rng = Rng(SEED ^ 0x77);
    let n = node();
    let cases = json("window-cases.json");
    let bases: Vec<Vec<u8>> = ["RW-h20-phase0", "SHA-S1-phase0", "RW-h3-phase0"].iter().map(|id| case_bytes(&cases, id)).collect();
    let heights = [20u64, 1, 3];
    for _ in 0..200 {
        let k = rng.below(3) as usize;
        let m = mutate(&mut rng, &bases[k]);
        let mut src = Scripted { height: heights[k], headers: m, calls: Vec::new() };
        let (o, _) = window::check_window(&n.cfg, &mut src, 1_700_000_205, true);
        let _ = o.to_json();
    }
}

/// Number of byte-identical (no-op) mutants this seed produces in the 5000-case genesis corpus.
/// Independently replayed by the reviewer (coordination/lp1-review-r2/genesis-noop-independent-v2.json:
/// 2 byte-identical trials, reported there as 2037 and 3053). Pinned so that a change to the
/// generator or the RNG is noticed rather than silently shifting the corpus.
const GENESIS_NOOP_MUTANTS: usize = 2;

#[test]
fn genesis_corpus_never_panics() {
    let mut rng = Rng(SEED ^ 0x9e);
    let p = lp1_node::fixtures::load_profile(&json("profile.json")).unwrap();
    // The unmodified preimage decodes and has the frozen hash: the identity a no-op must reproduce.
    let base_id = genesis::decode_identity(&p.genesis_pre).unwrap();
    assert_eq!(base_id.hash, p.genesis_hash);
    let mut noop_trials: Vec<usize> = Vec::new();
    let mut changed = 0usize;
    let mut changed_decoded = 0usize;
    for trial in 0..5_000 {
        let m = mutate(&mut rng, &p.genesis_pre);
        let res = genesis::decode_identity(&m);
        if m == p.genesis_pre {
            // No-op mutant: same bytes must give exactly the same decoded identity and hash.
            noop_trials.push(trial);
            assert_eq!(res.as_ref().ok(), Some(&base_id), "no-op trial {trial} decoded differently");
            continue;
        }
        changed += 1;
        if let Ok(id) = res {
            changed_decoded += 1;
            // A genuinely different preimage must not carry the frozen genesis hash.
            assert_ne!(id.hash, p.genesis_hash, "changed preimage at trial {trial} produced the frozen genesis hash");
        }
    }
    println!(
        "{{\"corpus\":\"genesis\",\"seed\":\"{:#x}\",\"trials\":5000,\"noopTrials\":{noop_trials:?},\"changed\":{changed},\"changedDecoded\":{changed_decoded}}}",
        SEED ^ 0x9e
    );
    assert_eq!(noop_trials.len() + changed, 5_000);
    assert_eq!(noop_trials.len(), GENESIS_NOOP_MUTANTS, "no-op trials {noop_trials:?}");
}

#[test]
fn rpc_corpus_envelopes_and_state() {
    let mut rng = Rng(SEED ^ 0x5a);
    let n = node();
    let r = Router::new(n.profile.chain_id, &n.chain);
    let before = n.chain.state_digest();
    let seeds = [
        r#"{"jsonrpc":"2.0","id":1,"method":"pocol_getHeaders","params":["0x7","0xe"]}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"eth_blockNumber","params":[]}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"eth_chainId"}"#,
        r#"{"jsonrpc":"2.0","id":4,"method":"eth_sendRawTransaction","params":["0x00"]}"#,
        r#"{"a":{"b":{"c":[1,2,{"d":"é"}]}},"jsonrpc":"2.0","id":5.0,"method":"x"}"#,
    ];
    for i in 0..20_000 {
        let base = seeds[i % seeds.len()].as_bytes();
        let m = mutate(&mut rng, base);
        let resp = r.handle(&m);
        let v = json::parse(resp.as_bytes()).unwrap_or_else(|e| panic!("reply not JSON ({e:?}): {resp}"));
        assert_eq!(v.get("jsonrpc"), Some(&json::JVal::Str("2.0".into())));
        assert!(v.get("result").is_some() != v.get("error").is_some(), "{resp}");
        let _ = json::integral_u32(std::str::from_utf8(&m).unwrap_or("x"));
        let _ = rpc::parse_qty(std::str::from_utf8(&m).unwrap_or("0x"));
    }
    assert_eq!(n.chain.state_digest(), before);
}

#[test]
fn asert_extreme_inputs_never_panic() {
    let mut rng = Rng(SEED ^ 0x3c);
    let edge = [i128::MIN, i128::MIN + 1, -1, 0, 1, i128::MAX - 1, i128::MAX, 1i128 << 97, -(1i128 << 97), u64::MAX as i128];
    for _ in 0..20_000 {
        let pick = |rng: &mut Rng| if rng.below(2) == 0 { edge[rng.below(edge.len() as u64) as usize] } else { (rng.next() as i64) as i128 };
        let tg = U256([rng.next(), rng.next(), rng.next(), rng.next()]);
        let r = asert(&tg, pick(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng), pick(&mut rng));
        if let Some(o) = r {
            assert!(o.target >= U256::ONE);
            assert!(o.shift_bits <= 512);
        }
    }
}
