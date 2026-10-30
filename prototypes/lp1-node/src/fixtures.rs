//! Runtime loading of the public fixture files that root copies byte-identically from
//! coordination/lp1-inputs/ into `<crate>/fixtures/`. Integers are read exactly (JSON integers or
//! the literal "2^N" form); floating point is never used.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::fixed::U256;
use crate::genesis::{self, GenesisIdentity};
use crate::hashes::sha256;
use crate::hex;

pub const FILES: [&str; 6] = ["profile.json", "chain.json", "window-cases.json", "asert-oracle.json", "hash-oracle.json", "PROVENANCE.json"];

/// `<crate root>/fixtures`, fixed at compile time so the binary works from any working directory.
pub fn default_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

pub fn read_file(dir: &Path, name: &str) -> Result<Vec<u8>, String> {
    let p = dir.join(name);
    fs::read(&p).map_err(|e| format!("read {}: {e}", p.display()))
}

pub fn read_json(dir: &Path, name: &str) -> Result<Value, String> {
    let b = read_file(dir, name)?;
    serde_json::from_slice(&b).map_err(|e| format!("{name}: {e}"))
}

fn get<'a>(v: &'a Value, key: &str, ctx: &str) -> Result<&'a Value, String> {
    v.get(key).ok_or_else(|| format!("{ctx}: missing {key}"))
}

pub fn as_u64(v: &Value, ctx: &str) -> Result<u64, String> {
    v.as_u64().ok_or_else(|| format!("{ctx}: not an exact unsigned integer"))
}

pub fn as_str<'a>(v: &'a Value, ctx: &str) -> Result<&'a str, String> {
    v.as_str().ok_or_else(|| format!("{ctx}: not a string"))
}

/// Exact U256 from a JSON integer, a canonical decimal string or "2^N".
pub fn as_u256(v: &Value, ctx: &str) -> Result<U256, String> {
    if let Some(n) = v.as_u64() {
        return Ok(U256::from_u64(n));
    }
    if let Some(s) = v.as_str() {
        if let Some(x) = U256::from_profile_str(s) {
            return Ok(x);
        }
    }
    Err(format!("{ctx}: not an exact 256-bit integer"))
}

/// The CP subset the header window uses (consensus.md:18-25).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cp {
    pub g_ts: u64,
    pub target_g: U256,
    pub t_blk: u64,
    pub tau: u64,
    pub nonce_mode: u64,
    pub c: u64,
    pub d_att: u64,
    pub d_fb_wait: u64,
    pub m: u64,
    pub gas_limit: u64,
    pub base_fee0: U256,
    pub h_end: u64,
}

#[derive(Clone, Debug)]
pub struct Profile {
    pub label: String,
    pub chain_id: u64,
    pub genesis_pre: Vec<u8>,
    pub genesis_hash: [u8; 32],
    pub fork_schedule: Vec<(u64, u64)>,
    pub cp: Cp,
    pub signer: [u8; 20],
    pub identity: GenesisIdentity,
}

fn cp_u64(id: &GenesisIdentity, name: &str) -> Result<u64, String> {
    let k = genesis::cp_index(name).ok_or_else(|| format!("unknown CP {name}"))?;
    id.cp[k].to_u64().ok_or_else(|| format!("CP {name} exceeds u64"))
}

