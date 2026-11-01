//! Scoped genesis identity binding for the synthetic V1NET fixture profile.
//!
//! What this checks: keccak256(genesisPre) == genesisHash; the GSV1 structure, counts, minimal
//! integers, field widths and fixed lengths (netprofile_ref.decode_genesis order up to the width
//! part of gsRange); and that every CP value carried by the profile JSON equals the CP decoded from
//! the literal preimage. What it does NOT do: CP lower/upper bounds beyond width, gsOrder, gsSys,
//! ParamGate, Draw, allocation or system-code verification. The profile is synthetic and
//! non-bootable; its roots are opaque fixture bytes.

use crate::fixed::U256;
use crate::hashes::keccak256;
use crate::rlp::{self, Item};

/// CP version 1 field names and widths in bits (consensus.md:18-25, netprofile_ref.CP_FIELDS).
pub const CP_FIELDS: [(&str, u32); 52] = [
    ("g_ts", 64),
    ("target_g", 256),
    ("T_blk", 32),
    ("tau", 32),
    ("nonceMode", 8),
    ("c", 8),
    ("D_att", 32),
    ("dFbWait", 32),
    ("m", 16),
    ("kappa", 8),
    ("S_max", 16),
    ("D", 16),
    ("W_w", 16),
    ("alpha_bp", 16),
    ("gamma_bp", 16),
    ("subsidy", 256),
    ("B_reg", 256),
    ("R_max", 8),
    ("M_max", 16),
    ("M_min", 16),
    ("REG_RECORDS_MAX", 32),
    ("SWEEP_MAX", 16),
    ("SWEEP_SCAN", 16),
    ("REG_OPS", 16),
    ("W_act", 32),
    ("Theta_inact", 32),
    ("E_win", 32),
    ("U", 32),
    ("E_max", 8),
    ("EVID_BYTES_MAX", 32),
    ("GAS_LIMIT", 64),
    ("baseFee0", 256),
    ("TX_MAX", 32),
    ("BODY_MAX", 32),
    ("B_code_max", 32),
    ("CODE_CAP", 64),
    ("TXBYTES_CAP", 64),
    ("TX_CAP", 64),
    ("SLOT_CAP", 64),
    ("ACCT_CAP", 64),
    ("SYS_SLOT_MAX", 32),
    ("SYS_KEYS_BLOCK_MAX", 32),
    ("SYS_KEYS_USER_MAX", 32),
    ("SYS_ACCT_BLOCK_MAX", 32),
    ("G_SYS", 64),
    ("H_END", 64),
    ("T_END", 64),
    ("H_CLOSE", 64),
    ("T_CLOSE", 64),
    ("Z_close", 32),
    ("RET", 32),
    ("P2", 32),
];

pub fn cp_index(name: &str) -> Option<usize> {
    CP_FIELDS.iter().position(|(n, _)| *n == name)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenesisIdentity {
    pub hash: [u8; 32],
    pub chain_id: u64,
    pub cp: Vec<U256>,
    pub alloc_root: [u8; 32],
    pub sys_code_hash: [u8; 32],
    pub m0_entries: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenesisError {
    pub code: &'static str,
    pub detail: String,
}

fn gerr<T>(code: &'static str, detail: &str) -> Result<T, GenesisError> {
    Err(GenesisError { code, detail: detail.to_string() })
}

fn min_int<'a>(it: &Item<'a>, name: &str) -> Result<&'a [u8], GenesisError> {
    match it.bytes() {
        Some(b) if b.is_empty() || b[0] != 0 => Ok(b),
        _ => gerr("gsInt", name),
    }
}

fn root32(it: &Item<'_>) -> Result<[u8; 32], GenesisError> {
    match it.bytes() {
        Some(b) if b.len() == 32 => {
            let mut o = [0u8; 32];
            o.copy_from_slice(b);
            Ok(o)
        }
        _ => gerr("gsLen", "root"),
    }
}

