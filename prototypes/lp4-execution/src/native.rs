//! Experimental native backend; full LP4 acceptance and boot remain disabled.
//! PAY contracts: development/LP4-PAY-APPROVAL.md and LP4-PAY-LIFECYCLE-APPROVAL.md.
//! Profile-bound code limits: development/LP4-CODE-APPROVAL.md (LP4-CODE-01).
use std::sync::Arc;
use revm::{Evm, InMemoryDB};
use revm::primitives::{Address, B256, EVMError, InvalidTransaction, ResultAndState, SpecId, TxEnv, U256};
use crate::{pay::{self,NativeContext},pay_address,CHAIN_ID,state_view::StateRef};

pub fn execute(snapshot:&InMemoryDB, tx:TxEnv) -> Result<ResultAndState,EVMError<core::convert::Infallible>> {
    execute_at(snapshot,tx,U256::ZERO)
}

/// Experimental explicit NUMBER for compiled controls; not a PoCol apply context.
pub fn execute_at(snapshot:&InMemoryDB, tx:TxEnv, height:U256) -> Result<ResultAndState,EVMError<core::convert::Infallible>> {
    execute_at_time(snapshot,tx,height,U256::ZERO)
}

/// Experimental NUMBER/TIMESTAMP context, not validated PoCol block admission.
pub fn execute_at_time(snapshot:&InMemoryDB, tx:TxEnv, height:U256, timestamp:U256) -> Result<ResultAndState,EVMError<core::convert::Infallible>> {
    execute_in_context(snapshot,tx,&NativeBlockContext{height,timestamp,gas_limit:U256::from(30_000_000u64),base_fee:U256::ZERO,prevrandao:B256::ZERO})
}

/// Host-supplied fields for execution controls. This is NOT validated block apply:
/// P5 must derive these fields and the complete ancestor cache from stored headers.
/// In particular prevrandao must eventually be SHA256('PoColRandao' || seal_p).
pub struct NativeBlockContext {
    pub height:U256,
    pub timestamp:U256,
    pub gas_limit:U256,
    pub base_fee:U256,
    pub prevrandao:B256,
}

