use lp1_node::gate::*;
use lp1_node::param_gate::RpcLimits;
use lp1_node::profile::{ConsensusParams, NodeKind, ProfileKind};
use lp1_node::{fixed::{U256, U512}, genesis_spec as gs, hashes::sha256, hex};

fn profile(end: bool) -> GateProfile {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../fixtures/profile.json")).unwrap();
    let mut s = gs::decode(&hex::decode(fixture["genesis_pre_hex"].as_str().unwrap()).unwrap()).unwrap();
    let mut c = ConsensusParams::from_values(&s.cp).unwrap();
    c.T_blk = if end { 10 } else { 60 };
    c.tau = if end { 600 } else { 3600 };
    c.nonceMode = if end { 1 } else { 0 };
    c.D_att = if end { 60 } else { 300 };
    c.dFbWait = if end { 6 } else { 25 };
    c.subsidy = if end { U256::from_u64(80000) } else { U256::from_u64(2_000_000_000_000_000_000) };
    c.H_END = if end { 700 } else { 129600 };
    c.Z_close = if end { 147 } else { 160 };
    c.H_CLOSE = c.H_END - c.Z_close as u64;
    c.T_END = c.g_ts + 10368000;
    c.T_CLOSE = c.T_END - 9600;
    c.target_g = calibration_target(1000, c.T_blk).unwrap();
    if end {
        c.M_max = 1024;
        c.R_max = 0;
        c.REG_RECORDS_MAX = 1024;
        c.RET = 8;
    }
    s.m0_list = (1..=if end { 1024u32 } else { 4u32 })
        .map(|i| {
            let mut id = [0; 20];
            id[16..].copy_from_slice(&i.to_be_bytes());
            gs::Member { id, reward_addr: id }
        })
        .collect();
    s.chain_id = if end { 777905 } else { 777001 };
    s.cp = c.to_values();
    let b = gs::encode(&s);
    GateProfile {
        id: if end { "end/full-synthetic" } else { "network-synthetic" }.into(),
        cp: c,
        genesis_hash: gs::genesis_hash(&b),
        genesis_pre: b,
        chain_id: s.chain_id,
        kind: if end { ProfileKind::Simnet } else { ProfileKind::Network },
        forks: vec![(1, 0)],
        local: LocalParams {
            delta: if end { 1 } else { 5 },
            phi: if end { 1 } else { 5 },
            capital_phi: if end { 2 } else { 10 },
            t_exec: if end { 1 } else { 3 },
            delta_rel: if end { 8 } else { 26 },
            gamma: if end { 5 } else { 30 },
            x_h: if end { 6 } else { 22 },
            x_w: if end { 10 } else { 36 },
            r_keep: 2048,
            k_hist: 32,
            k_final: 60,
            k_view: 12,
            hist_trie_budget: 1717986918,
            gossip: [131072, 197294, 3067, 43, 1368, 243],
            retention_stress: false,
        },
        rpc: RpcLimits::default(),
    }
}
fn refresh(p: &mut GateProfile) {
    let mut s = gs::decode(&p.genesis_pre).unwrap();
    s.cp = p.cp.to_values();
    s.chain_id = p.chain_id;
    p.genesis_pre = gs::encode(&s);
    p.genesis_hash = gs::genesis_hash(&p.genesis_pre);
}
fn facts(p: &GateProfile) -> (ResourceFacts, IdentityFacts, Calibration) {
    let s = gs::decode(&p.genesis_pre).unwrap();
    let artifact = sha256(b"explicit synthetic resource fixture, not measurement or proof");
    let phases = [Phase::Ordinary, Phase::Closing, Phase::Final]
        .iter()
        .map(|phase| PhaseFacts {
            phase: *phase,
            system_keys: if *phase == Phase::Ordinary { 1496 } else { 1395 },
            user_keys: if *phase == Phase::Ordinary { 256 } else { 0 },
            system_accounts: if *phase == Phase::Ordinary { 388 } else { 257 },
            user_accounts: if *phase == Phase::Ordinary { 16 } else { 0 },
            live_slots: 100000,
            call_gas: vec![13_000_000, 13_000_000],
            duration_ms: 500,
            retired_records: p.cp.RET,
        })
        .collect();
    (
        ResourceFacts {
            profile_binding: p.binding(),
            model: RESOURCE_MODEL.into(),
            class: FactClass::SyntheticFixture,
            artifact,
            phases,
            validate_p99_ms: 500,
            historical_trie_bytes: 1000000,
            retained_blocks: 32,
        },
        IdentityFacts {
            profile_binding: p.binding(),
            model: RESOURCE_MODEL.into(),
            class: FactClass::SyntheticFixture,
            artifact,
            alloc_root: s.alloc_root,
            sys_code_hash: s.sys_code_hash,
            alloc_balances: vec![U256::from_u64(1_000_000_000_000_000_000)],
            // The CP schema v2 fixtures stand for the v3 program set (the only set that reads CP schema v2's fields).
            m3_issuance: p.cp.schema_version() >= 2,
        },
        Calibration {
            profile_binding: p.binding(),
            model: RESOURCE_MODEL.into(),
            class: FactClass::SyntheticFixture,
            hashes_per_second: 1000,
            duration_seconds: 60,
            artifact,
        },
    )
}
fn verdict(p: &GateProfile, node: NodeKind) -> GateReport {
    let (r, i, c) = facts(p);
    check(p, Some(&r), Some(&i), Some(&c), node)
}
#[test]
fn pg_end_1_to_5_and_l() {
    let mut n = profile(false);
    assert!(verdict(&n, NodeKind::Node).accepted());
    n.cp.H_CLOSE = 129584;
    refresh(&mut n);
    assert_eq!(verdict(&n, NodeKind::Node).violations, ["R13"]);
    n = profile(false);
    n.cp.Z_close = 146;
    n.cp.H_CLOSE = n.cp.H_END - 146;
    refresh(&mut n);
    assert_eq!(verdict(&n, NodeKind::Node).violations, ["R20"]);
    let mut e = profile(true);
    assert!(verdict(&e, NodeKind::Simnet).accepted());
    assert_eq!(verdict(&e, NodeKind::Node).violations, ["R22"]);
    e.cp.R_max = 2;
    refresh(&mut e);
    assert_eq!(verdict(&e, NodeKind::Simnet).violations, ["R16"]);
    e = profile(true);
    e.cp.Z_close = 146;
    e.cp.H_CLOSE = 554;
    refresh(&mut e);
    assert_eq!(verdict(&e, NodeKind::Simnet).violations, ["R20"]);
}
#[test]
fn resource_binding_complete_version_and_missing_fail_closed() {
    let p = profile(false);
    let (r, i, c) = facts(&p);
    assert!(!check(&p, None, Some(&i), Some(&c), NodeKind::Node).accepted());
    assert!(!check(&p, Some(&r), None, Some(&c), NodeKind::Node).accepted());
    assert!(!check(&p, Some(&r), Some(&i), None, NodeKind::Node).accepted());
    for field in 0..52 {
        let mut q = p.clone();
        let mut v = q.cp.to_values();
        if let Some(n) = v[field].to_u64() {
            v[field] = U256::from_u64(n.saturating_add(1));
        } else {
            v[field] = U256::ONE;
        }
        if let Ok(cp) = ConsensusParams::from_values(&v) {
            q.cp = cp;
            assert_ne!(q.binding(), p.binding());
            assert!(!check(&q, Some(&r), Some(&i), Some(&c), NodeKind::Node).accepted());
        }
    }
    for mutate in [0, 1, 2, 3, 4, 5, 6, 7, 8] {
        let mut bad = r.clone();
        match mutate {
            0 => bad.profile_binding = [0; 32],
            1 => bad.model.push('x'),
            2 => bad.artifact = [0; 32],
            3 => {
                bad.phases.pop();
            }
            4 => bad.phases[1].phase = Phase::Ordinary,
            5 => bad.phases[1].retired_records -= 1,
            6 => bad.retained_blocks = 31,
            7 => bad.phases[0].call_gas.clear(),
            _ => bad.phases[1].user_keys = 1,
        };
        assert!(!check(&p, Some(&bad), Some(&i), Some(&c), NodeKind::Node).accepted(), "{}", mutate);
    }
    let mut q = p.clone();
    q.id.push('x');
    assert_ne!(p.binding(), q.binding());
    q = p.clone();
    q.rpc.timer_eps_ms += 1;
    assert_ne!(p.binding(), q.binding());
    q = p.clone();
    q.local.k_hist += 1;
    assert_ne!(p.binding(), q.binding());
    q = p.clone();
    q.forks.push((1, 100));
    assert_ne!(p.binding(), q.binding());
}
#[test]
fn numeric_sys_timing_trie_and_identity_negatives() {
    let p = profile(false);
    let (r, i, c) = facts(&p);
    for change in 0..10 {
        let mut bad = r.clone();
        let rule = match change {
            0 => {
                bad.validate_p99_ms = 3001;
                "R14"
            }
            1 => {
                bad.phases[0].live_slots = p.cp.SYS_SLOT_MAX + 1;
                "R18"
            }
            2 => {
                bad.phases[0].system_keys = p.cp.SYS_KEYS_BLOCK_MAX;
                "R18"
            }
            3 => {
                bad.phases[0].user_keys = 257;
                "R18"
            }
            4 => {
                bad.phases[0].system_accounts = 513;
                "R18"
            }
            5 => {
                bad.phases[0].call_gas = vec![30_000_001];
                "R18"
            }
            6 => {
                bad.phases[0].call_gas = vec![30_000_000; 4];
                "R18"
            }
            7 => {
                bad.phases[2].duration_ms = 3001;
                "R19"
            }
            8 => {
                bad.historical_trie_bytes = p.local.hist_trie_budget + 1;
                "R21"
            }
            _ => {
                bad.phases[0].user_accounts = 17;
                "R18"
            }
        };
        assert_eq!(check(&p, Some(&bad), Some(&i), Some(&c), NodeKind::Node).violations, vec![rule]);
    }
    let mut bad = i.clone();
    bad.alloc_balances = vec![U256::pow2(127).unwrap()];
    assert_eq!(check(&p, Some(&r), Some(&bad), Some(&c), NodeKind::Node).violations, ["R23"]);
    bad = i.clone();
    bad.alloc_root = [0; 32];
    assert_eq!(check(&p, Some(&r), Some(&bad), Some(&c), NodeKind::Node).violations, ["R24"]);
    bad = i.clone();
    bad.sys_code_hash = [0; 32];
    assert_eq!(check(&p, Some(&r), Some(&bad), Some(&c), NodeKind::Node).violations, ["R24"]);
}
#[test]
fn production_profile_and_version_restrictions_and_boot_disabled() {
    let mut e = profile(true);
    e.forks.push((2, 50));
    let v = verdict(&e, NodeKind::Node);
    assert_eq!(v.violations, ["R22"]);
    assert!(verdict(&e, NodeKind::Simnet).accepted());
    e = profile(true);
    e.kind = ProfileKind::Synthetic;
    assert_eq!(verdict(&e, NodeKind::Node).violations, ["R22"]);
    let mut n = profile(false);
    n.local.retention_stress = true;
    let (mut r, i, c) = facts(&n);
    r.historical_trie_bytes = u64::MAX;
    assert_eq!(check(&n, Some(&r), Some(&i), Some(&c), NodeKind::Node).violations, ["R21"]);
    e = profile(true);
    e.local.retention_stress = true;
    let (mut r, i, c) = facts(&e);
    r.historical_trie_bytes = u64::MAX;
    assert!(check(&e, Some(&r), Some(&i), Some(&c), NodeKind::Simnet).accepted());
    for class in [FactClass::SyntheticFixture, FactClass::Measurement, FactClass::ProvenBound] {
        let p = profile(false);
        let (mut r, mut i, c) = facts(&p);
        r.class = class;
        i.class = class;
        let v = check(&p, Some(&r), Some(&i), Some(&c), NodeKind::Node);
        assert!(v.accepted());
        assert!(!v.bootable());
        assert_eq!(v.resource_class, Some(class));
        assert_eq!(v.deferred.len(), 3);
    }
}
#[test]
fn each_rule_has_a_negative_vector_and_numeric_order() {
    let changes: &[(usize, fn(&mut GateProfile))] = &[
        (1, |p| p.local.phi = 4),
        (2, |p| p.local.delta_rel = 24),
        (3, |p| p.cp.dFbWait = 20),
        (4, |p| p.local.delta_rel = 2),
        (5, |p| p.cp.D_att = 74),
        (6, |p| p.local.x_h = 21),
        (7, |p| p.local.gamma = 0),
        (8, |p| p.cp.U = 2000),
        (9, |p| p.cp.B_code_max = 24576),
        (10, |p| p.cp.TX_MAX = p.cp.BODY_MAX),
        (11, |p| p.local.k_view = 32),
        (12, |p| p.cp.M_min = 5),
        (13, |p| p.cp.H_CLOSE = p.cp.H_END),
        (15, |p| p.local.gossip[5] = 242),
        (16, |p| p.cp.REG_RECORDS_MAX = 12577),
        (17, |p| p.cp.SWEEP_MAX = 5),
        (20, |p| p.cp.Z_close = 146),
        (22, |p| p.kind = ProfileKind::Synthetic),
        (24, |p| p.forks.clear()),
        (25, |p| p.cp.tau = 599),
        (26, |p| p.rpc.batch_wall_ms = 404000),
    ];
    for (rule, change) in changes {
        let mut p = profile(false);
        change(&mut p);
        refresh(&mut p);
        assert!(verdict(&p, NodeKind::Node).violations.contains(&format!("R{}", rule)), "{}", rule);
    }
    let mut p = profile(false);
    p.local.phi = 0;
    p.cp.U = 0;
    p.cp.SWEEP_MAX = 0;
    refresh(&mut p);
    let v = verdict(&p, NodeKind::Node);
    assert_eq!(v.violations, ["R1", "R8", "R17"]);
    let v = serde_json::json!({"profile":"fixture","sysOk":true});
    assert_eq!(reject_unknown_fields(&v, &["profile"]), Err(InputError::UnknownField("sysOk".into())));
    assert_eq!(reject_unknown_fields(&serde_json::json!(null), &["profile"]), Err(InputError::ExpectedObject));
}

