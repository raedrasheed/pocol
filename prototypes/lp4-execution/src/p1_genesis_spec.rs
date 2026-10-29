//! LP3 S1, first increment: GenesisSpec version 1 decode, canonical encode and genesisHash binding.
//!
//! Sources: consensus.md:7-34 (six fields, CP widths and bounds, M_0List rules, decode errors and
//! their evaluation order), D79, validation.md:59-77 (K1-K3, GSV1, L4n), and the accepted M1
//! reference decoder (m1-draft-0.21/tools/netprofile_ref.decode_genesis with the m1-draft-0.22 C30
//! explicit-stack framing parser). Codes and details follow that reference exactly, including the
//! reference convention `gsStructure` for shape errors (P-V3-2; the name is not given in the source,
//! DG-V3-4).
//!
//! Evaluation order, each stage over the whole input before the next: framing (L0), structure,
//! gsVersion, gsCount, gsInt, gsLen, gsRange, gsOrder, gsSys. A wrapped single byte (`81 xx`,
//! xx < 0x80) is valid framing and is judged as gsInt, not L0; this differs on purpose from the LP1
//! scoped decoder in `genesis`, which stays unchanged for the LP1 fixture profile.
//!
//! Resources: framing is validated with an explicit stack that holds only the end offset of each
//! open list (one usize per nesting level, at most one level per input byte, grown by doubling); the
//! later stages walk the validated bytes again instead of building a tree. Nothing recurses on input
//! depth, and no allocation is sized from a declared length: every declared length is compared with
//! the bytes actually present first. The other allocations are the result and the error detail. The
//! gsVersion detail is the exact decimal value, as the reference reports it (`decimal`,
//! O(n^1.59 log n) time and O(n) memory in the length of the specVersion item). Callers bound what
//! they read; `MAX_SPEC_BYTES` is the largest encoding that can be valid.
//!
//! Not here: ParamGate relations such as M_min <= |M_0List| <= M_max (R12, DG-V3-5), recomputing
//! allocRoot / sysCodeHash from alloc and system code (R24(a), LP4), ForkSchedule, profiles and
//! timing. A decoded spec is an identity only and is never bootable.

use crate::decimal::decimal;
use crate::fixed::U256;
use crate::hashes::{keccak256, H256};
use crate::hex;
use crate::rlp;

pub const SPEC_VERSION: u8 = 1;
pub const CP_LEN: usize = 52;
/// Index of alpha_bp in CP; gamma_bp (the next field) is bounded by it.
const ALPHA_BP: usize = 13;
/// FINAL_DESIGN.md:787.
pub const SYSTEM_ADDRESS: Address = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe];
/// Reserved ids 0x00..00C0C001 - 0x00..00C0C0FF (consensus.md:34).
pub const RESERVED_LOW: u32 = 0xC0_C001;
pub const RESERVED_HIGH: u32 = 0xC0_C0FF;

pub type Address = [u8; 20];

/// Upper bound of a CP value beyond its width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upper {
    None,
    Value(u64),
    /// gamma_bp <= 10^4 - alpha_bp.
    Gamma,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpField {
    pub name: &'static str,
    pub width: u32,
    pub lo: u64,
    pub hi: Upper,
}

const fn f(name: &'static str, width: u32, lo: u64, hi: Upper) -> CpField {
    CpField { name, width, lo, hi }
}

