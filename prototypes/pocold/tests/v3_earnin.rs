//! The v3 twin of tests/earnin.rs (V3-PR-E0): the same full-chain earn-in flow on a specVersion-2 devnet, which
//! selects the v3 programs (system-v3.json), without a probation pool or deadline since FN-2b (owner decisions 10b/10c of
//! 2026-11-13: ACCRUING records count within M_max and never exit by time). Real
//! blocks, real settlement through RewardEscrow, the escrow invariant after every block. A spec-2 genesis is the release
//! GenesisSpec (SPEC_VERSION 2), so these are default tests.
//! V3-PR-E2b adds the evidence penalties on chain (V3-16, V3-17): real type-2 evidence, the reporter's cut, the
//! whole-pot burn of an ACCRUING winner, the E_win boundary and a self-report, against the spec-1 earn-in chain. FN-2b C4
//! adds the removal at a full set on chain: registerReplacing through the wallet's quote decision (a removable member
//! chosen at random, confirmed by the registry's removalStatus view) and the node's pocol_getRemovable listing.
use pocol_lp4_execution::devnet::{self, DevnetConfig};

const PCOL: u128 = 1_000_000_000_000_000_000;

fn config() -> DevnetConfig {
    let mut cfg = DevnetConfig::new(777941, 1_700_000_000, &["one", "two"]);
    cfg.hash_rate = 16;
    cfg.c = 8;
    cfg.d_att = 73;
    cfg.r_max = 1;
    cfg.earnin = true;
    cfg.spec_version = 2;
    cfg
}

#[test]
fn a_v3_earnin_genesis_selects_the_v3_program_set() {
    let net = devnet::build(&config()).unwrap();
    assert_eq!(net.programs.sys_code_hash(), pocol_lp4_execution::apply::SystemPrograms::v3().sys_code_hash(), "spec 2 + earn-in selects the v3 set");
    // The spec-1 earn-in devnet keeps the earn-in set and its constants.
    let mut legacy = config();
    legacy.spec_version = 1;
    let net = devnet::build(&legacy).unwrap();
    assert_eq!(net.programs.sys_code_hash(), pocol_lp4_execution::apply::SystemPrograms::earnin().sys_code_hash());
}

mod on_chain {
    use super::*;
    use std::collections::HashMap;
    use pocol_lp4_execution::{admission, apply::{self, ProgramSet}, txsign::{Key, Tx}};
    use pocold::{chain::{Chain, Spec}, hashes, rlp, types::*, wallet::{self, RejoinStep}};
    use revm::primitives::{keccak256, Address, Bytes, B256, U256};

    const B: u128 = 20 * PCOL;

    struct Net { chain: Chain, keys: HashMap<Address, Key>, one: Address }

