//! Fresh S8 corpus for the additive M3 entry point, distinct from preserved RP checks.
mod common;
use lp1_node::{envelope, fixtures};
#[test]
fn seeded_strict_envelope_no_panic_and_canonical_acceptance() {
    let raw = fixtures::load_chain_bytes(&common::json("chain.json")).unwrap();
    let mut r = common::Rng(0x5338_c0de_2026_1017);
    let mut accepted = 0;
    for _ in 0..12000 {
        let mut b = raw[r.below(raw.len() as u64) as usize].clone();
        let j = r.below(b.len() as u64 + 1) as usize;
        match r.below(4) {
            0 => {
                if j < b.len() {
                    b[j] ^= 1 << r.below(8);
                }
            }
            1 => b.truncate(j),
            2 => b.insert(j, r.next() as u8),
            _ => {
                if j < b.len() {
                    b[j] = r.next() as u8;
                }
            }
        }
        if let Ok(h) = envelope::decode(&b) {
            accepted += 1;
            assert_eq!(h.encode(), b);
            assert!(h.shares.windows(2).all(|a| a[0] < a[1]));
            assert!(!h.shares.contains(&h.nonce));
        }
    }
    assert!(accepted > 0);
    println!("{{\"corpus\":\"S8 envelope\",\"seed\":\"0x5338c0de20261017\",\"cases\":12000,\"accepted\":{}}}", accepted);
}
#[test]
fn in_range_high_s_is_still_noncanonical_in_both_signatures() {
    use lp1_node::{hashes, header};
    let raw = fixtures::load_chain_bytes(&common::json("chain.json")).unwrap();
    let h = raw.iter().map(|b| header::decode(b).unwrap()).find(|h| h.is_signed()).unwrap();
    let mut high_s = hashes::HALF_N;
    high_s[31] += 1; // HALF_N ends in a0; this is in range and strictly above n/2.
    for template in [false, true] {
        let mut v = h.clone();
        let sig = if template { v.sig.as_mut_slice() } else { v.winner_sig.as_mut_slice() };
        sig[32..64].copy_from_slice(&high_s);
        assert_eq!(hashes::check_sig_shape(sig), Err(hashes::SigFault::HighS));
        assert!(envelope::decode(&v.encode()).is_err());
    }
}
