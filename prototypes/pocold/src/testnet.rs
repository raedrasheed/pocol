//! Public-information-only genesis preparation for independently held test keys.
//! Still ExperimentalDevnet admission; does not enable the production network profile.
//!
//! Phase 6a of the fresh network: every genesis this builder makes is a v3 genesis (specVersion 2, CP schema v2, the V3
//! programs, nonceMode 1, a continuous chain). The spec-1 options `continuous`, `earnin` and `fullNonce` are retired and
//! refused by name (each is implied); `"v3": true` is still accepted (the committed profiles state it), `"v3": false`
//! is refused. A config that states none of them builds exactly the genesis it built with `"v3": true` before.
//!
//! FN-4 (FRESH-NETWORK-DESIGN.md row C7): a config with `profile` is a complete network profile, such as the committed
//! `deploy/mainnet/v3-profile.json` and `v3-rehearsal-profile.json`. It states every CP field but g_ts and target_g
//! (`genesisTime` and the measured `hashRate`), its local timing and its founders (`founders`, the only allocation).
//! Values the genesis ceremony supplies are strings starting with `PLACEHOLDER`; the builder refuses any such value
//! anywhere in a config unless `TestPlaceholders` (the `--test-placeholders` flag) fills them with public test values,
//! and such a genesis is marked as a test genesis in its devnet.json. A test genesis also runs on the local
//! `TEST_CHAIN_ID`, never the profile's own id (recorded as `profileChainId`): design §1b keeps network ids distinct so
//! that transactions signed on one chain cannot replay on another (EIP-155 binds only the chain id).
use pocol_lp4_execution::{
    admission::{self, NodeLocal, Request},
    apply::{Genesis, SystemPrograms},
    devnet,
    gate::FactClass,
    genesis_spec::{self as gs, GenesisSpec, Member},
    profile::{ConsensusParams, ProfileIdentity, ProfileKind},
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
    /// FN-4: a complete network profile (module doc): the name it is recorded under.
    #[serde(default)]
    profile: Option<String>,
    /// Where the profile's values are decided (a decision record path); recorded, not interpreted.
    #[serde(default)]
    record: Option<String>,
    /// Every genesis is a v3 genesis (module doc): absent or true; false (a spec-1 genesis) is refused.
    #[serde(default = "yes")]
    v3: bool,
    #[serde(default)]
    cp: BTreeMap<String, serde_json::Value>,
    /// V3-05: the declared (or measured, V3-06 soak) WAN timing assumptions the local Δ_rel and Γ are derived from
    /// (`local_for_v3`). Absent: `TimingAssumptions::V3_DEFAULT` (FN-4).
    #[serde(default)]
    local_timing: Option<DeclaredTiming>,
    /// FN-4: the R25d calibration input H_cal, with its class: `measurement` (the founders' aggregate hash rate,
    /// measured) or `synthetic-fixture`. Absent: the synthetic (1<<18)/10 of every earlier testnet.
    #[serde(default)]
    hash_rate: Option<HashRate>,
    chain_id: u64,
    genesis_time: u64,
    registration_per_block: u8,
    #[serde(default)]
    miners: Vec<Miner>,
    #[serde(default)]
    accounts: Vec<Account>,
    /// FN-4: a profile's founding miners (owner decisions 8 and 9: four founders, 1,000,000 PCOL each including the
    /// 20 PCOL bond). Only a profile has founders; it has no `miners` or `accounts`.
    #[serde(default)]
    founders: Vec<Founder>,
}
fn yes() -> bool { true }
/// The spec-1 builder options phase 6a retired (each is implied by a v3 genesis), refused by name.
const RETIRED_OPTIONS: [&str; 3] = ["continuous", "earnin", "fullNonce"];
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HashRate {
    hashes_per_second: u64,
    class: String,
}
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Founder {
    id: Address,
    reward: Address,
    /// The reward account's genesis balance in wei (decimal); with the HELD bond B_reg it is FOUNDER_TOTAL_WEI.
    allocation_wei: String,
}

/// Owner decision 9 (2026-11-13): four founding miners.
pub const FOUNDERS: usize = 4;
/// Owner decision 8: 1,000,000 PCOL per founder, of which B_reg (20 PCOL) is the founder's HELD bond in Escrow
/// (FRESH-NETWORK-DESIGN §1b), so allocation + B_reg = 10^24 wei.
pub const FOUNDER_TOTAL_WEI: u128 = 1_000_000_000_000_000_000_000_000;
/// The prefix of a value the genesis ceremony fills (module doc).
pub const PLACEHOLDER: &str = "PLACEHOLDER";
/// FN-4: the chain id of every genesis the test flag fills, a local devnet id (7779xx, which the FN-0 draws exclude), in
/// place of the profile's mainnet or rehearsal id, so a test chain never signs under a public network's id.
pub const TEST_CHAIN_ID: u64 = 777_990;

