//! The consensus identity a test peer's Hello carries for a node on `spec` (VA-SPEC §2.1): what the node sends and
//! checks, never the build digest.
pub fn hello_identity(spec: &pocold::chain::Spec) -> [u8; 32] { pocold::identity::Consensus::of(spec).id }