#[test]
fn complete_local_rpc_and_identity_binding_controls() {
    let p = profile(false);
    let (r, i, c) = facts(&p);
    macro_rules! local { ($($f:ident),*)=>{$(let mut q=p.clone();q.local.$f+=1;assert_ne!(q.binding(),p.binding(),stringify!($f));assert!(!check(&q,Some(&r),Some(&i),Some(&c),NodeKind::Node).accepted());)*}; }
    local!(delta, phi, capital_phi, t_exec, delta_rel, gamma, x_h, x_w, r_keep, k_hist, k_final, k_view, hist_trie_budget);
    macro_rules! rpc { ($($f:ident),*)=>{$(let mut q=p.clone();q.rpc.$f+=1;assert_ne!(q.binding(),p.binding(),stringify!($f));assert!(!check(&q,Some(&r),Some(&i),Some(&c),NodeKind::Node).accepted());)*}; }
    rpc!(
        conn_max,
        req_buf,
        conn_buf,
        send_sock_buf,
        heavy_active,
        light_active,
        light_queue,
        heavy_resp_mem,
        light_resp_mem,
        resp_max,
        light_resp_max,
        batch_resp_max,
        scratch_heavy,
        scratch_light,
        mem_total,
        batch_max,
        err_item_max,
        logs_count_max,
        logs_scan_bytes,
        rate,
        bucket_cap,
        queue_wait_ms,
        compute_heavy_ms,
        cancel_max_ms,
        grace_ms,
        min_send_rate,
        batch_wall_ms,
        enforce_slack_ms,
        timer_eps_ms
    );
    for field in 0..6 {
        let mut q = p.clone();
        q.local.gossip[field] += 1;
        assert_ne!(q.binding(), p.binding());
    }
    let mut q = p.clone();
    q.local.retention_stress = true;
    assert_ne!(q.binding(), p.binding());
    let mut q = p.clone();
    q.genesis_hash[0] ^= 1;
    assert_ne!(q.binding(), p.binding());
    let mut q = p.clone();
    q.genesis_pre.push(0);
    assert_ne!(q.binding(), p.binding());
    let mut q = p.clone();
    q.chain_id += 1;
    assert_ne!(q.binding(), p.binding());
    for change in 0..3 {
        let mut bad = c.clone();
        match change {
            0 => bad.profile_binding = [0; 32],
            1 => bad.model.push('x'),
            _ => bad.artifact = [0; 32],
        };
        assert!(!check(&p, Some(&r), Some(&i), Some(&bad), NodeKind::Node).accepted());
    }
    for change in 0..3 {
        let mut bad = i.clone();
        match change {
            0 => bad.profile_binding = [0; 32],
            1 => bad.model.push('x'),
            _ => bad.artifact = [0; 32],
        };
        assert!(!check(&p, Some(&r), Some(&bad), Some(&c), NodeKind::Node).accepted());
    }
}

