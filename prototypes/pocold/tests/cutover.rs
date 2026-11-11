//! Cutover (EARN-IN-SPEC.md section 7) end to end: an "old" continuous v1 network with transfers, a deployed
//! contract with storage and a 100 PCOL registration; its stopped data directory becomes the genesis of an
//! earn-in network that keeps every user account, refunds the old bonds and funds 20 PCOL genesis bonds.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use pocol_lp4_execution::{apply, devnet::{self, DevnetConfig}, txsign::{Key, Tx}};
use pocold::{chain::{Chain, Spec}, hashes, node::{Config, Limits, Node}, types::*};
use revm::primitives::{keccak256, Address, Bytes, B256, U256};

const PCOL: u128 = 1_000_000_000_000_000_000;

fn local() -> pocol_lp4_execution::admission::NodeLocal { pocold::single_host_local() }

/// The old network's genesis directory, as `pocold init-devnet` writes it.
fn old_network(dir: &Path) -> (DevnetConfig, HashMap<Address, Key>, Vec<(Address, Address)>) {
    let mut cfg = DevnetConfig::new(777917, 1_700_000_000, &["one", "two", "three"]);
    cfg.hash_rate = 16; cfg.c = 8; cfg.d_att = 73; cfg.r_max = 1; cfg.h_end = 0; cfg.h_close = 0;
    let net = devnet::build(&cfg).unwrap();
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("genesis.spec.hex"), hex(&net.spec_bytes)).unwrap();
    std::fs::write(dir.join("alloc.json"), devnet::alloc_to_json(&net.alloc).to_string()).unwrap();
    std::fs::write(dir.join("devnet.json"), serde_json::json!({"forks":[[1,0]],"calibration":{"hashesPerSecond":cfg.hash_rate}}).to_string()).unwrap();
    let mut keys: HashMap<Address, Key> = net.miners.iter().map(|m| (m.id, Key::from_secret(&m.key.secret()).unwrap())).collect();
    for l in ["alice", "bob"] { let k = devnet::account_key(l); keys.insert(k.address(), k); }
    let miners = net.miners.iter().map(|m| (m.id, m.reward)).collect();
    (cfg, keys, miners)
}

