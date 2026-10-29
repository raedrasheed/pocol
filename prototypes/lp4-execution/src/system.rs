//! LP4-SYS-01 execution primitive. Not block admission or boot authorization.
//! This module is crate-private until P5 derives its inputs from validated UT,
//! stored headers and the prescribed phase schedule. Tests cannot authorize boot.
use revm::{InMemoryDB, DatabaseCommit, FrameOrResult,
    primitives::{Address, Bytes, EvmState, TxEnv, TxKind, U256}, interpreter::CallInputs};
use crate::{native::{self, NativeBlockContext}, profile::ProfileIdentity};
pub(crate) const PER_CALL_GAS:u64=30_000_000;
pub(crate) fn system_address()->Address { let mut a=[0xff;20];a[19]=0xfe;Address::from(a) }

/// Only structural metadata validation here. Cryptographic UT admission and
/// fallback validation belong to P5; this type is not publicly constructible.
#[derive(Clone,Copy)]
pub(crate) enum Proposer { Signed(Address), Fallback }
impl Proposer {
    fn origin(self)->Result<Address,SystemError>{match self{
        Self::Signed(a) if a!=Address::ZERO=>Ok(a),Self::Fallback=>Ok(Address::ZERO),
        _=>Err(SystemError::Metadata),
    }}
    pub(crate) fn pending_word(self)->Result<U256,SystemError>{
        let origin=self.origin()?;
        Ok((U256::from_be_slice(origin.as_slice())<<8)|U256::from(matches!(self,Self::Fallback) as u64))
    }
}
#[derive(Debug,PartialEq,Eq)]
pub(crate) enum SystemError { Metadata, Context, Budget, Execution(String), UnexpectedLogs, Poisoned, Genesis, Phase, Supply }
#[derive(Debug)]
pub(crate) struct Observation {pub raw_gas:u64,pub refund_counter:i64,pub output:Bytes}