/// Decodes the GSV1 preimage. Framing errors (strict RLP) are reported as L0.
pub fn decode_identity(pre: &[u8]) -> Result<GenesisIdentity, GenesisError> {
    let top = match rlp::decode_with_depth(pre, 3) {
        Ok(t) => t,
        Err(e) => return gerr("L0", e.code()),
    };
    let items = match top.list() {
        Some(v) => v,
        None => return gerr("gsStructure", "top not a list"),
    };
    let kinds = [false, false, true, false, true, false]; // true = list
    for (idx, node) in items.iter().take(6).enumerate() {
        if node.list().is_some() != kinds[idx] {
            return gerr("gsStructure", &format!("top[{idx}]"));
        }
    }
    let empty: [Item<'_>; 0] = [];
    let cp = if items.len() > 2 { items[2].list().unwrap_or(&empty[..]) } else { &empty[..] };
    let m0 = if items.len() > 4 { items[4].list().unwrap_or(&empty[..]) } else { &empty[..] };
    if cp.iter().any(|x| x.bytes().is_none()) {
        return gerr("gsStructure", "CP item is a list");
    }
    for e in m0 {
        match e.list() {
            Some(v) if v.iter().all(|x| x.bytes().is_some()) => {}
            _ => return gerr("gsStructure", "M_0List entry"),
        }
    }
    if let Some(first) = items.first() {
        // Value comparison (leading zeros are judged later as gsInt, as in the reference order).
        let v = first.bytes().unwrap_or(&[]);
        let significant: Vec<u8> = v.iter().copied().skip_while(|x| *x == 0).collect();
        if significant != [1u8] {
            return gerr("gsVersion", "specVersion");
        }
    }
    if items.len() != 6 {
        return gerr("gsCount", "top");
    }
    if cp.len() != 52 {
        return gerr("gsCount", "CP");
    }
    if m0.is_empty() {
        return gerr("gsCount", "M_0List empty");
    }
    for e in m0 {
        if e.list().map(|v| v.len()) != Some(2) {
            return gerr("gsCount", "M_0List entry");
        }
    }
    min_int(&items[0], "specVersion")?;
    let chain_b = min_int(&items[1], "chainId")?;
    let mut cp_vals = Vec::with_capacity(52);
    for (k, item) in cp.iter().enumerate() {
        let b = min_int(item, CP_FIELDS[k].0)?;
        cp_vals.push(b);
    }
    let alloc_root = root32(&items[3])?;
    let sys_code_hash = root32(&items[5])?;
    for e in m0 {
        let v = e.list().unwrap_or(&empty[..]);
        if v[0].bytes().map(|b| b.len()) != Some(20) || v[1].bytes().map(|b| b.len()) != Some(20) {
            return gerr("gsLen", "M_0List address");
        }
    }
    if chain_b.is_empty() || chain_b.len() > 8 {
        return gerr("gsRange", "chainId");
    }
    let mut chain_id = 0u64;
    for x in chain_b {
        chain_id = (chain_id << 8) | *x as u64;
    }
    let mut cp_out = Vec::with_capacity(52);
    for (k, b) in cp_vals.iter().enumerate() {
        let (name, width) = CP_FIELDS[k];
        let v = match U256::from_be_slice(b) {
            Some(v) => v,
            None => return gerr("gsRange", name),
        };
        if v.bits() > width {
            return gerr("gsRange", name);
        }
        cp_out.push(v);
    }
    Ok(GenesisIdentity { hash: keccak256(pre), chain_id, cp: cp_out, alloc_root, sys_code_hash, m0_entries: m0.len() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cp_table_shape() {
        assert_eq!(CP_FIELDS.len(), 52);
        assert_eq!(cp_index("H_END"), Some(45));
        assert_eq!(cp_index("GAS_LIMIT"), Some(30));
        assert_eq!(cp_index("baseFee0"), Some(31));
        assert_eq!(cp_index("nope"), None);
    }

    #[test]
    fn structural_rejections() {
        assert_eq!(decode_identity(&[0x80]).unwrap_err().code, "gsStructure");
        assert_eq!(decode_identity(&[0xc1, 0x02]).unwrap_err().code, "gsVersion");
        assert_eq!(decode_identity(&[0xc1, 0x01]).unwrap_err().code, "gsCount");
        assert_eq!(decode_identity(&[0xc2, 0x01]).unwrap_err().code, "L0");
        assert_eq!(decode_identity(&[0xc2, 0x01, 0xc0]).unwrap_err().code, "gsStructure");
    }
}