fn mine(chain: &Chain, keys: &HashMap<Address, Key>, txs: Vec<Vec<u8>>) -> Block { mine_with(chain, keys, txs, vec![]) }
fn mine_with(chain: &Chain, keys: &HashMap<Address, Key>, txs: Vec<Vec<u8>>, evidence: Vec<Vec<u8>>) -> Block {
    let parent = chain.full_entry(&chain.head).unwrap();
    let ctx = chain.child_ctx(&parent, 0).unwrap();
    let body = Body { txs, evidence };
    let effects = chain.evidence_effects(&parent, &body, parent.height + 1).unwrap();
    for ts in parent.ts + u64::from(chain.spec.cp().T_blk)..=ctx.window.upper() {
        let (mut header, _, _) = chain.build_signed(&parent, &keys[&ctx.proposer], ts, body.clone(), effects.clone()).unwrap();
        let tid = template_id(&header.ut_raw);
        let (mut shares, mut nonce) = (Vec::new(), 0u64);
        while u128::from(nonce) < ctx.assignment.nonce_max() {
            let hash = pow_hash(&tid, nonce);
            if le_target(&hash, &ctx.target) {
                header.nonce = nonce; header.shares = shares; reseal(&mut header);
                let owner = addr(&ctx.assignment.owner(nonce).unwrap());
                header.winner_sig = keys[&owner].sign_hash(&B256::from(hashes::win_msg(&tid, nonce, &share_root(&header))));
                reseal(&mut header);
                return Block { header, body };
            }
            if shares.len() < usize::from(chain.spec.cp().S_max) && le_target(&hash, &ctx.t_share) { shares.push(nonce); }
            nonce += 1;
        }
    }
    panic!("no solution");
}
fn tx(chain: &Chain, key: &Key, to: Option<Address>, value: u128, data: Vec<u8>, gas: u64) -> Vec<u8> {
    let head = chain.full_entry(&chain.head).unwrap();
    let base = chain.child_ctx(&head, 0).unwrap().base_fee;
    let nonce = head.state.accounts.get(&key.address()).map(|a| a.info.nonce).unwrap_or(0);
    let p = U256::from(1_000_000_000u64);
    Tx { chain_id: chain.spec.chain_id(), nonce, max_priority: p, max_fee: base * U256::from(2) + p, gas, to, value: U256::from(value), data: Bytes::from(data) }.sign_1559(key)
}
fn spec(dir: &Path) -> Spec { admitted(pocold::load_spec_for(dir, &local(), None), dir) }
fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pocol-cutover-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d); d
}
/// Every boot and every cutover here is admitted by ParamGate on a timing it measures now, on this host (R14/R19:
/// each calibration block execution median within t_exec, 1 s; in store mode durable commit latency is
/// measured separately from execution). Run beside one another these tests' imports, rebuilds and fsyncs are the load those
/// measurements are taken under: on Windows (runs 42863094336 and 42969344541) a store-mode cutover was refused on
/// [R14, R19] while three of them ran beside it on the same disk. So the tests that boot run one at a time.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn serial() -> std::sync::MutexGuard<'static, ()> { SERIAL.lock().unwrap_or_else(|e| e.into_inner()) }
/// Unwraps a boot or a cutover. A ParamGate refusal panics with the timing of a calibration of `net` run right after
/// it, in memory and through a scratch state store, so a refusal on R14/R19 shows what this host measured.
fn admitted<T>(r: Result<T, String>, net: &Path) -> T {
    r.unwrap_or_else(|e| {
        if !e.starts_with("-32018") { panic!("{e}") }
        let spec_bytes = unhex(std::fs::read_to_string(net.join("genesis.spec.hex")).unwrap().trim()).unwrap();
        let alloc = devnet::alloc_from_json(&serde_json::from_str(&std::fs::read_to_string(net.join("alloc.json")).unwrap()).unwrap()).unwrap();
        let memory = pocol_lp4_execution::admission::measure(&spec_bytes, &alloc, 1).map(|t| t.phase_ms);
        let dir = PathBuf::from(format!("{}-calibration", net.display()));
        let _ = std::fs::remove_dir_all(&dir); std::fs::create_dir_all(&dir).unwrap();
        let store = pocold::state_store::StateStore::open(&dir).map_err(|e| e.0)
            .and_then(|st| pocol_lp4_execution::admission::measure_on_store(&spec_bytes, &alloc, 1, &st).map(|t| t.phase_ms));
        let _ = std::fs::remove_dir_all(&dir);
        panic!("{e}; a calibration run right after it, phase ms in memory {memory:?}, through a state store {store:?}")
    })
}

#[test]
fn a_stopped_old_chain_becomes_an_earn_in_genesis_that_keeps_every_account() { scenario(false) }
/// The same from a store-mode data directory (state.redb trie versions), as `pocol start` nodes run.
#[test]
fn a_store_mode_old_chain_gives_the_same_cutover() { scenario(true) }

