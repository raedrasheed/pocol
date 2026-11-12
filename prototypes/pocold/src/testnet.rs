//! Public-information-only genesis preparation for independently held test keys.
//! Still ExperimentalDevnet admission; does not enable the production network profile.
use pocol_lp4_execution::{
    admission::{self, NodeLocal, Request},
    apply::{Genesis, SystemPrograms},
    devnet,
    gate::FactClass,
    genesis_spec::{self as gs, GenesisSpec, Member},
    profile::{ConsensusParams, NodeKind, ProfileIdentity, ProfileKind},
};
use revm::{
    primitives::{AccountInfo, Address, U256},
    InMemoryDB,
};
use serde::Deserialize;
use std::{collections::{BTreeMap, HashSet}, path::Path};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Config {
    #[serde(default)]
    v3: bool,
    #[serde(default)]
    cp: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    continuous: bool,
    /// Earn-in network (EARN-IN-SPEC.md): earn-in system programs, B_reg = 20 PCOL, nonceMode 1. Needs
    /// `continuous`.
    #[serde(default)]
    earnin: bool,
    /// nonceMode 1: the full 64-bit nonce domain from genesis, so no attempt can run out of nonces (as
    /// Mainnet after its v2 height). Absent or false keeps the v1 bounded range and the same genesis bytes.
    #[serde(default)]
    full_nonce: bool,
    /// V3-05: the declared (or measured, V3-06 soak) WAN timing assumptions a v3 profile's local Δ_rel and Γ are
    /// derived from (`local_for_v3`). Absent: `TimingAssumptions::V3_WAN`. Refused without `v3`: spec-1 local
    /// timing stays the fixed `local_for` derivation.
    #[serde(default)]
    local_timing: Option<DeclaredTiming>,
    chain_id: u64,
    genesis_time: u64,
    registration_per_block: u8,
    miners: Vec<Miner>,
    accounts: Vec<Account>,
}
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeclaredTiming {
    delta: u32,
    phi: u32,
    t_exec: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Miner {
    id: Address,
    reward: Address,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Account {
    address: Address,
    wei: String,
}

pub fn local() -> NodeLocal {
    NodeLocal {
        delta: 1,
        phi: 1,
        t_exec: 1,
        delta_rel: 8,
        gamma: 5,
        ..crate::single_host_local()
    }
}

/// Synthetic P11 local assumptions, with wait/attempt slack derived from the final CP.
/// This does not measure WAN latency or select the production v3 profile. Spec-1 profiles (every committed
/// network, the live H15000 chain included) keep exactly this derivation: Δ_rel = dFbWait + Δ + t_exec (8 at
/// dFbWait 6) and Γ the R6 slack (5). V3-05 changes only spec-2 (v3) profiles (`local_for_v3`).
pub fn local_for(cp: &ConsensusParams) -> Result<NodeLocal, String> {
    let mut local = local();
    local.delta_rel = cp.dFbWait.checked_add(local.delta).and_then(|v| v.checked_add(local.t_exec))
        .ok_or("local timing exceeds u32")?;
    local.gamma = gamma_for(cp, &local)?;
    Ok(local)
}
/// Γ: the attempt-advance slack R6 needs at these locals (`D_att + Γ ≥ 2(X^W + c·T_blk) + Φ`), never below
/// `local().gamma` (5).
fn gamma_for(cp: &ConsensusParams, local: &NodeLocal) -> Result<u32, String> {
    let metrics = crate::timing::metrics(local.delta.into(), local.capital_phi().into(), local.t_exec.into(), local.delta_rel.into(), cp.dFbWait.into());
    let need = (2 * (metrics.x_w + u128::from(cp.c) * u128::from(cp.T_blk)) + u128::from(local.capital_phi()))
        .saturating_sub(u128::from(cp.D_att));
    Ok(self::local().gamma.max(u32::try_from(need).map_err(|_| "local timing exceeds u32")?))
}

/// V3-05: the timing assumptions a v3 profile's local Δ_rel is derived from, in seconds: Δ the delivery bound
/// between honest nodes, φ the clock-skew bound (Φ = 2φ) and t_exec the validation bound. Declared by the
/// operator (`localTiming` in the public testnet config, kept in devnet.json) or measured: the WAN soak (V3-06)
/// reads Δ and φ from `pocol_templateLatency` (network.md, "Template latency").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimingAssumptions {
    pub delta: u32,
    pub phi: u32,
    pub t_exec: u32,
}
impl TimingAssumptions {
    /// The synthetic single-host assumptions of `local()`: Δ = φ = t_exec = 1 s (Δ_rel 7 by V3-05; spec-1
    /// profiles keep 8, `local_for`).
    pub const SINGLE_HOST: Self = Self { delta: 1, phi: 1, t_exec: 1 };
    /// The WAN bounds the owner's O-6=b timing holds under (D-V3-06): measured Δ ≤ 2 s, φ ≤ 2 s, t_exec 2 s,
    /// so Δ_rel = 13 (V3-06 (b): σ = 7, R2 13, R3 dFbWait ≥ 10). The default for a v3 profile.
    pub const V3_WAN: Self = Self { delta: 2, phi: 2, t_exec: 2 };
    /// The largest value of each assumption accepted, in seconds (one hour): beyond it no block-time
    /// profile is meaningful, and the arithmetic stays far inside u32.
    pub const MAX_SECONDS: u32 = 3600;
    fn check(self) -> Result<Self, String> {
        if self.delta == 0 || self.t_exec == 0 {
            return Err("local timing: delta and tExec must be at least 1 s".into());
        }
        if [self.delta, self.phi, self.t_exec].iter().any(|v| *v > Self::MAX_SECONDS) {
            return Err(format!("local timing: each of delta, phi and tExec is at most {} s", Self::MAX_SECONDS));
        }
        Ok(self)
    }
}

/// V3-05 (V3-DESIGN, item V3-05): the default Δ_rel is derived from Δ, φ and t_exec as ParamGate R2 at equality,
/// Δ_rel = σ + Δ + 2·t_exec with σ = max(Δ, Φ + 1) + t_exec (`timing::metrics`), rather than lowered to a
/// single-host minimum. At `V3_WAN` it is 13; at the governance WAN locals Δ = 5, φ = 5, t_exec = 3, it is 25.
pub fn derive_delta_rel(a: TimingAssumptions) -> Result<u32, String> {
    let a = a.check()?;
    let m = crate::timing::metrics(a.delta.into(), u64::from(a.phi) * 2, a.t_exec.into(), 0, 0);
    let v = m.sigma + u128::from(a.delta) + 2 * u128::from(a.t_exec);
    u32::try_from(v).map_err(|_| "local timing exceeds u32".into())
}

/// V3-05: the local timing of a v3 (spec-2) profile: Δ, φ and t_exec as assumed, Δ_rel from
/// `derive_delta_rel`, Γ the R6 slack at those locals (`gamma_for`). Whether the CP then admits them (R3
/// `dFbWait ≥ Φ + Δ + 2·t_exec`, R6, ...) is decided by admission, as for every profile.
pub fn local_for_v3(cp: &ConsensusParams, a: TimingAssumptions) -> Result<NodeLocal, String> {
    let a = a.check()?;
    let mut local = NodeLocal { delta: a.delta, phi: a.phi, t_exec: a.t_exec, ..local() };
    local.delta_rel = derive_delta_rel(a)?;
    local.gamma = gamma_for(cp, &local)?;
    Ok(local)
}

/// The local timing for a genesis of `spec_version`: spec 1 is exactly `local_for` (declared assumptions are
/// refused, so every spec-1 network keeps its timing); spec 2 (v3) is `local_for_v3` at the declared
/// assumptions, `TimingAssumptions::V3_WAN` by default.
pub fn local_for_spec(cp: &ConsensusParams, spec_version: u8, declared: Option<TimingAssumptions>) -> Result<NodeLocal, String> {
    if spec_version < 2 {
        if declared.is_some() {
            return Err("localTiming is a v3 profile input; a spec-1 network keeps its fixed local timing".into());
        }
        return local_for(cp);
    }
    local_for_v3(cp, declared.unwrap_or(TimingAssumptions::V3_WAN))
}

/// devnet.json's `localTiming`: the declared assumptions (`delta`, `phi`, `tExec`) and the `deltaRel` and `gamma`
/// recorded with them, when present. None when the file or the object is absent.
struct Recorded {
    assumptions: TimingAssumptions,
    delta_rel: Option<u64>,
    gamma: Option<u64>,
}
fn declared_in(dir: &Path) -> Result<Option<Recorded>, String> {
    use std::io::Read;
    let file = match std::fs::File::open(dir.join("devnet.json")) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("devnet.json: {e}")),
    };
    const LIMIT: u64 = 1 << 20;
    let mut text = Vec::new();
    file.take(LIMIT + 1).read_to_end(&mut text).map_err(|e| format!("devnet.json: {e}"))?;
    if text.len() as u64 > LIMIT { return Err("devnet.json exceeds 1 MiB".into()); }
    let info: serde_json::Value = serde_json::from_slice(&text).map_err(|e| format!("devnet.json: {e}"))?;
    let Some(t) = info.get("localTiming") else { return Ok(None) };
    let field = |name: &str| t.get(name).and_then(serde_json::Value::as_u64).and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| format!("devnet.json localTiming.{name} must be an unsigned 32-bit integer"));
    let recorded = |name: &str| match t.get(name) {
        None => Ok(None),
        Some(v) => v.as_u64().map(Some).ok_or_else(|| format!("devnet.json localTiming.{name} must be an unsigned integer")),
    };
    Ok(Some(Recorded {
        assumptions: TimingAssumptions { delta: field("delta")?, phi: field("phi")?, t_exec: field("tExec")? },
        delta_rel: recorded("deltaRel")?,
        gamma: recorded("gamma")?,
    }))
}

