//! SECURITY PROOF (defensive audit, in-session, synthetic only) for the late-launch ASERT concern.
//! A late start is not guarded at boot (no wall-clock vs g_ts check in node.rs). This test uses the
//! REAL consensus code compiled into pocold: asert::asert (lp1-node), timing::AttemptWindow, the PoW
//! rule types::le_target / pow_hash, and the real calibration gate::calibration_target (lp4). All
//! numbers are the earn-in testnet's (testnet.rs: t_blk=10, tau=600, d_att=60, hash_rate=(1<<18)/10,
//! m=32; target_g = R25d calibration of that hash rate). A simulated clock drives block timestamps
//! through the real AttemptWindow rule. Nothing here touches a live network.
//!
//! Proven facts:
//!  * on-time launch -> target stays at target_g (full difficulty) for the first blocks
//!  * target ~ target_g * 2^(dt/tau); it saturates to U256::MAX once the tip timestamp runs a
//!    computed amount ahead of schedule (the exact "~X hours late" number for these params)
//!  * block 1 is ALWAYS full difficulty (parent = genesis, dt = 0), whatever the wall clock
//!  * a start more than d_att=60 s late makes the signed attempt-0 proposer_timestamp() fail
//!    (OutsideWindow) -> the chain can only advance by fallback (empty-body) blocks
//!  * the PoW rule le_target(hash,target): at target_g a nonce passes with prob ~ target_g/2^256;
//!    at target=U256::MAX EVERY nonce passes (no work) -> and the share target target*m collapses too

use pocold::asert::{asert, AsertOut};
use pocold::fixed::U256 as F;
use pocold::timing::{AttemptWindow, TimingError};
use pocold::types::{from_fixed, le_target, pow_hash, to_fixed};
use revm::primitives::U256 as RU256;

const G_TS: i128 = 1_795_000_000; // synthetic launch time (new_network_config example value)
const T_BLK: i128 = 10;
const TAU: i128 = 600;
const D_ATT: u32 = 60;
const M: u64 = 32;
const HASH_RATE: u64 = (1 << 18) / 10; // 26214, as testnet.rs sets it

fn target_g() -> F {
    let cal = pocol_lp4_execution::gate::calibration_target(HASH_RATE, T_BLK as u32).unwrap();
    F::from_be_slice(&cal.to_be_bytes()).unwrap()
}
/// ASERT target for the child of a parent with (parent_ts, parent_height).
fn child(tg: &F, p_ts: i128, p_h: i128) -> AsertOut {
    asert(tg, G_TS, T_BLK, TAU, p_ts, p_h).unwrap()
}

#[test]
fn on_time_schedule_keeps_full_difficulty() {
    let tg = target_g();
    eprintln!("\n==== ON-TIME LAUNCH (control) ====");
    eprintln!("target_g = {} bits (R25d calibration of {HASH_RATE} H/s, t_blk={T_BLK})", tg.bits());
    for h in 1..=6i128 {
        // parent on schedule: ts = g_ts + t_blk*(h-1), height h-1
        let o = child(&tg, G_TS + T_BLK * (h - 1), h - 1);
        eprintln!("  block {h}: dt={} s={} target_bits={} (== target_g? {})", o.trace.delta_t, o.trace.s, o.target.bits(), o.target == tg);
        assert_eq!(o.target, tg, "on schedule the target must stay at target_g");
        assert_eq!(o.trace.s, 0);
    }
    eprintln!("RESULT: on-time launch keeps full calibrated difficulty at every early block.");
}

#[test]
fn asert_saturation_threshold_for_earnin_params() {
    let tg = target_g();
    eprintln!("\n==== DIFFICULTY vs TIP LATENESS (real asert) ====");
    eprintln!("target ~ target_g * 2^(dt/tau), tau={TAU}. dt = parent_ts - g_ts - t_blk*parent_height.");
    // Growth: each +tau of dt adds ~1 bit, until saturation.
    for k in [0i128, 1, 5, 10, 15, 16, 17, 18, 20] {
        let dt = k * TAU;
        let o = child(&tg, G_TS + T_BLK * 1 + dt, 1); // parent height 1, dt = k*tau
        eprintln!("  dt={dt:>6}s ({:>5.2}h): s={:>3} target_bits={:>3} is_MAX={} early={:?}",
            dt as f64 / 3600.0, o.trace.s, o.target.bits(), o.target == F::MAX, o.early);
    }
    // Exact saturation boundary (scan dt by 1 s).
    let mut sat: Option<i128> = None;
    let mut dt = 0i128;
    while dt <= 24 * 3600 {
        if child(&tg, G_TS + T_BLK + dt, 1).target == F::MAX { sat = Some(dt); break; }
        dt += 1;
    }
    let sat = sat.expect("target must saturate within 24 h of lateness");
    eprintln!("SATURATION: target first becomes U256::MAX at dt = {sat} s = {:.3} h of tip-lateness.", sat as f64 / 3600.0);
    // Just below the boundary it is not yet MAX.
    assert!(child(&tg, G_TS + T_BLK + sat - 1, 1).target < F::MAX);
    assert_eq!(child(&tg, G_TS + T_BLK + sat, 1).target, F::MAX);
    eprintln!("RESULT: for the earn-in params, difficulty collapses to trivial once the tip timestamp");
    eprintln!("        runs ~{:.2} h ahead of its scheduled slot.", sat as f64 / 3600.0);
}

