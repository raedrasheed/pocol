use lp1_node::assignment::*;
use lp1_node::fixed::U256;
use lp1_node::genesis_spec::Address;

fn id(n: u8) -> Address {
    let mut a = [0u8; 20];
    a[19] = n;
    a
}
fn member(n: u8, last: u64, grace: u64, tomb: bool) -> ParentMember {
    ParentMember { id: id(n), last_share: last, grace_start: grace, tomb }
}
fn members(height: u64, act: &[ParentMember], window: u32) -> Membership {
    Membership::from_parent(&ParentView { height: height - 1, seal: &[7; 32], act }, height, window, 65535).unwrap()
}

#[test]
fn membership_cutoff_grace_tombs_and_fallback() {
    let act = [member(3, 6, 6, false), member(2, 6, 7, false), member(1, 7, 0, false), member(4, 9, 9, true)];
    assert_eq!(members(10, &act, 3).ids(), &[id(1), id(2)]);
    assert_eq!(members(10, &act, 0).ids(), &[id(1), id(2), id(3)]);
    assert!(members(10, &[member(4, 9, 9, true)], 0).ids().is_empty());
    assert_eq!(members(1, &[member(1, 0, 0, false)], u32::MAX).ids(), &[id(1)]);
}

#[test]
fn inconsistent_parent_views_are_errors_not_certificates() {
    let act = [member(1, 0, 0, false)];
    let parent = ParentView { height: 0, seal: &[0; 32], act: &act };
    assert_eq!(Membership::from_parent(&parent, 0, 1, 1).unwrap_err(), AssignmentError::ParentHeight);
    assert_eq!(Membership::from_parent(&parent, 2, 1, 1).unwrap_err(), AssignmentError::ParentHeight);
    assert_eq!(Membership::from_parent(&parent, 1, 1, 0).unwrap_err(), AssignmentError::MemberBound);
    assert!(members(10, &[], 0).ids().is_empty());
    let check = |act: &[ParentMember]| Membership::from_parent(&ParentView { height: 9, seal: &[0; 32], act }, 10, 3, 10);
    assert_eq!(check(&[member(1, 1, 1, false), member(1, 1, 1, false)]).unwrap_err(), AssignmentError::DuplicateMember);
    // Apply the specified cutoff to supplied facts; validating registry transitions is not this API's job.
    assert_eq!(check(&[member(1, 10, 0, false)]).unwrap().ids(), &[id(1)]);
    assert_eq!(check(&[member(1, 0, 10, false)]).unwrap().ids(), &[id(1)]);
    let mut zero = member(0, 0, 0, false);
    assert_eq!(check(&[zero.clone()]).unwrap_err(), AssignmentError::ReservedMember);
    for low in [0xC0_C001u32, 0xC0_C0FF] {
        zero.id = [0; 20];
        zero.id[16..].copy_from_slice(&low.to_be_bytes());
        assert_eq!(check(&[zero.clone()]).unwrap_err(), AssignmentError::ReservedMember);
    }
    zero.id = [255; 20];
    zero.id[19] = 254;
    assert_eq!(check(&[zero]).unwrap_err(), AssignmentError::ReservedMember);
}

#[test]
fn nonce_max_exact_work_and_full_64bit_space() {
    let cap_target = U256::pow2(192).unwrap().checked_sub(&U256::ONE).unwrap();
    assert_eq!(nonce_max(&cap_target, NonceMode::Bounded, 1), Ok(NONCE_SPACE));
    assert_eq!(nonce_max(&U256::pow2(192).unwrap(), NonceMode::Bounded, 1), Ok(u64::MAX as u128));
    assert_eq!(nonce_max(&U256::MAX, NonceMode::Bounded, 1), Ok(1));
    assert_eq!(nonce_max(&U256::MAX, NonceMode::Bounded, 255), Ok(255));
    assert_eq!(nonce_max(&U256::ONE, NonceMode::Bounded, 1), Ok(NONCE_SPACE));
    assert_eq!(nonce_max(&U256::pow2(240).unwrap(), NonceMode::Bounded, 2), Ok(131070));
    assert_eq!(nonce_max(&U256::MAX, NonceMode::Unbounded, 1), Ok(NONCE_SPACE));
    assert_eq!(nonce_max(&U256::ZERO, NonceMode::Unbounded, 1), Err(AssignmentError::ZeroTargetOrC));
    assert_eq!(nonce_max(&U256::ONE, NonceMode::Unbounded, 0), Err(AssignmentError::ZeroTargetOrC));
}

#[test]
fn exact_seed_ticket_and_proposer_hash_domains() {
    let mut seal = [0u8; 32];
    for (i, b) in seal.iter_mut().enumerate() {
        *b = i as u8;
    }
    let seed = nonce_seed(&seal, 0x0102030405060708, 0x090a0b0c);
    assert_eq!(lp1_node::hex::encode(&seed), "07f7c97972f41ec183ed19cbb9a1078780a59e221aee9eaa53d1083a75639a92");
    let mut address = [0; 20];
    for (i, b) in address.iter_mut().enumerate() {
        *b = i as u8 + 1;
    }
    assert_eq!(lp1_node::hex::encode(&nonce_ticket(&seed, &address)), "bfc3c8e3f75ad20dfd0c33f6e5b4e0a202c9c63264cd737cb5ed35e934483df6");
    assert_eq!(lp1_node::hex::encode(&proposer_digest(&seal, 0x0102030405060708, 1)), "c9469b49743c53c6e376fef464cd7b23686748a5519b0fbb854364f33be4e70b");
}