/// The network's GenesisSpec (genesis.spec.hex, bounded read).
fn genesis_spec_in(dir: &Path) -> Result<gs::GenesisSpec, String> {
    use std::io::Read;
    let mut text = String::new();
    let limit = (gs::MAX_SPEC_BYTES * 2 + 3) as u64;
    std::fs::File::open(dir.join("genesis.spec.hex")).map_err(|e| format!("genesis.spec.hex: {e}"))?
        .take(limit + 1).read_to_string(&mut text).map_err(|e| format!("genesis.spec.hex: {e}"))?;
    if text.len() as u64 > limit { return Err("GenesisSpec exceeds size bound".into()); }
    let bytes = crate::types::unhex(text.trim())?;
    gs::decode(&bytes).map_err(|e| format!("GenesisSpec: {e:?}"))
}

/// The spec version of the network in `dir` (its genesis.spec.hex).
pub fn spec_version_of(dir: &Path) -> Result<u8, String> {
    Ok(genesis_spec_in(dir)?.spec_version)
}

/// The launcher uses the same final CP-derived local timing as the genesis builder: spec 1 `local_for`; a v3
/// genesis `local_for_v3` at the assumptions its devnet.json declares (`TimingAssumptions::V3_WAN` without one).
/// A v3 devnet.json whose recorded `deltaRel` or `gamma` differs from what V3-05 derives from its assumptions (a
/// file written before V3-05, or one edited by hand) is refused, so a node never runs a T_j other than the one the
/// file states.
pub fn local_for_network(dir: &Path) -> Result<NodeLocal, String> {
    let spec = genesis_spec_in(dir)?;
    let cp = ConsensusParams::from_values(&spec.cp).map_err(|e| format!("CP: {e:?}"))?;
    if spec.spec_version < 2 {
        return local_for(&cp);
    }
    let recorded = declared_in(dir)?;
    let local = local_for_spec(&cp, spec.spec_version, recorded.as_ref().map(|r| r.assumptions))?;
    if let Some(r) = recorded {
        let differs = |v: Option<u64>, want: u32| v.is_some_and(|v| v != u64::from(want));
        if differs(r.delta_rel, local.delta_rel) || differs(r.gamma, local.gamma) {
            let shown = |v: Option<u64>| v.map_or("absent".to_string(), |v| v.to_string());
            return Err(format!(
                "devnet.json localTiming records deltaRel {} and gamma {}, but V3-05 derives deltaRel {} and gamma {} from delta {}, phi {}, tExec {} \
                 (deltaRel = sigma + delta + 2 tExec); this v3 network's files predate V3-05 or were edited: rebuild them, or set deltaRel and gamma to the derived values",
                shown(r.delta_rel), shown(r.gamma), local.delta_rel, local.gamma, local.delta, local.phi, local.t_exec));
        }
    }
    Ok(local)
}

