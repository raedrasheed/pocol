//! London arithmetic; caller must bind inputs to validated parent/header (P5).
use revm::primitives::{U256,alloy_primitives::U512};
#[derive(Debug,Clone,Copy,PartialEq,Eq)] pub enum FeeError{ParentGas,Overflow,FeeCap}
pub fn next_base_fee(parent:U256,gas_used:u64,gas_limit:u64,floor:U256)->Result<U256,FeeError>{
 let target=gas_limit/2;if target==0 || gas_used>gas_limit{return Err(FeeError::ParentGas)}
 if gas_used==target{return Ok(parent.max(floor))}
 let distance=gas_used.abs_diff(target);let delta=U512::from(parent)*U512::from(distance)/U512::from(target)/U512::from(8);
 let fee=if gas_used>target{U512::from(parent)+delta.max(U512::from(1))}else{U512::from(parent)-delta};
 let bytes=fee.to_be_bytes::<64>();if bytes[..32].iter().any(|b|*b!=0){return Err(FeeError::Overflow)}
 Ok(U256::from_be_slice(&bytes[32..]).max(floor))
}
pub fn effective_fees(base:U256,max_fee:U256,priority:Option<U256>)->Result<(U256,U256),FeeError>{
 if max_fee<base || priority.map(|p|p>max_fee).unwrap_or(false){return Err(FeeError::FeeCap)}
 let tip=priority.map(|p|p.min(max_fee-base)).unwrap_or(max_fee-base);Ok((base+tip,tip))
}

#[cfg(test)]
mod floor_tests{
 use super::*;
 #[test] fn a_thousand_empty_blocks_reach_the_v3_floor_while_london_decays(){
  let floor=U256::from(10_000_000);let mut v3=U256::from(1_000_000_000);let mut v1=v3;
  for _ in 0..1000{v3=next_base_fee(v3,0,8_000_000,floor).unwrap();v1=next_base_fee(v1,0,8_000_000,U256::ZERO).unwrap();assert!(v3>=floor)}
  assert_eq!(v3,floor);assert_eq!(v1,U256::from(7));
  assert_eq!(next_base_fee(U256::from(1),4_000_000,8_000_000,floor),Ok(floor));
 }
 #[test] fn a_floor_does_not_mask_invalid_parent_gas_or_london_overflow(){
  assert_eq!(next_base_fee(U256::MAX,8_000_000,8_000_000,U256::MAX),Err(FeeError::Overflow));
  assert_eq!(next_base_fee(U256::ZERO,8_000_001,8_000_000,U256::MAX),Err(FeeError::ParentGas));
  assert_eq!(next_base_fee(U256::ZERO,0,1,U256::MAX),Err(FeeError::ParentGas));
 }
}
