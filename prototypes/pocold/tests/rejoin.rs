//! Rejoin mining against the real MinerRegistry, with the wallet's own calldata and step logic. No consensus rule is
//! involved beyond the existing Registry: this is the client path the wallet automates.
//! The spec-1 flow (a member that stopped sharing fell out of the activity window A_h on bounded nonce ranges and came
//! back by deregister and reactivate) was deleted with the spec-1 rule set in phase 6a. On the v3 rules (V3-01, D-V3-01;
//! FN-2b C4) a live member is never outside the nonce ranges: a miner that goes offline keeps its seat, its range and its bond,
//! and comes back by mining (`v3_*` below). It loses its seat only at a full set, past Theta_inact, to a newcomer's
//! registerReplacing; then it is refunded after U and rejoins by registering again (into a removable seat while the set
//! is full), earning its bond back from its rewards.
#[path="support/members.rs"] mod members_sweep;
use std::collections::HashMap;

use pocol_lp4_execution::{admission, apply, devnet::{self, DevnetConfig}, txsign::{Key, Tx}};
use pocold::{chain::{Chain, Spec}, hashes, types::*, wallet::{self, RejoinStep}};
use revm::primitives::{Address, Bytes, B256, U256};

const LABELS: [&str; 2] = ["one", "two"];
const PCOL: u128 = 1_000_000_000_000_000_000;
const B_REG: u128 = 20 * PCOL;

struct Net {
    chain: Chain,
    keys: HashMap<Address, Key>,
    one: Address,
    two: Address,
}
/// FN-3a: every test ends with the cold sweep of its chain (tests/support/members.rs).
impl Drop for Net { fn drop(&mut self) { crate::members_sweep::sweep_on_drop(&self.chain); } }


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

fn tx(net: &Net, key: &Key, to: Address, value: U256, data: Bytes, gas: u64) -> Vec<u8> {
    let head = net.chain.full_entry(&net.chain.head).unwrap();
    let base = net.chain.child_ctx(&head, 0).unwrap().base_fee;
    let nonce = head.state.accounts.get(&key.address()).map(|a| a.info.nonce).unwrap_or(0);
    let priority = U256::from(1_000_000_000u64);
    Tx { chain_id: net.chain.spec.chain_id(), nonce, max_priority: priority, max_fee: base * U256::from(2) + priority,
         gas, to: Some(to), value, data }.sign_1559(key)
}

fn record(net: &Net, id: Address) -> Option<apply::MinerRecord> { apply::miner_record(&net.chain.head().state, id) }
fn member(net: &Net, id: Address) -> bool {
    let head = net.chain.head();
    net.chain.membership(head, head.height + 1).unwrap().ids().contains(&raw_addr(id))
}
fn height(net: &Net) -> u64 { net.chain.head().height }
fn balance(net: &Net, id: Address) -> U256 { net.chain.head().state.accounts.get(&id).map(|a| a.info.balance).unwrap_or_default() }

/// The v3 devnet (DevnetConfig::new: CP schema v2, the V3 programs, nonceMode 1, a continuous chain, B_reg 20 PCOL),
/// with the same easy PoW, at `m_max` seats and `theta_inact` blocks of inactivity before a member is removable.
fn v3_net(m_max: u16, theta_inact: u32) -> Net {
    let mut cfg = DevnetConfig::new(777916, 1_700_000_000, &LABELS);
    cfg.hash_rate = 16;
    cfg.c = 8;
    cfg.d_att = 73;
    cfg.m_max = m_max;
    cfg.theta_inact = theta_inact;
    cfg.accounts.push(("carol".into(), U256::from(10u128.pow(24))));
    let candidate = devnet::build(&cfg).unwrap();
    let keys: HashMap<Address, Key> = candidate.miners.iter()
        .map(|m| (m.id, Key::from_secret(&m.key.secret()).unwrap())).collect();
    let (one, two) = (candidate.miners[0].id, candidate.miners[1].id);
    let timing = admission::measure(&candidate.spec_bytes, &candidate.alloc, 1).unwrap();
    let admitted = admission::admit(&candidate.request(&pocold::single_host_local(), Some(&timing))).unwrap();
    crate::members_sweep::oracle();
    let chain = Chain::new(Spec::new(admitted));
    assert_eq!(chain.spec.programs(), apply::ProgramSet::V3);
    assert!(chain.spec.v3_active(0) && pocold::members::all_live_everywhere(chain.spec.cp()), "the v3 membership: every live record (V3-01)");
    assert_eq!(chain.spec.effective_nonce_mode(1), pocold::assignment::NonceMode::Unbounded);
    Net { chain, keys, one, two }
}
fn call(net: &Net, key: &Key, data: Bytes, value: u128) -> Vec<u8> {
    tx(net, key, apply::program_address(0), U256::from(value), data, 1_000_000)
}
fn receipts_status(net: &Net) -> Vec<Vec<u8>> {
    let entry = net.chain.full_entry(&net.chain.head).unwrap();
    entry.receipts.iter().map(|r| pocold::rlp::decode(&r[1..]).unwrap().list().unwrap()[0].bytes().unwrap().to_vec()).collect()
}
fn removable(net: &Net) -> Vec<Address> {
    let head = net.chain.head();
    wallet::removable_members(&net.chain, &head.state, head.height + 1, &wallet::head_credited(&net.chain))
}
fn has_range(net: &Net, id: Address) -> bool {
    let context = net.chain.child_ctx(net.chain.head(), 0).unwrap();
    context.assignment.ranges().iter().any(|r| r.id() == raw_addr(id))
}
fn remove_inactive(victim: Address) -> Bytes {
    let mut d = revm::primitives::keccak256("removeInactive(address)")[..4].to_vec(); d.extend_from_slice(victim.into_word().as_slice()); d.into()
}

