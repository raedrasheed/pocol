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
//! The crate pins its dependencies through the consensus boundary (`Cargo.toml` is a `+` file), so SHA-256,
//! HMAC, PBKDF2, AES-256 and GCM are implemented here and checked against FIPS-180 and FIPS-197, the GCM
//! specification's test cases, RFC 7914's PBKDF2 vector, the `sha2` crate and a vector made with OpenSSL. They
//! avoid secret-indexed tables and secret-dependent branches: the S-box is computed (GF(2^8) inversion and the
//! affine map) and GHASH multiplies with masks. SHA-256 is implemented here, not taken from `sha2`, so that the
//! HMAC states (equivalent to the passphrase for this KDF) are ours to wipe: `sha2` 0.9 does not zeroize.
//!
//! Wiping is best effort. Every buffer that holds the passphrase, a key, an HMAC state, a PBKDF2 block, a round
//! key or an AES state is overwritten when it goes out of use. Copies the compiler makes in registers or on the
//! stack (the working variables of one SHA-256 or AES round, the GHASH accumulator) are not covered.
//!
//! KDF id 2 is reserved for a memory-hard KDF (scrypt or Argon2) in a later format version, once such a crate
//! can be pinned through the consensus lock flow. Until then the export policy below refuses passphrases that
//! guessing tools try first; PBKDF2 alone does not slow a GPU attack on a weak one enough.
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

/// At least this many different characters in a new passphrase.
pub const MIN_DISTINCT_CHARS: usize = 5;
/// The start of every refusal for a passphrase that is long enough but easy to guess.
pub const WEAK: &str = "This passphrase is too easy to guess";
/// Lower-case letter cores of the passwords and keyboard patterns that guessing tools try first. A passphrase is
/// refused when its letters, with digits and symbols removed and any repetition collapsed, are one of these:
/// "Password2024!", "P@ssw0rd1234", "iloveyou!!!!" and "passwordpassword" all reduce to an entry.
const COMMON: &[&str] = &[
    "password", "passwd", "passphrase", "mypassword", "mypassphrase", "secret", "letmein", "iloveyou", "welcome",
    "hello", "helloworld", "admin", "administrator", "root", "login", "changeme", "default", "master", "monkey",
    "dragon", "shadow", "sunshine", "princess", "football", "baseball", "soccer", "superman", "batman", "trustno",
    "whatever", "freedom", "starwars", "qwerty", "qwertz", "azerty", "qazwsx", "qazwsxedc", "abc", "test", "testing",
    "pocol", "pocolwallet", "wallet", "mywallet", "bitcoin", "ethereum", "crypto", "backup", "keybackup",
    "privatekey",
];
/// Runs a letter core of four or more may not be part of (forwards or backwards): the alphabet and keyboard rows.
const RUNS: &[&str] = &["abcdefghijklmnopqrstuvwxyz", "qwertyuiopasdfghjklzxcvbnm"];

