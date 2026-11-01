//! Fixed-width unsigned integers: `U256` (4 x u64) and `U512` (8 x u64), little-endian limbs.
//! No heap allocation in any arithmetic used by ASERT. Only the operations LP1 needs.

use std::cmp::Ordering;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct U256(pub [u64; 4]);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct U512(pub [u64; 8]);

impl U256 {
    pub const ZERO: U256 = U256([0; 4]);
    pub const ONE: U256 = U256([1, 0, 0, 0]);
    pub const MAX: U256 = U256([u64::MAX; 4]);

    pub fn from_u64(v: u64) -> U256 {
        U256([v, 0, 0, 0])
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0; 4]
    }

    /// Big-endian bytes, at most 32 (leading zeros allowed here; callers enforce minimality).
    pub fn from_be_slice(b: &[u8]) -> Option<U256> {
        if b.len() > 32 {
            return None;
        }
        let mut out = [0u64; 4];
        for (i, byte) in b.iter().rev().enumerate() {
            out[i / 8] |= (*byte as u64) << (8 * (i % 8));
        }
        Some(U256(out))
    }

    pub fn to_be_bytes(&self) -> [u8; 32] {
        let mut b = [0u8; 32];
        for i in 0..32 {
            b[31 - i] = (self.0[i / 8] >> (8 * (i % 8))) as u8;
        }
        b
    }

    /// Minimal big-endian encoding (zero is empty), as RLP integers require.
    pub fn to_min_be(&self) -> Vec<u8> {
        let b = self.to_be_bytes();
        let first = b.iter().position(|x| *x != 0).unwrap_or(32);
        b[first..].to_vec()
    }

    pub fn bits(&self) -> u32 {
        for i in (0..4).rev() {
            if self.0[i] != 0 {
                return 64 * i as u32 + 64 - self.0[i].leading_zeros();
            }
        }
        0
    }

    /// 2^n for n < 256.
    pub fn pow2(n: u32) -> Option<U256> {
        if n >= 256 {
            return None;
        }
        let mut l = [0u64; 4];
        l[(n / 64) as usize] = 1u64 << (n % 64);
        Some(U256(l))
    }

    /// Canonical decimal (no sign, no leading zero except "0").
    pub fn from_dec_str(s: &str) -> Option<U256> {
        if s.is_empty() || s.len() > 78 || !s.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        if s.len() > 1 && s.starts_with('0') {
            return None;
        }
        let mut acc = U512::ZERO;
        for c in s.bytes() {
            acc = acc.mul_small(10)?.add_small((c - b'0') as u64)?;
        }
        acc.to_u256()
    }

    /// Accepts a canonical decimal string or the exact form "2^N" used by fixture profiles.
    pub fn from_profile_str(s: &str) -> Option<U256> {
        if let Some(rest) = s.strip_prefix("2^") {
            if rest.is_empty() || !rest.bytes().all(|c| c.is_ascii_digit()) || rest.len() > 3 {
                return None;
            }
            if rest.len() > 1 && rest.starts_with('0') {
                return None;
            }
            let n: u32 = rest.parse().ok()?;
            return U256::pow2(n);
        }
        U256::from_dec_str(s)
    }

    /// Divide by a small non-zero divisor; returns (quotient, remainder).
    pub fn div_rem_small(&self, d: u64) -> (U256, u64) {
        let mut out = [0u64; 4];
        let mut rem: u128 = 0;
        for i in (0..4).rev() {
            let cur = (rem << 64) | self.0[i] as u128;
            out[i] = (cur / d as u128) as u64;
            rem = cur % d as u128;
        }
        (U256(out), rem as u64)
    }

    pub fn to_dec_string(&self) -> String {
        if self.is_zero() {
            return "0".to_string();
        }
        let mut digits = Vec::new();
        let mut v = *self;
        while !v.is_zero() {
            let (q, r) = v.div_rem_small(10);
            digits.push(b'0' + r as u8);
            v = q;
        }
        digits.reverse();
        String::from_utf8(digits).unwrap_or_default()
    }

    pub fn checked_sub(&self, o: &U256) -> Option<U256> {
        let mut out = [0u64; 4];
        let mut borrow = 0u64;
        for i in 0..4 {
            let (a, b1) = self.0[i].overflowing_sub(o.0[i]);
            let (a, b2) = a.overflowing_sub(borrow);
            out[i] = a;
            borrow = (b1 || b2) as u64;
        }
        if borrow != 0 {
            None
        } else {
            Some(U256(out))
        }
    }

    pub fn shr1(&self) -> U256 {
        let mut out = [0u64; 4];
        for i in 0..4 {
            out[i] = self.0[i] >> 1;
            if i + 1 < 4 {
                out[i] |= self.0[i + 1] << 63;
            }
        }
        U256(out)
    }

    /// Low 64 bits if the value fits, else None.
    pub fn to_u64(&self) -> Option<u64> {
        if self.0[1] == 0 && self.0[2] == 0 && self.0[3] == 0 {
            Some(self.0[0])
        } else {
            None
        }
    }
}