/// V3-01: genesis member two goes offline (one mines every block) past W_act and past Theta_inact. It never leaves the
/// membership: at every height it is in A_next with a nonce range, its record stays a live HELD status-1 record with its
/// 20 PCOL bond, and the wallet's rejoin step is Active (nothing to reactivate). Below a full set nobody is removable and
/// removeInactive reverts. When it comes back it mines in its own range at once: it wins, its shares are recorded and its
/// pots pay its reward address at settlement.
#[test]
fn v3_an_offline_member_keeps_its_seat_and_range_and_comes_back_by_mining() {
    let mut net = v3_net(64, 64);
    let cp = net.chain.spec.cp().clone();
    let (w_act, theta) = (u64::from(cp.W_act), u64::from(cp.Theta_inact));
    let alice = devnet::account_key("alice");
    let fund = tx(&net, &alice, net.two, U256::from(PCOL), Bytes::new(), 21_000);
    next(&mut net, Who::One, vec![fund]);
    while height(&net) <= w_act.max(theta) + 2 {
        assert!(member(&net, net.two) && has_range(&net, net.two), "offline, two keeps its seat and range at {}", height(&net) + 1);
        next(&mut net, Who::One, vec![]);
    }
    let idle = record(&net, net.two).unwrap();
    assert!(idle.last_share.max(idle.grace_start) + theta < height(&net) + 1, "two has been idle past Theta_inact: {idle:?}");
    assert_eq!((idle.status, idle.tomb, idle.bond_state, idle.bond), (1, false, 1, U256::from(B_REG)), "a live HELD member with its bond");
    assert_eq!(wallet::chain_rejoin_step(&net.chain, net.two), RejoinStep::Active, "nothing to reactivate on v3");
    assert!(wallet::rejoin_refusal(RejoinStep::Active).unwrap().contains("already in the active mining set"));
    // Below a full set (2 of 64 seats) the inactivity rule is not enforced: nobody is removable, removeInactive reverts.
    assert_eq!(removable(&net), Vec::<Address>::new());
    let t = call(&net, &alice, remove_inactive(net.two), 0);
    next(&mut net, Who::One, vec![t]);
    assert_eq!(receipts_status(&net), vec![Vec::<u8>::new()], "removeInactive below a full set reverts");
    assert_eq!(record(&net, net.two).unwrap(), idle, "two's record is unchanged");
    // two comes back: it wins in its own range at once, its shares are recorded, and its pots pay it at settlement.
    let reward = idle.reward;
    let before = balance(&net, reward);
    next(&mut net, Who::Two, vec![]);
    let won = height(&net);
    next(&mut net, Who::Two, vec![]);
    assert!(record(&net, net.two).unwrap().last_share >= won, "its shares are recorded");
    while height(&net) <= won + u64::from(cp.D) + 1 { next(&mut net, Who::Two, vec![]); }
    assert!(balance(&net, reward) > before, "settle({won}) pays two's reward address");
    assert_eq!(wallet::chain_rejoin_step(&net.chain, net.two), RejoinStep::Active);
    assert_eq!(net.chain.child_ctx(net.chain.head(), 0).unwrap().assignment.ranges().len(), 2, "both members split the full nonce domain");
}

