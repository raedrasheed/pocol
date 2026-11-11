//! Portable, passphrase-encrypted backup of one wallet key (CRY-02), shared by the Android bridge and the
//! `pocol` command that the Windows wallet calls. Node-local: no consensus rule reads it.
//!
//! File format, version 1, exactly [`LEN`] = 98 bytes, integers big-endian:
//!
//! | offset | bytes | field                                                      |
//! |--------|-------|------------------------------------------------------------|
//! | 0      | 16    | magic `POCOL-KEY-BACKUP` (ASCII)                           |
//! | 16     | 1     | format version, 1                                          |
//! | 17     | 1     | KDF id, 1 = PBKDF2-HMAC-SHA256                             |
//! | 18     | 4     | PBKDF2 iterations, [`MIN_ITERATIONS`]..=[`MAX_ITERATIONS`] |
//! | 22     | 16    | random salt                                                |
//! | 38     | 12    | random AES-GCM nonce                                       |
//! | 50     | 32    | AES-256-GCM ciphertext of the 32-byte secp256k1 secret      |
//! | 82     | 16    | AES-GCM tag; the additional data is bytes 0..50            |
//!
//! The AES key is PBKDF2-HMAC-SHA256(passphrase as UTF-8 bytes, salt, iterations, 32 bytes). The header is
//! the GCM additional data, so a changed version, KDF id, iteration count, salt or nonce fails authentication
//! like a wrong passphrase. Any standard library can open the file (Python hashlib + AESGCM, Java
//! PBKDF2WithHmacSHA256 + AES/GCM/NoPadding, .NET Rfc2898DeriveBytes + AesGcm, WebCrypto).
//!
//! The crate pins its dependencies through the consensus boundary (`Cargo.toml` is a `+` file), so AES-GCM,
//! GHASH and PBKDF2 are implemented here on the existing `sha2` dependency and checked against FIPS-197,
//! the GCM specification's test cases, RFC 7914's PBKDF2 vector and a vector made with OpenSSL. They avoid
//! secret-indexed tables and secret-dependent branches: the S-box is computed (GF(2^8) inversion and the
//! affine map) and GHASH multiplies with masks.
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

pub const MAGIC: &[u8; 16] = b"POCOL-KEY-BACKUP";
pub const VERSION: u8 = 1;
pub const KDF_PBKDF2_SHA256: u8 = 1;
/// OWASP's 2023 figure for PBKDF2-HMAC-SHA256, the same count the extension's wallet files use.
pub const DEFAULT_ITERATIONS: u32 = 600_000;
/// Files outside this range are refused before any key derivation: a low count would be a weak file, and a
/// huge one would make opening it a denial of service.
pub const MIN_ITERATIONS: u32 = 600_000;
pub const MAX_ITERATIONS: u32 = 10_000_000;
const HEADER: usize = 50;
pub const LEN: usize = HEADER + 32 + 16;
/// Export policy: at least 12 characters. Opening accepts any non-empty passphrase up to the byte bound.
pub const MIN_PASSPHRASE_CHARS: usize = 12;
pub const MAX_PASSPHRASE_BYTES: usize = 1024;
pub const EXTENSION: &str = "pocol-backup";

/// The passphrase a new backup may use: UTF-8, at least [`MIN_PASSPHRASE_CHARS`] characters, at most
/// [`MAX_PASSPHRASE_BYTES`] bytes, not only whitespace.
pub fn check_new_passphrase(passphrase: &[u8]) -> Result<(), String> {
    let text = std::str::from_utf8(passphrase).map_err(|_| "Passphrase must be valid text")?;
    if passphrase.len() > MAX_PASSPHRASE_BYTES {
        return Err(format!("Passphrase exceeds {MAX_PASSPHRASE_BYTES} bytes"));
    }
    if text.chars().count() < MIN_PASSPHRASE_CHARS || text.trim().is_empty() {
        return Err(format!("Use a passphrase of at least {MIN_PASSPHRASE_CHARS} characters"));
    }
    Ok(())
}