fn cp_number(value: &serde_json::Value) -> Result<pocol_lp4_execution::fixed::U256, String> {
    let number = if let Some(n) = value.as_u64() { U256::from(n) }
        else if let Some(text) = value.as_str().filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())) {
            text.parse::<U256>().map_err(|_| "CP value is outside uint256")?
        } else { return Err("CP value must be an unsigned integer or decimal string".into()); };
    Ok(devnet::to_fixed(number))
}

/// Resolve once from typed public input; cutover reads exactly this CP[16] before transforming funds.
fn resolve(config: &[u8], cutover: bool) -> Result<(Config, devnet::DevnetConfig, Vec<pocol_lp4_execution::fixed::U256>), String> {
    if config.len() > 262144 { return Err("testnet config exceeds 256 KiB".into()); }
    let mut input: Config = serde_json::from_slice(config).map_err(|_| "invalid public genesis config schema")?;
    if input.local_timing.is_some() && !input.v3 { return Err("localTiming is a v3 profile input; a spec-1 network keeps its fixed local timing".into()); }
    if input.v3 {
        if gs::SPEC_VERSION < 2 && !pocol_lp4_execution::V3_PREVIEW { return Err("v3 config requires a v3-preview build".into()); }
        input.earnin = true; input.continuous = true; input.full_nonce = true;
    }
    if input.chain_id == 777001 || input.chain_id == 0 || input.chain_id >= (1u64 << 53)
        || input.genesis_time > u64::MAX - 10_000_000
        || !(1..=if input.v3 { 256 } else { 64 }).contains(&input.miners.len())
        || input.accounts.len() > 10000 || input.registration_per_block > 4
        || (input.earnin && !input.continuous) || (cutover && !input.accounts.is_empty()) {
        return Err("invalid experimental testnet parameters".into());
    }
    let mut cfg = devnet::DevnetConfig::new(input.chain_id, input.genesis_time, &[]);
    cfg.spec_version = if input.v3 { 2 } else { 1 };
    cfg.t_blk = 10; cfg.tau = 600; cfg.d_att = 60; cfg.d_fb_wait = 6;
    cfg.hash_rate = (1 << 18) / 10; cfg.r_max = input.registration_per_block;
    cfg.earnin = input.earnin;
    if input.earnin || input.full_nonce { cfg.nonce_mode = 1; }
    let schema = gs::cp_schema(cfg.spec_version).unwrap();
    let mut overrides = BTreeMap::new();
    for (name, raw) in &input.cp {
        let index = schema.iter().position(|f| f.name == name).ok_or_else(|| format!("unknown CP field {name}"))?;
        if ["g_ts", "target_g", "H_END", "H_CLOSE", "T_END", "T_CLOSE"].contains(&name.as_str()) {
            return Err(format!("CP field {name} is derived by the builder"));
        }
        let value = cp_number(raw).map_err(|e| format!("CP {name}: {e}"))?;
        let field = &schema[index];
        if value.bits() > field.width || value < pocol_lp4_execution::fixed::U256::from_u64(field.lo) {
            return Err(format!("CP {name}: gsRange"));
        }
        if name == "T_blk" { cfg.t_blk = value.to_u64().unwrap() as u32; }
        overrides.insert(index, value);
    }
    if input.v3 {
        // Time-based synthetic defaults. Explicit CP overrides below take precedence.
        cfg.h_half = 126_230_400u64.div_ceil(u64::from(cfg.t_blk));
        cfg.t_prob = u32::try_from(2_592_000u64.div_ceil(u64::from(cfg.t_blk))).unwrap();
        cfg.e_epoch = u32::try_from(86_400u64.div_ceil(u64::from(cfg.t_blk))).unwrap();
    }
    let mut cp = cfg.cp();
    if input.continuous { for value in &mut cp[45..=48] { *value = pocol_lp4_execution::fixed::U256::ZERO; } }
    for (index, value) in overrides { cp[index] = value; }
    let params = ConsensusParams::from_values(&cp).map_err(|e| format!("CP {}: {}", e.detail, e.code))?;
    if (input.earnin || input.full_nonce) && params.nonceMode != 1 { return Err("nonceMode contradicts earnin/fullNonce/v3".into()); }
    Ok((input, cfg, cp))
}