fn scenario(store: bool) {
    let _serial = serial();
    let tag = if store { "store" } else { "whole" };
    let limits = || Limits { state_store: store, ..Default::default() };
    let (old_dir, data) = (temp(&format!("old-{tag}")), temp(&format!("data-{tag}")));
    let (_cfg, keys, miners) = old_network(&old_dir);
    let mut chain = Chain::new(spec(&old_dir));
    let node_spec = admitted(pocold::load_spec_for(&old_dir, &local(), store.then_some(data.as_path())), &old_dir);
    let mut node = Node::open(Config { dir: data.clone(), name: "old".into(), miner: None, listen: None, peers: vec![], rpc: None,
        stop_at: None, log: false, limits: limits() }, node_spec).unwrap();
    let (alice, bob) = (devnet::account_key("alice"), devnet::account_key("bob"));
    let carol = Address::with_last_byte(0xca);
    // Contract: stores 42 at slot 0 and has the one-byte runtime 0x00.
    let init = unhex("602a600055600060005360016000f3").unwrap();
    let mut register = keccak256("register(address)")[..4].to_vec(); register.extend_from_slice(bob.address().into_word().as_slice());
    let mut blocks = Vec::new();
    for step in 0..2 {
        let txs = if step == 0 { vec![tx(&chain, &alice, Some(carol), 3 * PCOL, vec![], 21_000)] }
            else { vec![tx(&chain, &alice, None, 0, init.clone(), 300_000), tx(&chain, &bob, Some(carol), PCOL, vec![], 21_000)] };
        let b = mine(&chain, &keys, txs); let (e, _) = chain.validate(&b, None, 0).unwrap(); chain.insert(e); blocks.push(b);
    }
    let reg = tx(&chain, &bob, Some(apply::program_address(0)), 100 * PCOL, register, 1_000_000);
    let b = mine(&chain, &keys, vec![reg]); let (e, _) = chain.validate(&b, None, 0).unwrap(); chain.insert(e); blocks.push(b);
    for _ in 0..6 { let b = mine(&chain, &keys, vec![]); let (e, _) = chain.validate(&b, None, 0).unwrap(); chain.insert(e); blocks.push(b); }
    for b in &blocks { assert!(node.import(b.clone(), None).unwrap()); }
    let s = chain.head().height;
    let old = chain.head().state.clone();
    assert_eq!(apply::miner_record(&old, bob.address()).map(|r| (r.bond_state, r.bond)), Some((1, U256::from(100 * PCOL))), "bob registered on the old chain");
    let contract = old.accounts.iter().find(|(_, a)| a.info.code.as_ref().is_some_and(|c| c.original_byte_slice() == [0u8])).map(|(a, _)| *a).expect("contract deployed");
    drop(node);

    let config = serde_json::json!({"continuous":true,"earnin":true,"chainId":777918,"genesisTime":1_800_000_000u64,"registrationPerBlock":1,
        "miners":miners.iter().map(|(id,reward)|serde_json::json!({"id":id,"reward":reward})).collect::<Vec<_>>(),"accounts":[]}).to_string();
    let out = temp(&format!("new-{tag}"));
    // A height above the old head is refused, and nothing is written.
    assert!(pocold::cutover::run(&old_dir, &data, s + 1, config.as_bytes(), &out, limits(), &local()).unwrap_err().contains("above the old head"));
    assert!(!out.exists());
    admitted(pocold::cutover::run(&old_dir, &data, s, config.as_bytes(), &out, limits(), &local()), &old_dir);

    let alloc = devnet::alloc_from_json(&serde_json::from_str(&std::fs::read_to_string(out.join("alloc.json")).unwrap()).unwrap()).unwrap();
    let report: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(out.join("cutover-report.json")).unwrap()).unwrap();
    let info: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(out.join("devnet.json")).unwrap()).unwrap();
    assert_eq!(info["earnin"], true);
    assert_eq!((report["old"]["height"].as_u64(), report["old"]["blockHash"].as_str()), (Some(s), Some(hex(&chain.head().hash).as_str())));
    let bond20 = U256::from(20 * PCOL);
    let rewards: Vec<Address> = miners.iter().map(|(_, r)| *r).collect();
    let mut expected_supply = U256::ZERO;
    for (a, acct) in &old.accounts {
        if !pocold::testnet_allowed(*a) { continue }
        let i = &acct.info;
        let refund = apply::record_list(&old).into_iter().filter_map(|id| apply::miner_record(&old, id)).filter(|r| r.reward == *a && r.bond_state == 1 && !r.tomb).fold(U256::ZERO, |t, r| t + r.bond);
        expected_supply += i.balance + refund;
        if i.nonce == 0 && i.balance.is_zero() && refund.is_zero() && i.code.as_ref().is_none_or(|c| c.is_empty()) { continue }
        let new = &alloc.accounts.get(a).unwrap_or_else(|| panic!("{a:#x} missing")).info;
        let deducted = if rewards.contains(a) { bond20 } else { U256::ZERO };
        assert_eq!((new.nonce, new.balance), (i.nonce, i.balance + refund - deducted), "{a:#x}");
    }
    assert_eq!(report["totalSupply"].as_str().unwrap(), expected_supply.to_string(), "supply equals old balances plus refunded bonds");
    let c = &alloc.accounts[&contract];
    assert_eq!(c.info.code.as_ref().unwrap().original_byte_slice(), [0u8], "contract code carried over");
    assert_eq!(c.storage.get(&U256::ZERO), Some(&U256::from(42)), "contract storage carried over");
    assert_eq!(alloc.accounts[&carol].info.balance, U256::from(4 * PCOL));
    for i in 0..5 { assert!(!alloc.accounts.contains_key(&apply::program_address(i)), "system programs are rebuilt, not copied"); }

    // The new genesis is admitted and runs the earn-in rules: 20 PCOL genesis bonds, bob's old record gone.
    let new_spec = spec(&out);
    assert_eq!(new_spec.chain_id(), 777918);
    let genesis = Chain::new(new_spec);
    for (id, _) in &miners { assert_eq!(apply::miner_record(&genesis.head().state, *id).unwrap().bond, bond20); }
    assert!(apply::miner_record(&genesis.head().state, bob.address()).is_none());
    // Deterministic: the same inputs give the same genesis and allocation bytes.
    let again = temp(&format!("again-{tag}"));
    admitted(pocold::cutover::run(&old_dir, &data, s, config.as_bytes(), &again, limits(), &local()), &old_dir);
    for f in ["genesis.spec.hex", "alloc.json"] { assert_eq!(std::fs::read(out.join(f)).unwrap(), std::fs::read(again.join(f)).unwrap(), "{f}"); }
    for d in [old_dir, data, out, again] { let _ = std::fs::remove_dir_all(d); }
}

