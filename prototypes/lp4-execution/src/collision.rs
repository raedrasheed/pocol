// SPDX-License-Identifier: MIT
// Includes adapted revm 19.7.0 create-frame logic, Copyright (c) 2021-2025 draganrakita.
// Full upstream notice: ../licenses/REVM-MIT.txt.
//! LP4-CREATE-01: development/LP4-COLLISION-APPROVAL.md.
//! Complete local state backend only, never a generic partial Database cache.
use std::sync::Arc;
use revm::{handler::register::EvmHandler, Context, Database, FrameOrResult, FrameResult};
use crate::state_view::LocalState;
use revm::primitives::{Address,U256};
use revm::interpreter::{CreateOutcome,Gas,InstructionResult,InterpreterResult};

/// InMemoryDB's backing EmptyDB contains no hidden slots/accounts. Its inventory
/// is the complete base state, overlaid by all current journal values. Newly
/// created accounts have cleared base storage. Reading this fact does not warm
/// any account/slot or mutate the journal.
pub fn storage_nonempty<EXT,DB:Database+LocalState>(context:&Context<EXT,DB>,address:Address)->bool {
    let journal=context.evm.journaled_state.state.get(&address);
    if journal.map(|a|a.storage.values().any(|s|s.present_value()!=U256::ZERO)).unwrap_or(false){return true;}
    if journal.map(|a|a.is_created()).unwrap_or(false){return false;}
    let mut any=false;
    context.evm.db.local().each_slot(&address,&mut |key,value|{
        if journal.and_then(|a|a.storage.get(key)).map(|s|s.present_value()).unwrap_or(*value)!=U256::ZERO{any=true}
    });
    any
}

// Cancun create-frame adapter based on revm 19.7.0's make_create_frame,
// MIT; source: revm/src/context/evm_context.rs. The added complete
// storage check occurs at the ordinary collision boundary, before any account
// creation/value checkpoint. No Ethereum geth implementation code is used.
pub fn register<EXT:'static,DB:Database+LocalState>(handler:&mut EvmHandler<'_,EXT,DB>){
    assert_eq!(handler.cfg.spec_id,revm::primitives::SpecId::CANCUN);
    handler.execution.create=Arc::new(move |context,inputs|{
        use revm::interpreter::{Contract,Interpreter};
        use revm::primitives::{Bytecode,Bytes,B256,CreateScheme,keccak256};
        let fail=|result|Ok(FrameOrResult::Result(FrameResult::Create(CreateOutcome::new(
            InterpreterResult{result,gas:Gas::new(inputs.gas_limit),output:Default::default()},None))));
        if context.evm.journaled_state.depth()>revm::CALL_STACK_LIMIT {return fail(InstructionResult::CallTooDeep);}
        if context.evm.balance(inputs.caller)?.data<inputs.value{return fail(InstructionResult::OutOfFunds);}
        let Some(nonce)=context.evm.journaled_state.inc_nonce(inputs.caller) else{return fail(InstructionResult::Return);};
        let target=inputs.created_address(nonce-1);
        if context.evm.precompiles.contains(&target){return fail(InstructionResult::CreateCollision);}
        context.evm.load_account(target)?;
        if storage_nonempty(context,target){return fail(InstructionResult::CreateCollision);}
        let checkpoint=match context.evm.journaled_state.create_account_checkpoint(inputs.caller,target,inputs.value,revm::primitives::SpecId::CANCUN){
            Ok(checkpoint)=>checkpoint,Err(result)=>return fail(result)
        };
        let hash=match inputs.scheme{CreateScheme::Create=>B256::ZERO,CreateScheme::Create2{..}=>keccak256(&inputs.init_code)};
        let contract=Contract::new(Bytes::new(),Bytecode::new_legacy(inputs.init_code.clone()),Some(hash),target,None,inputs.caller,inputs.value);
        Ok(FrameOrResult::new_create_frame(target,checkpoint,Interpreter::new(contract,inputs.gas_limit,false)))
    });
}