/// A new backup of `secret` with a fresh random salt and nonce.
pub fn seal(secret: &[u8; 32], passphrase: &[u8]) -> Result<Vec<u8>, String> {
    check_new_passphrase(passphrase)?;
    let mut random = Zeroizing::new([0u8; 28]);
    getrandom::getrandom(random.as_mut()).map_err(|_| "OS randomness unavailable")?;
    let salt: [u8; 16] = random[..16].try_into().unwrap();
    let nonce: [u8; 12] = random[16..].try_into().unwrap();
    seal_with(secret, passphrase, DEFAULT_ITERATIONS, &salt, &nonce)
}

fn seal_with(secret: &[u8; 32], passphrase: &[u8], iterations: u32, salt: &[u8; 16], nonce: &[u8; 12]) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(LEN);
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.push(KDF_PBKDF2_SHA256);
    out.extend_from_slice(&iterations.to_be_bytes());
    out.extend_from_slice(salt);
    out.extend_from_slice(nonce);
    let key = derive(passphrase, salt, iterations);
    let mut body = Zeroizing::new(*secret);
    let tag = gcm_seal(&key, nonce, &out[..HEADER], body.as_mut());
    out.extend_from_slice(body.as_ref());
    out.extend_from_slice(&tag);
    debug_assert_eq!(out.len(), LEN);
    Ok(out)
}

/// The secret in `file`. The header is checked (magic, version, KDF, bounded iterations) before the KDF
/// runs; a wrong passphrase and a changed byte give the same error.
pub fn open(file: &[u8], passphrase: &[u8]) -> Result<Zeroizing<[u8; 32]>, String> {
    if file.len() < 17 || &file[..16] != MAGIC {
        return Err("Not a PoCol key backup".into());
    }
    if file[16] != VERSION {
        return Err(format!("Unsupported key backup version {}; update PoCol to open it", file[16]));
    }
    if file.len() != LEN {
        return Err("Damaged key backup: wrong length".into());
    }
    if file[17] != KDF_PBKDF2_SHA256 {
        return Err("Unsupported key backup KDF".into());
    }
    let iterations = u32::from_be_bytes(file[18..22].try_into().unwrap());
    if !(MIN_ITERATIONS..=MAX_ITERATIONS).contains(&iterations) {
        return Err(format!("Key backup KDF iterations outside {MIN_ITERATIONS}..={MAX_ITERATIONS}"));
    }
    if passphrase.is_empty() || passphrase.len() > MAX_PASSPHRASE_BYTES {
        return Err("Enter the backup passphrase".into());
    }
    let salt: [u8; 16] = file[22..38].try_into().unwrap();
    let nonce: [u8; 12] = file[38..50].try_into().unwrap();
    let key = derive(passphrase, &salt, iterations);
    let mut secret = Zeroizing::new([0u8; 32]);
    secret.copy_from_slice(&file[HEADER..HEADER + 32]);
    let tag: [u8; 16] = file[HEADER + 32..].try_into().unwrap();
    if !gcm_open(&key, &nonce, &file[..HEADER], secret.as_mut(), &tag) {
        secret.zeroize();
        return Err("Wrong passphrase or damaged key backup".into());
    }
    Ok(secret)
}

fn derive(passphrase: &[u8], salt: &[u8; 16], iterations: u32) -> Zeroizing<[u8; 32]> {
    let mut key = Zeroizing::new([0u8; 32]);
    pbkdf2_sha256(passphrase, salt, iterations, key.as_mut());
    key
}

// ---- HMAC-SHA256 and PBKDF2 (RFC 2104, RFC 8018) ----

/// The inner and outer hash states after the padded key block, cloned for every message.
struct Hmac {
    inner: Sha256,
    outer: Sha256,
}
impl Hmac {
    fn new(key: &[u8]) -> Self {
        let mut block = Zeroizing::new([0u8; 64]);
        if key.len() > 64 {
            block[..32].copy_from_slice(&Sha256::digest(key));
        } else {
            block[..key.len()].copy_from_slice(key);
        }
        let mut pad = Zeroizing::new([0u8; 64]);
        let (mut inner, mut outer) = (Sha256::new(), Sha256::new());
        for (p, b) in pad.iter_mut().zip(block.iter()) {
            *p = b ^ 0x36;
        }
        inner.update(pad.as_ref());
        for (p, b) in pad.iter_mut().zip(block.iter()) {
            *p = b ^ 0x5c;
        }
        outer.update(pad.as_ref());
        Self { inner, outer }
    }
    fn mac(&self, parts: &[&[u8]]) -> Zeroizing<[u8; 32]> {
        let mut inner = self.inner.clone();
        for p in parts {
            inner.update(p);
        }
        let mut outer = self.outer.clone();
        outer.update(inner.finalize());
        let mut out = Zeroizing::new([0u8; 32]);
        out.copy_from_slice(&outer.finalize());
        out
    }
}

