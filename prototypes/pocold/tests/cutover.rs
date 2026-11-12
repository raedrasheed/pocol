//! Cutover (EARN-IN-SPEC.md section 7) end to end: an "old" continuous v1 network with transfers, a deployed
//! contract with storage and a 100 PCOL registration; its stopped data directory becomes the genesis of an
//! earn-in network that keeps every user account, refunds the old bonds and funds 20 PCOL genesis bonds.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use pocol_lp4_execution::{apply, devnet::{self, DevnetConfig}, txsign::{Key, Tx}};
use pocold::{chain::{Chain, Spec}, hashes, node::{Config, Limits, Node}, types::*};
use revm::primitives::{keccak256, Address, Bytes, B256, U256};

const PCOL: u128 = 1_000_000_000_000_000_000;

/// A counting allocator switched on per thread (`peak`), so a test can bound the heap one code path requests while
/// other threads allocate freely (V3-33: the old-chain export's block stream).
mod meter {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    thread_local! { static ON: Cell<bool> = const { Cell::new(false) }; static NOW: Cell<isize> = const { Cell::new(0) }; static PEAK: Cell<isize> = const { Cell::new(0) }; }
    fn add(d: isize) { let _ = ON.try_with(|on| if on.get() { let _ = NOW.try_with(|n| { n.set(n.get() + d); let _ = PEAK.try_with(|p| p.set(p.get().max(n.get()))); }); }); }
    struct Meter;
    unsafe impl GlobalAlloc for Meter {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 { let p = unsafe { System.alloc(l) }; if !p.is_null() { add(l.size() as isize) } p }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) { unsafe { System.dealloc(p, l) }; add(-(l.size() as isize)) }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 { let q = unsafe { System.realloc(p, l, n) }; if !q.is_null() { add(n as isize - l.size() as isize) } q }
    }
    #[global_allocator] static A: Meter = Meter;
    /// The most heap bytes held at once, above what was held at its start, while `f` runs on this thread.
    pub fn peak<T>(f: impl FnOnce() -> T) -> (T, isize) {
        ON.with(|o| o.set(true)); NOW.with(|n| n.set(0)); PEAK.with(|p| p.set(0));
        let r = f(); let x = PEAK.with(|p| p.get()); ON.with(|o| o.set(false)); (r, x)
    }
}

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
fn a_stopped_old_chain_becomes_an_earn_in_genesis_that_keeps_every_account() { scenario(false, 20) }
/// The same from a store-mode data directory (state.redb trie versions), as `pocol start` nodes run.
#[test]
fn a_store_mode_old_chain_gives_the_same_cutover() { scenario(true, 20) }

#[test]
fn a_cutover_reads_a_25_pcol_bond_from_target_cp() { scenario(false, 25) }