    fn net() -> Net {
        let net = net_with(config());
        assert_eq!(net.chain.spec.probation(), None, "FN-2b: no probation pool or deadline on the v3 set");
        net
    }
    fn net_with(cfg: DevnetConfig) -> Net {
        let mut candidate = devnet::build(&cfg).unwrap();
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

    /// tests/earnin.rs's scenario on the v3 set (no accruing count: slot 8 stays zero).
    #[test]
    fn a_zero_bond_miner_earns_its_bond_then_is_paid_and_top_up_promotes_at_once_on_the_v3_set() {
        let mut net = net();
        let one = net.one;
        assert_eq!(record(&net, one).bond, U256::from(B), "genesis bonds are 20 PCOL");
        let alice = devnet::account_key("alice");
        let a = alice.address();
        assert_eq!(nonce(&net, a), 0, "the registration is alice's first transaction (R9)");
        let t = register(&net, &alice, 0);
        let _ = next(&mut net, one, vec![t]);
        let r = record(&net, a);
        assert_eq!((r.status, r.bond_state, r.bond, r.reward), (1, 4, U256::ZERO, a), "an ACCRUING record");
        assert_eq!(accruing_count(&net), U256::ZERO, "FN-2b: no accruing count");
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
        let before = balance(&net, a);
        for _ in 0..6 { let _ = next(&mut net, a, vec![]); }
        assert!(balance(&net, a) > before, "a HELD member is paid normally");

        // Top-up: bob joins with 5 PCOL, tops up part, then sends more than the remainder: the bond fills to B and
        // the rest returns to bob.
        let bob = devnet::account_key("bob");
        let t = register(&net, &bob, 5 * PCOL);
        let _ = next(&mut net, a, vec![t]);
        assert_eq!((record(&net, bob.address()).bond_state, record(&net, bob.address()).bond), (4, U256::from(5 * PCOL)));
        let part = top_up(&net, &bob, 4 * PCOL);
        let _ = next(&mut net, a, vec![part]);
        assert_eq!((record(&net, bob.address()).bond_state, record(&net, bob.address()).bond), (4, U256::from(9 * PCOL)), "a partial top-up stays ACCRUING");
        let before = balance(&net, bob.address());
        let over = top_up(&net, &bob, 16 * PCOL);
        let _ = next(&mut net, a, vec![over]);
        let r = record(&net, bob.address());
        assert_eq!((r.bond_state, r.bond), (1, U256::from(B)), "promoted in the top-up's block");
        let spent = before - balance(&net, bob.address());
        assert!(spent > U256::from(11 * PCOL) && spent < U256::from(11 * PCOL + PCOL / 100), "only the 11 PCOL remainder (plus the fee) left bob: {spent}");
        assert_eq!(accruing_count(&net), U256::ZERO);
    }

    /// FN-2b C2/C3 on chain: ACCRUING newcomers have no pool of their own and no deadline. At M_max 4 (two genesis members)
    /// alice (zero bond) and bob (1 PCOL) both register ACCRUING in consecutive blocks, carol's zero-bond registration is
    /// refused by M_max (a status-0 receipt), and 70 blocks on (there is no deadline to reach) alice's top-up lands and she
    /// is still an active ACCRUING record; the wallet's rejoin step has no deadline either (Spec::probation None).
    #[test]
    fn accruing_members_count_within_m_max_and_never_exit_by_time() {
        let mut cfg = config(); cfg.m_max = 4; cfg.accounts.push(("carol".into(), U256::from(10u128.pow(24))));
        let mut net = net_with(cfg);
        let one = net.one;
        let (alice, bob, carol) = (devnet::account_key("alice"), devnet::account_key("bob"), devnet::account_key("carol"));
        for (key, value) in [(&alice, 0), (&bob, PCOL)] { let t = register(&net, key, value); let _ = next(&mut net, one, vec![t]); }
        assert_eq!((record(&net, alice.address()).bond_state, record(&net, bob.address()).bond_state), (4, 4), "two ACCRUING records, no pool");
        let t = register(&net, &carol, 0);
        let _ = next(&mut net, one, vec![t]);
        assert_eq!(receipts_status(&net), vec![Vec::<u8>::new()], "the set is full: M_max, not a pool, refuses carol");
        assert!(apply::miner_record(&net.chain.head().state, carol.address()).is_none());
        let registered = record(&net, alice.address()).registered_at;
        while net.chain.head().height < registered + 70 { let _ = next(&mut net, one, vec![]); }
        let t = top_up(&net, &alice, PCOL);
        let _ = next(&mut net, one, vec![t]);
        let r = record(&net, alice.address());
        assert_eq!((r.status, r.bond_state, r.bond), (1, 4, U256::from(PCOL)), "no deadline: still active and ACCRUING, the top-up accepted");
        let head = net.chain.head();
        let member = net.chain.membership(head, head.height + 1).unwrap().ids().contains(&raw_addr(alice.address()));
        let step = wallet::rejoin_step(Some(&r), head.height + 1, member, u64::from(net.chain.spec.cp().W_act), net.chain.spec.probation().map(|p| p.blocks));
        assert!(matches!(step, RejoinStep::Exit | RejoinStep::Active), "no deadline in the wallet's rejoin step: {step:?}");
        assert_eq!(accruing_count(&net), U256::ZERO);
    }

    // ---------------- FN-2b C4: removal at a full set on chain (owner decisions 10b and Q2(a) of 2026-11-13)
    /// A replacement network: M_max 3 (the two genesis members and one more), Theta_inact 4.
    fn replace_net() -> Net {
        let mut cfg = config();
        cfg.m_max = 3; cfg.theta_inact = 4;
        cfg.accounts.push(("carol".into(), U256::from(10u128.pow(24))));
        net_with(cfg)
    }
    fn replacing_tx(net: &Net, key: &Key, victim: Address, value: u128) -> Vec<u8> {
        tx(net, key, wallet::replacing_calldata(key.address(), victim).to_vec(), U256::from(value))
    }
    /// `key`'s transaction at its next nonce plus `offset` (several transactions of one sender in one block).
    fn tx_at(net: &Net, key: &Key, data: Vec<u8>, value: U256, offset: u64) -> Vec<u8> {
        let head = net.chain.full_entry(&net.chain.head).unwrap();
        let base = net.chain.child_ctx(&head, 0).unwrap().base_fee;
        let priority = U256::from(1_000_000_000u64);
        Tx { chain_id: net.chain.spec.chain_id(), nonce: nonce(net, key.address()) + offset, max_priority: priority, max_fee: base * U256::from(2) + priority,
             gas: 1_000_000, to: Some(apply::program_address(0)), value, data: Bytes::from(data) }.sign_1559(key)
    }
    fn remove_tx(net: &Net, key: &Key, victim: Address) -> Vec<u8> {
        let mut d = keccak256("removeInactive(address)")[..4].to_vec(); d.extend_from_slice(victim.into_word().as_slice());
        tx(net, key, d, U256::ZERO)
    }
    fn receipts_status(net: &Net) -> Vec<Vec<u8>> {
        let entry = net.chain.full_entry(&net.chain.head).unwrap();
        entry.receipts.iter().map(|r| rlp::decode(&r[1..]).unwrap().list().unwrap()[0].bytes().unwrap().to_vec()).collect()
    }
    /// The members removable at the next block, as the wallet and pocol_getRemovable list them.
    fn removable(net: &Net) -> Vec<Address> {
        let head = net.chain.head();
        wallet::removable_members(&net.chain, &head.state, head.height + 1, &wallet::head_credited(&net.chain))
    }
    /// The registry's removalStatus view on the head state at the next block (what the wallet's quote asks).
    fn removal_code(net: &Net, victim: Address) -> u64 {
        let head = net.chain.head();
        wallet::removal_view_on(&head.state, net.chain.spec.profile(), head.height + 1, head.ts + 1, Address::repeat_byte(9), victim).unwrap()
    }
    /// The wallet's listing agrees with the registry's own rule for every active member (and a stranger): removable iff
    /// removalStatus is 0 and the next block's phase 0 does not credit it (an owner of the head's template);
    /// pocol_getRemovable serves the same list.
    fn listing_matches_the_registry(net: &Net) {
        let head = net.chain.head();
        let list = removable(net);
        let credited = wallet::head_credited(&net.chain);
        for id in apply::active_list(&head.state).into_iter().chain([Address::repeat_byte(7)]) {
            let theta_one = net.chain.spec.cp().Theta_inact <= 1;
            assert_eq!(list.contains(&id), removal_code(net, id) == 0 && (theta_one || !credited.contains(&id)), "{id:#x} at {}", head.height + 1);
        }
        let j = wallet::removable_json(&net.chain).unwrap();
        assert_eq!(j["removable"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect::<Vec<_>>(), list.iter().map(|a| format!("{a:#x}")).collect::<Vec<_>>());
        assert_eq!(j["height"].as_u64(), Some(head.height + 1));
    }

    /// FN-2b C4 on chain: genesis member two never mines while one mines every block. Below a full set (two of three
    /// seats) nobody is removable and removeInactive reverts. alice fills the set; once two and alice are idle for
    /// Theta_inact the wallet lists both (never one, which mines), and its quote decision picks either of them,
    /// confirmed by the registry's view. In one block bob's registerReplacing naming one is a status-0 receipt and carol's
    /// naming alice lands: alice exits as by deregister (refund after U, no burn), carol takes the seat, and register alone
    /// is still refused at the full set. alice's refund comes at her due height; she registers again with the full bond,
    /// HELD at once, into two's seat. The escrow invariant holds after every block (next()).
    #[test]
    fn an_idle_member_is_replaced_on_chain_and_refunded_after_u() {
        let mut net = replace_net();
        let one = net.one;
        let two = net.chain.spec.profile().spec().m0_list.iter().map(|m| Address::from(m.id)).find(|m| *m != one).unwrap();
        let (alice, bob, carol) = (devnet::account_key("alice"), devnet::account_key("bob"), devnet::account_key("carol"));
        while net.chain.head().height < 6 { let _ = next(&mut net, one, vec![]); }
        assert_eq!((removable(&net), removal_code(&net, two)), (vec![], 2), "two of three seats: nobody is removable");
        listing_matches_the_registry(&net);
        let t = remove_tx(&net, &bob, two);
        let _ = next(&mut net, one, vec![t]);
        assert_eq!(receipts_status(&net), vec![Vec::<u8>::new()], "removeInactive below a full set reverts");
        assert!(wallet::replace_plan(&net.chain, bob.address(), U256::ZERO, |_| 0).unwrap().is_none(), "below a full set the wallet registers normally");
        let t = register(&net, &alice, 0);
        let _ = next(&mut net, one, vec![t]);
        assert_eq!(apply::active_list(&net.chain.head().state).len(), 3, "alice fills the set");
        assert_eq!(removable(&net), vec![two], "alice is in grace; one mines");
        listing_matches_the_registry(&net);
        let joined = record(&net, alice.address()).grace_start;
        while net.chain.head().height + 1 < joined + 4 { let _ = next(&mut net, one, vec![]); }
        let mut list = removable(&net); list.sort();
        let mut want = vec![two, alice.address()]; want.sort();
        assert_eq!(list, want, "past Theta_inact both idle members are removable");
        listing_matches_the_registry(&net);
        // The wallet's quote decision: the picked member of the list (each index reachable), confirmed by the registry.
        let picks: std::collections::BTreeSet<Address> = (0..2).map(|i| wallet::replace_plan(&net.chain, carol.address(), U256::from(B), |n| { assert_eq!(n, 2); i }).unwrap().unwrap().victim).collect();
        assert_eq!(picks.len(), 2, "either removable member can be picked");
        let k = removable(&net).iter().position(|x| *x == alice.address()).unwrap();
        let plan = wallet::replace_plan(&net.chain, carol.address(), U256::ZERO, |_| k).unwrap().unwrap();
        assert_eq!(plan.victim, alice.address());
        let d = plan.describe();
        assert_eq!((d["replacing"]["removable"].as_u64(), d["replacing"]["refundAfterBlocks"].as_u64()), (Some(2), Some(u64::from(net.chain.spec.cp().U))));
        assert!(d["steps"][1].as_str().unwrap().contains("refunded after") && d["steps"][2].as_str().unwrap().contains("filled from your rewards"), "{d}");
        assert!(wallet::replace_plan(&net.chain, one, U256::ZERO, |_| 0).unwrap_err().contains("already has a registry record"));
        // A replacement already pending in the node's pool takes its victim out of the next quote's draw.
        let pending = vec![replacing_tx(&net, &bob, alice.address(), B), tx(&net, &carol, vec![1, 2, 3], U256::ZERO)];
        assert_eq!(wallet::pending_victims(&net.chain, pending.iter().map(|r| r.as_slice())), vec![alice.address()]);
        let other = wallet::replace_plan_excluding(&net.chain, carol.address(), U256::ZERO, &[alice.address()], |n| { assert_eq!(n, 1); 0 }).unwrap().unwrap();
        assert_eq!(other.victim, two, "alice is named by the pending replacement: the draw is among the others");
        assert!(wallet::replace_plan_excluding(&net.chain, carol.address(), U256::ZERO, &[alice.address(), two], |_| 0).unwrap_err().contains("pending replacement"));
        let mut reg = keccak256("register(address)")[..4].to_vec(); reg.extend_from_slice(bob.address().into_word().as_slice());
        let txs = vec![replacing_tx(&net, &bob, one, B), tx(&net, &carol, wallet::replacing_calldata(carol.address(), plan.victim).to_vec(), U256::ZERO),
            tx_at(&net, &bob, reg, U256::from(B), 1)];
        let _ = next(&mut net, one, txs);
        let h = net.chain.head().height;
        assert_eq!(receipts_status(&net), vec![vec![], vec![1u8], vec![]], "one is not removable; carol replaces alice; register alone at a full set fails");
        let a = record(&net, alice.address());
        assert_eq!((a.status, a.exit_height, a.due_height, a.tomb, a.bond_state), (2, h, h + u64::from(net.chain.spec.cp().U), false, 4), "alice exits as by deregister, not erased");
        assert_eq!((record(&net, carol.address()).status, apply::active_list(&net.chain.head().state).len()), (1, 3), "carol takes the seat; the set stays full");
        assert!(apply::miner_record(&net.chain.head().state, bob.address()).is_none(), "bob's refused calls registered nothing");
        listing_matches_the_registry(&net);
        // The replaced member's wallet: the set is still full, so reactivate() would revert (Capacity). Its step is Full
        // until the refund at its due height, and Rejoin mining is refused with that reason; reactivation itself reverts.
        let step = wallet::chain_rejoin_step(&net.chain, alice.address());
        assert_eq!(step, RejoinStep::Full { refund: a.due_height }, "no rejoin while every seat is taken");
        assert!(wallet::rejoin_refusal(step).unwrap().contains("All seats are taken"));
        // The replaced member's status (G3b'): no bond still to earn (it registered with 0 PCOL, but an exited record earns
        // nothing more: its bond is refunded at its due height) and no reactivation yet; a top-up is refused because the
        // record has exited.
        let st = { let head = net.chain.head(); wallet::v3_status(&net.chain, &head.state, alice.address(), Some(&a), head.height + 1).unwrap() };
        assert_eq!((&st["bondRemaining"], &st["reactivation"]["lastAt"]), (&serde_json::Value::Null, &serde_json::Value::Null), "{st}");
        assert!(wallet::v3_top_up_refusal(Some(&a), U256::from(B)).unwrap().contains("has exited"));
        while net.chain.head().height + 1 < a.exit_height + u64::from(net.chain.spec.cp().W_act) { let _ = next(&mut net, one, vec![]); }
        assert_eq!(wallet::chain_rejoin_step(&net.chain, alice.address()), RejoinStep::Full { refund: a.due_height }, "inside its reactivation window, still full");
        let t = tx(&net, &alice, wallet::reactivate_calldata().to_vec(), U256::ZERO);
        let _ = next(&mut net, one, vec![t]);
        assert_eq!(receipts_status(&net), vec![Vec::<u8>::new()], "reactivate at a full set reverts, as the wallet says");
        assert_eq!(record(&net, alice.address()).status, 2);
        while net.chain.head().height < a.due_height { let _ = next(&mut net, one, vec![]); }
        assert!(apply::miner_record(&net.chain.head().state, alice.address()).is_none(), "the sweep refunded and erased alice at her due height");
        // alice returns with the full bond into the seat of two (carol, joined at h, is past grace too; pick two).
        let k = removable(&net).iter().position(|x| *x == two).unwrap();
        let plan = wallet::replace_plan(&net.chain, alice.address(), U256::from(B), |_| k).unwrap().unwrap();
        assert_eq!(plan.victim, two);
        let t = replacing_tx(&net, &alice, plan.victim, B);
        let _ = next(&mut net, one, vec![t]);
        assert_eq!(receipts_status(&net), vec![vec![1u8]]);
        let r = record(&net, alice.address());
        assert_eq!((r.status, r.bond_state, r.bond), (1, 1, U256::from(B)), "alice returns HELD at once");
        assert_eq!(record(&net, two).status, 2, "two exits");
        listing_matches_the_registry(&net);
    }

    /// V3-08 on chain (V3-PR-E1): one identity's second maintenance operation in a block is a status-0 receipt, while
    /// another identity's registration in that block lands; the next block accepts the maintenance operation.
    #[test]
    fn one_maintenance_operation_per_identity_per_block_on_chain() {
        let mut net = net();
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

    /// V3-09 on chain, the wallet's rejoin step (G3b'), under the v3 network's membership rule (nonceMode 1, as the network
    /// builder sets it for every v3 network): genesis member two never shares, yet past W_act it is still in A_next and its
    /// step stays Active (D-V3-01: every live member keeps a nonce range), so rejoin only follows a voluntary exit. two
    /// exits and reactivates at L, which starts its C_react 200 clock (U 40, W_act 32). Exiting again at L + 1, the
    /// clock-free rule would offer [L + 33, L + 41); the v3 clock opens only at L + 200, past the refund sweep, so the
    /// wallet's step (chain_rejoin_step, what the status and Rejoin mining use) is ClockMissed at once, naming the clock and
    /// the refund height, and a reactivation in that window is indeed a status-0 receipt. The status fields show the clock
    /// while its next allowed height is ahead.
    #[test]
    fn the_wallet_rejoin_reads_the_reactivation_clock_on_chain() {
        let mut cfg = config();
        cfg.c_react = 200;
        cfg.nonce_mode = 1;
        let mut net = net_with(cfg);
        assert_eq!(net.chain.spec.effective_nonce_mode(1), pocold::assignment::NonceMode::Unbounded, "the v3 network's nonce domain from genesis");
        let one = net.one;
        let two = net.chain.spec.profile().spec().m0_list.iter().map(|m| Address::from(m.id)).find(|m| *m != one).unwrap();
        let (w_act, u) = (u64::from(net.chain.spec.cp().W_act), u64::from(net.chain.spec.cp().U));
        assert_eq!((w_act, u, net.chain.spec.cp().C_react), (32, 40, 200));
        let step = |net: &Net| {
            let head = net.chain.head();
            let r = apply::miner_record(&head.state, two);
            let member = net.chain.membership(head, head.height + 1).unwrap().ids().contains(&raw_addr(two));
            (wallet::rejoin_step(r.as_ref(), head.height + 1, member, w_act, None), wallet::chain_rejoin_step(&net.chain, two))
        };
        let status = |net: &Net| {
            let head = net.chain.head();
            wallet::v3_status(&net.chain, &head.state, two, apply::miner_record(&head.state, two).as_ref(), head.height + 1).unwrap()
        };
        assert_eq!(status(&net), serde_json::json!({"bondRemaining": null, "reactivation": {"lastAt": null, "cooldownBlocks": 200, "allowedFrom": null}}), "a HELD genesis member that never reactivated");
        // two never shares: past W_act it stays in A_next (D-V3-01), so its step is Active, never Exit or Cooldown.
        while net.chain.head().height < w_act + 2 { let _ = next(&mut net, one, vec![]); }
        let r = record(&net, two);
        assert!(r.status == 1 && net.chain.head().height + 1 >= r.last_share.max(r.grace_start) + w_act, "two has no share within W_act: {r:?}");
        assert_eq!(step(&net).1, RejoinStep::Active, "a status-1 member that stops sharing stays in the active set on v3");
        assert_eq!(wallet::rejoin_step(Some(&r), net.chain.head().height + 1, false, w_act, None), RejoinStep::Exit, "only a bounded-nonce membership would leave it out");
        // two pays its own fees (FN-2a: an ordinary transfer, no creation fee).
        let alice = devnet::account_key("alice");
        let fund = {
            let head = net.chain.full_entry(&net.chain.head).unwrap();
            let base = net.chain.child_ctx(&head, 0).unwrap().base_fee;
            let priority = U256::from(1_000_000_000u64);
            Tx { chain_id: net.chain.spec.chain_id(), nonce: nonce(&net, alice.address()), max_priority: priority, max_fee: base * U256::from(2) + priority,
                 gas: 21_000, to: Some(two), value: U256::from(PCOL), data: Bytes::new() }.sign_1559(&alice)
        };
        let _ = next(&mut net, one, vec![fund]);
        assert_eq!(balance(&net, two), U256::from(PCOL));
        let t = tx(&net, &net.keys[&two], wallet::exit_calldata().to_vec(), U256::ZERO);
        let _ = next(&mut net, one, vec![t]);
        let e1 = net.chain.head().height;
        assert_eq!(record(&net, two).status, 2);
        assert_eq!(step(&net), (RejoinStep::Wait { opens: e1 + w_act, closes: e1 + u }, RejoinStep::Wait { opens: e1 + w_act, closes: e1 + u }), "no reactivation yet: the same window");
        while net.chain.head().height + 1 < e1 + w_act { let _ = next(&mut net, one, vec![]); }
        assert_eq!(step(&net).1, RejoinStep::Reactivate { closes: e1 + u });
        let t = tx(&net, &net.keys[&two], wallet::reactivate_calldata().to_vec(), U256::ZERO);
        let _ = next(&mut net, one, vec![t]);
        let l = net.chain.head().height;
        assert_eq!((receipts_status(&net), record(&net, two).status), (vec![vec![1u8]], 1), "reactivated at {l}");
        assert_eq!(wallet::react_clock(&net.chain.head().state, two), l, "the clock is the reactivation height");
        assert_eq!(step(&net).1, RejoinStep::Active);
        let st = status(&net);
        assert_eq!((&st["reactivation"]["lastAt"], &st["reactivation"]["allowedFrom"], &st["reactivation"]["cooldownBlocks"], &st["bondRemaining"]),
            (&serde_json::json!(l), &serde_json::json!(l + 200), &serde_json::json!(200), &serde_json::Value::Null), "{st}");
        let t = tx(&net, &net.keys[&two], wallet::exit_calldata().to_vec(), U256::ZERO);
        let _ = next(&mut net, one, vec![t]);
        let e2 = net.chain.head().height;
        assert_eq!((e2, record(&net, two).status), (l + 1, 2));
        let shut = RejoinStep::ClockMissed { refund: e2 + u, allowed_from: l + 200 };
        assert_eq!(step(&net), (RejoinStep::Wait { opens: e2 + w_act, closes: e2 + u }, shut), "the clock-free rule waits; the v3 clock says the window never opens");
        let why = wallet::rejoin_refusal(shut).unwrap();
        assert!(why.contains("reactivated recently") && why.contains(&format!("block {}", l + 200)) && why.contains(&format!("block {}", e2 + u)), "Rejoin mining is refused, naming the clock: {why}");
        while net.chain.head().height + 1 < e2 + w_act { let _ = next(&mut net, one, vec![]); }
        let t = tx(&net, &net.keys[&two], wallet::reactivate_calldata().to_vec(), U256::ZERO);
        let _ = next(&mut net, one, vec![t]);
        assert_eq!((receipts_status(&net), record(&net, two).status), (vec![Vec::<u8>::new()], 2), "the registry refuses it inside C_react, as the wallet said");
        assert_eq!(step(&net).1, shut);
    }

    /// V3-09 ParamGate (lp1-node gate.rs R8 on CP schema v2): admission refuses a v3 devnet whose reactivation window
    /// before the refund sweep is shorter than two blocks or whose C_react is below U, and admits the boundary.
    #[test]
    fn admission_requires_the_v3_reactivation_window_and_cooldown() {
        for (w_act, u, c_react, ok) in [(32, 34, 34, true), (32, 33, 40, false), (32, 40, 39, false), (32, 40, 40, true)] {
            let mut cfg = config();
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

    /// D = 5, so that the evidence block (drawn deterministically) lands before settle(j). (FN-2b: R18 admits a v3 profile only
    /// up to D = 5 at G_SYS 96M, since its final class composes D - 1 settleFinal calls at the all-ACCRUING worst settle.)
    fn net_spec(spec: u8) -> Net {
        let mut cfg = config();
        cfg.spec_version = spec;
        cfg.d = 5;
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
    /// alice, a zero-bond ACCRUING member. One block, proposed by `one`, carries type-2 evidence against both before settle(j).
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