fn pbkdf2_sha256(passphrase: &[u8], salt: &[u8], iterations: u32, out: &mut [u8]) {
    let prf = Hmac::new(passphrase);
    for (i, chunk) in out.chunks_mut(32).enumerate() {
        let index = (i as u32 + 1).to_be_bytes();
        let mut u = prf.mac(&[salt, &index]);
        let mut t = Zeroizing::new(*u);
        for _ in 1..iterations {
            u = prf.mac(&[u.as_ref()]);
            for (a, b) in t.iter_mut().zip(u.iter()) {
                *a ^= b;
            }
        }
        chunk.copy_from_slice(&t[..chunk.len()]);
    }
}

// ---- AES-256 (FIPS-197), encryption direction only ----

/// Multiplication in GF(2^8) modulo x^8 + x^4 + x^3 + x + 1, without branches on the operands.
fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0u8;
    for _ in 0..8 {
        p ^= a & 0u8.wrapping_sub(b & 1);
        let high = a >> 7;
        a = (a << 1) ^ (0x1b & 0u8.wrapping_sub(high));
        b >>= 1;
    }
    p
}

/// The S-box computed as the affine map of x^254 (the inverse, 0 for 0): no table indexed by secret bytes.
fn sub_byte(x: u8) -> u8 {
    // x^254 = x^(2+4+8+16+32+64+128); the exponent is public, so the loop shape is fixed.
    let mut result = 1u8;
    let mut power = gf_mul(x, x);
    for _ in 1..8 {
        result = gf_mul(result, power);
        power = gf_mul(power, power);
    }
    result ^ result.rotate_left(1) ^ result.rotate_left(2) ^ result.rotate_left(3) ^ result.rotate_left(4) ^ 0x63
}

struct Aes256 {
    round_keys: Zeroizing<[[u8; 16]; 15]>,
}
impl Aes256 {
    fn new(key: &[u8; 32]) -> Self {
        let mut w = Zeroizing::new([[0u8; 4]; 60]);
        for (i, word) in w.iter_mut().take(8).enumerate() {
            word.copy_from_slice(&key[4 * i..4 * i + 4]);
        }
        let mut rcon = 1u8;
        for i in 8..60 {
            let mut t = w[i - 1];
            if i % 8 == 0 {
                t = [sub_byte(t[1]) ^ rcon, sub_byte(t[2]), sub_byte(t[3]), sub_byte(t[0])];
                rcon = gf_mul(rcon, 2);
            } else if i % 8 == 4 {
                t = [sub_byte(t[0]), sub_byte(t[1]), sub_byte(t[2]), sub_byte(t[3])];
            }
            for j in 0..4 {
                w[i][j] = w[i - 8][j] ^ t[j];
            }
        }
        let mut round_keys = Zeroizing::new([[0u8; 16]; 15]);
        for (r, rk) in round_keys.iter_mut().enumerate() {
            for c in 0..4 {
                rk[4 * c..4 * c + 4].copy_from_slice(&w[4 * r + c]);
            }
        }
        Self { round_keys }
    }
    fn encrypt(&self, block: &mut [u8; 16]) {
        let add = |b: &mut [u8; 16], k: &[u8; 16]| b.iter_mut().zip(k).for_each(|(x, y)| *x ^= y);
        add(block, &self.round_keys[0]);
        for round in 1..15 {
            for b in block.iter_mut() {
                *b = sub_byte(*b);
            }
            // ShiftRows: the state is column-major, byte r + 4c is row r of column c.
            let s = *block;
            for c in 0..4 {
                for r in 0..4 {
                    block[r + 4 * c] = s[r + 4 * ((c + r) % 4)];
                }
            }
            if round != 14 {
                for c in 0..4 {
                    let col = [block[4 * c], block[4 * c + 1], block[4 * c + 2], block[4 * c + 3]];
                    for r in 0..4 {
                        block[4 * c + r] = gf_mul(col[r], 2) ^ gf_mul(col[(r + 1) % 4], 3) ^ col[(r + 2) % 4] ^ col[(r + 3) % 4];
                    }
                }
            }
            add(block, &self.round_keys[round]);
        }
    }
}