/// CP version 1 in order: name, width in bits, lower bound, upper bound (consensus.md:18-25).
pub const CP_SCHEMA: [CpField; CP_LEN] = [
    f("g_ts", 64, 0, Upper::None),
    f("target_g", 256, 1, Upper::None),
    f("T_blk", 32, 1, Upper::None),
    f("tau", 32, 1, Upper::None),
    f("nonceMode", 8, 0, Upper::Value(1)),
    f("c", 8, 1, Upper::None),
    f("D_att", 32, 0, Upper::None),
    f("dFbWait", 32, 0, Upper::None),
    f("m", 16, 1, Upper::None),
    f("kappa", 8, 1, Upper::None),
    f("S_max", 16, 0, Upper::Value(256)),
    f("D", 16, 0, Upper::None),
    f("W_w", 16, 0, Upper::None),
    f("alpha_bp", 16, 0, Upper::Value(10_000)),
    f("gamma_bp", 16, 0, Upper::Gamma),
    f("subsidy", 256, 0, Upper::None),
    f("B_reg", 256, 0, Upper::None),
    f("R_max", 8, 0, Upper::None),
    f("M_max", 16, 1, Upper::None),
    f("M_min", 16, 1, Upper::None),
    f("REG_RECORDS_MAX", 32, 0, Upper::None),
    f("SWEEP_MAX", 16, 0, Upper::None),
    f("SWEEP_SCAN", 16, 0, Upper::None),
    f("REG_OPS", 16, 0, Upper::None),
    f("W_act", 32, 0, Upper::None),
    f("Theta_inact", 32, 0, Upper::None),
    f("E_win", 32, 0, Upper::None),
    f("U", 32, 0, Upper::None),
    f("E_max", 8, 0, Upper::None),
    f("EVID_BYTES_MAX", 32, 0, Upper::None),
    f("GAS_LIMIT", 64, 0, Upper::None),
    f("baseFee0", 256, 0, Upper::None),
    f("TX_MAX", 32, 0, Upper::None),
    f("BODY_MAX", 32, 0, Upper::None),
    f("B_code_max", 32, 0, Upper::None),
    f("CODE_CAP", 64, 0, Upper::None),
    f("TXBYTES_CAP", 64, 0, Upper::None),
    f("TX_CAP", 64, 0, Upper::None),
    f("SLOT_CAP", 64, 0, Upper::None),
    f("ACCT_CAP", 64, 0, Upper::None),
    f("SYS_SLOT_MAX", 32, 0, Upper::None),
    f("SYS_KEYS_BLOCK_MAX", 32, 0, Upper::None),
    f("SYS_KEYS_USER_MAX", 32, 0, Upper::None),
    f("SYS_ACCT_BLOCK_MAX", 32, 0, Upper::None),
    f("G_SYS", 64, 0, Upper::None),
    f("H_END", 64, 0, Upper::None),
    f("T_END", 64, 0, Upper::None),
    f("H_CLOSE", 64, 0, Upper::None),
    f("T_CLOSE", 64, 0, Upper::None),
    f("Z_close", 32, 0, Upper::None),
    f("RET", 32, 1, Upper::None),
    f("P2", 32, 0, Upper::None),
];

/// ParamGate R12 (governance.md:89): M_max >= |M_0List|, and M_max is a u16.
pub const MAX_MEMBERS: usize = u16::MAX as usize;

const fn be_len(v: u64) -> usize {
    let mut n = 0;
    let mut x = v;
    while x > 0 {
        n += 1;
        x >>= 8;
    }
    n
}

/// RLP item length of an unsigned integer `v` (canonical form).
const fn uint_item_len(v: u64) -> usize {
    if v < 0x80 {
        1
    } else {
        1 + be_len(v)
    }
}

const fn list_len(payload: usize) -> usize {
    if payload < 56 {
        1 + payload
    } else {
        1 + be_len(payload as u64) + payload
    }
}

const fn max_cp_payload() -> usize {
    let mut sum = 0;
    let mut k = 0;
    while k < CP_LEN {
        let f = CP_SCHEMA[k];
        sum += match f.hi {
            Upper::None => 1 + (f.width / 8) as usize,
            Upper::Value(h) => uint_item_len(h),
            Upper::Gamma => uint_item_len(10_000),
        };
        k += 1;
    }
    sum
}

