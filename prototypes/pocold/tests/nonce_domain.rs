//! The nonce domain rules that hold whatever the fork schedule (moved from tests/full_nonce.rs, whose fork-v2
//! activation of a bounded spec-1 chain was deleted with the spec-1 rule set in phase 6a): the miner at the maximum
//! nonce, the full-domain assignment (nonceMode 1, every network since phase 6a) against the bounded one LP1 keeps
//! (NonceMode::Bounded, lp1-node), and the full nonce domain of an admitted spec at every fork version (fork 2 is a
//! no-op under nonceMode 1, which admission requires).
use std::sync::mpsc;
use std::time::Duration;

use pocold::{assignment::NonceMode, miner::{self, MinerEvent}, types::*};

#[test]
fn miner_handles_the_maximum_nonce_and_full_range_cancellation() {
    let (send, receive) = mpsc::channel();
    let handle = miner::start([11; 32], u64::MAX, u64::MAX, Fixed::MAX, Fixed::MAX, 256, send).expect("spawn miner");
    assert!(matches!(receive.recv_timeout(Duration::from_secs(2)).unwrap(), MinerEvent::Solution { nonce: u64::MAX, .. }));
    drop(handle);
    let (send, receive) = mpsc::channel();
    let handle = miner::start([12; 32], u64::MAX, u64::MAX, Fixed::ZERO, Fixed::ZERO, 0, send).expect("spawn miner");
    assert!(matches!(receive.recv_timeout(Duration::from_secs(2)).unwrap(), MinerEvent::Exhausted { hashes: 1, .. }));
    drop(handle);
    let (send, receive) = mpsc::channel();
    let handle = miner::start([13; 32], 0, u64::MAX, Fixed::ZERO, Fixed::ZERO, 0, send).expect("spawn miner");
    handle.stop();
    assert!(matches!(receive.recv_timeout(Duration::from_secs(2)), Err(mpsc::RecvTimeoutError::Disconnected)));
    drop(handle);
}

#[test]
fn difficulty_one_changes_from_two_nonce_slots_to_ranges_for_all_seven_members() {
    use pocold::assignment::{Assignment, Membership, ParentMember, ParentView};
    let act: Vec<_> = (1..=7u8).map(|n| {
        let mut id = [0; 20];
        id[19] = n;
        ParentMember { id, tomb: false, last_share: 0, grace_start: 0 }
    }).collect();
    let membership = Membership::from_parent(&ParentView { height: 0, seal: &[0; 32], act: &act }, 1, 32, 64, false).unwrap();
    assert_eq!(pocold::fixed::work(&Fixed::MAX), Fixed::ONE);
    let old = Assignment::new(&membership, 0, &Fixed::MAX, NonceMode::Bounded, 2).unwrap();
    assert_eq!(old.nonce_max(), 2);
    assert_eq!(old.ranges().iter().filter(|r| r.inclusive_end().is_some()).count(), 2);
    let new = Assignment::new(&membership, 0, &Fixed::MAX, NonceMode::Unbounded, 2).unwrap();
    assert_eq!(new.nonce_max(), 1u128 << 64);
    assert_eq!(new.ranges().iter().filter(|r| r.inclusive_end().is_some()).count(), 7);
    assert_eq!(new.ranges()[0].start(), 0);
    assert_eq!(new.ranges().last().unwrap().inclusive_end(), Some(u64::MAX));
    for pair in new.ranges().windows(2) {
        assert_eq!(pair[0].end_exclusive(), pair[1].start());
    }
}

/// An admitted spec with the fork schedule [(1,0),(2,V2),(3,H)] (fork 2 a nonce-rule no-op, fork 3 the FN-5 cap raise,
/// as tests/fn5_cap_raise.rs `spec_of` builds it): its nonce mode is the full domain at every height, before fork 2, at
/// each activation and past the last, and the nonce space is 2^64 even at the easiest target (work 1, where a bounded
/// range would hold c nonces). The fork versions themselves stay as scheduled (activeVersion is kept for later forks).
#[test]
fn the_full_nonce_domain_holds_at_every_fork_version() {
    use pocol_lp4_execution::{devnet::{self, DevnetConfig}, profile::CapRaise};
    const V2: u64 = 3;
    const H: u64 = 6;
    let c = DevnetConfig::new(777910, 1_700_000_000, &["a", "b", "c"]);
    let mut d = devnet::build(&c).unwrap();
    d.forks = vec![(1, 0), (2, V2), (3, H)];
    d.cap_raise = Some(CapRaise { code_cap: c.code_cap, slot_cap: c.slot_cap, acct_cap: c.acct_cap + 8, sys_slot_max: c.sys_slot_max });
    let spec = pocold::chain::Spec::new(d.admit(&pocold::single_host_local()).unwrap_or_else(|e| panic!("{e}")).0);
    assert_eq!(spec.cp().nonceMode, 1, "admission requires nonceMode 1");
    for (h, version) in [(0, 1), (1, 1), (V2 - 1, 1), (V2, 2), (H - 1, 2), (H, 3), (H + 1, 3), (u64::MAX, 3)] {
        assert_eq!(spec.active_version(h), version, "h={h}");
        assert_eq!(spec.effective_nonce_mode(h), NonceMode::Unbounded, "h={h}, version {version}");
        let full = pocold::assignment::nonce_max(&Fixed::MAX, spec.effective_nonce_mode(h), spec.cp_at(h).c).unwrap();
        assert_eq!(full, 1u128 << 64, "h={h}, version {version}: the full nonce domain");
    }
    assert!(pocold::assignment::nonce_max(&Fixed::MAX, NonceMode::Bounded, spec.cp().c).unwrap() < 1u128 << 64, "positive control: a bounded range at work 1 is narrower");
}