/// The passphrase a new backup may use: UTF-8, at least [`MIN_PASSPHRASE_CHARS`] characters, at most
/// [`MAX_PASSPHRASE_BYTES`] bytes, not only whitespace, and not trivially guessable: at least
/// [`MIN_DISTINCT_CHARS`] different characters, not a repeated shorter part ("abcabcabcabc"), not a run of
/// consecutive characters ("abcdefghijkl", "345678901234"), and not built on a [`COMMON`] password or a keyboard
/// run. The rules are fixed and need no data beyond this file, so every platform decides the same way.
pub fn check_new_passphrase(passphrase: &[u8]) -> Result<(), String> {
    let text = std::str::from_utf8(passphrase).map_err(|_| "Passphrase must be valid text")?;
    if passphrase.len() > MAX_PASSPHRASE_BYTES {
        return Err(format!("Passphrase exceeds {MAX_PASSPHRASE_BYTES} bytes"));
    }
    if text.chars().count() < MIN_PASSPHRASE_CHARS || text.trim().is_empty() {
        return Err(format!("Use a passphrase of at least {MIN_PASSPHRASE_CHARS} characters"));
    }
    let chars = Zeroizing::new(text.chars().flat_map(char::to_lowercase).collect::<Vec<char>>());
    let mut distinct = Zeroizing::new(chars.to_vec());
    distinct.sort_unstable();
    distinct.dedup();
    if distinct.len() < MIN_DISTINCT_CHARS {
        return Err(format!("{WEAK}: use at least {MIN_DISTINCT_CHARS} different characters"));
    }
    if period(&chars) < chars.len() {
        return Err(format!("{WEAK}: it repeats a shorter part"));
    }
    if is_run(&chars) {
        return Err(format!("{WEAK}: it is a sequence such as abcd or 1234"));
    }
    // Letters only, and letters inside a word whose symbols stand for letters (P@ssw0rd).
    let letters = Zeroizing::new(chars.iter().copied().filter(|c| c.is_alphabetic()).collect::<Vec<char>>());
    let first = chars.iter().position(|c| c.is_alphabetic());
    let last = chars.iter().rposition(|c| c.is_alphabetic());
    let inner = Zeroizing::new(match (first, last) {
        (Some(a), Some(b)) => chars[a..=b].iter().map(|&c| leet(c)).filter(|c| c.is_alphabetic()).collect::<Vec<char>>(),
        _ => Vec::new(),
    });
    for core in [&letters, &inner] {
        let unit = &core[..period(core)];
        let mut word = Zeroizing::new(String::with_capacity(4 * unit.len()));
        word.extend(unit.iter());
        let reversed = Zeroizing::new(word.chars().rev().collect::<String>());
        let in_run = unit.len() >= 4 && RUNS.iter().any(|r| r.contains(word.as_str()) || r.contains(reversed.as_str()));
        if COMMON.contains(&word.as_str()) || in_run {
            return Err(format!("{WEAK}: it is built on a common password or keyboard pattern"));
        }
    }
    Ok(())
}

/// The length of the shortest unit whose repetition (the last one possibly cut short) is `chars`; `chars.len()`
/// when no unit of at most half the length repeats.
fn period(chars: &[char]) -> usize {
    (1..=chars.len() / 2).find(|&p| (p..chars.len()).all(|i| chars[i] == chars[i - p])).unwrap_or(chars.len())
}

/// Whether every character is the next (or every one the previous) code point of the one before; within a
/// block of decimal digits 9 is followed by 0 ("789012", and the same in Arabic-Indic or any other digits).
fn is_run(chars: &[char]) -> bool {
    let step = |a: char, b: char| {
        let d = b as i64 - a as i64;
        match d {
            -9 if a.is_numeric() && b.is_numeric() => 1,
            9 if a.is_numeric() && b.is_numeric() => -1,
            d => d,
        }
    };
    let first = step(chars[0], chars[1]);
    (first == 1 || first == -1) && chars.windows(2).all(|w| step(w[0], w[1]) == first)
}

/// The letter a symbol or digit commonly stands for inside a word.
fn leet(c: char) -> char {
    match c {
        '@' | '4' => 'a',
        '3' => 'e',
        '1' | '!' => 'i',
        '0' => 'o',
        '$' | '5' => 's',
        '7' => 't',
        c => c,
    }
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

// ---- SHA-256 (FIPS 180-4) ----

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01,
    0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
    0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
    0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08,
    0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];
const SHA256_IV: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];

fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = Zeroizing::new([0u32; 64]);
    for (i, word) in block.chunks_exact(4).enumerate() {
        w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
    }
    let mut v = Zeroizing::new(*state);
    for i in 0..64 {
        let [a, b, c, d, e, f, g, h] = *v;
        let t1 = h
            .wrapping_add(e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25))
            .wrapping_add((e & f) ^ (!e & g))
            .wrapping_add(SHA256_K[i])
            .wrapping_add(w[i]);
        let t2 = (a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22)).wrapping_add((a & b) ^ (a & c) ^ (b & c));
        *v = [t1.wrapping_add(t2), a, b, c, d.wrapping_add(t1), e, f, g];
    }
    for (s, x) in state.iter_mut().zip(v.iter()) {
        *s = s.wrapping_add(*x);
    }
}

