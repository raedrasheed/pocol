//! Consensus v2 expands the nonce domain without changing genesis or replaying v1
//! blocks under new ownership rules. All activation heights here are test fixtures.
use std::collections::{BTreeMap, HashMap};
use std::sync::{mpsc, OnceLock};
use std::time::Duration;

use pocol_lp4_execution::{admission::{self, Timing}, apply, devnet::{self, DevnetConfig}, txsign::Key};
use pocold::{assignment::NonceMode, chain::{Chain, Reject, Spec}, hashes, miner::{self, MinerEvent}, types::*};
use revm::primitives::{Address, B256, U256};

const ACTIVATION: u64 = 4;
const LABELS: [&str; 7] = ["one", "two", "three", "four", "five", "six", "seven"];

struct Net {
    chain: Chain,
    keys: HashMap<Address, Key>,
}

fn fixture(activation: Option<u64>) -> (Spec, HashMap<Address, Key>) {
    let mut cfg = DevnetConfig::new(777914, 1_700_000_000, &LABELS);
    cfg.hash_rate = 16;
    cfg.c = 8;
    cfg.d_att = 73;
    let mut candidate = devnet::build(&cfg).unwrap();
    if let Some(height) = activation {
        candidate.forks.push((2, height));
    }
    let keys = candidate.miners.iter()
        .map(|m| (m.id, Key::from_secret(&m.key.secret()).unwrap())).collect();
    // The schedule is outside genesis. Measure the identical genesis workload
    // once, while each schedule still passes through its own complete admission.
    static TIMING: OnceLock<Timing> = OnceLock::new();
    let timing = TIMING.get_or_init(|| admission::measure(&candidate.spec_bytes, &candidate.alloc, 1).unwrap());
    let local = pocold::single_host_local();
    let admitted = admission::admit(&candidate.request(&local, Some(timing))).unwrap();
    (Spec::new(admitted), keys)
}

fn net(activation: Option<u64>) -> Net {
    let (spec, keys) = fixture(activation);
    Net { chain: Chain::new(spec), keys }
}

fn signed(net: &Net, parent: [u8; 32], start: u64, extra_time: u64) -> Block {
    let parent = net.chain.full_entry(&parent).unwrap();
    let context = net.chain.child_ctx(&parent, 0).unwrap();
    let first_time = parent.ts + u64::from(net.chain.spec.cp().T_blk) + extra_time;
    for timestamp in first_time..=context.window.upper() {
        let (mut header, _, _) = net.chain.build_signed(
            &parent, &net.keys[&context.proposer], timestamp, Body::default(), vec![],
        ).unwrap();
        let tid = template_id(&header.ut_raw);
        let mut nonce = start;
        let mut shares = Vec::new();
        loop {
            if u128::from(nonce) >= context.assignment.nonce_max() { break; }
            let hash = pow_hash(&tid, nonce);
            if le_target(&hash, &context.target) {
                header.nonce = nonce;
                header.shares = shares;
                reseal(&mut header);
                let owner = addr(&context.assignment.owner(nonce).unwrap());
                header.winner_sig = net.keys[&owner].sign_hash(&B256::from(
                    hashes::win_msg(&tid, nonce, &share_root(&header)),
                ));
                reseal(&mut header);
                return Block { header, body: Body::default() };
            }
            if shares.len() < usize::from(net.chain.spec.cp().S_max)
                && le_target(&hash, &context.t_share) {
                shares.push(nonce);
            }
            match nonce.checked_add(1) { Some(next) => nonce = next, None => break }
        }
    }
    panic!("no solution for deterministic test fixture");
}

fn import(net: &mut Net, block: &Block) {
    let (entry, _) = net.chain.validate(block, None, 0).unwrap();
    net.chain.insert(entry);
}

fn rejects_version(net: &Net, block: &Block) {
    assert!(matches!(net.chain.validate(block, None, 0), Err(Reject::Block(reason)) if reason == "netVersion"));
}