impl Ord for U256 {
    fn cmp(&self, o: &U256) -> Ordering {
        for i in (0..4).rev() {
            match self.0[i].cmp(&o.0[i]) {
                Ordering::Equal => continue,
                x => return x,
            }
        }
        Ordering::Equal
    }
}

impl PartialOrd for U256 {
    fn partial_cmp(&self, o: &U256) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl U512 {
    pub const ZERO: U512 = U512([0; 8]);

    pub fn from_u256(a: &U256) -> U512 {
        let mut l = [0u64; 8];
        l[..4].copy_from_slice(&a.0);
        U512(l)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0; 8]
    }

    /// a * m for a < 2^256 and m < 2^64: always fits in 320 bits.
    pub fn mul_u256_u64(a: &U256, m: u64) -> U512 {
        let mut out = [0u64; 8];
        let mut carry: u128 = 0;
        for i in 0..4 {
            let cur = a.0[i] as u128 * m as u128 + carry;
            out[i] = cur as u64;
            carry = cur >> 64;
        }
        out[4] = carry as u64;
        U512(out)
    }

    pub fn mul_small(&self, m: u64) -> Option<U512> {
        let mut out = [0u64; 8];
        let mut carry: u128 = 0;
        for i in 0..8 {
            let cur = self.0[i] as u128 * m as u128 + carry;
            out[i] = cur as u64;
            carry = cur >> 64;
        }
        if carry != 0 {
            None
        } else {
            Some(U512(out))
        }
    }

    pub fn add_small(&self, v: u64) -> Option<U512> {
        let mut out = self.0;
        let mut carry = v;
        for limb in out.iter_mut() {
            let (s, c) = limb.overflowing_add(carry);
            *limb = s;
            carry = c as u64;
            if carry == 0 {
                break;
            }
        }
        if carry != 0 {
            None
        } else {
            Some(U512(out))
        }
    }

    pub fn bits(&self) -> u32 {
        for i in (0..8).rev() {
            if self.0[i] != 0 {
                return 64 * i as u32 + 64 - self.0[i].leading_zeros();
            }
        }
        0
    }

    pub fn bit(&self, i: u32) -> bool {
        (self.0[(i / 64) as usize] >> (i % 64)) & 1 == 1
    }

    /// Shift left by k < 512; bits above 2^512 are discarded (callers guarantee none exist).
    pub fn shl(&self, k: u32) -> U512 {
        if k >= 512 {
            return U512::ZERO;
        }
        let limb = (k / 64) as usize;
        let bit = k % 64;
        let mut out = [0u64; 8];
        for i in limb..8 {
            let src = i - limb;
            let mut v = self.0[src] << bit;
            if bit > 0 && src > 0 {
                v |= self.0[src - 1] >> (64 - bit);
            }
            out[i] = v;
        }
        U512(out)
    }

    pub fn shr(&self, k: u32) -> U512 {
        if k >= 512 {
            return U512::ZERO;
        }
        let limb = (k / 64) as usize;
        let bit = k % 64;
        let mut out = [0u64; 8];
        for i in 0..(8 - limb) {
            let src = i + limb;
            let mut v = self.0[src] >> bit;
            if bit > 0 && src + 1 < 8 {
                v |= self.0[src + 1] << (64 - bit);
            }
            out[i] = v;
        }
        U512(out)
    }

    fn sub(&self, o: &U512) -> U512 {
        let mut out = [0u64; 8];
        let mut borrow = 0u64;
        for i in 0..8 {
            let (a, b1) = self.0[i].overflowing_sub(o.0[i]);
            let (a, b2) = a.overflowing_sub(borrow);
            out[i] = a;
            borrow = (b1 || b2) as u64;
        }
        U512(out)
    }

