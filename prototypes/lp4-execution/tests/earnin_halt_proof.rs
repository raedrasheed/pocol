//! Regression for the historical F1/F2/F3 earn-in constructor/self-reward halt.
//! The original witness expected registration from a non-durable constructor to
//! succeed and the due-height sweep to revert. Current contracts reject that
//! registration. Keep both destroyed and persistent constructors plus a durable
//! distinct-reward positive control, and require the due-height blocks to succeed.
//! Uses the real native engine and checked-in system bytecode; local synthetic data only.

use pocol_lp4_execution::{native, output, CHAIN_ID, pay::{registry_address, escrow_address}};
use revm::{DatabaseCommit, InMemoryDB, primitives::{Address, AccountInfo, Bytecode, Bytes, TxEnv, TxKind, U256, keccak256, ResultAndState}};

const PCOL: u128 = 1_000_000_000_000_000_000;
const U: u64 = 40;          // cfg(27): deregister -> sweep due offset, from the test config below
const H_REG: u64 = 5;       // height at which the attacker deploys
const DUE: u64 = H_REG + U; // 45: first height the sweep acts on the exited record

fn b() -> U256 { U256::from(20 * PCOL) }
fn system() -> Address { Address::from_word((U256::from(1) << 160u32).wrapping_sub(U256::from(2)).into()) }
fn caps() -> Address { Address::from_word(U256::from(0xc0c004u64).into()) }
fn name(n: &str) -> Address { Address::from_slice(&keccak256(format!("earnin:{n}"))[12..]) }
fn program(n: &str) -> Bytecode {
    let j: serde_json::Value = serde_json::from_str(include_str!("data/system-earnin.json")).unwrap();
    let p = j["programs"].as_array().unwrap().iter().find(|p| p["name"] == n).unwrap();
    let s = p["runtime"].as_str().unwrap().trim_start_matches("0x");
    Bytecode::new_raw(Bytes::from((0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect::<Vec<_>>()))
}
fn insert(db: &mut InMemoryDB, a: Address, balance: U256, nonce: u64, code: Bytecode) {
    db.insert_account_info(a, AccountInfo::new(balance, nonce, code.hash_slow(), code));
}
fn slot(id: Address) -> U256 { U256::from_be_bytes(keccak256([id.into_word().as_slice(), U256::from(7).to_be_bytes::<32>().as_slice()].concat()).0) }
fn base(n: u64) -> U256 { U256::from_be_bytes(keccak256(U256::from(n).to_be_bytes::<32>()).0) }
fn storage(db: &InMemoryDB, a: Address, k: U256) -> U256 { db.accounts.get(&a).and_then(|x| x.storage.get(&k).copied()).unwrap_or_default() }
fn put(db: &mut InMemoryDB, a: Address, k: U256, v: U256) { db.insert_account_storage(a, k, v).unwrap(); }
fn selector(s: &str) -> Vec<u8> { keccak256(s.as_bytes())[..4].to_vec() }

/// Earn-in genesis: Registry/Escrow/Caps programs, a continuous CP (cfg 47/48 = 0), one HELD genesis
/// member m0 (so the active count can exceed M_min=1), plus any extra durable accounts.
fn genesis(extra_durable: &[Address]) -> InMemoryDB {
    let mut db = InMemoryDB::default();
    insert(&mut db, system(), U256::ZERO, 1, Bytecode::default());
    for (a, n) in [(registry_address(), "MinerRegistry"), (escrow_address(), "RewardEscrow"), (caps(), "ChainCaps")] {
        insert(&mut db, a, U256::ZERO, 1, program(n));
    }
    for n in 0..52 { put(&mut db, caps(), U256::from(n), U256::ZERO); }
    // cfg: 11=D(settle offset) 16=B 17=reg-ops 18=active-cap 19=M_min 20=rec-cap 21=deletes 22=scans
    //      23=ops 24=reactivate-delay 25=inactive-delay 26=tomb-grace 27=exit->due 50=close-batch
    for (n, v) in [(11u64, 4u128), (16, 20 * PCOL), (17, 16), (18, 64), (19, 1), (20, 8192), (21, 16), (22, 64), (23, 64), (24, 32), (25, 64), (26, 16), (27, U as u128), (50, 64)] {
        put(&mut db, caps(), U256::from(n), U256::from(v));
    }
    let m0 = name("m0");
    insert(&mut db, m0, U256::ZERO, 1, Bytecode::default());
    let r = slot(m0);
    for (k, v) in [
        (U256::ZERO, U256::from(1)), (U256::from(1), U256::from(1)), (U256::from(4), b()),
        (base(5), U256::from_be_slice(m0.as_slice())), (base(6), U256::from_be_slice(m0.as_slice())),
        (r, U256::from(1) | (U256::from(1) << 16u32)), (r + U256::from(1), U256::from_be_slice(m0.as_slice())), (r + U256::from(2), b()),
    ] { put(&mut db, registry_address(), k, v); }
    db.accounts.get_mut(&escrow_address()).unwrap().info.balance = b();
    for a in extra_durable { insert(&mut db, *a, U256::ZERO, 1, Bytecode::default()); }
    db
}

fn exec(db: &mut InMemoryDB, tx: TxEnv, h: u64) -> ResultAndState {
    let r = native::execute_at(db, tx, U256::from(h)).unwrap();
    db.commit(r.state.clone());
    r
}
fn nonce_of(db: &InMemoryDB, a: Address) -> u64 { db.accounts.get(&a).map(|x| x.info.nonce).unwrap_or(0) }
fn system_call(db: &mut InMemoryDB, sig: &str, h: u64) -> ResultAndState {
    let tx = TxEnv { caller: system(), transact_to: TxKind::Call(registry_address()), data: Bytes::from(selector(sig)), value: U256::ZERO,
        gas_limit: 30_000_000, gas_price: U256::ZERO, nonce: Some(nonce_of(db, system())), chain_id: Some(CHAIN_ID), ..Default::default() };
    exec(db, tx, h)
}
/// One block tick at height h: markClose() then beginBlock(). Returns the beginBlock result.
fn block(db: &mut InMemoryDB, h: u64) -> ResultAndState {
    let mc = system_call(db, "markClose()", h);
    assert!(mc.result.is_success(), "markClose({h}) unexpectedly failed: {:?}", mc.result);
    system_call(db, "beginBlock()", h)
}

// ----- attacker contract init code (EVM assembly, built in Rust; synthetic) -----
fn sel4(s: &str) -> [u8; 4] { let h = keccak256(s.as_bytes()); [h[0], h[1], h[2], h[3]] }
fn store_selector(c: &mut Vec<u8>, s: [u8; 4]) {
    c.push(0x63); c.extend_from_slice(&s);   // PUSH4 selector
    c.extend_from_slice(&[0x60, 0xE0, 0x1b]); // PUSH1 224 ; SHL  (left-align into mem[0..4])
    c.extend_from_slice(&[0x60, 0x00, 0x52]); // PUSH1 0 ; MSTORE
}
fn do_call(c: &mut Vec<u8>, value: u8, arglen: u8) {
    // CALL(gas, 0xC0C001, value, argOff=0, argLen, retOff=0, retLen=0) ; POP
    c.extend_from_slice(&[0x60, 0x00, 0x60, 0x00, 0x60, arglen, 0x60, 0x00, 0x60, value,
        0x62, 0xC0, 0xC0, 0x01, 0x5a, 0xf1, 0x50]);
}
enum Reward { SelfAddr, Fixed(Address) }
fn init_code(reward: Reward, destroy: bool) -> Vec<u8> {
    let mut c = Vec::new();
    // register(reward) {value: 1}
    store_selector(&mut c, sel4("register(address)"));
    match reward {
        Reward::SelfAddr => c.extend_from_slice(&[0x30, 0x60, 0x04, 0x52]),              // ADDRESS ; PUSH1 4 ; MSTORE
        Reward::Fixed(a) => { c.push(0x73); c.extend_from_slice(a.as_slice()); c.extend_from_slice(&[0x60, 0x04, 0x52]); } // PUSH20 D ; PUSH1 4 ; MSTORE
    }
    do_call(&mut c, 1, 0x24);
    // deregister() {value: 0}
    store_selector(&mut c, sel4("deregister()"));
    do_call(&mut c, 0, 0x04);
    if destroy { c.extend_from_slice(&[0x33, 0xff]); }          // CALLER ; SELFDESTRUCT
    else { c.extend_from_slice(&[0x60, 0x00, 0x60, 0x00, 0xf3]); } // PUSH1 0 ; PUSH1 0 ; RETURN (empty runtime, nonce stays 1)
    c
}
fn deploy(db: &mut InMemoryDB, attacker: Address, init: Vec<u8>, h: u64) -> ResultAndState {
    let tx = TxEnv { caller: attacker, transact_to: TxKind::Create, data: Bytes::from(init), value: U256::from(1u64),
        gas_limit: 30_000_000, gas_price: U256::ZERO, nonce: Some(nonce_of(db, attacker)), chain_id: Some(CHAIN_ID), ..Default::default() };
    exec(db, tx, h)
}
fn record(db: &InMemoryDB, id: Address) -> (U256, U256, U256, U256) {
    let r = slot(id);
    (storage(db, registry_address(), r), storage(db, registry_address(), r + U256::from(1)), storage(db, registry_address(), r + U256::from(2)), storage(db, registry_address(), r + U256::from(3)))
}
fn exists(db: &InMemoryDB, a: Address) -> bool {
    db.accounts.get(&a).map(|x| x.account_state != revm::db::AccountState::NotExisting && (x.info.nonce > 0 || x.info.code_hash != revm::primitives::KECCAK_EMPTY)).unwrap_or(false)
}

#[test]
fn constructor_self_reward_selfdestruct_cannot_create_a_halt_record() {
    let mut db = genesis(&[]);
    let attacker = name("attacker");
    insert(&mut db, attacker, U256::from(1000 * PCOL), 0, Bytecode::default());
    let x = attacker.create(0);
    assert!(block(&mut db, H_REG).result.is_success());
    let escrow_before = db.accounts[&escrow_address()].info.balance;
    let dep = deploy(&mut db, attacker, init_code(Reward::SelfAddr, true), H_REG);
    // The constructor ignores the failed CALL and may finish successfully. Its
    // registry effects, rather than the outer CREATE status, are the regression.
    assert!(dep.result.is_success());
    assert_eq!(record(&db, x), (U256::ZERO, U256::ZERO, U256::ZERO, U256::ZERO),
        "a non-durable self reward must never leave a registry/bond record");
    assert!(!exists(&db, x), "the constructor's SELFDESTRUCT still deletes the account");
    assert_eq!(db.accounts[&escrow_address()].info.balance, escrow_before,
        "the rejected registration must not transfer a bond");
    for h in [DUE - 1, DUE, DUE + 1, DUE + 2] {
        assert!(block(&mut db, h).result.is_success(), "no refund halt at height {h}");
    }
}

#[test]
fn control_durable_distinct_reward_continues() {
    // Same settings, but the reward is a durable distinct account D that stays present.
    let d = name("D");
    let mut db = genesis(&[d]);
    let attacker = name("attacker2");
    insert(&mut db, attacker, U256::from(1000 * PCOL), 0, Bytecode::default());
    let x = attacker.create(0);

    eprintln!("\n==== CONTROL A: durable distinct reward D (probe runs and passes) ====");
    assert!(block(&mut db, H_REG).result.is_success());
    let dep = deploy(&mut db, attacker, init_code(Reward::Fixed(d), true), H_REG);
    assert!(dep.result.is_success(), "deploy register(D)+deregister+selfdestruct: {:?} out={:02x?}", dep.result, output(&dep.result));
    let (_, w1, _, _) = record(&db, x);
    eprintln!("  X record reward = {:?} (durable D), X durable now? {}", Address::from_word(w1.into()), exists(&db, x));
    assert_eq!(Address::from_word(w1.into()), d, "reward is D");
    assert!(exists(&db, d), "D stays present");

    let r = block(&mut db, DUE);
    eprintln!("beginBlock({DUE}): success={} (refund pays durable D, record erased)", r.result.is_success());
    assert!(r.result.is_success(), "refund to a durable reward succeeds; no halt: {:?} out={:02x?}", r.result, output(&r.result));
    let (w0, _, _, _) = record(&db, x);
    assert_eq!(w0, U256::ZERO, "X record erased after a successful sweep");
    for h in [DUE + 1, DUE + 2, DUE + 3] {
        assert!(block(&mut db, h).result.is_success(), "blocks keep being produced at h={h}");
    }
    eprintln!("RESULT: durable reward -> refund succeeds, chain keeps producing blocks.");
}

#[test]
fn constructor_self_reward_is_rejected_even_if_the_contract_later_persists() {
    let mut db = genesis(&[]);
    let attacker = name("attacker3");
    insert(&mut db, attacker, U256::from(1000 * PCOL), 0, Bytecode::default());
    let x = attacker.create(0);
    assert!(block(&mut db, H_REG).result.is_success());
    let escrow_before = db.accounts[&escrow_address()].info.balance;
    let dep = deploy(&mut db, attacker, init_code(Reward::SelfAddr, false), H_REG);
    assert!(dep.result.is_success());
    assert!(exists(&db, x), "the non-destroyed contract persists with a nonce");
    assert_eq!(record(&db, x), (U256::ZERO, U256::ZERO, U256::ZERO, U256::ZERO),
        "durability is required at registration, not after the constructor finishes");
    assert_eq!(db.accounts[&escrow_address()].info.balance, escrow_before);
    for h in [DUE, DUE + 1, DUE + 2] {
        assert!(block(&mut db, h).result.is_success(), "blocks continue at height {h}");
    }
}