/// FN-2b C4 (owner decisions 10b and Q2(a)): at a full set (M_max 2), past Theta_inact (4), the offline member two is
/// removable, and carol's registerReplacing takes its seat. two exits as by deregister: its wallet step is Full (no
/// reactivation while every seat is taken), and the sweep refunds its 20 PCOL bond to its reward address at
/// exit + U and erases the record. two rejoins by registering again with no bond into the seat of carol, idle past
/// Theta_inact itself: ACCRUING, with a nonce range at once, and its wins fill its bond to B_reg, HELD.
#[test]
fn v3_an_offline_member_replaced_at_a_full_set_rejoins_by_registering_again() {
    let mut net = v3_net(2, 4);
    let cp = net.chain.spec.cp().clone();
    let (alice, carol) = (devnet::account_key("alice"), devnet::account_key("carol"));
    let fund = tx(&net, &alice, net.two, U256::from(PCOL), Bytes::new(), 21_000);
    next(&mut net, Who::One, vec![fund]);
    while removable(&net).is_empty() { assert!(height(&net) < 20); next(&mut net, Who::One, vec![]); }
    assert_eq!(removable(&net), vec![net.two], "the full set: two, idle past Theta_inact, is removable; one mines");
    let reward = record(&net, net.two).unwrap().reward;
    let t = call(&net, &carol, wallet::replacing_calldata(carol.address(), net.two), B_REG);
    next(&mut net, Who::One, vec![t]);
    assert_eq!(receipts_status(&net), vec![vec![1u8]], "carol's registerReplacing");
    let h = height(&net);
    let out = record(&net, net.two).unwrap();
    assert_eq!((out.status, out.exit_height, out.due_height, out.tomb, out.bond), (2, h, h + u64::from(cp.U), false, U256::from(B_REG)), "two exits as by deregister");
    assert!(!member(&net, net.two) && !has_range(&net, net.two), "an exited record holds no seat");
    assert_eq!(wallet::chain_rejoin_step(&net.chain, net.two), RejoinStep::Full { refund: out.due_height }, "no reactivation while every seat is taken");
    while height(&net) + 1 < out.due_height { next(&mut net, Who::One, vec![]); }
    assert!(record(&net, net.two).is_some());
    // The block at exit + U (its pots, settled D behind, were drawn after the exit: none pays two as proposer).
    let before = balance(&net, reward);
    next(&mut net, Who::One, vec![]);
    assert_eq!(record(&net, net.two), None, "the sweep erases the record at exit + U");
    assert_eq!(balance(&net, reward) - before, U256::from(B_REG), "and refunds the bond to the reward address");
    assert_eq!(wallet::chain_rejoin_step(&net.chain, net.two), RejoinStep::Unavailable, "rejoining is a new registration");
    // carol never mined: past Theta_inact her seat is removable, and two registers into it with no bond (earn-in).
    while !removable(&net).contains(&carol.address()) { next(&mut net, Who::One, vec![]); }
    let two_key = Key::from_secret(&net.keys[&net.two].secret()).unwrap();
    let t = call(&net, &two_key, wallet::replacing_calldata(net.two, carol.address()), 0);
    next(&mut net, Who::One, vec![t]);
    assert_eq!(receipts_status(&net), vec![vec![1u8]], "two's registerReplacing");
    let back = record(&net, net.two).unwrap();
    assert_eq!((back.status, back.bond_state, back.bond), (1, apply::SystemPrograms::ACCRUING, U256::ZERO), "two returns ACCRUING");
    assert!(member(&net, net.two) && has_range(&net, net.two), "with a seat and a nonce range at once");
    assert_eq!(record(&net, carol.address()).unwrap().status, 2, "carol exits");
    let start = height(&net);
    while record(&net, net.two).unwrap().bond_state != 1 {
        assert!(height(&net) < start + 40, "two's wins fill its bond: {:?}", record(&net, net.two));
        next(&mut net, Who::Two, vec![]);
    }
    assert_eq!(record(&net, net.two).unwrap().bond, U256::from(B_REG), "HELD at B_reg, earned from its rewards");
    assert_eq!(wallet::chain_rejoin_step(&net.chain, net.two), RejoinStep::Active);
}
