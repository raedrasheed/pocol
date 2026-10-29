//! S2: typed conversion, schedule boundaries, identity/gate bypass negatives.
use lp1_node::fixed::U256;
use lp1_node::genesis_spec::{self as gs, GenesisSpec, Upper, CP_SCHEMA};
use lp1_node::profile::{ConsensusParams, ForkSchedule, NodeKind, ProfileError, ProfileIdentity, ProfileKind};
use lp1_node::{hashes::H256, hex};

fn spec() -> GenesisSpec {
    let p: serde_json::Value = serde_json::from_str(include_str!("../fixtures/profile.json")).unwrap();
    gs::decode(&hex::decode(p["genesis_pre_hex"].as_str().unwrap()).unwrap()).unwrap()
}
fn validate(s: &GenesisSpec, kind: ProfileKind, node: NodeKind, forks: &[(u16, u64)]) -> Result<ProfileIdentity, ProfileError> {
    let b = gs::encode(s);
    ProfileIdentity::validate(&b, &gs::genesis_hash(&b), s.chain_id, kind, node, forks)
}

#[test]
fn typed_cp_preserves_all_values_and_exact_encoding() {
    let mut s = spec();
    for (i, f) in CP_SCHEMA.iter().enumerate() {
        s.cp[i] = match f.hi {
            Upper::Gamma => U256::ZERO,
            Upper::Value(n) => U256::from_u64(n),
            Upper::None => {
                if f.width == 256 {
                    U256::MAX
                } else {
                    U256::from_u64(u64::MAX >> (64 - f.width))
                }
            }
        };
    }
    let cp = ConsensusParams::from_values(&s.cp).unwrap();
    assert_eq!(cp.to_values(), s.cp);
    assert_eq!(cp.target_g, U256::MAX);
    assert_eq!(cp.baseFee0, U256::MAX);
    assert_eq!(cp.g_ts, u64::MAX);
    assert_eq!(cp.M_max, u16::MAX);
    assert_eq!(cp.RET, u32::MAX);
    let b = gs::encode(&s);
    assert_eq!(gs::decode(&b).unwrap(), s);
    let p = validate(&s, ProfileKind::Synthetic, NodeKind::Simnet, &[(1, 0)]).unwrap();
    assert_eq!(p.cp().to_values(), p.spec().cp);
    assert_eq!(p.genesis_hash(), gs::genesis_hash(&b));
    assert_eq!(p.kind(), ProfileKind::Synthetic);
    assert_eq!(p.node_kind(), NodeKind::Simnet);
    assert!(!p.bootable());
}

#[test]
fn every_width_minimum_and_upper_bound_is_checked_before_narrowing() {
    let s = spec();
    for (i, field) in CP_SCHEMA.iter().enumerate() {
        let mut v = s.cp;
        if field.width < 256 {
            v[i] = U256::pow2(field.width).unwrap();
            assert_eq!(ConsensusParams::from_values(&v).unwrap_err().detail, field.name);
        }
        if field.lo > 0 {
            v = s.cp;
            v[i] = U256::from_u64(field.lo - 1);
            assert_eq!(ConsensusParams::from_values(&v).unwrap_err().detail, field.name);
        }
        if let Upper::Value(n) = field.hi {
            v = s.cp;
            v[i] = U256::from_u64(n + 1);
            assert_eq!(ConsensusParams::from_values(&v).unwrap_err().detail, field.name);
        }
    }
    let mut v = s.cp;
    v[13] = U256::from_u64(10000);
    v[14] = U256::ONE;
    assert_eq!(ConsensusParams::from_values(&v).unwrap_err().detail, "gamma_bp");
    v[14] = U256::ZERO;
    assert!(ConsensusParams::from_values(&v).is_ok());
}

