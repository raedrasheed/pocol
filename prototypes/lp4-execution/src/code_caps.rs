//! LP4-CODE-01: development/LP4-CODE-APPROVAL.md.
//! Complete EmptyDB-backed inventory only; not a partial/persisted state proof.
use std::sync::Arc;
use revm::{Context, Database, InMemoryDB, handler::register::EvmHandler};
use crate::state_view::LocalState;
use revm::primitives::{AccountInfo, EVMError, KECCAK_EMPTY};
use revm::interpreter::InstructionResult;

#[derive(Clone, Copy, Debug)]
pub struct CodeLimits { pub runtime: u32, pub logical: u64 }

/// Resolve actual runtime bytes and verify their content address. An unknown
/// nonempty hash must never silently resolve to EmptyDB's empty-code fallback.
pub(crate) fn code_len(db:&dyn crate::view::View, info:&AccountInfo)->Result<u64,String> {
    let code=info.code.clone().or_else(||db.contract(&info.code_hash));
    match code {
        Some(code) if code.hash_slow()==info.code_hash =>
            u64::try_from(code.original_bytes().len()).map_err(|_|"code length overflow".into()),
        None if info.code_hash==KECCAK_EMPTY => Ok(0),
        None => Err("missing runtime bytes".into()),
        Some(_) => Err("runtime code hash mismatch".into()),
    }
}

/// Logical bytes per existing address, not unique code hashes. EmptyDB has no
/// hidden accounts: callers with a different backing store cannot use this API.
pub fn inventory(db:&dyn crate::view::View)->Result<u64,String> {
    let mut infos=Vec::new();db.each_account(&mut |_,a|{if let Some(i)=a.info(){infos.push(i)}});
    infos.iter().try_fold(0u64,|sum,info| {
        sum.checked_add(code_len(db,info)?).ok_or_else(||"logical code overflow".into())
    })
}

/// Logical code of the given addresses' current accounts (absent or NotExisting: 0).
pub fn code_of(db:&dyn crate::view::View,addresses:&[revm::primitives::Address])->Result<u64,String> {
    addresses.iter().filter_map(|a|db.account(a).and_then(|x|x.info())).try_fold(0u64,|sum,info|{
        sum.checked_add(code_len(db,&info)?).ok_or_else(||"logical code overflow".into())
    })
}
/// The checks of `validate_snapshot` for a state that passed it before and has since changed only at
/// `touched`, whose logical code was `before`; `total` is the inventory before the change. Returns the
/// new inventory. Untouched accounts are unchanged, so checking the touched ones and the updated total
/// gives the same Ok/Err, with the same texts, as a full scan (decision package C-2, item 2).
pub(crate) fn revalidate_touched(db:&dyn crate::view::View,limits:CodeLimits,touched:&[revm::primitives::Address],total:u64,before:u64)->Result<u64,String> {
    if limits.runtime<24577 {return Err("R9: B_code_max below 24577".into());}
    let mut after=0u64;
    for info in touched.iter().filter_map(|a|db.account(a).and_then(|x|x.info())) {
        let n=code_len(db,&info)?;
        if n>u64::from(limits.runtime) {return Err("snapshot exceeds B_code_max".into());}
        after=after.checked_add(n).ok_or_else(||"logical code overflow".to_string())?;
    }
    let total=total.checked_sub(before).and_then(|t|t.checked_add(after)).ok_or_else(||"logical code overflow".to_string())?;
    if total>limits.logical {return Err("snapshot exceeds CODE_CAP".into());}
    Ok(total)
}

/// `validate_snapshot`, then the inventory: the code total of a state that passes the limits.
pub fn checked_total(db:&InMemoryDB,limits:CodeLimits)->Result<u64,String> {validate_snapshot(db,limits)?;inventory(db)}

pub fn validate_snapshot(db:&dyn crate::view::View,limits:CodeLimits)->Result<(),String> {
    if limits.runtime<24577 {return Err("R9: B_code_max below 24577".into());}
    let mut infos=Vec::new();db.each_account(&mut |_,a|{if let Some(i)=a.info(){infos.push(i)}});
    for a in &infos {
        if code_len(db,a)?>u64::from(limits.runtime) {return Err("snapshot exceeds B_code_max".into());}
    }
    if inventory(db)?>limits.logical {return Err("snapshot exceeds CODE_CAP".into());}
    Ok(())
}

fn current_total<EXT,DB:Database+LocalState>(context:&Context<EXT,DB>)->Result<u64,String> {
    let db=context.evm.db.local();
    let state=&context.evm.journaled_state.state;
    // A known base total (decision package C-2, item 2): the base accounts outside the journal are
    // the base total less the base accounts the journal holds, so only journaled accounts are read.
    if let Some(total)=context.evm.db.code_total() {
        let mut sum=total;
        for address in state.keys() {
            if let Some(info)=db.account(address).and_then(|a|a.info()) {
                sum=sum.checked_sub(code_len(db,&info)?).ok_or_else(||"logical code total below a member".to_string())?;
            }
        }
        return state.values().try_fold(sum,|sum,a|sum.checked_add(code_len(db,&a.info)?).ok_or_else(||"logical code overflow".into()));
    }
    let mut base=Vec::new();db.each_account(&mut |address,a|{if !state.contains_key(address){if let Some(i)=a.info(){base.push(i)}}});
    // SELFDESTRUCT schedules end-of-transaction deletion. Its code remains
    // observable until that deletion, so no early credit is granted here.
    // After actual commit/removal the next complete inventory releases charge.
    base.into_iter().chain(state.values().map(|a|a.info.clone())).try_fold(0u64,|sum,info| {
        sum.checked_add(code_len(db,&info)?).ok_or_else(||"logical code overflow".into())
    })
}

/// Keep ordinary Cancun configuration during validation AND opcode execution.
/// Only the existing deposit handler sees the amended runtime limit. This leaves
/// EIP3860's 49152 limit, word gas, and exact error ordering untouched, without
/// copying or modifying the interpreter or its transaction validator.
pub fn register<EXT:'static,DB:Database+LocalState>(handler:&mut EvmHandler<'_,EXT,DB>,limits:CodeLimits) {
    assert_eq!(handler.cfg.spec_id,revm::primitives::SpecId::CANCUN);
    let previous=handler.execution.create_return.clone();
    handler.execution.create_return=Arc::new(move |context,frame,mut result| {
        if result.result.is_ok() && result.output.first()!=Some(&0xef)
            && result.output.len()<=limits.runtime as usize {
            let total=current_total(context).map_err(EVMError::Custom)?;
            let old=context.evm.journaled_state.state.get(&frame.created_address)
                .map(|a|code_len(context.evm.db.local(),&a.info)).transpose()
                .map_err(EVMError::Custom)?.unwrap_or(0);
            let proposed=total.checked_sub(old).and_then(|n|n.checked_add(result.output.len() as u64))
                .ok_or_else(||EVMError::Custom("logical code overflow".into()))?;
            if proposed>limits.logical {
                // The upstream failure path reverts the create checkpoint;
                // enclosing frames retain Cancun create-failure/nonce/gas rules.
                result.result=InstructionResult::CreateContractSizeLimit;
            }
        }
        let saved=context.evm.env.cfg.limit_contract_code_size;
        context.evm.env.cfg.limit_contract_code_size=Some(limits.runtime as usize);
        let outcome=previous(context,frame,result);
        context.evm.env.cfg.limit_contract_code_size=saved;
        outcome
    });
}
