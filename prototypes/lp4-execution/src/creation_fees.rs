//! V3-21: persisted additions pay a sender-funded, classified burn. The canonical
//! transaction diff releases removed units first, then prices each new unit at
//! the running occupancy. Reverted/temporary effects never enter that diff.
use revm::primitives::{alloy_primitives::U512,keccak256,Address,Log,U256};

#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub struct CreationBurn{pub accounts:U256,pub slots:U256,pub code:U256}
impl CreationBurn{
    pub fn total(self)->Option<U256>{self.accounts.checked_add(self.slots)?.checked_add(self.code)}
    pub fn checked_add(self,r:Self)->Option<Self>{Some(Self{accounts:self.accounts.checked_add(r.accounts)?,slots:self.slots.checked_add(r.slots)?,code:self.code.checked_add(r.code)?})}
    /// One bounded standard log, retained by the ordinary receipt/history path.
    /// ChainCaps has no EVM log instruction; USER code cannot forge this source.
    /// This is a receipt annotation, never an extra Ethereum receipt RLP field.
    pub fn log(self,sender:Address)->Option<Log>{
        if self==Self::default(){return None}
        let mut data=Vec::with_capacity(96);
        for amount in [self.accounts,self.slots,self.code]{data.extend_from_slice(&amount.to_be_bytes::<32>())}
        Log::new(crate::apply::caps(),vec![keccak256("CreationFeeBurn(address,uint256,uint256,uint256)"),sender.into_word()],data.into())
    }
}

/// Exact sum of the integer price of each unit. No floating-point consensus
/// arithmetic, rounding of an aggregate, or wrapping F0*CAP intermediate.
/// `used` is the live count after this transaction's removals, before additions.
/// Work is bounded by newly persisted state (and its existing gas/dirty caps).
pub fn price(f0:U256,cap:u64,used:u64,units:u64)->Option<U256>{
    let remaining=cap.checked_sub(used)?;
    if units>remaining{return None}
    if units==0||f0==U256::ZERO{return Some(U256::ZERO)}
    let numerator=U512::from(f0)*U512::from(cap);
    let mut sum=U512::ZERO;
    for offset in 0..units{sum=sum.checked_add(numerator/U512::from(remaining-offset))?;}
    let bytes=sum.to_be_bytes::<64>();
    if bytes[..32].iter().any(|b|*b!=0){return None}
    Some(U256::from_be_slice(&bytes[32..]))
}

#[cfg(test)]
mod tests{
 use super::*;
 #[test] fn exact_prices_count_creations_and_round_each_unit(){
  assert_eq!(price(U256::from(3),10,0,3),Some(U256::from(3+3+3)));
  assert_eq!(price(U256::from(3),10,7,3),Some(U256::from(10+15+30)));
  // Sum of individually rounded prices, not a rounded harmonic approximation.
  assert_eq!(price(U256::from(1),10,0,3),Some(U256::from(3)));
  for cap in 1..40u64{for used in 0..=cap{for units in 0..=cap-used{
   let oracle=(used..used+units).map(|n|37u128*u128::from(cap)/u128::from(cap-n)).sum::<u128>();
   assert_eq!(price(U256::from(37),cap,used,units),Some(U256::from(oracle)));
  }}}
 }
 #[test] fn full_caps_zero_coefficients_and_wide_products_do_not_wrap(){
  assert_eq!(price(U256::ZERO,5,5,1),None);
  assert_eq!(price(U256::MAX,5,5,0),Some(U256::ZERO));
  assert_eq!(price(U256::ZERO,5,4,1),Some(U256::ZERO));
  assert_eq!(price(U256::MAX,u64::MAX,0,1),Some(U256::MAX));
  assert_eq!(price(U256::MAX,u64::MAX,0,2),None);
  assert_eq!(price(U256::MAX,u64::MAX,u64::MAX-1,1),None);
  assert_eq!(price(U256::from(1),2,3,0),None);
 }
 #[test] fn account_fill_cost_is_harmonic_and_last_unit_costs_f0_times_cap(){
  let cap=200_000u64;let f0=1_000_000_000_000_000u64;
  let exact=price(U256::from(f0),cap,0,cap).unwrap();
  let approx=f0 as f64*cap as f64*((cap as f64).ln()+0.58);
  let measured=exact.to_string().parse::<f64>().unwrap();
  assert!((measured-approx).abs()/approx<0.01);
  assert_eq!(price(U256::from(f0),cap,cap-1,1),Some(U256::from(f0)*U256::from(cap)));
  assert!(exact>U256::from(f0)*U256::from(cap)*U256::from(10));
 }
}
