//! Rejoin mining against the real MinerRegistry, with the wallet's own calldata and step logic.
//! A member that stopped sharing is outside A_h and gets no nonce range. `deregister()` followed by
//! `reactivate()` included in [exit+W_act, exit+U) puts it back with a range. If the window passes,
//! the sweep refunds the bond and erases the record. No consensus rule is involved beyond the
//! existing Registry: this is the client path the wallet automates.
#[path="support/members.rs"] mod members_sweep;
use std::collections::HashMap;
use std::sync::OnceLock;

use pocol_lp4_execution::{admission::{self, Timing}, apply, devnet::{self, DevnetConfig}, txsign::{Key, Tx}};
use pocold::{chain::{Chain, Spec}, hashes, types::*, wallet::{self, RejoinStep}};
use revm::primitives::{Address, Bytes, B256, U256};

const LABELS: [&str; 2] = ["one", "two"];
/// Mainnet runs protocol version 2 (full nonce domain); activate it right after genesis.
const ACTIVATION: u64 = 2;
const PCOL: u128 = 1_000_000_000_000_000_000;

struct Net {
    chain: Chain,
    keys: HashMap<Address, Key>,
    one: Address,
    two: Address,
}
/// FN-3a: every test ends with the cold sweep of its chain (tests/support/members.rs).
impl Drop for Net { fn drop(&mut self) { crate::members_sweep::sweep_on_drop(&self.chain); } }

fn net() -> Net {
    let mut cfg = DevnetConfig::new(777915, 1_700_000_000, &LABELS);
    cfg.hash_rate = 16;
    cfg.c = 8;
    cfg.d_att = 73;
    // Mainnet's Registry allows one registration or reactivation per block.
    cfg.r_max = 1;
    let mut candidate = devnet::build(&cfg).unwrap();
    candidate.forks.push((2, ACTIVATION));
    let keys: HashMap<Address, Key> = candidate.miners.iter()
        .map(|m| (m.id, Key::from_secret(&m.key.secret()).unwrap())).collect();
    let (one, two) = (candidate.miners[0].id, candidate.miners[1].id);
    static TIMING: OnceLock<Timing> = OnceLock::new();
    let timing = TIMING.get_or_init(|| admission::measure(&candidate.spec_bytes, &candidate.alloc, 1).unwrap());
    let local = pocold::single_host_local();
    let admitted = admission::admit(&candidate.request(&local, Some(timing))).unwrap();
    Net { chain: Chain::new(Spec::new(admitted)), keys, one, two }
}

/// A block on the head whose winning nonce and shares all lie in `miner`'s range.
fn mine(net: &Net, miner: Address, txs: Vec<Vec<u8>>) -> Block {
    let parent = net.chain.full_entry(&net.chain.head).unwrap();
    let context = net.chain.child_ctx(&parent, 0).unwrap();
    let range = context.assignment.ranges().iter().find(|r| r.id() == raw_addr(miner))
        .expect("the miner has a nonce range");
    let first_time = parent.ts + u64::from(net.chain.spec.cp().T_blk);
    let body = Body { txs, evidence: vec![] };
    for timestamp in first_time..=context.window.upper() {
        let (mut header, _, _) = net.chain.build_signed(
            &parent, &net.keys[&context.proposer], timestamp, body.clone(), vec![],
        ).unwrap();
        let tid = template_id(&header.ut_raw);
        let mut shares = Vec::new();
        let mut nonce = range.start();
        while nonce < range.end_exclusive().min(context.assignment.nonce_max()) {
            let hash = pow_hash(&tid, nonce as u64);
            if le_target(&hash, &context.target) {
                header.nonce = nonce as u64;
                header.shares = shares;
                reseal(&mut header);
                header.winner_sig = net.keys[&miner].sign_hash(&B256::from(
                    hashes::win_msg(&tid, nonce as u64, &share_root(&header)),
                ));
                reseal(&mut header);
                return Block { header, body };
            }
            if shares.len() < usize::from(net.chain.spec.cp().S_max) && le_target(&hash, &context.t_share) {
                shares.push(nonce as u64);
            }
            nonce += 1;
        }
    }
    panic!("no solution for deterministic test fixture");
}

#[derive(Clone, Copy)]
enum Who { One, Two }
/// Mines the next block in `who`'s range with `txs`, validates it as a peer would, and imports it.
fn next(net: &mut Net, who: Who, txs: Vec<Vec<u8>>) {
    let miner = match who { Who::One => net.one, Who::Two => net.two };
    let block = mine(net, miner, txs);
    let (entry, _) = net.chain.validate(&block, None, 0).unwrap();
    assert!(net.chain.insert(entry));
}
/// (W_act, U) of the Registry.
fn window(net: &Net) -> (u64, u64) { let cp = net.chain.spec.cp(); (u64::from(cp.W_act), u64::from(cp.U)) }

fn tx(net: &Net, key: &Key, to: Address, value: U256, data: Bytes, gas: u64) -> Vec<u8> {
    let head = net.chain.full_entry(&net.chain.head).unwrap();
    let base = net.chain.child_ctx(&head, 0).unwrap().base_fee;
    let nonce = head.state.accounts.get(&key.address()).map(|a| a.info.nonce).unwrap_or(0);
    let priority = U256::from(1_000_000_000u64);
    Tx { chain_id: net.chain.spec.chain_id(), nonce, max_priority: priority, max_fee: base * U256::from(2) + priority,
         gas, to: Some(to), value, data }.sign_1559(key)
}
fn registry_call(net: &Net, data: Bytes) -> Vec<u8> {
    tx(net, &net.keys[&net.two], apply::program_address(0), U256::ZERO, data, 1_000_000)
}

