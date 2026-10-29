//! Transactional USER phase only. NOT PoCol block acceptance or persisted commit.
//! Context must later be bound to stored headers by P5. No system phase or final
//! header state-root comparison is claimed here. Budgets are local API bounds.
use std::collections::HashSet;
use revm::{InMemoryDB,db::AccountState};
use crate::view::{View,WorkState};
use revm::primitives::{Address,B256,EvmState,Log,TxEnv,U256};
use crate::{native::{self,NativeBlockContext},wire::{self,WireError},receipts,fees};
/// `DirtyAccounts`/`AcctCap`: even the halt's own nonce/fee effects would exceed the
/// limit, so the transaction cannot be included (block invalid; builders skip it).
#[derive(Debug,Clone,PartialEq,Eq)] pub enum UserError{Context,Budget,Wire(usize,WireError),Execution(usize,String),Gas,Arithmetic,Root,Commitments,DirtyAccounts(usize),AcctCap(usize)}

/// execution.md السقوف (A03): limits whose violation is an exceptional halt of the
/// transaction. All of its gas is charged, its state changes are reverted except the
/// nonce increment and the fee payment, its receipt has status 0 and no logs, and the
/// block stays valid. Log accounting follows decision LOG-ACCT-01.
pub mod caps{
    /// Per-block log bytes; one log is charged 36 + 32·|topics| + |data| (LOG-ACCT-01).
    pub const LOG_BLOCK_MAX:u64=32*1024;
    /// Logs per block (LG2: 546 succeed, 547 halt; lemma P1).
    pub const LOG_COUNT_MAX:u64=546;
    /// Aggregate charged log bytes over the canonical chain (execution.md:82: 1 GiB). Not a CP field: it
    /// enters neither GenesisSpec nor genesisHash. The caps-test build (validation.md:2065, simnet only,
    /// absent from the release) sets 2^24 so that T18b L6 reaches it.
    #[cfg(not(feature="caps-test"))] pub const LOG_CAP:u64=1<<30;
    #[cfg(feature="caps-test")] pub const LOG_CAP:u64=1<<24;
    /// Distinct storage keys and distinct accounts changed by USER transactions in a block.
    pub const DIRTY_BLOCK_MAX:usize=4096;
    pub const ACCT_DIRTY_BLOCK_MAX:usize=2048;
    /// User-context SYS allowance per block: SYS_KEYS_USER_MAX keys (CP) and 16 accounts; none while closing.
    pub const USER_SYS_ACCOUNTS:usize=16;
}
/// Why a transaction halted exceptionally.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum Halt{LogCount,LogBytes,LogCap,DirtyKeys,DirtyAccounts,SlotCap,AcctCap,SysKeys,SysAccounts,SysWriteClosing}
/// LOG-ACCT-01 charge of a transaction's logs.
pub fn log_charge(logs:&[Log])->u64{logs.iter().map(|l|36+32*l.data.topics().len() as u64+l.data.data.len() as u64).sum()}
/// Block-level context of the USER phase.
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub struct UserContext{
    /// Charged log bytes of the canonical ancestors (LOG_CAP).
    pub cum_log_bytes:u64,
    /// The block executes in the closing phase (no user SYS writes).
    pub closing:bool,
}
#[derive(Clone,Debug,PartialEq,Eq)] pub struct UserCommitments{
 pub transactions_root:B256,pub receipts_root:B256,pub gas_used:u64,pub logs_bloom:[u8;256],
}
pub struct UserCandidate<S=InMemoryDB>{state:S,commitments:UserCommitments,receipts:Vec<Vec<u8>>,tips:U256,log_bytes:u64,halts:Vec<(usize,Halt)>,dirty_accounts:u64}
impl<S> UserCandidate<S>{
 pub fn state(&self)->&S{&self.state}
 /// The state, and the rest of the candidate without it (C-1 E3: the block keeps working on the state).
 pub(crate) fn split(self)->(S,UserCandidate<()>){
  let UserCandidate{state,commitments,receipts,tips,log_bytes,halts,dirty_accounts}=self;
  (state,UserCandidate{state:(),commitments,receipts,tips,log_bytes,halts,dirty_accounts})
 }
 /// Charged log bytes of this block (LOG-ACCT-01).
 pub fn log_bytes(&self)->u64{self.log_bytes}
 /// Transactions that halted exceptionally, by index.
 pub fn halts(&self)->&[(usize,Halt)]{&self.halts}
 /// Accounts the block's USER transactions made dirty (ACCT_DIRTY_BLOCK_MAX metering; reporting only).
 pub fn dirty_accounts(&self)->u64{self.dirty_accounts}
 pub fn commitments(&self)->&UserCommitments{&self.commitments}
 pub fn receipts(&self)->&[Vec<u8>]{&self.receipts}
 pub fn tips(&self)->U256{self.tips}
}
impl UserCandidate{
 /// Intermediate USER root, never the final PoCol header state root.
 pub fn state_root(&self,account_budget:usize,slot_budget_per_account:usize)->Result<B256,UserError>{
  let mut accounts=Vec::new();
  for (address,a) in &self.state.accounts{
   if a.account_state==AccountState::NotExisting{continue}
   if accounts.len()>=account_budget || a.storage.len()>slot_budget_per_account{return Err(UserError::Budget)}
   let number=|v:U256|crate::fixed::U256::from_be_slice(&v.to_be_bytes::<32>()).unwrap();
   let slots=a.storage.iter().map(|(k,v)|(number(*k),number(*v))).collect::<Vec<_>>();
   accounts.push(crate::trie::Account{address:address.0.0,nonce:a.info.nonce,balance:number(a.info.balance),storage_root:crate::trie::storage_root(&slots,slot_budget_per_account).map_err(|_|UserError::Root)?,code_hash:a.info.code_hash.0});
  }
  crate::trie::state_root(&accounts,account_budget).map(B256::from).map_err(|_|UserError::Root)
 }
}
/// Builds an isolated candidate. It cannot mutate the caller's snapshot.
pub fn execute_user_phase(snapshot:&InMemoryDB,raw:&[Vec<u8>],block:&NativeBlockContext,byte_budget:usize,entry_budget:usize)->Result<UserCandidate,UserError>{
 execute_user_phase_inner(snapshot,raw,block,byte_budget,entry_budget,None)
}
/// Profile-bound USER phase. Identity/schema is not ParamGate/boot acceptance.
pub fn execute_for_profile(snapshot:&InMemoryDB,raw:&[Vec<u8>],block:&NativeBlockContext,byte_budget:usize,entry_budget:usize,profile:&crate::profile::ProfileIdentity)->Result<UserCandidate,UserError>{
 execute_for_block(snapshot,raw,block,byte_budget,entry_budget,profile,UserContext::default())
}
/// Profile-bound USER phase of a block, with its log history and closing state.
pub fn execute_for_block(snapshot:&InMemoryDB,raw:&[Vec<u8>],block:&NativeBlockContext,byte_budget:usize,entry_budget:usize,profile:&crate::profile::ProfileIdentity,ctx:UserContext)->Result<UserCandidate,UserError>{
 if block.gas_limit!=U256::from(profile.cp().GAS_LIMIT){return Err(UserError::Context)}
 execute_user_phase_inner_ctx(snapshot,raw,block,byte_budget,entry_budget,Some(profile),ctx)
}

