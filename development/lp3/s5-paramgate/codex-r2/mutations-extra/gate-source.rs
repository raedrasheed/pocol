//! LP3 amendment 01: numeric, profile-bound resource evaluator. Never a boot permit.
use crate::fixed::{U256, U512};
use crate::genesis_spec as gs;
use crate::hashes::{sha256, H256};
use crate::param_gate::{registry_metrics, RpcLimits};
use crate::profile::{ConsensusParams, NodeKind, ProfileKind};

pub const RESOURCE_MODEL: &str = "pocol-lp3-numeric-facts-v1";
#[derive(Clone, Debug)]
pub struct LocalParams {
    pub delta: u32,
    pub phi: u32,
    pub capital_phi: u32,
    pub t_exec: u32,
    pub delta_rel: u32,
    pub gamma: u32,
    pub x_h: u64,
    pub x_w: u64,
    pub r_keep: u32,
    pub k_hist: u32,
    pub k_final: u32,
    pub k_view: u32,
    pub hist_trie_budget: u64,
    /// tx/template/block/share/evidence1/evidence2 byte limits, network.md order.
    pub gossip: [u32; 6],
    pub retention_stress: bool,
}
#[derive(Clone, Debug)]
pub struct GateProfile {
    pub id: String,
    pub cp: ConsensusParams,
    pub genesis_pre: Vec<u8>,
    pub genesis_hash: H256,
    pub chain_id: u64,
    pub kind: ProfileKind,
    pub forks: Vec<(u16, u64)>,
    pub local: LocalParams,
    pub rpc: RpcLimits,
}
impl GateProfile {
    /// Canonical fixed-width domain-separated binding of every profile/CP/local/RPC field.
    /// NodeKind is an evaluation context, not a resource-driving profile field.
    pub fn binding(&self) -> H256 {
        let mut b = b"PoColLP3Profile-v1".to_vec();
        fn bytes(b: &mut Vec<u8>, s: &[u8]) {
            b.extend_from_slice(&(s.len() as u64).to_be_bytes());
            b.extend_from_slice(s);
        }
        bytes(&mut b, self.id.as_bytes());
        bytes(&mut b, &self.genesis_pre);
        b.extend_from_slice(&self.genesis_hash);
        b.extend_from_slice(&self.chain_id.to_be_bytes());
        b.push(match self.kind {
            ProfileKind::Network => 0,
            ProfileKind::Simnet => 1,
            ProfileKind::Synthetic => 2,
        });
        for v in self.cp.to_values() {
            b.extend_from_slice(&v.to_be_bytes());
        }
        b.extend_from_slice(&(self.forks.len() as u64).to_be_bytes());
        for (v, h) in &self.forks {
            b.extend_from_slice(&v.to_be_bytes());
            b.extend_from_slice(&h.to_be_bytes());
        }
        let l = &self.local;
        for n in [
            l.delta as u64,
            l.phi as u64,
            l.capital_phi as u64,
            l.t_exec as u64,
            l.delta_rel as u64,
            l.gamma as u64,
            l.x_h,
            l.x_w,
            l.r_keep as u64,
            l.k_hist as u64,
            l.k_final as u64,
            l.k_view as u64,
            l.hist_trie_budget,
        ] {
            b.extend_from_slice(&n.to_be_bytes());
        }
        for n in l.gossip {
            b.extend_from_slice(&n.to_be_bytes());
        }
        b.push(l.retention_stress as u8);
        let r = &self.rpc;
        for n in [
            r.conn_max,
            r.req_buf,
            r.conn_buf,
            r.send_sock_buf,
            r.heavy_active,
            r.light_active,
            r.light_queue,
            r.heavy_resp_mem,
            r.light_resp_mem,
            r.resp_max,
            r.light_resp_max,
            r.batch_resp_max,
            r.scratch_heavy,
            r.scratch_light,
            r.mem_total,
            r.batch_max,
            r.err_item_max,
            r.logs_count_max,
            r.logs_scan_bytes,
            r.rate,
            r.bucket_cap,
            r.queue_wait_ms,
            r.compute_heavy_ms,
            r.cancel_max_ms,
            r.grace_ms,
            r.min_send_rate,
            r.batch_wall_ms,
            r.enforce_slack_ms,
            r.timer_eps_ms,
        ] {
            b.extend_from_slice(&n.to_be_bytes());
        }
        sha256(&b)
    }
}
/// Classification is provenance, not verification of an external proof or a boot right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FactClass {
    SyntheticFixture,
    Measurement,
    ProvenBound,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Ordinary,
    Closing,
    Final,
}
#[derive(Clone, Debug)]
pub struct PhaseFacts {
    pub phase: Phase,
    pub system_keys: u32,
    pub user_keys: u32,
    pub system_accounts: u32,
    pub user_accounts: u32,
    pub live_slots: u32,
    pub call_gas: Vec<u64>,
    pub duration_ms: u64,
    pub retired_records: u32,
}
#[derive(Clone, Debug)]
pub struct ResourceFacts {
    pub profile_binding: H256,
    pub model: String,
    pub class: FactClass,
    /// Nonzero digest of the reproducible fixture, measurement trace or proof artifact.
    pub artifact: H256,
    pub phases: Vec<PhaseFacts>,
    pub validate_p99_ms: u64,
    pub historical_trie_bytes: u64,
    pub retained_blocks: u64,
}
#[derive(Clone, Debug)]
pub struct IdentityFacts {
    pub profile_binding: H256,
    pub model: String,
    pub class: FactClass,
    pub artifact: H256,
    pub alloc_root: H256,
    pub sys_code_hash: H256,
    pub alloc_balances: Vec<U256>,
}
#[derive(Clone, Debug)]
pub struct Calibration {
    pub profile_binding: H256,
    pub model: String,
    pub class: FactClass,
    pub hashes_per_second: u64,
    pub duration_seconds: u64,
    pub artifact: H256,
}
#[derive(Clone, Debug)]
pub struct GateReport {
    /// Unique R1..R26 in numeric order. Missing/mismatched facts also violate their rule.
    pub violations: Vec<String>,
    pub missing: Vec<&'static str>,
    pub resource_class: Option<FactClass>,
    pub identity_class: Option<FactClass>,
    pub calibration_class: Option<FactClass>,
    pub deferred: [&'static str; 3],
}
impl GateReport {
    pub fn accepted(&self) -> bool {
        self.violations.is_empty() && self.missing.is_empty()
    }
    pub fn bootable(&self) -> bool {
        false
    }
}
pub fn calibration_target(hashes_per_second: u64, t_blk: u32) -> Option<U256> {
    let denominator = hashes_per_second as u128 * t_blk as u128;
    if denominator == 0 {
        return None;
    }
    let mut n = U512::ZERO;
    n.0[4] = 1;
    let mut d = U512::ZERO;
    d.0[0] = denominator as u64;
    d.0[1] = (denominator >> 64) as u64;
    let q = n.div_rem(&d)?.0;
    if q.bits() > 256 {
        Some(U256::MAX)
    } else {
        Some(q.to_u256()?.checked_sub(&U256::ONE).unwrap_or(U256::ONE).max(U256::ONE))
    }
}
fn add512(a: &mut U512, b: &U512) {
    let mut carry = 0u128;
    for i in 0..8 {
        let s = a.0[i] as u128 + b.0[i] as u128 + carry;
        a.0[i] = s as u64;
        carry = s >> 64;
    }
    // At most usize::MAX balances of 256 bits plus two 320-bit terms: < 512 bits.
    debug_assert_eq!(carry, 0);
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputError {
    UnknownField(String),
    ExpectedObject,
}
/// Input-adapter guard for an explicit schema: typed check() itself has no unknown fields.
pub fn reject_unknown_fields(value: &serde_json::Value, allowed: &[&str]) -> Result<(), InputError> {
    let object = value.as_object().ok_or(InputError::ExpectedObject)?;
    for k in object.keys() {
        if !allowed.contains(&k.as_str()) {
            return Err(InputError::UnknownField(k.clone()));
        }
    }
    Ok(())
}

pub fn check(
    p: &GateProfile,
    resource: Option<&ResourceFacts>,
    identity: Option<&IdentityFacts>,
    calibration: Option<&Calibration>,
    node: NodeKind,
) -> GateReport {
    let c = &p.cp;
    let l = &p.local;
    let t = l.t_exec as u128;
    let phi = l.capital_phi as u128;
    let d = l.delta as u128;
    let m = crate::timing::metrics(l.delta as u64, l.capital_phi as u64, l.t_exec as u64, l.delta_rel as u64, c.dFbWait as u64);
    let mut bad = [false; 26];
    let mut missing = Vec::new();
    bad[0] = phi != 2 * l.phi as u128;
    bad[1] = (l.delta_rel as u128) < m.sigma + d + 2 * t;
    bad[2] = (c.dFbWait as u128) < phi + d + 2 * t;
    bad[3] = phi >= c.dFbWait as u128 || (l.delta_rel as u128) < t;
    bad[4] = 3 * c.dFbWait as u128 > c.D_att as u128;
    bad[5] = l.x_h as u128 != m.x_h || l.x_w as u128 != m.x_w;
    bad[6] = (c.D_att as u128 + l.gamma as u128) < 2 * (m.x_w + c.c as u128 * c.T_blk as u128) + phi;
    bad[7] = c.D < 2 || (c.U as u128) < c.E_win as u128 + c.D as u128 || c.E_win > l.r_keep;
    bad[8] = (c.S_max as u128) < 4 * c.kappa as u128 * c.m as u128 || c.B_code_max < 24577;
    bad[9] = (c.BODY_MAX as u128) < c.TX_MAX as u128 + 16384;
    bad[10] = l.k_final > l.r_keep || l.k_view >= l.k_hist || l.k_view as u128 + 2 > 512;
    let spec = gs::verify_hash(&p.genesis_pre, &p.genesis_hash).ok();
    let members = spec.as_ref().map_or(0, |s| s.m0_list.len());
    bad[11] = c.M_min < 1 || members < c.M_min as usize || members > c.M_max as usize;
    bad[12] = c.H_END <= c.Z_close as u64 || c.H_CLOSE as u128 + c.Z_close as u128 > c.H_END as u128 || c.T_CLOSE > c.T_END;
    let binding = p.binding();
    let rf = resource.filter(|r| r.profile_binding == binding && r.model == RESOURCE_MODEL && r.artifact != [0; 32]);
    if rf.is_none() {
        missing.push("resourceFacts:missing-or-binding-model-artifact-mismatch");
    }
    let phases = rf.filter(|r| {
        r.phases.len() == 3 && [Phase::Ordinary, Phase::Closing, Phase::Final].iter().all(|p| r.phases.iter().filter(|f| f.phase == *p).count() == 1)
    });
    if rf.is_some() && phases.is_none() {
        missing.push("resourceFacts:phase-coverage");
    }
    bad[13] = rf.map_or(true, |r| r.validate_p99_ms as u128 > t * 1000);
    // network.md byte limits. Template carries BODY_MAX plus the fixed 686-byte envelope.
    let required = [c.TX_MAX as u128, c.BODY_MAX as u128 + 686, 3067, 43, 1368, 243];
    bad[14] = l.gossip.iter().zip(required.iter()).any(|(limit, need)| (*limit as u128) < *need);
    let reg = registry_metrics(c.U, c.E_win, c.D, c.M_max, c.R_max, c.REG_RECORDS_MAX, c.RET, c.P2);
    bad[15] = reg.as_ref().map_or(true, |r| (c.REG_RECORDS_MAX as u128) < r.records_required);
    bad[16] = (c.SWEEP_MAX as u128) < c.R_max as u128 + c.E_max as u128;
    bad[17] = phases.map_or(true, |r| {
        r.phases.iter().any(|f| {
            f.live_slots > c.SYS_SLOT_MAX
                || f.system_keys as u128 + f.user_keys as u128 > c.SYS_KEYS_BLOCK_MAX as u128
                || f.user_keys > c.SYS_KEYS_USER_MAX
                || f.system_accounts as u128 + f.user_accounts as u128 > c.SYS_ACCT_BLOCK_MAX as u128
                || f.user_accounts > 16
                || (f.phase != Phase::Ordinary && (f.user_keys != 0 || f.user_accounts != 0))
                || f.call_gas.is_empty()
                || f.call_gas.iter().any(|g| *g > 30_000_000)
                || f.call_gas.iter().map(|g| *g as u128).sum::<u128>() > c.G_SYS as u128
                || (f.phase == Phase::Closing && f.retired_records < c.RET)
        })
    });
    bad[18] = phases.map_or(true, |r| r.phases.iter().any(|f| f.duration_ms as u128 > t * 1000));
    bad[19] = reg.as_ref().map_or(true, |r| (c.Z_close as u128) < r.close_blocks_required);
    bad[20] = rf.map_or(true, |r| {
        r.retained_blocks < (l.k_hist as u64).min(c.H_END)
            || (r.historical_trie_bytes > l.hist_trie_budget && !(p.kind == ProfileKind::Simnet && node == NodeKind::Simnet && l.retention_stress))
    });
    bad[21] = (node == NodeKind::Node && p.kind != ProfileKind::Network) || ((p.kind == ProfileKind::Network) != (p.chain_id == 777001));
    let id = identity.filter(|i| i.profile_binding == binding && i.model == RESOURCE_MODEL && i.artifact != [0; 32]);
    if id.is_none() {
        missing.push("identityFacts:missing-or-binding-model-artifact-mismatch");
    }
    bad[22] = id.map_or(true, |i| {
        let mut total = U512::ZERO;
        for balance in &i.alloc_balances {
            add512(&mut total, &U512::from_u256(balance));
        }
        add512(&mut total, &U512::mul_u256_u64(&c.B_reg, members as u64));
        add512(&mut total, &U512::mul_u256_u64(&c.subsidy, c.H_END));
        total.bits() > 127
    });
    bad[23] = spec.as_ref().map_or(true,|s|s.chain_id!=p.chain_id || s.cp!=c.to_values() || id.map_or(true,|i|s.alloc_root!=i.alloc_root || s.sys_code_hash!=i.sys_code_hash))
        || p.forks.first()!=Some(&(1,0)) || p.forks.windows(2).any(|w|w[0].1>=w[1].1)
        // Only v1 is implemented: TestFork-v2 is rejected even in simnet until implemented.
        || p.forks.iter().any(|(v,_)|*v!=1) || p.chain_id==0;
    let cal = calibration.filter(|r| r.profile_binding == binding && r.model == RESOURCE_MODEL && r.artifact != [0; 32]);
    if cal.is_none() {
        missing.push("calibrationFacts:missing-or-binding-model-artifact-mismatch");
    }
    bad[24] = c.target_g.is_zero()
        || c.T_blk == 0
        || c.tau == 0
        || (p.kind == ProfileKind::Network && (c.tau as u128) < 10 * c.T_blk as u128)
        || cal.map_or(true, |r| {
            r.hashes_per_second == 0 || r.duration_seconds == 0 || r.artifact == [0; 32] || calibration_target(r.hashes_per_second, c.T_blk) != Some(c.target_g)
        });
    bad[25] = !p.rpc.satisfies_r26();
    GateReport {
        violations: bad.iter().enumerate().filter(|(_, b)| **b).map(|(i, _)| format!("R{}", i + 1)).collect(),
        missing,
        resource_class: rf.map(|r| r.class),
        identity_class: id.map(|i| i.class),
        calibration_class: cal.map(|i| i.class),
        deferred: ["LP4:general-R18", "LP4:general-R21", "LP4:caps64-minimum-SYS_SLOT_MAX"],
    }
}
