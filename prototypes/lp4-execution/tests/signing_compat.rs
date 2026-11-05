//! Public synthetic inputs only. Compare private signing with an independent
//! RFC6979 oracle, retain legacy bytes below the order, and test low-s recovery.
use k256::ecdsa::SigningKey as LegacySigningKey;
use pocol_lp4_execution::txsign::{address_of, recover, Key};
use revm::primitives::{keccak256, B256};

const ORDER: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe,
    0xba, 0xae, 0xdc, 0xe6, 0xaf, 0x48, 0xa0, 0x3b, 0xbf, 0xd2, 0x5e, 0x8c, 0xd0, 0x36, 0x41, 0x41,
];

fn small(n: u8) -> [u8; 32] {
    let mut value = [0; 32];
    value[31] = n;
    value
}

fn previous(mut value: [u8; 32]) -> [u8; 32] {
    for byte in value.iter_mut().rev() {
        let (next, borrow) = byte.overflowing_sub(1);
        *byte = next;
        if !borrow {
            break;
        }
    }
    value
}

fn synthetic(tag: &[u8], number: u64) -> [u8; 32] {
    keccak256([tag, &number.to_be_bytes()].concat()).0
}

#[test]
fn private_signer_preserves_rfc6979_addresses_and_low_s_at_edges() {
    let oracle = secp256k1::Secp256k1::signing_only();
    let mut after_order = ORDER;
    after_order[31] += 1;
    let mut secrets = vec![
        small(1),
        small(2),
        previous(ORDER),
        previous(previous(ORDER)),
    ];
    let cases = std::env::var("POCOL_SIGNING_COMPAT_KEYS")
        .map(|value| value.parse::<u64>().expect("numeric fixture-key count"))
        .unwrap_or(64);
    assert!(
        (1..=4096).contains(&cases),
        "fixture-key count must be 1..=4096"
    );
    secrets.extend((0..cases).map(|n| synthetic(b"pocol-public-signing-fixture-key", n)));
    let mut digests = vec![
        [0; 32],
        small(1),
        previous(ORDER),
        ORDER,
        after_order,
        [0xff; 32],
    ];
    digests.extend((0..8).map(|n| synthetic(b"pocol-public-signing-fixture-message", n)));

    let (mut legacy_equal, mut corrected_edges) = (0, 0);
    for (key_index, secret) in secrets.iter().enumerate() {
        let legacy = LegacySigningKey::from_slice(secret).unwrap();
        let fixed = Key::from_secret(secret).unwrap();
        let oracle_key = secp256k1::SecretKey::from_slice(secret).unwrap();
        let address = address_of(legacy.verifying_key());
        assert_eq!(fixed.address(), address, "address at key {key_index}");
        assert_eq!(fixed.secret(), *secret, "key serialization at {key_index}");
        for (digest_index, digest) in digests.iter().enumerate() {
            let (signature, recovery_id) = legacy.sign_prehash_recoverable(digest).unwrap();
            assert!(signature.normalize_s().is_none());
            let actual = fixed.sign_hash(&B256::from(*digest));
            let (oracle_id, oracle_signature) = oracle
                .sign_ecdsa_recoverable(&secp256k1::Message::from_digest(*digest), &oracle_key)
                .serialize_compact();
            assert_eq!(
                &actual[..64],
                oracle_signature,
                "RFC6979 oracle at {key_index}/{digest_index}"
            );
            assert_eq!(actual[64], oracle_id.to_i32() as u8);
            if digest < &ORDER {
                assert_eq!(
                    &actual[..64],
                    &signature.to_bytes()[..],
                    "legacy signature at {key_index}/{digest_index}"
                );
                assert_eq!(actual[64], recovery_id.to_byte());
                legacy_equal += 1;
            } else {
                // The old backend failed to apply RFC6979 bits2octets at/above n.
                // Preserve validity of old signatures, not that nonstandard nonce.
                let mut old = [0u8; 65];
                old[..64].copy_from_slice(&signature.to_bytes());
                old[64] = recovery_id.to_byte();
                assert_eq!(recover(&B256::from(*digest), &old), Some(address));
                if actual != old {
                    corrected_edges += 1;
                }
            }
            assert_eq!(recover(&B256::from(*digest), &actual), Some(address));
            assert_eq!(fixed.sign_hash(&B256::from(*digest)), actual, "determinism");
        }
    }
    assert!(legacy_equal > 0 && corrected_edges > 0);
    println!("RFC6979 oracle pairs: {}; legacy-identical pairs: {legacy_equal}; corrected digest edges: {corrected_edges}", secrets.len() * digests.len());
}

#[test]
fn private_signer_refuses_invalid_secret_scalars() {
    let mut after_order = ORDER;
    after_order[31] += 1;
    for secret in [[0; 32], ORDER, after_order, [0xff; 32]] {
        assert!(Key::from_secret(&secret).is_err());
        assert!(LegacySigningKey::from_slice(&secret).is_err());
    }
}

#[test]
fn recovery_policy_still_rejects_high_s_bad_id_and_bad_length() {
    let key = Key::from_secret(&small(1)).unwrap();
    let digest = B256::from(synthetic(b"pocol-public-policy-fixture", 0));
    let signature = key.sign_hash(&digest);
    assert_eq!(recover(&digest, &signature), Some(key.address()));
    let mut high = signature;
    let mut borrow = 0i16;
    for i in (0..32).rev() {
        let difference = ORDER[i] as i16 - signature[32 + i] as i16 - borrow;
        high[32 + i] = difference.rem_euclid(256) as u8;
        borrow = i16::from(difference < 0);
    }
    high[64] ^= 1;
    assert_eq!(recover(&digest, &high), None);
    let mut bad_id = signature;
    bad_id[64] = 27;
    assert_eq!(recover(&digest, &bad_id), None);
    assert_eq!(recover(&digest, &signature[..64]), None);
    assert_eq!(recover(&digest, &[0; 65]), None);
}
