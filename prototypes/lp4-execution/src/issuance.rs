//! V3-19 issuance (O-19 = M3; V3-DESIGN.md V3-19, V3-DECISIONS.md D-V3-19), for the v3 program set only. The spec-1
//! program sets keep crediting CP 15 every block (apply.rs phase 2).
//!
//! ```text
//! issuance(h) = h == 0 ? 0 : max(k >= 256 ? 0 : S0 >> k, S_tail),   k = floor(h / H_half)
//! ```
//!
//! S0 is CP 15 (the M1 subsidy field), H_half and S_tail are CP schema v2 fields 52 and 53. issuance(h) depends only on h
//! and the CP, so it is known before block h is mined (P6). The host credits it to Escrow and passes it to
//! closeBlock(tips, issued); the program does not recompute it.
//!
//! Supply bound: over every height h in 1..2^64 the sum is at most 2·S0·H_half + S_tail·2^64, because
//! max(a, b) <= a + b, era k (H_half heights, fewer for k = 0) issues at most H_half·S0/2^k from the S0 term, and the eras
//! sum to less than 2·S0·H_half; the tail adds at most S_tail per height. `future_bound` is that number; genesis_builder
//! and ParamGate R23 add it to the allocations and the genesis bonds and require the total below 2^127.
use revm::primitives::U256;

/// Halvings after which the S0 term is zero (a U256 shifted by 256 or more is zero).
pub const HALVINGS:u64=256;

/// issuance(h) for S0, H_half and S_tail. H_half = 0 cannot be decoded (genesis_spec.rs: H_half >= 1 on CP schema v2);
/// here it reads as no S0 term, so the function never divides by zero.
pub fn issuance(h:u64,s0:U256,h_half:u64,s_tail:U256)->U256{
    if h==0{return U256::ZERO}
    let k=h.checked_div(h_half).unwrap_or(u64::MAX);
    let halved=if k>=HALVINGS{U256::ZERO}else{s0>>(k as usize)};
    halved.max(s_tail)
}
/// issuance(h) at a CP schema v2 profile (S0 = CP 15, H_half = CP 52, S_tail = CP 53).
pub fn for_cp(cp:&crate::profile::ConsensusParams,h:u64)->U256{
    issuance(h,crate::devnet::from_fixed(&cp.subsidy),cp.H_half,crate::devnet::from_fixed(&cp.S_tail))
}
/// The supply bound's issuance term 2·S0·H_half + S_tail·2^64 (module doc); None on U256 overflow.
pub fn future_bound(s0:U256,h_half:u64,s_tail:U256)->Option<U256>{
    let halving=s0.checked_mul(U256::from(h_half))?.checked_mul(U256::from(2))?;
    let tail=s_tail.checked_mul(U256::from(1)<<64)?;
    halving.checked_add(tail)
}
/// The sum of issuance(h) for h in 1..=n, in closed form per era (for the tests and the supply bound's check).
pub fn cumulative(n:u64,s0:U256,h_half:u64,s_tail:U256)->U256{
    if n==0||h_half==0{return s_tail*U256::from(n)}
    let mut total=U256::ZERO;let mut k=0u64;
    loop{
        // Heights of era k within 1..=n: [max(1, k*H_half), min(n, (k+1)*H_half - 1)].
        let lo=k.saturating_mul(h_half).max(1);
        if lo>n{break}
        let hi=n.min(k.saturating_add(1).saturating_mul(h_half).saturating_sub(1));
        if hi>=lo{total+=issuance(lo,s0,h_half,s_tail)*U256::from(hi-lo+1);}
        if hi==n||hi==u64::MAX{break}
        k+=1;
        if k>=HALVINGS{ total+=s_tail*U256::from(n-hi); break }
    }
    total
}

#[cfg(test)]
mod tests{
    use super::*;
    const PCOL:u128=1_000_000_000_000_000_000;
    fn s0()->U256{U256::from(PCOL)}
    fn tail()->U256{s0()/U256::from(32)}
    const H:u64=4_207_680;
    /// The owner's values (V3-ROADMAP §5): S0 = 1 PCOL, H_half = 4,207,680 blocks (4 years at 30 s), S_tail = S0/32.
    #[test] fn vectors_at_the_era_boundaries(){
        let i=|h|issuance(h,s0(),H,tail());
        assert_eq!(i(0),U256::ZERO);
        assert_eq!(i(1),s0());
        assert_eq!(i(H-1),s0());
        assert_eq!(i(H),s0()/U256::from(2));
        assert_eq!(i(2*H-1),s0()/U256::from(2));
        assert_eq!(i(2*H),s0()/U256::from(4));
        assert_eq!(i(5*H-1),s0()/U256::from(16));
        assert_eq!(i(5*H),tail(),"S0/32 = the tail at era 5");
        assert_eq!(i(6*H),tail(),"the tail floors every later era");
        assert_eq!(i(255*H),tail());
        assert_eq!(i(256*H),tail(),"k >= 256: the S0 term is zero");
        assert_eq!(i(u64::MAX),tail());
        // Without a tail the S0 term itself: zero from k = 256 on (S0 < 2^256), and S0 >> k below it.
        let z=|h|issuance(h,s0(),H,U256::ZERO);
        assert_eq!(z(60*H),U256::ZERO,"1 PCOL < 2^60");
        assert_eq!(z(59*H),s0()>>59usize);
        assert_eq!(issuance(256*H,U256::MAX,H,U256::ZERO),U256::ZERO);
        assert_eq!(issuance(256*H-1,U256::MAX,H,U256::ZERO),U256::from(1),"U256::MAX >> 255");
        assert_eq!(issuance(7,s0(),0,tail()),tail(),"H_half = 0 (undecodable) never divides by zero");
    }
    #[test] fn the_supply_bound_covers_every_height(){
        // The closed-form sum over all of 1..2^64-1 is below the bound, and the bound is below 2^127 at the owner's values.
        let all=cumulative(u64::MAX,s0(),H,tail());
        let bound=future_bound(s0(),H,tail()).unwrap();
        assert!(all<=bound,"{all} > {bound}");
        assert!(bound<U256::from(1)<<127);
        // Compressed fixture: the closed form equals the per-height sum.
        for (s,hh,t) in [(U256::from(1000),4u64,U256::from(31)),(U256::from(1u64<<20),3,U256::ZERO),(U256::from(7),1,U256::from(1))]{
            for n in 0..400u64{
                let direct=(1..=n).map(|h|issuance(h,s,hh,t)).fold(U256::ZERO,|a,b|a+b);
                assert_eq!(cumulative(n,s,hh,t),direct,"n={n}");
                assert!(direct<=future_bound(s,hh,t).unwrap());
            }
        }
        assert_eq!(future_bound(U256::MAX,2,U256::ZERO),None);
        assert_eq!(future_bound(U256::ZERO,1,U256::MAX),None);
    }
}