#[test]
fn exact_supply_calibration_and_timing_boundaries() {
    assert_eq!(calibration_target(1, 1), Some(U256::MAX));
    assert_eq!(calibration_target(0, 1), None);
    let mut p = profile(false);
    p.cp.subsidy = U256::ZERO;
    p.cp.B_reg = U256::ZERO;
    refresh(&mut p);
    let (mut r, mut i, c) = facts(&p);
    i.alloc_balances = vec![U256::pow2(127).unwrap().checked_sub(&U256::ONE).unwrap()];
    r.validate_p99_ms = 3000;
    for f in &mut r.phases {
        f.duration_ms = 3000;
    }
    assert!(check(&p, Some(&r), Some(&i), Some(&c), NodeKind::Node).accepted());
    i.alloc_balances.push(U256::ONE);
    assert_eq!(check(&p, Some(&r), Some(&i), Some(&c), NodeKind::Node).violations, ["R23"]);
    i.alloc_balances = vec![U256::MAX; 4];
    assert_eq!(check(&p, Some(&r), Some(&i), Some(&c), NodeKind::Node).violations, ["R23"]);
    for forks in [vec![], vec![(1, 1)], vec![(1, 0), (1, 0)], vec![(1, 0), (1, 10), (1, 9)], vec![(1, 0), (65535, 20)],
        vec![(1, 0), (3, 20)], vec![(1, 0), (2, 20), (2, 30)], vec![(1, 0), (2, 20), (1, 30)]] {
        let mut q = profile(false);
        q.forks = forks;
        assert!(verdict(&q, NodeKind::Node).violations.contains(&"R24".into()));
    }
}

