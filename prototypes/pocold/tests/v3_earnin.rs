//! The v3 twin of tests/earnin.rs (V3-PR-E0): the same full-chain earn-in flow on a specVersion-2 devnet, which
//! selects the v3 programs (system-v3.json) whose probation pool is CP schema v2 P_MAX/T_PROB (V3-14). Real
//! blocks, real settlement through RewardEscrow, the escrow invariant after every block. A spec-2 genesis decodes
//! only in a v3-preview build, so the chain tests need `--features v3-preview`; a release build must refuse it.
//! V3-PR-E2b adds the evidence penalties on chain (V3-16, V3-17): real type-2 evidence, the reporter's cut, the
//! whole-pot burn of an ACCRUING winner, the E_win boundary and a self-report, against the spec-1 earn-in chain.
use pocol_lp4_execution::devnet::{self, DevnetConfig};

#[cfg_attr(not(feature = "v3-preview"), allow(dead_code))]
const PCOL: u128 = 1_000_000_000_000_000_000;

fn config(p_max: u16, t_prob: u32) -> DevnetConfig {
    let mut cfg = DevnetConfig::new(777941, 1_700_000_000, &["one", "two"]);
    cfg.hash_rate = 16;
    cfg.c = 8;
    cfg.d_att = 73;
    cfg.r_max = 1;
    cfg.earnin = true;
    cfg.spec_version = 2;
    cfg.p_max = p_max;
    cfg.t_prob = t_prob;
    cfg
}

#[test]
fn a_v3_earnin_genesis_is_dormant_outside_a_preview_build() {
    let built = devnet::build(&config(64, 86_400));
    if pocol_lp4_execution::V3_PREVIEW {
        let net = built.unwrap();
        assert_eq!(net.programs.sys_code_hash(), pocol_lp4_execution::apply::SystemPrograms::v3().sys_code_hash(), "spec 2 + earn-in selects the v3 set");
    } else {
        assert!(built.err().unwrap().contains("gsVersion"), "a release build cannot decode a specVersion-2 genesis, so the v3 set is unreachable");
    }
    // The spec-1 earn-in devnet keeps the earn-in set and its constants.
    let mut legacy = config(64, 86_400);
    legacy.spec_version = 1;
    let net = devnet::build(&legacy).unwrap();
    assert_eq!(net.programs.sys_code_hash(), pocol_lp4_execution::apply::SystemPrograms::earnin().sys_code_hash());
}

#[cfg(feature = "v3-preview")]
mod preview {
    use super::*;
    use std::collections::HashMap;
    use pocol_lp4_execution::{admission, apply::{self, Probation, ProgramSet}, txsign::{Key, Tx}};
    use pocold::{chain::{Chain, Spec}, hashes, rlp, types::*, wallet::{self, RejoinStep}};
    use revm::primitives::{keccak256, Address, Bytes, B256, U256};

    const B: u128 = 20 * PCOL;

    struct Net { chain: Chain, keys: HashMap<Address, Key>, one: Address }

    fn net(p_max: u16, t_prob: u32) -> Net {
        let mut candidate = devnet::build(&config(p_max, t_prob)).unwrap();
        candidate.forks.push((2, 2));
        let mut keys: HashMap<Address, Key> = candidate.miners.iter()
            .map(|m| (m.id, Key::from_secret(&m.key.secret()).unwrap())).collect();
        for label in ["alice", "bob"] { let k = devnet::account_key(label); keys.insert(k.address(), k); }
        let one = candidate.miners[0].id;
        let timing = admission::measure(&candidate.spec_bytes, &candidate.alloc, 1).unwrap();
        let admitted = admission::admit(&candidate.request(&pocold::single_host_local(), Some(&timing))).unwrap();
        let chain = Chain::new(Spec::new(admitted));
        assert_eq!(chain.spec.programs(), ProgramSet::V3);
        assert!(chain.spec.earnin_rules() && chain.spec.v3_active(0));
        assert_eq!(chain.spec.probation(), Some(Probation { seats: u64::from(p_max), blocks: u64::from(t_prob) }), "the CP values, not the earn-in constants");
        Net { chain, keys, one }
    }

