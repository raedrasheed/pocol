#[path="support/admission.rs"] mod f;
use pocol_lp4_execution::{profile::{ForkSchedule,ProfileError,ProfileIdentity,ProfileKind,NodeKind,MAX_FORK_VERSION},pool::Pool,user_phase};

#[test]
fn full_nonce_profile_keeps_genesis_and_requires_an_explicit_valid_schedule() {
    let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!("data/profile-execution.json")).unwrap();
    let row = &rows[0];
    let bytes = f::raw(row["genesisSpec"].as_str().unwrap());
    let hash = f::raw(row["genesisHash"].as_str().unwrap()).try_into().unwrap();
    let chain = row["chain"].as_u64().unwrap();
    let kind = if chain == 777001 { ProfileKind::Network } else { ProfileKind::Synthetic };
    let node = if chain == 777001 { NodeKind::Node } else { NodeKind::Simnet };
    let original = ProfileIdentity::validate(&bytes, &hash, chain, kind, node, &[(1, 0)]).unwrap();
    let upgraded = ProfileIdentity::validate(&bytes, &hash, chain, kind, node, &[(1, 0), (2, 50)]).unwrap();
    assert_eq!(upgraded.genesis_hash(), original.genesis_hash());
    assert_eq!(upgraded.spec(), original.spec());
    assert_eq!(upgraded.cp(), original.cp());
    assert_eq!(original.forks().active_version(u64::MAX), 1);
    assert_eq!(upgraded.forks().active_version(49), 1);
    assert_eq!(upgraded.forks().active_version(50), 2);
    assert_eq!(upgraded.forks().active_version(51), 2);
    assert!(!upgraded.bootable());
    assert_eq!(ForkSchedule::new(&[(1, 0), (2, 50), (1, 51)]), Err(ProfileError::ForkVersionOrder));
    assert_eq!(ForkSchedule::new(&[(1, 0), (2, 50), (2, 51)]), Err(ProfileError::ForkVersionOrder));
    assert_eq!(ForkSchedule::new(&[(1, 0), (3, 50)]), Err(ProfileError::UnsupportedVersion(3)));
}

#[test] fn nine_profile_signed_executions_match_independent_full_roots_receipts_and_gas(){
 let rows:Vec<serde_json::Value>=serde_json::from_str(include_str!("data/profile-execution.json")).unwrap();assert_eq!(rows.len(),9);
 for row in rows{let chain=row["chain"].as_u64().unwrap();let profile=ProfileIdentity::validate(&f::raw(row["genesisSpec"].as_str().unwrap()),&f::raw(row["genesisHash"].as_str().unwrap()).try_into().unwrap(),chain,if chain==777001{ProfileKind::Network}else{ProfileKind::Synthetic},if chain==777001{NodeKind::Node}else{NodeKind::Simnet},&[(1,0)]).unwrap();assert!(!profile.bootable());
 let(_,db,block)=f::fixture();let before=format!("{db:?}");let bytes=f::raw(row["raw"].as_str().unwrap());let candidate=user_phase::execute_for_profile(&db,&[bytes.clone()],&block,bytes.len(),1,&profile);assert_eq!(candidate.is_ok(),true);let c=candidate.unwrap();
 assert_eq!(c.state_root(3,1).unwrap().as_slice(),f::raw(row["stateRoot"].as_str().unwrap()));assert_eq!(c.receipts(),&[f::raw(row["receipt"].as_str().unwrap())]);assert_eq!(c.commitments().gas_used,row["gasUsed"].as_u64().unwrap());
 let mut pool=Pool::new(profile);let hash=pool.submit(&bytes,&db,&block).unwrap();assert_eq!(hash.as_slice(),f::raw(row["hash"].as_str().unwrap()));assert_eq!(format!("{db:?}"),before);
 }
}

/// ForkSchedule::new as of main 9c502ff (p1_profile.rs:221-243), verbatim, as the reference verdict.
fn head_verdict(entries: &[(u16, u64)]) -> Result<(), ProfileError> {
    if entries.first() != Some(&(1, 0)) {
        return Err(ProfileError::ForkStart);
    }
    let mut previous = None;
    let mut activated_v2 = false;
    for &(version, height) in entries {
        if previous.map_or(false, |h| height <= h) {
            return Err(ProfileError::ForkHeight);
        }
        if version != 1 && version != 2 {
            return Err(ProfileError::UnsupportedVersion(version));
        }
        if activated_v2 {
            return Err(ProfileError::ForkVersionOrder);
        }
        activated_v2 = version == 2;
        previous = Some(height);
    }
    Ok(())
}

#[test]
fn generic_schedule_rule_gives_the_historical_verdict_for_every_small_schedule() {
    let cells: Vec<(u16, u64)> = (0..=3u16).flat_map(|v| (0..=3u64).map(move |h| (v, h))).collect();
    let (mut checked, mut accepted) = (0, 0);
    let mut entries = Vec::new();
    fn walk(cells: &[(u16, u64)], entries: &mut Vec<(u16, u64)>, checked: &mut u32, accepted: &mut u32) {
        let got = ForkSchedule::new(entries).map(|f| assert_eq!(f.entries(), &entries[..]));
        assert_eq!(got, head_verdict(entries), "{entries:?}");
        *checked += 1;
        *accepted += got.is_ok() as u32;
        if entries.len() < 4 {
            for &c in cells {
                entries.push(c);
                walk(cells, entries, checked, accepted);
                entries.pop();
            }
        }
    }
    walk(&cells, &mut entries, &mut checked, &mut accepted);
    assert_eq!(checked, 1 + 16 + 256 + 4096 + 65536);
    assert!(accepted > 1, "the enumeration reaches accepted schedules");
}

#[test]
fn a_higher_bound_accepts_later_versions_without_changing_the_default() {
    let f = ForkSchedule::new_bounded(&[(1, 0), (2, 10), (3, 20)], 3).unwrap();
    assert_eq!([f.active_version(9), f.active_version(10), f.active_version(19), f.active_version(25)], [1, 2, 2, 3]);
    assert_eq!(ForkSchedule::new(&[(1, 0), (2, 10), (3, 20)]), Err(ProfileError::UnsupportedVersion(3)));
    assert_eq!(MAX_FORK_VERSION, 2);
    // Skipping a version is a profile-level question left to the v3 PR (D3); the runtime shape refuses it.
    assert_eq!(ForkSchedule::new_bounded(&[(1, 0), (3, 5)], 3).unwrap().active_version(5), 3);
    assert_eq!(ForkSchedule::new_bounded(&[(1, 0), (3, 5), (2, 9)], 3), Err(ProfileError::ForkVersionOrder));
    assert_eq!(ForkSchedule::new_bounded(&[(1, 0), (3, 5), (3, 9)], 3), Err(ProfileError::ForkVersionOrder));
    assert_eq!(ForkSchedule::new_bounded(&[(1, 0), (4, 5)], 3), Err(ProfileError::UnsupportedVersion(4)));
}
