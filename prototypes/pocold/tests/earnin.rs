//! Earn-in network on a full chain (EARN-IN-SPEC.md): real blocks, real settlement through
//! RewardEscrow. A zero-bond registration as an account's first transaction (R9) earns its 20 PCOL
//! bond from its own wins and shares, then is promoted and paid normally; a top-up promotes at once and
//! returns whatever exceeds the remainder.
#[path="support/members.rs"] mod members_sweep;
use std::collections::HashMap;
use std::sync::OnceLock;

use pocol_lp4_execution::{admission::{self, Timing}, apply, devnet::{self, DevnetConfig}, txsign::{Key, Tx}};
use pocold::{chain::{Chain, Spec}, hashes, types::*};
use revm::primitives::{keccak256, Address, Bytes, B256, U256};

const PCOL: u128 = 1_000_000_000_000_000_000;
const B: u128 = 20 * PCOL;

struct Net { chain: Chain, keys: HashMap<Address, Key>, one: Address }
/// FN-3a: every test ends with the cold sweep of its chain (tests/support/members.rs).
impl Drop for Net{fn drop(&mut self){crate::members_sweep::sweep_on_drop(&self.chain);}}

fn net() -> Net {
    let mut cfg = DevnetConfig::new(777916, 1_700_000_000, &["one", "two"]);
    cfg.hash_rate = 16;
    cfg.c = 8;
    cfg.d_att = 73;
    cfg.r_max = 1;
    cfg.earnin = true;
    let mut candidate = devnet::build(&cfg).unwrap();
    candidate.forks.push((2, 2));
    let mut keys: HashMap<Address, Key> = candidate.miners.iter()
        .map(|m| (m.id, Key::from_secret(&m.key.secret()).unwrap())).collect();
    for label in ["alice", "bob"] { let k = devnet::account_key(label); keys.insert(k.address(), k); }
    let one = candidate.miners[0].id;
    static TIMING: OnceLock<Timing> = OnceLock::new();
    let timing = TIMING.get_or_init(|| admission::measure(&candidate.spec_bytes, &candidate.alloc, 1).unwrap());
    let admitted = admission::admit(&candidate.request(&pocold::single_host_local(), Some(timing))).unwrap();
    Net { chain: Chain::new(Spec::new(admitted)), keys, one }
}

/// The next block, won (and shared) entirely in `miner`'s nonce range.
fn next(net: &mut Net, miner: Address, txs: Vec<Vec<u8>>) -> Address {
    let parent = net.chain.full_entry(&net.chain.head).unwrap();
    let context = net.chain.child_ctx(&parent, 0).unwrap();
    let range = context.assignment.ranges().iter().find(|r| r.id() == raw_addr(miner)).unwrap_or_else(|| panic!("miner has no range at {}: {:?}", parent.height + 1, context.assignment.ranges().iter().map(|r| r.id()).collect::<Vec<_>>()));
    let body = Body { txs, evidence: vec![] };
    for timestamp in parent.ts + u64::from(net.chain.spec.cp().T_blk)..=context.window.upper() {
        let (mut header, _, _) = net.chain.build_signed(&parent, &net.keys[&context.proposer], timestamp, body.clone(), vec![]).unwrap();
        let tid = template_id(&header.ut_raw);
        let (mut shares, mut nonce) = (Vec::new(), range.start());
        while nonce < range.end_exclusive().min(context.assignment.nonce_max()) {
            let hash = pow_hash(&tid, nonce as u64);
            if le_target(&hash, &context.target) {
                header.nonce = nonce as u64; header.shares = shares; reseal(&mut header);
                header.winner_sig = net.keys[&miner].sign_hash(&B256::from(hashes::win_msg(&tid, nonce as u64, &share_root(&header))));
                reseal(&mut header);
                let (entry, _) = net.chain.validate(&Block { header, body }, None, 0).unwrap();
                assert!(net.chain.insert(entry));
                let h = net.chain.head();
                apply::check_escrow_invariant(&h.state, net.chain.spec.profile(), h.height, false).unwrap();
                // The owner list the child's recordShares receives (shares and winner of this real block) names
                // only registered records: members of the parent's ActList, still records after this block.
                let owners = h.result.as_ref().expect("a validated block has a result").owner_sequence();
                let active = apply::active_list(&parent.state);
                for id in owners {
                    assert!(active.contains(&id) && apply::miner_record(&parent.state, id).is_some() && apply::miner_record(&h.state, id).is_some(),
                        "recordShares owner {id} at {} is not a registered member of the parent's ActList", h.height);
                }
                return context.proposer;
            }
            if shares.len() < usize::from(net.chain.spec.cp().S_max) && le_target(&hash, &context.t_share) { shares.push(nonce as u64); }
            nonce += 1;
        }
    }
    panic!("no solution for deterministic test fixture");
}

