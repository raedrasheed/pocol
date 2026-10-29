//! secp256k1 public-key recovery behind the shared `hashes::recover_address` (lp1-node/src/hashes.rs).
//! pocold uses k256 0.13.4, as lp4-execution does, instead of the unmaintained libsecp256k1 0.5.0. The
//! two give the same verdicts and keys on every tested input (D-DEP-SECP,
//! development/audit-2026-10-22/evidence/secp-equivalence.md).
use crate::hashes::{SigFault,H256};

/// Uncompressed 65-byte public key for a signature that already passed `check_sig_shape`.
pub fn recover_uncompressed(msg:&H256,rs:&[u8;64],v:u8)->Result<[u8;65],SigFault>{
    let s=k256::ecdsa::Signature::from_slice(rs).map_err(|_|SigFault::RRange)?;
    let id=k256::ecdsa::RecoveryId::from_byte(v).ok_or(SigFault::BadV)?;
    let pk=k256::ecdsa::VerifyingKey::recover_from_prehash(msg,&s,id).map_err(|_|SigFault::NotRecoverable)?;
    let mut o=[0u8;65];o.copy_from_slice(pk.to_encoded_point(false).as_bytes());Ok(o)
}
