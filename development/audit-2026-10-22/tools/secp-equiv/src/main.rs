//! Equivalence of libsecp256k1 0.5.0 and k256 0.13.4 for PoCol's recover_address (lp1-node
//! hashes.rs): same check_sig_shape filter, then recovery; and RFC 6979 signing determinism.
use sha3::{Digest, Keccak256};
const N: [u8; 32] = [0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xfe,0xba,0xae,0xdc,0xe6,0xaf,0x48,0xa0,0x3b,0xbf,0xd2,0x5e,0x8c,0xd0,0x36,0x41,0x41];
const HALF_N: [u8; 32] = [0x7f,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0x5d,0x57,0x6e,0x73,0x57,0xa4,0x50,0x1d,0xdf,0xe9,0x2f,0x46,0x68,0x1b,0x20,0xa0];
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum F { Length, RRange, SRange, HighS, BadV, NotRecoverable }
fn keccak(b: &[u8]) -> [u8; 32] { let d = Keccak256::digest(b); let mut o = [0u8; 32]; o.copy_from_slice(&d); o }
fn shape(sig: &[u8]) -> Result<([u8; 64], u8), F> {
    if sig.len() != 65 { return Err(F::Length); }
    let mut rs = [0u8; 64]; rs.copy_from_slice(&sig[..64]);
    let (r, s) = (&sig[..32], &sig[32..64]); let z = [0u8; 32];
    if r == &z[..] || r >= &N[..] { return Err(F::RRange); }
    if s == &z[..] { return Err(F::SRange); }
    if s > &HALF_N[..] { return Err(F::HighS); }
    if sig[64] > 1 { return Err(F::BadV); }
    Ok((rs, sig[64]))
}
fn addr(pub65: &[u8]) -> [u8; 20] { let k = keccak(&pub65[1..]); let mut a = [0u8; 20]; a.copy_from_slice(&k[12..]); a }
fn old(msg: &[u8; 32], sig: &[u8]) -> Result<[u8; 20], F> {
    let (rs, v) = shape(sig)?;
    let m = libsecp256k1::Message::parse(msg);
    let s = libsecp256k1::Signature::parse_standard_slice(&rs).map_err(|_| F::RRange)?;
    let id = libsecp256k1::RecoveryId::parse(v).map_err(|_| F::BadV)?;
    let pk = libsecp256k1::recover(&m, &s, &id).map_err(|_| F::NotRecoverable)?;
    Ok(addr(&pk.serialize()))
}
fn new(msg: &[u8; 32], sig: &[u8]) -> Result<[u8; 20], F> {
    let (rs, v) = shape(sig)?;
    let s = k256::ecdsa::Signature::from_slice(&rs).map_err(|_| F::RRange)?;
    let id = k256::ecdsa::RecoveryId::from_byte(v).ok_or(F::BadV)?;
    let vk = k256::ecdsa::VerifyingKey::recover_from_prehash(msg, &s, id).map_err(|_| F::NotRecoverable)?;
    Ok(addr(vk.to_encoded_point(false).as_bytes()))
}
fn be_add(a: &[u8; 32], k: u64) -> [u8; 32] { let mut o = *a; let mut c = k as u128; for i in (0..32).rev() { let t = o[i] as u128 + (c & 0xff); o[i] = t as u8; c = (c >> 8) + (t >> 8); } o }
fn be_sub(a: &[u8; 32], k: u64) -> [u8; 32] { let mut o = *a; let mut b = k as i128; for i in (0..32).rev() { let t = o[i] as i128 - (b & 0xff); o[i] = t.rem_euclid(256) as u8; b = (b >> 8) + if t < 0 { 1 } else { 0 }; } o }
fn main() {
    let n_valid: u64 = std::env::args().nth(1).and_then(|x| x.parse().ok()).unwrap_or(20000);
    let mut stats = std::collections::BTreeMap::<String, u64>::new(); let mut bad = Vec::new();
    let mut bump = |k: &str| *stats.entry(k.to_string()).or_default() += 1;
    // A: honest signatures, compared with the signer's address; and RFC 6979 signing equality.
    for i in 0..n_valid {
        let sk = keccak(&[b"sk".as_ref(), &i.to_be_bytes()].concat()); let msg = keccak(&[b"msg".as_ref(), &i.to_be_bytes()].concat());
        let Ok(k) = k256::ecdsa::SigningKey::from_bytes((&sk).into()) else { bump("A skipped key"); continue };
        let (sig, rec) = k.sign_prehash_recoverable(&msg).unwrap();
        let (sig, rec) = match sig.normalize_s() { Some(n) => (n, k256::ecdsa::RecoveryId::new(!rec.is_y_odd(), rec.is_x_reduced())), None => (sig, rec) };
        let mut s65 = [0u8; 65]; s65[..64].copy_from_slice(&sig.to_bytes()); s65[64] = rec.to_byte();
        let want = addr(k.verifying_key().to_encoded_point(false).as_bytes());
        let (o, n) = (old(&msg, &s65), new(&msg, &s65));
        if o != Ok(want) || n != Ok(want) { bad.push(format!("A{i}: old={o:?} new={n:?}")); }
        // libsecp256k1 signs with RFC 6979 and low-s: identical bytes and recovery id expected.
        let lsk = libsecp256k1::SecretKey::parse(&sk).unwrap();
        let (ls, lr) = libsecp256k1::sign(&libsecp256k1::Message::parse(&msg), &lsk);
        if ls.serialize() != s65[..64] || lr.serialize() != s65[64] { bad.push(format!("A{i}: RFC6979 signature differs")); }
        bump("A honest signatures");
    }
    // B: arbitrary bytes through the shape filter: identical verdicts and addresses.
    for i in 0..n_valid * 2 {
        let r = keccak(&[b"r".as_ref(), &i.to_be_bytes()].concat()); let mut s = keccak(&[b"s".as_ref(), &i.to_be_bytes()].concat());
        if i % 2 == 0 { s[0] &= 0x7f; }
        let msg = keccak(&[b"m".as_ref(), &i.to_be_bytes()].concat());
        let mut s65 = [0u8; 65]; s65[..32].copy_from_slice(&r); s65[32..64].copy_from_slice(&s); s65[64] = (i % 3) as u8;
        let (o, n) = (old(&msg, &s65), new(&msg, &s65));
        if o != n { bad.push(format!("B{i}: old={o:?} new={n:?}")); }
        bump(&format!("B {}", match o { Ok(_) => "Ok".to_string(), Err(e) => format!("{e:?}") }));
    }
    // C: boundaries of r, s, v and the message (reduced mod n by both).
    let one = be_add(&[0u8; 32], 1); let p = { let mut x = [0xffu8; 32]; x[27] = 0xfe; x[28] = 0xff; x[29] = 0xfc; x[30] = 0x2f; x[31] = 0x00; be_add(&x, 0x2f) };
    let rs = [one, be_add(&one, 1), be_add(&one, 4), be_sub(&N, 1), N, be_add(&N, 1), be_sub(&p, 1), [0xffu8; 32], [0u8; 32], keccak(b"x-coordinate")];
    let ss = [one, HALF_N, be_add(&HALF_N, 1), be_sub(&N, 1), [0u8; 32], keccak(b"s")];
    let ms = [[0u8; 32], one, be_sub(&N, 1), N, be_add(&N, 1), [0xffu8; 32], keccak(b"msg")];
    for r in &rs { for s in &ss { for m in &ms { for v in 0..3u8 {
        let mut s65 = [0u8; 65]; s65[..32].copy_from_slice(r); s65[32..64].copy_from_slice(s); s65[64] = v;
        let (o, n) = (old(m, &s65), new(m, &s65));
        if o != n { bad.push(format!("C r={r:02x?} s={s:02x?} m={m:02x?} v={v}: old={o:?} new={n:?}")); }
        bump(&format!("C {}", match o { Ok(_) => "Ok".to_string(), Err(e) => format!("{e:?}") }));
    }}}}
    for l in [0usize, 64, 66] { let s = vec![1u8; l]; if old(&[0u8; 32], &s) != new(&[0u8; 32], &s) { bad.push(format!("len {l}")); } bump("C lengths"); }
    // D: honest signatures with the message replaced by m + n (same residue): identical results.
    for i in 0..200u64 {
        let sk = keccak(&[b"dk".as_ref(), &i.to_be_bytes()].concat()); let k = k256::ecdsa::SigningKey::from_bytes((&sk).into()).unwrap();
        let msg = be_sub(&N, i + 1); let (sig, rec) = k.sign_prehash_recoverable(&msg).unwrap();
        let (sig, rec) = match sig.normalize_s() { Some(n) => (n, k256::ecdsa::RecoveryId::new(!rec.is_y_odd(), rec.is_x_reduced())), None => (sig, rec) };
        let mut s65 = [0u8; 65]; s65[..64].copy_from_slice(&sig.to_bytes()); s65[64] = rec.to_byte();
        let want = addr(k.verifying_key().to_encoded_point(false).as_bytes());
        if old(&msg, &s65) != Ok(want) || new(&msg, &s65) != Ok(want) { bad.push(format!("D{i}")); }
        bump("D messages near n");
    }
    println!("{}", serde_like(&stats, &bad));
    std::process::exit(if bad.is_empty() { 0 } else { 1 });
}
fn serde_like(stats: &std::collections::BTreeMap<String, u64>, bad: &[String]) -> String {
    let s: Vec<String> = stats.iter().map(|(k, v)| format!("\"{k}\":{v}")).collect();
    format!("{{\"cases\":{{{}}},\"mismatches\":{},\"first\":{:?}}}", s.join(","), bad.len(), bad.iter().take(5).collect::<Vec<_>>())
}