#[test]
fn partition_empty_ranges_completeness_and_owner_edges() {
    let act = [member(3, 9, 0, false), member(1, 9, 0, false), member(2, 9, 0, false)];
    let m = members(10, &act, 3);
    let one = Assignment::new(&m, 0, &U256::MAX, NonceMode::Bounded, 1).unwrap();
    assert_eq!(one.ranges().iter().filter(|r| r.inclusive_end().is_none()).count(), 2);
    assert_eq!(one.owner(0), Some(one.ranges()[2].id()));
    assert_eq!(one.owner(1), None);
    for mode in [NonceMode::Bounded, NonceMode::Unbounded] {
        let map = Assignment::new(&m, u32::MAX, &U256::MAX, mode, 7).unwrap();
        assert_eq!(map.ranges()[0].start(), 0);
        assert_eq!(map.ranges().last().unwrap().end_exclusive(), map.nonce_max());
        for pair in map.ranges().windows(2) {
            assert_eq!(pair[0].end_exclusive(), pair[1].start());
            assert!(pair[0].ticket() < pair[1].ticket());
        }
        for r in map.ranges() {
            if r.start() < r.end_exclusive() {
                assert_eq!(map.owner(r.start() as u64), Some(r.id()));
                assert_eq!(map.owner(r.inclusive_end().unwrap()), Some(r.id()));
            }
        }
        if mode == NonceMode::Unbounded {
            assert!(map.owner(u64::MAX).is_some());
        } else {
            for n in 0..7 {
                assert_eq!(map.ranges().iter().filter(|r| r.start() <= n && n < r.end_exclusive()).count(), 1);
            }
            assert_eq!(map.owner(7), None);
        }
    }
    let reverse = members(10, &act.iter().cloned().rev().collect::<Vec<_>>(), 3);
    assert_eq!(Assignment::new(&m, 4, &U256::MAX, NonceMode::Bounded, 7), Assignment::new(&reverse, 4, &U256::MAX, NonceMode::Bounded, 7));
    assert_eq!(Assignment::new(&members(10, &[], 0), 0, &U256::MAX, NonceMode::Bounded, 1).unwrap_err(), AssignmentError::EmptyMembership);
}

#[test]
fn exact_weight_window_sorted_choice_uniform_fallback_and_duplicates() {
    let m = members(10, &[member(2, 9, 0, false), member(1, 9, 0, false)], 3);
    let rows = [
        ShareCount { height: 5, id: id(1), count: 100 },
        ShareCount { height: 6, id: id(1), count: 2 },
        ShareCount { height: 8, id: id(1), count: 3 },
        ShareCount { height: 7, id: id(2), count: 4 },
        ShareCount { height: 9, id: id(2), count: 100 },
        ShareCount { height: 7, id: id(3), count: 100 },
    ];
    let w = DrawWeights::new(&m, 3, &rows).unwrap();
    assert_eq!(w.weights(), &[(id(1), 5), (id(2), 4)]);
    for i in 0..9 {
        assert_eq!(w.choose(i), Ok(if i < 5 { id(1) } else { id(2) }));
    }
    assert_eq!(w.choose(9), Err(AssignmentError::IndexOutOfRange));
    assert_eq!(DrawWeights::new(&m, 0, &rows).unwrap().weights(), &[(id(1), 1), (id(2), 1)]);
    assert_eq!(DrawWeights::new(&members(1, &[member(1, 0, 0, false)], 1), u16::MAX, &[]).unwrap().total(), 1);
    assert_eq!(DrawWeights::new(&m, 3, &[rows[0].clone(), rows[0].clone()]).unwrap_err(), AssignmentError::DuplicateCount);
    assert_eq!(DrawWeights::new(&members(10, &[], 0), 1, &[]).unwrap_err(), AssignmentError::EmptyMembership);
}

#[test]
fn full_256bit_unbiased_rejection_boundaries() {
    assert!(matches!(UnbiasedSampler::new(0), Err(AssignmentError::ZeroWeight)));
    assert_eq!(UnbiasedSampler::new(1).unwrap().sample(&[255; 32]), Some(0));
    assert_eq!(UnbiasedSampler::new(2).unwrap().sample(&[255; 32]), Some(1));
    let s = UnbiasedSampler::new(3).unwrap();
    assert_eq!(s.sample(&[255; 32]), None);
    let mut digest = [255; 32];
    digest[31] = 254;
    assert_eq!(s.sample(&digest), Some(2));
    assert_eq!(UnbiasedSampler::new(u128::MAX).unwrap().sample(&[255; 32]), None);
    assert_eq!(UnbiasedSampler::new(u128::MAX).unwrap().sample(&digest), Some(u128::MAX - 1));
}

#[test]
fn seeded_partition_properties_and_permutation_invariance() {
    let mut seed = 0x533420261017u64;
    for n in 1..=32u8 {
        let act: Vec<_> = (1..=n).map(|i| member(i, 99, 0, false)).collect();
        let m = members(100, &act, 2);
        for _ in 0..8 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let map = Assignment::new(&m, seed as u32, &U256::MAX, NonceMode::Bounded, (seed % 255 + 1) as u8).unwrap();
            let mut end = 0;
            for r in map.ranges() {
                assert_eq!(r.start(), end);
                assert!(r.start() <= r.end_exclusive());
                end = r.end_exclusive();
            }
            assert_eq!(end, map.nonce_max());
            for nonce in 0..map.nonce_max() as u64 {
                assert!(map.owner(nonce).is_some());
            }
            assert_eq!(map.owner(map.nonce_max() as u64), None);
        }
    }
}