/// An incremental SHA-256 whose state and buffered block are wiped on drop (and by [`Zeroize`]).
#[derive(Clone)]
struct Sha256 {
    state: [u32; 8],
    block: [u8; 64],
    used: usize,
    length: u64,
}
impl Sha256 {
    fn new() -> Self {
        Self { state: SHA256_IV, block: [0; 64], used: 0, length: 0 }
    }
    fn update(&mut self, mut data: &[u8]) {
        self.length = self.length.wrapping_add(data.len() as u64);
        while !data.is_empty() {
            let take = (64 - self.used).min(data.len());
            self.block[self.used..self.used + take].copy_from_slice(&data[..take]);
            self.used += take;
            data = &data[take..];
            if self.used == 64 {
                compress(&mut self.state, &self.block);
                self.used = 0;
            }
        }
    }
    fn finalize(mut self) -> Zeroizing<[u8; 32]> {
        let bits = self.length.wrapping_mul(8);
        let used = self.used;
        self.block[used] = 0x80;
        self.block[used + 1..].fill(0);
        if used >= 56 {
            compress(&mut self.state, &self.block);
            self.block.fill(0);
        }
        self.block[56..].copy_from_slice(&bits.to_be_bytes());
        compress(&mut self.state, &self.block);
        let mut out = Zeroizing::new([0u8; 32]);
        for (o, s) in out.chunks_exact_mut(4).zip(self.state.iter()) {
            o.copy_from_slice(&s.to_be_bytes());
        }
        out
    }
}
impl Zeroize for Sha256 {
    fn zeroize(&mut self) {
        self.state.zeroize();
        self.block.zeroize();
        self.used.zeroize();
        self.length.zeroize();
    }
}
impl Drop for Sha256 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

// ---- HMAC-SHA256 and PBKDF2 (RFC 2104, RFC 8018) ----