/// FN-4 (FRESH-NETWORK-DESIGN §1a, rows 15 and 53): a CP schema v2 genesis built here carries S_tail = S0/32, so the M3
/// schedule reaches its tail exactly at era 5 (S0 >> 5). A builder check, not a consensus rule: the decoder accepts any
/// S_tail (genesis_spec.rs), and lp4's generic `devnet::build` builds any tail (the V3-19 supply-bound tests probe them).
pub fn tail_is_s0_over_32(subsidy: U256, s_tail: U256) -> Result<(), String> {
    let want = subsidy / U256::from(32);
    if s_tail != want { return Err(format!("S_tail {s_tail} is not S0/32 = {want} (S0 {subsidy})")); }
    Ok(())
}

/// FN-4: the explicit test flag that lets the builder fill a profile's ceremony placeholders. Never a launch input:
/// the founders become the public deterministic keys of `test_founder`, the calibration class `synthetic-fixture`,
/// the chain id `TEST_CHAIN_ID` (the profile's id is kept as `profileChainId`), and devnet.json records `testPlaceholders`.
#[derive(Clone, Copy, Debug)]
pub struct TestPlaceholders {
    pub genesis_time: u64,
    pub hash_rate: u64,
}
/// The public test identity that fills founder `i`'s placeholders (keys derived from a public label).
pub fn test_founder(i: usize) -> devnet::MinerIdentity {
    devnet::miner_identity(&format!("fn4-test-founder-{i}"))
}

