//! Who peers (VA-SPEC §2): a consensus identity that anyone can compute from public data, plus a wire version range.
//! The identity commits to the chain, the genesis (and through it CP, sysCodeHash and the alloc), the consensus rules
//! revision, the caps-test and unpinned flags and the full fork schedule, so two nodes peer only when they run the same
//! rules on the same chain with the same schedule, whatever their build digest. It is self-declared, not remote
//! attestation; block validation stays authoritative. The build digest (`COMPATIBILITY_HEX`) is informational status.
use sha2::{Digest, Sha256};
/// The highest wire version this build speaks; Hello item 7 carries the sender's maximum (VA-SPEC §2.2).
pub const PROTOCOL_VERSION: u64 = 2;
/// The lowest wire version a peer may declare (VA-SPEC §2.2). The 11-item Hello layout is frozen across every accepted
/// range, so changing it requires raising this.
pub const PROTOCOL_VERSION_MIN: u64 = 2;
/// The consensus rules revision (VA-SPEC §2.3). Raised only by a `bump` history line in consensus-sources.lock,
/// and only together with a new genesis; the consensus guard checks the lock agrees. Rules 2 are the v3 rules of the
/// fresh network (spec 2 decoded by default, FN-2a/FN-2b); invitations and releases of rules 1 are refused at import.
pub const CONSENSUS_RULES_VERSION: u32 = 2;
/// The build digest of all included sources: install-integrity status for operators, never a peering condition.
pub const COMPATIBILITY_HEX: &str = env!("POCOL_COMPATIBILITY_SHA256");
pub const BUILD_INFO: &str = include_str!(concat!(env!("OUT_DIR"), "/build-info.json"));
/// Whether build.rs found the consensus-boundary aggregate equal to the lock's last history line (plan §1.5).
pub const PINNED: bool = matches!(env!("POCOL_CONSENSUS_PINNED").as_bytes(), b"1");
/// The boundary aggregate of an unpinned tree (64 hex), empty when pinned: an unpinned build appends it to its
/// identity, so it peers only with byte-identical unpinned trees (attack finding A1-2).
pub const SOURCES: &str = env!("POCOL_CONSENSUS_SOURCES");
/// Identity flag bits (§1.1). Caps-test is lp4's own feature, the crate whose LOG_CAP it changes (A1-3).
pub const FLAG_CAPS_TEST: u8 = 1;
pub const FLAG_UNPINNED: u8 = 2;
/// The fork schedule entries an identity can carry (`ForkSchedule` allows 1..=16).
pub const FORKS_MAX: usize = 16;

/// This build's flags: bit0 caps-test (`pocol_lp4_execution::CAPS_TEST`), bit1 unpinned.
pub fn flags() -> u8 {
    (pocol_lp4_execution::CAPS_TEST as u8 * FLAG_CAPS_TEST)
        | if PINNED { 0 } else { FLAG_UNPINNED }
}
/// Unpinned builds append their actual aggregate. build.rs supplies and validates this value.
pub fn sources() -> Option<[u8; 32]> {
    if PINNED { return None }
    Some(unhex32(env!("POCOL_CONSENSUS_SOURCES_ACTUAL")).expect("build.rs supplies the actual 64-hex boundary aggregate"))
}
fn unhex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) { return None }
    let mut out = [0; 32];
    for (i, b) in out.iter_mut().enumerate() { *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?; }
    Some(out)
}
fn hex(b: &[u8]) -> String { b.iter().map(|x| format!("{x:02x}")).collect() }