fn expected_payments(net: &Net, block: &Block) -> BTreeMap<Address, U256> {
    let entry = net.chain.get(&block.hash()).unwrap();
    let result = entry.result.as_ref().unwrap();
    let amount = apply::pending(&entry.state, entry.height).0;
    let cp = net.chain.spec.cp();
    let winner = amount * U256::from(cp.alpha_bp) / U256::from(10_000);
    let proposer = amount * U256::from(cp.gamma_bp) / U256::from(10_000);
    let rest = amount - winner - proposer;
    let mut counts = BTreeMap::<Address, u64>::new();
    for owner in result.owner_sequence() { *counts.entry(owner).or_default() += 1; }
    let total: u64 = counts.values().sum();
    let denominator = total.max(u64::from(cp.m) * u64::from(cp.kappa));
    let mut amounts = BTreeMap::<Address, U256>::new();
    *amounts.entry(result.winner).or_default() += winner;
    *amounts.entry(Address::from_slice(&block.header.proposer)).or_default() += proposer;
    for (owner, count) in counts {
        *amounts.entry(owner).or_default() += rest * U256::from(count) / U256::from(denominator);
    }
    amounts
}

#[test]
fn activation_preserves_v1_history_and_settles_its_rewards_across_the_fork() {
    let mut upgraded = net(Some(ACTIVATION));
    let mut legacy = net(None);
    assert_eq!(upgraded.chain.spec.genesis_hash(), legacy.chain.spec.genesis_hash());
    assert_eq!(upgraded.chain.spec.spec_bytes(), legacy.chain.spec.spec_bytes());
    assert_eq!(upgraded.chain.spec.cp().nonceMode, 0);
    assert_eq!(upgraded.chain.spec.effective_nonce_mode(ACTIVATION - 1), NonceMode::Bounded);
    assert_eq!(upgraded.chain.spec.effective_nonce_mode(ACTIVATION), NonceMode::Unbounded);
    assert_eq!(upgraded.chain.spec.effective_nonce_mode(ACTIVATION + 1), NonceMode::Unbounded);
    assert_eq!(legacy.chain.spec.effective_nonce_mode(u64::MAX), NonceMode::Bounded);
    let mut blocks = Vec::new();
    for _ in 1..ACTIVATION {
        let block = signed(&legacy, legacy.chain.head, 0, 0);
        assert_eq!(block.header.protocol_version, 1);
        import(&mut legacy, &block);
        import(&mut upgraded, &block);
        assert_eq!(legacy.chain.head, upgraded.chain.head);
        assert_eq!(legacy.chain.head().header.state_root, upgraded.chain.head().header.state_root);
        assert_eq!(legacy.chain.head().result, upgraded.chain.head().result);
        blocks.push(block);
    }
    let previous = blocks.last().unwrap().clone();
    let old_result = upgraded.chain.head().result.clone().unwrap();
    let payments = expected_payments(&upgraded, &previous);
    let legacy_at_activation = signed(&legacy, legacy.chain.head, 0, 0);
    rejects_version(&upgraded, &legacy_at_activation);
    let mut premature = previous.clone();
    premature.header.protocol_version = 2;
    reseal(&mut premature.header);
    rejects_version(&upgraded, &premature);

    let context = upgraded.chain.child_ctx(upgraded.chain.head(), 0).unwrap();
    assert_eq!(context.assignment.nonce_max(), 1u128 << 64);
    assert_eq!(context.assignment.ranges().len(), LABELS.len());
    assert_eq!(context.assignment.ranges()[0].start(), 0);
    assert_eq!(context.assignment.ranges().last().unwrap().end_exclusive(), 1u128 << 64);
    for range in context.assignment.ranges() {
        assert!(range.inclusive_end().is_some(), "every member receives a range");
        assert_eq!(context.assignment.owner(range.start() as u64), Some(range.id()));
        assert_eq!(context.assignment.owner(range.inclusive_end().unwrap()), Some(range.id()));
    }
    for pair in context.assignment.ranges().windows(2) {
        assert_eq!(pair[0].end_exclusive(), pair[1].start());
    }
    assert!(context.assignment.owner(u64::MAX).is_some());
    let activation = signed(&upgraded, upgraded.chain.head, u64::MAX - 1_000_000, 0);
    assert_eq!(activation.header.protocol_version, 2);
    assert!(u128::from(activation.header.nonce) >= legacy.chain.child_ctx(legacy.chain.head(), 0).unwrap().assignment.nonce_max());
    rejects_version(&legacy, &activation);
    import(&mut upgraded, &activation);
    assert_eq!(apply::pending(&upgraded.chain.head().state, ACTIVATION - 1).2, old_result.winner);
    let mut counts = BTreeMap::<Address, u64>::new();
    for owner in old_result.owner_sequence() { *counts.entry(owner).or_default() += 1; }
    assert_eq!(apply::share_total(&upgraded.chain.head().state, ACTIVATION - 1), old_result.owner_sequence().len() as u64);
    for (owner, count) in counts {
        assert_eq!(apply::share_count(&upgraded.chain.head().state, ACTIVATION - 1, owner), count);
    }
    blocks.push(activation);

    let settlement = ACTIVATION - 1 + u64::from(upgraded.chain.spec.cp().D);
    while upgraded.chain.head().height < settlement {
        let next = signed(&upgraded, upgraded.chain.head, 0, 0);
        let before = upgraded.chain.head().state.clone();
        import(&mut upgraded, &next);
        assert_eq!(next.header.protocol_version, 2);
        if next.header.h == settlement {
            for id in apply::record_list(&before) {
                let reward = apply::miner_record(&before, id).unwrap().reward;
                let delta = upgraded.chain.head().state.accounts[&reward].info.balance - before.accounts[&reward].info.balance;
                assert_eq!(delta, payments.get(&id).copied().unwrap_or_default());
            }
            assert_eq!(apply::pending(&upgraded.chain.head().state, ACTIVATION - 1).0, U256::ZERO);
        }
        blocks.push(next);
    }
    apply::check_escrow_invariant(&upgraded.chain.head().state, upgraded.chain.spec.profile(), settlement, false).unwrap();
    // Restart-style replay uses the block's height, even after the active head is v2.
    let mut replay = net(Some(ACTIVATION));
    for block in &blocks {
        import(&mut replay, block);
        let original = upgraded.chain.get(&block.hash()).unwrap();
        assert_eq!(replay.chain.head().header.state_root, original.header.state_root);
        assert_eq!(replay.chain.head().result, original.result);
    }
    assert_eq!(replay.chain.head, upgraded.chain.head);
    let historical_parent = upgraded.chain.full_entry(&previous.header.parent_hash).unwrap();
    let historical_context = upgraded.chain.child_ctx(&historical_parent, previous.header.a).unwrap();
    assert!(historical_context.assignment.nonce_max() < 1u128 << 64);
}