fn scenario(store: bool, bond_pcol: u128) {
    let _serial = serial();
    let tag = format!("{}-{bond_pcol}",if store { "store" } else { "whole" });
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

    let mut config = serde_json::json!({"continuous":true,"earnin":true,"chainId":777918,"genesisTime":1_800_000_000u64,"registrationPerBlock":1,
        "miners":miners.iter().map(|(id,reward)|serde_json::json!({"id":id,"reward":reward})).collect::<Vec<_>>(),"accounts":[]});
    if bond_pcol!=20 { config["cp"]=serde_json::json!({"B_reg":(bond_pcol*PCOL).to_string()}); }
    let config=config.to_string();
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
    let new_bond = U256::from(bond_pcol * PCOL);
    let rewards: Vec<Address> = miners.iter().map(|(_, r)| *r).collect();
    let mut expected_supply = U256::ZERO;
    for (a, acct) in &old.accounts {
        if !pocold::testnet_allowed(*a) { continue }
        let i = &acct.info;
        let refund = apply::record_list(&old).into_iter().filter_map(|id| apply::miner_record(&old, id)).filter(|r| r.reward == *a && r.bond_state == 1 && !r.tomb).fold(U256::ZERO, |t, r| t + r.bond);
        expected_supply += i.balance + refund;
        if i.nonce == 0 && i.balance.is_zero() && refund.is_zero() && i.code.as_ref().is_none_or(|c| c.is_empty()) { continue }
        let new = &alloc.accounts.get(a).unwrap_or_else(|| panic!("{a:#x} missing")).info;
        let deducted = if rewards.contains(a) { new_bond } else { U256::ZERO };
        assert_eq!((new.nonce, new.balance), (i.nonce, i.balance + refund - deducted), "{a:#x}");
    }
    assert_eq!(report["totalSupply"].as_str().unwrap(), expected_supply.to_string(), "supply equals old balances plus refunded bonds");
    let c = &alloc.accounts[&contract];
    assert_eq!(c.info.code.as_ref().unwrap().original_byte_slice(), [0u8], "contract code carried over");
    assert_eq!(c.storage.get(&U256::ZERO), Some(&U256::from(42)), "contract storage carried over");
    assert_eq!(alloc.accounts[&carol].info.balance, U256::from(4 * PCOL));
    for i in 0..5 { assert!(!alloc.accounts.contains_key(&apply::program_address(i)), "system programs are rebuilt, not copied"); }

    // The new genesis is admitted and runs the earn-in rules: target-CP genesis bonds, bob's old record gone.
    let new_spec = spec(&out);
    assert_eq!(new_spec.chain_id(), 777918);
    let genesis = Chain::new(new_spec);
    for (id, _) in &miners { assert_eq!(apply::miner_record(&genesis.head().state, *id).unwrap().bond, new_bond); }
    assert!(apply::miner_record(&genesis.head().state, bob.address()).is_none());
    // Deterministic: the same inputs give the same genesis and allocation bytes, also when the run writes the V3-33
    // old-chain export (`--export-chain`) from the same rebuilt state: that export is the one `export-chain` writes,
    // and the cutover's own output does not change with it.
    let (again, exported, separate) = (temp(&format!("again-{tag}")), temp(&format!("with-export-{tag}")), temp(&format!("separate-export-{tag}")));
    std::fs::create_dir_all(&exported).unwrap();
    assert!(pocold::cutover::run_with_export(&old_dir, &data, s, config.as_bytes(), &again, Some(&exported), limits(), &local()).unwrap_err().contains("never overwritten"));
    assert!(!again.exists(), "an existing export directory is refused before anything is written");
    std::fs::remove_dir(&exported).unwrap();
    assert!(pocold::cutover::run_with_export(&old_dir, &data, s, config.as_bytes(), &again, Some(&again), limits(), &local()).unwrap_err().contains("different directories"));
    admitted(pocold::cutover::run_with_export(&old_dir, &data, s, config.as_bytes(), &again, Some(&exported), limits(), &local()), &old_dir);
    for f in ["genesis.spec.hex", "alloc.json", "cutover-report.json"] { assert_eq!(std::fs::read(out.join(f)).unwrap(), std::fs::read(again.join(f)).unwrap(), "{f}"); }
    admitted(pocold::cutover::export::run(&old_dir, &data, s, &separate, limits(), &local()), &old_dir);
    use pocold::cutover::export::{MANIFEST, PARTS};
    for f in [MANIFEST, PARTS[0], PARTS[1], PARTS[2]] { assert_eq!(std::fs::read(exported.join(f)).unwrap(), std::fs::read(separate.join(f)).unwrap(), "{f} of the export written by the cutover"); }
    pocold::cutover::export::verify(&exported, Some((chain.spec.chain_id(), chain.spec.genesis_hash()))).unwrap_or_else(|e| panic!("{e}"));
    for d in [old_dir, data, out, again, exported, separate] { let _ = std::fs::remove_dir_all(d); }
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

// ---- V3-33 re-genesis conduct (O-33=b): the freeze switch and the old-chain data export. ----

/// The block a node would produce on its head as Draw(h,0) signing at the earliest timestamp that gives a solution:
/// the node's own selection (or `forced`, given to its builder as is), its builder, then a nonce and the winner's
/// signature. Not recorded (no F7 record) and not broadcast.
fn node_block(n: &mut Node, keys: &HashMap<Address, Key>, forced: Option<Vec<Vec<u8>>>) -> Block {
    let p = n.chain.full_entry(&n.chain.head).unwrap();
    let ctx = n.chain.child_ctx(&p, 0).unwrap();
    n.cfg.miner = Some(Key::from_secret(&keys[&ctx.proposer].secret()).unwrap());
    for ts in (p.ts + u64::from(n.chain.spec.cp().T_blk)).max(ctx.window.lower())..=ctx.window.upper() {
        let sel = match &forced { Some(txs) => pocold::node::Selection { txs: txs.clone(), ..Default::default() }, None => n.template_selection(&p, ts) };
        let (mut header, body) = n.build_template_unrecorded(&p, sel, ts).expect("a template");
        let tid = template_id(&header.ut_raw);
        let (mut shares, mut nonce) = (Vec::new(), 0u64);
        while u128::from(nonce) < ctx.assignment.nonce_max() {
            let hash = pow_hash(&tid, nonce);
            if le_target(&hash, &ctx.target) {
                header.nonce = nonce; header.shares = shares; reseal(&mut header);
                let owner = addr(&ctx.assignment.owner(nonce).unwrap());
                header.winner_sig = keys[&owner].sign_hash(&B256::from(hashes::win_msg(&tid, nonce, &share_root(&header))));
                reseal(&mut header);
                n.cfg.miner = None;
                return Block { header, body };
            }
            if shares.len() < usize::from(n.chain.spec.cp().S_max) && le_target(&hash, &ctx.t_share) { shares.push(nonce); }
            nonce += 1;
        }
    }
    panic!("no solution");
}
/// The template header and body a node's builder gives on its head at `ts` for `txs` with Draw(h,0)'s key.
fn template_bytes(n: &mut Node, keys: &HashMap<Address, Key>, txs: Vec<Vec<u8>>, ts: u64) -> (Vec<u8>, Vec<u8>) {
    let p = n.chain.full_entry(&n.chain.head).unwrap();
    let proposer = n.chain.child_ctx(&p, 0).unwrap().proposer;
    n.cfg.miner = Some(Key::from_secret(&keys[&proposer].secret()).unwrap());
    let (h, b) = n.build_template_unrecorded(&p, pocold::node::Selection { txs, ..Default::default() }, ts).expect("a template");
    n.cfg.miner = None;
    (h.encoded, b.encode())
}
fn feed(chain: &mut Chain, nodes: &mut [&mut Node], b: &Block) {
    let (e, _) = chain.validate(b, None, 0).unwrap_or_else(|e| panic!("valid on the reference chain: {e:?}")); chain.insert(e);
    for n in nodes.iter_mut() { assert!(n.import(b.clone(), None).unwrap(), "imported as the new head"); }
}
fn rpc_send(n: &mut Node, raw: &[u8]) -> serde_json::Value {
    let body = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"eth_sendRawTransaction","params":[hex(raw)]}).to_string();
    serde_json::from_str(&pocold::rpc::handle(n, body.as_bytes())).unwrap()
}

