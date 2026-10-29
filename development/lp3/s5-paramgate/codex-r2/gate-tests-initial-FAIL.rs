use lp1_node::gate::*;
use lp1_node::param_gate::RpcLimits;
use lp1_node::profile::{ConsensusParams, NodeKind, ProfileKind};
use lp1_node::{fixed::U256, genesis_spec as gs, hashes::sha256, hex};

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
        },
        Calibration { hashes_per_second: 1000, duration_seconds: 60, artifact },
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
    assert_eq!(v.violations, ["R22", "R24"]);
    assert!(verdict(&e, NodeKind::Simnet).violations.contains(&"R24".to_string()));
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
        (2, |p| p.local.delta_rel = 25),
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
    assert_eq!(reject_unknown_fields(&v, &["profile"]), Err("sysOk".into()));
}