/// A13: CacheDB::commit stores every accessed slot (zero values and slots that were only
/// read) and the EIP-161 cleanup leaves NotExisting entries, so containers would grow with
/// every key ever touched. In this EmptyDB-backed state an absent slot reads 0 and an absent
/// account does not exist, exactly as those entries do, and roots ignore both; dropping them
/// keeps every container proportional to the live state that SLOT_CAP/ACCT_CAP bound.
/// Tables never shrink by themselves, so a table left oversized by such entries is shrunk:
/// every table then has at most the buckets its current entries need (resources::memory).
pub fn normalize(db:&mut InMemoryDB,touched:&[Address]){
 for a in touched{
  let gone=match db.accounts.get_mut(a){Some(x) if x.account_state==AccountState::NotExisting||x.info().is_none()=>true,
   Some(x)=>{x.storage.retain(|_,v|*v!=U256::ZERO);if x.storage.capacity()>2*x.storage.len()+8{x.storage.shrink_to_fit()}false},None=>false};
  if gone{db.accounts.remove(a);}
 }
 if db.accounts.capacity()>2*db.accounts.len()+16{db.accounts.shrink_to_fit()}
}
/// Commits one transaction's state diff. EIP161 cleanup, as in the existing Cancun control runner:
/// CacheDB::commit alone keeps touched empty accounts, so they are removed before the next
/// transaction; with `normalized`, only live entries are kept (A13). The USER phase of a block and
/// the pool's pending state both commit through here.
pub fn commit_tx(state:&mut InMemoryDB,diff:EvmState,normalized:bool){commit_tx_on(state,diff,normalized)}
/// `commit_tx` on a whole state or an overlay (C-1 E3).
pub fn commit_tx_on<S:WorkState+?Sized>(state:&mut S,diff:EvmState,normalized:bool){
 let removed:Vec<_>=diff.iter().filter(|(_,a)|a.is_selfdestructed()||(a.is_touched()&&a.is_empty())).map(|(a,_)|*a).collect();
 let touched:Vec<_>=diff.iter().filter(|(_,a)|a.is_touched()).map(|(a,_)|*a).collect();
 state.commit(diff);
 for address in removed{state.remove(address);}
 if normalized{state.normalize(&touched)}
}
/// Code bytes no existing account references (A13), kept out of later snapshots.
pub fn drop_unreferenced_code(db:&mut InMemoryDB){
 let used:HashSet<B256>=db.accounts.values().filter_map(|a|a.info()).map(|i|i.code_hash).collect();
 db.contracts.retain(|h,_|used.contains(h)||*h==revm::primitives::KECCAK_EMPTY||*h==B256::ZERO);
 if db.contracts.capacity()>2*db.contracts.len()+16{db.contracts.shrink_to_fit()}
}
/// Per-block meter of the A03 limits; live counts follow the state incrementally.
struct BlockMeter{log_bytes:u64,log_count:u64,cum_log:u64,dirty_keys:HashSet<(Address,U256)>,dirty_accts:HashSet<Address>,
    sys_keys:HashSet<(Address,U256)>,sys_accts:HashSet<Address>,live_slots:u64,live_accts:u64,closing:bool,
    /// SLOT_CAP, ACCT_CAP, SYS_KEYS_USER_MAX when profile-bound.
    caps:Option<(u64,u64,u64)>}
