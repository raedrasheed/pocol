//! SECURITY PROOF (defensive audit, in-session, synthetic only) for the late-launch ASERT concern.
//! A late start is not guarded at boot (no wall-clock vs g_ts check in node.rs). This test uses the
//! REAL consensus code compiled into pocold: asert::asert (lp1-node), timing::AttemptWindow, the PoW
//! rule types::le_target / pow_hash, and the real calibration gate::calibration_target (lp4). FN-4: the
//! timing numbers are the fresh network's final CP (deploy/mainnet/v3-profile.json, FRESH-NETWORK-CP.md:
//! t_blk=10, tau=600, d_att=90, m=32; `final_cp_constants_are_the_committed_profile` checks them against
//! the profile). The profile's hash rate is measured at the genesis ceremony; until then H_cal is the
//! synthetic (1<<18)/10 that `init-testnet --test-placeholders` fills in, and target_g is its R25d
//! calibration. A measured rate moves target_g, and with it the lateness at which the target collapses to
//! U256::MAX and the length of the near-free burst after a late start (a smaller target_g needs more
//! doublings); block 1's full difficulty, the d_att signing limit and the height at which full difficulty
//! returns after a late start do not depend on it. `launch_delay_across_admissible_hash_rates` checks both
//! kinds at eight rates spanning the ceremony's admissible range (1 to 2^53-1 H/s), and the ignored
//! `ceremony_filled_profile_record` prints the figures of one filled profile for the ceremony record
//! (deploy/mainnet/README.md, step 4). A simulated clock drives block timestamps through the
//! real AttemptWindow rule. Nothing here touches a live network.
//!
//! Proven facts:
//!  * on-time launch -> target stays at target_g (full difficulty) for the first blocks
//!  * target ~ target_g * 2^(dt/tau); it saturates to U256::MAX once the tip timestamp runs a
//!    computed amount ahead of schedule (the exact "~X hours late" number for these params)
//!  * block 1 is ALWAYS full difficulty (parent = genesis, dt = 0), whatever the wall clock
//!  * a start more than d_att=90 s late makes the signed attempt-0 proposer_timestamp() fail
//!    (OutsideWindow) -> the chain can only advance by fallback (empty-body) blocks
//!  * the PoW rule le_target(hash,target): at target_g a nonce passes with prob ~ target_g/2^256;
//!    at target=U256::MAX EVERY nonce passes (no work) -> and the share target target*m collapses too

use pocold::asert::{asert, AsertOut};
use pocold::fixed::U256 as F;
use pocold::timing::{AttemptWindow, TimingError};
use pocold::types::{from_fixed, le_target, pow_hash, to_fixed};
use revm::primitives::U256 as RU256;

const G_TS: i128 = 1_795_000_000; // synthetic launch time
const T_BLK: i128 = 10;
const TAU: i128 = 600;
const D_ATT: u32 = 90;
const M: u64 = 32;
const HASH_RATE: u64 = (1 << 18) / 10; // 26214, the test flag's synthetic H_cal (testnet.rs init)

/// FN-4: the constants above are the committed final profile's (T_blk, tau, D_att, m), and the profile's
/// hash rate is still the ceremony placeholder this test stands in for.
#[test]
fn final_cp_constants_are_the_committed_profile() {
    let p: serde_json::Value = serde_json::from_str(include_str!("../../../deploy/mainnet/v3-profile.json")).unwrap();
    let cp = &p["cp"];
    assert_eq!((cp["T_blk"].as_i64(), cp["tau"].as_i64(), cp["D_att"].as_u64(), cp["m"].as_u64()), (Some(T_BLK as i64), Some(TAU as i64), Some(u64::from(D_ATT)), Some(M)));
    assert!(p["hashRate"]["hashesPerSecond"].as_str().is_some_and(|s| s.starts_with("PLACEHOLDER")), "measured at the ceremony");
    let r: serde_json::Value = serde_json::from_str(include_str!("../../../deploy/mainnet/v3-rehearsal-profile.json")).unwrap();
    assert_eq!(r["cp"], p["cp"], "the rehearsal runs the same CP");
}

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
    assert_eq!(on_time, Ok((G_TS + 30) as u64), "a <=d_att-late start can still sign block 1");
    assert_eq!(w.proposer_timestamp((G_TS + i128::from(D_ATT)) as u64), Ok((G_TS + i128::from(D_ATT)) as u64), "the last second of the window");
    assert_eq!(w.proposer_timestamp((G_TS + i128::from(D_ATT) + 1) as u64), Err(TimingError::OutsideWindow), "one second past d_att");
    assert_eq!(late, Err(TimingError::OutsideWindow), "a >d_att-late start cannot sign at attempt 0");
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

