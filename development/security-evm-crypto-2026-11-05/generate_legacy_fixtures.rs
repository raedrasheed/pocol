//! One-off historical fixture generator. Build in a scratch Cargo package with
//! k256 =0.13.4, sha3 =0.9.1 and serde_json =1.0.145, not in product lockfiles.
//! Every private scalar below is a public synthetic test value.
use k256::ecdsa::SigningKey;
use sha3::{Digest, Keccak256};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn previous(mut n: [u8; 32]) -> [u8; 32] {
    for byte in n.iter_mut().rev() {
        let (next, borrow) = byte.overflowing_sub(1);
        *byte = next;
        if !borrow { break; }
    }
    n
}
fn main() {
    let order: [u8; 32] = [
        0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xfe,
        0xba,0xae,0xdc,0xe6,0xaf,0x48,0xa0,0x3b,0xbf,0xd2,0x5e,0x8c,0xd0,0x36,0x41,0x41,
    ];
    let mut one = [0; 32]; one[31] = 1;
    let mut two = one; two[31] = 2;
    let mut after = order; after[31] += 1;
    let mut digests = vec![[0;32],one,previous(order),order,after,[0xff;32]];
    for n in 0..8u64 {
        let bytes = [b"pocol-public-signing-fixture-message".as_slice(), &n.to_be_bytes()].concat();
        digests.push(Keccak256::digest(&bytes).into());
    }
    let mut rows = Vec::new();
    for secret in [one,two,previous(order),previous(previous(order))] {
        let key = SigningKey::from_slice(&secret).unwrap();
        let point = key.verifying_key().to_encoded_point(false);
        let address = Keccak256::digest(&point.as_bytes()[1..]);
        for digest in &digests {
            let (signature, recovery_id) = key.sign_prehash_recoverable(digest).unwrap();
            assert!(signature.normalize_s().is_none());
            let mut bytes = signature.to_bytes().to_vec(); bytes.push(recovery_id.to_byte());
            rows.push(serde_json::json!({"public_synthetic_secret":hex(&secret),"digest":hex(digest),
                "legacy_signature":hex(&bytes),"address":hex(&address[12..]),
                "new_signature_expected_identical":digest < &order}));
        }
    }
    println!("{}",serde_json::to_string_pretty(&serde_json::json!({
        "public_synthetic_inputs_only":true,"generator_k256":"0.13.4",
        "scope":"Historical signature bytes including the old high-digest RFC6979 edge behavior",
        "vectors":rows})).unwrap());
}