/// Effects of one transaction's state diff on the meter, before it is committed.
#[derive(Default)]
struct Effects{keys:Vec<(Address,U256)>,accts:Vec<Address>,sys_keys:Vec<(Address,U256)>,sys_accts:Vec<Address>,d_slots:i64,d_accts:i64}
fn is_system(a:&Address)->bool{(0..5).any(|i|*a==crate::apply::program_address(i))}
/// Live (nonzero) storage slots and existing accounts of the user state (outside the five
/// system programs), counted as genesis check_caps does (D-A03-CAPS).
/// `live_counts` of any state view (C-1 E3).
pub(crate) fn live_counts_view(db:&dyn View)->(u64,u64){
 let (mut slots,mut accts)=(0u64,0u64);let mut users=Vec::new();
 db.each_account(&mut |k,a|{if a.info().is_some()&&!is_system(k){users.push(*k)}});
 for k in &users{accts+=1;db.each_slot(k,&mut |_,v|{if *v!=U256::ZERO{slots+=1}});}
 (slots,accts)
}
pub(crate) fn live_counts(db:&InMemoryDB)->(u64,u64){
 let mut slots=0u64;let mut accts=0u64;
 for (_,a) in db.accounts.iter().filter(|(k,a)|a.info().is_some()&&!is_system(k)){accts+=1;slots+=a.storage.values().filter(|v|**v!=U256::ZERO).count() as u64;}
 (slots,accts)
}
fn effects(pre:&dyn View,post:&EvmState)->Effects{
 let mut e=Effects::default();
 for (a,acct) in post{
  if !acct.is_touched(){continue}
  let pre_info=pre.account(a).and_then(|x|x.info());
  let mut pre_nonzero:Vec<U256>=Vec::new();pre.each_slot(a,&mut |k,v|{if *v!=U256::ZERO{pre_nonzero.push(*k)}});
  let removed=acct.is_selfdestructed()||acct.is_empty();
  let changed:Vec<U256>=if removed{vec![]}else{acct.storage.iter().filter(|(_,v)|v.is_changed()).map(|(k,_)|*k).collect()};
  let pre_slots=pre_nonzero.len() as i64;
  let info_changed=match &pre_info{Some(i)=>removed||i.balance!=acct.info.balance||i.nonce!=acct.info.nonce||i.code_hash!=acct.info.code_hash,None=>!removed};
  if info_changed||!changed.is_empty()||(removed&&pre_slots>0){e.accts.push(*a)}
  for k in &changed{e.keys.push((*a,*k))}
  let user=!is_system(a);
  if removed{if pre_info.is_some()&&user{e.d_accts-=1;e.d_slots-=pre_slots;}
   // Removed slots of a removed account are dirty as well.
   for k in &pre_nonzero{e.keys.push((*a,*k))}}
  else if user{
   if pre_info.is_none(){e.d_accts+=1}
   for (_,v) in acct.storage.iter().filter(|(_,v)|v.is_changed()){
    if v.original_value()==U256::ZERO&&v.present_value()!=U256::ZERO{e.d_slots+=1}else if v.original_value()!=U256::ZERO&&v.present_value()==U256::ZERO{e.d_slots-=1}
   }
  }
  if is_system(a)&&!changed.is_empty(){e.sys_accts.push(*a);for k in &changed{e.sys_keys.push((*a,*k))}}
 }
 e
}
/// Nonzero storage slots of the five system programs.
fn sys_slots(db:&InMemoryDB)->u64{
 db.accounts.iter().filter(|(k,a)|is_system(k)&&a.info().is_some()).map(|(_,a)|a.storage.values().filter(|v|**v!=U256::ZERO).count() as u64).sum()
}
/// A13: the pending pool state follows the user-state caps a block enforces (SLOT_CAP,
/// ACCT_CAP; D-A03-CAPS) and system storage stays within SYS_SLOT_MAX, so after
/// `normalize` it is bounded like any snapshot. A transaction whose effects would pass a cap
/// would halt in a block; the pool refuses it instead of keeping its effects.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct PoolMeter{live_slots:u64,live_accts:u64,sys_slots:u64,slot_cap:u64,acct_cap:u64,sys_slot_max:u64}
impl PoolMeter{
 /// C-1 E3: from counts already known (a state summary's user slots, user accounts and system slots).
 pub fn from_counts(live_slots:u64,live_accts:u64,sys_slots:u64,profile:&crate::profile::ProfileIdentity)->Self{let cp=profile.cp();
  Self{live_slots,live_accts,sys_slots,slot_cap:cp.SLOT_CAP,acct_cap:cp.ACCT_CAP,sys_slot_max:u64::from(cp.SYS_SLOT_MAX)}}
 pub fn of(db:&InMemoryDB,profile:&crate::profile::ProfileIdentity)->Self{
  let (live_slots,live_accts)=live_counts(db);let cp=profile.cp();
  Self{live_slots,live_accts,sys_slots:sys_slots(db),slot_cap:cp.SLOT_CAP,acct_cap:cp.ACCT_CAP,sys_slot_max:u64::from(cp.SYS_SLOT_MAX)}
 }
 /// Admits the state diff of one transaction executed on `pre`, or names the cap it would pass.
 pub fn admit(&mut self,pre:&dyn View,post:&EvmState)->Result<(),Halt>{
  let e=effects(pre,post);let mut d_sys=0i64;
  for (a,acct) in post.iter().filter(|(a,x)|is_system(a)&&x.is_touched()){let _=a;
   for (_,v) in acct.storage.iter().filter(|(_,v)|v.is_changed()){
    if v.original_value()==U256::ZERO&&v.present_value()!=U256::ZERO{d_sys+=1}else if v.original_value()!=U256::ZERO&&v.present_value()==U256::ZERO{d_sys-=1}
   }
  }
  let (s,a,y)=(add(self.live_slots,e.d_slots),add(self.live_accts,e.d_accts),add(self.sys_slots,d_sys));
  if s>self.slot_cap{return Err(Halt::SlotCap)}if a>self.acct_cap{return Err(Halt::AcctCap)}if y>self.sys_slot_max{return Err(Halt::SysKeys)}
  self.live_slots=s;self.live_accts=a;self.sys_slots=y;Ok(())
 }
 pub fn live(&self)->(u64,u64,u64){(self.live_slots,self.live_accts,self.sys_slots)}
}
fn new_count<T:std::hash::Hash+Eq+Copy>(set:&HashSet<T>,items:&[T])->usize{let mut seen=HashSet::new();items.iter().filter(|x|!set.contains(x)&&seen.insert(**x)).count()}
fn add(n:u64,d:i64)->u64{if d>=0{n.saturating_add(d as u64)}else{n.saturating_sub(d.unsigned_abs())}}
impl BlockMeter{
 /// The first violated limit, if the transaction's logs and state diff were committed.
 fn check(&self,logs:&[Log],e:&Effects)->Option<Halt>{
  let n=logs.len() as u64;let charge=log_charge(logs);
  if self.log_count+n>caps::LOG_COUNT_MAX{return Some(Halt::LogCount)}
  if self.log_bytes+charge>caps::LOG_BLOCK_MAX{return Some(Halt::LogBytes)}
  if self.cum_log.saturating_add(self.log_bytes+charge)>caps::LOG_CAP{return Some(Halt::LogCap)}
  if self.dirty_keys.len()+new_count(&self.dirty_keys,&e.keys)>caps::DIRTY_BLOCK_MAX{return Some(Halt::DirtyKeys)}
  if self.dirty_accts.len()+new_count(&self.dirty_accts,&e.accts)>caps::ACCT_DIRTY_BLOCK_MAX{return Some(Halt::DirtyAccounts)}
  if let Some((slot_cap,acct_cap,sys_keys))=self.caps{
   if add(self.live_slots,e.d_slots)>slot_cap{return Some(Halt::SlotCap)}
   if add(self.live_accts,e.d_accts)>acct_cap{return Some(Halt::AcctCap)}
   if !e.sys_keys.is_empty()&&self.closing{return Some(Halt::SysWriteClosing)}
   if (self.sys_keys.len()+new_count(&self.sys_keys,&e.sys_keys)) as u64>sys_keys{return Some(Halt::SysKeys)}
   if self.sys_accts.len()+new_count(&self.sys_accts,&e.sys_accts)>caps::USER_SYS_ACCOUNTS{return Some(Halt::SysAccounts)}
  }
  None
 }
 fn commit(&mut self,logs:&[Log],e:Effects){
  self.log_count+=logs.len() as u64;self.log_bytes+=log_charge(logs);
  self.dirty_keys.extend(e.keys);self.dirty_accts.extend(e.accts);self.sys_keys.extend(e.sys_keys);self.sys_accts.extend(e.sys_accts);
  self.live_slots=add(self.live_slots,e.d_slots);self.live_accts=add(self.live_accts,e.d_accts);
 }
}
/// Exceptional halt applied to the pre-transaction state: nonce + 1, the sender pays
/// gas_limit·price, the coinbase (Escrow) receives gas_limit·tip. Nothing else changes.
fn apply_halt<S:WorkState+?Sized>(state:&mut S,m:&mut BlockMeter,tx:&TxEnv,price:U256,tip:U256,i:usize)->Result<(),UserError>{
 let fee=price.checked_mul(U256::from(tx.gas_limit)).ok_or(UserError::Arithmetic)?;let reward=tip.checked_mul(U256::from(tx.gas_limit)).ok_or(UserError::Arithmetic)?;
 let existed=state.account(&tx.caller).and_then(|a|a.info()).is_some();
 let mut sender=state.account(&tx.caller).and_then(|a|a.info()).unwrap_or_default();
 sender.nonce=sender.nonce.checked_add(1).ok_or(UserError::Arithmetic)?;sender.balance=sender.balance.checked_sub(fee).ok_or(UserError::Arithmetic)?;
 state.insert_info(tx.caller,sender);
 let coinbase=crate::pay::escrow_address();
 if reward!=U256::ZERO{let mut c=state.account(&coinbase).and_then(|a|a.info()).unwrap_or_default();c.balance=c.balance.checked_add(reward).ok_or(UserError::Arithmetic)?;state.insert_info(coinbase,c);}
 let mut touched=vec![tx.caller];if reward!=U256::ZERO{touched.push(coinbase)}
 if m.dirty_accts.len()+new_count(&m.dirty_accts,&touched)>caps::ACCT_DIRTY_BLOCK_MAX{return Err(UserError::DirtyAccounts(i))}
 if !existed{if let Some((_,acct_cap,_))=m.caps{if m.live_accts+1>acct_cap{return Err(UserError::AcctCap(i))}}m.live_accts+=1;}
 m.dirty_accts.extend(touched);Ok(())
}
/// Logical code at `touched` before a commit (C-2 item 2; no work on the profile-free control path,
/// where `touched` is empty).
fn code_change_start(state:&dyn View,touched:&[Address],i:usize)->Result<u64,UserError>{
 crate::code_caps::code_of(state,touched).map_err(|e|UserError::Execution(i,e))
}
/// The running code total after a commit that changed only `touched`.
fn code_change_end(state:&dyn View,touched:&[Address],total:u64,before:u64,i:usize)->Result<u64,UserError>{
 if touched.is_empty(){return Ok(total)}
 let after=crate::code_caps::code_of(state,touched).map_err(|e|UserError::Execution(i,e))?;
 total.checked_sub(before).and_then(|t|t.checked_add(after)).ok_or_else(||UserError::Execution(i,"logical code overflow".into()))
}
fn execute_user_phase_inner(snapshot:&InMemoryDB,raw:&[Vec<u8>],block:&NativeBlockContext,byte_budget:usize,entry_budget:usize,profile:Option<&crate::profile::ProfileIdentity>)->Result<UserCandidate,UserError>{
 execute_user_phase_inner_ctx(snapshot,raw,block,byte_budget,entry_budget,profile,UserContext::default())
}
fn execute_user_phase_inner_ctx(snapshot:&InMemoryDB,raw:&[Vec<u8>],block:&NativeBlockContext,byte_budget:usize,entry_budget:usize,profile:Option<&crate::profile::ProfileIdentity>,ctx:UserContext)->Result<UserCandidate,UserError>{
 if raw.len()>entry_budget || raw.iter().try_fold(0usize,|n,v|n.checked_add(v.len())).ok_or(UserError::Budget)?>byte_budget{return Err(UserError::Budget)}
 let live=if profile.is_some(){live_counts(snapshot)}else{(0,0)};
 execute_user_phase_on(snapshot.clone(),live,None,raw,block,byte_budget,entry_budget,profile,ctx)
}
/// The USER phase on a state it owns, whole or an overlay (C-1 E3). `live` is the state's live user
/// slots and accounts (`live_counts`). `code_total` is its logical code total when the caller knows the
/// state passes `validate_snapshot` (C-1 E2: a block's summary); with None, the state is validated at
/// the first transaction, as before.
pub(crate) fn execute_user_phase_on<S:WorkState>(mut state:S,live:(u64,u64),known_total:Option<u64>,raw:&[Vec<u8>],block:&NativeBlockContext,byte_budget:usize,entry_budget:usize,profile:Option<&crate::profile::ProfileIdentity>,ctx:UserContext)->Result<UserCandidate<S>,UserError>{
 if raw.len()>entry_budget || raw.iter().try_fold(0usize,|n,v|n.checked_add(v.len())).ok_or(UserError::Budget)?>byte_budget{return Err(UserError::Budget)}
 let limit:u64=block.gas_limit.try_into().map_err(|_|UserError::Gas)?;
 let mut used=0u64;let mut tips=U256::ZERO;let mut encoded=Vec::new();let mut bloom=[0u8;256];
 let (live_slots,live_accts)=live;
 let mut meter=BlockMeter{log_bytes:0,log_count:0,cum_log:ctx.cum_log_bytes,dirty_keys:HashSet::new(),dirty_accts:HashSet::new(),sys_keys:HashSet::new(),sys_accts:HashSet::new(),
  live_slots,live_accts,closing:ctx.closing,caps:profile.map(|p|(p.cp().SLOT_CAP,p.cp().ACCT_CAP,u64::from(p.cp().SYS_KEYS_USER_MAX)))};
 let mut halts=Vec::new();
 // C-2 item 2: the state's logical code total, set at the first transaction and kept by each commit.
 let mut code_total=0u64;
 for (i,bytes) in raw.iter().enumerate(){
  let signed=match profile{Some(p)=>wire::decode_for_profile(bytes,p),None=>wire::decode_signed(bytes,byte_budget)}.map_err(|e|UserError::Wire(i,e))?;let tx=signed.tx_env();
  // Cancun reserves declared transaction gas against remaining block gas, not
  // merely the actual post-refund gas consumption of this transaction.
  if tx.gas_limit>limit.checked_sub(used).ok_or(UserError::Gas)?{return Err(UserError::Gas)}
  let (price,tip)=fees::effective_fees(block.base_fee,tx.gas_price,tx.gas_priority_fee).map_err(|_|UserError::Arithmetic)?;
  // C-2 item 1: the snapshot is validated once, at the point where the first transaction's own check
  // ran (after its decode, gas and fee checks; same index, same error text). Every later state comes
  // from committing this engine's results, so the later per-transaction scans could not fail.
  if let (Some(p),0)=(profile,i){
   match known_total{
    Some(t)=>code_total=t,
    None=>{
     let limits=crate::code_caps::CodeLimits{runtime:p.cp().B_code_max,logical:p.cp().CODE_CAP};
     let custom=|e|UserError::Execution(0,format!("{:?}",revm::primitives::EVMError::<core::convert::Infallible>::Custom(e)));
     crate::code_caps::validate_snapshot(state.as_view(),limits).map_err(custom)?;
     code_total=crate::code_caps::inventory(state.as_view()).map_err(custom)?;
    }
   }
  }
  let result=match profile{Some(p)=>native::execute_on_validated(state.as_view(),code_total,tx.clone(),block,p),None=>native::execute_in_context(state.as_view(),tx.clone(),block)}.map_err(|e|UserError::Execution(i,format!("{e:?}")))?;
  let e=effects(state.as_view(),&result.state);
  // The caps are PoCol execution: the profile-free Cancun control path stays geth-equivalent.
  if let Some(why)=profile.and_then(|_|meter.check(result.result.logs(),&e)){
   // Exceptional halt (A03): all gas, no logs, status 0; only nonce and fee remain.
   let touched=[tx.caller,crate::pay::escrow_address()];let before=code_change_start(state.as_view(),&touched,i)?;
   apply_halt(&mut state,&mut meter,&tx,price,tip,i)?;halts.push((i,why));
   code_total=code_change_end(state.as_view(),&touched,code_total,before,i)?;
   used=used.checked_add(tx.gas_limit).ok_or(UserError::Gas)?;if used>limit{return Err(UserError::Gas)}
   tips=tips.checked_add(tip.checked_mul(U256::from(tx.gas_limit)).ok_or(UserError::Arithmetic)?).ok_or(UserError::Arithmetic)?;
   encoded.push(receipts::encode(signed.kind(),false,used,&[]).map_err(|_|UserError::Root)?);
   continue;
  }
  meter.commit(result.result.logs(),e);
  let gas=result.result.gas_used();used=used.checked_add(gas).ok_or(UserError::Gas)?;if used>limit{return Err(UserError::Gas)}
  tips=tips.checked_add(tip.checked_mul(U256::from(gas)).ok_or(UserError::Arithmetic)?).ok_or(UserError::Arithmetic)?;
  encoded.push(receipts::encode(signed.kind(),result.result.is_success(),used,result.result.logs()).map_err(|_|UserError::Root)?);
  for (a,b) in bloom.iter_mut().zip(receipts::bloom(result.result.logs())){*a|=b}
  let touched:Vec<Address>=if profile.is_some(){result.state.keys().copied().collect()}else{vec![]};
  let before=code_change_start(state.as_view(),&touched,i)?;
  commit_tx_on(&mut state,result.state,profile.is_some());
  code_total=code_change_end(state.as_view(),&touched,code_total,before,i)?;
 }
 let commitments=UserCommitments{transactions_root:receipts::ordered_root(raw,entry_budget).map_err(|_|UserError::Root)?,receipts_root:receipts::ordered_root(&encoded,entry_budget).map_err(|_|UserError::Root)?,gas_used:used,logs_bloom:bloom};
 Ok(UserCandidate{state,commitments,receipts:encoded,tips,log_bytes:meter.log_bytes,halts,dirty_accounts:meter.dirty_accts.len() as u64})
}
/// Validates USER-phase commitments only. No storage/block acceptance side effect.
pub fn validate_user_phase(snapshot:&InMemoryDB,raw:&[Vec<u8>],block:&NativeBlockContext,byte_budget:usize,entry_budget:usize,expected:&UserCommitments)->Result<UserCandidate,UserError>{
 let candidate=execute_user_phase(snapshot,raw,block,byte_budget,entry_budget)?;
 if candidate.commitments()!=expected{return Err(UserError::Commitments)}
 Ok(candidate)
}