#[test]
fn first_block_always_full_difficulty_and_window_clamp() {
    let tg = target_g();
    eprintln!("\n==== BLOCK 1 DIFFICULTY + ATTEMPT-WINDOW CLAMP ====");
    // Block 1's parent is genesis (ts = g_ts, height 0) -> dt = 0 -> full difficulty, whatever the clock.
    let o = child(&tg, G_TS, 0);
    eprintln!("  block 1 (parent genesis): dt={} target_bits={} (== target_g? {})", o.trace.delta_t, o.target.bits(), o.target == tg);
    assert_eq!(o.target, tg, "block 1 is always at full difficulty");

    // The signed proposer's attempt-0 window is [g_ts+1, g_ts+d_att]. A clock past g_ts+d_att cannot sign.
    let w = AttemptWindow::new(G_TS as u64, 0, D_ATT).unwrap();
    let on_time = w.proposer_timestamp((G_TS + 30) as u64);
    let late = w.proposer_timestamp((G_TS + 3 * 3600) as u64);
    eprintln!("  proposer_timestamp(clock=g_ts+30s)  = {:?}", on_time);
    eprintln!("  proposer_timestamp(clock=g_ts+3h)   = {:?}", late);
    assert_eq!(on_time, Ok((G_TS + 30) as u64), "a <=60s-late start can still sign block 1");
    assert_eq!(late, Err(TimingError::OutsideWindow), "a >60s-late start cannot sign at attempt 0");
    eprintln!("RESULT: block 1 keeps full difficulty; a start >d_att={D_ATT}s late forces fallback (no signed block 1).");
}

#[test]
fn pow_and_share_acceptance_collapse() {
    let tg = target_g();
    let tid = [0x11u8; 32];
    eprintln!("\n==== PoW ACCEPTANCE RULE le_target(pow_hash(tid,nonce), target) ====");
    // At the calibrated target_g: a near-top hash fails; the zero hash passes. Acceptance prob ~ target_g/2^256.
    let hi = [0xffu8; 32];
    let lo = [0x00u8; 32];
    eprintln!("  target_g bits={} => a uniform hash passes with prob ~ 2^-{}", tg.bits(), 256 - tg.bits());
    assert!(!le_target(&hi, &tg), "a top hash must fail at target_g (real work needed)");
    assert!(le_target(&lo, &tg), "the zero hash passes at target_g");
    // A concrete mined example: scan nonces for one that meets target_g (proves work is required).
    let mut found = None;
    for n in 0u64..200_000 { if le_target(&pow_hash(&tid, n), &tg) { found = Some(n); break; } }
    eprintln!("  first nonce meeting target_g within 200k tries: {:?}", found);

    // At target = U256::MAX: EVERY hash passes, including the top hash -> no work at all.
    assert!(le_target(&hi, &F::MAX), "at target=U256::MAX even the top hash passes (no work)");
    let mut all = true;
    for n in 0u64..1000 { if !le_target(&pow_hash(&tid, n), &F::MAX) { all = false; break; } }
    eprintln!("  at target=U256::MAX, first 1000 nonces ALL accepted? {all}");
    assert!(all, "every nonce is a valid PoW solution at U256::MAX");

    // Share target t_share = target * m (chain.rs:442, clamped to MAX). It collapses with the target.
    let share = |t: F| -> F { to_fixed(from_fixed(&t).checked_mul(RU256::from(M)).unwrap_or(RU256::MAX)) };
    eprintln!("  share target bits: at target_g = {}  |  at U256::MAX = {}", share(tg).bits(), share(F::MAX).bits());
    assert_eq!(share(F::MAX), F::MAX, "share target also collapses to U256::MAX");
    eprintln!("RESULT: a trivial block target makes both the block PoW and the per-share target trivial.");
}