/// V3-33 on a local three-member devnet, three nodes: A frozen after S = 3, B without a freeze, C with a freeze far ahead.
/// A's builder takes user transactions up to block S and none above it (whatever its pool or selection holds), its
/// pool refuses them from head S on with -32016 {frozen}, and it accepts B's block above S that carries one (validity
/// is unchanged). A's frozen template is the template B builds with no user transaction (same SYSTEM phases), and C's,
/// whose freeze is not active, is byte-equal to B's.
#[test]
fn a_frozen_node_builds_no_user_transaction_above_s_and_accepts_a_peers_block_that_has_one() {
    let _serial = serial();
    let (old_dir, da, db, dc) = (temp("freeze-net"), temp("freeze-a"), temp("freeze-b"), temp("freeze-c"));
    let (_cfg, keys, _miners) = old_network(&old_dir);
    let s = 3u64;
    let open = |dir: &PathBuf, freeze: Option<u64>| Node::open(Config { dir: dir.clone(), name: "n".into(), miner: None, listen: None, peers: vec![], rpc: None,
        stop_at: None, log: false, limits: Limits { freeze_user_txs_after: freeze, ..Default::default() } }, spec(&old_dir)).unwrap();
    let (mut a, mut b, mut c) = (open(&da, Some(s)), open(&db, None), open(&dc, Some(1000)));
    let mut chain = Chain::new(spec(&old_dir));
    let (alice, carol) = (devnet::account_key("alice"), Address::with_last_byte(0xca));
    for _ in 0..2 { let blk = mine(&chain, &keys, vec![]); feed(&mut chain, &mut [&mut a, &mut b, &mut c], &blk); }
    let freeze = |n: &Node| n.status_json().get("userTxFreeze").cloned();
    assert_eq!(freeze(&a), Some(serde_json::json!({"freezeUserTxsAfter":3,"active":false,"refused":0,"relayDropped":0})), "head 2: not yet active");
    assert_eq!(freeze(&b), None, "a node without a freeze reports nothing new");
    assert_eq!(freeze(&c).unwrap()["active"], false);

    // Head 2, block 3 = S: A still takes user transactions.
    let t1 = tx(&chain, &alice, Some(carol), PCOL, vec![], 21_000);
    for n in [&mut a, &mut b, &mut c] { n.submit_tx(t1.clone(), false).unwrap(); }
    let b3 = node_block(&mut a, &keys, None);
    assert_eq!(b3.body.txs, vec![t1.clone()], "block S built by A carries the user transaction");
    // A second one pooled at A that block S does not take: the head at S drains it from A's pool.
    let t1b = tx(&chain, &devnet::account_key("bob"), Some(carol), PCOL, vec![], 21_000);
    a.submit_tx(t1b.clone(), false).unwrap();
    feed(&mut chain, &mut [&mut a, &mut b, &mut c], &b3);
    a.pool_settle();
    assert_eq!(a.status_json()["pool"], 0, "the frozen pool drops what it held");
    assert_eq!(freeze(&a).unwrap()["active"], true, "head S: active");

    // Head S: A's pool refuses, in process and over RPC; B and C still pool.
    let t2 = tx(&chain, &alice, Some(carol), PCOL, vec![], 21_000);
    let e = a.submit_tx(t2.clone(), false).unwrap_err();
    assert_eq!(e, pocold::node::user_tx_frozen(3));
    assert!(e.starts_with(pocold::node::USER_TX_FROZEN));
    let r = rpc_send(&mut a, &t2);
    assert_eq!(r["error"]["code"], -32016, "{r}");
    assert_eq!(r["error"]["data"], serde_json::json!({"reason":"frozen","final":true,"freezeUserTxsAfter":3,"head":3}), "{r}");
    assert!(r["error"]["message"].as_str().unwrap().contains("frozen"), "{r}");
    assert_eq!(freeze(&a).unwrap()["refused"].as_u64().unwrap(), 4, "t1 and t1b at the revalidation of head S, t2 in process and over RPC");
    for n in [&mut b, &mut c] { n.submit_tx(t2.clone(), false).unwrap(); }
    let r = rpc_send(&mut b, &t2);
    assert!(r.get("error").is_none() || r["error"]["data"]["reason"] != "frozen", "B never answers frozen: {r}");

    // A's builder above S: nothing from the selection, nothing even when handed a transaction, and otherwise the
    // template B builds with no user transaction. C's inactive freeze changes nothing.
    let head = a.chain.full_entry(&a.chain.head).unwrap();
    let ts = (head.ts + u64::from(a.chain.spec.cp().T_blk)).max(a.chain.child_ctx(&head, 0).unwrap().window.lower());
    assert!(a.template_selection(&head, ts).txs.is_empty() && a.template_selection(&head, ts).rest.is_empty());
    assert_eq!(b.template_selection(&head, ts).txs, vec![t2.clone()]);
    let frozen = template_bytes(&mut a, &keys, vec![t2.clone()], ts);
    assert_eq!(frozen, template_bytes(&mut b, &keys, vec![], ts), "A's frozen template is B's without user transactions (SYSTEM phases unchanged)");
    let open_b = template_bytes(&mut b, &keys, vec![t2.clone()], ts);
    assert_ne!(open_b, frozen, "positive control: B's template carries t2");
    assert_eq!(template_bytes(&mut c, &keys, vec![t2.clone()], ts), open_b, "an inactive freeze builds the same bytes");

    // Validity is unchanged: B's block S+1 with a user transaction is valid on the reference chain and A imports it.
    let b4 = node_block(&mut b, &keys, None);
    assert_eq!(b4.body.txs, vec![t2.clone()]);
    feed(&mut chain, &mut [&mut a, &mut b, &mut c], &b4);
    assert_eq!(a.chain.head().hash, b4.hash(), "the frozen node follows a peer's block with user transactions above S");
    // A's block S+2: no user transaction although B still pools one; B and C import it.
    let t3 = tx(&chain, &alice, Some(carol), PCOL, vec![], 21_000);
    b.submit_tx(t3.clone(), false).unwrap();
    assert!(a.submit_tx(t3.clone(), false).is_err());
    let b5 = node_block(&mut a, &keys, Some(vec![t3.clone()]));
    assert!(b5.body.txs.is_empty(), "A's block above S carries no user transaction");
    feed(&mut chain, &mut [&mut a, &mut b, &mut c], &b5);
    assert_eq!(b.chain.head().hash, b5.hash());
    drop((a, b, c));
    for d in [old_dir, da, db, dc] { let _ = std::fs::remove_dir_all(d); }
}