    /// The next block, won (and shared) entirely in `miner`'s nonce range.
    fn next(net: &mut Net, miner: Address, txs: Vec<Vec<u8>>) -> Address {
        let parent = net.chain.full_entry(&net.chain.head).unwrap();
        let context = net.chain.child_ctx(&parent, 0).unwrap();
        let range = context.assignment.ranges().iter().find(|r| r.id() == raw_addr(miner)).unwrap_or_else(|| panic!("miner has no range at {}", parent.height + 1));
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
    fn top_up(net: &Net, key: &Key, value: u128) -> Vec<u8> { tx(net, key, keccak256("topUp()")[..4].to_vec(), U256::from(value)) }
    fn record(net: &Net, id: Address) -> apply::MinerRecord { apply::miner_record(&net.chain.head().state, id).expect("record") }
    fn accruing_count(net: &Net) -> U256 {
        net.chain.head().state.accounts[&apply::program_address(0)].storage.get(&U256::from(8)).copied().unwrap_or_default()
    }
    fn balance(net: &Net, a: Address) -> U256 { net.chain.head().state.accounts.get(&a).map(|x| x.info.balance).unwrap_or_default() }
    fn nonce(net: &Net, a: Address) -> u64 { net.chain.head().state.accounts.get(&a).map(|x| x.info.nonce).unwrap_or(0) }

    /// tests/earnin.rs's scenario on the v3 set at the O-14=b values P_MAX 64 / T_PROB 86,400.
    #[test]
    fn a_zero_bond_miner_earns_its_bond_then_is_paid_and_top_up_promotes_at_once_on_the_v3_set() {
        let mut net = net(64, 86_400);
        let one = net.one;
        assert_eq!(record(&net, one).bond, U256::from(B), "genesis bonds are 20 PCOL");
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
            let before = record(&net, a);
            let proposer = next(&mut net, a, vec![]);
            // D1 as amended by V3-13 (v3_active): an ACCRUING member below B_reg/4 is never the proposer.
            if before.bond_state == 4 && before.bond < U256::from(B / 4) { assert_ne!(proposer, a, "an ACCRUING member below B/4 was drawn as proposer at {}", net.chain.head().height); }
            let r = record(&net, a);
            assert!(r.bond >= last && r.bond <= U256::from(B), "the bond only grows up to B");
            last = r.bond;
            if r.bond_state == 4 { assert_eq!(balance(&net, a), funded, "nothing is paid out while accruing"); }
        }
        let promoted = record(&net, a);
        assert_eq!((promoted.bond_state, promoted.bond), (1, U256::from(B)), "promoted to HELD at exactly B");
        assert_eq!(accruing_count(&net), U256::ZERO);
        let before = balance(&net, a);
        for _ in 0..6 { let _ = next(&mut net, a, vec![]); }
        assert!(balance(&net, a) > before, "a HELD member is paid normally");

        // Top-up: bob joins with 5 PCOL, tops up part, then sends more than the remainder: the bond fills to B and
        // the rest returns to bob.
        let bob = devnet::account_key("bob");
        let t = register(&net, &bob, 5 * PCOL);
        let _ = next(&mut net, a, vec![t]);
        assert_eq!((record(&net, bob.address()).bond_state, record(&net, bob.address()).bond), (4, U256::from(5 * PCOL)));
        // The wallet's probation deadline (status/top-up quote) is registeredAt + CP T_PROB.
        assert_eq!(net.chain.spec.probation().unwrap().blocks, 86_400, "the CP T_PROB, not the earn-in 60,480");
        let part = top_up(&net, &bob, 4 * PCOL);
        let _ = next(&mut net, a, vec![part]);
        assert_eq!((record(&net, bob.address()).bond_state, record(&net, bob.address()).bond), (4, U256::from(9 * PCOL)), "a partial top-up stays on probation");
        let before = balance(&net, bob.address());
        let over = top_up(&net, &bob, 16 * PCOL);
        let _ = next(&mut net, a, vec![over]);
        let r = record(&net, bob.address());
        assert_eq!((r.bond_state, r.bond), (1, U256::from(B)), "promoted in the top-up's block");
        let spent = before - balance(&net, bob.address());
        assert!(spent > U256::from(11 * PCOL) && spent < U256::from(11 * PCOL + PCOL / 100), "only the 11 PCOL remainder (plus the fee) left bob: {spent}");
        assert_eq!(accruing_count(&net), U256::ZERO);
    }

