//! V3-PR-G (the reward gateway): a short v3 devnet chain produced by the preview build and served by pocold's own
//! JSON-RPC server, against which deploy/rewards/test_pocol_rewards.py (`V3DevnetDifferential`) compares the gateway's
//! per-block owners and payouts (deploy/rewards/pocol_rewards.py `snapshot_v3`) with the node's state: reward-address
//! balances, record bonds, RewardEscrow's burned total, pending pots and carry, and the ShareLedger.
//!
//! The chain (a specVersion-2 devnet, the v3 program set, M_max 4, P_MAX 1, an 8-block epoch, D 4) carries every v3
//! reward rule the gateway mirrors: tips and the proposer's tip share (V3-20), issuance M3 (V3-19) and the lagged carry
//! (V3-15) on every block, all-live membership (D-V3-01: `three` never mines and keeps its range although W_act is 4), an omitted share
//! credited by kind-3 evidence (V3-02), a seat contest (V3-10) and a probation-pool contest whose victim stays exited,
//! payable (its pending pot fills its accrued bond) and punishable (owner decision 2026-11-13, V3-14), type-2 evidence
//! against that ACCRUING winner (the whole pot burns, V3-16) and against a HELD winner (offender-only burn), each with
//! the reporter's cut rho (V3-17). Test-only: no consensus source is touched. Needs `--features v3-preview` and python3.
#[cfg(feature = "v3-preview")]
mod preview {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};
    use pocol_lp4_execution::{admission, apply, devnet::{self, DevnetConfig}, txsign::{Key, Tx}};
    use pocold::{chain::{late_encode, Spec}, hashes, node::{Config, Limits, Node}, rlp, types::*, wallet};
    use revm::primitives::{keccak256, Address, Bytes, B256, U256};

    const PCOL: u128 = 1_000_000_000_000_000_000;
    const B: u128 = 20 * PCOL;
    const CHAIN_ID: u64 = 777963;

    struct Net { node: Node, keys: HashMap<Address, Key>, nonces: HashMap<Address, u64> }

    fn config() -> DevnetConfig {
        let mut c = DevnetConfig::new(CHAIN_ID, 1_700_000_000, &["one", "two", "three"]);
        c.hash_rate = 16; c.c = 8; c.d_att = 73; c.r_max = 1; c.earnin = true; c.spec_version = 2;
        c.p_max = 1; c.t_prob = 86_400; c.m_max = 4; c.e_epoch = 8; c.theta_bp = 10_000; c.k_sigma = 1; c.theta_inact = 4;
        // W_act 4: the spec-1 activity filter would drop the idle `three` from the nonce ranges; v3 keeps every live member.
        c.w_act = 4;
        for label in ["carol", "erin", "frank", "gina"] { c.accounts.push((label.into(), U256::from(10u128.pow(24)))); }
        c
    }
    struct Temp(PathBuf);
    impl Drop for Temp { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }

    fn id(label: &str) -> Address { devnet::miner_identity(label).id }
    fn account(label: &str) -> Key { devnet::account_key(label) }
    fn head_height(n: &Net) -> u64 { n.node.chain.head().height }
    fn record(n: &Net, who: Address) -> apply::MinerRecord {
        let e = n.node.chain.full_entry(&n.node.chain.head).unwrap();
        apply::miner_record(&*e.state, who).expect("record")
    }

    /// A signed EIP-1559 transaction of `key` at its next nonce, with a 1 gwei priority fee (the block's tips).
    fn tx(n: &mut Net, key: &Key, to: Address, data: Vec<u8>, value: u128) -> Vec<u8> {
        let head = n.node.chain.full_entry(&n.node.chain.head).unwrap();
        let base = n.node.chain.child_ctx(&head, 0).unwrap().base_fee;
        let nonce = n.nonces.entry(key.address()).or_insert(0);
        let priority = U256::from(1_000_000_000u64);
        let raw = Tx { chain_id: CHAIN_ID, nonce: *nonce, max_priority: priority, max_fee: base * U256::from(2) + priority, gas: 1_000_000,
                       to: Some(to), value: U256::from(value), data: Bytes::from(data) }.sign_1559(key);
        *nonce += 1;
        raw
    }
    fn register(n: &mut Net, key: &Key, reward: Address, value: u128) -> Vec<u8> {
        let mut data = keccak256("register(address)")[..4].to_vec();
        data.extend_from_slice(reward.into_word().as_slice());
        tx(n, key, apply::program_address(0), data, value)
    }
    fn contest(n: &mut Net, key: &Key, reward: Address, victim: Address, value: u128) -> Vec<u8> {
        tx(n, key, apply::program_address(0), wallet::contest_calldata(reward, victim).to_vec(), value)
    }
    fn receipts_status(n: &Net) -> Vec<Vec<u8>> {
        let e = n.node.chain.full_entry(&n.node.chain.head).unwrap();
        e.receipts.iter().map(|r| rlp::decode(&r[1..]).unwrap().list().unwrap()[0].bytes().unwrap().to_vec()).collect()
    }

    /// The next block, imported through the node: the drawn proposer signs it; its header shares one nonce of each of
    /// `owners`' ranges; `omit`'s first two shares are found but left out (returned, for a later kind-3 item); `winner`
    /// solves it in its own range. Returns (height, proposer, omitted nonces).
    fn mine(n: &mut Net, winner: Address, owners: &[Address], omit: Option<Address>, body: Body) -> (u64, Address, Vec<u64>) {
        let chain = &n.node.chain;
        let parent = chain.full_entry(&chain.head).unwrap();
        let ctx = chain.child_ctx(&parent, 0).unwrap();
        let effects = chain.evidence_effects(&parent, &body, parent.height + 1).unwrap();
        let nmax = ctx.assignment.nonce_max();
        let range = |who: Address| ctx.assignment.ranges().iter().find(|r| r.id() == raw_addr(who)).cloned()
            .unwrap_or_else(|| panic!("{who:#x} has no range at {}", parent.height + 1));
        'templates: for ts in parent.ts + u64::from(chain.spec.cp().T_blk)..=ctx.window.upper() {
            let (mut header, _, _) = chain.build_signed(&parent, &n.keys[&ctx.proposer], ts, body.clone(), effects.clone()).unwrap();
            let tid = template_id(&header.ut_raw);
            let share = |n: u128| le_target(&pow_hash(&tid, n as u64), &ctx.t_share);
            let mut shares = Vec::new();
            for o in owners {
                let r = range(*o);
                match (r.start()..r.end_exclusive().min(nmax)).find(|x| share(*x)) { Some(x) => shares.push(x as u64), None => continue 'templates }
            }
            let mut omitted = Vec::new();
            if let Some(o) = omit {
                let r = range(o);
                omitted = (r.start()..r.end_exclusive().min(nmax)).filter(|x| share(*x) && !shares.contains(&(*x as u64))).take(2).map(|x| x as u64).collect();
                if omitted.len() < 2 { continue }
            }
            shares.sort_unstable(); shares.dedup();
            let r = range(winner);
            let found = (r.start()..r.end_exclusive().min(nmax)).find(|x| !shares.contains(&(*x as u64)) && !omitted.contains(&(*x as u64))
                && le_target(&pow_hash(&tid, *x as u64), &ctx.target));
            let Some(nonce) = found else { continue };
            header.nonce = nonce as u64; header.shares = shares; reseal(&mut header);
            header.winner_sig = n.keys[&winner].sign_hash(&B256::from(hashes::win_msg(&tid, nonce as u64, &share_root(&header))));
            reseal(&mut header);
            let block = Block { header, body };
            let hash = block.hash();
            n.node.import(block, None).unwrap_or_else(|e| panic!("block {}: {e:?}", parent.height + 1));
            assert_eq!(n.node.chain.head, hash, "block {} is the head", parent.height + 1);
            return (parent.height + 1, ctx.proposer, omitted);
        }
        panic!("no solution for the deterministic fixture at {}", parent.height + 1);
    }
    fn quiet(n: &mut Net, winner: Address, owners: &[Address]) -> (u64, Address) {
        let (h, p, _) = mine(n, winner, owners, None, Body::default());
        (h, p)
    }

    /// Type-2 evidence against the winner of canonical block `height`: its signed winner copy and a second copy of the
    /// same template and nonce without header shares (another share root), signed by the same winner (tests/v3_earnin.rs).
    fn equivocation(n: &Net, height: u64) -> Vec<u8> {
        let e = n.node.chain.canonical_at(height).unwrap();
        let (h, winner) = (e.header.clone(), e.result.clone().unwrap().winner);
        let tid = template_id(&h.ut_raw);
        let mut alt = h.clone(); alt.shares = vec![]; reseal(&mut alt);
        let (root1, root2) = (share_root(&h), share_root(&alt));
        assert_ne!(root1, root2, "the block has header shares");
        let sig2 = n.keys[&winner].sign_hash(&B256::from(hashes::win_msg(&tid, h.nonce, &root2)));
        let mut payload = Vec::new();
        rlp::encode_bytes(&[2], &mut payload); rlp::encode_bytes(&tid, &mut payload); rlp::encode_bytes(&h.nonce.to_be_bytes(), &mut payload);
        rlp::encode_bytes(&root1, &mut payload); rlp::encode_bytes(&h.winner_sig, &mut payload); rlp::encode_bytes(&root2, &mut payload); rlp::encode_bytes(&sig2, &mut payload);
        let mut out = Vec::new(); rlp::encode_list_payload(&payload, &mut out); out
    }

    #[test]
    fn the_reward_gateway_reproduces_a_v3_devnet_chain_over_rpc() {
        let cfg = config();
        let mut candidate = devnet::build(&cfg).unwrap();
        candidate.forks.push((2, 2));
        let genesis = candidate.genesis_hash;
        let mut keys: HashMap<Address, Key> = candidate.miners.iter().map(|m| (m.id, Key::from_secret(&m.key.secret()).unwrap())).collect();
        for label in ["alice", "bob", "carol"] { let k = account(label); keys.insert(k.address(), k); }
        let timing = admission::measure(&candidate.spec_bytes, &candidate.alloc, 1).unwrap();
        let admitted = admission::admit(&candidate.request(&pocold::single_host_local(), Some(&timing))).unwrap();
        let spec = Spec::new(admitted);
        assert_eq!(spec.programs(), apply::ProgramSet::V3);
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let dir = Temp(std::env::temp_dir().join(format!("pocol-v3-reward-gateway-{}", std::process::id())));
        let _ = std::fs::remove_dir_all(&dir.0);
        let mut limits = Limits::default();
        limits.rpc.rate = 100_000; limits.rpc.bucket_cap = 100_000;
        let node = Node::open(Config { dir: dir.0.clone(), name: "reward-gateway".into(), miner: None, listen: None, peers: vec![],
            rpc: Some(format!("127.0.0.1:{port}").parse().unwrap()), stop_at: None, log: false, limits }, spec).unwrap();
        let mut n = Net { node, keys, nonces: HashMap::new() };
        let (one, two, three) = (id("one"), id("two"), id("three"));
        let (alice, bob, carol) = (account("alice"), account("bob"), account("carol"));
        let (erin, frank, gina) = (account("erin"), account("frank"), account("gina"));

        // 1: the three reward sinks of the newcomers make themselves durable PAY recipients (and pay tips).
        let txs = vec![tx(&mut n, &erin, erin.address(), vec![], 0), tx(&mut n, &frank, frank.address(), vec![], 0), tx(&mut n, &gina, gina.address(), vec![], 0)];
        mine(&mut n, one, &[one], None, Body { txs, evidence: vec![] });
        // 2: alice registers with a zero bond (ACCRUING, the one probation seat), paid to erin; the set is now full.
        let t = register(&mut n, &alice, erin.address(), 0);
        mine(&mut n, two, &[one, two], None, Body { txs: vec![t], evidence: vec![] });
        assert_eq!((record(&n, alice.address()).bond_state, apply::active_list(&*n.node.chain.full_entry(&n.node.chain.head).unwrap().state).len()), (4, 4));
        // 3: alice (all-live: a range without a share in the last W_act) wins as an ACCRUING winner.
        quiet(&mut n, alice.address(), &[alice.address(), one]);
        // 4: one wins and leaves two's shares out of its header; 5 credits them late (kind 3), with tips.
        let (_, _, omitted) = mine(&mut n, one, &[one], Some(two), Body::default());
        let t = tx(&mut n, &alice, bob.address(), vec![], PCOL);
        mine(&mut n, one, &[one, two], None, Body { txs: vec![t], evidence: vec![late_encode(4, &omitted)] });
        // 6..10: one and two work; three never does (it keeps its range: all-live membership).
        while head_height(&n) < 10 { quiet(&mut n, one, &[one, two]); }
        // 11 (epoch 0's late window closed): bob contests the idle three for its seat, paid to frank, full bond.
        let t = contest(&mut n, &bob, frank.address(), three, B);
        mine(&mut n, one, &[one, two], None, Body { txs: vec![t], evidence: vec![] });
        assert_eq!(receipts_status(&n), vec![vec![1u8]], "bob's seat contest");
        let r = record(&n, three);
        assert_eq!((r.status, r.exit_height, r.tomb), (2, 11, false), "three exits as by deregister");
        // 12..15: alice stays idle for all of epoch 1 (templates 8..15).
        while head_height(&n) < 15 { quiet(&mut n, two, &[one, two, bob.address()]); }
        // 16: one wins with a share of alice (epoch 2); 17: two; 18: alice wins again, ACCRUING.
        quiet(&mut n, one, &[one, alice.address()]);
        let t = tx(&mut n, &alice, bob.address(), vec![], PCOL);
        mine(&mut n, two, &[two, bob.address()], None, Body { txs: vec![t], evidence: vec![] });
        let (k, _) = quiet(&mut n, alice.address(), &[alice.address(), one]);
        assert_eq!(k, 18);
        // 19: carol, a probationer, faces the full pool and displaces the idle alice (pool contest), paid to gina.
        let t = contest(&mut n, &carol, gina.address(), alice.address(), 0);
        mine(&mut n, one, &[one, two], None, Body { txs: vec![t], evidence: vec![] });
        assert_eq!(receipts_status(&n), vec![vec![1u8]], "carol's pool contest");
        let a = record(&n, alice.address());
        assert_eq!((a.status, a.bond_state, a.tomb), (2, 4, false), "alice exits, still ACCRUING (owner decision 2026-11-13)");
        let bond_at_exit = a.bond;
        // 20: settle(16) pays the exited alice's share into her accrued bond.
        quiet(&mut n, one, &[one, two]);
        assert!(record(&n, alice.address()).bond > bond_at_exit, "the exited pool victim is payable");
        // 21: type-2 evidence against alice for pot 18 (recorded ACCRUING: settle(18) at 22 burns it whole).
        let ev = equivocation(&n, 18);
        mine(&mut n, two, &[one, two], None, Body { txs: vec![], evidence: vec![ev] });
        assert!(record(&n, alice.address()).tomb, "the exited pool victim is punishable");
        // 22: two wins (HELD) with shares of one and two; 23: type-2 evidence against it before settle(22) at 26: the pot
        // settles normally, two's parts burn (offender-only).
        let (j, _) = quiet(&mut n, two, &[one, two]);
        let ev = equivocation(&n, j);
        let t = tx(&mut n, &alice, bob.address(), vec![], PCOL);
        mine(&mut n, one, &[one, bob.address()], None, Body { txs: vec![t], evidence: vec![ev] });
        assert!(record(&n, two).tomb);
        while head_height(&n) < 28 { quiet(&mut n, one, &[one, bob.address()]); }

        // Serve the chain over pocold's JSON-RPC and run the gateway's differential test against it.
        n.node.start_network().unwrap();
        let names: serde_json::Map<String, serde_json::Value> = [("one", one), ("two", two), ("three", three), ("alice", alice.address()),
            ("bob", bob.address()), ("carol", carol.address())].into_iter().map(|(k, v)| (k.to_string(), serde_json::json!(format!("{v:#x}")))).collect();
        let fixture = serde_json::json!({"rpc": format!("http://127.0.0.1:{port}"), "chainId": CHAIN_ID, "genesisHash": format!("{genesis:#x}"),
            "forkSchedule": [[1, 0], [2, 2]], "head": head_height(&n), "members": names,
            "lateBlock": 5, "seatContest": 11, "poolContest": 19, "poolVictimPaid": 20, "wholePotBurn": 18, "offenderPot": j});
        // POCOL_V3_DEVNET_SERVE=<file> (manual use): write the fixture there and serve the chain until the file is removed
        // (at most an hour), so that `POCOL_V3_DEVNET="$(cat <file>)" python3 -B -m unittest test_pocol_rewards` can run
        // against it from deploy/rewards.
        if let Ok(path) = std::env::var("POCOL_V3_DEVNET_SERVE") {
            std::fs::write(&path, fixture.to_string()).unwrap();
            let until = Instant::now() + Duration::from_secs(3600);
            n.node.run_controlled(|_| !std::path::Path::new(&path).exists() || Instant::now() > until);
            return;
        }
        let rewards = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../deploy/rewards");
        let mut child = std::process::Command::new("python3").args(["-B", "-m", "unittest", "-v", "test_pocol_rewards.V3DevnetDifferential"])
            .current_dir(&rewards).env("POCOL_V3_DEVNET", fixture.to_string()).spawn().expect("python3");
        let deadline = Instant::now() + Duration::from_secs(900);
        let mut status = None;
        n.node.run_controlled(|_| { status = child.try_wait().unwrap(); status.is_some() || Instant::now() > deadline });
        let status = match status { Some(s) => s, None => { let _ = child.kill(); panic!("the differential test did not finish") } };
        assert!(status.success(), "deploy/rewards V3DevnetDifferential failed: {status}");
    }
}