fn record(net: &Net, id: Address) -> Option<apply::MinerRecord> { apply::miner_record(&net.chain.head().state, id) }
fn member(net: &Net, id: Address) -> bool {
    let head = net.chain.head();
    net.chain.membership(head, head.height + 1).unwrap().ids().contains(&raw_addr(id))
}
fn step(net: &Net, id: Address) -> RejoinStep {
    wallet::rejoin_step(record(net, id).as_ref(), net.chain.head().height + 1, member(net, id), u64::from(net.chain.spec.cp().W_act), net.chain.spec.probation().map(|p| p.blocks))
}
fn height(net: &Net) -> u64 { net.chain.head().height }
fn balance(net: &Net, id: Address) -> U256 { net.chain.head().state.accounts.get(&id).map(|a| a.info.balance).unwrap_or_default() }

/// Funds `two` (its identity pays the fees), lets it fall out of A_h while `one` mines, then exits it.
fn inactive_then_exited(net: &mut Net) -> u64 {
    let funding = tx(net, &devnet::account_key("alice"), net.two, U256::from(10 * PCOL), Bytes::new(), 21_000);
    next(net, Who::One, vec![funding]);
    assert!(member(net, net.two), "both members start active (grace from genesis)");
    while member(net, net.two) {
        assert!(height(net) < 100, "an idle member must leave A_h after W_act blocks");
        next(net, Who::One, vec![]);
    }
    let (w_act, u) = window(net);
    let idle = record(net, net.two).unwrap();
    assert_eq!((idle.status, idle.tomb, idle.bond_state), (1, false, 1), "still a held, status-1 member");
    assert!(idle.last_share.max(idle.grace_start) + w_act < height(net) + 1);
    assert_eq!(step(net, net.two), RejoinStep::Exit);
    let context = net.chain.child_ctx(net.chain.head(), 0).unwrap();
    assert!(context.assignment.ranges().iter().all(|r| r.id() != raw_addr(net.two)), "no nonce range while inactive");

    // Step 1 of the wallet's plan: deregister(). The bond stays held.
    let exit_call = registry_call(net, wallet::exit_calldata());
    next(net, Who::One, vec![exit_call]);
    let exit = height(net);
    let exited = record(net, net.two).unwrap();
    assert_eq!((exited.status, exited.bond_state, exited.exit_height, exited.due_height), (2, 1, exit, exit + u));
    assert_eq!(exited.bond, U256::from(100 * PCOL));
    assert_eq!(step(net, net.two), RejoinStep::Wait { opens: exit + w_act, closes: exit + u });
    exit
}

#[test]
fn an_inactive_member_rejoins_with_a_nonce_range_and_its_bond() {
    let mut net = net();
    let exit = inactive_then_exited(&mut net);
    let (w_act, u) = window(&net);
    let opens = exit + w_act;

    // One block early: the Registry rejects it (the wallet never signs before `opens`).
    while height(&net) + 1 < opens - 1 { next(&mut net, Who::One, vec![]); }
    let early = registry_call(&net, wallet::reactivate_calldata());
    next(&mut net, Who::One, vec![early]);
    assert_eq!(record(&net, net.two).unwrap().status, 2, "reactivate() before exit+W_act reverts");
    assert_eq!(step(&net, net.two), RejoinStep::Reactivate { closes: exit + u });

    // Step 2: reactivate() at the first allowed height.
    let reactivate = registry_call(&net, wallet::reactivate_calldata());
    next(&mut net, Who::One, vec![reactivate]);
    assert_eq!(height(&net), opens);
    let back = record(&net, net.two).unwrap();
    assert_eq!((back.status, back.bond_state, back.grace_start, back.bond), (1, 1, opens, U256::from(100 * PCOL)));
    assert!(member(&net, net.two));
    assert_eq!(step(&net, net.two), RejoinStep::Active);
    let context = net.chain.child_ctx(net.chain.head(), 0).unwrap();
    assert_eq!(context.assignment.ranges().len(), 2, "both members split the nonce domain again");

    // Step 3: with mining on it wins and shares in its own range, which keeps it active past the grace.
    next(&mut net, Who::Two, vec![]);
    let won = height(&net);
    next(&mut net, Who::Two, vec![]);
    assert!(record(&net, net.two).unwrap().last_share >= won, "its shares are recorded");
    while height(&net) <= opens + w_act { next(&mut net, Who::Two, vec![]); }
    assert!(member(&net, net.two), "a sharing member stays in A_h after the grace window");
}

#[test]
fn a_missed_window_refunds_the_bond_and_removes_the_record() {
    let mut net = net();
    let exit = inactive_then_exited(&mut net);
    let (_, u) = window(&net);
    let due = exit + u;
    while height(&net) + 1 < due { next(&mut net, Who::One, vec![]); }
    assert_eq!(step(&net, net.two), RejoinStep::Missed);
    let reward = record(&net, net.two).unwrap().reward;
    let before = balance(&net, reward);
    next(&mut net, Who::One, vec![]);
    assert_eq!(record(&net, net.two), None, "the sweep erases the record at exit+U");
    assert_eq!(balance(&net, reward) - before, U256::from(100 * PCOL), "and pays the bond to the reward address");
    assert_eq!(step(&net, net.two), RejoinStep::Unavailable);
}
