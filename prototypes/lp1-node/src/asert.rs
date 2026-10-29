//! Bounded ASERT (consensus.md:126-137). Checked i128 for dt / e / s / f with floor semantics,
//! u128 for the cubic, fixed [u64; 8] for X and Y. No heap allocation on any path.

use crate::fixed::{U256, U512};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Early {
    No,
    Low,
    High,
}

impl Early {
    pub fn as_json(&self) -> &'static str {
        match self {
            Early::No => "null",
            Early::Low => "\"low\"",
            Early::High => "\"high\"",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AsertTrace {
    pub delta_t: i128,
    pub e: i128,
    pub s: i128,
    pub f: u16,
    /// Absent on the early saturation paths: no polynomial or multiply was run.
    pub factor: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AsertOut {
    pub trace: AsertTrace,
    pub target: U256,
    pub early: Early,
    /// Native oracle metric: max(bits(X), bits(Y)); 0 on the early exits.
    pub shift_bits: u32,
    /// v1_ref_027 metric: max bit length over |num|, |e| and (non-early) poly, X, Y.
    pub max_bits: u32,
}

/// |dt| bound of v1_ref_027.asert (beyond it the reference returns no target).
pub const DT_LIMIT_BITS: u32 = 97;

fn bits_i128(v: i128) -> u32 {
    128 - v.unsigned_abs().leading_zeros()
}

/// Parent-based ASERT. All arguments are exact integers; tau must be >= 1.
/// Returns None for tau < 1, |dt| >= 2^97 or any checked-arithmetic overflow.
pub fn asert(target_g: &U256, g_ts: i128, t_blk: i128, tau: i128, p_ts: i128, p_h: i128) -> Option<AsertOut> {
    if tau < 1 {
        return None;
    }
    let dt = p_ts.checked_sub(g_ts)?.checked_sub(t_blk.checked_mul(p_h)?)?;
    if dt.unsigned_abs() >= 1u128 << DT_LIMIT_BITS {
        return None;
    }
    let num = dt.checked_mul(65536)?;
    let e = num.div_euclid(tau); // tau > 0: Euclidean division is floor division
    let s = e.div_euclid(65536);
    let f = e - 65536 * s; // 0 <= f < 65536
    let mut trace = AsertTrace { delta_t: dt, e, s, f: f as u16, factor: None };
    let mut max_bits = bits_i128(num).max(bits_i128(e));
    if s >= 256 {
        return Some(AsertOut { trace, target: U256::MAX, early: Early::High, shift_bits: 0, max_bits });
    }
    if s <= -257 {
        return Some(AsertOut { trace, target: U256::ONE, early: Early::Low, shift_bits: 0, max_bits });
    }
    let fu = f as u128;
    // < 2^65 for f < 2^16: no overflow in u128.
    let poly = 195_766_423_245_049u128 * fu + 971_821_376u128 * fu * fu + 5127u128 * fu * fu * fu + (1u128 << 47);
    let big_f = 65536u64 + (poly >> 48) as u64;
    trace.factor = Some(big_f);
    let x = U512::mul_u256_u64(target_g, big_f);
    let xb = x.bits();
    let k = (s - 16) as i32; // -273 ..= 239
    let yb = if xb == 0 {
        0
    } else if k >= 0 {
        xb + k as u32
    } else {
        xb.saturating_sub((-k) as u32)
    };
    let poly_bits = 128 - poly.leading_zeros();
    max_bits = max_bits.max(poly_bits).max(xb).max(yb);
    let shift_bits = xb.max(yb.max(1));
    let target = if yb == 0 {
        U256::ONE
    } else if yb > 256 {
        U256::MAX
    } else {
        let y = if k >= 0 { x.shl(k as u32) } else { x.shr((-k) as u32) };
        y.clamp_u256()
    };
    Some(AsertOut { trace, target, early: Early::No, shift_bits, max_bits })
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: i128 = 1_700_000_000;

    fn run(tg: &U256, dt: i128) -> AsertOut {
        asert(tg, G, 10, 600, G + 200 + dt, 20).unwrap()
    }

    #[test]
    fn named_boundaries() {
        let tg = U256::pow2(240).unwrap();
        // dt = 0: s = 0, f = 0, F = 65536 + floor(2^47 / 2^48) = 65536, Y = tg * 2^16 >> 16 = tg.
        let o = run(&tg, 0);
        assert_eq!(o.target, tg);
        assert_eq!(o.early, Early::No);
        assert_eq!(o.max_bits, 257); // bits(X) = 240 + 17
                                     // dt = -1: e = floor(-65536/600) = -110, s = -1, f = 65426.
        let o = run(&tg, -1);
        assert!(o.target < tg);
        // s = 255 (dt = 255*600) stays in the shift path; s = 256 exits high.
        let o = run(&tg, 255 * 600);
        assert_eq!(o.early, Early::No);
        assert_eq!(o.target, U256::MAX);
        let o = run(&tg, 256 * 600);
        assert_eq!((o.early, o.target, o.shift_bits), (Early::High, U256::MAX, 0));
        // s = -256 stays in the shift path; s = -257 exits low.
        let o = run(&tg, -256 * 600);
        assert_eq!(o.early, Early::No);
        assert_eq!(o.target, U256::ONE);
        let o = run(&tg, -257 * 600);
        assert_eq!((o.early, o.target, o.shift_bits), (Early::Low, U256::ONE, 0));
        // Largest shift with the largest target_g reaches the 512-bit width exactly at most.
        let o = run(&U256::MAX, 255 * 600 + 599);
        assert!(o.shift_bits <= 512);
        assert_eq!(o.target, U256::MAX);
        // One ulp of dt below an s boundary floors toward -inf.
        let o = asert(&tg, G, 10, 3, G + 200 - 1, 20).unwrap();
        assert_eq!(o.early, Early::No);
    }

    #[test]
    fn rejection_cases() {
        let tg = U256::ONE;
        assert_eq!(asert(&tg, G, 10, 0, G, 0), None);
        assert_eq!(asert(&tg, G, 10, -5, G, 0), None);
        assert_eq!(asert(&tg, 0, 0, 600, 1i128 << 97, 0), None);
        assert!(asert(&tg, 0, 0, 600, (1i128 << 97) - 1, 0).is_some());
        assert_eq!(asert(&tg, 0, i128::MAX, 600, 0, 2), None);
    }

    #[test]
    fn mutation_sensitivity() {
        // Changing any input by one unit around the default parent changes the result or branch.
        let tg = U256::pow2(240).unwrap();
        let base = asert(&tg, G, 10, 600, G + 5000, 20).unwrap();
        assert_ne!(asert(&tg, G, 10, 600, G + 5600, 20).unwrap().target, base.target);
        assert_ne!(asert(&tg, G, 10, 600, G + 5000, 80).unwrap().target, base.target);
        assert_ne!(asert(&tg, G, 10, 300, G + 5000, 20).unwrap().target, base.target);
        assert_ne!(asert(&U256::pow2(241).unwrap(), G, 10, 600, G + 5000, 20).unwrap().target, base.target);
    }
}
