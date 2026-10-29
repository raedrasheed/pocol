//! T0 semantic core mapping: preserved M1 lists V labels but supplies no literal V bytes.
mod common;
use lp1_node::{envelope as e, fixed::U256, fixtures, hashes, header, rlp};
fn headers() -> Vec<header::Header> {
    fixtures::load_chain_bytes(&common::json("chain.json")).unwrap().iter().map(|b| header::decode(b).unwrap()).collect()
}
fn wrap(v: &[Vec<u8>]) -> Vec<u8> {
    common::outer(v)
}
fn malformed_ut(h: &header::Header, index: usize, value: Option<Vec<u8>>) -> Vec<u8> {
    let b = h.encode();
    let outer = rlp::decode(&b).unwrap();
    let a = outer.list().unwrap();
    let st = a[0].list().unwrap();
    let mut ut: Vec<Vec<u8>> = st[0].list().unwrap().iter().map(|v| v.raw.to_vec()).collect();
    if let Some(v) = value {
        ut[index] = v;
    } else {
        ut.pop();
    }
    wrap(&[wrap(&[wrap(&ut), st[1].raw.to_vec()]), a[1].raw.to_vec(), a[2].raw.to_vec(), a[3].raw.to_vec()])
}
#[test]
fn t0_l0_shapes_and_canonical_rejections() {
    let hs = headers();
    for h in &hs {
        assert!(e::decode(&h.encode()).is_ok());
    }
    let h = hs.iter().find(|h| h.is_signed()).unwrap();
    let mut b = h.encode();
    b.push(0);
    assert!(e::decode(&b).is_err());
    // V7 overlong/minimal integer, V12 zero target, V15 missing UT field.
    assert!(e::decode(&malformed_ut(h, 4, Some(vec![0x82, 0, 1]))).is_err());
    assert!(e::decode(&malformed_ut(h, 8, Some(vec![0x80]))).is_err());
    assert!(e::decode(&malformed_ut(h, 0, None)).is_err());
    for template in [false, true] {
        for high in [false, true] {
            let mut v = h.clone();
            let sig = if template { v.sig.as_mut_slice() } else { v.winner_sig.as_mut_slice() };
            if high {
                sig[32..64].copy_from_slice(&hashes::N_ORDER);
            } else {
                sig[64] = 27;
            }
            assert!(e::decode(&v.encode()).is_err());
        }
    }
    let mut v = h.clone();
    v.proposer = vec![1; 19];
    assert!(e::decode(&v.encode()).is_err());
    for shares in [vec![2, 1], vec![h.nonce], vec![1, 1]] {
        let mut v = h.clone();
        v.shares = shares;
        assert!(e::decode(&v.encode()).is_err());
    }
    // V11 exact 8-byte nonce, independent of integer minimality.
    let b = h.encode();
    let a = rlp::decode(&b).unwrap();
    let a = a.list().unwrap();
    let mut n = Vec::new();
    rlp::encode_bytes(&[1; 7], &mut n);
    assert!(e::decode(&wrap(&[a[0].raw.to_vec(), n, a[2].raw.to_vec(), a[3].raw.to_vec()])).is_err());
}
#[test]
fn t0_l1_net_and_signed_diagnostics() {
    let n = common::node();
    let hs = headers();
    let h = hs.iter().find(|h| h.is_signed()).unwrap();
    let good = e::diagnose(&h.encode(), &n.cfg);
    assert_eq!(good.first, None);
    assert!(!good.not_evaluable.is_empty());
    let mut cfg = n.cfg.clone();
    cfg.chain_id ^= 1;
    assert_eq!(e::diagnose(&h.encode(), &cfg).violated, vec!["netChain"]);
    let mut bad = h.clone();
    bad.a = 1;
    assert_eq!(e::diagnose(&bad.encode(), &cfg).violated, vec!["netChain", "3"]);
    cfg = n.cfg.clone();
    cfg.genesis_hash[0] ^= 1;
    assert_eq!(e::diagnose(&h.encode(), &cfg).first, Some("netGenesis"));
    cfg = n.cfg.clone();
    cfg.fork_schedule = vec![(2, 0)];
    assert_eq!(e::diagnose(&h.encode(), &cfg).first, Some("netVersion"));
    cfg.fork_schedule = vec![(1, h.h + 1)];
    assert_eq!(e::diagnose(&h.encode(), &cfg).first, Some("netVersion"));
    for variant in 0..4 {
        let mut v = h.clone();
        match variant {
            0 => v.a = 1,
            1 => v.sig.clear(),
            2 => v.proposer.clear(),
            _ => v.proposer[0] ^= 1,
        };
        assert_eq!(e::diagnose(&v.encode(), &n.cfg).first, Some("3"));
    }
}
#[test]
fn t0_l2_pow_and_max_shape_not_full_validity() {
    let hs = headers();
    for h in &hs {
        assert!(h.nonce < (1 << 28));
        let tid = hashes::sha256(&h.ut_raw);
        let p = hashes::sha256(&hashes::tid_nonce_preimage(&tid, h.nonce));
        assert!(U256::from_be_slice(&p).unwrap() <= h.target);
    }
    let mut h = hs.iter().find(|h| h.is_signed()).unwrap().clone();
    h.chain_id = u64::MAX;
    h.h = u64::MAX;
    h.a = u32::MAX;
    h.protocol_version = 1;
    h.ts = u64::MAX;
    h.target = U256::MAX;
    h.gas_limit = u64::MAX;
    h.gas_used = u64::MAX;
    h.base_fee = U256::MAX;
    h.nonce = 0;
    h.shares = (1..=256).collect();
    let b = h.encode();
    assert_eq!(b.len(), 3067);
    let d = e::decode(&b).unwrap();
    let tid = hashes::sha256(&d.ut_raw);
    assert!(U256::from_be_slice(&hashes::sha256(&hashes::tid_nonce_preimage(&tid, 0))).unwrap() <= d.target);
    // Authentication, parent linking, state and full H validation are deliberately unevaluated.
}