fn tx(net: &Net, key: &Key, data: Vec<u8>, value: U256) -> Vec<u8> {
    let head = net.chain.full_entry(&net.chain.head).unwrap();
    let base = net.chain.child_ctx(&head, 0).unwrap().base_fee;
    let nonce = head.state.accounts.get(&key.address()).map(|a| a.info.nonce).unwrap_or(0);
    let priority = U256::from(1_000_000_000u64);
    Tx { chain_id: net.chain.spec.chain_id(), nonce, max_priority: priority, max_fee: base * U256::from(2) + priority, gas: 1_000_000,
         to: Some(apply::program_address(0)), value, data: Bytes::from(data) }.sign_1559(key)
}
fn register(net: &Net, key: &Key, value: u128) -> Vec<u8> {
    let mut data = keccak256("register(address)")[..4].to_vec();
    data.extend_from_slice(key.address().into_word().as_slice());
    tx(net, key, data, U256::from(value))
}
fn record(net: &Net, id: Address) -> apply::MinerRecord { apply::miner_record(&net.chain.head().state, id).expect("record") }
fn accruing_count(net: &Net) -> U256 {
    net.chain.head().state.accounts[&apply::program_address(0)].storage.get(&U256::from(8)).copied().unwrap_or_default()
}
fn balance(net: &Net, a: Address) -> U256 { net.chain.head().state.accounts.get(&a).map(|x| x.info.balance).unwrap_or_default() }
fn nonce(net: &Net, a: Address) -> u64 { net.chain.head().state.accounts.get(&a).map(|x| x.info.nonce).unwrap_or(0) }

#[test]
fn a_zero_bond_miner_earns_its_bond_then_is_paid_and_top_up_promotes_at_once() {
    let mut net = net();
    let one = net.one;
    assert_eq!(apply::miner_record(&net.chain.head().state, net.one).unwrap().bond, U256::from(B), "genesis bonds are 20 PCOL");
    let alice = devnet::account_key("alice");
    let a = alice.address();
    assert_eq!(nonce(&net, a), 0, "the registration is alice's first transaction (R9)");
    let t = register(&net, &alice, 0);
    let _ = next(&mut net, one, vec![t]);
    let r = record(&net, a);
    assert_eq!((r.status, r.bond_state, r.bond, r.reward), (1, 4, U256::ZERO, a), "a probation record");
    assert_eq!(accruing_count(&net), U256::from(1));
    let funded = balance(&net, a);

    // Alice mines: her winner share and her share payments settle into the bond, not her balance.
    let mut last = U256::ZERO;
    while record(&net, a).bond_state == 4 {
        assert!(net.chain.head().height < 200, "the bond must fill within the fixture's horizon");
        let accruing_before = record(&net, a).bond_state == 4;
        let proposer = next(&mut net, a, vec![]);
        // D1: while she is ACCRUING and full members remain in A_h, she is never the proposer.
        if accruing_before { assert_ne!(proposer, a, "an ACCRUING member was drawn as proposer at {}", net.chain.head().height); }
        let r = record(&net, a);
        assert!(r.bond >= last && r.bond <= U256::from(B), "the bond only grows up to B");
        last = r.bond;
        if r.bond_state == 4 { assert_eq!(balance(&net, a), funded, "nothing is paid out while accruing"); }
    }
    let promoted = record(&net, a);
    assert_eq!((promoted.bond_state, promoted.bond), (1, U256::from(B)), "promoted to HELD at exactly B");
    assert_eq!(accruing_count(&net), U256::ZERO);
    // From now on her rewards reach her balance (any overflow of the promoting payment already did).
    let before = balance(&net, a);
    for _ in 0..6 { let _ = next(&mut net, a, vec![]); }
    assert!(balance(&net, a) > before, "a HELD member is paid normally");

    // Alice is the active miner by now (the genesis miners stopped sharing), so she mines the rest.
    // Top-up: bob joins with 5 PCOL, tops up part, then sends more than the remainder (as a wallet
    // quote does when rewards accrued meanwhile): the bond fills to B and the rest returns to bob.
    let bob = devnet::account_key("bob");
    let t = register(&net, &bob, 5 * PCOL);
    let _ = next(&mut net, a, vec![t]);
    assert_eq!((record(&net, bob.address()).bond_state, record(&net, bob.address()).bond), (4, U256::from(5 * PCOL)));
    let part = tx(&net, &bob, keccak256("topUp()")[..4].to_vec(), U256::from(4 * PCOL));
    let _ = next(&mut net, a, vec![part]);
    assert_eq!((record(&net, bob.address()).bond_state, record(&net, bob.address()).bond), (4, U256::from(9 * PCOL)), "a partial top-up stays on probation");
    let before = balance(&net, bob.address());
    let over = tx(&net, &bob, keccak256("topUp()")[..4].to_vec(), U256::from(16 * PCOL));
    let _ = next(&mut net, a, vec![over]);
    let r = record(&net, bob.address());
    assert_eq!((r.bond_state, r.bond), (1, U256::from(B)), "promoted in the top-up's block");
    let spent = before - balance(&net, bob.address());
    assert!(spent > U256::from(11 * PCOL) && spent < U256::from(11 * PCOL + PCOL / 100), "only the 11 PCOL remainder (plus the fee) left bob: {spent}");
    assert_eq!(accruing_count(&net), U256::ZERO);
}