/// Execute against an immutable snapshot, including its BLOCKHASH ancestor cache.
/// No EIP-4788 system call is inserted here. Transaction/block validation and all
/// other resource ceilings remain separate obligations, never caller flags.
/// This chain-only API retains baseline Cancun code limits for old controls.
/// PoCol candidates must use execute_for_profile for approved CODE-01 limits.
pub fn execute_in_context(snapshot:&dyn crate::view::View, tx:TxEnv, block:&NativeBlockContext) -> Result<ResultAndState,EVMError<core::convert::Infallible>> {
    execute_on_chain(snapshot,tx,block,CHAIN_ID,None,None)
}
/// Bound to the same accepted LP3 identity used by signed decoding. P5 still
/// must supply validated parent/header context; no schema check authorizes boot.
pub fn execute_for_profile(snapshot:&dyn crate::view::View,tx:TxEnv,block:&NativeBlockContext,profile:&crate::profile::ProfileIdentity)->Result<ResultAndState,EVMError<core::convert::Infallible>>{
    let limits=crate::code_caps::CodeLimits{runtime:profile.cp().B_code_max,logical:profile.cp().CODE_CAP};
    crate::code_caps::validate_snapshot(snapshot,limits).map_err(EVMError::Custom)?;
    execute_on_chain(snapshot,tx,block,profile.spec().chain_id,Some(limits),None)
}
/// `execute_for_profile` on a state already known to pass `validate_snapshot`: the USER phase
/// validates its snapshot once, then changes it only by committing this engine's results. Within the
/// phase code enters only through CREATE, whose `create_return` hook enforces B_code_max and CODE_CAP,
/// and under Cancun (EIP-6780) code leaves only within its creating transaction, so the check holds
/// after every commit and the per-transaction scan is redundant (decision package C-2, item 1).
/// `code_total` is the state's logical code total (`code_caps::inventory`), kept by the phase, so a
/// create does not rescan the state (item 2). Debug builds still run the full checks, so a broken
/// invariant panics there.
/// Also the transaction pool's path (pocold): its pending state is the head state, checked once per
/// new head, changed only by `user_phase::commit_tx` of this engine's results.
pub fn execute_on_validated(state:&dyn crate::view::View,code_total:u64,tx:TxEnv,block:&NativeBlockContext,profile:&crate::profile::ProfileIdentity)->Result<ResultAndState,EVMError<core::convert::Infallible>>{
    let limits=crate::code_caps::CodeLimits{runtime:profile.cp().B_code_max,logical:profile.cp().CODE_CAP};
    debug_assert_eq!(crate::code_caps::validate_snapshot(state,limits),Ok(()),"USER-phase state left the code limits");
    debug_assert_eq!(crate::code_caps::inventory(state),Ok(code_total),"USER-phase running code total");
    execute_on_chain(state,tx,block,profile.spec().chain_id,Some(limits),Some(code_total))
}
/// The EVMError::Custom text of a simulation that `stop` ended.
pub const SIMULATION_STOPPED:&str="rpc simulation stopped";
/// The gas of one frame between two polls of a simulation's `stop` (network.md RpcGuard (F): the
/// cancellation token is checked every 10^6 gas).
pub const STOP_GAS:u64=1_000_000;
/// RPC simulation only (eth_call/eth_estimateGas, network.md RpcGuard (F)): `execute_for_profile` with
/// `stop` polled before every CALL/CREATE-family instruction, at every 64th JUMP/JUMPI, and at the
/// first JUMP/JUMPI or data-sized instruction (KECCAK256, the *COPY family, LOG0-4) once the frame has
/// spent STOP_GAS since the last poll. Between two polls a frame therefore runs at most STOP_GAS gas
/// plus one straight-line run of fixed-cost instructions (bounded by the code size) and one data-sized
/// instruction. A precompile call is not interpreted: it runs to its end (up to the call's gas), so
/// the cancellation gap is not bounded by STOP_GAS there. When `stop` returns true the execution ends
/// with EVMError::Custom(SIMULATION_STOPPED). Block validation, SYSTEM and USER never use it.
pub fn simulate_for_profile(snapshot:&dyn crate::view::View,tx:TxEnv,block:&NativeBlockContext,profile:&crate::profile::ProfileIdentity,stop:std::rc::Rc<dyn Fn()->bool>)->Result<ResultAndState,EVMError<core::convert::Infallible>>{
    let limits=crate::code_caps::CodeLimits{runtime:profile.cp().B_code_max,logical:profile.cp().CODE_CAP};
    crate::code_caps::validate_snapshot(snapshot,limits).map_err(EVMError::Custom)?;
    validate_nonce(snapshot,tx.caller,tx.nonce)?;
    build_engine(snapshot,None,tx,block,profile.spec().chain_id,Some(limits)).modify()
        .append_handler_register_box(Box::new(move|h|stop_register(h,stop.clone()))).build().transact()
}
/// Pool admission of an RPC-submitted transaction (pocold eth_sendRawTransaction, network.md RpcGuard
/// (D) step 4 and (F)): the same execution as `execute_on_validated` (with `code_total`) or
/// `execute_for_profile` (without), with `stop` polled as in `simulate_for_profile`. The hook only reads
/// the clock and calls the next instruction, so while `stop` returns false the result is the same.
/// Its result is never a block's: blocks are validated with `execute_on_validated` or the USER phase.
pub fn admit_for_profile(state:&dyn crate::view::View,code_total:Option<u64>,tx:TxEnv,block:&NativeBlockContext,profile:&crate::profile::ProfileIdentity,stop:std::rc::Rc<dyn Fn()->bool>)->Result<ResultAndState,EVMError<core::convert::Infallible>>{
    let limits=crate::code_caps::CodeLimits{runtime:profile.cp().B_code_max,logical:profile.cp().CODE_CAP};
    match code_total{
        Some(t)=>{debug_assert_eq!(crate::code_caps::validate_snapshot(state,limits),Ok(()),"pool state left the code limits");
            debug_assert_eq!(crate::code_caps::inventory(state),Ok(t),"pool running code total");}
        None=>crate::code_caps::validate_snapshot(state,limits).map_err(EVMError::Custom)?,
    }
    validate_nonce(state,tx.caller,tx.nonce)?;
    build_engine(state,code_total,tx,block,profile.spec().chain_id,Some(limits)).modify()
        .append_handler_register_box(Box::new(move|h|stop_register(h,stop.clone()))).build().transact()
}
fn stop_register<EXT:'static,DB:revm::Database>(h:&mut revm::handler::register::EvmHandler<'_,EXT,DB>,stop:std::rc::Rc<dyn Fn()->bool>){
    use revm::interpreter::{opcode::*,InstructionResult};
    // `n` counts hooked instructions; `mark` is the frame gas remaining at the last poll (a frame
    // switch changes it abruptly, which only adds a poll).
    let (n,mark)=(std::rc::Rc::new(std::cell::Cell::new(0u32)),std::rc::Rc::new(std::cell::Cell::new(u64::MAX)));
    for (op,every) in [(JUMP,64),(JUMPI,64),(KECCAK256,0),(CALLDATACOPY,0),(CODECOPY,0),(EXTCODECOPY,0),(RETURNDATACOPY,0),(MCOPY,0),
        (LOG0,0),(LOG1,0),(LOG2,0),(LOG3,0),(LOG4,0),(CALL,1),(CALLCODE,1),(DELEGATECALL,1),(STATICCALL,1),(CREATE,1),(CREATE2,1)]{
        let (stop,n,mark)=(stop.clone(),n.clone(),mark.clone());
        h.instruction_table.update_boxed(op,move|prev,interp,host|{
            let k=n.get().wrapping_add(1);n.set(k);let g=interp.gas.remaining();
            if (every!=0&&k%every==0)||mark.get().abs_diff(g)>=STOP_GAS{mark.set(g);
                if stop(){host.evm.error=Err(EVMError::Custom(SIMULATION_STOPPED.into()));interp.instruction_result=InstructionResult::FatalExternalError;return}}
            prev(interp,host)
        });
    }
}
fn execute_on_chain(snapshot:&dyn crate::view::View,tx:TxEnv,block:&NativeBlockContext,chain_id:u64,limits:Option<crate::code_caps::CodeLimits>,code_total:Option<u64>)->Result<ResultAndState,EVMError<core::convert::Infallible>>{
    validate_nonce(snapshot,tx.caller,tx.nonce)?;
    build_engine(snapshot,code_total,tx,block,chain_id,limits).transact()
}

