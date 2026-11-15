//! The nonce domain rules that hold whatever the fork schedule (moved from tests/full_nonce.rs, whose fork-v2
//! activation of a bounded spec-1 chain was deleted with the spec-1 rule set in phase 6a): the miner at the maximum
//! nonce, the full-domain assignment (nonceMode 1, every network since phase 6a) against the bounded one LP1 keeps
//! (NonceMode::Bounded, lp1-node), and the full-nonce rule of every fork version (fork 2 is a no-op under nonceMode 1).
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

#[test]
fn the_full_nonce_domain_holds_from_version_two_onwards_not_only_at_two() {
    for (version, nonce_mode, full) in [(3, 0, true), (7, 0, true), (2, 0, true), (1, 1, true), (1, 0, false), (u16::MAX, 0, true)] {
        assert_eq!(pocold::chain::full_nonce_rule(version, nonce_mode), full, "version {version}, nonceMode {nonce_mode}");
    }
}