// ---- GCM (NIST SP 800-38D) with a 96-bit nonce ----

/// X·Y in GF(2^128) with GCM's bit order (algorithm 1 of SP 800-38D), masks instead of branches.
fn gf128_mul(x: u128, y: u128) -> u128 {
    let (mut z, mut v) = (0u128, y);
    for i in 0..128 {
        z ^= v & 0u128.wrapping_sub((x >> (127 - i)) & 1);
        v = (v >> 1) ^ ((0xe1u128 << 120) & 0u128.wrapping_sub(v & 1));
    }
    z
}

fn ghash(h: u128, aad: &[u8], ciphertext: &[u8]) -> u128 {
    let mut y = 0u128;
    for data in [aad, ciphertext] {
        for chunk in data.chunks(16) {
            let mut block = [0u8; 16];
            block[..chunk.len()].copy_from_slice(chunk);
            y = gf128_mul(y ^ u128::from_be_bytes(block), h);
        }
    }
    let lengths = ((aad.len() as u128 * 8) << 64) | (ciphertext.len() as u128 * 8);
    gf128_mul(y ^ lengths, h)
}

/// The counter stream XORed into `data` from counter 2 on, and E(K, J0) for the tag.
fn ctr(aes: &Aes256, nonce: &[u8; 12], data: &mut [u8]) -> [u8; 16] {
    let mut counter = [0u8; 16];
    counter[..12].copy_from_slice(nonce);
    counter[15] = 1;
    let mut tag_mask = counter;
    aes.encrypt(&mut tag_mask);
    for (i, chunk) in data.chunks_mut(16).enumerate() {
        counter[12..].copy_from_slice(&(i as u32 + 2).to_be_bytes());
        let mut stream = counter;
        aes.encrypt(&mut stream);
        chunk.iter_mut().zip(stream.iter()).for_each(|(d, s)| *d ^= s);
        stream.zeroize();
    }
    tag_mask
}

fn hash_key(aes: &Aes256) -> u128 {
    let mut h = [0u8; 16];
    aes.encrypt(&mut h);
    u128::from_be_bytes(h)
}

/// Encrypts `data` in place and returns the tag.
fn gcm_seal(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8]) -> [u8; 16] {
    let aes = Aes256::new(key);
    let mask = ctr(&aes, nonce, data);
    (ghash(hash_key(&aes), aad, data) ^ u128::from_be_bytes(mask)).to_be_bytes()
}