/// target_g for a measured rate `rate` (R25d), through the real calibration.
fn target_g_at(rate: u64) -> F {
    let cal = pocol_lp4_execution::gate::calibration_target(rate, T_BLK as u32).unwrap();
    F::from_be_slice(&cal.to_be_bytes()).unwrap()
}
/// The least tip lateness dt (s) at which the target is U256::MAX, by bisection over [0, 48 h] (the target is
/// non-decreasing in dt); None when it does not saturate within 48 h.
fn collapse_after(tg: &F) -> Option<i128> {
    let max_at = |dt: i128| child(tg, G_TS + T_BLK + dt, 1).target == F::MAX;
    let (mut lo, mut hi) = (0i128, 48 * 3600i128);
    if !max_at(hi) { return None; }
    if max_at(lo) { return Some(0); }
    while hi - lo > 1 { let mid = (lo + hi) / 2; if max_at(mid) { hi = mid } else { lo = mid } }
    assert!(!max_at(hi - 1) && max_at(hi));
    Some(hi)
}
/// The late-start simulation of `late_start_burst_and_recovery` for target_g `tg` and a clock `late` seconds past
/// g_ts: (blocks of at most 256 expected hashes, blocks a single nonce meets with prob >= 1/2, the height at which the
/// target is back at full difficulty).
fn late_start(tg: &F, late: i128) -> (i128, i128, Option<i128>) {
    let clock = (G_TS + late) as u64;
    let d = D_ATT as i128;
    let (mut p_ts, mut p_h) = (G_TS, 0i128);
    let (mut trivial, mut free) = (0i128, 0i128);
    for h in 1..=4000i128 {
        let o = child(tg, p_ts, p_h);
        let bits = o.target.bits();
        if bits >= 248 { trivial += 1; }
        if bits == 256 { free += 1; }
        if o.trace.s == 0 && h > 1 { return (trivial, free, Some(h)); }
        let w = AttemptWindow::new(p_ts as u64, 0, d as u32).unwrap();
        let ts = match w.proposer_timestamp(clock) {
            Ok(t) => t as i128,
            Err(_) => { let a = (((clock as i128) - p_ts - 1) / d).max(1); p_ts + a * d + 1 }
        };
        p_ts = ts; p_h = h;
    }
    (trivial, free, None)
}

