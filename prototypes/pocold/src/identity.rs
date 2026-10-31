//! Strict compatibility for one experimental testnet release, not remote attestation.
//! This is deliberately broader than a normative constants digest: all included Rust
//! sources, Cargo inputs and embedded production artifacts must match. Coordinated
//! upgrades are required; no automatic downgrade to the legacy handshake is allowed.
pub const PROTOCOL_VERSION: u64 = 2;
pub const COMPATIBILITY_HEX: &str = env!("POCOL_COMPATIBILITY_SHA256");
pub const BUILD_INFO: &str = include_str!(concat!(env!("OUT_DIR"), "/build-info.json"));

pub fn compatibility() -> [u8; 32] {
    let mut out = [0; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&COMPATIBILITY_HEX[2 * i..2 * i + 2], 16).expect("build digest");
    }
    out
}

pub fn matches(version: u64, digest: &[u8; 32]) -> bool {
    version == PROTOCOL_VERSION && *digest == compatibility()
}

/// Bind peers to the same locally approved activation heights as well as the same
/// sources. Legacy v1-only networks retain the existing handshake digest.
pub fn compatibility_for(forks: &[(u16, u64)]) -> [u8; 32] {
    if forks == [(1, 0)] { return compatibility(); }
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"pocol-consensus-schedule-v1\0");
    hash.update(compatibility());
    for (version, height) in forks {
        hash.update(version.to_be_bytes());
        hash.update(height.to_be_bytes());
    }
    hash.finalize().into()
}

pub fn matches_for(version: u64, digest: &[u8; 32], forks: &[(u16, u64)]) -> bool {
    version == PROTOCOL_VERSION && *digest == compatibility_for(forks)
}

#[cfg(test)]
mod schedule_tests {
    use super::*;
    #[test]
    fn different_activation_heights_cannot_connect() {
        let a = [(1, 0), (2, 100)];
        let b = [(1, 0), (2, 101)];
        assert_eq!(compatibility_for(&[(1, 0)]), compatibility());
        assert!(matches_for(PROTOCOL_VERSION, &compatibility_for(&a), &a));
        assert!(!matches_for(PROTOCOL_VERSION, &compatibility_for(&a), &b));
        assert!(!matches_for(PROTOCOL_VERSION, &compatibility(), &a));
    }
}