#[test]
fn reorg_crossing_activation_replays_the_selected_branch_identically() {
    let mut node = net(Some(ACTIVATION));
    let mut prefix = Vec::new();
    for _ in 1..ACTIVATION - 1 {
        let block = signed(&node, node.chain.head, 0, 0);
        import(&mut node, &block);
        prefix.push(block);
    }
    let fork_parent = node.chain.head;
    for _ in 0..3 {
        let block = signed(&node, node.chain.head, 0, 0);
        import(&mut node, &block);
    }
    let abandoned = node.chain.head;
    let mut parent = fork_parent;
    let mut branch = Vec::new();
    for i in 0..8 {
        let block = signed(&node, parent, 0, if i == 0 { 1 } else { 0 });
        parent = block.hash();
        import(&mut node, &block);
        branch.push(block);
    }
    assert_eq!(node.chain.head, parent);
    assert!(!node.chain.is_canonical(&abandoned));
    let mut replay = net(Some(ACTIVATION));
    for block in prefix.iter().chain(branch.iter()) { import(&mut replay, block); }
    assert_eq!(replay.chain.head, node.chain.head);
    assert_eq!(apply::state_root(&replay.chain.head().state).unwrap(), apply::state_root(&node.chain.head().state).unwrap());
    assert_eq!(replay.chain.head().result, node.chain.head().result);
}

