//! London arithmetic; caller must bind inputs to validated parent/header (P5).
use revm::primitives::{U256,alloy_primitives::U512};
#[derive(Debug,Clone,Copy,PartialEq,Eq)] pub enum FeeError{ParentGas,Overflow,FeeCap}
pub fn next_base_fee(parent:U256,gas_used:u64,gas_limit:u64)->Result<U256,FeeError>{
 let target=gas_limit/2;if target==0 || gas_used>gas_limit{return Err(FeeError::ParentGas)}
 if gas_used==target{return Ok(parent)}
 let distance=gas_used.abs_diff(target);let delta=U512::from(parent)*U512::from(distance)/U512::from(target)/U512::from(8);
 let fee=if gas_used>target{U512::from(parent)+delta.max(U512::from(1))}else{U512::from(parent)-delta};
 let bytes=fee.to_be_bytes::<64>();if bytes[..32].iter().any(|b|*b!=0){return Err(FeeError::Overflow)}
 Ok(U256::from_be_slice(&bytes[32..]))
}
pub fn effective_fees(base:U256,max_fee:U256,priority:Option<U256>)->Result<(U256,U256),FeeError>{
 if max_fee<base || priority.map(|p|p>max_fee).unwrap_or(false){return Err(FeeError::FeeCap)}
 let tip=priority.map(|p|p.min(max_fee-base)).unwrap_or(max_fee-base);Ok((base+tip,tip))
}