/// Largest encoding that can pass decode and ParamGate R12: specVersion 1, chainId and every CP
/// value at their widest, both roots, and MAX_MEMBERS entries of 43 bytes. Any longer input fails
/// R12 even if it decodes. Used to bound tools that read untrusted input; not a decode rule.
pub const MAX_SPEC_BYTES: usize = list_len(1 + 9 + list_len(max_cp_payload()) + 33 + list_len(MAX_MEMBERS * list_len(21 + 21)) + 33);

pub fn cp_index(name: &str) -> Option<usize> {
    CP_SCHEMA.iter().position(|x| x.name == name)
}

/// One decoded M_0List entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Member {
    pub id: Address,
    pub reward_addr: Address,
}

/// RLP([specVersion, chainId, CP, allocRoot, M_0List, sysCodeHash]), version 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenesisSpec {
    pub chain_id: u64,
    pub cp: [U256; CP_LEN],
    pub alloc_root: H256,
    pub m0_list: Vec<Member>,
    pub sys_code_hash: H256,
}

impl GenesisSpec {
    pub fn cp_value(&self, name: &str) -> Option<U256> {
        cp_index(name).map(|k| self.cp[k])
    }
}

/// GsError{code, path} of implementation.md:118. `detail` is the reference decoder's detail string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GsError {
    pub code: &'static str,
    pub detail: String,
}

fn gs<T>(code: &'static str, detail: impl Into<String>) -> Result<T, GsError> {
    Err(GsError { code, detail: detail.into() })
}

// ------------------------------------------------------------------ framing (L0), explicit stack

enum Head {
    Byte,
    Str { n: usize, st: usize },
    List { n: usize, st: usize },
}

/// Long-form length: `ll` big-endian bytes after the prefix at `i`.
fn long_len(b: &[u8], i: usize, ll: usize, short: &'static str) -> Result<(usize, usize), GsError> {
    if b.len() - (i + 1) < ll {
        return gs("L0", "truncated length");
    }
    let lb = &b[i + 1..i + 1 + ll];
    if lb[0] == 0 {
        return gs("L0", "leading zero in length");
    }
    let mut n: u64 = 0;
    for x in lb {
        n = (n << 8) | *x as u64;
    }
    if n < 56 {
        return gs("L0", short);
    }
    // Lengths beyond the address space cannot be present; report them as truncated by the caller.
    let n = if n > usize::MAX as u64 { usize::MAX } else { n as usize };
    Ok((n, i + 1 + ll))
}

fn header(b: &[u8], i: usize) -> Result<Head, GsError> {
    if i >= b.len() {
        return gs("L0", "truncated");
    }
    let p = b[i];
    if p < 0x80 {
        Ok(Head::Byte)
    } else if p <= 0xb7 {
        Ok(Head::Str { n: (p - 0x80) as usize, st: i + 1 })
    } else if p <= 0xbf {
        let (n, st) = long_len(b, i, (p - 0xb7) as usize, "long form for short string")?;
        Ok(Head::Str { n, st })
    } else if p <= 0xf7 {
        Ok(Head::List { n: (p - 0xc0) as usize, st: i + 1 })
    } else {
        let (n, st) = long_len(b, i, (p - 0xf7) as usize, "long form for short list")?;
        Ok(Head::List { n, st })
    }
}

/// Strict RLP framing with the reference's L0 details at the same points and in the same order.
/// Only the end offsets of open lists are kept.
fn check_framing(b: &[u8]) -> Result<(), GsError> {
    let mut ends: Vec<usize> = Vec::new();
    let mut i = 0usize;
    loop {
        match header(b, i)? {
            Head::Byte => i += 1,
            Head::Str { n, st } => {
                if n > b.len() - st {
                    return gs("L0", "truncated string");
                }
                i = st + n;
            }
            Head::List { n, st } => {
                if n > b.len() - st {
                    return gs("L0", "truncated list");
                }
                i = st;
                if n > 0 {
                    ends.push(st + n);
                    continue;
                }
            }
        }
        // An item is complete at i; close every list that ends here.
        loop {
            match ends.last() {
                None => {
                    if i != b.len() {
                        return gs("L0", format!("{} trailing bytes", b.len() - i));
                    }
                    return Ok(());
                }
                Some(end) if i > *end => return gs("L0", "item crosses list end"),
                Some(end) if i < *end => break,
                Some(_) => {
                    ends.pop();
                }
            }
        }
    }
}

