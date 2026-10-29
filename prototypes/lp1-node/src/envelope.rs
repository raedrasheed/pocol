//! M3/T0 stateless consensus envelope entry point, distinct from legacy scoped RP parsing.
//! No parent-state/Draw/ownership, execution or H-full certificate is implied.
use crate::{
    hashes,
    header::{self, DecodeFail, Header},
    window::NetConfig,
};

/// Rule 1 includes canonical signatures and strict explicit-share ordering.
pub fn decode(bytes: &[u8]) -> Result<Header, DecodeFail> {
    let h = header::decode(bytes)?;
    if !h.sig.is_empty() && hashes::check_sig_shape(&h.sig).is_err() {
        return Err(DecodeFail { detail: "signature shape" });
    }
    if hashes::check_sig_shape(&h.winner_sig).is_err() {
        return Err(DecodeFail { detail: "winner signature shape" });
    }
    if h.shares.windows(2).any(|v| v[0] >= v[1]) || h.shares.contains(&h.nonce) {
        return Err(DecodeFail { detail: "share order/winner" });
    }
    Ok(h)
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnosis {
    pub first: Option<&'static str>,
    pub violated: Vec<&'static str>,
    pub not_evaluable: Vec<&'static str>,
}
/// Evaluate only stateless Rule 1, network identity and Rule 3. No supplied-ok flags.
pub fn diagnose(bytes: &[u8], cfg: &NetConfig) -> Diagnosis {
    let h = match decode(bytes) {
        Ok(h) => h,
        Err(_) => return Diagnosis { first: Some("1"), violated: vec!["1"], not_evaluable: vec!["parent-dependent rules", "state/execution rules"] },
    };
    let mut v = Vec::new();
    if h.chain_id != cfg.chain_id {
        v.push("netChain");
    }
    if h.genesis_hash != cfg.genesis_hash {
        v.push("netGenesis");
    }
    if cfg.active_version(h.h) != Some(h.protocol_version as u64) {
        v.push("netVersion");
    }
    if h.is_signed() {
        let valid = h.a == 0
            && h.proposer.len() == 20
            && h.sig.len() == 65
            && hashes::recover_address(&hashes::sig_msg(&hashes::sha256(&h.ut_raw)), &h.sig).map_or(false, |id| id.as_slice() == h.proposer.as_slice());
        if !valid {
            v.push("3");
        }
    }
    Diagnosis { first: v.first().copied(), violated: v, not_evaluable: vec!["parent-dependent rules", "state/execution rules"] }
}
