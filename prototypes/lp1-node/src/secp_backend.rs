//! secp256k1 public-key recovery behind `hashes::recover_address`: libsecp256k1 0.5.0, the accepted
//! LP1/LP2 dependency for Rust 1.58.1. pocold compiles the same `hashes.rs` with its own backend
//! (k256). Both give the same verdicts on every tested input
//! (development/audit-2026-10-22/evidence/secp-equivalence.md).

use crate::hashes::{SigFault, H256};

/// Uncompressed 65-byte public key for a signature that already passed `check_sig_shape`.
pub fn recover_uncompressed(msg: &H256, rs: &[u8; 64], v: u8) -> Result<[u8; 65], SigFault> {
    let m = libsecp256k1::Message::parse(msg);
    let s = libsecp256k1::Signature::parse_standard_slice(rs).map_err(|_| SigFault::RRange)?;
    let id = libsecp256k1::RecoveryId::parse(v).map_err(|_| SigFault::BadV)?;
    let pk = libsecp256k1::recover(&m, &s, &id).map_err(|_| SigFault::NotRecoverable)?;
    Ok(pk.serialize())
}