/// The runbook cuts S at least 60 blocks below the stopped head, below the K_hist window (32) of kept states.
/// The cutover rebuilds the state at S and writes the same files as a cutover taken while S was the head.
#[test]
fn a_cutover_70_blocks_below_the_head_rebuilds_the_state_and_matches_one_at_the_head() { deep("deep-whole", Limits::default) }
#[test]
fn a_store_mode_cutover_70_blocks_below_the_head_rebuilds_the_state_and_matches_one_at_the_head() {
    deep("deep-store", || Limits { state_store: true, ..Default::default() })
}
/// As Mainnet's nodes are at that depth: store mode with checkpoints (state.redb kept from one open to the
/// next) and the entries below the slim depth slimmed (C-1 S4), so the rebuild runs through restored entries.
#[test]
fn a_store_mode_cutover_through_slimmed_entries_and_checkpoints_matches_one_at_the_head() {
    deep("deep-slim", || Limits { state_store: true, slim_depth: Some(8), checkpoint_every: 8, ..Default::default() })
}

fn deep(tag: &str, limits: impl Fn() -> Limits) {
    let _serial = serial();
    let (store, slim) = (limits().state_store, limits().slim_depth.is_some());
    let (old_dir, data) = (temp(&format!("old-{tag}")), temp(&format!("data-{tag}")));
    let (_cfg, keys, miners) = old_network(&old_dir);
    let open = || Node::open(Config { dir: data.clone(), name: "old".into(), miner: None, listen: None, peers: vec![], rpc: None,
        stop_at: None, log: false, limits: limits() }, admitted(pocold::load_spec_for(&old_dir, &local(), store.then_some(data.as_path())), &old_dir)).unwrap();
    let mut chain = Chain::new(spec(&old_dir));
    let grow = |node: &mut Node, chain: &mut Chain, txs: Vec<Vec<u8>>| {
        let b = mine(chain, &keys, txs); let (e, _) = chain.validate(&b, None, 0).unwrap(); chain.insert(e);
        assert!(node.import(b, None).unwrap());
    };
    // A state at S that differs from genesis: a transfer and a contract with storage.
    let alice = devnet::account_key("alice");
    let init = unhex("602a600055600060005360016000f3").unwrap();
    let mut node = open();
    let t = tx(&chain, &alice, Some(Address::with_last_byte(0xca)), 3 * PCOL, vec![], 21_000); grow(&mut node, &mut chain, vec![t]);
    let t = tx(&chain, &alice, None, 0, init, 300_000); grow(&mut node, &mut chain, vec![t]);
    for _ in 0..3 { grow(&mut node, &mut chain, vec![]); }
    let s = chain.head().height;
    drop(node);
    let config = serde_json::json!({"continuous":true,"earnin":true,"chainId":777918,"genesisTime":1_800_000_000u64,"registrationPerBlock":1,
        "miners":miners.iter().map(|(id,reward)|serde_json::json!({"id":id,"reward":reward})).collect::<Vec<_>>(),"accounts":[]}).to_string();
    let at_head = temp(&format!("at-head-{tag}"));
    admitted(pocold::cutover::run(&old_dir, &data, s, config.as_bytes(), &at_head, limits(), &local()), &old_dir);
    // Positive control: the files compared below carry the transfer and the contract.
    let alloc = devnet::alloc_from_json(&serde_json::from_str(&std::fs::read_to_string(at_head.join("alloc.json")).unwrap()).unwrap()).unwrap();
    assert_eq!(alloc.accounts[&Address::with_last_byte(0xca)].info.balance, U256::from(3 * PCOL), "the transfer at S");
    assert!(alloc.accounts.values().any(|a| a.info.code.as_ref().is_some_and(|c| c.original_byte_slice() == [0u8]) && a.storage.get(&U256::ZERO) == Some(&U256::from(42))),
        "the contract at S, with its code and storage");

    // 70 more blocks: on open the node keeps no state at S (R21), so this cutover has to rebuild it.
    let mut node = open();
    for _ in 0..70 { grow(&mut node, &mut chain, vec![]); }
    drop(node);
    let mut node = open();
    let top = node.chain.head().height;
    assert_eq!(top, s + 70);
    let at_s = node.chain.canonical_at(s).unwrap().hash;
    assert!(node.chain.view_of(&at_s).is_none(), "the state at S is pruned before the cutover");
    if slim {
        assert!(node.chain.get(&at_s).unwrap().slim, "positive control: the entry at S is slimmed");
        assert!(node.stats.resumed_from.is_some(), "positive control: the open resumed from a checkpoint");
    } else {
        // The rebuild checks every re-executed state against its header: the cutover's rebuild refuses a
        // stateRoot at S that the re-execution does not reach, names the height, and keeps no state for S.
        // (A slimmed entry's header is read back from the journal, so this in-memory change applies to the
        // entries that are not slimmed.)
        node.chain.blocks.get_mut(&at_s).unwrap().header.state_root = [0xee; 32];
        match pocold::cutover::rebuild_at(&mut node, &at_s, s) { Err(m) => assert!(m.contains("stateRoot") && m.contains(&format!("at height {s}:")), "{m}"), r => panic!("{r:?}") }
        assert!(node.chain.view_of(&at_s).is_none(), "no state at S after the refused rebuild");
    }
    drop(node);
    let below = temp(&format!("below-{tag}"));
    admitted(pocold::cutover::run(&old_dir, &data, s, config.as_bytes(), &below, limits(), &local()), &old_dir);

    for f in ["genesis.spec.hex", "alloc.json"] { assert_eq!(std::fs::read(at_head.join(f)).unwrap(), std::fs::read(below.join(f)).unwrap(), "{f}"); }
    let json = |dir: &Path, f: &str| -> serde_json::Value { serde_json::from_str(&std::fs::read_to_string(dir.join(f)).unwrap()).unwrap() };
    let (mut r1, mut r2) = (json(&at_head, "cutover-report.json"), json(&below, "cutover-report.json"));
    assert_eq!((r1["old"]["head"].as_u64(), r2["old"]["head"].as_u64()), (Some(s), Some(top)));
    r1["old"].as_object_mut().unwrap().remove("head"); r2["old"].as_object_mut().unwrap().remove("head");
    assert_eq!(r1, r2, "the report, apart from the old head");
    let (mut d1, mut d2) = (json(&at_head, "devnet.json"), json(&below, "devnet.json"));
    d1.as_object_mut().unwrap().remove("timing"); d2.as_object_mut().unwrap().remove("timing");
    assert_eq!(d1, d2, "devnet.json, apart from the measured timing");
    // Again on the same data directory (step 2 can be run again): the same bytes. With checkpoints (deep-slim)
    // this run opens the store the first deep cutover wrote; without them each open starts the store anew.
    let again = temp(&format!("again-{tag}"));
    admitted(pocold::cutover::run(&old_dir, &data, s, config.as_bytes(), &again, limits(), &local()), &old_dir);
    for f in ["genesis.spec.hex", "alloc.json", "cutover-report.json"] { assert_eq!(std::fs::read(below.join(f)).unwrap(), std::fs::read(again.join(f)).unwrap(), "{f}, run twice"); }
    // The old data directory still opens at the same head (the rollback keeps it), from its checkpoint where it
    // has one, and its store is consistent.
    let node = open();
    assert_eq!(node.chain.head().height, top);
    if slim { assert!(node.stats.resumed_from.is_some(), "the old data directory still resumes from its checkpoint"); }
    drop(node);
    if store { pocold::state_store::StateStore::open(&data).unwrap().recount().unwrap(); }
    for d in [old_dir, data, at_head, below, again] { let _ = std::fs::remove_dir_all(d); }
}