#[test]
fn schedule_requires_origin_strict_heights_and_implemented_versions() {
    assert_eq!(ForkSchedule::new(&[]), Err(ProfileError::ForkStart));
    assert_eq!(ForkSchedule::new(&[(1, 1)]), Err(ProfileError::ForkStart));
    assert_eq!(ForkSchedule::new(&[(2, 0)]), Err(ProfileError::ForkStart));
    for entries in [&[(1, 0), (1, 0)][..], &[(1, 0), (1, 10), (1, 9)][..]] {
        assert_eq!(ForkSchedule::new(entries), Err(ProfileError::ForkHeight));
    }
    for v in [0, 2, 3, u16::MAX] {
        assert_eq!(ForkSchedule::new(&[(1, 0), (v, 50)]), Err(ProfileError::UnsupportedVersion(v)));
    }
    let f = ForkSchedule::new(&[(1, 0), (1, 50), (1, u64::MAX)]).unwrap();
    assert_eq!(f.entries(), &[(1, 0), (1, 50), (1, u64::MAX)]);
    for h in [0, 1, 49, 50, 51, u64::MAX - 1, u64::MAX] {
        assert_eq!(f.active_version(h), 1);
    }
}

#[test]
fn kind_matrix_and_network_chain_id_separation() {
    for chain in [1, 777001, 777002, 777902, u64::MAX] {
        let mut s = spec();
        s.chain_id = chain;
        for kind in [ProfileKind::Network, ProfileKind::Simnet, ProfileKind::Synthetic] {
            for node in [NodeKind::Node, NodeKind::Simnet] {
                let result = validate(&s, kind, node, &[(1, 0)]);
                let allowed = match (kind, node, chain) {
                    (ProfileKind::Network, _, 777001) => true,
                    (ProfileKind::Simnet, NodeKind::Simnet, id) | (ProfileKind::Synthetic, NodeKind::Simnet, id) => id != 777001,
                    _ => false,
                };
                assert_eq!(result.is_ok(), allowed, "{kind:?} {node:?} {chain}");
                if let Ok(p) = result {
                    assert!(!p.bootable());
                } else {
                    assert_eq!(result.unwrap_err(), ProfileError::R22);
                }
            }
        }
    }
}

#[test]
fn hash_and_chain_binding_cannot_be_replaced_by_matching_fork_schedule() {
    let s = spec();
    let b = gs::encode(&s);
    let h = gs::genesis_hash(&b);
    let call = |bytes: &[u8], hash: &H256, chain| ProfileIdentity::validate(bytes, hash, chain, ProfileKind::Synthetic, NodeKind::Simnet, &[(1, 0)]);
    let mut wrong = h;
    wrong[0] ^= 1;
    assert!(matches!(call(&b, &wrong, s.chain_id), Err(ProfileError::Genesis(e)) if e.code == "genesisHash"));
    assert_eq!(call(&b, &h, s.chain_id + 1), Err(ProfileError::ChainId));
    let mut changed = s.clone();
    changed.alloc_root[0] ^= 1;
    assert!(matches!(call(&gs::encode(&changed), &h, s.chain_id), Err(ProfileError::Genesis(e)) if e.code == "genesisHash"));
    changed = s.clone();
    changed.sys_code_hash[0] ^= 1;
    assert!(matches!(call(&gs::encode(&changed), &h, s.chain_id), Err(ProfileError::Genesis(e)) if e.code == "genesisHash"));
    assert!(!call(&b, &h, s.chain_id).unwrap().bootable());
    assert!(matches!(call(&[], &h, s.chain_id), Err(ProfileError::Genesis(e)) if e.code == "L0"));
    assert_eq!(call(&vec![0; gs::MAX_SPEC_BYTES + 1], &h, s.chain_id), Err(ProfileError::InputTooLarge));
}

#[test]
fn testfork_v2_never_becomes_bootable_even_on_simnet() {
    let s = spec();
    assert_eq!(validate(&s, ProfileKind::Simnet, NodeKind::Simnet, &[(1, 0), (2, 50)]), Err(ProfileError::UnsupportedVersion(2)));
    assert_eq!(validate(&s, ProfileKind::Simnet, NodeKind::Node, &[(1, 0), (2, 50)]), Err(ProfileError::R22));
}
