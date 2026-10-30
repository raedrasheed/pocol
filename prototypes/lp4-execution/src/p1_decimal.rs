//! Exact decimal rendering of an unsigned big-endian integer of any length.
//!
//! The accepted GenesisSpec reference reports gsVersion with the integer value of the specVersion
//! item, which can be as long as the input. Repeated division by 10^9 costs O(n^2) in the length n;
//! here the base-2^32 limbs are split in halves recursively and recombined with Karatsuba
//! multiplication in base 10^9, so the cost is O(n^1.59 log n) and memory O(n). Recursion depth is
//! O(log n).

const BASE: u64 = 1_000_000_000;
/// Below this many limbs, plain repeated division is faster than splitting.
const SMALL: usize = 64;
/// Below this many base-10^9 digits on either side, schoolbook multiplication is used.
const KARATSUBA_MIN: usize = 64;

/// Decimal digits of the big-endian unsigned value `be` (leading zero bytes allowed; empty is "0").
pub fn decimal(be: &[u8]) -> String {
    let first = be.iter().position(|x| *x != 0).unwrap_or(be.len());
    let s = &be[first..];
    if s.is_empty() {
        return "0".to_string();
    }
    let mut limbs = Vec::with_capacity((s.len() + 3) / 4);
    let mut end = s.len();
    while end > 0 {
        let start = end.saturating_sub(4);
        limbs.push(s[start..end].iter().fold(0u32, |a, x| (a << 8) | *x as u32));
        end = start;
    }
    // pows[k] = (2^32)^(2^k) in base 10^9, enough for every split below.
    let mut pows = vec![vec![294_967_296u32, 4]];
    while (1usize << pows.len()) < limbs.len() {
        let last = &pows[pows.len() - 1];
        let next = mul(last, last);
        pows.push(next);
    }
    render(&convert(&limbs, &pows))
}

/// Base-2^32 little-endian limbs to base-10^9 little-endian digits.
fn convert(limbs: &[u32], pows: &[Vec<u32>]) -> Vec<u32> {
    if limbs.len() <= SMALL {
        return small(limbs);
    }
    let mut k = 0;
    while (1usize << (k + 1)) < limbs.len() {
        k += 1;
    }
    let h = 1usize << k;
    let lo = convert(&limbs[..h], pows);
    let hi = convert(&limbs[h..], pows);
    let mut r = mul(&hi, &pows[k]);
    add_at(&mut r, &lo, 0);
    trim(&mut r);
    r
}

fn small(limbs: &[u32]) -> Vec<u32> {
    let mut w = limbs.to_vec();
    trim(&mut w);
    let mut out = Vec::new();
    while !w.is_empty() {
        let mut rem = 0u64;
        for x in w.iter_mut().rev() {
            let cur = (rem << 32) | *x as u64;
            *x = (cur / BASE) as u32;
            rem = cur % BASE;
        }
        out.push(rem as u32);
        trim(&mut w);
    }
    out
}

fn trim(v: &mut Vec<u32>) {
    while v.last() == Some(&0) {
        v.pop();
    }
}

/// Adds `x * 10^(9*shift)` into `r`, growing `r` as needed.
fn add_at(r: &mut Vec<u32>, x: &[u32], shift: usize) {
    if r.len() < shift + x.len() {
        r.resize(shift + x.len(), 0);
    }
    let mut carry = 0u64;
    let mut i = shift;
    for d in x {
        let t = r[i] as u64 + *d as u64 + carry;
        r[i] = (t % BASE) as u32;
        carry = t / BASE;
        i += 1;
    }
    while carry > 0 {
        if i == r.len() {
            r.push(0);
        }
        let t = r[i] as u64 + carry;
        r[i] = (t % BASE) as u32;
        carry = t / BASE;
        i += 1;
    }
}

/// r -= x, where r >= x.
fn sub_assign(r: &mut Vec<u32>, x: &[u32]) {
    let mut borrow = 0i64;
    let mut i = 0;
    while i < r.len() && (i < x.len() || borrow != 0) {
        let mut t = r[i] as i64 - borrow - x.get(i).copied().unwrap_or(0) as i64;
        borrow = 0;
        if t < 0 {
            t += BASE as i64;
            borrow = 1;
        }
        r[i] = t as u32;
        i += 1;
    }
    debug_assert!(borrow == 0, "sub_assign underflow");
    trim(r);
}

/// Rows of `a` added into u64 accumulators before carries are normalized: each product is below
/// 10^18, so a normalized slot (< 10^9) plus 16 products stays below 2^64.
const ROWS_PER_CARRY: usize = 16;

fn school(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut acc = vec![0u64; a.len() + b.len() + 1];
    let mut i0 = 0;
    while i0 < a.len() {
        let i1 = (i0 + ROWS_PER_CARRY).min(a.len());
        for (i, x) in a.iter().enumerate().take(i1).skip(i0) {
            let x = *x as u64;
            if x == 0 {
                continue;
            }
            for (slot, y) in acc[i..i + b.len()].iter_mut().zip(b) {
                *slot += x * *y as u64;
            }
        }
        let mut carry = 0u64;
        let mut k = i0;
        while k < i1 + b.len() || carry > 0 {
            let t = acc[k] + carry;
            acc[k] = t % BASE;
            carry = t / BASE;
            k += 1;
        }
        i0 = i1;
    }
    let mut r: Vec<u32> = acc.into_iter().map(|x| x as u32).collect();
    trim(&mut r);
    r
}