/// The network-synthetic profile on CP schema v2 (the v3 registry programs), with the given W_act, U and C_react, its
/// genesis a specVersion-2 GenesisSpec (SPEC_VERSION 2 decodes it); the R8 premises below are evaluated from the CP.
fn profile_v2(w_act: u32, u: u32, c_react: u32) -> GateProfile {
    let mut p = profile(false);
    p.cp.W_act = w_act;
    p.cp.U = u;
    let mut values = p.cp.to_values();
    for f in &gs::CP_SCHEMA_V2[gs::CP_LEN..] {
        values.push(U256::from_u64(match f.name {
            "R_react" => 4,
            "C_react" => c_react as u64,
            _ => f.lo,
        }));
    }
    p.cp = ConsensusParams::from_values(&values).unwrap();
    assert_eq!((p.cp.schema_version(), p.cp.C_react), (2, c_react));
    let mut s = gs::decode(&p.genesis_pre).unwrap();
    s.spec_version = 2;
    s.cp = p.cp.to_values();
    p.genesis_pre = gs::encode(&s);
    p.genesis_hash = gs::genesis_hash(&p.genesis_pre);
    p
}

/// V3-09 (V3-DECISIONS.md D-V3-09): on CP schema v2, R8 also requires U >= W_act + 2 (the reactivation window before
/// the refund sweep holds at least two blocks) and C_react >= U. CP schema v1 profiles keep their R8 unchanged.
#[test]
fn r8_v3_registry_window_and_cooldown() {
    let base = profile(false);
    let (w, u) = (base.cp.W_act, base.cp.U);
    assert!(u >= w + 2 && !verdict(&base, NodeKind::Node).violations.contains(&"R8".to_string()), "fixture W_act {w} U {u}");
    // Without facts: R8 depends on the CP and local parameters alone (the facts of a spec-2 genesis cannot be built
    // where it does not decode).
    let r8 = |p: &GateProfile| check(p, None, None, None, NodeKind::Node).violations.contains(&"R8".to_string());
    // Boundaries on CP schema v2.
    for (w_act, c_react, bad) in [(u - 2, u, false), (u - 1, u, true), (u, u, true), (u - 2, u - 1, true), (u - 2, u + 1000, false), (0, u, false)] {
        let p = profile_v2(w_act, u, c_react);
        assert_eq!(r8(&p), bad, "W_act {w_act} U {u} C_react {c_react}");
        assert_eq!(v3_registry_window_violated(&p.cp), bad);
    }
    // The same W_act on CP schema v1 is not an R8 matter (no C_react, earn-in programs).
    let mut p = profile(false);
    p.cp.W_act = u;
    refresh(&mut p);
    assert!(!r8(&p), "a CP schema v1 profile keeps its R8");
    assert!(!v3_registry_window_violated(&p.cp));
    // A violation of the original R8 terms is still R8 on schema v2.
    let mut p = profile_v2(u - 2, u, u);
    p.cp.D = 1;
    assert!(r8(&p));
}

