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

fn mine(chain: &Chain, keys: &HashMap<Address, Key>, txs: Vec<Vec<u8>>) -> Block {
    let parent = chain.full_entry(&chain.head).unwrap();
    let ctx = chain.child_ctx(&parent, 0).unwrap();
    let body = Body { txs, evidence: vec![] };
    for ts in parent.ts + u64::from(chain.spec.cp().T_blk)..=ctx.window.upper() {
        let (mut header, _, _) = chain.build_signed(&parent, &keys[&ctx.proposer], ts, body.clone(), vec![]).unwrap();
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
fn spec(dir: &Path) -> Spec { pocold::load_spec_for(dir, &local(), None).unwrap() }
fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("pocol-cutover-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d); d
}

#[test]
fn a_stopped_old_chain_becomes_an_earn_in_genesis_that_keeps_every_account() { scenario(false) }
/// The same from a store-mode data directory (state.redb trie versions), as `pocol start` nodes run.
#[test]
fn a_store_mode_old_chain_gives_the_same_cutover() { scenario(true) }

fn scenario(store: bool) {
    let tag = if store { "store" } else { "whole" };
    let limits = || Limits { state_store: store, ..Default::default() };
    let (old_dir, data) = (temp(&format!("old-{tag}")), temp(&format!("data-{tag}")));
    let (_cfg, keys, miners) = old_network(&old_dir);
    let mut chain = Chain::new(spec(&old_dir));
    let node_spec = pocold::load_spec_for(&old_dir, &local(), store.then_some(data.as_path())).unwrap();
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
    pocold::cutover::run(&old_dir, &data, s, config.as_bytes(), &out, limits(), &local()).unwrap();

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
    pocold::cutover::run(&old_dir, &data, s, config.as_bytes(), &again, limits(), &local()).unwrap();
    for f in ["genesis.spec.hex", "alloc.json"] { assert_eq!(std::fs::read(out.join(f)).unwrap(), std::fs::read(again.join(f)).unwrap(), "{f}"); }
    for d in [old_dir, data, out, again] { let _ = std::fs::remove_dir_all(d); }
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
