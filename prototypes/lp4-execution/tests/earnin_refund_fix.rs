//! SECURITY FIX TEST (defensive, in-session, synthetic only) for F1/F2/F3, through the FULL block
//! application path (apply_block), on an earn-in devnet with synthetic keys.
//!
//! These tests encode the FIXED behavior, so they FAIL on the audited db24d9a system programs and
//! PASS once the two-part fix is compiled into tests/data/system-earnin.json:
//!   (source)  register skips the PAY-02 durability probe only when msg.sender==tx.origin (an EOA),
//!             so a same-transaction-created contract can no longer store a self reward.
//!   (sink)    the mandatory SYSTEM refund paths (beginBlock sweep, markClose) never revert on a
//!             failed bond PAY; the bond stays in escrow and the record stays live (claimable).
//!
//! Test A (sink): two records come due in one sweep, one with a durable reward and one with a
//! non-durable reward. Block production continues, the valid payee is paid, the undeliverable bond
//! stays accounted and claimable, balances are conserved, and a second block does not pay twice.
//! Test B (source): a contract that registers itself and self-destructs in one transaction is
//! rejected at registration; no record is stored and the chain never halts.

use pocol_lp4_execution::apply::{
    apply_block, chain_facts, check_escrow_invariant, escrow, miner_record, record_list, registry,
    BlockInput, Proposer,
};
use pocol_lp4_execution::devnet::{self, DevnetConfig};
use revm::primitives::{keccak256, Address, AccountInfo, Bytes, U256, B256};
use revm::InMemoryDB;

fn earnin_net() -> devnet::Devnet {
    let mut cfg = DevnetConfig::new(777903, 1_800_000_000, &["a", "b", "c"]);
    cfg.earnin = true;
    // Earn-in enables miner registration: cfg(17) r_max > 0 (matches earnin_halt_proof.rs's genesis,
    // which sets reg-ops=16). The DevnetConfig default r_max=0 is the pre-earn-in profile that
    // implements no registration at all; leaving it 0 would make every register() revert RegOpsLimit
    // before the reward-durability check, so the source-fix test could not observe that check.
    cfg.r_max = 16;
    cfg.h_end = 0; // continuous: markClose is a no-op, the sweep runs in beginBlock
    devnet::build(&cfg).expect("earn-in devnet")
}
fn name(n: &str) -> Address { Address::from_slice(&keccak256(format!("earnin:{n}"))[12..]) }
fn bal(db: &InMemoryDB, a: Address) -> U256 { db.accounts.get(&a).map(|x| x.info.balance).unwrap_or_default() }
fn total_value(db: &InMemoryDB) -> U256 {
    // Sum of every account balance; PAY moves value between accounts and never creates/destroys it,
    // so this is invariant across a refund (burn, if chosen, would move it into escrow's own balance).
    db.accounts.values().fold(U256::ZERO, |s, a| s + a.info.balance)
}
fn arr(slot: u64) -> U256 { U256::from_be_bytes(keccak256(U256::from(slot).to_be_bytes::<32>()).0) }
fn rslot(id: Address) -> U256 { U256::from_be_bytes(keccak256([id.into_word().as_slice(), U256::from(7).to_be_bytes::<32>().as_slice()].concat()).0) }
fn put(db: &mut InMemoryDB, k: U256, v: U256) { db.insert_account_storage(registry(), k, v).unwrap(); }
fn getw(db: &InMemoryDB, k: U256) -> U256 { db.accounts.get(&registry()).and_then(|x| x.storage.get(&k).copied()).unwrap_or_default() }

/// Plant an already-exited HELD record (as deregister would leave it) with a chosen reward and bond,
/// due at `due`. Keeps the escrow invariant: bond is added to held_total (slot 4) and to escrow balance.
fn plant_exiting(db: &mut InMemoryDB, id: Address, reward: Address, bond: U256, due: u64) {
    let n = getw(db, U256::ZERO); // current record count
    let pos = n.to::<u64>();
    let r = rslot(id);
    let w0 = U256::from(2u64) | (U256::from(1u64) << 16) | (U256::from(pos) << 24); // status=2(exited), bond_state=1(HELD), recPos=pos
    put(db, r, w0);
    put(db, r + U256::from(1), U256::from_be_slice(reward.as_slice()));
    put(db, r + U256::from(2), bond);
    put(db, r + U256::from(3), U256::from(due) << 64); // exit=0, due=<due>
    put(db, arr(5) + n, U256::from_be_slice(id.as_slice())); // record list[pos]=id (not in the active list)
    put(db, U256::ZERO, n + U256::from(1)); // rec_len += 1
    put(db, U256::from(4), getw(db, U256::from(4)) + bond); // held_total += bond
    let e = escrow();
    let mut info = db.accounts.get(&e).unwrap().info.clone();
    info.balance += bond;
    db.insert_account_info(e, info); // escrow holds the planted bond
}