/// V3-33: two nodes (one whole-state, one store-mode) that hold the same canonical chain write byte-equal exports of
/// it up to S, also once one of them is past S; the export verifies, a changed part does not (even with the manifest's
/// digests recomputed), and bad inputs are refused without writing anything.
#[test]
fn two_nodes_export_byte_equal_old_chain_data_that_verifies() {
    use pocold::cutover::export;
    let _serial = serial();
    let (old_dir, d1, d2) = (temp("export-net"), temp("export-d1"), temp("export-d2"));
    let (_cfg, keys, _miners) = old_network(&old_dir);
    let lim = |store: bool| Limits { state_store: store, ..Default::default() };
    let open = |dir: &PathBuf, store: bool| Node::open(Config { dir: dir.clone(), name: "old".into(), miner: None, listen: None, peers: vec![], rpc: None,
        stop_at: None, log: false, limits: lim(store) }, admitted(pocold::load_spec_for(&old_dir, &local(), store.then_some(dir.as_path())), &old_dir)).unwrap();
    let (mut n1, mut n2) = (open(&d1, false), open(&d2, true));
    let mut chain = Chain::new(spec(&old_dir));
    let (alice, bob, carol) = (devnet::account_key("alice"), devnet::account_key("bob"), Address::with_last_byte(0xca));
    let init = unhex("602a600055600060005360016000f3").unwrap();
    let mut register = keccak256("register(address)")[..4].to_vec(); register.extend_from_slice(bob.address().into_word().as_slice());
    let blk = mine(&chain, &keys, vec![tx(&chain, &alice, Some(carol), 3 * PCOL, vec![], 21_000)]); feed(&mut chain, &mut [&mut n1, &mut n2], &blk);
    let t = vec![tx(&chain, &alice, None, 0, init, 300_000), tx(&chain, &bob, Some(carol), PCOL, vec![], 21_000)];
    let blk = mine(&chain, &keys, t); feed(&mut chain, &mut [&mut n1, &mut n2], &blk);
    let reg = tx(&chain, &bob, Some(apply::program_address(0)), 100 * PCOL, register, 1_000_000);
    let blk = mine(&chain, &keys, vec![reg]); feed(&mut chain, &mut [&mut n1, &mut n2], &blk);
    for _ in 0..3 { let blk = mine(&chain, &keys, vec![]); feed(&mut chain, &mut [&mut n1, &mut n2], &blk); }
    let s = chain.head().height;
    let (head_s, root_s) = (chain.head().hash, chain.head().header.state_root);
    // n1 goes on past S; the export of S must not change.
    for _ in 0..3 { let blk = mine(&chain, &keys, vec![]); feed(&mut chain, &mut [&mut n1], &blk); }
    drop((n1, n2));

    let (o1, o2) = (temp("export-o1"), temp("export-o2"));
    // Refusals first: nothing is written.
    for (h, want) in [(0, "at least 1"), (s + 4, "above the head")] {
        let e = admitted_err(export::run(&old_dir, &d1, h, &o1, lim(false), &local()));
        assert!(e.contains(want), "{h}: {e}"); assert!(!o1.exists());
    }
    std::fs::create_dir_all(&o1).unwrap();
    assert!(admitted_err(export::run(&old_dir, &d1, s, &o1, lim(false), &local())).contains("never overwritten"));
    std::fs::remove_dir(&o1).unwrap();
    let m1 = admitted(export::run(&old_dir, &d1, s, &o1, lim(false), &local()), &old_dir);
    let m2 = admitted(export::run(&old_dir, &d2, s, &o2, lim(true), &local()), &old_dir);
    assert!(!o1.with_file_name(format!("{}.partial", o1.file_name().unwrap().to_string_lossy())).exists(), "the partial directory became the export");
    for f in [export::MANIFEST, export::PARTS[0], export::PARTS[1], export::PARTS[2]] {
        assert_eq!(std::fs::read(o1.join(f)).unwrap(), std::fs::read(o2.join(f)).unwrap(), "{f} is byte-equal on both nodes");
    }
    assert_eq!((m1["height"].as_u64(), m1["headHash"].as_str(), m1["stateRoot"].as_str()), (Some(s), Some(hex(&head_s).as_str()), Some(hex(&root_s).as_str())));
    assert_eq!(m1["parts"], m2["parts"]);
    assert_eq!(m1["genesisHash"], hex(&chain.spec.genesis_hash()));

    let g = (chain.spec.chain_id(), chain.spec.genesis_hash());
    let v = export::verify(&o1, Some(g)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((v["blocks"].as_u64(), v["transactions"].as_u64()), (Some(s + 1), Some(4)), "{v}");
    let facts: serde_json::Value = serde_json::from_slice(&std::fs::read(o1.join("facts.json")).unwrap()).unwrap();
    let bonds = facts["cutover"]["bondsCredited"].as_array().unwrap();
    assert!(bonds.iter().any(|b| b["bond"] == (100 * PCOL).to_string()), "bob's HELD bond is in the facts: {facts}");
    assert!(facts["registry"].as_array().unwrap().iter().any(|r| r["id"] == format!("{:#x}", bob.address())));
    let state = std::fs::read_to_string(o1.join("state.jsonl")).unwrap();
    assert!(state.contains(&format!("\"address\":\"{carol:#x}\",\"nonce\":0,\"balance\":\"{}\"", 4 * PCOL)), "carol's balance at S");
    assert!(state.contains("\"code\":\"0x00\""), "the contract's code");
    assert!(export::verify(&o1, Some((g.0 + 1, g.1))).unwrap_err().contains("another network"));

    // Tampering: a changed byte; a changed balance or fact with the manifest's digests recomputed.
    let copy = |tag: &str| { let t = temp(tag); std::fs::create_dir_all(&t).unwrap();
        for f in [export::MANIFEST, export::PARTS[0], export::PARTS[1], export::PARTS[2]] { std::fs::copy(o1.join(f), t.join(f)).unwrap(); } t };
    let redigest = |t: &Path| {
        let mut m: serde_json::Value = serde_json::from_slice(&std::fs::read(t.join(export::MANIFEST)).unwrap()).unwrap();
        for p in m["parts"].as_array_mut().unwrap() {
            let b = std::fs::read(t.join(p["name"].as_str().unwrap())).unwrap();
            p["bytes"] = serde_json::json!(b.len()); p["sha256"] = serde_json::json!(hex(&pocold::hashes::sha256(&b)));
        }
        std::fs::write(t.join(export::MANIFEST), serde_json::to_vec_pretty(&m).unwrap()).unwrap();
    };
    let t = copy("export-t1");
    let mut b = std::fs::read(t.join("blocks.jsonl")).unwrap(); let i = b.len() / 2; b[i] = if b[i] == b'a' { b'b' } else { b'a' };
    std::fs::write(t.join("blocks.jsonl"), &b).unwrap();
    assert!(export::verify(&t, None).unwrap_err().contains("do not match the manifest"));
    redigest(&t);
    assert!(export::verify(&t, None).is_err(), "a changed block fails its own checks");
    let t2 = copy("export-t2");
    std::fs::write(t2.join("state.jsonl"), state.replace(&format!("\"balance\":\"{}\"", 4 * PCOL), &format!("\"balance\":\"{}\"", 5 * PCOL))).unwrap();
    redigest(&t2);
    assert!(export::verify(&t2, None).unwrap_err().contains("world-state root"), "a changed balance does not reach the block's stateRoot");
    let t3 = copy("export-t3");
    let mut f = facts.clone(); f["cutover"]["allocTotal"] = serde_json::json!("1");
    std::fs::write(t3.join("facts.json"), serde_json::to_vec_pretty(&f).unwrap()).unwrap();
    redigest(&t3);
    assert!(export::verify(&t3, None).unwrap_err().contains("facts.json"));
    for d in [old_dir, d1, d2, o1, o2, t, t2, t3] { let _ = std::fs::remove_dir_all(d); }
}
/// The refusal of an export: a ParamGate refusal is reported like `admitted`, any other error is returned.
fn admitted_err(r: Result<serde_json::Value, String>) -> String {
    match r { Ok(v) => panic!("refused, not {v}"), Err(e) if e.starts_with("-32018") => panic!("{e}"), Err(e) => e }
}

/// V3-33, bounded memory: the export streams the blocks one at a time, so the heap its block stream holds at once does
/// not grow with the chain. Measured on one node at 8, 48, 96 and 160 blocks, every block with user transactions: at
/// every length the peak is a fixed part plus a few times the longest block line, far below the export's size.
#[test]
fn the_export_streams_its_blocks_in_memory_that_does_not_grow_with_the_chain() {
    use pocold::cutover::export;
    let _serial = serial();
    let (old_dir, data, out) = (temp("stream-net"), temp("stream-data"), temp("stream-out"));
    let (_cfg, keys, _miners) = old_network(&old_dir);
    let mut node = Node::open(Config { dir: data.clone(), name: "old".into(), miner: None, listen: None, peers: vec![], rpc: None,
        stop_at: None, log: false, limits: Limits::default() }, spec(&old_dir)).unwrap();
    let mut chain = Chain::new(spec(&old_dir));
    let alice = devnet::account_key("alice");
    std::fs::create_dir_all(&out).unwrap();
    let mut peaks = Vec::new();
    for (round, n) in [8u64, 48, 96, 160].into_iter().enumerate() {
        while chain.head().height < n {
            let t: Vec<_> = (0..4).map(|k| { let mut t = tx(&chain, &alice, Some(Address::with_last_byte(0xc0 + k)), PCOL, vec![], 21_000);
                // tx() reads the head's nonce; the k-th of this block follows it.
                if k > 0 { t = next_nonce(&chain, &alice, Address::with_last_byte(0xc0 + k), k as u64); } t }).collect();
            let blk = mine(&chain, &keys, t); feed(&mut chain, &mut [&mut node], &blk);
        }
        let hash = node.chain.head().hash;
        let path = out.join(format!("blocks-{round}.jsonl"));
        let (r, peak) = meter::peak(|| export::write_blocks(&node, n, &hash, &path));
        let (part, _) = r.unwrap();
        assert_eq!(part["records"].as_u64(), Some(n + 1));
        assert!(part["bytes"].as_u64().unwrap() > 1000 * n, "positive control: {n} blocks of transactions were written: {part}");
        let longest = std::fs::read_to_string(&path).unwrap().lines().map(str::len).max().unwrap() as isize;
        peaks.push((n, peak, longest));
    }
    eprintln!("EXPORT-STREAM-PEAK (blocks, peak, longest line) {peaks:?}");
    // What the stream holds at once: the file's buffer, one block's bytes, its decoded header and its line (the hex
    // text, twice the bytes): a few times the longest line, plus a fixed part. Not the chain: 160 blocks are written.
    for (n, peak, longest) in &peaks {
        assert!(*peak <= 64 * 1024 + 4 * longest, "{n} blocks: the stream held {peak} B, more than one block's work ({longest} B lines)");
    }
    let written: u64 = std::fs::metadata(out.join(format!("blocks-{}.jsonl", peaks.len() - 1))).unwrap().len();
    assert!(written as isize > 4 * peaks.last().unwrap().1, "positive control: the export is larger than what the stream held ({written} B)");
    drop(node);
    for d in [old_dir, data, out] { let _ = std::fs::remove_dir_all(d); }
}
/// `tx` with an explicit nonce offset from the head's (several transactions of one sender in one block).
fn next_nonce(chain: &Chain, key: &Key, to: Address, k: u64) -> Vec<u8> {
    let head = chain.full_entry(&chain.head).unwrap();
    let base = chain.child_ctx(&head, 0).unwrap().base_fee;
    let nonce = head.state.accounts.get(&key.address()).map(|a| a.info.nonce).unwrap_or(0) + k;
    let p = U256::from(1_000_000_000u64);
    Tx { chain_id: chain.spec.chain_id(), nonce, max_priority: p, max_fee: base * U256::from(2) + p, gas: 21_000, to: Some(to), value: U256::from(PCOL), data: Bytes::new() }.sign_1559(key)
}