/// Shared hooks only. SYSTEM uses direct frames; USER retains transact(). The engine reads `snapshot`
/// through a borrowed `StateRef` instead of a clone (decision package C-2, item 3); `code_total` is the
/// snapshot's logical code total when the caller knows it.
pub(crate) fn build_engine<'a>(snapshot:&'a dyn crate::view::View,code_total:Option<u64>,tx:TxEnv,block:&NativeBlockContext,chain_id:u64,limits:Option<crate::code_caps::CodeLimits>)->Evm<'a,NativeContext,StateRef<'a>>{
    Evm::builder().with_db(StateRef::new(snapshot,code_total)).with_external_context(NativeContext::default())
        .with_spec_id(SpecId::CANCUN)
        .modify_cfg_env(|cfg|cfg.chain_id=chain_id)
        .modify_block_env(|b|{b.number=block.height;b.timestamp=block.timestamp;b.gas_limit=block.gas_limit;b.basefee=block.base_fee;b.prevrandao=Some(block.prevrandao);b.coinbase=pay::escrow_address();b.blob_excess_gas_and_price=Some(revm::primitives::BlobExcessGasAndPrice::new(0,false));})
        .modify_tx_env(|t|*t=tx)
        .append_handler_register(|handler|{
            let previous=handler.execution.execute_frame.clone();
            handler.execution.execute_frame=Arc::new(move |frame,memory,tables,context|{
                context.external.active_code=frame.interpreter().contract.bytecode_address;
                previous(frame,memory,tables,context)
            });
            let previous=handler.execution.call.clone();
            handler.execution.call=Arc::new(move |context,inputs|{
                if inputs.bytecode_address==pay_address() || inputs.target_address==pay_address() {
                    pay::call(context,&inputs)
                } else { previous(context,inputs) }
            });
        }).append_handler_register(crate::collision::register)
        // D-2: MODEXP at address 5 is crate::modexp's (revm's results and gas; a faster exact path for one-word moduli).
        .append_handler_register(|handler|{
            let previous=handler.pre_execution.load_precompiles.clone();
            handler.pre_execution.load_precompiles=Arc::new(move||{let mut p=previous();p.extend([crate::modexp::BERLIN]);p});
        })
        .append_handler_register_box(Box::new(move |handler| {
            if let Some(limits)=limits {crate::code_caps::register(handler,limits);}
        })).build()
}

/// EIP-2681: maximum u64 transaction/account nonce cannot be incremented.
/// revm 19.7.0 can otherwise return a normal result for max-nonce creation.
pub fn validate_nonce(snapshot:&dyn crate::view::View,caller:Address,nonce:Option<u64>) -> Result<(),EVMError<core::convert::Infallible>> {
    if nonce==Some(u64::MAX) || snapshot.account(&caller).map(|a|a.info.nonce==u64::MAX).unwrap_or(false) {
        Err(EVMError::Transaction(InvalidTransaction::NonceOverflowInTransaction))
    } else {Ok(())}
}