#[test]
fn late_start_burst_and_recovery() {
    let tg = target_g();
    let late: i128 = 3 * 3600; // 3 h late start
    let clock = (G_TS + late) as u64;
    eprintln!("\n==== LATE START (clock = g_ts + {:.1} h), simulated ====", late as f64 / 3600.0);

    // --- the first two blocks, via the real AttemptWindow rule ---
    // Block 1: parent = genesis. The signed attempt-0 window cannot reach the clock, so the block is
    // produced by fallback at the attempt whose window brings the timestamp up toward the clock.
    let p_ts = G_TS;
    let a = (((clock as i128) - p_ts - 1) / (D_ATT as i128)).max(0); // highest attempt with lower(a) <= clock
    let w0 = AttemptWindow::new(p_ts as u64, 0, D_ATT).unwrap();
    let signed_ok = w0.proposer_timestamp(clock).is_ok();
    let ts1 = p_ts + a * D_ATT as i128 + 1; // fallback_timestamp(a) = lower(a)
    let t1 = child(&tg, p_ts, 0);
    eprintln!("  block 1: signed@a0 possible={signed_ok}; produced as FALLBACK at attempt {a}, ts=g_ts+{}s", ts1 - G_TS);
    eprintln!("           block-1 target_bits={} (== target_g? {}) -> FULL difficulty still required", t1.target.bits(), t1.target == tg);
    assert!(!signed_ok, "cannot sign block 1 from a 3h-late clock");
    assert_eq!(t1.target, tg, "block 1 is still full difficulty");

    // Block 2: parent = block 1 (late timestamp). dt is now huge -> target collapses.
    let t2 = child(&tg, ts1, 1);
    eprintln!("  block 2: parent ts=g_ts+{}s, dt={} s={} target_bits={} (eased by ~2^{})", ts1 - G_TS, t2.trace.delta_t, t2.trace.s, t2.target.bits(), t2.target.bits() as i32 - tg.bits() as i32);
    assert!(t2.target > tg, "block 2 target is drastically eased after the late block 1");

    // --- full catch-up simulation, block by block, with the real AttemptWindow rule and asert.
    //     Clock model: trivial-difficulty blocks mine ~instantly, so the wall clock stays at the
    //     late start value during the burst (the faithful worst case). A nonce meets a target of
    //     `bits` within ~2^(256-bits) hashes, so bits>=248 means <=256 expected hashes ("trivial")
    //     and bits==256 means a single nonce passes with prob >=1/2 ("free"). ---
    let d = D_ATT as i128;
    let (mut p_ts, mut p_h) = (G_TS, 0i128);
    let (mut trivial, mut free, mut signed_first, mut recover_h) = (0i128, 0i128, None, None);
    for h in 1..=4000i128 {
        let o = child(&tg, p_ts, p_h);
        let bits = o.target.bits();
        if bits >= 248 { trivial += 1; }
        if bits == 256 { free += 1; }
        let w = AttemptWindow::new(p_ts as u64, 0, d as u32).unwrap();
        let (ts, kind, att) = match w.proposer_timestamp(clock) {
            Ok(t) => { if signed_first.is_none() && h > 1 { signed_first = Some(h); } (t as i128, "signed  ", 0i128) }
            Err(_) => { let a = (((clock as i128) - p_ts - 1) / d).max(1); (p_ts + a * d + 1, "fallback", a) }
        };
        if h <= 3 || h % 200 == 0 { eprintln!("  h={h:>4} {kind} a={att:<3} ts=g_ts+{:>6}s dt={:>6} s={:>3} target_bits={bits}", ts - G_TS, o.trace.delta_t, o.trace.s); }
        if o.trace.s == 0 && h > 1 { recover_h = Some(h); break; }
        p_ts = ts; p_h = h;
    }
    eprintln!("  trivial-target blocks (<=256 expected hashes): {trivial}; of which 'free' (prob>=1/2): {free}");
    eprintln!("  first block a signed proposer could build: h={:?}; difficulty back to full at h={:?}", signed_first, recover_h);
    assert!(trivial >= 100, "a 3h-late start leaves a long run of trivial-difficulty blocks");
    eprintln!("RESULT: a >d_att-late start => block 1 full difficulty but FORCED FALLBACK; then a burst of");
    eprintln!("        ~{trivial} trivial-difficulty blocks (mineable in <=256 hashes) before difficulty recovers.");
}
