//! Local experimental transaction pool. No boot, mining, broadcast or disk write.
//! Policy: only transactions executable on the supplied snapshot; no future-nonce
//! queue or replacement. This conservative service policy is not a consensus rule.
//! Final-chain admission and stored-header binding remain P5/P6 integration gates.
//! The quota counts raw envelopes only, NOT total Rust/allocator memory. P7/M7
//! must prove the complete64MiB retained-memory bound before full acceptance.
use revm::{InMemoryDB,primitives::B256};
use crate::{profile::ProfileIdentity,native::{self,NativeBlockContext},wire::{self,SignedEnvelope,WireError}};
pub const MAX_POOL_BYTES:usize=64*1024*1024;
#[derive(Debug,PartialEq,Eq)] pub enum PoolError{Size,Wire(WireError),Full,Context,Execution(String)}
impl PoolError{pub fn rpc_code(&self)->i64{match self{Self::Size=>-32014,Self::Full=>-32016,Self::Wire(e)=>e.specified_pool_code().unwrap_or(-32000) as i64,_=>-32000}}}
pub struct Pool{profile:ProfileIdentity,budget:usize,bytes:usize,entries:Vec<SignedEnvelope>}
impl Pool{
 pub fn new(profile:ProfileIdentity)->Self{Self::with_budget(profile,MAX_POOL_BYTES)}
 /// Raw payload quota, at most64MiB; not a proof of total pool memory.
 pub fn with_budget(profile:ProfileIdentity,budget:usize)->Self{Self{profile,budget:budget.min(MAX_POOL_BYTES),bytes:0,entries:Vec::new()}}
 pub fn identity(&self)->&ProfileIdentity{&self.profile}
 pub fn pending(&self)->&[SignedEnvelope]{&self.entries}
 pub fn bytes(&self)->usize{self.bytes}
 pub fn submit(&mut self,raw:&[u8],snapshot:&InMemoryDB,block:&NativeBlockContext)->Result<B256,PoolError>{
  if raw.len()>self.profile.cp().TX_MAX as usize{return Err(PoolError::Size)}
  let signed=wire::decode_for_profile(raw,&self.profile).map_err(PoolError::Wire)?;let hash=signed.hash();
  let duplicate=self.entries.iter().any(|s|s.hash()==hash);
  if !duplicate && raw.len()>self.budget.saturating_sub(self.bytes){return Err(PoolError::Full)}
  if block.gas_limit!=revm::primitives::U256::from(self.profile.cp().GAS_LIMIT){return Err(PoolError::Context)}
  native::execute_for_profile(snapshot,signed.tx_env(),block,&self.profile).map_err(|e|PoolError::Execution(format!("{e:?}")))?;
  if duplicate{return Ok(hash)}
  self.bytes+=raw.len();self.entries.push(signed);Ok(hash)
 }
 /// Required reorg recheck: remove all old entries before validating against the
 /// new snapshot/context. Rejected entries stay removed; no inherited success.
 pub fn revalidate(&mut self,snapshot:&InMemoryDB,block:&NativeBlockContext)->Vec<(B256,PoolError)>{
  let old=std::mem::take(&mut self.entries);self.bytes=0;let mut rejected=Vec::new();
  for tx in old{if let Err(e)=self.submit(tx.raw(),snapshot,block){rejected.push((tx.hash(),e));}}rejected
 }
}
