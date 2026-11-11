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
    chain_id: u64,
    genesis_time: u64,
    registration_per_block: u8,
    miners: Vec<Miner>,
    accounts: Vec<Account>,
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
/// This does not measure WAN latency or select the production v3 profile.
pub fn local_for(cp: &ConsensusParams) -> Result<NodeLocal, String> {
    let mut local = local();
    local.delta_rel = cp.dFbWait.checked_add(local.delta).and_then(|v| v.checked_add(local.t_exec))
        .ok_or("local timing exceeds u32")?;
    let metrics = crate::timing::metrics(local.delta.into(), local.capital_phi().into(), local.t_exec.into(), local.delta_rel.into(), cp.dFbWait.into());
    let need = (2 * (metrics.x_w + u128::from(cp.c) * u128::from(cp.T_blk)) + u128::from(local.capital_phi()))
        .saturating_sub(u128::from(cp.D_att));
    local.gamma = local.gamma.max(u32::try_from(need).map_err(|_| "local timing exceeds u32")?);
    Ok(local)
}
/// The launcher uses the same final CP-derived local timing as the genesis builder.
pub fn local_for_network(dir: &Path) -> Result<NodeLocal, String> {
    use std::io::Read;
    let mut text = String::new();
    let limit = (gs::MAX_SPEC_BYTES * 2 + 3) as u64;
    std::fs::File::open(dir.join("genesis.spec.hex")).map_err(|e| format!("genesis.spec.hex: {e}"))?
        .take(limit + 1).read_to_string(&mut text).map_err(|e| format!("genesis.spec.hex: {e}"))?;
    if text.len() as u64 > limit { return Err("GenesisSpec exceeds size bound".into()); }
    let bytes = crate::types::unhex(text.trim())?;
    let spec = gs::decode(&bytes).map_err(|e| format!("GenesisSpec: {e:?}"))?;
    let cp = ConsensusParams::from_values(&spec.cp).map_err(|e| format!("CP: {e:?}"))?;
    local_for(&cp)
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
    let local = local_for(&params)?;
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