#[cfg(test)]
mod tests{
 use super::*;
 fn meter(closing:bool,sys_keys:u64)->BlockMeter{BlockMeter{log_bytes:0,log_count:0,cum_log:0,dirty_keys:HashSet::new(),dirty_accts:HashSet::new(),sys_keys:HashSet::new(),sys_accts:HashSet::new(),
  live_slots:0,live_accts:0,closing,caps:Some((u64::MAX,u64::MAX,sys_keys))}}
 fn sys_write(n:u64)->Effects{let a=crate::apply::program_address(0);Effects{keys:(0..n).map(|k|(a,U256::from(k))).collect(),accts:vec![a],sys_keys:(0..n).map(|k|(a,U256::from(k))).collect(),sys_accts:vec![a],d_slots:0,d_accts:0}}
 #[test] fn user_context_sys_allowance_and_closing(){
  // v1 programs expose no USER-callable storage writes; the rule is enforced regardless.
  assert_eq!(meter(false,256).check(&[],&sys_write(256)),None);
  assert_eq!(meter(false,256).check(&[],&sys_write(257)),Some(Halt::SysKeys));
  assert_eq!(meter(true,256).check(&[],&sys_write(1)),Some(Halt::SysWriteClosing));
  assert_eq!(meter(true,256).check(&[],&Effects::default()),None,"no SYS write, no halt while closing");
  let mut m=meter(false,256);m.sys_accts=(0..16u8).map(|i|Address::from([i;20])).collect();
  assert_eq!(m.check(&[],&sys_write(1)),Some(Halt::SysAccounts),"a 17th system account");
 }
}
