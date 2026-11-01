//! Public-information-only genesis preparation for independently held test keys.
//! Still ExperimentalDevnet admission; does not enable the production network profile.
use pocol_lp4_execution::{
    admission::{self, NodeLocal, Request},
    apply::{Genesis, SystemPrograms},
    devnet,
    gate::FactClass,
    genesis_spec::{self as gs, GenesisSpec, Member},
    profile::{NodeKind, ProfileIdentity, ProfileKind},
};
use revm::{
    primitives::{AccountInfo, Address, U256},
    InMemoryDB,
};
use serde::Deserialize;
use std::{collections::HashSet, path::Path};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Config {
    #[serde(default)]
    continuous: bool,
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

fn allowed(address: Address) -> bool {
    let numeric = U256::from_be_slice(address.as_slice());
    !address.is_zero()
        && address != pocol_lp4_execution::pay_address()
        && !(U256::from(0xc0c001u64)..=U256::from(0xc0c0ffu64)).contains(&numeric)
        && address != Address::from_word(((U256::from(1) << 160u32) - U256::from(2)).into())
}

pub fn prepare(config: &[u8], out: &Path) -> Result<(), String> {
    if out.exists() {
        return Err("output must be a new directory; genesis is never overwritten".into());
    }
    if config.len() > 262144 {
        return Err("testnet config exceeds 256 KiB".into());
    }
    let input: Config =
        serde_json::from_slice(config).map_err(|_| "invalid public genesis config schema")?;
    if input.chain_id == 777001
        || input.chain_id == 0
        || input.chain_id >= (1u64 << 53)
        || input.genesis_time > u64::MAX - 10_000_000
        || !(1..=64).contains(&input.miners.len())
        || input.accounts.len() > 10000
        || input.registration_per_block > 4
    {
        return Err("invalid experimental testnet parameters".into());
    }
    let mut cfg = devnet::DevnetConfig::new(input.chain_id, input.genesis_time, &[]);
    cfg.t_blk = 10;
    cfg.tau = 600;
    cfg.d_att = 60;
    cfg.d_fb_wait = 6;
    cfg.hash_rate = (1 << 18) / 10;
    cfg.r_max = input.registration_per_block;
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
        // Preserve an explicitly funded reward account and give it the required durable nonce.
        let balance = alloc
            .accounts
            .get(&miner.reward)
            .map(|a| a.info.balance)
            .unwrap_or_default();
        alloc.insert_account_info(
            miner.reward,
            AccountInfo {
                nonce: 1,
                balance,
                ..Default::default()
            },
        );
        members.push(Member {
            id: miner.id.0 .0,
            reward_addr: miner.reward.0 .0,
        });
    }
    members.sort_by_key(|m| m.id);
    let programs = if input.continuous {SystemPrograms::continuous()} else {SystemPrograms::v1()};
    let mut cp=cfg.cp();
    if input.continuous {for i in 45..=48{cp[i]=pocol_lp4_execution::fixed::U256::ZERO;}}
    let mut spec = GenesisSpec {
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
    let local = local();
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
        "registrationPerBlock":cfg.r_max,"continuous":input.continuous,
        "miners":input.miners.iter().map(|m|serde_json::json!({"id":m.id,"reward":m.reward})).collect::<Vec<_>>(),
        "calibration":{"hashesPerSecond":cfg.hash_rate,"class":"synthetic-fixture"},
        "timing":timing.to_json(),"admission":crate::admission_json(&admitted)});
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