/// Checks the tag first (constant-time comparison), then decrypts `data` in place. False leaves `data` as
/// ciphertext.
fn gcm_open(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8], tag: &[u8; 16]) -> bool {
    let aes = Aes256::new(key);
    let mut mask = [0u8; 16];
    mask[..12].copy_from_slice(nonce);
    mask[15] = 1;
    aes.encrypt(&mut mask);
    let expected = (ghash(hash_key(&aes), aad, data) ^ u128::from_be_bytes(mask)).to_be_bytes();
    if expected.iter().zip(tag).fold(0u8, |acc, (a, b)| acc | (a ^ b)) != 0 {
        return false;
    }
    ctr(&aes, nonce, data);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }
    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }
    const SECRET: [u8; 32] = [0x4c; 32];
    const PASS: &[u8] = "correct horse battery staple ✓".as_bytes();

    #[test]
    fn aes256_matches_fips_197_appendix_c3() {
        let key: [u8; 32] = core::array::from_fn(|i| i as u8);
        let mut block: [u8; 16] = unhex("00112233445566778899aabbccddeeff").try_into().unwrap();
        Aes256::new(&key).encrypt(&mut block);
        assert_eq!(hex(&block), "8ea2b7ca516745bfeafc49904b496089");
    }
    #[test]
    fn computed_sbox_matches_the_published_corners() {
        assert_eq!([sub_byte(0x00), sub_byte(0x01), sub_byte(0x53), sub_byte(0xff)], [0x63, 0x7c, 0xed, 0x16]);
    }
    #[test]
    fn gcm_matches_the_specification_test_cases_13_14_and_16() {
        // McGrew–Viega test cases for AES-256 (also produced by OpenSSL through Python's cryptography).
        let zero = [0u8; 32];
        assert_eq!(hex(&gcm_seal(&zero, &[0; 12], &[], &mut [])), "530f8afbc74536b9a963b4f1c4cb738b");
        let mut data = [0u8; 16];
        let tag = gcm_seal(&zero, &[0; 12], &[], &mut data);
        assert_eq!((hex(&data), hex(&tag)), ("cea7403d4d606b6e074ec5d3baf39d18".into(), "d0d1c8a799996bf0265b98b5d48ab919".into()));
        let key: [u8; 32] = unhex("feffe9928665731c6d6a8f9467308308feffe9928665731c6d6a8f9467308308").try_into().unwrap();
        let nonce: [u8; 12] = unhex("cafebabefacedbaddecaf888").try_into().unwrap();
        let aad = unhex("feedfacedeadbeeffeedfacedeadbeefabaddad2");
        let plain = unhex("d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a721c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b39");
        let mut data = plain.clone();
        let tag = gcm_seal(&key, &nonce, &aad, &mut data);
        assert_eq!(hex(&data), "522dc1f099567d07f47f37a32a84427d643a8cdcbfe5c0c97598a2bd2555d1aa8cb08e48590dbb3da7b08b1056828838c5f61e6393ba7a0abcc9f662");
        assert_eq!(hex(&tag), "76fc6ece0f4e1768cddf8853bb2d551b");
        assert!(gcm_open(&key, &nonce, &aad, &mut data, &tag));
        assert_eq!(data, plain);
        let mut bad = tag;
        bad[0] ^= 1;
        let mut again = data.clone();
        gcm_seal(&key, &nonce, &aad, &mut again);
        assert!(!gcm_open(&key, &nonce, &aad, &mut again.clone(), &bad));
        assert!(!gcm_open(&key, &nonce, &aad[1..], &mut again, &tag));
    }
    #[test]
    fn pbkdf2_matches_rfc_7914_and_the_python_reference() {
        let mut out = [0u8; 64];
        pbkdf2_sha256(b"passwd", b"salt", 1, &mut out);
        assert_eq!(hex(&out), "55ac046e56e3089fec1691c22544b605f94185216dde0465e68b9d57c20dacbc49ca9cccf179b645991664b39d77ef317c71b845b1e30bd509112041d3a19783");
        let mut out = [0u8; 32];
        pbkdf2_sha256(b"password", b"salt", 4096, &mut out);
        assert_eq!(hex(&out), "c5e478d59288c841aa530db6845c4c8d962893a001ce4e11a4963873aa98134a");
        // A key longer than the 64-byte block is hashed first.
        let long = [0x61u8; 100];
        let mut a = [0u8; 32];
        pbkdf2_sha256(&long, b"salt", 2, &mut a);
        let mut b = [0u8; 32];
        pbkdf2_sha256(&Sha256::digest(&long), b"salt", 2, &mut b);
        assert_eq!(a, b);
    }
    /// The whole file for fixed salt and nonce, made independently with Python's `hashlib.pbkdf2_hmac` and
    /// OpenSSL's AES-GCM (`cryptography`'s AESGCM): header, then AESGCM(key).encrypt(nonce, secret, header).
    #[test]
    fn a_fixed_backup_equals_the_openssl_reference_file() {
        let salt: [u8; 16] = core::array::from_fn(|i| 0xa0 + i as u8);
        let nonce: [u8; 12] = core::array::from_fn(|i| 0xb0 + i as u8);
        let file = seal_with(&SECRET, PASS, DEFAULT_ITERATIONS, &salt, &nonce).unwrap();
        assert_eq!(hex(&file), REFERENCE);
        assert_eq!(open(&unhex(REFERENCE), PASS).unwrap().as_ref(), &SECRET);
    }
    const REFERENCE: &str = concat!(
        "504f434f4c2d4b45592d4241434b5550", "01", "01", "000927c0", "a0a1a2a3a4a5a6a7a8a9aaabacadaeaf", "b0b1b2b3b4b5b6b7b8b9babb",
        "4f0b9183ee8e37b2b41738cf2128bce96f4f0a4ffb0f3ee1bfae4f0ee9afd0fa",
        "9056f0bc2660ad0a7deebdd50843bcd2",
    );
    #[test]
    fn round_trip_with_random_salt_and_nonce() {
        let a = seal(&SECRET, PASS).unwrap();
        let b = seal(&SECRET, PASS).unwrap();
        assert_eq!((a.len(), &a[..16], a[16], a[17]), (LEN, &MAGIC[..], VERSION, KDF_PBKDF2_SHA256));
        assert_eq!(u32::from_be_bytes(a[18..22].try_into().unwrap()), DEFAULT_ITERATIONS);
        assert_ne!(a[22..50], b[22..50], "fresh salt and nonce");
        assert!(!a.windows(32).any(|w| w == SECRET), "the secret never appears in the file");
        assert_eq!(open(&a, PASS).unwrap().as_ref(), &SECRET);
        assert_eq!(open(&b, PASS).unwrap().as_ref(), &SECRET);
    }
    #[test]
    fn wrong_passphrase_is_refused() {
        let file = seal(&SECRET, PASS).unwrap();
        assert_eq!(open(&file, b"correct horse battery stapl").unwrap_err(), "Wrong passphrase or damaged key backup");
        assert_eq!(open(&file, b"").unwrap_err(), "Enter the backup passphrase");
    }
    #[test]
    fn every_tampered_byte_is_refused() {
        let file = unhex(REFERENCE);
        // Header bytes the parser checks fail early; every other byte (salt, nonce, ciphertext, tag, and an
        // iteration count still inside the bounds) fails authentication. Iterations in range cost a full KDF,
        // so this flips one byte of each field.
        for at in [0, 15, 16, 17, 21, 22, 37, 38, 49, 50, 81, 82, 97] {
            let mut bad = file.clone();
            bad[at] ^= 0x01;
            let e = open(&bad, PASS).unwrap_err();
            let expected = match at {
                0..=15 => "Not a PoCol key backup",
                16 => "Unsupported key backup version 0; update PoCol to open it",
                17 => "Unsupported key backup KDF",
                _ => "Wrong passphrase or damaged key backup",
            };
            assert_eq!(e, expected, "byte {at}");
        }
        for len in [0, 16, 17, LEN - 1, LEN + 1] {
            let mut cut = file.clone();
            cut.resize(len, 0);
            assert!(open(&cut, PASS).is_err(), "length {len}");
        }
    }
    #[test]
    fn a_future_version_is_named_and_refused() {
        let mut file = unhex(REFERENCE);
        file[16] = 2;
        file.extend_from_slice(&[0; 40]);
        assert_eq!(open(&file, PASS).unwrap_err(), "Unsupported key backup version 2; update PoCol to open it");
    }
    #[test]
    fn kdf_iterations_are_bounded_before_any_derivation() {
        let file = unhex(REFERENCE);
        for iterations in [0, 1, MIN_ITERATIONS - 1, MAX_ITERATIONS + 1, u32::MAX] {
            let mut bad = file.clone();
            bad[18..22].copy_from_slice(&iterations.to_be_bytes());
            let started = std::time::Instant::now();
            assert_eq!(open(&bad, PASS).unwrap_err(), format!("Key backup KDF iterations outside {MIN_ITERATIONS}..={MAX_ITERATIONS}"));
            assert!(started.elapsed() < std::time::Duration::from_millis(50), "refused without running the KDF");
        }
    }
    #[test]
    fn new_passphrases_follow_the_export_policy() {
        assert!(check_new_passphrase(b"short").is_err());
        assert!(check_new_passphrase(b"            ").is_err(), "whitespace only");
        assert!(check_new_passphrase(&[0xff; 20]).is_err(), "not UTF-8");
        assert!(check_new_passphrase(&vec![b'a'; MAX_PASSPHRASE_BYTES + 1]).is_err());
        assert!(check_new_passphrase("١٢٣٤٥٦٧٨٩٠١٢".as_bytes()).is_ok(), "12 characters, 24 bytes");
        assert_eq!(seal(&SECRET, b"eleven char").unwrap_err(), "Use a passphrase of at least 12 characters");
    }
}