/// The consensus identity (VA-SPEC §2.1), pure:
/// SHA-256("pocol-consensus-identity-v1\0" ‖ chainId u64 ‖ genesisHash ‖ rules u32 ‖ flags u8 ‖ n u8 ‖
/// (version u16 ‖ height u64)×n ‖ [sources, with bit1]), integers big-endian. `sources` must be given exactly
/// when unpinned is set and `forks` must hold 1..=16 entries (a validated schedule); anything else is a caller bug.
pub fn consensus_id(chain_id: u64, genesis: &[u8; 32], rules: u32, flags: u8, forks: &[(u16, u64)], sources: Option<[u8; 32]>) -> [u8; 32] {
    assert!((1..=FORKS_MAX).contains(&forks.len()), "a consensus identity covers a validated schedule");
    assert_eq!(flags & FLAG_UNPINNED != 0, sources.is_some(), "the unpinned flag requires the aggregate");
    let mut h = Sha256::new();
    h.update(b"pocol-consensus-identity-v1\0");
    h.update(chain_id.to_be_bytes());
    h.update(genesis);
    h.update(rules.to_be_bytes());
    h.update([flags, forks.len() as u8]);
    for (v, height) in forks { h.update(v.to_be_bytes()); h.update(height.to_be_bytes()); }
    if let Some(s) = sources { h.update(s); }
    h.finalize().into()
}

/// What this node peers on, set once from its spec (`Consensus::of`). The inputs are kept so that `explain` can name
/// what a refused peer differs in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Consensus { pub chain_id: u64, pub genesis: [u8; 32], pub forks: Vec<(u16, u64)>, pub rules: u32, pub flags: u8, pub sources: Option<[u8; 32]>, pub id: [u8; 32] }
impl Consensus {
    pub fn new(chain_id: u64, genesis: [u8; 32], forks: &[(u16, u64)], rules: u32, flags: u8, sources: Option<[u8; 32]>) -> Self {
        Self { chain_id, genesis, forks: forks.to_vec(), rules, flags, sources, id: consensus_id(chain_id, &genesis, rules, flags, forks, sources) }
    }
    /// This build's identity on `spec`'s chain and configured schedule.
    pub fn of(spec: &crate::chain::Spec) -> Self {
        Self::new(spec.chain_id(), spec.genesis_hash(), spec.profile().forks().entries(), CONSENSUS_RULES_VERSION, flags(), sources())
    }
    pub fn hex(&self) -> String { hex(&self.id) }
    fn with(&self, forks: &[(u16, u64)], rules: u32, flags: u8, sources: Option<[u8; 32]>) -> [u8; 32] {
        consensus_id(self.chain_id, &self.genesis, rules, flags, forks, sources)
    }
}

/// Why a Hello is refused: another chain or genesis, a wire version below `PROTOCOL_VERSION_MIN` (the value it
/// declared), or another consensus identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mismatch { Network, Wire(u64), Consensus }
/// The one acceptance predicate (VA-SPEC §2): both inbound eviction (`net::Net::eligible`) and the node's close
/// decision use it, so they cannot diverge. Ok holds the session's wire version, min(peer's maximum, ours).
pub fn accepts(local: &Consensus, chain_id: u64, genesis: &[u8; 32], protocol: u64, id: &[u8; 32]) -> Result<u64, Mismatch> {
    if chain_id != local.chain_id || *genesis != local.genesis { return Err(Mismatch::Network) }
    if protocol < PROTOCOL_VERSION_MIN { return Err(Mismatch::Wire(protocol)) }
    if *id != local.id { return Err(Mismatch::Consensus) }
    Ok(protocol.min(PROTOCOL_VERSION))
}