    /// The pool size and deadline the wallet quotes from are the ones the v3 programs enforce on chain: P_MAX 1
    /// admits one probation seat, and T_PROB 12 ends probation 12 blocks after registeredAt (top-up refused,
    /// the record swept out, the wallet's rejoin step `Missed`).
    #[test]
    fn the_cp_probation_pool_is_enforced_on_chain_and_matches_the_wallet() {
        let (alice, bob) = (devnet::account_key("alice"), devnet::account_key("bob"));
        // Control: the same blocks on a chain that differs only in P_MAX 2 admit bob's probation seat, so P_MAX is
        // what refuses it below (not RegOpsLimit, the record caps or the reward probe, which both chains share).
        let mut control = net(2, 12);
        let miner = control.one;
        for (key, value) in [(&alice, 0), (&bob, PCOL)] { let t = register(&control, key, value); let _ = next(&mut control, miner, vec![t]); }
        assert_eq!((record(&control, bob.address()).bond_state, accruing_count(&control)), (4, U256::from(2)), "P_MAX 2 admits the second probation seat");

        let mut net = net(1, 12);
        let one = net.one;
        let t = register(&net, &alice, 0);
        let _ = next(&mut net, one, vec![t]);
        assert_eq!(record(&net, alice.address()).bond_state, 4);
        let t = register(&net, &bob, PCOL);
        let _ = next(&mut net, one, vec![t]);
        let entry = net.chain.full_entry(&net.chain.head).unwrap();
        assert_eq!(entry.receipts.len(), 1);
        let receipt = rlp::decode(&entry.receipts[0][1..]).unwrap();
        assert_eq!(receipt.list().unwrap()[0].bytes().unwrap(), &[] as &[u8], "the refused registration is a status-0 receipt");
        assert!(apply::miner_record(&net.chain.head().state, bob.address()).is_none(), "the second probation seat is refused at P_MAX 1 (Capacity)");
        assert_eq!(accruing_count(&net), U256::from(1));
        let t = register(&net, &bob, B);
        let _ = next(&mut net, one, vec![t]);
        assert_eq!(record(&net, bob.address()).bond_state, 1, "the full bond still registers");

        let r = record(&net, alice.address());
        let deadline = r.registered_at + net.chain.spec.probation().unwrap().blocks;
        assert_eq!(net.chain.spec.probation().unwrap().blocks, 12);
        while net.chain.head().height + 2 < deadline { let _ = next(&mut net, one, vec![]); }
        // A top-up included at deadline - 1 is accepted.
        let t = top_up(&net, &alice, PCOL);
        let _ = next(&mut net, one, vec![t]);
        assert_eq!(net.chain.head().height, deadline - 1);
        assert_eq!(record(&net, alice.address()).bond, U256::from(PCOL), "inside probation");
        // At the deadline markClose sweeps the record out before the transaction runs, so a top-up there fails.
        let t = top_up(&net, &alice, PCOL);
        let _ = next(&mut net, one, vec![t]);
        assert_eq!(net.chain.head().height, deadline);
        let r = record(&net, alice.address());
        assert_eq!((r.status, r.bond_state, r.bond), (2, 4, U256::from(PCOL)), "exited at registeredAt + T_PROB, the top-up refused");
        let head = net.chain.head();
        let member = net.chain.membership(head, head.height + 1).unwrap().ids().contains(&raw_addr(alice.address()));
        let step = wallet::rejoin_step(Some(&r), head.height + 1, member, u64::from(net.chain.spec.cp().W_act), net.chain.spec.probation().map(|p| p.blocks));
        assert_eq!(step, RejoinStep::Missed, "the wallet offers no reactivation past the CP deadline");
        assert_eq!(accruing_count(&net), U256::from(1), "an exited probation record keeps its seat until the refund");
    }