fn input<'a>(h: u64, txs: &'a [Vec<u8>], pr: Option<pocol_lp4_execution::apply::ParentResult>, net: &devnet::Devnet) -> BlockInput<'a> {
    BlockInput {
        height: h, timestamp: 1_800_000_000 + 2 * h, base_fee: U256::from(1_000_000_000u64),
        gas_limit: 8_000_000, prevrandao: B256::ZERO, proposer: Proposer::Fallback,
        parent_result: pr, evidence: vec![], evidence_raw: &[], transactions: txs,
        ancestors: vec![(0, net.genesis_hash)], cum_log_bytes: 0,
    }
}

#[test]
fn refund_failsafe_pays_valid_payee_and_keeps_bad_claimable() {
    let net = earnin_net();
    let d_reward = name("durable-reward"); // PAY-02 durable: inserted below with nonce=1
    let ghost = name("ghost-reward"); // never created -> not durable (nonce 0, no code) -> PAY fails
    let (good_id, bad_id) = (name("good-id"), name("bad-id"));
    let (bond_good, bond_bad) = (U256::from(5_000_000u64), U256::from(7_000_000u64));

    let mut parent = net.genesis.state().clone();
    // PAY-02 durability (src/pay.rs::durable) is nonce>0 OR code — a positive balance alone is NOT
    // durable. Make the valid payee durable the way genesis makes miner reward accounts durable.
    parent.insert_account_info(d_reward, AccountInfo { nonce: 1, balance: U256::ZERO, ..Default::default() });
    plant_exiting(&mut parent, good_id, d_reward, bond_good, 1);
    plant_exiting(&mut parent, bad_id, ghost, bond_bad, 1);
    assert_eq!(record_list(&parent).len(), 5, "3 genesis + 2 planted");
    let (dbal0, held0, val0) = (bal(&parent, d_reward), chain_facts(&parent).held_total, total_value(&parent));

    let o = apply_block(&parent, &net.genesis, &net.profile, &input(1, &[], None, &net));
    let out = match o {
        Ok(out) => out,
        Err(e) => panic!("FAIL-ON-ORIGINAL: the sweep halted the block instead of continuing: {e:?}"),
    };
    let s = &out.state;
    eprintln!("\n==== F1-F3 sink fix through apply_block ====");
    eprintln!("block applied at h=1 (no halt). good_id erased={}, bad_id present={}", miner_record(s, good_id).is_none(), miner_record(s, bad_id).is_some());
    // Valid payee got exactly its bond; its record is gone (paid once).
    assert!(miner_record(s, good_id).is_none(), "the valid payee's record is erased after payment");
    assert_eq!(bal(s, d_reward) - dbal0, bond_good, "durable reward paid exactly its bond");
    // Undeliverable bond stays accounted and claimable: record still live, bond still in escrow.
    let bad = miner_record(s, bad_id).expect("the undeliverable record stays (claimable)");
    assert_eq!(bad.status, 2, "bad record still in the exited/claimable state");
    assert_eq!(bad.bond, bond_bad, "its bond is untouched");
    // Only the deliverable bond left the held pool; the undeliverable one is still fully accounted.
    // held_total is the registry's own bond accounting, so it is not confounded by the block's
    // issuance (a per-block subsidy that is minted into Escrow's pending pot and shows up in the raw
    // Escrow balance and in total_value, but never passes through held_total).
    assert_eq!(held0 - chain_facts(s).held_total, bond_good, "held_total dropped by exactly the paid bond");
    assert_eq!(chain_facts(s).held_total, held0 - bond_good, "the undeliverable bond_bad stays in the held pool");
    check_escrow_invariant(s, &net.profile, 1, false).expect("Escrow still fully backs pending + held + burned");
    // Conservation isolated from issuance: the refund moves the bond inside total_value (Escrow->payee),
    // so it nets to zero there; the only whole-chain delta is the block subsidy (>=0). A refund that
    // burned or duplicated a bond would make total_value fall below val0 -> checked_sub would panic.
    let minted = total_value(s).checked_sub(val0).expect("the refund never destroys whole-chain value");
    eprintln!("  paid bond_good={} to durable reward; bond_bad={} still held & claimable; block subsidy minted={}", bond_good, bad.bond, minted);

    // No double claim: a second block sweeps again; the bad record stays, the good one is not re-paid.
    let pr = pocol_lp4_execution::apply::ParentResult { block_hash: B256::from(U256::from(1)), winner: net.miners[0].id, shares: vec![] };
    let out2 = apply_block(s, &net.genesis, &net.profile, &input(2, &[], Some(pr), &net)).expect("second block also applies");
    assert!(miner_record(&out2.state, good_id).is_none(), "good payee not resurrected");
    assert_eq!(bal(&out2.state, d_reward), bal(s, d_reward), "durable reward NOT paid twice");
    assert_eq!(miner_record(&out2.state, bad_id).map(|r| r.bond), Some(bond_bad), "bad bond still claimable, not double-handled");
    eprintln!("  second block applied; no double payment. RESULT: chain continues, funds conserved, single claim.");
}