    /// Long division for divisors below 2^511 (LP1 divides at most 2^256 by at most 2^256).
    pub fn div_rem(&self, d: &U512) -> Option<(U512, U512)> {
        if d.is_zero() || d.bits() >= 512 {
            return None;
        }
        let mut q = U512::ZERO;
        let mut r = U512::ZERO;
        for i in (0..512u32).rev() {
            r = r.shl(1);
            if self.bit(i) {
                r.0[0] |= 1;
            }
            if r >= *d {
                r = r.sub(d);
                q.0[(i / 64) as usize] |= 1u64 << (i % 64);
            }
        }
        Some((q, r))
    }

    pub fn to_u256(&self) -> Option<U256> {
        if self.0[4..].iter().any(|x| *x != 0) {
            return None;
        }
        let mut l = [0u64; 4];
        l.copy_from_slice(&self.0[..4]);
        Some(U256(l))
    }

    /// min(self, 2^256 - 1).
    pub fn clamp_u256(&self) -> U256 {
        self.to_u256().unwrap_or(U256::MAX)
    }
}

impl Ord for U512 {
    fn cmp(&self, o: &U512) -> Ordering {
        for i in (0..8).rev() {
            match self.0[i].cmp(&o.0[i]) {
                Ordering::Equal => continue,
                x => return x,
            }
        }
        Ordering::Equal
    }
}

impl PartialOrd for U512 {
    fn partial_cmp(&self, o: &U512) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// floor(2^256 / (t + 1)) as used by work(t) and minWork(ceil) (consensus.md, browser.md:51).
pub fn work(t: &U256) -> U256 {
    let mut num = U512::ZERO;
    num.0[4] = 1;
    let d = U512::from_u256(t).add_small(1).unwrap_or(U512::ZERO);
    match num.div_rem(&d) {
        Some((q, _)) => q.clamp_u256(),
        None => U256::ZERO,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_round_trip_and_bounds() {
        let max = "115792089237316195423570985008687907853269984665640564039457584007913129639935";
        assert_eq!(U256::from_dec_str(max), Some(U256::MAX));
        assert_eq!(U256::MAX.to_dec_string(), max);
        assert_eq!(U256::from_dec_str("115792089237316195423570985008687907853269984665640564039457584007913129639936"), None);
        assert_eq!(U256::from_dec_str("01"), None);
        assert_eq!(U256::from_dec_str("-1"), None);
        assert_eq!(U256::from_dec_str("0"), Some(U256::ZERO));
        assert_eq!(U256::from_profile_str("2^240"), U256::pow2(240));
        assert_eq!(U256::from_profile_str("2^256"), None);
        assert_eq!(U256::from_profile_str("2^1.5"), None);
    }

    #[test]
    fn shifts_and_bits() {
        let one = U512::from_u256(&U256::ONE);
        assert_eq!(one.shl(511).bits(), 512);
        assert_eq!(one.shl(511).shr(511), one);
        assert_eq!(one.shl(512), U512::ZERO);
        let x = U512::mul_u256_u64(&U256::MAX, 131071);
        assert_eq!(x.bits(), 273);
        assert_eq!(x.shl(239).bits(), 512);
        assert_eq!(x.shr(300), U512::ZERO);
    }

    #[test]
    fn work_values() {
        // ceil = 2^244 gives minWork 4095 (browser.md:51 example used by the fixtures).
        let ceil = U256::pow2(244).unwrap();
        assert_eq!(work(&ceil), U256::from_u64(4095));
        assert_eq!(work(&U256::ZERO), U256::MAX); // 2^256 / 1 clamps to 2^256 - 1
        assert_eq!(work(&U256::MAX), U256::ONE);
    }

    #[test]
    fn byte_round_trip() {
        let v = U256::from_dec_str("1606938044258990275541962092341162602522202993782792835301376").unwrap();
        assert_eq!(v, U256::pow2(200).unwrap());
        assert_eq!(U256::from_be_slice(&v.to_be_bytes()).unwrap(), v);
        assert_eq!(v.to_min_be().len(), 26);
        assert_eq!(U256::ZERO.to_min_be(), Vec::<u8>::new());
    }
}