/// C-1 S4: a block whose type-2 evidence names an ancestor the node has slimmed. The rebuild's evidence walk
/// reads the chain's own entries, so the cutover restores that ancestor and rebuilds again, and writes the
/// same files as a cutover from a node that never slims.
#[test]
fn a_deep_cutover_restores_the_slimmed_ancestor_an_evidence_walk_reads() {
    let _serial = serial();
    let (old_dir, d_slim, d_full) = (temp("old-ev"), temp("data-ev-slim"), temp("data-ev-full"));
    let (_cfg, keys, miners) = old_network(&old_dir);
    let limits = |depth: u64| Limits { state_store: true, slim_depth: Some(depth), ..Default::default() };
    let open = |dir: &Path, depth: u64| Node::open(Config { dir: dir.to_path_buf(), name: "old".into(), miner: None, listen: None, peers: vec![],
        rpc: None, stop_at: None, log: false, limits: limits(depth) }, admitted(pocold::load_spec_for(&old_dir, &local(), Some(dir)), &old_dir)).unwrap();
    let mut chain = Chain::new(spec(&old_dir));
    let mut blocks: Vec<Block> = Vec::new();
    let grow = |chain: &mut Chain, blocks: &mut Vec<Block>, b: Block| { let (e, _) = chain.validate(&b, None, 0).unwrap(); chain.insert(e); blocks.push(b); };
    for _ in 0..4 { let b = mine(&chain, &keys, vec![]); grow(&mut chain, &mut blocks, b); }
    // b4's winner signs a second share root for the same template and nonce: type-2 evidence.
    let b4 = blocks.last().unwrap().clone();
    let winner = chain.head().result.clone().unwrap().winner;
    let tid = template_id(&b4.header.ut_raw);
    let mut alt = b4.header.clone(); alt.shares = if b4.header.shares.is_empty() { vec![1] } else { vec![] }; reseal(&mut alt);
    let (root1, root2) = (share_root(&b4.header), share_root(&alt)); assert_ne!(root1, root2);
    let sig2 = keys[&winner].sign_hash(&B256::from(hashes::win_msg(&tid, b4.header.nonce, &root2)));
    let mut ev = Vec::new();
    for f in [&[2u8][..], &tid[..], &b4.header.nonce.to_be_bytes()[..], &root1[..], &b4.header.winner_sig[..], &root2[..], &sig2[..]] { pocold::rlp::encode_bytes(f, &mut ev); }
    let mut e2 = Vec::new(); pocold::rlp::encode_list_payload(&ev, &mut e2);
    // The first height from 7 whose proposer is not the winner carries it (within E_win of b4).
    loop {
        let p = chain.full_entry(&chain.head).unwrap(); let proposer = chain.child_ctx(&p, 0).unwrap().proposer;
        if p.height >= 6 && proposer != winner { let b = mine_with(&chain, &keys, vec![], vec![e2.clone()]); grow(&mut chain, &mut blocks, b); break }
        assert!(p.height < 4 + 15, "no other proposer within E_win");
        let b = mine(&chain, &keys, vec![]); grow(&mut chain, &mut blocks, b);
    }
    let j = chain.head().height;
    assert!(blocks.last().unwrap().body.evidence.len() == 1 && apply::miner_record(&chain.head().state, winner).unwrap().tomb, "positive control: the evidence tombstoned the winner");
    // Its burned bond leaves its reward address under the 20 PCOL genesis bond: fund it, and cut at that block.
    let reward = miners.iter().find(|(id, _)| *id == winner).unwrap().1;
    let t = tx(&chain, &devnet::account_key("alice"), Some(reward), 30 * PCOL, vec![], 21_000);
    let b = mine(&chain, &keys, vec![t]); grow(&mut chain, &mut blocks, b);
    let s = chain.head().height;
    for _ in 0..50 { let b = mine(&chain, &keys, vec![]); grow(&mut chain, &mut blocks, b); }
    for (dir, depth) in [(&d_slim, 3), (&d_full, u64::MAX)] { let mut n = open(dir, depth); for b in &blocks { assert!(n.import(b.clone(), None).unwrap()); } }
    let n = open(&d_slim, 3);
    assert!(n.chain.get(&b4.hash()).unwrap().slim, "positive control: the evidence's ancestor is slimmed");
    assert!(n.chain.view_of(&n.chain.canonical_at(s).unwrap().hash).is_none(), "the state at S is pruned before the cutover");
    assert!(j < s, "the rebuild of S re-executes the evidence block");
    drop(n);
    let config = serde_json::json!({"continuous":true,"earnin":true,"chainId":777918,"genesisTime":1_800_000_000u64,"registrationPerBlock":1,
        "miners":miners.iter().map(|(id,reward)|serde_json::json!({"id":id,"reward":reward})).collect::<Vec<_>>(),"accounts":[]}).to_string();
    let (o_slim, o_full) = (temp("out-ev-slim"), temp("out-ev-full"));
    admitted(pocold::cutover::run(&old_dir, &d_slim, s, config.as_bytes(), &o_slim, limits(3), &local()), &old_dir);
    admitted(pocold::cutover::run(&old_dir, &d_full, s, config.as_bytes(), &o_full, limits(u64::MAX), &local()), &old_dir);
    for f in ["genesis.spec.hex", "alloc.json", "cutover-report.json"] { assert_eq!(std::fs::read(o_slim.join(f)).unwrap(), std::fs::read(o_full.join(f)).unwrap(), "{f}"); }
    // One round, as for the node that never slims: the rebuild reads restored copies of the slimmed entries the
    // evidence block's walk reads (heights j - 2 down to the template's block b4; PR7, A1-7). Before PR7 the
    // first round ended with NeedsRebuild and a second one ran after restoring them (and before that fix, one
    // round per slimmed entry: 3 here).
    let mut n = open(&d_slim, 3);
    let at_s = n.chain.canonical_at(s).unwrap().hash;
    assert_eq!(pocold::cutover::rebuild_at(&mut n, &at_s, s), Ok(1));
    assert!(n.chain.get(&b4.hash()).unwrap().slim, "no slimmed entry was restored in place for it");
    assert!(n.chain.view_of(&at_s).is_some());
    drop(n);
    let mut n = open(&d_full, u64::MAX);
    assert_eq!(pocold::cutover::rebuild_at(&mut n, &at_s, s), Ok(1));
    drop(n);
    for d in [old_dir, d_slim, d_full, o_slim, o_full] { let _ = std::fs::remove_dir_all(d); }
}

