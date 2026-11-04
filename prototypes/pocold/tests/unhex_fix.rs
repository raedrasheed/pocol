//! SECURITY FIX TEST (defensive, in-session, synthetic) for F4: the shared hex decoder
//! `pocold::types::unhex` byte-sliced a &str, so a non-ASCII char at an even byte index cut
//! mid-character and panicked ("byte index is not a char boundary"). The fix rejects non-ASCII up
//! front and returns a clear Err, so the two actual decode paths an attacker can reach — the
//! invitation/boot genesis.spec.hex decode (lib.rs::load_spec_for) and the JSON-RPC hex params
//! (rpc::handle) — reject the input gracefully and the node keeps running.
//!
//! These tests encode the FIXED behavior, so each FAILS on the audited db24d9a `unhex` (it panics,
//! which libtest reports as a failed test) and PASSES once the `is_ascii` guard is compiled in.
//! All inputs are synthetic; nothing here touches a live network.

use pocold::chain::Spec;
use pocold::node::{Config, Node};
use pocold::types::unhex;
use pocol_lp4_execution::devnet::{self, DevnetConfig};

fn tmp(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("pocold-unhexfix-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn devnet() -> devnet::Devnet {
    devnet::build(&DevnetConfig::new(777951, pocold::node::now_s(), &["a", "b", "c"])).expect("devnet")
}
fn spec_of(n: &devnet::Devnet) -> Spec {
    let t = pocol_lp4_execution::admission::measure(&n.spec_bytes, &n.alloc, 1).expect("measure");
    Spec::new(pocol_lp4_execution::admission::admit(&n.request(&pocold::single_host_local(), Some(&t))).expect("admit"))
}

// ---- Test 1 (decoder): multi-byte / non-ASCII input is a clean Err, valid hex is unchanged ----
#[test]
fn unhex_rejects_multibyte_without_panic() {
    // Correct-input behavior preserved exactly.
    assert_eq!(unhex("0x00ff").unwrap(), vec![0x00, 0xff], "0x-prefixed hex decodes");
    assert_eq!(unhex("00ff").unwrap(), vec![0x00, 0xff], "bare hex decodes");
    assert_eq!(unhex("0xDEADbeef").unwrap(), vec![0xde, 0xad, 0xbe, 0xef], "mixed case decodes");
    assert!(unhex("0x").unwrap().is_empty(), "empty body decodes to empty");
    // Invalid ASCII stays a graceful Err (unchanged behavior).
    assert!(unhex("0x0").is_err(), "odd length is a graceful Err");
    assert!(unhex("0xzz").is_err(), "non-hex ASCII is a graceful Err");
    // Multi-byte / non-ASCII at several positions and widths: each cut the old slicer mid-char.
    //  "00\u{20AC}0" = 30 30 E2 82 AC 30 (len 6 even): s[2..4] splits the 3-byte euro sign.
    //  "\u{20AC}0"   = E2 82 AC 30         (len 4 even): s[0..2] splits it at index 0.
    //  "ff00\u{1F4A9}" = 66 66 30 30 F0 9F 92 A9 (len 8 even): s[4..6] splits the 4-byte char.
    //  "\u{00FF}00"  = C3 BF 30 30         (len 4 even): s[0..2] = the 2-byte char (a boundary),
    //                  so from_str_radix already Err'd here even on the original — still Err now.
    for s in ["0x00\u{20AC}0", "0x\u{20AC}0", "0xff00\u{1F4A9}", "0x\u{00FF}00", "\u{20AC}\u{20AC}"] {
        let r = unhex(s);
        assert!(r.is_err(), "non-ASCII input {s:?} must be Err, not a panic (got {r:?})");
    }
    eprintln!("F4 decoder: valid hex decodes unchanged; every multi-byte/non-ASCII input returns Err, no panic.");
}

// ---- Test 2 (invitation/boot path): load_spec_for on a non-ASCII genesis.spec.hex ----
#[test]
fn invitation_import_rejects_nonascii_genesis_spec() {
    let n = devnet();
    let dir = tmp("invite");
    // A well-formed invitation/boot directory (as init-devnet / cutover writes it).
    std::fs::write(dir.join("genesis.spec.hex"), pocold::types::hex(&n.spec_bytes)).unwrap();
    std::fs::write(dir.join("alloc.json"), serde_json::to_string(&devnet::alloc_to_json(&n.alloc)).unwrap()).unwrap();
    std::fs::write(dir.join("devnet.json"), serde_json::json!({"calibration":{"hashesPerSecond":n.hash_rate}}).to_string()).unwrap();
    let local = pocold::single_host_local();

    // Correct input: the real boot decode accepts it (or, on a CPU-starved host, fails only on the
    // R14/R19 timing measurement — never on the hex decode, never a panic).
    let good = pocold::load_spec_for(&dir, &local, None);
    assert!(good.is_ok() || good.as_ref().err().is_some_and(|e| e.contains("timing")),
        "valid genesis.spec.hex must admit or fail only on host timing, got err {:?}", good.as_ref().err());
    eprintln!("invitation path, valid spec: {}", if good.is_ok() { "admitted" } else { "host-timing only (not a decode fault)" });

    // Attacker-supplied invitation: a non-ASCII char at an even index in genesis.spec.hex.
    std::fs::write(dir.join("genesis.spec.hex"), "0x00\u{20AC}0").unwrap();
    let bad_err = pocold::load_spec_for(&dir, &local, None).err();
    assert!(bad_err.is_some(), "non-ASCII genesis.spec.hex must be rejected (FAIL-ON-ORIGINAL: it panicked here)");
    assert!(bad_err.as_deref().is_some_and(|e| e.contains("non-ascii")), "the rejection is the clear hex error, got {bad_err:?}");
    eprintln!("invitation path, non-ASCII spec: rejected with {:?}", bad_err.unwrap());

    // The node/process continues: a later decode after the rejection still returns a Result (no abort).
    std::fs::write(dir.join("genesis.spec.hex"), pocold::types::hex(&n.spec_bytes)).unwrap();
    let again = pocold::load_spec_for(&dir, &local, None);
    assert!(again.is_ok() || again.as_ref().err().is_some_and(|e| e.contains("timing")),
        "after rejecting bad input the boot decoder still handles valid input, got err {:?}", again.as_ref().err());
    eprintln!("invitation path continues after rejection: {}", if again.is_ok() { "admitted" } else { "host-timing only" });
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- Test 3 (RPC path): rpc::handle on a non-ASCII hex param ----
#[test]
fn rpc_rejects_nonascii_hex_param_and_keeps_serving() {
    let n = devnet();
    let dir = tmp("rpc");
    let mut node = Node::open(Config {
        dir: dir.clone(), name: "unhexfix".into(), miner: None, listen: None, peers: vec![],
        rpc: None, stop_at: None, log: false, limits: Default::default(),
    }, spec_of(&n)).unwrap();

    // eth_getBlockByHash hex param carries a non-ASCII char at an even byte index. On the original
    // unhex this panics the RPC handler (a worker-thread DoS); the fix returns a clean JSON-RPC error.
    let bad = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"eth_getBlockByHash\",\"params\":[\"0x00\u{20AC}0\",false]}";
    let resp: serde_json::Value = serde_json::from_str(&pocold::rpc::handle(&mut node, bad.as_bytes())).unwrap();
    assert!(resp.get("error").is_some(), "non-ASCII hash param is a clean JSON-RPC error, got {resp}");
    assert!(resp.get("result").is_none(), "no result for a rejected param");
    eprintln!("RPC path, non-ASCII param: clean error {}", resp["error"]);

    // The node keeps serving: a well-formed request right after still answers with a result.
    let ok = "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"eth_blockNumber\",\"params\":[]}";
    let resp2: serde_json::Value = serde_json::from_str(&pocold::rpc::handle(&mut node, ok.as_bytes())).unwrap();
    assert!(resp2.get("result").is_some(), "node still answers a valid RPC after rejecting bad input, got {resp2}");
    eprintln!("RPC path continues after rejection: eth_blockNumber -> {}", resp2["result"]);
    let _ = std::fs::remove_dir_all(&dir);
}