/// Private candidate: no canonical storage reference or commit/public finish API.
/// Any failure poisons all prior effects; the caller cannot resume a failed block.
/// Disk persistence, USER integration and validated phase ordering remain P5/P9.
pub(crate) struct Candidate<'a> {
    state:InMemoryDB,
    /// The state's logical code total, kept across calls (decision package C-2, item 2).
    code_total:Option<u64>,
    profile:&'a ProfileIdentity,
    block:&'a NativeBlockContext,
    proposer:Proposer,
    used:u64,
    failed:bool,
    image:Option<&'a crate::genesis_builder::GenesisImage>,
    stage:Stage,
    tips:U256,
    user:Option<crate::user_phase::UserCandidate>,
    final_block:bool,
    issued:U256,
}
#[derive(Clone,Copy,PartialEq,Eq)] enum Stage{System,Ready,Running,Complete}
impl<'a> Candidate<'a> {
    pub(crate) fn new(snapshot:&InMemoryDB,profile:&'a ProfileIdentity,block:&'a NativeBlockContext,proposer:Proposer)->Result<Self,SystemError>{
        proposer.origin()?;
        if block.height==U256::ZERO || block.gas_limit!=U256::from(profile.cp().GAS_LIMIT){return Err(SystemError::Context)}
        let limits=crate::code_caps::CodeLimits{runtime:profile.cp().B_code_max,logical:profile.cp().CODE_CAP};
        crate::code_caps::validate_snapshot(snapshot,limits).map_err(SystemError::Execution)?;
        let code_total=Some(crate::code_caps::inventory(snapshot).map_err(SystemError::Execution)?);
        Ok(Self{state:snapshot.clone(),profile,block,proposer,used:0,failed:false,image:None,stage:Stage::System,tips:U256::ZERO,user:None,final_block:false,issued:U256::ZERO,code_total})
    }
    pub(crate) fn call(&mut self,to:Address,data:Bytes)->Result<Observation,SystemError>{
        if self.failed{return Err(SystemError::Poisoned)}
        if !matches!(self.stage,Stage::System|Stage::Running){self.poison();return Err(SystemError::Phase)}
        let result=self.call_inner(to,data);
        if result.is_err(){self.poison();}
        result
    }
    fn poison(&mut self){self.failed=true;self.state=InMemoryDB::default();self.user=None;self.tips=U256::ZERO;self.issued=U256::ZERO;}
    /// Actual genesis/root/code/CP proof required. Still crate-private: complete
    /// current UT/stored-header phase validation remains mandatory in P5.
    pub(crate) fn from_genesis(snapshot:&InMemoryDB,image:&'a crate::genesis_builder::GenesisImage,profile:&'a ProfileIdentity,block:&'a NativeBlockContext,proposer:Proposer)->Result<Self,SystemError>{
        image.verify_candidate(snapshot,profile).map_err(|_|SystemError::Genesis)?;
        let height:u64=block.height.try_into().map_err(|_|SystemError::Context)?;
        let time:u64=block.timestamp.try_into().map_err(|_|SystemError::Context)?;
        if height==0||height>profile.cp().H_END{return Err(SystemError::Context)}
        if height==1&&crate::genesis_builder::world_root(snapshot).map_err(|_|SystemError::Genesis)?!=image.root(){return Err(SystemError::Genesis)}
        let reg=&snapshot.accounts[&crate::pay::registry_address()];let counters=reg.storage.get(&U256::from(3)).copied().unwrap_or_default();
        let close_h:u64=((counters>>187u32)&U256::from(u64::MAX)).try_into().map_err(|_|SystemError::Context)?;
        let cursor=(counters>>90u32)&U256::from(u32::MAX);let len=reg.storage.get(&U256::ZERO).copied().unwrap_or_default();
        let parent=crate::finality_core::ParentFacts{closing:((counters>>89u32)&U256::from(1))!=U256::ZERO,close_h,cursor_done:cursor>=len,is_final:false};
        let mut candidate=Self::new(snapshot,profile,block,proposer)?;candidate.image=Some(image);
        candidate.final_block=crate::finality_core::final_predicate(parent,height,time,u32::from(profile.cp().D),profile.cp().H_END,profile.cp().T_END);Ok(candidate)
    }
    pub(crate) fn execute_users(&mut self,raw:&[Vec<u8>])->Result<(),SystemError>{
        if self.failed{return Err(SystemError::Poisoned)}
        let result=(||{
            if self.stage!=Stage::System||self.image.is_none(){return Err(SystemError::Phase)}
            let reg=&self.state.accounts[&crate::pay::registry_address()];let c=reg.storage.get(&U256::from(3)).copied().unwrap_or_default();
            if (c>>123u32)&U256::from(u64::MAX)!=self.block.height||(c>>24u32)&U256::from(u64::MAX)!=self.block.height||(c>>88u32)&U256::from(1)==U256::ZERO{return Err(SystemError::Phase)}
            if raw.iter().any(|r|r.len()>self.profile.cp().TX_MAX as usize){return Err(SystemError::Budget)}
            let budget=self.profile.cp().BODY_MAX as usize;
            let users=crate::user_phase::execute_for_profile(&self.state,raw,self.block,budget,budget,self.profile).map_err(|e|SystemError::Execution(format!("{e:?}")))?;
            // The USER phase replaced the state: the next call checks it whole again (C-2 item 2).
            self.state=users.state().clone();self.code_total=None;self.tips=users.tips();self.user=Some(users);self.stage=Stage::Ready;Ok(())
        })();if result.is_err(){self.poison();}result
    }
    /// No externally supplied amount/recipient/finality/tips/authorization flag.
    /// Every error poisons all USER/SYS/issuance effects of this private candidate.
    pub(crate) fn phase2(&mut self)->Result<Observation,SystemError>{
        if self.failed{return Err(SystemError::Poisoned)}
        let result=(||{
            if self.stage!=Stage::Ready{return Err(SystemError::Phase)}
            let image=self.image.ok_or(SystemError::Genesis)?;
            image.verify_candidate(&self.state,self.profile).map_err(|_|SystemError::Genesis)?;
            if !self.final_block{
                let amount=U256::from_be_bytes(self.profile.spec().cp[15].to_be_bytes());
                self.tips.checked_add(amount).ok_or(SystemError::Supply)?;
                let escrow=self.state.accounts.get_mut(&crate::pay::escrow_address()).ok_or(SystemError::Genesis)?;
                escrow.info.balance=escrow.info.balance.checked_add(amount).ok_or(SystemError::Supply)?;
                self.issued=amount;
            }
            image.verify_candidate(&self.state,self.profile).map_err(|_|SystemError::Genesis)?;
            self.stage=Stage::Running;
            let selector=if self.final_block{"finalize(uint256)"}else{"closeBlock(uint256)"};
            let mut input=revm::primitives::keccak256(selector.as_bytes())[..4].to_vec();input.extend(self.tips.to_be_bytes::<32>());
            let observation=self.call(crate::pay::escrow_address(),Bytes::from(input))?;
            image.verify_candidate(&self.state,self.profile).map_err(|_|SystemError::Genesis)?;
            self.stage=Stage::Complete;Ok(observation)
        })();if result.is_err(){self.poison();}result
    }
    /// Final state commitment only; transaction/receipt/body/header validation
    /// and real disk commit still belong to the complete P5/P9 acceptance path.
    pub(crate) fn check_state_root(&mut self,expected:revm::primitives::B256)->Result<(),SystemError>{
        if self.failed{return Err(SystemError::Poisoned)}
        let valid=self.stage==Stage::Complete&&crate::genesis_builder::world_root(&self.state).map(|r|r==expected).unwrap_or(false);if !valid{self.poison();return Err(SystemError::Context)}Ok(())
    }
    pub(crate) fn issued(&self)->Result<U256,SystemError>{if self.failed{Err(SystemError::Poisoned)}else{Ok(self.issued)}}
    fn call_inner(&mut self,to:Address,data:Bytes)->Result<Observation,SystemError>{
        let remaining=self.profile.cp().G_SYS.checked_sub(self.used).ok_or(SystemError::Budget)?;
        let allocation=remaining.min(PER_CALL_GAS);
        if allocation==0{return Err(SystemError::Budget)}
        let origin=self.proposer.origin()?;
        let limits=crate::code_caps::CodeLimits{runtime:self.profile.cp().B_code_max,logical:self.profile.cp().CODE_CAP};
        let tx=TxEnv{caller:origin,transact_to:TxKind::Call(to),gas_limit:allocation,
            gas_price:U256::ZERO,value:U256::ZERO,data,chain_id:Some(self.profile.spec().chain_id),..Default::default()};
        let mut evm=native::build_engine(&self.state,self.code_total,tx,self.block,self.profile.spec().chain_id,Some(limits));
        // No transact(), caller deduction, intrinsic gas or transaction post-handler.
        evm.handler.pre_execution.load_accounts(&mut evm.context).map_err(|e|SystemError::Execution(format!("{e:?}")))?;
        let precompiles=evm.handler.pre_execution.load_precompiles();
        evm.context.evm.set_precompiles(precompiles);
        for address in [system_address(),to,crate::pay::escrow_address()] {
            evm.context.evm.journaled_state.warm_preloaded_addresses.insert(address);
        }
        if origin!=Address::ZERO{evm.context.evm.journaled_state.warm_preloaded_addresses.insert(origin);}
        let mut input=CallInputs::new_boxed(&evm.context.evm.env.tx,allocation).ok_or(SystemError::Context)?;
        input.caller=system_address();
        let first=evm.handler.execution.call(&mut evm.context,input).map_err(|e|SystemError::Execution(format!("{e:?}")))?;
        let result=match first{
            FrameOrResult::Frame(frame)=>evm.run_the_loop(frame).map_err(|e|SystemError::Execution(format!("{e:?}")))?,
            FrameOrResult::Result(result)=>result,
        }.into_interpreter_result();
        if !result.result.is_ok(){return Err(SystemError::Execution(format!("{:?}",result.result)))}
        let raw=result.gas.spent();
        self.used=self.used.checked_add(raw).ok_or(SystemError::Budget)?;
        if self.used>self.profile.cp().G_SYS{return Err(SystemError::Budget)}
        let (state,logs)=evm.context.evm.journaled_state.finalize();
        // Initial approved programs emit none. A log is an error, never dropped.
        if !logs.is_empty(){return Err(SystemError::UnexpectedLogs)}
        drop(evm);
        // C-2 item 2: `new` checked the whole snapshot and set the total; each call then checks what it
        // touched (the same Ok/Err and texts as the full scan it replaces). After the USER phase the
        // total is unknown and the call checks the whole state, as before.
        let touched:Vec<Address>=state.keys().copied().collect();
        let before=match self.code_total{Some(_)=>crate::code_caps::code_of(&self.state,&touched).map_err(SystemError::Execution)?,None=>0};
        commit_epoch(&mut self.state,state);
        self.code_total=Some(match self.code_total{
            Some(total)=>crate::code_caps::revalidate_touched(&self.state,limits,&touched,total,before).map_err(SystemError::Execution)?,
            None=>{crate::code_caps::validate_snapshot(&self.state,limits).map_err(SystemError::Execution)?;crate::code_caps::inventory(&self.state).map_err(SystemError::Execution)?}});
        debug_assert_eq!(crate::code_caps::validate_snapshot(&self.state,limits),Ok(()));
        debug_assert_eq!(crate::code_caps::inventory(&self.state),Ok(self.code_total.unwrap()));
        Ok(Observation{raw_gas:raw,refund_counter:result.gas.refunded(),output:result.output})
    }
    pub(crate) fn snapshot(&self)->Result<&InMemoryDB,SystemError>{if self.failed{Err(SystemError::Poisoned)}else{Ok(&self.state)}}
    pub(crate) fn used(&self)->u64{self.used}
}
fn commit_epoch(db:&mut InMemoryDB,state:EvmState){
    let removed:Vec<_>=state.iter().filter(|(_,a)|a.is_selfdestructed()||(a.is_touched()&&a.is_empty())).map(|(a,_)|*a).collect();
    db.commit(state);
    for address in removed{let a=db.accounts.entry(address).or_default();a.account_state=revm::db::AccountState::NotExisting;a.storage.clear();a.info=Default::default();}
}

#[cfg(test)] mod tests;

#[cfg(test)] mod issuance_tests;

#[cfg(test)] mod chunk_tests;