/// Parses profile.json and binds it to the literal genesis preimage. Every CP value present in the
/// JSON must equal the decoded literal CP; the window uses the decoded literal values.
pub fn load_profile(v: &Value) -> Result<Profile, String> {
    let label = as_str(get(v, "label", "profile")?, "label")?.to_string();
    let chain_id = as_u64(get(v, "chain_id", "profile")?, "chain_id")?;
    let pre = hex::decode(as_str(get(v, "genesis_pre_hex", "profile")?, "genesis_pre_hex")?).ok_or("genesis_pre_hex: not hex")?;
    if pre.len() != 341 {
        return Err(format!("genesis preimage is {} bytes, frozen profile is 341", pre.len()));
    }
    let gh = hex::decode_fixed::<32>(as_str(get(v, "genesis_hash_hex", "profile")?, "genesis_hash_hex")?).ok_or("genesis_hash_hex")?;
    let identity = genesis::decode_identity(&pre).map_err(|e| format!("genesis {}: {}", e.code, e.detail))?;
    if identity.hash != gh {
        return Err("keccak256(genesis_pre) != genesis_hash".into());
    }
    if identity.chain_id != chain_id {
        return Err("profile chain_id != genesis chainId".into());
    }
    let fs_v = get(v, "fork_schedule", "profile")?.as_array().ok_or("fork_schedule: not an array")?;
    let mut fork_schedule: Vec<(u64, u64)> = Vec::new();
    for pair in fs_v {
        let a = pair.as_array().ok_or("fork_schedule entry: not an array")?;
        if a.len() != 2 {
            return Err("fork_schedule entry: not a [version, startHeight] pair".into());
        }
        let ver = as_u64(&a[0], "fork version")?;
        let start = as_u64(&a[1], "fork startHeight")?;
        if ver > u16::MAX as u64 {
            return Err("fork version exceeds protocolVersion width".into());
        }
        if let Some((_, prev)) = fork_schedule.last() {
            if start <= *prev {
                return Err("fork_schedule startHeight not strictly ascending".into());
            }
        }
        fork_schedule.push((ver, start));
    }
    if fork_schedule.is_empty() {
        return Err("fork_schedule empty".into());
    }
    let cpj = get(v, "cp", "profile")?.as_object().ok_or("cp: not an object")?;
    for (name, val) in cpj {
        let k = genesis::cp_index(name).ok_or_else(|| format!("cp.{name}: not a CP field"))?;
        let x = as_u256(val, name)?;
        if x != identity.cp[k] {
            return Err(format!("cp.{name} differs from the literal genesis CP"));
        }
    }
    let target_g = identity.cp[genesis::cp_index("target_g").unwrap_or(1)];
    let base_fee0 = identity.cp[genesis::cp_index("baseFee0").unwrap_or(31)];
    let cp = Cp {
        g_ts: cp_u64(&identity, "g_ts")?,
        target_g,
        t_blk: cp_u64(&identity, "T_blk")?,
        tau: cp_u64(&identity, "tau")?,
        nonce_mode: cp_u64(&identity, "nonceMode")?,
        c: cp_u64(&identity, "c")?,
        d_att: cp_u64(&identity, "D_att")?,
        d_fb_wait: cp_u64(&identity, "dFbWait")?,
        m: cp_u64(&identity, "m")?,
        gas_limit: cp_u64(&identity, "GAS_LIMIT")?,
        base_fee0,
        h_end: cp_u64(&identity, "H_END")?,
    };
    // Lower bounds the window arithmetic relies on (CP_FIELDS lower bounds for these fields).
    if cp.target_g.is_zero() || cp.t_blk < 1 || cp.tau < 1 || cp.c < 1 || cp.m < 1 || cp.nonce_mode > 1 {
        return Err("CP outside the bounds the header window requires".into());
    }
    let signer = hex::decode_fixed::<20>(as_str(get(v, "signer_address", "profile")?, "signer_address")?).ok_or("signer_address")?;
    Ok(Profile { label, chain_id, genesis_pre: pre, genesis_hash: gh, fork_schedule, cp, signer, identity })
}

/// chain.json: `headers[i] = {height: i+1, rlp_hex}`. Returns raw encodings in height order.
pub fn load_chain_bytes(v: &Value) -> Result<Vec<Vec<u8>>, String> {
    let hs = get(v, "headers", "chain")?.as_array().ok_or("headers: not an array")?;
    let mut out = Vec::with_capacity(hs.len());
    for (i, h) in hs.iter().enumerate() {
        let height = as_u64(get(h, "height", "header")?, "height")?;
        if height != i as u64 + 1 {
            return Err(format!("chain entry {i} has height {height}"));
        }
        out.push(hex::decode(as_str(get(h, "rlp_hex", "header")?, "rlp_hex")?).ok_or("rlp_hex: not hex")?);
    }
    Ok(out)
}

/// Verifies each fixture file's SHA-256 against PROVENANCE.json `outputs` (A0 binding).
pub fn verify_provenance(dir: &Path) -> Result<Vec<(String, bool)>, String> {
    let p = read_json(dir, "PROVENANCE.json")?;
    if p.get("privateKeysExported") != Some(&Value::Bool(false)) {
        return Err("PROVENANCE.privateKeysExported is not false".into());
    }
    let outs = get(&p, "outputs", "PROVENANCE")?.as_array().ok_or("outputs")?;
    let mut res = Vec::new();
    for o in outs {
        let path = as_str(get(o, "path", "output")?, "path")?;
        if path.contains('/') || path.contains('\\') || path.contains("..") {
            return Err(format!("output path {path} is not a plain file name"));
        }
        let want = as_str(get(o, "sha256", "output")?, "sha256")?;
        let got = hex::encode(&sha256(&read_file(dir, path)?));
        res.push((path.to_string(), got == want));
    }
    for f in FILES.iter().take(5) {
        if !res.iter().any(|(p, _)| p == f) {
            return Err(format!("{f} has no provenance entry"));
        }
    }
    Ok(res)
}