    /// V3-08 on chain (V3-PR-E1): one identity's second maintenance operation in a block is a status-0 receipt, while
    /// another identity's registration in that block lands; the next block accepts the maintenance operation.
    #[test]
    fn one_maintenance_operation_per_identity_per_block_on_chain() {
        let mut net = net(64, 86_400);
        let one = net.one;
        let (alice, bob) = (devnet::account_key("alice"), devnet::account_key("bob"));
        let t = register(&net, &alice, B);
        let _ = next(&mut net, one, vec![t]);
        let reward = |net: &Net, key: &Key, to: Address, offset: u64| {
            let mut data = keccak256("setRewardAddress(address)")[..4].to_vec();
            data.extend_from_slice(to.into_word().as_slice());
            let raw = tx(net, key, data, U256::ZERO);
            if offset == 0 { return raw; }
            // The same transaction at the sender's next nonce (two transactions of one sender in one block).
            let head = net.chain.full_entry(&net.chain.head).unwrap();
            let base = net.chain.child_ctx(&head, 0).unwrap().base_fee;
            let priority = U256::from(1_000_000_000u64);
            let mut data = keccak256("setRewardAddress(address)")[..4].to_vec();
            data.extend_from_slice(to.into_word().as_slice());
            Tx { chain_id: net.chain.spec.chain_id(), nonce: nonce(net, key.address()) + offset, max_priority: priority, max_fee: base * U256::from(2) + priority,
                 gas: 1_000_000, to: Some(apply::program_address(0)), value: U256::ZERO, data: Bytes::from(data) }.sign_1559(key)
        };
        let txs = vec![reward(&net, &alice, alice.address(), 0), reward(&net, &alice, bob.address(), 1), register(&net, &bob, B)];
        let _ = next(&mut net, one, txs);
        let entry = net.chain.full_entry(&net.chain.head).unwrap();
        let status: Vec<Vec<u8>> = entry.receipts.iter().map(|r| rlp::decode(&r[1..]).unwrap().list().unwrap()[0].bytes().unwrap().to_vec()).collect();
        assert_eq!(status, vec![vec![1u8], vec![], vec![1u8]], "maintenance, the same identity's second maintenance (refused), another identity's join");
        assert_eq!(record(&net, alice.address()).reward, alice.address(), "the refused call changed nothing");
        assert_eq!(record(&net, bob.address()).status, 1);
        let t = reward(&net, &alice, bob.address(), 0);
        let _ = next(&mut net, one, vec![t]);
        assert_eq!(record(&net, alice.address()).reward, bob.address(), "accepted in the next block");
    }

    /// V3-09 ParamGate (lp1-node gate.rs R8 on CP schema v2): admission refuses a v3 devnet whose reactivation window
    /// before the refund sweep is shorter than two blocks or whose C_react is below U, and admits the boundary.
    #[test]
    fn admission_requires_the_v3_reactivation_window_and_cooldown() {
        for (w_act, u, c_react, ok) in [(32, 34, 34, true), (32, 33, 40, false), (32, 40, 39, false), (32, 40, 40, true)] {
            let mut cfg = config(64, 86_400);
            cfg.w_act = w_act; cfg.u = u; cfg.c_react = c_react;
            let candidate = devnet::build(&cfg).unwrap();
            let timing = admission::measure(&candidate.spec_bytes, &candidate.alloc, 1).unwrap();
            match admission::admit(&candidate.request(&pocold::single_host_local(), Some(&timing))) {
                Ok(_) => assert!(ok, "W_act {w_act} U {u} C_react {c_react} admitted"),
                Err(e) => { assert!(!ok, "W_act {w_act} U {u} C_react {c_react}: {e}"); assert!(e.to_string().contains("R8"), "{e}"); }
            }
        }
    }

