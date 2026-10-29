//! Actual MPT genesis construction and R24/R23/CODE/PAY-02 checks.
//! Not complete ParamGate, five-program semantic acceptance, or boot authority.
use revm::{InMemoryDB,primitives::{Address,Bytecode,Bytes,B256,U256,keccak256}};
use crate::{profile::ProfileIdentity,pay,code_caps::{self,CodeLimits}};
#[derive(Debug,PartialEq,Eq)]pub(crate) enum GenesisError{Profile,Code,Supply,Capacity,RewardNotDurable,Root}
/// Constructed only after actual complete genesis root/code/CP checks. No success flag.
pub(crate) struct GenesisImage{state:InMemoryDB,profile:B256,codes:[B256;5],root:B256,alloc_sum:U256,bonds:U256,supply_bound:U256}
impl GenesisImage{
 pub(crate) fn state(&self)->&InMemoryDB{&self.state}
 pub(crate) fn root(&self)->B256{self.root}
 pub(crate) fn initial_supply(&self)->U256{self.alloc_sum+self.bonds}
 /// C-1 E2: `verify_candidate` with the whole-state counts read from `s`, the summary of `db`
 /// (state_summary): the same outcome. The fixed accounts are read from `db` as before.
 pub(crate) fn verify_summary(&self,db:&dyn crate::view::View,s:&crate::state_summary::Summary,p:&ProfileIdentity)->Result<(),GenesisError>{
  if B256::from(p.genesis_hash())!=self.profile{return Err(GenesisError::Profile)}
  s.code_check(CodeLimits{runtime:p.cp().B_code_max,logical:p.cp().CODE_CAP}).map_err(|_|GenesisError::Code)?;
  if s.user_accounts>p.cp().ACCT_CAP||s.user_slots>p.cp().SLOT_CAP{return Err(GenesisError::Capacity)}
  if s.sys_slots>u64::from(p.cp().SYS_SLOT_MAX){return Err(GenesisError::Capacity)}
  if s.balance>revm::primitives::alloy_primitives::U512::from(self.supply_bound){return Err(GenesisError::Supply)}
  self.fixed_accounts(db,p)
 }
 fn fixed_accounts(&self,db:&dyn crate::view::View,p:&ProfileIdentity)->Result<(),GenesisError>{
  if let Some(a)=db.account(&crate::pay_address()).and_then(|a|a.info()){if a.code_hash!=revm::primitives::KECCAK_EMPTY{return Err(GenesisError::Code)}}
  for (i,hash) in self.codes.iter().enumerate(){let a=db.account(&system(i)).and_then(|a|a.info()).ok_or(GenesisError::Code)?;if a.code_hash!=*hash{return Err(GenesisError::Code)}}
  for (i,value) in p.spec().cp.iter().enumerate(){db.account(&system(3)).ok_or(GenesisError::Code)?;if db.slot(&system(3),&U256::from(i))!=number(*value){return Err(GenesisError::Profile)}}
  Ok(())
 }
 pub(crate) fn verify_candidate(&self,db:&InMemoryDB,p:&ProfileIdentity)->Result<(),GenesisError>{
  if B256::from(p.genesis_hash())!=self.profile{return Err(GenesisError::Profile)}
  check_caps(db,p)?;
  // System storage is bounded by SYS_SLOT_MAX (R18, D-A03-CAPS); only SYSTEM calls write
  // it, so exceeding it cannot be a transaction halt and invalidates the candidate. The
  // admission gate derives SYS_SLOT_MAX >= every reachable footprint (resources::derive).
  let sys:u64=(0..5).filter_map(|i|db.accounts.get(&system(i))).map(|a|a.storage.values().filter(|v|**v!=U256::ZERO).count() as u64).sum();
  if sys>u64::from(p.cp().SYS_SLOT_MAX){return Err(GenesisError::Capacity)}
  if sum_balances(db)?>self.supply_bound{return Err(GenesisError::Supply)}
  self.fixed_accounts(db,p)
 }
}
pub(crate) fn system(i:usize)->Address{Address::from_word(U256::from(0xc0c001u64+i as u64).into())}
fn number(n:crate::fixed::U256)->U256{U256::from_be_bytes(n.to_be_bytes())}
fn add(a:U256,b:U256)->Result<U256,GenesisError>{a.checked_add(b).ok_or(GenesisError::Supply)}
fn sum_balances(db:&InMemoryDB)->Result<U256,GenesisError>{db.accounts.values().filter_map(|a|a.info()).try_fold(U256::ZERO,|n,a|add(n,a.balance))}
fn put(db:&mut InMemoryDB,a:Address,k:U256,v:U256){db.insert_account_storage(a,k,v).expect("EmptyDB is infallible");}
fn base(slot:u64)->U256{U256::from_be_bytes(keccak256(U256::from(slot).to_be_bytes::<32>()).0)}
fn record(a:Address)->U256{U256::from_be_bytes(keccak256([a.into_word().as_slice(),U256::from(7).to_be_bytes::<32>().as_slice()].concat()).0)}
pub(crate) fn code_hash(programs:&[Bytes;5])->B256{
 let mut payload=Vec::new();for p in programs{crate::rlp::encode_bytes(p,&mut payload)}crate::rlp::encode_bytes(&[],&mut payload);
 let mut encoded=Vec::new();crate::rlp::encode_list_payload(&payload,&mut encoded);keccak256(encoded)
}
fn check_caps(db:&InMemoryDB,p:&ProfileIdentity)->Result<(),GenesisError>{
 code_caps::validate_snapshot(db,CodeLimits{runtime:p.cp().B_code_max,logical:p.cp().CODE_CAP}).map_err(|_|GenesisError::Code)?;
 // D-A03-CAPS: SLOT_CAP/ACCT_CAP bound the user state; the five system programs are
 // bounded by SYS_SLOT_MAX (R18). USER transactions halt before exceeding a cap (A03).
 let mut accounts=0u64;let mut slots=0u64;
 for a in db.accounts.iter().filter(|(k,a)|a.info().is_some()&&!(0..5).any(|i|**k==system(i))).map(|(_,a)|a){
  accounts=accounts.checked_add(1).ok_or(GenesisError::Capacity)?;
  slots=slots.checked_add(u64::try_from(a.storage.values().filter(|v|**v!=U256::ZERO).count()).map_err(|_|GenesisError::Capacity)?).ok_or(GenesisError::Capacity)?;
 }
 if accounts>p.cp().ACCT_CAP || slots>p.cp().SLOT_CAP{return Err(GenesisError::Capacity)}Ok(())
}
/// Pure owned construction used by the verifier; not an admitted profile or root.
/// Existing alloc nonce/balance/storage are preserved except mandated writes.
fn construct(alloc:&InMemoryDB,p:&ProfileIdentity,programs:&[Bytes;5])->Result<GenesisImage,GenesisError>{
 let n=p.spec().m0_list.len();if n<usize::from(p.cp().M_min)||n>usize::from(p.cp().M_max)||n>p.cp().REG_RECORDS_MAX as usize{return Err(GenesisError::Profile)}
 if programs.iter().any(|p|p.is_empty())||code_hash(programs).0!=p.spec().sys_code_hash{return Err(GenesisError::Code)}
 check_caps(alloc,p)?;
 if let Some(a)=alloc.accounts.get(&crate::pay_address()).and_then(|a|a.info()){if a.code_hash!=revm::primitives::KECCAK_EMPTY{return Err(GenesisError::Code)}}
 let alloc_sum=sum_balances(alloc)?;let bond=number(p.spec().cp[16]);let bonds=bond.checked_mul(U256::from(n)).ok_or(GenesisError::Supply)?;
 let future=number(p.spec().cp[15]).checked_mul(U256::from(p.cp().H_END)).ok_or(GenesisError::Supply)?;
 let supply_bound=add(add(alloc_sum,bonds)?,future)?;
 if supply_bound>=(U256::from(1)<<127){return Err(GenesisError::Supply)}
 let mut db=alloc.clone();let mut hashes=[B256::ZERO;5];
 for (i,code) in programs.iter().enumerate(){let code=Bytecode::new_raw(code.clone());hashes[i]=code.hash_slow();let mut info=db.accounts.get(&system(i)).and_then(|a|a.info()).unwrap_or_default();info.code_hash=hashes[i];info.code=Some(code);db.insert_account_info(system(i),info);}
 for (i,v) in p.spec().cp.iter().enumerate(){put(&mut db,system(3),U256::from(i),number(*v));}
 let registry=system(0);for (slot,v) in [(0,U256::from(n)),(1,U256::from(n)),(2,U256::ZERO),(3,U256::ZERO),(4,bonds)]{put(&mut db,registry,U256::from(slot),v)}
 for (i,m) in p.spec().m0_list.iter().enumerate(){let id=Address::from(m.id);let reward=Address::from(m.reward_addr);
  let info=db.accounts.get(&reward).and_then(|a|a.info()).ok_or(GenesisError::RewardNotDurable)?;
  if info.nonce==0&&info.code_hash==revm::primitives::KECCAK_EMPTY{return Err(GenesisError::RewardNotDurable)}
  let r=record(id);let packed=U256::from(1)|(U256::from(1)<<16)|(U256::from(i)<<24)|(U256::from(i)<<56);
  for (k,v) in [(0,packed),(1,U256::from_be_slice(reward.as_slice())),(2,bond),(3,U256::ZERO)]{put(&mut db,registry,r+U256::from(k),v)}
  put(&mut db,registry,base(5)+U256::from(i),U256::from_be_slice(id.as_slice()));put(&mut db,registry,base(6)+U256::from(i),U256::from_be_slice(id.as_slice()));
 }
 let a=db.accounts.get_mut(&pay::escrow_address()).ok_or(GenesisError::Code)?;a.info.balance=add(a.info.balance,bonds)?;
 check_caps(&db,p)?;let root=world_root(&db)?;
 Ok(GenesisImage{state:db,profile:B256::from(p.genesis_hash()),codes:hashes,root,alloc_sum,bonds,supply_bound})
}
pub(crate) fn build(alloc:&InMemoryDB,p:&ProfileIdentity,programs:&[Bytes;5])->Result<GenesisImage,GenesisError>{
 let image=construct(alloc,p,programs)?;if image.root.0!=p.spec().alloc_root{return Err(GenesisError::Root)}Ok(image)
}
pub(crate) fn world_root(db:&InMemoryDB)->Result<B256,GenesisError>{
 let num=|n:U256|crate::fixed::U256::from_be_slice(&n.to_be_bytes::<32>()).unwrap();let mut accounts=Vec::new();
 for (address,a) in &db.accounts{let Some(info)=a.info() else{continue};let slots:Vec<_>=a.storage.iter().filter(|(_,v)|**v!=U256::ZERO).map(|(k,v)|(num(*k),num(*v))).collect();
  accounts.push(crate::trie::Account{address:address.0.0,nonce:info.nonce,balance:num(info.balance),storage_root:crate::trie::storage_root(&slots,slots.len()).map_err(|_|GenesisError::Root)?,code_hash:info.code_hash.0});
 }crate::trie::state_root(&accounts,accounts.len()).map(B256::from).map_err(|_|GenesisError::Root)
}
pub(crate) fn construct_root(alloc:&InMemoryDB,p:&ProfileIdentity,programs:&[Bytes;5])->Result<B256,GenesisError>{Ok(construct(alloc,p,programs)?.root)}
/// The constructed genesis state without the allocRoot comparison (A02 fact assembly).
pub(crate) fn construct_state(alloc:&InMemoryDB,p:&ProfileIdentity,programs:&[Bytes;5])->Result<(B256,InMemoryDB),GenesisError>{let i=construct(alloc,p,programs)?;Ok((i.root,i.state))}
#[cfg(test)]pub(crate) fn derive_fixture_root(alloc:&InMemoryDB,p:&ProfileIdentity,programs:&[Bytes;5])->Result<B256,GenesisError>{Ok(construct(alloc,p,programs)?.root)}
