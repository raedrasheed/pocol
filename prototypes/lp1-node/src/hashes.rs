//! SHA-256 / Keccak-256, the V1 domain messages (consensus.md:59, v1_ref_027.py) and secp256k1
//! public-key recovery with explicit low-S, r/s range and v in {0,1} checks. Verification only:
//! this crate never signs and holds no private keys.

use sha2::Digest;

pub type H256 = [u8; 32];

pub fn sha256(data: &[u8]) -> H256 {
    let d = sha2::Sha256::digest(data);
    let mut o = [0u8; 32];
    o.copy_from_slice(&d);
    o
}

pub fn keccak256(data: &[u8]) -> H256 {
    let d = sha3::Keccak256::digest(data);
    let mut o = [0u8; 32];
    o.copy_from_slice(&d);
    o
}

/// TemplateID preimage is RLP(UT); powHash / shareHash preimage is tid || be64(n).
pub fn tid_nonce_preimage(tid: &H256, n: u64) -> [u8; 40] {
    let mut p = [0u8; 40];
    p[..32].copy_from_slice(tid);
    p[32..].copy_from_slice(&n.to_be_bytes());
    p
}

/// keccak(0x19 || "PoCol template" || 0x0a || tid)
pub fn sig_msg_preimage(tid: &H256) -> Vec<u8> {
    let mut p = Vec::with_capacity(48);
    p.push(0x19);
    p.extend_from_slice(b"PoCol template");
    p.push(0x0a);
    p.extend_from_slice(tid);
    p
}

pub fn sig_msg(tid: &H256) -> H256 {
    keccak256(&sig_msg_preimage(tid))
}

/// keccak(0x19 || "PoCol winner" || 0x0a || tid || be64(nonce) || shareRoot)
pub fn win_msg_preimage(tid: &H256, nonce: u64, share_root: &H256) -> Vec<u8> {
    let mut p = Vec::with_capacity(86);
    p.push(0x19);
    p.extend_from_slice(b"PoCol winner");
    p.push(0x0a);
    p.extend_from_slice(tid);
    p.extend_from_slice(&nonce.to_be_bytes());
    p.extend_from_slice(share_root);
    p
}

pub fn win_msg(tid: &H256, nonce: u64, share_root: &H256) -> H256 {
    keccak256(&win_msg_preimage(tid, nonce, share_root))
}

/// shareRoot = keccak(RLP(shareItems)); the argument is the exact list encoding.
pub fn share_root(share_list_rlp: &[u8]) -> H256 {
    keccak256(share_list_rlp)
}

pub fn block_hash_preimage(tid: &H256, nonce: u64, share_root: &H256) -> [u8; 72] {
    let mut p = [0u8; 72];
    p[..32].copy_from_slice(tid);
    p[32..40].copy_from_slice(&nonce.to_be_bytes());
    p[40..].copy_from_slice(share_root);
    p
}

/// blockHash = keccak(tid || be64(nonce) || shareRoot) for a non-genesis header.
pub fn block_hash(tid: &H256, nonce: u64, share_root: &H256) -> H256 {
    keccak256(&block_hash_preimage(tid, nonce, share_root))
}

/// secp256k1 group order n and floor(n/2), big-endian.
pub const N_ORDER: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe, 0xba, 0xae, 0xdc, 0xe6, 0xaf, 0x48, 0xa0, 0x3b, 0xbf, 0xd2,
    0x5e, 0x8c, 0xd0, 0x36, 0x41, 0x41,
];
pub const HALF_N: [u8; 32] = [
    0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x5d, 0x57, 0x6e, 0x73, 0x57, 0xa4, 0x50, 0x1d, 0xdf, 0xe9,
    0x2f, 0x46, 0x68, 0x1b, 0x20, 0xa0,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SigFault {
    Length,
    RRange,
    SRange,
    HighS,
    BadV,
    NotRecoverable,
}

/// Shape and range checks of the 65-byte r || s || v form, in the order of v1_ref_027.recover.
pub fn check_sig_shape(sig: &[u8]) -> Result<([u8; 64], u8), SigFault> {
    if sig.len() != 65 {
        return Err(SigFault::Length);
    }
    let mut rs = [0u8; 64];
    rs.copy_from_slice(&sig[..64]);
    let r = &sig[..32];
    let s = &sig[32..64];
    let zero = [0u8; 32];
    if r == &zero[..] || r >= &N_ORDER[..] {
        return Err(SigFault::RRange);
    }
    if s == &zero[..] {
        return Err(SigFault::SRange);
    }
    if s > &HALF_N[..] {
        return Err(SigFault::HighS);
    }
    let v = sig[64];
    if v > 1 {
        return Err(SigFault::BadV);
    }
    Ok((rs, v))
}

/// Recovers the 20-byte address keccak(pub64)[12..] that produced `sig` over `msg`.
pub fn recover_address(msg: &H256, sig: &[u8]) -> Result<[u8; 20], SigFault> {
    let (rs, v) = check_sig_shape(sig)?;
    // The recovery primitive comes from the including crate: libsecp256k1 in lp1-node (accepted
    // Rust 1.58.1 toolchain), k256 in pocold (audit 2026-10-22, D-DEP-SECP).
    let ser = crate::secp_backend::recover_uncompressed(msg, &rs, v)?;
    let k = keccak256(&ser[1..]);
    let mut a = [0u8; 20];
    a.copy_from_slice(&k[12..]);
    Ok(a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex;

    #[test]
    fn keccak_known_answers_k1_k3() {
        assert_eq!(hex::encode(&keccak256(b"")), "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470");
        assert_eq!(hex::encode(&keccak256(&[0xc0])), "1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347");
        assert_eq!(hex::encode(&keccak256(&[0x80])), "56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421");
    }

    #[test]
    fn sha256_known_answer() {
        assert_eq!(hex::encode(&sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn domain_preimages_layout() {
        let tid = [7u8; 32];
        let p = sig_msg_preimage(&tid);
        assert_eq!(p.len(), 1 + 14 + 1 + 32);
        assert_eq!(&p[1..15], b"PoCol template");
        let w = win_msg_preimage(&tid, 0x0102, &[9u8; 32]);
        assert_eq!(w.len(), 1 + 12 + 1 + 32 + 8 + 32);
        assert_eq!(&w[46..54], &[0, 0, 0, 0, 0, 0, 1, 2]);
        assert_eq!(&tid_nonce_preimage(&tid, 1)[32..], &[0, 0, 0, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn signature_shape_faults() {
        let mut sig = [1u8; 65];
        sig[64] = 0;
        assert!(check_sig_shape(&sig).is_ok());
        assert_eq!(check_sig_shape(&sig[..64]), Err(SigFault::Length));
        let mut hs = sig;
        hs[32..64].copy_from_slice(&HALF_N);
        assert!(check_sig_shape(&hs).is_ok()); // s == n/2 is low
        hs[63] += 1;
        assert_eq!(check_sig_shape(&hs), Err(SigFault::HighS));
        let mut bv = sig;
        bv[64] = 27;
        assert_eq!(check_sig_shape(&bv), Err(SigFault::BadV));
        let mut zr = sig;
        zr[..32].copy_from_slice(&[0u8; 32]);
        assert_eq!(check_sig_shape(&zr), Err(SigFault::RRange));
        let mut nr = sig;
        nr[..32].copy_from_slice(&N_ORDER);
        assert_eq!(check_sig_shape(&nr), Err(SigFault::RRange));
        let mut zs = sig;
        zs[32..64].copy_from_slice(&[0u8; 32]);
        assert_eq!(check_sig_shape(&zs), Err(SigFault::SRange));
    }
}