/// One item of framing-checked input: kind and payload range.
#[derive(Clone, Copy)]
struct Item {
    list: bool,
    start: usize,
    len: usize,
    wrapped: bool,
}

impl Item {
    fn at(b: &[u8], i: usize) -> Result<Item, GsError> {
        Ok(match header(b, i)? {
            Head::Byte => Item { list: false, start: i, len: 1, wrapped: false },
            Head::Str { n, st } => Item { list: false, start: st, len: n, wrapped: n == 1 && b[st] < 0x80 },
            Head::List { n, st } => Item { list: true, start: st, len: n, wrapped: false },
        })
    }

    fn end(&self) -> usize {
        self.start + self.len
    }

    fn bytes<'a>(&self, b: &'a [u8]) -> &'a [u8] {
        &b[self.start..self.end()]
    }

    /// Children of a list item, in order. Framing was checked, so they tile the payload exactly.
    fn children<'a>(&self, b: &'a [u8]) -> Children<'a> {
        Children { b, i: self.start, end: if self.list { self.end() } else { self.start } }
    }
}

struct Children<'a> {
    b: &'a [u8],
    i: usize,
    end: usize,
}

impl Iterator for Children<'_> {
    type Item = Item;

    fn next(&mut self) -> Option<Item> {
        if self.i >= self.end {
            return None;
        }
        let it = Item::at(self.b, self.i).ok()?;
        self.i = it.end();
        Some(it)
    }
}

// ------------------------------------------------------------------ GenesisSpec decode

fn significant(v: &[u8]) -> &[u8] {
    let first = v.iter().position(|x| *x != 0).unwrap_or(v.len());
    &v[first..]
}

/// Bit length of a big-endian value without leading zero bytes.
fn bit_len(v: &[u8]) -> u64 {
    match v.first() {
        None => 0,
        Some(x) => 8 * (v.len() as u64 - 1) + (8 - x.leading_zeros()) as u64,
    }
}

fn is_reserved(id: &Address) -> bool {
    if *id == [0u8; 20] || *id == SYSTEM_ADDRESS {
        return true;
    }
    let low = (id[17] as u32) << 16 | (id[18] as u32) << 8 | id[19] as u32;
    id[..17].iter().all(|x| *x == 0) && (RESERVED_LOW..=RESERVED_HIGH).contains(&low)
}

fn arr<const N: usize>(v: &[u8]) -> [u8; N] {
    let mut o = [0u8; N];
    o.copy_from_slice(v);
    o
}

