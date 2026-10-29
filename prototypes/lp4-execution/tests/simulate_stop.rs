//! RPC simulation cancellation (network.md RpcGuard (و), :286): `native::simulate_for_profile` polls its
//! stop hook at least every STOP_GAS gas of a frame, ends the execution when the hook says so, and
//! otherwise returns exactly what block execution returns. A precompile call is not interpreted, so it
//! is never polled (pocold reports CANCEL_MAX as not enforced for that reason).
#[path="support/admission.rs"] mod f;
use pocol_lp4_execution::native::{self,NativeBlockContext,STOP_GAS,SIMULATION_STOPPED};
use revm::{InMemoryDB,primitives::{Address,AccountInfo,Bytecode,Bytes,B256,EVMError,ExecutionResult,U256,TxEnv,TxKind}};
use std::{cell::Cell,rc::Rc};

fn setup(code:&[u8])->(InMemoryDB,Address,Address,NativeBlockContext){
    let mut db=InMemoryDB::default();let (caller,target)=(Address::from([0x11;20]),Address::from([0x33;20]));let b=Bytecode::new_raw(Bytes::copy_from_slice(code));
    db.insert_account_info(caller,AccountInfo::new(U256::ZERO,0,Bytecode::default().hash_slow(),Bytecode::default()));
    db.insert_account_info(target,AccountInfo::new(U256::ZERO,1,b.hash_slow(),b));
    (db,caller,target,NativeBlockContext{height:U256::from(1),timestamp:U256::from(1),gas_limit:U256::from(30_000_000u64),base_fee:U256::ZERO,prevrandao:B256::ZERO})
}
fn tx(caller:Address,to:Address,data:Vec<u8>,gas:u64)->TxEnv{TxEnv{caller,transact_to:TxKind::Call(to),data:Bytes::from(data),gas_limit:gas,gas_price:U256::ZERO,chain_id:Some(777902),..Default::default()}}
/// A stop hook that never stops and counts its polls.
fn counting()->(Rc<Cell<u64>>,Rc<dyn Fn()->bool>){let n=Rc::new(Cell::new(0u64));let m=n.clone();(n,Rc::new(move||{m.set(m.get()+1);false}))}

/// A loop whose body hashes 512 KiB (about 98k gas per iteration): 64 iterations are about 6.3M gas,
/// so a poll every 64th JUMP alone would leave more than 6·STOP_GAS between polls. The gas rule polls
/// at least once per STOP_GAS of the frame. The unstopped result equals block execution's.
#[test] fn the_stop_hook_is_polled_at_least_every_stop_gas(){
    let p=f::profile(777902,None);
    // JUMPDEST; PUSH3 0x080000; PUSH1 0; KECCAK256; POP; PUSH1 0; JUMP — runs until out of gas.
    let (db,caller,target,block)=setup(&[0x5b,0x62,0x08,0x00,0x00,0x60,0x00,0x20,0x50,0x60,0x00,0x56]);
    let (polls,stop)=counting();
    let r=native::simulate_for_profile(&db,tx(caller,target,vec![],30_000_000),&block,&p,stop).unwrap();
    let gas=r.result.gas_used();assert!(matches!(r.result,ExecutionResult::Halt{..}),"the loop runs out of gas: {:?}",r.result);
    // Between two polls: at most STOP_GAS plus one iteration (~98k gas), so >= gas/1.1M - 1 polls;
    // a poll every 64th JUMP alone gives about gas/6.3M. Asserted with margin on both sides.
    assert!(polls.get()>=gas/(2*STOP_GAS),"{} polls for {gas} gas: about one per STOP_GAS ({STOP_GAS})",polls.get());
    let chain=native::execute_for_profile(&db,tx(caller,target,vec![],30_000_000),&block,&p).unwrap();
    assert_eq!(chain.result,r.result,"an unstopped simulation is block execution");
    println!("STOP-POLLS {}",serde_json::json!({"gasUsed":gas,"polls":polls.get(),"stopGas":STOP_GAS}));
}
/// The hook ends the execution at its first poll once it returns true: SIMULATION_STOPPED.
#[test] fn a_stop_ends_the_simulation(){
    let p=f::profile(777902,None);let (db,caller,target,block)=setup(&[0x5b,0x60,0x00,0x56]);// JUMPDEST; PUSH1 0; JUMP
    let n=Rc::new(Cell::new(0u32));let m=n.clone();let stop:Rc<dyn Fn()->bool>=Rc::new(move||{m.set(m.get()+1);m.get()>=3});
    let r=native::simulate_for_profile(&db,tx(caller,target,vec![],30_000_000),&block,&p,stop);
    assert!(matches!(&r,Err(EVMError::Custom(s)) if s==SIMULATION_STOPPED),"{r:?}");assert_eq!(n.get(),3,"stopped at the poll that said so");
}
/// A direct call to a precompile (BLAKE2F, 0x09) is not interpreted: the hook is never polled, so the
/// cancellation gap there is the whole precompile run (bounded only by the call's gas).
#[test] fn a_precompile_call_is_never_polled(){
    let p=f::profile(777902,None);let (db,caller,_,block)=setup(&[]);
    let mut input=1_000_000u32.to_be_bytes().to_vec();input.extend([0u8;64+128+16]);input.push(1);// rounds, h, m, t, f
    let (polls,stop)=counting();
    let r=native::simulate_for_profile(&db,tx(caller,Address::with_last_byte(9),input,2_000_000),&block,&p,stop).unwrap();
    assert!(r.result.is_success(),"{:?}",r.result);assert_eq!(polls.get(),0,"no poll inside a precompile");
}
