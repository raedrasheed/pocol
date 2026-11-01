//! V1 header item codec: [ST = [UT(18), sig], nonce(8), shareList(<= 256 x 8 bytes), winnerSig(65)].
//! Field widths, minimal integers and size limits follow v1_ref_027.parse_header; every decode
//! failure is rule "1" with a detail string. `encode` rebuilds the bytes from the typed fields.

use crate::fixed::U256;
use crate::rlp::{self, Item};

pub const TAG: &[u8] = b"PoCol-tpl-v1";
pub const UT_MAX: usize = 613;
pub const ST_MAX: usize = 682;
pub const HDR_MAX: usize = 3067;
pub const SHARES_MAX: usize = 256;
pub const UT_FIELDS: usize = 18;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub chain_id: u64,
    pub genesis_hash: [u8; 32],
    pub parent_hash: [u8; 32],
    pub h: u64,
    pub a: u32,
    pub protocol_version: u16,
    pub ts: u64,
    pub target: U256,
    pub state_root: [u8; 32],
    pub tx_root: [u8; 32],
    pub receipts_root: [u8; 32],
    pub logs_bloom: Vec<u8>,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub base_fee: U256,
    pub evidence_root: [u8; 32],
    /// Empty or 20 bytes.
    pub proposer: Vec<u8>,
    /// Empty or 65 bytes.
    pub sig: Vec<u8>,
    pub nonce: u64,
    pub shares: Vec<u64>,
    pub winner_sig: [u8; 65],
    /// Exact RLP(UT): the TemplateID preimage.
    pub ut_raw: Vec<u8>,
    /// Exact RLP(shareItems): the shareRoot preimage.
    pub share_list_raw: Vec<u8>,
    /// Exact item encoding.
    pub encoded: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecodeFail {
    pub detail: &'static str,
}

fn fail<T>(detail: &'static str) -> Result<T, DecodeFail> {
    Err(DecodeFail { detail })
}

fn uint_bytes<'a>(it: &Item<'a>, width: usize, name: &'static str) -> Result<&'a [u8], DecodeFail> {
    match it.bytes() {
        Some(b) if b.len() <= width && (b.is_empty() || b[0] != 0) => Ok(b),
        _ => fail(name),
    }
}

fn uint_u64(it: &Item<'_>, width: usize, name: &'static str) -> Result<u64, DecodeFail> {
    let b = uint_bytes(it, width, name)?;
    let mut v = 0u64;
    for x in b {
        v = (v << 8) | *x as u64;
    }
    Ok(v)
}

fn uint_u256(it: &Item<'_>, name: &'static str) -> Result<U256, DecodeFail> {
    let b = uint_bytes(it, 32, name)?;
    match U256::from_be_slice(b) {
        Some(v) => Ok(v),
        None => fail(name),
    }
}

fn fixed32(it: &Item<'_>, name: &'static str) -> Result<[u8; 32], DecodeFail> {
    match it.bytes() {
        Some(b) if b.len() == 32 => {
            let mut o = [0u8; 32];
            o.copy_from_slice(b);
            Ok(o)
        }
        _ => fail(name),
    }
}