    // ---------------- V3-PR-E2b: evidence penalties on chain (V3-16, V3-17)

    /// D = 8, so that the evidence block (drawn deterministically) lands before settle(j).
    fn net_spec(spec: u8) -> Net {
        let mut cfg = config(64, 86_400);
        cfg.spec_version = spec;
        cfg.d = 8;
        let mut candidate = devnet::build(&cfg).unwrap();
        candidate.forks.push((2, 2));
        let mut keys: HashMap<Address, Key> = candidate.miners.iter().map(|m| (m.id, Key::from_secret(&m.key.secret()).unwrap())).collect();
        for label in ["alice", "bob"] { let k = devnet::account_key(label); keys.insert(k.address(), k); }
        let one = candidate.miners[0].id;
        let timing = admission::measure(&candidate.spec_bytes, &candidate.alloc, 1).unwrap();
        let admitted = admission::admit(&candidate.request(&pocold::single_host_local(), Some(&timing))).unwrap();
        let chain = Chain::new(Spec::new(admitted));
        assert_eq!(chain.spec.programs(), if spec == 2 { ProgramSet::V3 } else { ProgramSet::EarnIn });
        Net { chain, keys, one }
    }

    /// The next block with this body, won by `winner` in its own range, its header shares one nonce of each of
    /// `owners`' ranges. Evidence in the body is evaluated as validation does. Returns (height, drawn proposer).
    fn mine(net: &mut Net, winner: Address, owners: &[Address], body: Body) -> (u64, Address) {
        let parent = net.chain.full_entry(&net.chain.head).unwrap();
        let context = net.chain.child_ctx(&parent, 0).unwrap();
        let effects = net.chain.evidence_effects(&parent, &body, parent.height + 1).unwrap();
        let nmax = context.assignment.nonce_max();
        let range = |who: Address| context.assignment.ranges().iter().find(|r| r.id() == raw_addr(who)).cloned().unwrap_or_else(|| panic!("no range at {}", parent.height + 1));
        'templates: for timestamp in parent.ts + u64::from(net.chain.spec.cp().T_blk)..=context.window.upper() {
            let (mut header, _, _) = net.chain.build_signed(&parent, &net.keys[&context.proposer], timestamp, body.clone(), effects.clone()).unwrap();
            let tid = template_id(&header.ut_raw);
            let mut shares = Vec::new();
            for o in owners {
                let r = range(*o);
                match (r.start()..r.end_exclusive().min(nmax)).find(|n| le_target(&pow_hash(&tid, *n as u64), &context.t_share)) {
                    Some(n) => shares.push(n as u64), None => continue 'templates,
                }
            }
            shares.sort_unstable(); shares.dedup();
            let r = range(winner);
            let found = (r.start()..r.end_exclusive().min(nmax)).find(|n| !shares.contains(&(*n as u64)) && le_target(&pow_hash(&tid, *n as u64), &context.target));
            let Some(nonce) = found else { continue };
            header.nonce = nonce as u64; header.shares = shares; reseal(&mut header);
            header.winner_sig = net.keys[&winner].sign_hash(&B256::from(hashes::win_msg(&tid, nonce as u64, &share_root(&header))));
            reseal(&mut header);
            let (entry, _) = net.chain.validate(&Block { header, body }, None, 0).unwrap();
            assert!(net.chain.insert(entry));
            let h = net.chain.head();
            apply::check_escrow_invariant(&h.state, net.chain.spec.profile(), h.height, false).unwrap();
            return (h.height, context.proposer);
        }
        panic!("no solution for deterministic test fixture");
    }

    /// Type-2 evidence against the winner of the canonical block at `height`: its signed winner copy and a second copy of
    /// the same template and nonce with no header shares (a different share root), signed by the same winner.
    fn equivocation(net: &Net, height: u64) -> Vec<u8> {
        let hash = *net.chain.canonical.get(height as usize).unwrap();
        let e = net.chain.get(&hash).unwrap();
        let (h, winner) = (e.header.clone(), e.result.clone().unwrap().winner);
        let tid = template_id(&h.ut_raw);
        let mut alt = h.clone(); alt.shares = vec![]; reseal(&mut alt);
        let (root1, root2) = (share_root(&h), share_root(&alt));
        assert_ne!(root1, root2, "the block has header shares");
        let sig2 = net.keys[&winner].sign_hash(&B256::from(hashes::win_msg(&tid, h.nonce, &root2)));
        let mut payload = Vec::new();
        rlp::encode_bytes(&[2], &mut payload); rlp::encode_bytes(&tid, &mut payload); rlp::encode_bytes(&h.nonce.to_be_bytes(), &mut payload);
        rlp::encode_bytes(&root1, &mut payload); rlp::encode_bytes(&h.winner_sig, &mut payload); rlp::encode_bytes(&root2, &mut payload); rlp::encode_bytes(&sig2, &mut payload);
        let mut out = Vec::new(); rlp::encode_list_payload(&payload, &mut out); out
    }
    fn pot_words(net: &Net, j: u64) -> (U256, U256, U256) {
        let b = U256::from_be_bytes(keccak256([U256::from(j).to_be_bytes::<32>(), U256::from(3).to_be_bytes::<32>()].concat()).0);
        let s = &net.chain.head().state.accounts[&apply::program_address(1)].storage;
        let w = |k: U256| s.get(&k).copied().unwrap_or_default();
        (w(b), w(b + U256::from(1)), w(b + U256::from(2)))
    }

    /// V3-16/V3-17 on a real chain (fails on the original): block j is won by `two` with a share of `one`; block k by
    /// alice, a zero-bond probationer. One block, proposed by `one`, carries type-2 evidence against both before settle(j).
    /// On the v3 chain pot j settles normally (one's share is paid in settle(j), two's winner part burns), pot k burns
    /// whole (alice was ACCRUING when its result was recorded), and one's reward address receives 25% of two's 20 PCOL
    /// bond. On the spec-1 earn-in chain both pots burn whole and nobody is paid for reporting.
    #[test]
    fn type2_evidence_settles_the_pot_and_pays_the_reporter_on_the_v3_chain() {
        let mut out = Vec::new();
        for spec in [1u8, 2] {
            let mut net = net_spec(spec);
            let one = net.one;
            let two = net.chain.spec.profile().spec().m0_list.iter().map(|m| Address::from(m.id)).find(|m| *m != one).unwrap();
            let alice = devnet::account_key("alice");
            let t = register(&net, &alice, 0);
            mine(&mut net, one, &[one], Body { txs: vec![t], evidence: vec![] });
            let (j, prop_j) = mine(&mut net, two, &[one, two], Body::default());
            let (k, _) = mine(&mut net, alice.address(), &[alice.address()], Body::default());
            let d = u64::from(net.chain.spec.cp().D);
            // The evidence block's drawn proposer must not be `two` (it is the reporter); alice is not drawn below B/4.
            loop {
                let p = net.chain.full_entry(&net.chain.head).unwrap();
                if net.chain.child_ctx(&p, 0).unwrap().proposer == one { break }
                mine(&mut net, one, &[one], Body::default());
            }
            let evidence = vec![equivocation(&net, j), equivocation(&net, k)];
            let reward = record(&net, one).reward;
            let before = balance(&net, reward);
            let (e, reporter) = mine(&mut net, one, &[one], Body { txs: vec![], evidence });
            assert_eq!(reporter, one);
            assert!(e < j + d, "the evidence precedes settle(j)'s block (block {e}, j {j})");
            let cut = balance(&net, reward) - before;
            let (pj, pk) = (pot_words(&net, j), pot_words(&net, k));
            assert!(record(&net, two).tomb && record(&net, alice.address()).tomb);
            // settle(j): one's payment from pot j alone (the block at j + D settles nothing else; its own cut if the
            // evidence landed there).
            while net.chain.head().height + 1 < j + d { mine(&mut net, one, &[one], Body::default()); }
            let before = balance(&net, reward);
            mine(&mut net, one, &[one], Body::default());
            assert_eq!(net.chain.head().height, j + d);
            let paid_j = balance(&net, reward) - before;
            out.push((spec, pj.1 >> 168u32, pk.1 >> 168u32, pk.2 >> 160u32, cut, paid_j, prop_j == one));
        }
        eprintln!("{out:?}");
        let (earnin, v3) = (&out[0], &out[1]);
        assert_eq!((earnin.1, earnin.2, earnin.4, earnin.5), (U256::from(1), U256::from(1), U256::ZERO, U256::ZERO), "the earn-in chain burns both pots whole, no reporter");
        assert_eq!((v3.1, v3.2, v3.3), (U256::ZERO, U256::from(1), U256::from(1)), "v3: pot j normal, pot k whole-burn with its ACCRUING winner bit");
        assert_eq!(v3.4, U256::from(5 * PCOL), "25% of two's 20 PCOL bond to the reporter one");
        // Pot j: 2 PCOL, header shares of one and two plus two's winner share; R = 1.7 PCOL over Q = max(3, 64).
        let share = U256::from(17 * PCOL / 10) / U256::from(64);
        assert_eq!(v3.5, share + if v3.6 { U256::from(PCOL / 10) } else { U256::ZERO }, "one's share of pot j (and its proposer part if it proposed j)");
    }

    /// The evidence window on the v3 chain, with a self-report: an equivocation by `one` (the only member left to propose
    /// after the others are punished) is valid evidence exactly up to E_win blocks later, refused one block later; `one`
    /// includes it against itself at the boundary and loses (1 - rho) * B: its bond burns, its reward address receives
    /// 25% of it, and the pot settled in that block pays the tomb nothing.
    #[test]
    fn evidence_window_boundary_and_self_report_on_the_v3_chain() {
        let mut net = net_spec(2);
        let one = net.one;
        let two = net.chain.spec.profile().spec().m0_list.iter().map(|m| Address::from(m.id)).find(|m| *m != one).unwrap();
        // two equivocates first and is punished, so that one is the only proposer from then on.
        let (j2, _) = mine(&mut net, two, &[one, two], Body::default());
        loop {
            let p = net.chain.full_entry(&net.chain.head).unwrap();
            if net.chain.child_ctx(&p, 0).unwrap().proposer == one { break }
            mine(&mut net, one, &[one], Body::default());
        }
        let ev = equivocation(&net, j2);
        mine(&mut net, one, &[one], Body { txs: vec![], evidence: vec![ev] });
        let (jj, _) = mine(&mut net, one, &[one], Body::default());
        let ev = equivocation(&net, jj);
        let e_win = u64::from(net.chain.spec.cp().E_win);
        while net.chain.head().height + 1 < jj + e_win { mine(&mut net, one, &[one], Body::default()); }
        let p = net.chain.full_entry(&net.chain.head).unwrap();
        let body = Body { txs: vec![], evidence: vec![ev.clone()] };
        assert!(net.chain.evidence_effects(&p, &body, p.height + 2).is_err(), "jj + E_win + 1 is outside the window");
        assert_eq!(net.chain.evidence_effects(&p, &body, p.height + 1).unwrap(), vec![apply::EvidenceEffect::PunishAndInvalidate { accused: one, j: jj }], "exactly jj + E_win");
        let reward = record(&net, one).reward;
        let (held, before) = (apply::chain_facts(&net.chain.head().state).held_total, balance(&net, reward));
        let (h, reporter) = mine(&mut net, one, &[one], body);
        assert_eq!((h, reporter), (jj + e_win, one), "a self-report at the boundary");
        let r = record(&net, one);
        assert!(r.tomb && r.bond_state == 2);
        assert_eq!(held - apply::chain_facts(&net.chain.head().state).held_total, U256::from(B), "the 20 PCOL bond left the held total");
        assert_eq!(balance(&net, reward) - before, U256::from(B / 4), "rho = 25% back: the self-report loses 15 PCOL");
    }
}