pub(crate) fn cutover_params(config: &[u8]) -> Result<(Vec<Address>, U256), String> {
    let (input, _, cp) = resolve(config, true)?;
    if !input.earnin || !input.continuous { return Err("the new network config must set earnin and continuous, or v3".into()); }
    let mut seen = HashSet::new();
    for miner in &input.miners {
        if !allowed(miner.id) || !allowed(miner.reward) || !seen.insert(miner.id) { return Err("invalid or duplicate miner identity".into()); }
    }
    Ok((input.miners.iter().map(|m| m.reward).collect(), devnet::from_fixed(&cp[16])))
}

pub(crate) fn allowed(address: Address) -> bool {
    let numeric = U256::from_be_slice(address.as_slice());
    !address.is_zero()
        && address != pocol_lp4_execution::pay_address()
        && !(U256::from(0xc0c001u64)..=U256::from(0xc0c0ffu64)).contains(&numeric)
        && address != Address::from_word(((U256::from(1) << 160u32) - U256::from(2)).into())
}

pub fn prepare(config: &[u8], out: &Path) -> Result<(), String> {
    build(config, None, out)
}
/// Cutover genesis (EARN-IN-SPEC.md section 7): `alloc` is the allocation built from the old chain, and
/// `report` is written next to the genesis as cutover-report.json with the new genesis hash added. The
/// config must list no accounts; every balance comes from the old chain.
pub fn prepare_cutover(config: &[u8], alloc: InMemoryDB, report: serde_json::Value, out: &Path) -> Result<(), String> {
    build(config, Some((alloc, report)), out)
}
fn build(config: &[u8], cutover: Option<(InMemoryDB, serde_json::Value)>, out: &Path) -> Result<(), String> {
    if out.exists() {
        return Err("output must be a new directory; genesis is never overwritten".into());
    }
    let (input, cfg, cp) = resolve(config, cutover.is_some())?;
    let params = ConsensusParams::from_values(&cp).map_err(|e| format!("CP: {e:?}"))?;
    let (mut alloc, report) = match cutover {
        Some((alloc, report)) => (alloc, Some(report)),
        None => (InMemoryDB::default(), None),
    };
    let mut seen = HashSet::new();
    for account in &input.accounts {
        if !allowed(account.address)
            || !seen.insert(account.address)
            || account.wei.is_empty()
            || !account.wei.bytes().all(|b| b.is_ascii_digit())
        {
            return Err("invalid or duplicate allocation account".into());
        }
        let balance = account
            .wei
            .parse::<U256>()
            .map_err(|_| "invalid allocation amount")?;
        alloc.insert_account_info(
            account.address,
            AccountInfo {
                balance,
                ..Default::default()
            },
        );
    }
    seen.clear();
    let mut members = Vec::new();
    for miner in &input.miners {
        if !allowed(miner.id) || !allowed(miner.reward) || !seen.insert(miner.id) {
            return Err("invalid or duplicate miner identity".into());
        }
        // Preserve an existing reward account (its balance, and in a cutover its nonce, code and storage)
        // and give it at least the required durable nonce.
        let mut info = alloc
            .accounts
            .get(&miner.reward)
            .map(|a| a.info.clone())
            .unwrap_or_default();
        info.nonce = info.nonce.max(1);
        alloc.insert_account_info(miner.reward, info);
        members.push(Member {
            id: miner.id.0 .0,
            reward_addr: miner.reward.0 .0,
        });
    }
    members.sort_by_key(|m| m.id);
    let programs = if input.earnin {SystemPrograms::earnin()} else if input.continuous {SystemPrograms::continuous()} else {SystemPrograms::v1()};
    let mut spec = GenesisSpec {
        spec_version: cfg.spec_version,
        chain_id: cfg.chain_id,
        cp,
        alloc_root: [0; 32],
        m0_list: members,
        sys_code_hash: programs.sys_code_hash().0,
    };
    let identity = |spec: &GenesisSpec| {
        let bytes = gs::encode(spec);
        ProfileIdentity::validate(
            &bytes,
            &gs::genesis_hash(&bytes),
            spec.chain_id,
            ProfileKind::Simnet,
            NodeKind::Simnet,
            &[(1, 0)],
        )
        .map_err(|_| "invalid profile identity")
    };
    spec.alloc_root = Genesis::derive_root(&alloc, &identity(&spec)?, &programs)
        .map_err(|e| format!("genesis: {e:?}"))?
        .0;
    let profile = identity(&spec)?;
    // admit below performs the complete genesis/root validation; there is no unchecked boot path.
    let bytes = gs::encode(&spec);
    let timing = admission::measure(&bytes, &alloc, 1)?;
    let declared = input.local_timing.map(|t| TimingAssumptions { delta: t.delta, phi: t.phi, t_exec: t.t_exec });
    let local = local_for_spec(&params, cfg.spec_version, declared)?;
    let request = Request {
        spec_bytes: &bytes,
        alloc: &alloc,
        forks: &[(1, 0)],
        kind: ProfileKind::Simnet,
        node: NodeKind::Simnet,
        local: &local,
        calibration: Some((cfg.hash_rate, FactClass::SyntheticFixture)),
        timing: Some(&timing),
        scope: admission::Scope::ExperimentalDevnet,
    };
    let admitted = admission::admit(&request).map_err(|e| e.to_string())?;
    let info = serde_json::json!({"chainId":cfg.chain_id, "genesisHash":crate::types::hex(&profile.genesis_hash()),
        "genesisTime":cfg.g_ts, "profileKind":"simnet", "keyMode":"independent-key-files", "forks":[[1,0]],
        "note":"Experimental candidate only; independent keys do not establish public-network readiness.",
        "registrationPerBlock":params.R_max,"continuous":input.continuous,
        "miners":input.miners.iter().map(|m|serde_json::json!({"id":m.id,"reward":m.reward})).collect::<Vec<_>>(),
        "calibration":{"hashesPerSecond":cfg.hash_rate,"class":"synthetic-fixture"},
        "timing":timing.to_json(),"admission":crate::admission_json(&admitted)});
    let mut info = info;
    if input.earnin {
        info["earnin"] = serde_json::json!(true);
    }
    if input.full_nonce || (!input.cp.is_empty() && params.nonceMode == 1) {
        info["fullNonce"] = serde_json::json!(true);
    }
    if input.v3 {
        info["v3"] = serde_json::json!(true);
        info["specVersion"] = serde_json::json!(2);
    }
    if input.v3 || !input.cp.is_empty() {
        info["localTiming"] = serde_json::json!({"delta":local.delta,"phi":local.phi,"tExec":local.t_exec,"deltaRel":local.delta_rel,"gamma":local.gamma});
    }
    if input.v3 {
        // V3-05: where the assumptions came from; deltaRel = sigma + delta + 2 tExec, recomputed by the launcher.
        info["localTiming"]["source"] = serde_json::json!(if declared.is_some() { "declared" } else { "v3-wan-default" });
    }
    // Admission must succeed before any output. Partial I/O failures remain visible and are not overwritten.
    std::fs::create_dir(out).map_err(|e| format!("output directory: {e}"))?;
    for (name, contents) in [
        ("genesis.spec.hex", crate::types::hex(&bytes)),
        (
            "alloc.json",
            serde_json::to_string_pretty(&devnet::alloc_to_json(&alloc)).unwrap(),
        ),
        ("devnet.json", serde_json::to_string_pretty(&info).unwrap()),
    ] {
        std::fs::write(out.join(name), contents).map_err(|e| format!("write {name}: {e}"))?;
    }
    if let Some(mut report) = report {
        report["newGenesis"] = serde_json::json!({"chainId":cfg.chain_id,"genesisHash":crate::types::hex(&profile.genesis_hash()),"genesisTime":cfg.g_ts});
        std::fs::write(out.join("cutover-report.json"), serde_json::to_string_pretty(&report).unwrap())
            .map_err(|e| format!("write cutover-report.json: {e}"))?;
    }
    println!(
        "{}",
        serde_json::json!({"chainId":cfg.chain_id,"genesisHash":crate::types::hex(&profile.genesis_hash()),
        "scope":"experimental-devnet","publicLaunchReady":false})
    );
    Ok(())
}