fn mul(a: &[u32], b: &[u32]) -> Vec<u32> {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }
    if a.len() < KARATSUBA_MIN || b.len() < KARATSUBA_MIN {
        return school(a, b);
    }
    let (a, b) = if a.len() >= b.len() { (a, b) } else { (b, a) };
    if 2 * b.len() <= a.len() {
        // Unbalanced: multiply b by slices of a of its own length and add them in place.
        let mut r = Vec::with_capacity(a.len() + b.len() + 1);
        for (idx, chunk) in a.chunks(b.len()).enumerate() {
            add_at(&mut r, &mul(trimmed(chunk), b), idx * b.len());
        }
        trim(&mut r);
        return r;
    }
    let m = a.len() / 2;
    let (a0, a1) = a.split_at(m);
    let (b0, b1) = b.split_at(m);
    let (a0, b0) = (trimmed(a0), trimmed(b0));
    let z0 = mul(a0, b0);
    let z2 = mul(a1, b1);
    let mut sa = a0.to_vec();
    add_at(&mut sa, a1, 0);
    let mut sb = b0.to_vec();
    add_at(&mut sb, b1, 0);
    let mut z1 = mul(&sa, &sb);
    sub_assign(&mut z1, &z0);
    sub_assign(&mut z1, &z2);
    let mut r = Vec::with_capacity(a.len() + b.len() + 1);
    r.extend_from_slice(&z0);
    add_at(&mut r, &z1, m);
    add_at(&mut r, &z2, 2 * m);
    trim(&mut r);
    r
}

fn trimmed(v: &[u32]) -> &[u32] {
    let n = v.iter().rposition(|x| *x != 0).map_or(0, |p| p + 1);
    &v[..n]
}

fn render(d: &[u32]) -> String {
    let mut s = String::with_capacity(d.len() * 9);
    match d.last() {
        None => s.push('0'),
        Some(top) => {
            s.push_str(&top.to_string());
            for x in d.iter().rev().skip(1) {
                s.push_str(&format!("{x:09}"));
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference: repeated division over the whole value, no splitting.
    fn slow(be: &[u8]) -> String {
        let mut limbs: Vec<u32> = Vec::new();
        let mut end = be.len();
        while end > 0 {
            let start = end.saturating_sub(4);
            limbs.push(be[start..end].iter().fold(0u32, |a, x| (a << 8) | *x as u32));
            end = start;
        }
        render(&small(&limbs))
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    #[test]
    fn known_values() {
        assert_eq!(decimal(&[]), "0");
        assert_eq!(decimal(&[0, 0]), "0");
        assert_eq!(decimal(&[1]), "1");
        assert_eq!(decimal(&[0, 2]), "2");
        assert_eq!(decimal(&[0xff; 8]), u64::MAX.to_string());
        assert_eq!(decimal(&[0xff; 16]), u128::MAX.to_string());
        assert_eq!(decimal(&[0xff; 32]), "115792089237316195423570985008687907853269984665640564039457584007913129639935");
        let mut p = vec![1u8];
        p.extend(vec![0u8; 16]);
        assert_eq!(decimal(&p), "340282366920938463463374607431768211456");
        // 10^9 boundaries inside base-10^9 digits.
        assert_eq!(decimal(&1_000_000_000u64.to_be_bytes()), "1000000000");
        assert_eq!(decimal(&999_999_999u64.to_be_bytes()), "999999999");
        assert_eq!(decimal(&1_000_000_000_000_000_000u64.to_be_bytes()), "1000000000000000000");
    }

    #[test]
    fn split_and_karatsuba_match_repeated_division() {
        let mut rng = Rng(0x4c50_3302_0001_0001);
        // Lengths straddle SMALL (64 limbs = 256 bytes), KARATSUBA_MIN and powers of two.
        let lens = [1usize, 3, 4, 5, 255, 256, 257, 260, 511, 512, 513, 1000, 1024, 1025, 2049, 4096, 6000];
        for &n in &lens {
            for pattern in 0..4 {
                let v: Vec<u8> = (0..n)
                    .map(|i| match pattern {
                        0 => rng.next() as u8,
                        1 => 0xff,
                        2 => {
                            if i == 0 {
                                1
                            } else {
                                0
                            }
                        }
                        _ => {
                            if i % 97 == 0 {
                                0
                            } else {
                                rng.next() as u8
                            }
                        }
                    })
                    .collect();
                assert_eq!(decimal(&v), slow(&v), "len {n} pattern {pattern}");
            }
        }
    }

    #[test]
    fn multiplication_matches_schoolbook() {
        let mut rng = Rng(0x4c50_3302_0001_0002);
        for (la, lb) in [(32, 32), (33, 100), (100, 33), (64, 1), (200, 199), (257, 64), (500, 500), (700, 40)] {
            let a: Vec<u32> = (0..la).map(|_| (rng.next() % BASE) as u32).collect();
            let b: Vec<u32> = (0..lb).map(|_| (rng.next() % BASE) as u32).collect();
            let mut a = a;
            let mut b = b;
            trim(&mut a);
            trim(&mut b);
            assert_eq!(mul(&a, &b), school(&a, &b), "{la}x{lb}");
        }
    }
}