/// GenesisSpec.decode. Rejections are values, never panics, at any input length or depth.
pub fn decode(b: &[u8]) -> Result<GenesisSpec, GsError> {
    check_framing(b)?;
    let top = Item::at(b, 0)?;
    if !top.list {
        return gs("gsStructure", "top not a list");
    }
    // structure: the positions that exist have the right kind
    let kinds = [false, false, true, false, true, false];
    let mut slot: [Option<Item>; 6] = [None; 6];
    let mut top_len = 0usize;
    for (idx, it) in top.children(b).enumerate() {
        if idx < 6 {
            if it.list != kinds[idx] {
                return gs("gsStructure", format!("top[{idx}]"));
            }
            slot[idx] = Some(it);
        }
        top_len += 1;
    }
    let none = Item { list: true, start: 0, len: 0, wrapped: false };
    let cp = slot[2].unwrap_or(none);
    let m0 = slot[4].unwrap_or(none);
    if cp.children(b).any(|x| x.list) {
        return gs("gsStructure", "CP item is a list");
    }
    for e in m0.children(b) {
        if !e.list || e.children(b).any(|x| x.list) {
            return gs("gsStructure", "M_0List entry");
        }
    }
    // gsVersion (by value; a non-minimal 1 is judged later as gsInt)
    if let Some(v) = slot[0] {
        let raw = v.bytes(b);
        if significant(raw) != [SPEC_VERSION] {
            return gs("gsVersion", decimal(raw));
        }
    }
    // gsCount
    let (version, chain, alloc_root, sys_code_hash) = match slot {
        [Some(v), Some(c), Some(_), Some(a), Some(_), Some(s)] if top_len == 6 => (v, c, a, s),
        _ => return gs("gsCount", format!("top {top_len}")),
    };
    let cp_len = cp.children(b).count();
    if cp_len != CP_LEN {
        return gs("gsCount", format!("CP {cp_len}"));
    }
    let mut members = 0usize;
    for e in m0.children(b) {
        members += 1;
        let n = e.children(b).count();
        if n != 2 {
            return gs("gsCount", format!("M_0List entry {n}"));
        }
    }
    if members == 0 {
        return gs("gsCount", "M_0List empty");
    }
    // gsInt: specVersion, chainId and every CP item
    let ints = [("specVersion", version), ("chainId", chain)];
    for (name, it) in ints.iter().copied().chain(CP_SCHEMA.iter().map(|x| x.name).zip(cp.children(b))) {
        if it.wrapped || it.bytes(b).first() == Some(&0) {
            return gs("gsInt", name);
        }
    }
    // gsLen
    if alloc_root.len != 32 || sys_code_hash.len != 32 {
        return gs("gsLen", "root");
    }
    let pair = |e: Item| {
        let mut c = e.children(b);
        (c.next().unwrap_or(none), c.next().unwrap_or(none))
    };
    for e in m0.children(b) {
        let (id, reward) = pair(e);
        if id.len != 20 || reward.len != 20 {
            return gs("gsLen", "M_0List address");
        }
    }
    // gsRange
    let chain = chain.bytes(b);
    if chain.is_empty() || chain.len() > 8 {
        return gs("gsRange", "chainId");
    }
    let chain_id = chain.iter().fold(0u64, |a, x| (a << 8) | *x as u64);
    let mut vals = [U256::ZERO; CP_LEN];
    for ((k, field), it) in CP_SCHEMA.iter().enumerate().zip(cp.children(b)) {
        let v = it.bytes(b);
        if bit_len(v) > field.width as u64 {
            return gs("gsRange", field.name);
        }
        let x = match U256::from_be_slice(v) {
            Some(x) => x,
            None => return gs("gsRange", field.name),
        };
        vals[k] = x;
        if x < U256::from_u64(field.lo) {
            return gs("gsRange", field.name);
        }
        let hi = match field.hi {
            Upper::None => None,
            Upper::Value(h) => Some(h),
            Upper::Gamma => vals[ALPHA_BP].to_u64().map(|alpha| 10_000u64.saturating_sub(alpha)),
        };
        if let Some(h) = hi {
            if x > U256::from_u64(h) {
                return gs("gsRange", field.name);
            }
        }
    }
    // gsOrder: ids strictly ascending bytewise
    let id_of = |e: Item| arr::<20>(pair(e).0.bytes(b));
    let mut prev: Option<Address> = None;
    for e in m0.children(b) {
        let id = id_of(e);
        if prev.map_or(false, |p| p >= id) {
            return gs("gsOrder", "ids");
        }
        prev = Some(id);
    }
    // gsSys
    for e in m0.children(b) {
        let id = id_of(e);
        if is_reserved(&id) {
            return gs("gsSys", format!("0x{}", hex::encode(&id)));
        }
    }
    let mut m0_list = Vec::with_capacity(members);
    for e in m0.children(b) {
        let (id, reward) = pair(e);
        m0_list.push(Member { id: arr::<20>(id.bytes(b)), reward_addr: arr::<20>(reward.bytes(b)) });
    }
    Ok(GenesisSpec { chain_id, cp: vals, alloc_root: arr::<32>(alloc_root.bytes(b)), m0_list, sys_code_hash: arr::<32>(sys_code_hash.bytes(b)) })
}