/// The inner and outer hash states after the padded key block, cloned for every message. They are equivalent to
/// the passphrase, so they are wiped with the `Hmac` (the `Sha256` drop), as is every clone.
struct Hmac {
    inner: Sha256,
    outer: Sha256,
}
impl Hmac {
    fn new(key: &[u8]) -> Self {
        let mut block = Zeroizing::new([0u8; 64]);
        if key.len() > 64 {
            let mut long = Sha256::new();
            long.update(key);
            block[..32].copy_from_slice(long.finalize().as_ref());
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
        outer.update(inner.finalize().as_ref());
        outer.finalize()
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
        // `t` holds key-schedule bytes, so it is wiped like `w`.
        let mut t = Zeroizing::new([0u8; 4]);
        for i in 8..60 {
            *t = w[i - 1];
            if i % 8 == 0 {
                *t = [sub_byte(t[1]) ^ rcon, sub_byte(t[2]), sub_byte(t[3]), sub_byte(t[0])];
                rcon = gf_mul(rcon, 2);
            } else if i % 8 == 4 {
                *t = [sub_byte(t[0]), sub_byte(t[1]), sub_byte(t[2]), sub_byte(t[3])];
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
        // Copies of the AES state, wiped after the last round.
        let (mut s, mut col) = (Zeroizing::new([0u8; 16]), Zeroizing::new([0u8; 4]));
        add(block, &self.round_keys[0]);
        for round in 1..15 {
            for b in block.iter_mut() {
                *b = sub_byte(*b);
            }
            // ShiftRows: the state is column-major, byte r + 4c is row r of column c.
            *s = *block;
            for c in 0..4 {
                for r in 0..4 {
                    block[r + 4 * c] = s[r + 4 * ((c + r) % 4)];
                }
            }
            if round != 14 {
                for c in 0..4 {
                    *col = [block[4 * c], block[4 * c + 1], block[4 * c + 2], block[4 * c + 3]];
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

/// The counter stream XORed into `data` from counter 2 on; E(K, J0), the tag mask, goes to `tag_mask`.
fn ctr(aes: &Aes256, nonce: &[u8; 12], data: &mut [u8], tag_mask: &mut [u8; 16]) {
    let mut counter = [0u8; 16];
    counter[..12].copy_from_slice(nonce);
    counter[15] = 1;
    *tag_mask = counter;
    aes.encrypt(tag_mask);
    let mut stream = Zeroizing::new([0u8; 16]);
    for (i, chunk) in data.chunks_mut(16).enumerate() {
        counter[12..].copy_from_slice(&(i as u32 + 2).to_be_bytes());
        *stream = counter;
        aes.encrypt(&mut stream);
        chunk.iter_mut().zip(stream.iter()).for_each(|(d, s)| *d ^= s);
    }
}

fn hash_key(aes: &Aes256) -> u128 {
    let mut h = Zeroizing::new([0u8; 16]);
    aes.encrypt(&mut h);
    u128::from_be_bytes(*h)
}

/// Encrypts `data` in place and returns the tag.
fn gcm_seal(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8]) -> [u8; 16] {
    let aes = Aes256::new(key);
    let mut mask = Zeroizing::new([0u8; 16]);
    ctr(&aes, nonce, data, &mut mask);
    (ghash(hash_key(&aes), aad, data) ^ u128::from_be_bytes(*mask)).to_be_bytes()
}

/// Checks the tag first (constant-time comparison), then decrypts `data` in place. False leaves `data` as
/// ciphertext.
fn gcm_open(key: &[u8; 32], nonce: &[u8; 12], aad: &[u8], data: &mut [u8], tag: &[u8; 16]) -> bool {
    let aes = Aes256::new(key);
    let mut mask = Zeroizing::new([0u8; 16]);
    mask[..12].copy_from_slice(nonce);
    mask[15] = 1;
    aes.encrypt(&mut mask);
    let expected = (ghash(hash_key(&aes), aad, data) ^ u128::from_be_bytes(*mask)).to_be_bytes();
    if expected.iter().zip(tag).fold(0u8, |acc, (a, b)| acc | (a ^ b)) != 0 {
        return false;
    }
    ctr(&aes, nonce, data, &mut mask);
    true
}

// ---- Passphrase input of the `pocol` command ----

/// Where the `pocol` command reads a passphrase from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassphraseInput {
    /// A pipe or file (the Windows wallet writes the line through a pipe).
    Pipe,
    /// A terminal with echo switched off.
    Hidden,
    /// A terminal whose echo could not be switched off: typing would show the passphrase.
    Visible,
}
pub const CANCELLED: &str = "cancelled; nothing was changed";

/// One passphrase line (at most [`MAX_PASSPHRASE_BYTES`] bytes) from `input`, without its line ending. A terminal
/// whose echo is still on is refused before anything is read. `cancelled` (the Ctrl-C flag) is checked before
/// each read, when a read is interrupted and after the line, so Ctrl-C cancels the prompt instead of being
/// ignored. The line is collected in one pre-sized buffer that never reallocates, and is wiped on drop.
pub fn read_passphrase(input: &mut dyn std::io::BufRead, from: PassphraseInput, cancelled: &dyn Fn() -> bool) -> Result<Zeroizing<Vec<u8>>, String> {
    if from == PassphraseInput::Visible {
        return Err("terminal echo cannot be switched off, so the passphrase would be shown; nothing was read. Pipe the passphrase on stdin instead".into());
    }
    let limit = MAX_PASSPHRASE_BYTES + 2;
    let mut line = Zeroizing::new(Vec::with_capacity(limit));
    loop {
        if cancelled() {
            return Err(CANCELLED.into());
        }
        let buffer = match input.fill_buf() {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.to_string()),
        };
        if buffer.is_empty() {
            break;
        }
        let room = &buffer[..buffer.len().min(limit - line.len())];
        let (take, done) = match room.iter().position(|&b| b == b'\n') {
            Some(i) => (i + 1, true),
            None => (room.len(), false),
        };
        line.extend_from_slice(&room[..take]);
        input.consume(take);
        if done || line.len() == limit {
            break;
        }
    }
    if cancelled() {
        return Err(CANCELLED.into());
    }
    if line.last() == Some(&b'\n') {
        line.pop();
        if line.last() == Some(&b'\r') {
            line.pop();
        }
    } else if line.is_empty() {
        return Err("no passphrase on stdin".into());
    }
    if line.len() > MAX_PASSPHRASE_BYTES {
        return Err(format!("passphrase exceeds {MAX_PASSPHRASE_BYTES} bytes"));
    }
    Ok(line)
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
        pbkdf2_sha256(&<sha2::Sha256 as sha2::Digest>::digest(&long), b"salt", 2, &mut b);
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
        assert!(check_new_passphrase("كلمة سر قوية".as_bytes()).is_ok(), "12 characters, 22 bytes");
        assert_eq!(seal(&SECRET, b"eleven char").unwrap_err(), "Use a passphrase of at least 12 characters");
    }
    #[test]
    fn trivially_guessable_passphrases_are_refused() {
        let weak = [
            ("aaaaaaaaaaaa", "different characters"),
            ("abababababababab", "different characters"),
            ("1111 2222 3333", "different characters"),
            ("abcdeabcdeabcde", "repeats"),
            ("Pocol!Pocol!Pocol!", "repeats"),
            ("abcdefghijklm", "sequence"),
            ("ZYXWVUTSRQPO", "sequence"),
            ("345678901234", "sequence"),
            ("٣٤٥٦٧٨٩٠١٢٣٤", "sequence"),
            ("Password2024!", "common"),
            ("P@ssw0rd1234", "common"),
            ("passwordpassword1", "common"),
            ("iloveyou!!!!", "common"),
            ("1qaz2wsx3edc", "common"),
            ("qwertyuiop123", "common"),
            ("mnbvcxzlkj99", "common"),
            ("2024 bitcoin 2024", "common"),
            ("abcd 77291830", "common"),
        ];
        for (pass, why) in weak {
            let e = check_new_passphrase(pass.as_bytes()).unwrap_err();
            assert!(e.starts_with(WEAK), "{pass}: {e}");
            assert!(e.contains(why), "{pass}: {e}");
            assert_eq!(seal(&SECRET, pass.as_bytes()).unwrap_err(), e, "seal applies the same policy");
        }
        for strong in [
            "correct horse battery staple",
            "a long backup passphrase ✓",
            "Tr0ub4dor&3 river",
            "my password is long and odd",
            "8472 9183 2201",
            "kettle-violet-41-orbit",
            "كلمة سر قوية",
        ] {
            assert_eq!(check_new_passphrase(strong.as_bytes()), Ok(()), "{strong}");
        }
    }
    #[test]
    fn sha256_matches_fips_180_and_the_sha2_crate() {
        let digest = |m: &[u8]| {
            let mut h = Sha256::new();
            h.update(m);
            *h.finalize()
        };
        assert_eq!(hex(&digest(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(hex(&digest(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let data: Vec<u8> = (0..300u32).map(|i| (i * 31 + 7) as u8).collect();
        for len in 0..data.len() {
            let theirs = <sha2::Sha256 as sha2::Digest>::digest(&data[..len]);
            assert_eq!(digest(&data[..len])[..], theirs[..], "length {len}");
            // Split updates give the same digest.
            let mut h = Sha256::new();
            h.update(&data[..len / 3]);
            h.update(&data[len / 3..len]);
            assert_eq!(h.finalize()[..], theirs[..], "split at {}", len / 3);
        }
    }
    #[test]
    fn hmac_states_are_wiped() {
        // The HMAC states equal the passphrase for this KDF: `Sha256` zeroizes its state and block (also on drop).
        let prf = Hmac::new(PASS);
        let mut inner = prf.inner.clone();
        assert_ne!(inner.state, SHA256_IV, "the key block is absorbed");
        inner.update(b"partial");
        inner.zeroize();
        assert_eq!((inner.state, inner.block, inner.used, inner.length), ([0; 8], [0; 64], 0, 0));
    }
    /// Reads in the given chunks; an error chunk is returned once, and sets `signalled` as Ctrl-C would.
    struct Chunks<'a>(Vec<std::io::Result<Vec<u8>>>, Vec<u8>, &'a std::cell::Cell<bool>);
    impl std::io::Read for Chunks<'_> {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> { unreachable!() }
    }
    impl std::io::BufRead for Chunks<'_> {
        fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
            if self.1.is_empty() && !self.0.is_empty() {
                match self.0.remove(0) {
                    Ok(b) => self.1 = b,
                    Err(e) => {
                        self.2.set(true);
                        return Err(e);
                    }
                }
            }
            Ok(&self.1)
        }
        fn consume(&mut self, n: usize) { self.1.drain(..n); }
    }
    #[test]
    fn a_terminal_that_still_echoes_is_refused_before_anything_is_read() {
        let mut input = std::io::Cursor::new(b"a long backup passphrase\n".to_vec());
        let e = read_passphrase(&mut input, PassphraseInput::Visible, &|| false).unwrap_err();
        assert!(e.contains("echo cannot be switched off"), "{e}");
        assert_eq!(input.position(), 0, "nothing was read");
        for from in [PassphraseInput::Pipe, PassphraseInput::Hidden] {
            let mut input = std::io::Cursor::new(b"a long backup passphrase\r\nnext\n".to_vec());
            assert_eq!(read_passphrase(&mut input, from, &|| false).unwrap().as_slice(), b"a long backup passphrase");
            assert_eq!(read_passphrase(&mut input, from, &|| false).unwrap().as_slice(), b"next");
            assert_eq!(read_passphrase(&mut input, from, &|| false).unwrap_err(), "no passphrase on stdin");
        }
        let mut long = std::io::Cursor::new(vec![b'a'; MAX_PASSPHRASE_BYTES + 5]);
        assert!(read_passphrase(&mut long, PassphraseInput::Pipe, &|| false).unwrap_err().contains("exceeds"));
        let mut last = std::io::Cursor::new(b"no line ending".to_vec());
        assert_eq!(read_passphrase(&mut last, PassphraseInput::Pipe, &|| false).unwrap().as_slice(), b"no line ending");
    }
    #[test]
    fn ctrl_c_cancels_the_passphrase_prompt() {
        // Ctrl-C sets the stop flag and interrupts the read: the prompt ends in a cancel, not a retry.
        let stop = std::cell::Cell::new(false);
        let mut interrupted = Chunks(vec![Ok(b"half".to_vec()), Err(std::io::ErrorKind::Interrupted.into()), Ok(b" typed\n".to_vec())], Vec::new(), &stop);
        assert_eq!(read_passphrase(&mut interrupted, PassphraseInput::Hidden, &|| stop.get()).unwrap_err(), CANCELLED);
        // Where the read restarts (Windows retries a console read), a line typed after Ctrl-C cancels too.
        let set = std::cell::Cell::new(0);
        let after_line = || {
            set.set(set.get() + 1);
            set.get() > 1
        };
        let mut typed = std::io::Cursor::new(b"a long backup passphrase\n".to_vec());
        assert_eq!(read_passphrase(&mut typed, PassphraseInput::Hidden, &after_line).unwrap_err(), CANCELLED, "Ctrl-C then Enter cancels");
        let other = std::cell::Cell::new(false);
        let mut retry = Chunks(vec![Err(std::io::ErrorKind::Interrupted.into()), Ok(b"a long backup passphrase\n".to_vec())], Vec::new(), &other);
        assert_eq!(read_passphrase(&mut retry, PassphraseInput::Hidden, &|| false).unwrap().as_slice(), b"a long backup passphrase", "an interrupt without Ctrl-C retries");
    }
}