/// The facts above at eight rates spanning the ceremony's admissible range (Initialize-PoColNetwork.ps1: 1 to
/// 2^53-1 H/s; sampled, not every rate): at each, on schedule the target stays at target_g, block 1 is at full
/// difficulty, the d_att limit holds, the target collapses within 24 h of tip lateness and later than at the rate
/// before it, and after a 3 h late start full difficulty returns at the same height at all eight. The near-free
/// burst's length is printed: it shrinks as the rate rises (none at the sampled 1e9 and 2^53-1 H/s).
#[test]
fn launch_delay_across_admissible_hash_rates() {
    let rates: [u64; 8] = [1, 1_000, 26_214, 1_000_000, 4_000_000, 10_000_000, 1_000_000_000, (1u64 << 53) - 1];
    eprintln!("\n==== LATE LAUNCH ACROSS ADMISSIBLE HASH RATES (t_blk={T_BLK}, tau={TAU}, d_att={D_ATT}) ====");
    eprintln!("  {:>16} {:>9} {:>14} {:>22} {:>9}", "H/s", "tg bits", "collapse (h)", "near-free after 3 h", "recovery");
    let w = AttemptWindow::new(G_TS as u64, 0, D_ATT).unwrap();
    assert_eq!(w.proposer_timestamp((G_TS + i128::from(D_ATT)) as u64), Ok((G_TS + i128::from(D_ATT)) as u64));
    assert_eq!(w.proposer_timestamp((G_TS + i128::from(D_ATT) + 1) as u64), Err(TimingError::OutsideWindow));
    let (mut last_collapse, mut recovery) = (None::<i128>, None::<Option<i128>>);
    for rate in rates {
        let tg = target_g_at(rate);
        for h in 1..=6i128 {
            let o = child(&tg, G_TS + T_BLK * (h - 1), h - 1);
            assert_eq!((o.target == tg, o.trace.s), (true, 0), "{rate} H/s: on schedule block {h} stays at target_g");
        }
        assert_eq!(child(&tg, G_TS, 0).target, tg, "{rate} H/s: block 1 is at full difficulty");
        let collapse = collapse_after(&tg).unwrap_or_else(|| panic!("{rate} H/s: no collapse within 48 h"));
        assert!(collapse <= 24 * 3600, "{rate} H/s: the target collapses within 24 h of lateness ({collapse} s)");
        if let Some(prev) = last_collapse { assert!(collapse > prev, "{rate} H/s: a higher rate collapses later ({collapse} <= {prev})"); }
        last_collapse = Some(collapse);
        let (trivial, free, rec) = late_start(&tg, 3 * 3600);
        assert!(rec.is_some(), "{rate} H/s: full difficulty returns after a 3 h late start");
        match recovery { None => recovery = Some(rec), Some(r) => assert_eq!(rec, r, "{rate} H/s: the recovery height does not depend on the rate") }
        eprintln!("  {rate:>16} {:>9} {:>14.2} {:>14} ({free:>4} free) {:>9}", tg.bits(), collapse as f64 / 3600.0, trivial, rec.unwrap());
    }
    // The committed synthetic rate reproduces late_start_burst_and_recovery's run.
    assert!(late_start(&target_g(), 3 * 3600).0 >= 100);
}

/// The ceremony record (deploy/mainnet/README.md, step 4): run on the filled profile after `init-testnet`, from a
/// development checkout at the release commit, never on a founder device:
/// `POCOL_FILLED_PROFILE=<absolute path of the filled profile> cargo +1.99.1 test --release --locked --manifest-path
/// prototypes/pocold/Cargo.toml --test launch_delay_asert -- --ignored --nocapture` (cargo runs tests from
/// prototypes/pocold, so a relative path resolves there). The profile holds only public
/// values. It checks that the profile runs the committed final CP on a fresh-network chain id with a measured rate,
/// and prints target_g, the collapse threshold, the near-free burst after a 3 h late start and the recovery height.
#[test]
#[ignore = "ceremony record: POCOL_FILLED_PROFILE=<path>, run with --ignored --nocapture"]
fn ceremony_filled_profile_record() {
    let path = std::env::var_os("POCOL_FILLED_PROFILE").expect("set POCOL_FILLED_PROFILE to the ceremony's filled profile");
    let p: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", std::path::Path::new(&path).display()))).unwrap();
    let committed: serde_json::Value = serde_json::from_str(include_str!("../../../deploy/mainnet/v3-profile.json")).unwrap();
    assert_eq!(p["cp"], committed["cp"], "the filled profile runs the committed final CP");
    let chain_id = p["chainId"].as_u64().expect("chainId");
    assert!(chain_id == 1_656_550_731 || chain_id == 2_656_089_850, "a fresh-network chain id (rehearsal or mainnet): {chain_id}");
    assert_eq!(p["hashRate"]["class"], "measurement");
    let rate = p["hashRate"]["hashesPerSecond"].as_u64().expect("a measured integer hash rate, not a placeholder");
    assert!((1..=(1u64 << 53) - 1).contains(&rate), "the ceremony's range 1 to 2^53-1: {rate}");
    assert!(!p.to_string().contains("PLACEHOLDER"), "no placeholder is left");
    let tg = target_g_at(rate);
    let collapse = collapse_after(&tg).expect("the target collapses within 48 h");
    let (trivial, free, rec) = late_start(&tg, 3 * 3600);
    eprintln!("{}", serde_json::json!({"chainId": chain_id, "hashesPerSecond": rate, "targetGBits": tg.bits(),
        "collapseAfterSeconds": collapse, "lateStart3h": {"nearFreeBlocks": trivial, "freeBlocks": free, "fullDifficultyAtHeight": rec}}));
}