// ------------------------------------------------------------------ encode and identity

fn encode_uint(v: &[u8], out: &mut Vec<u8>) {
    rlp::encode_bytes(significant(v), out);
}

/// Encoded length of a canonical integer item of at most 55 significant bytes.
fn uint_len(v: &[u8]) -> usize {
    match significant(v) {
        [x] if *x < 0x80 => 1,
        m => 1 + m.len(),
    }
}

fn push_list_header(out: &mut Vec<u8>, payload: usize) {
    if payload < 56 {
        out.push(0xc0 + payload as u8);
    } else {
        let be = (payload as u64).to_be_bytes();
        let first = be.iter().position(|x| *x != 0).unwrap_or(7);
        out.push(0xf7 + (8 - first) as u8);
        out.extend_from_slice(&be[first..]);
    }
}

/// GenesisSpec.encode: canonical RLP. It does not validate; decode(encode(s)) == Ok(s) exactly when
/// `s` satisfies the schema. Lengths are computed first and the output is allocated once.
pub fn encode(s: &GenesisSpec) -> Vec<u8> {
    let cp_payload: usize = s.cp.iter().map(|v| uint_len(&v.to_be_bytes())).sum();
    let entry = list_len(21 + 21);
    let m0_payload = s.m0_list.len() * entry;
    let payload = 1 + uint_len(&s.chain_id.to_be_bytes()) + list_len(cp_payload) + 33 + list_len(m0_payload) + 33;
    let mut out = Vec::with_capacity(list_len(payload));
    push_list_header(&mut out, payload);
    encode_uint(&[SPEC_VERSION], &mut out);
    encode_uint(&s.chain_id.to_be_bytes(), &mut out);
    push_list_header(&mut out, cp_payload);
    for v in &s.cp {
        encode_uint(&v.to_be_bytes(), &mut out);
    }
    rlp::encode_bytes(&s.alloc_root, &mut out);
    push_list_header(&mut out, m0_payload);
    for e in &s.m0_list {
        push_list_header(&mut out, 42);
        rlp::encode_bytes(&e.id, &mut out);
        rlp::encode_bytes(&e.reward_addr, &mut out);
    }
    rlp::encode_bytes(&s.sys_code_hash, &mut out);
    debug_assert_eq!(out.len(), list_len(payload));
    out
}

/// genesisHash = keccak256(GenesisSpec bytes); ForkSchedule is not included (consensus.md:11).
pub fn genesis_hash(b: &[u8]) -> H256 {
    keccak256(b)
}