/// V3-02 (V3-DECISIONS.md D-V3-02): on CP schema v2, R10 (the byte limits) also requires that the largest kind-3 late-share item a block
/// can need, min(S_max, kappa*m - 1) nonces, fits EVID_BYTES_MAX. CP schema v1 profiles keep their R10 unchanged.
#[test]
fn r10_v3_late_share_item_fits_evid_bytes_max() {
    // Exact RLP sizes: [3, be64(j), [k eight-byte nonces]].
    // outer header + kind byte + (0x88 + j) + nonce list header + 9 bytes per nonce.
    assert_eq!(late_item_bytes(1), 1 + 1 + 9 + 1 + 9);
    assert_eq!(late_item_bytes(6), 2 + 1 + 9 + 1 + 54, "the item payload passes 55 bytes first");
    assert_eq!(late_item_bytes(7), 2 + 1 + 9 + 2 + 63, "then the nonce list's");
    assert_eq!(late_item_bytes(63), 3 + 1 + 9 + 3 + 567, "about 0.6 KB at kappa*m = 64");
    assert_eq!(late_item_bytes(256), 3 + 1 + 9 + 3 + 2304, "about 2.3 KB at S_max = 256");
    let u = profile(false).cp.U;
    let r10 = |p: &GateProfile| check(p, None, None, None, NodeKind::Node).violations.contains(&"R10".to_string());
    let base = profile_v2(u - 2, u, u);
    let k = (base.cp.S_max as u64).min(base.cp.kappa as u64 * base.cp.m as u64 - 1);
    let need = late_item_bytes(k) as u32;
    assert!(!r10(&base) && !v3_late_share_sizing_violated(&base.cp), "fixture EVID_BYTES_MAX {} >= {need}", base.cp.EVID_BYTES_MAX);
    for (evid, bad) in [(need, false), (need - 1, true), (need + 1, false)] {
        let mut p = profile_v2(u - 2, u, u);
        p.cp.EVID_BYTES_MAX = evid;
        assert_eq!((r10(&p), v3_late_share_sizing_violated(&p.cp)), (bad, bad), "EVID_BYTES_MAX {evid} for k = {k}");
    }
    // The same EVID_BYTES_MAX on CP schema v1 is not an R10 matter (no late shares).
    let mut p = profile(false);
    p.cp.EVID_BYTES_MAX = need - 1;
    refresh(&mut p);
    assert!(!r10(&p) && !v3_late_share_sizing_violated(&p.cp));
}

