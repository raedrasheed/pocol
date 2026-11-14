//! V3-PR-G (the reward gateway): a short v3 devnet chain produced by pocold and served by pocold's own
//! JSON-RPC server, against which deploy/rewards/test_pocol_rewards.py (`V3DevnetDifferential`) compares the gateway's
//! per-block owners and payouts (deploy/rewards/pocol_rewards.py `snapshot_v3`) with the node's state: reward-address
//! balances, record bonds, RewardEscrow's burned total, pending pots and carry, and the ShareLedger.
//!
//! The chain (a specVersion-2 devnet, the v3 program set, M_max 4, Theta_inact 1, D 4) carries every v3 reward rule the
//! gateway mirrors: tips and the proposer's tip share (V3-20), issuance M3 (V3-19) and the lagged carry (V3-15) on every
//! block, all-live membership (D-V3-01: `three` never mines and keeps its range although W_act is 4), an omitted share
//! credited by kind-3 evidence (V3-02), two registerReplacing calls at the full set (FN-2b C4, owner decision Q2(a) of
//! 2026-11-13: the idle HELD `three`, and the idle ACCRUING alice, whose victim record stays exited, payable (its pending
//! pot fills its accrued bond) and punishable until its refund), type-2 evidence
//! against that ACCRUING winner (the whole pot burns, V3-16) and against a HELD winner (offender-only burn), each with
//! the reporter's cut rho (V3-17). Then (blocks 24..50): type-2 evidence against a share owner who is not the winner, against
//! pot h-D's ACCRUING winner in block h (invalidated before its own settlement) and against the parent block's ACCRUING
//! winner, late shares carried by the block that settles their pot, a fallback block, the drawn proposer's own registry
//! call in the block it proposes, closing (markClose refunds every live record, an ACCRUING payee included, before the
//! block's settlement) and the final block (finalize burns the tips and carries, settleFinal settles its last pots).
//! Test-only: no consensus source is touched. Needs python3.
mod on_chain {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};
    use pocol_lp4_execution::{admission, apply, devnet::{self, DevnetConfig}, txsign::{Key, Tx}};
    use pocold::{chain::{late_encode, Spec}, hashes, node::{Config, Limits, Node}, rlp, types::*, wallet};
    use revm::primitives::{keccak256, Address, Bytes, B256, U256};

    const PCOL: u128 = 1_000_000_000_000_000_000;
    const B: u128 = 20 * PCOL;
    const CHAIN_ID: u64 = 777963;
    const H_CLOSE: u64 = 46;
    /// T_CLOSE = T_END, between the timestamps of blocks 45 and 50 (this fixture's blocks are T_blk = 4 s apart): closing
    /// starts by height at H_CLOSE, and the final block is H_CLOSE + D.
    const T_END_OFFSET: u64 = 190;

    struct Net { node: Node, keys: HashMap<Address, Key>, nonces: HashMap<Address, u64> }
    impl Net {
        /// The members removable at the next block (the node's pocol_getRemovable list).
        fn removable(&self) -> Vec<Address> {
            let e = self.node.chain.full_entry(&self.node.chain.head).unwrap();
            wallet::removable_members(&self.node.chain, &*e.state, e.height + 1, &wallet::head_credited(&self.node.chain))
        }
    }

    fn config() -> DevnetConfig {
        let mut c = DevnetConfig::new(CHAIN_ID, 1_700_000_000, &["one", "two", "three"]);
        c.hash_rate = 16; c.c = 8; c.d_att = 73; c.r_max = 1; c.earnin = true; c.spec_version = 2;
        // Theta_inact 1 (below D 4): a member idle for one block is removable at the full set, so the pot of its last share
        // can still be pending when it is replaced (the victim stays payable).
        c.m_max = 4; c.theta_inact = 1;
        // W_act 4: the spec-1 activity filter would drop the idle `three` from the nonce ranges; v3 keeps every live member.
        c.w_act = 4;
        // Closing from H_CLOSE (or T_CLOSE), and the final block at the first height >= closeHeight + D with a timestamp at
        // T_END = T_CLOSE (consensus §8); H_END = H_CLOSE + Z_close (R13) is only the ceiling.
        c.h_close = H_CLOSE; c.h_end = H_CLOSE + u64::from(c.z_close); c.t_close_offset = T_END_OFFSET; c.t_end_offset = T_END_OFFSET;
        for label in ["carol", "erin", "frank", "gina", "dave", "hank"] { c.accounts.push((label.into(), U256::from(10u128.pow(24)))); }
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
    fn replacing(n: &mut Net, key: &Key, reward: Address, victim: Address, value: u128) -> Vec<u8> {
        tx(n, key, apply::program_address(0), wallet::replacing_calldata(reward, victim).to_vec(), value)
    }
    fn receipts_status(n: &Net) -> Vec<Vec<u8>> {
        let e = n.node.chain.full_entry(&n.node.chain.head).unwrap();
        e.receipts.iter().map(|r| rlp::decode(&r[1..]).unwrap().list().unwrap()[0].bytes().unwrap().to_vec()).collect()
    }

    /// The next block, imported through the node: the drawn proposer signs it (or, with `fallback`, the first fallback
    /// attempt with a solution); its header shares one nonce of each of `owners`' ranges; `omit`'s first two shares are
    /// found but left out (returned, for a later kind-3 item); `winner` solves it in its own range. Returns (height,
    /// proposer (zero for a fallback block), omitted nonces, each owner's header share nonce).
    fn mine_with(n: &mut Net, winner: Address, owners: &[Address], omit: Option<Address>, body: Body, fallback: bool)
        -> (u64, Address, Vec<u64>, Vec<(Address, u64)>) {
        let chain = &n.node.chain;
        let parent = chain.full_entry(&chain.head).unwrap();
        let effects = chain.evidence_effects(&parent, &body, parent.height + 1).unwrap();
        let signed_ctx = chain.child_ctx(&parent, 0).unwrap();
        let candidates: Vec<_> = if fallback {
            assert!(body.txs.is_empty() && body.evidence.is_empty(), "a fallback block has the empty body");
            (0..16).filter_map(|a| chain.build_fallback(&parent, a).ok().map(|(h, _, ctx)| (h, ctx))).collect()
        } else {
            (parent.ts + u64::from(chain.spec.cp().T_blk)..=signed_ctx.window.upper()).map(|ts| {
                let (h, _, ctx) = chain.build_signed(&parent, &n.keys[&signed_ctx.proposer], ts, body.clone(), effects.clone()).unwrap();
                (h, ctx)
            }).collect()
        };
        'templates: for (mut header, ctx) in candidates {
            let nmax = ctx.assignment.nonce_max();
            let range = |who: Address| ctx.assignment.ranges().iter().find(|r| r.id() == raw_addr(who)).cloned()
                .unwrap_or_else(|| panic!("{who:#x} has no range at {}", parent.height + 1));
            let tid = template_id(&header.ut_raw);
            let share = |n: u128| le_target(&pow_hash(&tid, n as u64), &ctx.t_share);
            let mut shares = Vec::new();
            let mut mine = Vec::new();
            for o in owners {
                let r = range(*o);
                match (r.start()..r.end_exclusive().min(nmax)).find(|x| share(*x)) { Some(x) => { shares.push(x as u64); mine.push((*o, x as u64)) } None => continue 'templates }
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
            let proposer = if fallback { Address::ZERO } else { ctx.proposer };
            n.node.import(block, None).unwrap_or_else(|e| panic!("block {}: {e:?}", parent.height + 1));
            assert_eq!(n.node.chain.head, hash, "block {} is the head", parent.height + 1);
            return (parent.height + 1, proposer, omitted, mine);
        }
        panic!("no solution for the deterministic fixture at {}", parent.height + 1);
    }
    fn mine(n: &mut Net, winner: Address, owners: &[Address], omit: Option<Address>, body: Body) -> (u64, Address, Vec<u64>) {
        let (h, p, omitted, _) = mine_with(n, winner, owners, omit, body, false);
        (h, p, omitted)
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

    /// Type-2 evidence by `signer` for `nonce` of canonical block `height`'s template: two winner signatures over two
    /// share roots (the header's and the empty list's). The signer must own the nonce there; it need not be the
    /// winning nonce, so a share owner who is not the winner can be accused (chain.rs evidence_type2).
    fn equivocation_by(n: &Net, height: u64, signer: Address, nonce: u64) -> Vec<u8> {
        let h = n.node.chain.canonical_at(height).unwrap().header.clone();
        let tid = template_id(&h.ut_raw);
        let mut alt = h.clone(); alt.shares = vec![]; reseal(&mut alt);
        let (root1, root2) = (share_root(&h), share_root(&alt));
        assert_ne!(root1, root2, "the block has header shares");
        let sig1 = n.keys[&signer].sign_hash(&B256::from(hashes::win_msg(&tid, nonce, &root1)));
        let sig2 = n.keys[&signer].sign_hash(&B256::from(hashes::win_msg(&tid, nonce, &root2)));
        let mut payload = Vec::new();
        rlp::encode_bytes(&[2], &mut payload); rlp::encode_bytes(&tid, &mut payload); rlp::encode_bytes(&nonce.to_be_bytes(), &mut payload);
        rlp::encode_bytes(&root1, &mut payload); rlp::encode_bytes(&sig1, &mut payload); rlp::encode_bytes(&root2, &mut payload); rlp::encode_bytes(&sig2, &mut payload);
        let mut out = Vec::new(); rlp::encode_list_payload(&payload, &mut out); out
    }
    fn calldata(signature: &str, arg: Option<Address>) -> Vec<u8> {
        let mut data = keccak256(signature)[..4].to_vec();
        if let Some(a) = arg { data.extend_from_slice(a.into_word().as_slice()); }
        data
    }

    #[test]
    fn the_reward_gateway_reproduces_a_v3_devnet_chain_over_rpc() {
        let cfg = config();
        let mut candidate = devnet::build(&cfg).unwrap();
        candidate.forks.push((2, 2));
        let genesis = candidate.genesis_hash;
        let mut keys: HashMap<Address, Key> = candidate.miners.iter().map(|m| (m.id, Key::from_secret(&m.key.secret()).unwrap())).collect();
        for label in ["alice", "bob", "carol", "dave", "hank"] { let k = account(label); keys.insert(k.address(), k); }
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
        // Every block's parent state stays readable over RPC (the differential reads each block at h - 1 and h).
        n.node.chain.k_hist = 1_000;
        let (one, two, three) = (id("one"), id("two"), id("three"));
        let (alice, bob, carol) = (account("alice"), account("bob"), account("carol"));
        let (erin, frank, gina) = (account("erin"), account("frank"), account("gina"));
        let (dave, hank) = (account("dave"), account("hank"));

        // 1: the three reward sinks of the newcomers make themselves durable PAY recipients (and pay tips).
        let txs = vec![tx(&mut n, &erin, erin.address(), vec![], 0), tx(&mut n, &frank, frank.address(), vec![], 0), tx(&mut n, &gina, gina.address(), vec![], 0)];
        mine(&mut n, one, &[one], None, Body { txs, evidence: vec![] });
        // 2: alice registers with a zero bond (ACCRUING, counted within M_max), paid to erin; the set is now full.
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
        // 11: at the full set bob registers in the seat of the idle three (removable: no credited share ever; at Theta_inact
        // 1 every member without a share credited in block 11 itself is too), paid to frank, full bond.
        assert!(n.removable().contains(&three), "three is removable");
        let t = replacing(&mut n, &bob, frank.address(), three, B);
        mine(&mut n, one, &[one, two], None, Body { txs: vec![t], evidence: vec![] });
        assert_eq!(receipts_status(&n), vec![vec![1u8]], "bob's registerReplacing");
        let r = record(&n, three);
        assert_eq!((r.status, r.exit_height, r.tomb), (2, 11, false), "three exits as by deregister");
        // 12..15: alice stays idle.
        while head_height(&n) < 15 { quiet(&mut n, two, &[one, two, bob.address()]); }
        // 16: one wins with a share of alice; 17: two; 18: alice wins again, ACCRUING (her lastShare 18).
        quiet(&mut n, one, &[one, alice.address()]);
        let t = tx(&mut n, &alice, bob.address(), vec![], PCOL);
        mine(&mut n, two, &[two, bob.address()], None, Body { txs: vec![t], evidence: vec![] });
        let (k, _) = quiet(&mut n, alice.address(), &[alice.address(), one]);
        assert_eq!(k, 18);
        // 19: the set is full and alice is idle for Theta_inact: carol (zero bond) registers in her seat, paid to gina.
        assert!(n.removable().contains(&alice.address()), "alice is removable at 19");
        let t = replacing(&mut n, &carol, gina.address(), alice.address(), 0);
        mine(&mut n, one, &[one, two], None, Body { txs: vec![t], evidence: vec![] });
        assert_eq!(receipts_status(&n), vec![vec![1u8]], "carol's registerReplacing");
        let a = record(&n, alice.address());
        assert_eq!((a.status, a.bond_state, a.tomb), (2, 4, false), "alice exits as by deregister, still ACCRUING");
        let bond_at_exit = a.bond;
        // 20: settle(16) pays the exited alice's share into her accrued bond.
        quiet(&mut n, one, &[one, two]);
        assert!(record(&n, alice.address()).bond > bond_at_exit, "the exited victim is payable");
        // 21: type-2 evidence against alice for pot 18 (recorded ACCRUING: settle(18) at 22 burns it whole).
        let ev = equivocation(&n, 18);
        mine(&mut n, two, &[one, two], None, Body { txs: vec![], evidence: vec![ev] });
        assert!(record(&n, alice.address()).tomb, "the exited victim is punishable");
        // 22: two wins (HELD) with shares of one and two; 23: type-2 evidence against it before settle(22) at 26: the pot
        // settles normally, two's parts burn (offender-only).
        let (j, _) = quiet(&mut n, two, &[one, two]);
        let ev = equivocation(&n, j);
        let t = tx(&mut n, &alice, bob.address(), vec![], PCOL);
        mine(&mut n, one, &[one, bob.address()], None, Body { txs: vec![t], evidence: vec![ev] });
        assert!(record(&n, two).tomb);
        // 24: one wins; erin funds one's identity, which later calls MinerRegistry itself.
        let t = tx(&mut n, &erin, one, vec![], 2 * PCOL);
        mine(&mut n, one, &[one, bob.address()], None, Body { txs: vec![t], evidence: vec![] });
        // 25: carol (ACCRUING) wins with a share of bob; 26: type-2 evidence against bob for his share nonce of template 25
        // (a share owner, not the winner: pot 25 settles normally at 29, carol's parts fill her bond, bob's burn).
        let (accused_pot, _, _, nonces) = mine_with(&mut n, carol.address(), &[carol.address(), bob.address()], None, Body::default(), false);
        let bob_nonce = nonces.iter().find(|(o, _)| *o == bob.address()).unwrap().1;
        let ev = equivocation_by(&n, accused_pot, bob.address(), bob_nonce);
        mine(&mut n, one, &[one, carol.address()], None, Body { txs: vec![], evidence: vec![ev] });
        assert!(record(&n, bob.address()).tomb);
        // 27: carol wins again (ACCRUING); 28: one leaves two of its own shares out of its header.
        let (same_block_pot, _) = quiet(&mut n, carol.address(), &[carol.address(), one]);
        let (late_pot, _, omitted) = mine(&mut n, one, &[one], Some(one), Body::default());
        quiet(&mut n, one, &[one, carol.address()]);
        quiet(&mut n, one, &[one, carol.address()]);
        // 31: type-2 evidence against carol for pot 27 in the block that settles it (invalidate, then settle(27) burns it whole).
        let ev = equivocation(&n, same_block_pot);
        let (k, _, _) = mine(&mut n, one, &[one], None, Body { txs: vec![], evidence: vec![ev] });
        assert_eq!(k, same_block_pot + 4);
        // 32: block 28's omitted shares credited late in the block that settles pot 28 (kind 3 for template h - D); dave
        // registers with a zero bond (ACCRUING, his own reward address).
        let t = register(&mut n, &dave, dave.address(), 0);
        let (k, _, _) = mine(&mut n, one, &[one], None, Body { txs: vec![t], evidence: vec![late_encode(late_pot, &omitted)] });
        assert_eq!(k, late_pot + 4);
        assert_eq!(record(&n, dave.address()).bond_state, 4);
        // 33: dave (ACCRUING) wins; 34: type-2 evidence against him for the parent block (recordResult marks pot 33's winner
        // ACCRUING in the same phase 0, so pot 33 burns whole at 37); hank registers with a zero bond.
        let (parent_pot, _) = quiet(&mut n, dave.address(), &[dave.address(), one]);
        let ev = equivocation(&n, parent_pot);
        let t = register(&mut n, &hank, hank.address(), 0);
        mine(&mut n, one, &[one], None, Body { txs: vec![t], evidence: vec![ev] });
        assert!(record(&n, dave.address()).tomb);
        // 35: a fallback block (no proposer: the winner takes the proposer part at 39, no tip share).
        let (fallback_block, _, _, _) = mine_with(&mut n, one, &[one, hank.address()], None, Body::default(), true);
        // The drawn proposer calls MinerRegistry in its own block (one: setRewardAddress to erin; hank: a topUp below
        // B_reg), with tips: the tip share is paid on the record as the user phase left it.
        let mut own_tx_block = 0;
        for _ in 0..4 {
            let parent = n.node.chain.full_entry(&n.node.chain.head).unwrap();
            let drawn = n.node.chain.child_ctx(&parent, 0).unwrap().proposer;
            let t = if drawn == one { let key = Key::from_secret(&n.keys[&one].secret()).unwrap(); tx(&mut n, &key, apply::program_address(0), calldata("setRewardAddress(address)", Some(erin.address())), 0) }
                else if drawn == hank.address() { tx(&mut n, &hank, apply::program_address(0), calldata("topUp()", None), PCOL) }
                else { quiet(&mut n, one, &[one, hank.address()]); continue };
            let (k, p, _) = mine(&mut n, one, &[one, hank.address()], None, Body { txs: vec![t], evidence: vec![] });
            assert_eq!((p, receipts_status(&n)), (drawn, vec![vec![1u8]]), "the proposer's own registry call");
            own_tx_block = k;
            break;
        }
        assert!(own_tx_block > 0, "one or hank proposes within four blocks");
        // Until closing hank (ACCRUING) shares every block, so the pot of the first closing block's settle pays a record
        // markClose has just refunded (PAID): directly, not into its bond.
        let closing = |n: &Net| apply::chain_facts(&*n.node.chain.full_entry(&n.node.chain.head).unwrap().state).closing;
        while !closing(&n) {
            assert!(head_height(&n) < H_CLOSE, "closing starts by H_CLOSE");
            let before = record(&n, hank.address());
            quiet(&mut n, one, &[one, hank.address()]);
            if !closing(&n) { assert_eq!(record(&n, hank.address()).bond_state, 4) } else { assert_eq!(before.bond_state, 4) }
        }
        let close_height = head_height(&n);
        assert!(close_height > own_tx_block + 4, "the scenarios end before closing");
        // markClose refunded every live record (one, hank, the exited three) before settle(closeHeight - D).
        assert_eq!((record(&n, hank.address()).bond_state, record(&n, one).bond_state, record(&n, three).bond_state), (3, 3, 3));
        // Closing blocks with tips (a self-transfer of gina's) up to the final block, which burns them and settles its last
        // D - 1 pots (settleFinal).
        while !n.node.chain.head().is_final {
            assert!(head_height(&n) < H_CLOSE + 40, "the final block follows closing");
            let t = tx(&mut n, &gina, gina.address(), vec![], 0);
            mine(&mut n, one, &[one, hank.address()], None, Body { txs: vec![t], evidence: vec![] });
        }
        let final_block = head_height(&n);
        assert!(final_block >= close_height + 4);

        // Serve the chain over pocold's JSON-RPC and run the gateway's differential test against it.
        n.node.start_network().unwrap();
        let names: serde_json::Map<String, serde_json::Value> = [("one", one), ("two", two), ("three", three), ("alice", alice.address()),
            ("bob", bob.address()), ("carol", carol.address()), ("dave", dave.address()), ("hank", hank.address()), ("erin", erin.address())].into_iter().map(|(k, v)| (k.to_string(), serde_json::json!(format!("{v:#x}")))).collect();
        let fixture = serde_json::json!({"rpc": format!("http://127.0.0.1:{port}"), "chainId": CHAIN_ID, "genesisHash": format!("{genesis:#x}"),
            "forkSchedule": [[1, 0], [2, 2]], "head": head_height(&n), "members": names,
            "lateBlock": 5, "replacingHeld": 11, "replacingAccruing": 19, "victimPaid": 20, "wholePotBurn": 18, "offenderPot": j,
            "shareOwnerAccusedPot": accused_pot, "sameBlockBurnPot": same_block_pot, "lateNowPot": late_pot, "parentEvidencePot": parent_pot,
            "fallbackBlock": fallback_block, "ownRegistryCallBlock": own_tx_block, "closeHeight": close_height, "finalHeight": final_block});
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