/// GenesisSpec.verify, identity part only: decode, then bind the supplied genesisHash (L4n N11,
/// R24(a) first clause). allocRoot / sysCodeHash recomputation is LP4; the result is not bootable.
pub fn verify_hash(b: &[u8], expected: &H256) -> Result<GenesisSpec, GsError> {
    let spec = decode(b)?;
    if genesis_hash(b) != *expected {
        return gs("genesisHash", "R24(a)");
    }
    Ok(spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(b: &[u8]) -> &'static str {
        match decode(b) {
            Ok(_) => "ok",
            Err(e) => e.code,
        }
    }

    #[test]
    fn schema_shape() {
        assert_eq!(CP_SCHEMA.len(), 52);
        assert_eq!(cp_index("alpha_bp"), Some(ALPHA_BP));
        assert_eq!(cp_index("gamma_bp"), Some(14));
        assert_eq!(cp_index("H_END"), Some(45));
        assert_eq!(cp_index("P2"), Some(51));
        assert_eq!(cp_index("nope"), None);
        // The LP1 scoped table carries the same names and widths.
        for (k, x) in CP_SCHEMA.iter().enumerate() {
            assert_eq!((x.name, x.width), crate::genesis::CP_FIELDS[k]);
        }
    }

    #[test]
    fn framing_details() {
        assert_eq!(decode(&[]).unwrap_err(), GsError { code: "L0", detail: "truncated".into() });
        assert_eq!(decode(&[0xb8]).unwrap_err().detail, "truncated length");
        assert_eq!(decode(&[0xb8, 0x00]).unwrap_err().detail, "leading zero in length");
        assert_eq!(decode(&[0xb8, 0x37]).unwrap_err().detail, "long form for short string");
        assert_eq!(decode(&[0xf8, 0x37]).unwrap_err().detail, "long form for short list");
        assert_eq!(decode(&[0x82, 0x01]).unwrap_err().detail, "truncated string");
        assert_eq!(decode(&[0xc2, 0x01]).unwrap_err().detail, "truncated list");
        assert_eq!(decode(&[0xc1, 0x82, 0x01, 0x02]).unwrap_err().detail, "item crosses list end");
        assert_eq!(decode(&[0xc0, 0x00, 0x00]).unwrap_err().detail, "2 trailing bytes");
        // Declared lengths far beyond the input are rejected before any allocation.
        assert_eq!(decode(&[0xbf, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]).unwrap_err().detail, "truncated string");
        assert_eq!(decode(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]).unwrap_err().detail, "truncated list");
    }

    #[test]
    fn structure_precedes_version_and_wrapped_ints_are_gs_int() {
        assert_eq!(decode(&[0x80]).unwrap_err(), GsError { code: "gsStructure", detail: "top not a list".into() });
        assert_eq!(decode(&[0xc1, 0xc0]).unwrap_err(), GsError { code: "gsStructure", detail: "top[0]".into() });
        assert_eq!(code(&[0xc0]), "gsCount");
        assert_eq!(decode(&[0xc1, 0x02]).unwrap_err(), GsError { code: "gsVersion", detail: "2".into() });
        assert_eq!(decode(&[0xc1, 0x80]).unwrap_err().detail, "0");
        assert_eq!(decode(&[0xc1, 0x01]).unwrap_err(), GsError { code: "gsCount", detail: "top 1".into() });
        // 81 01 and 82 00 01 carry the value 1: version passes, count fails first here.
        assert_eq!(code(&[0xc2, 0x81, 0x01]), "gsCount");
        assert_eq!(code(&[0xc3, 0x82, 0x00, 0x01]), "gsCount");
    }

    #[test]
    fn reserved_ids() {
        let mut id = [0u8; 20];
        assert!(is_reserved(&id));
        assert!(is_reserved(&SYSTEM_ADDRESS));
        id[17..].copy_from_slice(&[0xc0, 0xc0, 0x01]);
        assert!(is_reserved(&id));
        id[17..].copy_from_slice(&[0xc0, 0xc0, 0xff]);
        assert!(is_reserved(&id));
        id[17..].copy_from_slice(&[0xc0, 0xc0, 0x00]);
        assert!(!is_reserved(&id));
        id[17..].copy_from_slice(&[0xc0, 0xc1, 0x00]);
        assert!(!is_reserved(&id));
        id[0] = 1;
        id[17..].copy_from_slice(&[0xc0, 0xc0, 0x01]);
        assert!(!is_reserved(&id));
    }

    #[test]
    fn bit_lengths() {
        assert_eq!(bit_len(&[]), 0);
        assert_eq!(bit_len(&[1]), 1);
        assert_eq!(bit_len(&[0xff]), 8);
        assert_eq!(bit_len(&[1, 0]), 9);
    }
}
