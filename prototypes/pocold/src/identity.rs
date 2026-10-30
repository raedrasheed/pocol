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