#[test]
fn persisted_checkpoint_reopens_across_activation_and_refuses_rewriting_history() {
    use pocold::node::{Config, Limits, Node};
    let dir = std::env::temp_dir().join(format!("pocol-full-nonce-checkpoint-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let config = || Config {
        dir: dir.clone(), name: "full-nonce".into(), miner: None, listen: None,
        peers: vec![], rpc: None, stop_at: None, log: false,
        limits: Limits { state_store: true, checkpoint_every: 2, ..Default::default() },
    };
    let mut source = net(Some(ACTIVATION));
    let mut node = Node::open(config(), fixture(Some(ACTIVATION)).0).unwrap();
    for _ in 0..ACTIVATION + 3 {
        let block = signed(&source, source.chain.head, 0, 0);
        import(&mut source, &block);
        assert!(node.import(block, None).unwrap());
    }
    let head = node.chain.head;
    let genesis = node.chain.spec.genesis_hash();
    let expected = source.chain.full_entry(&head).unwrap();
    node.checkpoint_now().unwrap();
    assert!(node.stats.checkpoints_written > 0);
    drop(node);

    let node = Node::open(config(), fixture(Some(ACTIVATION)).0).unwrap();
    assert!(node.stats.resumed_from.is_some(), "the stored checkpoint was actually restored");
    assert_eq!(node.chain.spec.genesis_hash(), genesis);
    assert_eq!(node.chain.head, head);
    let view = node.chain.view_of(&head).unwrap();
    let restored = pocold::chain::whole_state(view.v());
    assert_eq!(apply::state_root(&restored).unwrap(), apply::state_root(&expected.state).unwrap());
    assert_eq!(node.chain.head().result, expected.result);
    for id in apply::record_list(&expected.state) {
        let reward = apply::miner_record(&expected.state, id).unwrap().reward;
        assert_eq!(restored.accounts[&reward].info.balance, expected.state.accounts[&reward].info.balance);
    }
    drop(view);
    drop(node);

    // Validate journal headers before trusting any checkpoint. Disabling the
    // already-used fork, moving it earlier, or delaying it must fail closed.
    for activation in [None, Some(ACTIVATION - 1), Some(ACTIVATION + 1)] {
        let error = match Node::open(config(), fixture(activation).0) {
            Ok(_) => panic!("a different historical fork schedule was accepted"),
            Err(error) => error,
        };
        assert!(error.contains("conflicts with the configured consensus fork schedule"), "{error}");
    }
    // Refused configurations did not erase or reinterpret the stored chain.
    let node = Node::open(config(), fixture(Some(ACTIVATION)).0).unwrap();
    assert_eq!(node.chain.head, head);
    drop(node);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn miner_handles_the_maximum_nonce_and_full_range_cancellation() {
    let (send, receive) = mpsc::channel();
    let handle = miner::start([11; 32], u64::MAX, u64::MAX, Fixed::MAX, Fixed::MAX, 256, send);
    assert!(matches!(receive.recv_timeout(Duration::from_secs(2)).unwrap(), MinerEvent::Solution { nonce: u64::MAX, .. }));
    drop(handle);
    let (send, receive) = mpsc::channel();
    let handle = miner::start([12; 32], u64::MAX, u64::MAX, Fixed::ZERO, Fixed::ZERO, 0, send);
    assert!(matches!(receive.recv_timeout(Duration::from_secs(2)).unwrap(), MinerEvent::Exhausted { hashes: 1, .. }));
    drop(handle);
    let (send, receive) = mpsc::channel();
    let handle = miner::start([13; 32], 0, u64::MAX, Fixed::ZERO, Fixed::ZERO, 0, send);
    handle.stop();
    assert!(matches!(receive.recv_timeout(Duration::from_secs(2)), Err(mpsc::RecvTimeoutError::Disconnected)));
    drop(handle);
}

#[test]
fn difficulty_one_changes_from_two_nonce_slots_to_ranges_for_all_seven_members() {
    use pocold::assignment::{Assignment, Membership, ParentMember, ParentView};
    let act: Vec<_> = (1..=7u8).map(|n| {
        let mut id = [0; 20];
        id[19] = n;
        ParentMember { id, tomb: false, last_share: 0, grace_start: 0 }
    }).collect();
    let membership = Membership::from_parent(&ParentView { height: 0, seal: &[0; 32], act: &act }, 1, 32, 64).unwrap();
    assert_eq!(pocold::fixed::work(&Fixed::MAX), Fixed::ONE);
    let old = Assignment::new(&membership, 0, &Fixed::MAX, NonceMode::Bounded, 2).unwrap();
    assert_eq!(old.nonce_max(), 2);
    assert_eq!(old.ranges().iter().filter(|r| r.inclusive_end().is_some()).count(), 2);
    let new = Assignment::new(&membership, 0, &Fixed::MAX, NonceMode::Unbounded, 2).unwrap();
    assert_eq!(new.nonce_max(), 1u128 << 64);
    assert_eq!(new.ranges().iter().filter(|r| r.inclusive_end().is_some()).count(), 7);
    assert_eq!(new.ranges()[0].start(), 0);
    assert_eq!(new.ranges().last().unwrap().inclusive_end(), Some(u64::MAX));
    for pair in new.ranges().windows(2) {
        assert_eq!(pair[0].end_exclusive(), pair[1].start());
    }
}