/// Parses an already strictly decoded header item (the window path decodes the outer list first).
pub fn parse_item(item: &Item<'_>) -> Result<Header, DecodeFail> {
    let top = match item.list() {
        Some(v) if v.len() == 4 => v,
        _ => return fail("header arity"),
    };
    let st = match top[0].list() {
        Some(v) if v.len() == 2 => v,
        _ => return fail("template arity"),
    };
    let ut = match st[0].list() {
        Some(v) if v.len() == UT_FIELDS => v,
        _ => return fail("template arity"),
    };
    if ut[0].bytes() != Some(TAG) {
        return fail("tag");
    }
    let chain_id = uint_u64(&ut[1], 8, "integer")?;
    let genesis_hash = fixed32(&ut[2], "genesisHash")?;
    let parent_hash = fixed32(&ut[3], "parentHash")?;
    let h = uint_u64(&ut[4], 8, "integer")?;
    let a = uint_u64(&ut[5], 4, "integer")? as u32;
    let protocol_version = uint_u64(&ut[6], 2, "integer")? as u16;
    let ts = uint_u64(&ut[7], 8, "integer")?;
    let target = uint_u256(&ut[8], "integer")?;
    let state_root = fixed32(&ut[9], "stateRoot")?;
    let tx_root = fixed32(&ut[10], "txRoot")?;
    let receipts_root = fixed32(&ut[11], "receiptsRoot")?;
    let logs_bloom = match ut[12].bytes() {
        Some(b) if b.len() == 256 => b.to_vec(),
        _ => return fail("logsBloom"),
    };
    let gas_limit = uint_u64(&ut[13], 8, "integer")?;
    let gas_used = uint_u64(&ut[14], 8, "integer")?;
    let base_fee = uint_u256(&ut[15], "integer")?;
    let evidence_root = fixed32(&ut[16], "evidenceRoot")?;
    let proposer = match ut[17].bytes() {
        Some(b) if b.is_empty() || b.len() == 20 => b.to_vec(),
        _ => return fail("proposer"),
    };
    if target.is_zero() {
        return fail("target range");
    }
    let sig = match st[1].bytes() {
        Some(b) if b.is_empty() || b.len() == 65 => b.to_vec(),
        _ => return fail("sig"),
    };
    let nonce = match top[1].bytes() {
        Some(b) if b.len() == 8 => {
            let mut a8 = [0u8; 8];
            a8.copy_from_slice(b);
            u64::from_be_bytes(a8)
        }
        _ => return fail("nonce"),
    };
    let share_items = match top[2].list() {
        Some(v) if v.len() <= SHARES_MAX => v,
        _ => return fail("shareList"),
    };
    let mut shares = Vec::with_capacity(share_items.len());
    for s in share_items {
        match s.bytes() {
            Some(b) if b.len() == 8 => {
                let mut a8 = [0u8; 8];
                a8.copy_from_slice(b);
                shares.push(u64::from_be_bytes(a8));
            }
            _ => return fail("shareList"),
        }
    }
    let winner_sig = match top[3].bytes() {
        Some(b) if b.len() == 65 => {
            let mut w = [0u8; 65];
            w.copy_from_slice(b);
            w
        }
        _ => return fail("winnerSig"),
    };
    // Strict decoding is canonical, so each raw span equals RLP.encode of the decoded value.
    if st[0].raw.len() > UT_MAX || top[0].raw.len() > ST_MAX || item.raw.len() > HDR_MAX {
        return fail("size");
    }
    Ok(Header {
        chain_id,
        genesis_hash,
        parent_hash,
        h,
        a,
        protocol_version,
        ts,
        target,
        state_root,
        tx_root,
        receipts_root,
        logs_bloom,
        gas_limit,
        gas_used,
        base_fee,
        evidence_root,
        proposer,
        sig,
        nonce,
        shares,
        winner_sig,
        ut_raw: st[0].raw.to_vec(),
        share_list_raw: top[2].raw.to_vec(),
        encoded: item.raw.to_vec(),
    })
}

/// Decodes one standalone header encoding. Oversized input is refused before structural decoding.
pub fn decode(b: &[u8]) -> Result<Header, DecodeFail> {
    if b.len() > HDR_MAX {
        return fail("size");
    }
    match rlp::decode(b) {
        Ok(it) => parse_item(&it),
        Err(e) => fail(e.code()),
    }
}

fn push_uint_u64(v: u64, out: &mut Vec<u8>) {
    let be = v.to_be_bytes();
    let first = be.iter().position(|x| *x != 0).unwrap_or(8);
    rlp::encode_bytes(&be[first..], out);
}

impl Header {
    pub fn is_signed(&self) -> bool {
        !self.proposer.is_empty() || !self.sig.is_empty()
    }

    pub fn encode_ut(&self) -> Vec<u8> {
        let mut p = Vec::with_capacity(UT_MAX);
        rlp::encode_bytes(TAG, &mut p);
        push_uint_u64(self.chain_id, &mut p);
        rlp::encode_bytes(&self.genesis_hash, &mut p);
        rlp::encode_bytes(&self.parent_hash, &mut p);
        push_uint_u64(self.h, &mut p);
        push_uint_u64(self.a as u64, &mut p);
        push_uint_u64(self.protocol_version as u64, &mut p);
        push_uint_u64(self.ts, &mut p);
        rlp::encode_bytes(&self.target.to_min_be(), &mut p);
        rlp::encode_bytes(&self.state_root, &mut p);
        rlp::encode_bytes(&self.tx_root, &mut p);
        rlp::encode_bytes(&self.receipts_root, &mut p);
        rlp::encode_bytes(&self.logs_bloom, &mut p);
        push_uint_u64(self.gas_limit, &mut p);
        push_uint_u64(self.gas_used, &mut p);
        rlp::encode_bytes(&self.base_fee.to_min_be(), &mut p);
        rlp::encode_bytes(&self.evidence_root, &mut p);
        rlp::encode_bytes(&self.proposer, &mut p);
        let mut out = Vec::with_capacity(p.len() + 3);
        rlp::encode_list_payload(&p, &mut out);
        out
    }