/// Every string value starting with PLACEHOLDER, by JSON pointer.
fn placeholders(value: &serde_json::Value, path: &str, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) if s.starts_with(PLACEHOLDER) => out.push(if path.is_empty() { "/".into() } else { path.to_string() }),
        serde_json::Value::Array(a) => for (i, v) in a.iter().enumerate() { placeholders(v, &format!("{path}/{i}"), out) },
        serde_json::Value::Object(o) => for (k, v) in o { placeholders(v, &format!("{path}/{k}"), out) },
        _ => {}
    }
}
/// Any JSON value, refused when an object at any depth repeats a key (checked only; the value is discarded).
struct NoDuplicateKeys;
impl<'de> Deserialize<'de> for NoDuplicateKeys {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visit;
        impl<'de> serde::de::Visitor<'de> for Visit {
            type Value = NoDuplicateKeys;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { f.write_str("JSON without duplicate keys") }
            fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> { Ok(NoDuplicateKeys) }
            fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E> { Ok(NoDuplicateKeys) }
            fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E> { Ok(NoDuplicateKeys) }
            fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> { Ok(NoDuplicateKeys) }
            fn visit_str<E>(self, _: &str) -> Result<Self::Value, E> { Ok(NoDuplicateKeys) }
            fn visit_unit<E>(self) -> Result<Self::Value, E> { Ok(NoDuplicateKeys) }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                while seq.next_element::<NoDuplicateKeys>()?.is_some() {}
                Ok(NoDuplicateKeys)
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut keys = HashSet::new();
                while let Some(key) = map.next_key::<String>()? {
                    if !keys.insert(key) { return Err(serde::de::Error::custom("duplicate key")); }
                    map.next_value::<NoDuplicateKeys>()?;
                }
                Ok(NoDuplicateKeys)
            }
        }
        deserializer.deserialize_any(Visit)
    }
}
/// The config with its placeholders filled by `test`, and, when any was filled, the config's own chainId (replaced by
/// `TEST_CHAIN_ID`). Without `test` a placeholder is refused (before any output); with it, only the ceremony inputs a
/// profile has (genesisTime, hashRate.hashesPerSecond, each founder's id and reward) are filled, and any other
/// placeholder is still refused.
fn fill(config: &[u8], test: Option<&TestPlaceholders>) -> Result<(serde_json::Value, Option<serde_json::Value>), String> {
    // `serde_json::Value` silently keeps the last of duplicate keys, while the typed `Config` refused them before the
    // config was first read as a Value, so the refusal is made here, for every object at any depth (cp included).
    serde_json::from_slice::<NoDuplicateKeys>(config).map_err(|_| "invalid public genesis config schema")?;
    let mut value: serde_json::Value = serde_json::from_slice(config).map_err(|_| "invalid public genesis config schema")?;
    let mut found = Vec::new();
    placeholders(&value, "", &mut found);
    if found.is_empty() { return Ok((value, None)); }
    let Some(test) = test else {
        return Err(format!("the config has genesis-ceremony placeholders at {}: fill them (FRESH-NETWORK-CP.md), \
            or pass --test-placeholders for a local test genesis", found.join(", ")));
    };
    for path in &found {
        let filled = match path.as_str() {
            "/genesisTime" => serde_json::json!(test.genesis_time),
            "/hashRate/hashesPerSecond" => serde_json::json!(test.hash_rate),
            p => match p.strip_prefix("/founders/").and_then(|r| r.split_once('/')) {
                Some((i, field @ ("id" | "reward"))) => {
                    let i: usize = i.parse().map_err(|_| format!("placeholder at {p}"))?;
                    let f = test_founder(i);
                    serde_json::json!(if field == "id" { f.id } else { f.reward })
                }
                _ => return Err(format!("no test value for the placeholder at {p}")),
            },
        };
        *value.pointer_mut(path).expect("found above") = filled;
    }
    if value.pointer("/hashRate").is_some() {
        // A filled rate is not a measurement.
        value["hashRate"]["class"] = serde_json::json!("synthetic-fixture");
    }
    // Design §1b: a test chain never runs under the profile's (public network's) chain id.
    let object = value.as_object_mut().ok_or("invalid public genesis config schema")?;
    let profile_chain_id = object.insert("chainId".into(), serde_json::json!(TEST_CHAIN_ID)).unwrap_or(serde_json::Value::Null);
    Ok((value, Some(profile_chain_id)))
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
    /// The synthetic single-host assumptions of `local()`: Δ = φ = t_exec = 1 s (Δ_rel 7 by V3-05).
    pub const SINGLE_HOST: Self = Self { delta: 1, phi: 1, t_exec: 1 };
    /// The WAN bounds the owner's O-6=b timing holds under (D-V3-06): measured Δ ≤ 2 s, φ ≤ 2 s, t_exec 2 s,
    /// so Δ_rel = 13 (V3-06 (b): σ = 7, R2 13, R3 dFbWait ≥ 10). The default for a v3 profile.
    pub const V3_WAN: Self = Self { delta: 2, phi: 2, t_exec: 2 };
    /// FN-4 (FRESH-NETWORK-DESIGN §1c): the default of a v3 profile, Δ = φ = t_exec = 1 s (Δ_rel 7). The fresh network's
    /// CP (dFbWait 8) also admits the measured fallback (2,1,1), Δ_rel 8, declared as `localTiming`.
    pub const V3_DEFAULT: Self = Self::SINGLE_HOST;
    /// A local devnet's assumptions (`pocold init-devnet`, Phase 5a): its nodes share one host and one clock, Δ 1 s, φ 0,
    /// t_exec 1 s, so Δ_rel is 5 by V3-05, as `single_host_local`.
    pub const DEVNET: Self = Self { delta: 1, phi: 0, t_exec: 1 };
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

/// devnet.json's `localTiming` for the V3-05 locals `local` derived from declared assumptions: what `local_for_network`
/// reads back (its `deltaRel` and `gamma` must be the derived ones).
pub fn local_timing_json(local: &NodeLocal) -> serde_json::Value {
    serde_json::json!({"delta":local.delta,"phi":local.phi,"tExec":local.t_exec,"deltaRel":local.delta_rel,"gamma":local.gamma,"source":"declared"})
}

/// The local timing for a genesis of `spec_version`: spec 2 (v3) is `local_for_v3` at the declared
/// assumptions, `TimingAssumptions::V3_DEFAULT` (1,1,1) by default (FN-4).
pub fn local_for_spec(cp: &ConsensusParams, spec_version: u8, declared: Option<TimingAssumptions>) -> Result<NodeLocal, String> {
    if spec_version < 2 {
        return Err(retired_spec(spec_version));
    }
    local_for_v3(cp, declared.unwrap_or(TimingAssumptions::V3_DEFAULT))
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

/// The launcher uses the same final CP-derived local timing as the genesis builder: `local_for_v3` at the assumptions
/// its devnet.json declares (`TimingAssumptions::V3_DEFAULT` without one).
/// A v3 devnet.json whose recorded `deltaRel` or `gamma` differs from what V3-05 derives from its assumptions (a
/// file written before V3-05, or one edited by hand) is refused, so a node never runs a T_j other than the one the
/// file states.
pub fn local_for_network(dir: &Path) -> Result<NodeLocal, String> {
    let spec = genesis_spec_in(dir)?;
    let cp = ConsensusParams::from_values(&spec.cp).map_err(|e| format!("CP: {e:?}"))?;
    if spec.spec_version < 2 {
        return Err(retired_spec(spec.spec_version));
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

/// Why a network of `spec_version` < 2 does not run: the spec-1 rule set is retired (phase 6a); boot admission refuses
/// it too (R24).
fn retired_spec(spec_version: u8) -> String {
    format!("GenesisSpec version {spec_version}: the spec-1 rule set is retired; this release runs only v3 (specVersion 2) networks")
}

fn cp_number(value: &serde_json::Value) -> Result<pocol_lp4_execution::fixed::U256, String> {
    let number = if let Some(n) = value.as_u64() { U256::from(n) }
        else if let Some(text) = value.as_str().filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())) {
            text.parse::<U256>().map_err(|_| "CP value is outside uint256")?
        } else { return Err("CP value must be an unsigned integer or decimal string".into()); };
    Ok(devnet::to_fixed(number))
}

/// The CP fields a profile does not state: g_ts is `genesisTime`, target_g the R25d calibration of `hashRate`.
const PROFILE_DERIVED: [&str; 2] = ["g_ts", "target_g"];

/// FN-4: a profile's own shape (module doc), checked before anything is derived from it. Its founders become the
/// miners and the only allocation: each founder's reward account gets `allocationWei`.
fn profile_input(input: &mut Config, test_filled: bool) -> Result<(), String> {
    let name = input.profile.as_deref().unwrap_or_default();
    if name.is_empty() || name.len() > 64 { return Err("profile: the name is 1 to 64 characters".into()); }
    if !input.miners.is_empty() || !input.accounts.is_empty() {
        return Err("profile: the founders are the only members and the only allocation (no miners or accounts)".into());
    }
    if input.founders.len() != FOUNDERS { return Err(format!("profile: {FOUNDERS} founders are required, not {}", input.founders.len())); }
    if input.local_timing.is_none() { return Err("profile: localTiming is required".into()); }
    match &input.hash_rate {
        Some(r) if r.class == "measurement" => {}
        // `fill` marks the test value it put in place of the measured rate.
        Some(r) if test_filled && r.class == "synthetic-fixture" => {}
        Some(_) => return Err("profile: hashRate.class must be \"measurement\" (a measured H_cal, R25d)".into()),
        None => return Err("profile: hashRate is required".into()),
    }
    let schema = gs::cp_schema(2).unwrap();
    let missing: Vec<&str> = schema.iter().map(|f| f.name).filter(|n| !PROFILE_DERIVED.contains(n) && !input.cp.contains_key(*n)).collect();
    if !missing.is_empty() { return Err(format!("profile: the CP must state every field; missing {}", missing.join(", "))); }
    for f in &input.founders {
        input.miners.push(Miner { id: f.id, reward: f.reward });
        input.accounts.push(Account { address: f.reward, wei: f.allocation_wei.clone() });
    }
    let distinct = |v: Vec<Address>| v.iter().collect::<HashSet<_>>().len() == v.len();
    if !distinct(input.founders.iter().map(|f| f.id).collect()) || !distinct(input.founders.iter().map(|f| f.reward).collect()) {
        return Err("profile: founder ids and reward addresses must each be distinct".into());
    }
    Ok(())
}

/// FN-4 founder allocation check, before the genesis is built: each founder's allocation plus the HELD bond B_reg is
/// exactly FOUNDER_TOTAL_WEI (owner decision 8).
fn check_allocation(input: &Config, b_reg: U256) -> Result<(), String> {
    for (i, f) in input.founders.iter().enumerate() {
        let allocation = Some(f.allocation_wei.as_str()).filter(|w| !w.is_empty() && w.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|w| w.parse::<U256>().ok()).ok_or_else(|| format!("founder {i}: allocationWei must be a decimal wei amount"))?;
        if allocation.checked_add(b_reg) != Some(U256::from(FOUNDER_TOTAL_WEI)) {
            return Err(format!("founder {i}: allocation {allocation} + B_reg {b_reg} is not {FOUNDER_TOTAL_WEI} wei"));
        }
    }
    Ok(())
}

/// FN-4: the founders as the admitted genesis holds them: each a live HELD record (status 1, bond state 1) with its
/// B_reg bond and its reward address, the reward account durable (nonce >= 1, PAY-02) with its allocation; Escrow
/// holds the founders' bonds; the initial supply is FOUNDERS x FOUNDER_TOTAL_WEI.
fn check_founders(input: &Config, genesis: &pocol_lp4_execution::apply::Genesis, b_reg: U256) -> Result<(), String> {
    use pocol_lp4_execution::apply;
    let state = genesis.state();
    for (i, f) in input.founders.iter().enumerate() {
        let r = apply::miner_record(state, f.id).ok_or_else(|| format!("founder {i}: no registry record"))?;
        if r.status != 1 || r.tomb || r.bond_state != 1 || r.bond != b_reg || r.reward != f.reward {
            return Err(format!("founder {i}: the genesis record is not a live HELD record with bond {b_reg} and its reward address"));
        }
        let account = state.accounts.get(&f.reward).map(|a| &a.info).ok_or_else(|| format!("founder {i}: no reward account"))?;
        if account.nonce < 1 || account.balance.checked_add(b_reg) != Some(U256::from(FOUNDER_TOTAL_WEI)) {
            return Err(format!("founder {i}: the reward account needs nonce >= 1 (PAY-02) and its allocation"));
        }
    }
    let n = U256::from(input.founders.len());
    let escrow = state.accounts.get(&pocol_lp4_execution::pay::escrow_address()).map_or(U256::ZERO, |a| a.info.balance);
    if escrow != b_reg * n { return Err(format!("Escrow holds {escrow} wei, not the founders' bonds {}", b_reg * n)); }
    if genesis.initial_supply() != U256::from(FOUNDER_TOTAL_WEI) * n {
        return Err(format!("initial supply {} is not {} founders x {FOUNDER_TOTAL_WEI} wei", genesis.initial_supply(), input.founders.len()));
    }
    Ok(())
}

/// FN-4: the genesis supply bound the builder (genesis_builder.rs) and ParamGate R23 hold below 2^127: the initial
/// supply plus the v3 issuance bound 2·S0·H_half + S_tail·2^64 (issuance.rs). About 2.3058·10^36 wei, 121 bits, on the
/// fresh network's profile (FRESH-NETWORK-DESIGN §1b).
pub fn supply_bound(initial: U256, cp: &ConsensusParams) -> Result<U256, String> {
    let future = pocol_lp4_execution::issuance::future_bound(devnet::from_fixed(&cp.subsidy), cp.H_half, devnet::from_fixed(&cp.S_tail))
        .ok_or("supply bound overflows uint256")?;
    let bound = initial.checked_add(future).ok_or("supply bound overflows uint256")?;
    if bound >= U256::from(1) << 127 { return Err(format!("supply bound {bound} is not below 2^127")); }
    Ok(bound)
}

/// Resolve once from typed public input.
fn resolve(config: serde_json::Value, test_filled: bool) -> Result<(Config, devnet::DevnetConfig, Vec<pocol_lp4_execution::fixed::U256>), String> {
    if let Some(gone) = RETIRED_OPTIONS.iter().find(|k| config.get(**k).is_some()) {
        return Err(format!("{gone}: a retired spec-1 option; every genesis this builder makes is a v3 genesis (continuous, the V3 programs, nonceMode 1), so remove it"));
    }
    let mut input: Config = serde_json::from_value(config).map_err(|_| "invalid public genesis config schema")?;
    if !input.v3 { return Err("\"v3\": false asks for a spec-1 genesis; the spec-1 rule set is retired and this builder makes only v3 genesis files".into()); }
    if let Some(r) = &input.hash_rate {
        if r.hashes_per_second == 0 || !["measurement", "synthetic-fixture"].contains(&r.class.as_str()) {
            return Err("hashRate: hashesPerSecond is at least 1 and class is measurement or synthetic-fixture".into());
        }
    }
    if input.profile.is_some() { profile_input(&mut input, test_filled)?; }
    else if !input.founders.is_empty() { return Err("founders belong to a network profile (\"profile\")".into()); }
    if input.chain_id == 777001 || input.chain_id == 0 || input.chain_id >= (1u64 << 53)
        || input.genesis_time > u64::MAX - 10_000_000
        || !(1..=256).contains(&input.miners.len())
        || input.accounts.len() > 10000 || input.registration_per_block > 4 {
        return Err("invalid experimental testnet parameters".into());
    }
    // The builder's own base values: the devnet values (DevnetConfig::new) with the testnet timing, and the S0 of 2 PCOL
    // (S_tail S0/32) this builder has always written for a config that does not state it, so every config builds the
    // genesis it built before phase 6a (a profile states every field).
    let mut cfg = devnet::DevnetConfig::new(input.chain_id, input.genesis_time, &[]);
    cfg.subsidy = U256::from(2_000_000_000_000_000_000u128); cfg.s_tail = U256::from(62_500_000_000_000_000u64);
    cfg.t_blk = 10; cfg.tau = 600; cfg.d_att = 60; cfg.d_fb_wait = 6;
    cfg.hash_rate = input.hash_rate.as_ref().map_or((1 << 18) / 10, |r| r.hashes_per_second); cfg.r_max = input.registration_per_block;
    let schema = gs::cp_schema(cfg.spec_version).unwrap();
    let mut overrides = BTreeMap::new();
    for (name, raw) in &input.cp {
        let index = schema.iter().position(|f| f.name == name).ok_or_else(|| format!("unknown CP field {name}"))?;
        let value = cp_number(raw).map_err(|e| format!("CP {name}: {e}"))?;
        // A v3 chain is continuous: it may state H_END/H_CLOSE/T_END/T_CLOSE as the 0 the builder derives (a profile does).
        let stated_zero = value.is_zero() && ["H_END", "H_CLOSE", "T_END", "T_CLOSE"].contains(&name.as_str());
        if ["g_ts", "target_g", "H_END", "H_CLOSE", "T_END", "T_CLOSE"].contains(&name.as_str()) && !stated_zero {
            return Err(format!("CP field {name} is derived by the builder"));
        }
        let field = &schema[index];
        if value.bits() > field.width || value < pocol_lp4_execution::fixed::U256::from_u64(field.lo) {
            return Err(format!("CP {name}: gsRange"));
        }
        if name == "T_blk" { cfg.t_blk = value.to_u64().unwrap() as u32; }
        overrides.insert(index, value);
    }
    // Time-based synthetic defaults. Explicit CP overrides below take precedence.
    cfg.h_half = 126_230_400u64.div_ceil(u64::from(cfg.t_blk));
    // A continuous chain: DevnetConfig::new has H_END 0, so H_END, T_END, H_CLOSE and T_CLOSE (cp[45..=48]) are 0.
    let mut cp = cfg.cp();
    for (index, value) in overrides { cp[index] = value; }
    let params = ConsensusParams::from_values(&cp).map_err(|e| format!("CP {}: {}", e.detail, e.code))?;
    if params.nonceMode != 1 { return Err("nonceMode contradicts v3: a v3 genesis has nonceMode 1, the full 64-bit nonce domain".into()); }
    // FN-4: the tail of the M3 schedule is S0/32 (FRESH-NETWORK-DESIGN §1a row 53).
    tail_is_s0_over_32(devnet::from_fixed(&params.subsidy), devnet::from_fixed(&params.S_tail))?;
    if input.profile.is_some() {
        if params.R_max != input.registration_per_block {
            return Err(format!("profile: registrationPerBlock {} is not the CP's R_max {}", input.registration_per_block, params.R_max));
        }
        check_allocation(&input, devnet::from_fixed(&params.B_reg))?;
    }
    Ok((input, cfg, cp))
}

pub(crate) fn allowed(address: Address) -> bool {
    let numeric = U256::from_be_slice(address.as_slice());
    !address.is_zero()
        && address != pocol_lp4_execution::pay_address()
        && !(U256::from(0xc0c001u64)..=U256::from(0xc0c0ffu64)).contains(&numeric)
        && address != Address::from_word(((U256::from(1) << 160u32) - U256::from(2)).into())
}

pub fn prepare(config: &[u8], out: &Path) -> Result<(), String> {
    build(config, out, None)
}
/// `prepare` with the explicit test flag (module doc): `Some` fills a profile's ceremony placeholders with public test
/// values; `None` is `prepare`, which refuses every placeholder.
pub fn prepare_with(config: &[u8], out: &Path, test: Option<&TestPlaceholders>) -> Result<(), String> {
    build(config, out, test)
}
fn build(config: &[u8], out: &Path, test: Option<&TestPlaceholders>) -> Result<(), String> {
    if out.exists() {
        return Err("output must be a new directory; genesis is never overwritten".into());
    }
    if config.len() > 262144 { return Err("testnet config exceeds 256 KiB".into()); }
    let (value, profile_chain_id) = fill(config, test)?;
    let test_filled = profile_chain_id.is_some();
    let (input, cfg, cp) = resolve(value, test_filled)?;
    let params = ConsensusParams::from_values(&cp).map_err(|e| format!("CP: {e:?}"))?;
    let mut alloc = InMemoryDB::default();
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
        // Preserve an existing reward account (its allocated balance) and give it at least the required
        // durable nonce.
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
    // V3-34: a v3 genesis (specVersion 2, CP schema v2) runs the V3 programs, the only bundled set.
    let programs = SystemPrograms::v3();
    let mut spec = GenesisSpec {
        spec_version: cfg.spec_version,
        chain_id: cfg.chain_id,
        cp,
        alloc_root: [0; 32],
        m0_list: members,
        sys_code_hash: programs.sys_code_hash().0,
    };
    // OWNER-DECISIONS-2026-11-20 D5: the kinds and scope of this chain (`boot_admission`): the network profile on a
    // network node in scope public-network on a public network chain, else a simnet profile on a simnet node.
    let (kind, node, scope) = crate::boot_admission(cfg.chain_id);
    let identity = |spec: &GenesisSpec| {
        let bytes = gs::encode(spec);
        ProfileIdentity::validate(
            &bytes,
            &gs::genesis_hash(&bytes),
            spec.chain_id,
            kind,
            node,
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
    let measured = input.hash_rate.as_ref().is_some_and(|r| r.class == "measurement");
    let request = Request {
        spec_bytes: &bytes,
        alloc: &alloc,
        forks: &[(1, 0)],
        kind,
        node,
        local: &local,
        calibration: Some((cfg.hash_rate, if measured { FactClass::Measurement } else { FactClass::SyntheticFixture })),
        timing: Some(&timing),
        scope,
    };
    let admitted = admission::admit(&request).map_err(|e| e.to_string())?;
    if input.profile.is_some() { check_founders(&input, admitted.genesis(), devnet::from_fixed(&params.B_reg))?; }
    let bound = supply_bound(admitted.genesis().initial_supply(), &params)?;
    let public = scope == admission::Scope::PublicNetwork;
    let info = serde_json::json!({"chainId":cfg.chain_id, "genesisHash":crate::types::hex(&profile.genesis_hash()),
        "genesisTime":cfg.g_ts, "profileKind":if kind == ProfileKind::Network { "network" } else { "simnet" }, "keyMode":"independent-key-files", "forks":[[1,0]],
        "note":if public { "Public network genesis admitted in scope public-network with its reported conditions and waivers; not specification compliance; launch approval and readiness are reported by a node booted on it." }
            else { "Experimental candidate only; independent keys do not establish public-network readiness." },
        "registrationPerBlock":params.R_max,
        "miners":input.miners.iter().map(|m|serde_json::json!({"id":m.id,"reward":m.reward})).collect::<Vec<_>>(),
        "calibration":{"hashesPerSecond":cfg.hash_rate,"class":if measured {"measurement"} else {"synthetic-fixture"}},
        "timing":timing.to_json(),"admission":crate::admission_json(&admitted)});
    let mut info = info;
    // The keys every v3 genesis has written (readers of devnet.json, such as the wallets and deploy tooling, use them).
    info["v3"] = serde_json::json!(true);
    info["specVersion"] = serde_json::json!(2);
    info["localTiming"] = serde_json::json!({"delta":local.delta,"phi":local.phi,"tExec":local.t_exec,"deltaRel":local.delta_rel,"gamma":local.gamma});
    // V3-05: where the assumptions came from; deltaRel = sigma + delta + 2 tExec, recomputed by the launcher.
    info["localTiming"]["source"] = serde_json::json!(if declared.is_some() { "declared" } else { "v3-default" });
    info["supply"] = serde_json::json!({"initialWei":admitted.genesis().initial_supply().to_string(),"boundWei":bound.to_string(),"boundBits":bound.bit_len()});
    if let Some(name) = &input.profile {
        info["profile"] = serde_json::json!(name);
        if let Some(record) = &input.record { info["record"] = serde_json::json!(record); }
        info["founders"] = input.founders.iter().map(|f| serde_json::json!({"id":f.id,"reward":f.reward,"allocationWei":f.allocation_wei,
            "bondWei":devnet::from_fixed(&params.B_reg).to_string()})).collect();
    }
    if test_filled {
        info["testPlaceholders"] = serde_json::json!(true);
        info["profileChainId"] = profile_chain_id.clone().unwrap_or_default();
        info["note"] = serde_json::json!("TEST GENESIS: ceremony placeholders filled with public test keys and a synthetic hash rate, on the local test chain id; never a launch genesis.");
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
    let mut summary = serde_json::json!({"chainId":cfg.chain_id,"genesisHash":crate::types::hex(&profile.genesis_hash()),
        "scope":admitted.scope().name(),"profileKind":format!("{:?}",admitted.profile().kind()).to_ascii_lowercase(),
        "nodeKind":format!("{:?}",admitted.profile().node_kind()).to_ascii_lowercase()});
    if test_filled {
        summary["testPlaceholders"] = serde_json::json!(true);
        summary["profileChainId"] = profile_chain_id.unwrap_or_default();
    }
    // V3-28 H1, OWNER-DECISIONS-2026-11-20 D4: launch approval, blockers, technical readiness and the identity fields,
    // computed from this build and the admission above (readiness.rs).
    crate::readiness::annotate(&mut summary, &crate::readiness::of_admitted(&admitted));
    println!("{summary}");
    Ok(())
}

pub fn init(args: &[String]) -> Result<(), String> {
    // FN-4: `--test-placeholders` is the explicit test flag (module doc): genesis time now, the synthetic hash rate.
    let test_flag = args.iter().filter(|a| a.as_str() == "--test-placeholders").count();
    let args: Vec<String> = args.iter().filter(|a| a.as_str() != "--test-placeholders").cloned().collect();
    if args.len() != 6 || test_flag > 1 {
        return Err("usage: pocold init-testnet --config PUBLIC.json --out NEW_DIR [--test-placeholders]".into());
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
    let test = (test_flag == 1).then(|| TestPlaceholders { genesis_time: crate::node::now_s(), hash_rate: (1 << 18) / 10 });
    prepare_with(&input, Path::new(value("--out")?), test.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
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

    /// Phase 6a: every genesis is a v3 genesis. A config without "v3" builds the same bytes as with "v3": true; the
    /// retired spec-1 options and "v3": false are refused by name, before any output; a spec-1 network's local timing is
    /// refused with its reason.
    #[test]
    fn every_genesis_is_v3_and_the_spec1_options_are_refused() {
        let miner = devnet::miner_identity("testnet-v3-only");
        let base = serde_json::json!({"chainId":777917,"genesisTime":1_800_000_000u64,"registrationPerBlock":1,
            "miners":[{"id":miner.id,"reward":miner.reward}],
            "accounts":[{"address":devnet::account_key("testnet-v3-only").address(),"wei":"1000000000000000000000000"}]});
        let dir = |tag: &str| { let d = std::env::temp_dir().join(format!("pocold-testnet-v3only-{tag}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); d };
        let built = |config: &serde_json::Value, tag: &str| {
            let out = dir(tag); prepare(config.to_string().as_bytes(), &out).unwrap();
            let bytes = std::fs::read_to_string(out.join("genesis.spec.hex")).unwrap();
            let info: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(out.join("devnet.json")).unwrap()).unwrap();
            let _ = std::fs::remove_dir_all(&out); (bytes, info)
        };
        let (plain, info) = built(&base, "plain");
        let mut stated = base.clone(); stated["v3"] = serde_json::json!(true);
        assert_eq!(built(&stated, "stated").0, plain, "\"v3\": true builds the same genesis");
        let spec = gs::decode(&crate::types::unhex(plain.trim()).unwrap()).unwrap();
        assert_eq!((spec.spec_version, spec.sys_code_hash), (2, SystemPrograms::v3().sys_code_hash().0));
        let cp = ConsensusParams::from_values(&spec.cp).unwrap();
        assert_eq!((cp.nonceMode, cp.H_END, cp.T_blk, cp.D_att, cp.dFbWait), (1, 0, 10, 60, 6));
        assert_eq!((devnet::from_fixed(&cp.subsidy), devnet::from_fixed(&cp.S_tail)), (U256::from(2_000_000_000_000_000_000u128), U256::from(62_500_000_000_000_000u64)),
            "the builder's S0 of 2 PCOL for a config that does not state it");
        assert_eq!(info["v3"], true);
        for k in RETIRED_OPTIONS { assert!(info.get(k).is_none(), "devnet.json no longer writes {k}"); }
        assert_eq!(info["specVersion"], 2);
        for (key, value) in [("continuous", true), ("earnin", true), ("fullNonce", true), ("fullNonce", false), ("v3", false)] {
            let mut c = base.clone(); c[key] = serde_json::json!(value); let out = dir("refused");
            let e = prepare(c.to_string().as_bytes(), &out).unwrap_err();
            assert!(e.contains(if key == "v3" { "spec-1 genesis" } else { "retired spec-1 option" }) && (key == "v3" || e.starts_with(key)), "{key}: {e}");
            assert!(!out.exists());
        }
        let mut c = base.clone(); c["cp"] = serde_json::json!({"nonceMode": 0}); let out = dir("bounded");
        assert!(prepare(c.to_string().as_bytes(), &out).unwrap_err().contains("nonceMode contradicts v3"));
        assert!(local_for_spec(&cp, 1, None).unwrap_err().contains("spec-1 rule set is retired"));
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
        // O-6=b (D-V3-06): T_blk 30, D_att 150, dFbWait 10 at the V3_WAN assumptions (the default before FN-4).
        let ob = cp(30, 150, 10);
        let l = local_for_spec(&ob, 2, Some(TimingAssumptions::V3_WAN)).unwrap();
        assert_eq!((l.delta, l.phi, l.t_exec, l.delta_rel, l.gamma), (2, 2, 2, 13, 8));
        assert_eq!(r2_r3_r6(&ob, &l), (true, true, true));
        // FN-4: the v3 default is V3_DEFAULT (1,1,1), Δ_rel 7 (FRESH-NETWORK-DESIGN §1c).
        let l = local_for_spec(&ob, 2, None).unwrap();
        assert_eq!((l.delta, l.phi, l.t_exec, l.delta_rel, l.gamma), (1, 1, 1, 7, 5));
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