pub fn init(args: &[String]) -> Result<(), String> {
    if args.len() != 6 {
        return Err("usage: pocold init-testnet --config PUBLIC.json --out NEW_DIR".into());
    }
    let value = |flag: &str| -> Result<&str, String> {
        let at: Vec<_> = args
            .iter()
            .enumerate()
            .filter(|(_, a)| a.as_str() == flag)
            .map(|(i, _)| i)
            .collect();
        if at.len() != 1 {
            return Err(format!("provide exactly one {flag}"));
        }
        args.get(at[0] + 1)
            .map(String::as_str)
            .filter(|s| !s.starts_with("--"))
            .ok_or_else(|| format!("missing {flag} value"))
    };
    let path = Path::new(value("--config")?);
    if std::fs::metadata(path)
        .map_err(|_| "cannot inspect public config")?
        .len()
        > 262144
    {
        return Err("testnet config exceeds 256 KiB".into());
    }
    use std::io::Read;
    let mut input = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| "cannot open public config")?
        .take(262145)
        .read_to_end(&mut input)
        .map_err(|_| "cannot read public config")?;
    prepare(&input, Path::new(value("--out")?))
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The live H15000 chain's CP (deploy/mainnet/h15000), a spec-1 genesis.
    fn h15000() -> ConsensusParams {
        let bundle: serde_json::Value = serde_json::from_slice(&std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../deploy/mainnet/h15000/network.pocol.json")).unwrap()).unwrap();
        let spec = gs::decode(&crate::types::unhex(bundle["files"]["genesis.spec.hex"].as_str().unwrap().trim()).unwrap()).unwrap();
        assert_eq!(spec.spec_version, 1);
        ConsensusParams::from_values(&spec.cp).unwrap()
    }
    fn cp(t_blk: u32, d_att: u32, fb: u32) -> ConsensusParams {
        let mut c = devnet::DevnetConfig::new(777990, 1_800_000_000, &["a"]);
        c.t_blk = t_blk; c.d_att = d_att; c.d_fb_wait = fb;
        ConsensusParams::from_values(&c.cp()).unwrap()
    }
    /// The R2/R3/R6 inequalities ParamGate checks (lp1 gate.rs), at the given locals.
    fn r2_r3_r6(cp: &ConsensusParams, l: &NodeLocal) -> (bool, bool, bool) {
        let (d, t, phi) = (u128::from(l.delta), u128::from(l.t_exec), u128::from(l.capital_phi()));
        let m = crate::timing::metrics(l.delta.into(), l.capital_phi().into(), l.t_exec.into(), l.delta_rel.into(), cp.dFbWait.into());
        (u128::from(l.delta_rel) >= m.sigma + d + 2 * t, u128::from(cp.dFbWait) >= phi + d + 2 * t,
            u128::from(cp.D_att) + u128::from(l.gamma) >= 2 * (m.x_w + u128::from(cp.c) * u128::from(cp.T_blk)) + phi)
    }

    #[test]
    fn spec1_local_timing_is_unchanged_and_refuses_declared_assumptions() {
        let live = h15000();
        assert_eq!((live.T_blk, live.D_att, live.dFbWait), (10, 60, 6));
        let l = local_for(&live).unwrap();
        assert_eq!((l.delta, l.phi, l.t_exec, l.delta_rel, l.gamma), (1, 1, 1, 8, 5), "the live H15000 local timing");
        let s = local_for_spec(&live, 1, None).unwrap();
        assert_eq!((s.delta, s.phi, s.t_exec, s.delta_rel, s.gamma), (1, 1, 1, 8, 5));
        assert!(local_for_spec(&live, 1, Some(TimingAssumptions::V3_WAN)).unwrap_err().contains("v3 profile input"));
        // The spec-1 derivation at other CPs is the one before V3-05: dFbWait + Δ + t_exec, Γ the R6 slack.
        for (t, d, f, rel, gamma) in [(10, 60, 6, 8, 5), (30, 150, 10, 12, 5), (4, 25, 3, 5, 7), (10, 30, 6, 8, 32)] {
            let l = local_for(&cp(t, d, f)).unwrap();
            assert_eq!((l.delta_rel, l.gamma), (rel, gamma), "T_blk {t} D_att {d} dFbWait {f}");
        }
    }

    #[test]
    fn delta_rel_is_sigma_plus_delta_plus_two_t_exec() {
        let at = |delta, phi, t_exec| derive_delta_rel(TimingAssumptions { delta, phi, t_exec }).unwrap();
        assert_eq!(derive_delta_rel(TimingAssumptions::SINGLE_HOST).unwrap(), 7);
        assert_eq!(derive_delta_rel(TimingAssumptions::V3_WAN).unwrap(), 13, "O-6=b: Δ_rel 13 at Δ ≤ 2 s, φ ≤ 2 s");
        assert_eq!(at(5, 5, 3), 25, "V3-05: the governance WAN locals need Δ_rel ≥ 25");
        assert_eq!(at(1, 0, 1), 5, "single host, one clock (NodeLocal::single_host)");
        // σ = max(Δ, Φ+1) + t: the Δ branch once Δ exceeds Φ + 1.
        assert_eq!(at(10, 1, 1), 10 + 1 + 10 + 2);
        for delta in 1..=12 { for phi in 0..=12 { for t in 1..=6 {
            let rel = at(delta, phi, t);
            let sigma = delta.max(2 * phi + 1) + t;
            assert_eq!(rel, sigma + delta + 2 * t);
            // R2 holds at equality: one second less fails it.
            let l = NodeLocal { delta, phi, t_exec: t, delta_rel: rel, ..local() };
            let m = crate::timing::metrics(delta.into(), l.capital_phi().into(), t.into(), rel.into(), 0);
            assert_eq!(u128::from(rel), m.sigma + u128::from(delta) + 2 * u128::from(t));
        }}}
        for bad in [TimingAssumptions { delta: 0, phi: 1, t_exec: 1 }, TimingAssumptions { delta: 1, phi: 1, t_exec: 0 },
            TimingAssumptions { delta: 3601, phi: 1, t_exec: 1 }, TimingAssumptions { delta: 1, phi: 3601, t_exec: 1 },
            TimingAssumptions { delta: 1, phi: 1, t_exec: 3601 }] {
            assert!(derive_delta_rel(bad).is_err(), "{bad:?}");
        }
        let max = TimingAssumptions { delta: 3600, phi: 3600, t_exec: 3600 };
        assert_eq!(derive_delta_rel(max).unwrap(), (7201 + 3600) + 3600 + 7200);
    }

    #[test]
    fn the_v3_default_satisfies_r2_r3_and_r6_for_each_locals_profile() {
        // O-6=b (D-V3-06): T_blk 30, D_att 150, dFbWait 10 at the V3_WAN default.
        let ob = cp(30, 150, 10);
        let l = local_for_spec(&ob, 2, None).unwrap();
        assert_eq!((l.delta, l.phi, l.t_exec, l.delta_rel, l.gamma), (2, 2, 2, 13, 8));
        assert_eq!(r2_r3_r6(&ob, &l), (true, true, true));
        // The 60 s fallback with the governance WAN locals (V3-06 (c)): Δ_rel 25 by derivation.
        let oc = cp(60, 300, 25);
        let l = local_for_v3(&oc, TimingAssumptions { delta: 5, phi: 5, t_exec: 3 }).unwrap();
        assert_eq!(l.delta_rel, 25);
        assert_eq!(r2_r3_r6(&oc, &l), (true, true, true));
        // R2 holds for every profile by construction; R6 by Γ. R3 is a CP condition admission decides.
        for (cp, a) in [(&ob, TimingAssumptions::SINGLE_HOST), (&ob, TimingAssumptions::V3_WAN), (&oc, TimingAssumptions::V3_WAN),
            (&oc, TimingAssumptions { delta: 5, phi: 5, t_exec: 3 }), (&cp(10, 60, 6), TimingAssumptions::SINGLE_HOST)] {
            let l = local_for_v3(cp, a).unwrap();
            let (r2, _, r6) = r2_r3_r6(cp, &l);
            assert!(r2 && r6, "{a:?} at T_blk {}", cp.T_blk);
        }
        let (_, r3, _) = r2_r3_r6(&cp(10, 60, 6), &local_for_v3(&cp(10, 60, 6), TimingAssumptions::V3_WAN).unwrap());
        assert!(!r3, "the 10 s CP cannot carry the WAN assumptions (V3-06 (a)): admission refuses it");
    }
}