    pub fn encode_share_list(&self) -> Vec<u8> {
        let mut p = Vec::with_capacity(self.shares.len() * 9);
        for s in &self.shares {
            rlp::encode_bytes(&s.to_be_bytes(), &mut p);
        }
        let mut out = Vec::with_capacity(p.len() + 3);
        rlp::encode_list_payload(&p, &mut out);
        out
    }

    /// Re-encodes from typed fields (independent of the saved raw spans).
    pub fn encode(&self) -> Vec<u8> {
        let mut st = self.encode_ut();
        rlp::encode_bytes(&self.sig, &mut st);
        let mut st_l = Vec::new();
        rlp::encode_list_payload(&st, &mut st_l);
        let mut body = st_l;
        rlp::encode_bytes(&self.nonce.to_be_bytes(), &mut body);
        body.extend_from_slice(&self.encode_share_list());
        rlp::encode_bytes(&self.winner_sig, &mut body);
        let mut out = Vec::with_capacity(body.len() + 3);
        rlp::encode_list_payload(&body, &mut out);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal synthetic header encoding used for boundary tests (no signatures are valid).
    pub fn sample() -> Header {
        Header {
            chain_id: 777002,
            genesis_hash: [0x11; 32],
            parent_hash: [0x22; 32],
            h: 1,
            a: 0,
            protocol_version: 1,
            ts: 1_700_000_010,
            target: U256::pow2(240).unwrap(),
            state_root: [0x55; 32],
            tx_root: [0x56; 32],
            receipts_root: [0x56; 32],
            logs_bloom: vec![0u8; 256],
            gas_limit: 8_000_000,
            gas_used: 0,
            base_fee: U256::from_u64(1_000_000_000),
            evidence_root: [0x56; 32],
            proposer: vec![0xf3; 20],
            sig: vec![1u8; 65],
            nonce: 0x164d4,
            shares: vec![1, 5, 9],
            winner_sig: [2u8; 65],
            ut_raw: vec![],
            share_list_raw: vec![],
            encoded: vec![],
        }
    }

    #[test]
    fn typed_round_trip() {
        let h = sample();
        let enc = h.encode();
        let d = decode(&enc).unwrap();
        assert_eq!(d.encode(), enc);
        assert_eq!(d.encoded, enc);
        assert_eq!(d.ut_raw, h.encode_ut());
        assert_eq!(d.shares, vec![1, 5, 9]);
    }

    #[test]
    fn share_count_boundary() {
        let mut h = sample();
        h.shares = (0..256u64).collect();
        let enc = h.encode();
        assert!(enc.len() <= HDR_MAX);
        assert_eq!(decode(&enc).unwrap().shares.len(), 256);
        h.shares = (0..257u64).collect();
        assert_eq!(decode(&h.encode()).unwrap_err().detail, "shareList");
    }

    #[test]
    fn field_faults() {
        let base = sample();
        let mut t = base.clone();
        t.target = U256::ZERO;
        assert_eq!(decode(&t.encode()).unwrap_err().detail, "target range");
        let mut p = base.clone();
        p.proposer = vec![1; 19];
        assert_eq!(decode(&p.encode()).unwrap_err().detail, "proposer");
        let mut s = base.clone();
        s.sig = vec![1; 64];
        assert_eq!(decode(&s.encode()).unwrap_err().detail, "sig");
        let mut b = base.clone();
        b.logs_bloom = vec![0; 255];
        assert_eq!(decode(&b.encode()).unwrap_err().detail, "logsBloom");
        let mut out = Vec::new();
        rlp::encode_list_payload(&[0x80, 0x80, 0x80], &mut out);
        assert_eq!(decode(&out).unwrap_err().detail, "header arity");
        assert_eq!(decode(&vec![0u8; HDR_MAX + 1]).unwrap_err().detail, "size");
    }

    #[test]
    fn nonminimal_integer_rejected() {
        // Replace the minimal chainId 0x830bdb2a with a padded 0x8400 0bdb2a: the item is still
        // canonical RLP, but the integer has a leading zero.
        let enc = sample().encode();
        let pos = enc.windows(4).position(|w| w == [0x83, 0x0b, 0xdb, 0x2a]).unwrap();
        let mut bad = Vec::new();
        bad.extend_from_slice(&enc[..pos]);
        bad.extend_from_slice(&[0x84, 0x00, 0x0b, 0xdb, 0x2a]);
        bad.extend_from_slice(&enc[pos + 4..]);
        // Fix the three enclosing length prefixes (item, ST, UT: all two-byte 0xf9 lengths here).
        for off in [1usize, 4, 7] {
            let n = u16::from_be_bytes([bad[off], bad[off + 1]]) + 1;
            bad[off..off + 2].copy_from_slice(&n.to_be_bytes());
        }
        assert_eq!(decode(&bad).unwrap_err().detail, "integer");
    }
}