/// A refused consensus identity, explained for the log and status (VA-SPEC §2.5). Runs only on the close path, and
/// tries at most FORKS_MAX + RULES + 3 identities: a prefix of our schedule (the peer lacks an activation), another
/// rules revision, the other caps-test bit and, for an unpinned build, the pinned form. An unpinned peer's aggregate is
/// unknown here, so that case reads as the generic text.
pub fn explain(local: &Consensus, peer: &[u8; 32]) -> String {
    let (f, r, fl, s) = (&local.forks[..], local.rules, local.flags, local.sources);
    if let Some(k) = (1..f.len()).find(|&k| local.with(&f[..k], r, fl, s) == *peer) {
        let (v, h) = f[k];
        return format!("peer has not scheduled activation ({v},{h}): it must apply the published network upgrade");
    }
    if let Some(other) = (1..=r.saturating_add(1)).find(|&x| x != r && local.with(f, x, fl, s) == *peer) {
        return format!("peer runs consensus rules {other}; this node runs rules {r}");
    }
    if local.with(f, r, fl ^ FLAG_CAPS_TEST, s) == *peer {
        return if fl & FLAG_CAPS_TEST == 0 { "peer runs a caps-test build".into() } else { "this node runs a caps-test build; the peer does not".into() };
    }
    if fl & FLAG_UNPINNED != 0 {
        return if local.with(f, r, fl & !FLAG_UNPINNED, None) == *peer {
            "this node runs an unpinned (development) build; the peer runs the pinned rules: re-pin consensus-sources.lock or use a released build".into()
        } else {
            "this node runs an unpinned (development) build, which peers only with byte-identical trees".into()
        };
    }
    format!("different consensus rules, fork schedule or test build (ours {}…, theirs {}…). If this node has not applied a \
        published network upgrade, apply it (Settings → Upgrade or `pocol upgrade`) or update PoCol", &hex(&local.id)[..16], &hex(peer)[..16])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn local(forks: &[(u16, u64)]) -> Consensus { Consensus::new(7, [1; 32], forks, 1, 0, None) }
    #[test]
    fn a_scheduled_upgrade_identity_is_pinned() {
        // T11: a synthetic network (chain 777915, genesis 0x5a…5a) with one scheduled activation, computed from public
        // data only; deploy/public-testnet/test_pocol_identity.py pins the same value and rebuilds it with struct. The
        // new network's own golden identity is pinned once its genesis exists.
        let id = consensus_id(777915, &unhex32(&"5a".repeat(32)).unwrap(), 1, 0, &[(1, 0), (2, 1000)], None);
        assert_eq!(hex(&id), "633afe4beab508ba5ab174300e3961d6ebb72235b68b994d597c508c5abd3021");
    }
    #[test]
    fn different_heights_give_different_ids() {
        let (a, b) = (local(&[(1, 0), (2, 100)]), local(&[(1, 0), (2, 101)]));
        assert_ne!(a.id, b.id);
        assert_ne!(a.id, local(&[(1, 0)]).id);
        assert_eq!(accepts(&a, 7, &[1; 32], PROTOCOL_VERSION, &a.id), Ok(PROTOCOL_VERSION));
        assert_eq!(accepts(&a, 7, &[1; 32], PROTOCOL_VERSION, &b.id), Err(Mismatch::Consensus));
    }
    #[test]
    fn every_input_changes_the_id() {
        let base = consensus_id(7, &[1; 32], 1, 0, &[(1, 0)], None);
        for other in [consensus_id(8, &[1; 32], 1, 0, &[(1, 0)], None), consensus_id(7, &[2; 32], 1, 0, &[(1, 0)], None),
            consensus_id(7, &[1; 32], 2, 0, &[(1, 0)], None), consensus_id(7, &[1; 32], 1, FLAG_CAPS_TEST, &[(1, 0)], None),
            consensus_id(7, &[1; 32], 1, FLAG_UNPINNED, &[(1, 0)], Some([0; 32])), consensus_id(7, &[1; 32], 1, 0, &[(1, 0), (2, 0)], None)] {
            assert_ne!(base, other);
        }
        assert_ne!(consensus_id(7, &[1; 32], 1, FLAG_UNPINNED, &[(1, 0)], Some([0; 32])), consensus_id(7, &[1; 32], 1, FLAG_UNPINNED, &[(1, 0)], Some([1; 32])));
    }
    #[test]
    fn the_retired_preview_bit_is_not_a_flag_and_never_takes_an_aggregate(){
        // Bit 2 was the v3-preview build's flag; the v3 rules are the release rules (rules 2), so no build sets it and
        // a pinned identity never carries a source aggregate.
        assert_eq!(flags() & !(FLAG_CAPS_TEST | FLAG_UNPINNED), 0);
        assert!(std::panic::catch_unwind(||consensus_id(7,&[1;32],2,4,&[(1,0)],Some([8;32]))).is_err(),"only the unpinned bit takes the aggregate");
        assert!(std::panic::catch_unwind(||consensus_id(7,&[1;32],2,0,&[(1,0)],Some([8;32]))).is_err());
    }

    #[test]
    fn the_wire_range_accepts_any_maximum_from_min_and_runs_at_the_lower() {
        let a = local(&[(1, 0)]);
        assert_eq!(accepts(&a, 7, &[1; 32], PROTOCOL_VERSION_MIN, &a.id), Ok(PROTOCOL_VERSION_MIN));
        assert_eq!(accepts(&a, 7, &[1; 32], PROTOCOL_VERSION + 1, &a.id), Ok(PROTOCOL_VERSION));
        assert_eq!(accepts(&a, 7, &[1; 32], PROTOCOL_VERSION_MIN - 1, &a.id), Err(Mismatch::Wire(PROTOCOL_VERSION_MIN - 1)));
        // Another network is named first, whatever else differs.
        assert_eq!(accepts(&a, 8, &[1; 32], 0, &[0; 32]), Err(Mismatch::Network));
        assert_eq!(accepts(&a, 7, &[2; 32], PROTOCOL_VERSION, &a.id), Err(Mismatch::Network));
    }
    #[test]
    fn explain_names_what_a_refused_peer_differs_in() {
        let ours = local(&[(1, 0), (2, 1000)]);
        assert_eq!(explain(&ours, &local(&[(1, 0)]).id), "peer has not scheduled activation (2,1000): it must apply the published network upgrade");
        assert_eq!(explain(&ours, &Consensus::new(7, [1; 32], &ours.forks, 2, 0, None).id), "peer runs consensus rules 2; this node runs rules 1");
        assert_eq!(explain(&ours, &Consensus::new(7, [1; 32], &ours.forks, 1, FLAG_CAPS_TEST, None).id), "peer runs a caps-test build");
        // The reverse of a missing activation: this node is the one behind, so the text carries the instruction.
        let behind = explain(&local(&[(1, 0)]), &ours.id);
        assert!(behind.starts_with("different consensus rules, fork schedule or test build") && behind.contains("`pocol upgrade`"), "{behind}");
        let unpinned_peer = Consensus::new(7, [1; 32], &ours.forks, 1, FLAG_UNPINNED, Some([9; 32]));
        assert!(explain(&ours, &unpinned_peer.id).starts_with("different consensus rules"));
        assert!(explain(&unpinned_peer, &ours.id).starts_with("this node runs an unpinned (development) build; the peer runs the pinned rules"));
        assert!(explain(&unpinned_peer, &[0; 32]).ends_with("peers only with byte-identical trees"));
    }
    #[test]
    fn this_builds_flags_follow_the_lp4_caps_test_and_the_pinned_tree() {
        // T9: bit0 is lp4's CAPS_TEST (the crate that changes LOG_CAP), bit1 is set exactly when the tree is unpinned.
        assert_eq!(flags() & FLAG_CAPS_TEST != 0, pocol_lp4_execution::CAPS_TEST);
        assert_eq!(flags() & FLAG_UNPINNED != 0, !PINNED);
        assert_eq!(sources().is_some(), !PINNED);
        assert_eq!(flags() & !(FLAG_CAPS_TEST | FLAG_UNPINNED), 0);
    }
    #[test]
    fn build_info_reports_the_identity_constants() {
        // T18.
        let info: serde_json::Value = serde_json::from_str(BUILD_INFO).unwrap();
        assert_eq!(info["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(info["protocolVersionMin"], PROTOCOL_VERSION_MIN);
        assert_eq!(info["consensusRulesVersion"], CONSENSUS_RULES_VERSION);
        assert_eq!(info["maxForkVersion"], pocol_lp4_execution::profile::MAX_FORK_VERSION);
        assert_eq!(info["consensusPinned"], PINNED);
        assert!(info.get("v3Preview").is_none(), "no build is a v3-preview build");
    }
}