#[test]
fn a_genesis_miner_without_a_durable_bond_balance_is_refused() {
    let db = revm::InMemoryDB::default();
    let poor = Address::with_last_byte(0x77);
    let err = pocold::cutover::transform(&db, 1, 4, &[poor], U256::from(20 * PCOL)).unwrap_err();
    assert!(err.contains("no balance"), "{err}");
    let mut db = revm::InMemoryDB::default();
    db.insert_account_info(poor, revm::primitives::AccountInfo { balance: U256::from(30 * PCOL), ..Default::default() });
    assert!(pocold::cutover::transform(&db, 1, 4, &[poor], U256::from(20 * PCOL)).unwrap_err().contains("not durable"));
    db.insert_account_info(poor, revm::primitives::AccountInfo { balance: U256::from(10 * PCOL), nonce: 3, ..Default::default() });
    assert!(pocold::cutover::transform(&db, 1, 4, &[poor], U256::from(20 * PCOL)).unwrap_err().contains("less than the bond"));
    db.insert_account_info(poor, revm::primitives::AccountInfo { balance: U256::from(25 * PCOL), nonce: 3, ..Default::default() });
    let (alloc, _) = pocold::cutover::transform(&db, 1, 4, &[poor], U256::from(20 * PCOL)).unwrap();
    assert_eq!(alloc.accounts[&poor].info.balance, U256::from(5 * PCOL));
}