/// V3-19 (V3-DECISIONS.md D-V3-19): for a genesis running the v3 program set (CP schema v2, `m3_issuance`), R23's
/// issuance term is the M3 schedule's 2·S0·H_half + S_tail·2^64 (issuance(h) = max(k >= 256 ? 0 : S0 >> k, S_tail),
/// k = h / H_half, summed over every height), not CP 15 · H_END; with the allocations and the genesis bonds the total must
/// stay below 2^127. Exact boundaries for each term; CP schema v1, and a CP schema v2 genesis naming another program set,
/// keep CP 15 · H_END (u64::MAX blocks when continuous), as genesis_builder.rs does.
#[test]
fn r23_v3_m3_supply_bound() {
    let pow = |n: u32| U256::pow2(n).unwrap();
    let r23_set = |p: &GateProfile, alloc: Vec<U256>, m3: bool| {
        let i = IdentityFacts {
            profile_binding: p.binding(),
            model: RESOURCE_MODEL.into(),
            class: FactClass::SyntheticFixture,
            artifact: sha256(b"explicit synthetic identity fixture, not measurement or proof"),
            alloc_root: [0; 32],
            sys_code_hash: [0; 32],
            alloc_balances: alloc,
            m3_issuance: m3,
        };
        check(p, None, Some(&i), None, NodeKind::Node).violations.contains(&"R23".to_string())
    };
    let r23 = |p: &GateProfile, alloc: Vec<U256>| r23_set(p, alloc, true);
    let u = profile(false).cp.U;
    let v2 = |s0: U256, h_half: u64, tail: U256| {
        let mut p = profile_v2(u - 2, u, u);
        p.cp.subsidy = s0;
        p.cp.H_half = h_half;
        p.cp.S_tail = tail;
        p.cp.B_reg = U256::ZERO; // the bonds term is the members' B_reg, zero here whatever the genesis decodes to
        p
    };
    let below = |n: u32, less: u64| pow(n).checked_sub(&U256::from_u64(less)).unwrap();
    // The tail term alone: (2^63 - 1)·2^64 = 2^127 - 2^64.
    let p = v2(U256::ZERO, 1, below(63, 1));
    assert!(!r23(&p, vec![below(64, 1)]), "2^127 - 1");
    assert!(r23(&p, vec![pow(64)]), "2^127");
    // The halving term alone: 2·2^125·1 = 2^126.
    let p = v2(pow(125), 1, U256::ZERO);
    assert!(!r23(&p, vec![below(126, 1)]));
    assert!(r23(&p, vec![pow(126)]));
    // A long half-life: 2·2^62·(2^64 - 1) = 2^127 - 2^63.
    let p = v2(pow(62), u64::MAX, U256::ZERO);
    assert!(!r23(&p, vec![below(63, 1)]));
    assert!(r23(&p, vec![pow(63)]));
    // Both terms: 2·2^124 + 2^62·2^64 = 2^125 + 2^126.
    let p = v2(pow(124), 1, pow(62));
    assert!(!r23(&p, vec![below(125, 1)]));
    assert!(r23(&p, vec![pow(125)]));
    let t = future_issuance(&p.cp, false, true);
    let mut want = [0u64; 8];
    want[1] = (1 << 61) | (1 << 62);
    assert_eq!(t.0, want, "2^125 + 2^126");
    // The owner's values (V3-ROADMAP §5): S0 1 PCOL, H_half 4,207,680, S_tail S0/32, with the 4,000,000 PCOL genesis
    // allocation: far below the bound, and the term is exactly 2·S0·H_half + S_tail·2^64.
    let pcol = 1_000_000_000_000_000_000u128;
    let p = v2(U256::from_be_slice(&pcol.to_be_bytes()).unwrap(), 4_207_680, U256::from_be_slice(&(pcol / 32).to_be_bytes()).unwrap());
    assert!(!r23(&p, vec![U256::from_be_slice(&(4_000_000 * pcol).to_be_bytes()).unwrap()]));
    let t = future_issuance(&p.cp, false, true);
    let exact = 2 * pcol * 4_207_680 + (pcol / 32) * (1u128 << 64);
    assert_eq!((t.0[0], t.0[1], t.0[2..].iter().all(|x| *x == 0)), (exact as u64, (exact >> 64) as u64, true));
    // CP 15 = 2^111: on CP schema v1 the flat term 2^111·H_END (H_END 129,600 > 2^16) exceeds 2^127; on CP schema v2 with
    // H_half 1 and no tail the M3 term is 2^112.
    let mut q = profile(false);
    q.cp.subsidy = pow(111);
    q.cp.B_reg = U256::ZERO;
    refresh(&mut q);
    assert!(r23(&q, vec![U256::ZERO]), "CP schema v1 keeps CP 15 · H_END");
    assert!(r23_set(&q, vec![U256::ZERO], false), "CP schema v1 keeps CP 15 · H_END whatever the set");
    assert!(!r23(&v2(pow(111), 1, U256::ZERO), vec![U256::ZERO]), "the v3 set on CP schema v2 takes the M3 term");
    // A CP schema v2 genesis naming another program set (the continuous or v1 programs credit CP 15 per block): the gate
    // keeps CP 15 · H_END, the term genesis_builder.rs checks there, and the M3 term alone would have admitted it.
    let other = v2(pow(111), 1, U256::ZERO);
    assert!(other.cp.H_END > 1 << 16, "H_END {} keeps 2^111 · H_END above 2^127", other.cp.H_END);
    assert!(r23_set(&other, vec![U256::ZERO], false), "CP schema v2, another program set: CP 15 · H_END");
    assert_eq!(future_issuance(&other.cp, false, false), U512::mul_u256_u64(&pow(111), other.cp.H_END));
    assert_eq!(future_issuance(&other.cp, true, false), U512::mul_u256_u64(&pow(111), u64::MAX), "continuous: u64::MAX blocks");
}