// ---- Test B: source fix rejects a same-tx self-register + self-destruct ----
fn sel4(s: &str) -> [u8; 4] { let h = keccak256(s.as_bytes()); [h[0], h[1], h[2], h[3]] }
fn store_sel(c: &mut Vec<u8>, s: [u8; 4]) { c.push(0x63); c.extend_from_slice(&s); c.extend_from_slice(&[0x60, 0xE0, 0x1b, 0x60, 0x00, 0x52]); }
fn do_call(c: &mut Vec<u8>, value: u8, arglen: u8) { c.extend_from_slice(&[0x60, 0x00, 0x60, 0x00, 0x60, arglen, 0x60, 0x00, 0x60, value, 0x62, 0xC0, 0xC0, 0x01, 0x5a, 0xf1, 0x50]); }
fn selfreg_selfdestruct_init() -> Vec<u8> {
    let mut c = Vec::new();
    store_sel(&mut c, sel4("register(address)")); c.extend_from_slice(&[0x30, 0x60, 0x04, 0x52]); do_call(&mut c, 1, 0x24);
    store_sel(&mut c, sel4("deregister()")); do_call(&mut c, 0, 0x04);
    c.extend_from_slice(&[0x33, 0xff]); // CALLER ; SELFDESTRUCT
    c
}

#[test]
fn register_rejects_same_tx_selfdestruct_reward() {
    use pocol_lp4_execution::txsign::Tx;
    let net = earnin_net();
    let attacker = devnet::account_key("alice"); // funded EOA
    let x = attacker.address().create(0); // the contract this CREATE tx deploys
    let tx = Tx { chain_id: 777903, nonce: 0, max_priority: U256::from(1_000_000_000u64), max_fee: U256::from(100_000_000_000u64), gas: 3_000_000, to: None, value: U256::from(1u64), data: Bytes::from(selfreg_selfdestruct_init()) }.sign_1559(&attacker);
    let txs = vec![tx];
    // The deploy block applies on both versions: register()/deregister() are inner CALLs whose
    // success the init code discards (POP), so the CREATE transaction never reverts. The versions
    // differ only in whether the self-register STORED a record. ORIGINAL: arg==msg.sender skips the
    // PAY-02 probe (R9), so the contract's self reward is accepted and a record is stored. FIXED:
    // the exemption also requires msg.sender==tx.origin, so a contract (sender!=origin) runs the
    // probe, which fails pre-transaction for this same-tx-created address -> RewardNotDurable.
    let out = apply_block(net.genesis.state(), &net.genesis, &net.profile, &input(1, &txs, None, &net)).expect("deploy block applies on both versions");
    eprintln!("\n==== F1-F3 source fix through apply_block ====");
    eprintln!("deploy block applied; halts={:?} records={} X present={}", out.halts.len(), record_list(&out.state).len(), miner_record(&out.state, x).is_some());
    // FIXED: the self-register is rejected (pre-transaction non-durable), so no record is stored.
    assert!(miner_record(&out.state, x).is_none(), "FAIL-ON-ORIGINAL: a self-destructing contract registered a self reward");
    assert_eq!(record_list(&out.state).len(), 3, "only the 3 genesis miners remain (the self-register stored nothing)");
    check_escrow_invariant(&out.state, &net.profile, 1, false).expect("escrow invariant intact");
    eprintln!("  registration rejected at the source; no non-durable reward stored. RESULT: attack neutralized, chain unaffected.");
}
