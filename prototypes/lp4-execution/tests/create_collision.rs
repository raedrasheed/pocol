//! Actual LP4-CREATE-01 execution boundaries, complete base storage incl unloaded keys.
use pocol_lp4_execution::{native,output,CHAIN_ID};
use revm::{InMemoryDB,primitives::{Address,AccountInfo,Bytecode,Bytes,TxEnv,TxKind,U256,ExecutionResult}};
fn user()->Address{Address::from([0x11;20])}
fn factory()->Address{Address::from([0x22;20])}
fn init()->Vec<u8>{vec![0x60,1,0x60,0,0x55,0x60,0,0x60,0,0xf3]}
fn put(db:&mut InMemoryDB,a:Address,nonce:u64,balance:u64,code:&[u8]){
 let b=Bytecode::new_raw(Bytes::copy_from_slice(code));db.insert_account_info(a,AccountInfo::new(U256::from(balance),nonce,b.hash_slow(),b));
}
fn setup(create2:bool,value:u8,revert:bool)->(InMemoryDB,Address,TxEnv){
 let mut db=InMemoryDB::default();put(&mut db,user(),1,1_000_000,&[]);
 let mut code=vec![0x36,0x5f,0x5f,0x37];
 if create2{code.extend([0x60,7]);}code.extend([0x36,0x5f,0x60,value,if create2{0xf5}else{0xf0}]);
 if revert{code.extend([0x5f,0x5f,0xfd]);}else{code.extend([0x5f,0x52,0x60,32,0x5f,0xf3]);}
 put(&mut db,factory(),1,100,&code);
 let target=if create2{factory().create2_from_code(U256::from(7).to_be_bytes::<32>(),&init())}else{factory().create(1)};
 put(&mut db,target,0,10,&[]);
 let tx=TxEnv{caller:user(),transact_to:TxKind::Call(factory()),data:Bytes::from(init()),gas_limit:500_000,gas_price:U256::ZERO,nonce:Some(1),chain_id:Some(CHAIN_ID),..Default::default()};(db,target,tx)
}
#[test] fn create_and_create2_block_unloaded_nonzero_and_allow_zero(){
 for create2 in [false,true]{for value in [0u64,1]{
  let (mut db,target,tx)=setup(create2,0,false);let key=U256::from(999_999);
  db.insert_account_storage(target,key,U256::from(value)).unwrap();
  let r=native::execute(&db,tx).unwrap();assert!(r.result.is_success());
  let returned=Address::from_word(revm::primitives::B256::from_slice(output(&r.result)));
  assert_eq!(returned,if value==0{target}else{Address::ZERO});
  assert_eq!(r.state[&factory()].info.nonce,2);
  if value==0{assert_eq!(r.state[&target].info.nonce,1);assert_eq!(r.state[&target].storage[&U256::ZERO].present_value(),U256::from(1));}
  else{assert_eq!(r.state[&target].info.nonce,0);assert_eq!(r.state[&target].info.balance,U256::from(10));}
  assert_eq!(db.accounts[&target].storage[&key],U256::from(value));assert_eq!(db.accounts[&factory()].info.nonce,1);
 }}
}
#[test] fn top_level_creation_collides_and_preserves_snapshot(){
 let (mut db,_,mut tx)=setup(false,0,false);let target=user().create(1);put(&mut db,target,0,10,&[]);
 db.insert_account_storage(target,U256::from(999),U256::from(1)).unwrap();tx.transact_to=TxKind::Create;
 let r=native::execute(&db,tx).unwrap();assert!(matches!(r.result,ExecutionResult::Halt{..}));assert_eq!(r.result.gas_used(),500_000);
 assert_eq!(r.state[&user()].info.nonce,2);assert_eq!(r.state[&target].info.nonce,0);assert_eq!(db.accounts[&user()].info.nonce,1);
}
#[test] fn funding_failure_precedes_collision_nonce_and_warmth(){
 for create2 in [false,true]{
  let (mut db,target,tx)=setup(create2,101,false);db.insert_account_storage(target,U256::from(999),U256::from(1)).unwrap();
  let r=native::execute(&db,tx).unwrap();assert!(r.result.is_success());assert_eq!(output(&r.result),[0u8;32]);
  assert_eq!(r.state[&factory()].info.nonce,1);assert!(!r.state.contains_key(&target));assert!(r.result.gas_used()<100_000);
 }
}
#[test] fn outer_revert_restores_factory_nonce_and_destination(){
 for create2 in [false,true]{for stored in [0u64,1]{
  let (mut db,target,tx)=setup(create2,0,true);db.insert_account_storage(target,U256::from(999),U256::from(stored)).unwrap();
  let r=native::execute(&db,tx).unwrap();assert!(matches!(r.result,ExecutionResult::Revert{..}));
  assert_eq!(r.state[&factory()].info.nonce,1);assert_eq!(r.state.get(&target).map(|a|a.info.nonce).unwrap_or(0),0);
  assert_eq!(db.accounts[&target].storage[&U256::from(999)],U256::from(stored));
 }}
}
#[test] fn complete_storage_overlay_and_rollback_facts(){
 let (mut db,target,_)=setup(false,0,false);
 db.insert_account_storage(target,U256::from(999),U256::from(1)).unwrap();
 db.insert_account_storage(target,U256::from(444),U256::from(2)).unwrap();
 let mut evm=revm::Evm::builder().with_db(db).build();
 {let inner=&mut evm.context.evm.inner;inner.journaled_state.load_account(target,&mut inner.db).unwrap();}
 assert!(evm.context.evm.journaled_state.state[&target].storage.is_empty());
 assert!(pocol_lp4_execution::collision::storage_nonempty(&evm.context,target));
 let checkpoint=evm.context.evm.journaled_state.checkpoint();
 for key in [999u64,444]{
  let inner=&mut evm.context.evm.inner;inner.journaled_state.sstore(target,U256::from(key),U256::ZERO,&mut inner.db).unwrap();
  assert_eq!(pocol_lp4_execution::collision::storage_nonempty(&evm.context,target),key!=444);
 }
 {let inner=&mut evm.context.evm.inner;inner.journaled_state.sstore(target,U256::from(333),U256::from(1),&mut inner.db).unwrap();}
 assert!(pocol_lp4_execution::collision::storage_nonempty(&evm.context,target));
 evm.context.evm.journaled_state.checkpoint_revert(checkpoint);
 assert!(pocol_lp4_execution::collision::storage_nonempty(&evm.context,target));
 assert_eq!(evm.context.evm.journaled_state.state[&target].storage[&U256::from(999)].present_value(),U256::from(1));
 assert_eq!(evm.context.evm.journaled_state.state[&target].storage[&U256::from(444)].present_value(),U256::from(2));
 // A new key in a newly-created account must be found even when its base
 // storage inventory is cleared by creation. This isolates the journal scan.
 let fresh=Address::from([0x66;20]);
 let created={let inner=&mut evm.context.evm.inner;
  inner.journaled_state.load_account(factory(),&mut inner.db).unwrap();
  inner.journaled_state.load_account(fresh,&mut inner.db).unwrap();
  let checkpoint=inner.journaled_state.create_account_checkpoint(factory(),fresh,U256::ZERO,revm::primitives::SpecId::CANCUN).unwrap();
  inner.journaled_state.sstore(fresh,U256::from(333),U256::from(1),&mut inner.db).unwrap();checkpoint
 };
 assert!(pocol_lp4_execution::collision::storage_nonempty(&evm.context,fresh));
 evm.context.evm.journaled_state.checkpoint_revert(created);
 assert!(!pocol_lp4_execution::collision::storage_nonempty(&evm.context,fresh));

}
#[test] fn existing_nonce_and_code_collision_remain_enforced(){
 for create2 in [false,true]{for code in [false,true]{
  let (mut db,target,tx)=setup(create2,0,false);put(&mut db,target,if code{0}else{1},10,if code{&[0x00]}else{&[]});
  let r=native::execute(&db,tx).unwrap();assert!(r.result.is_success());assert_eq!(output(&r.result),[0u8;32]);assert_eq!(r.state[&factory()].info.nonce,2);
 }}
}
#[test] fn collision_preserves_target_warmth(){
 for create2 in [false,true]{
  let (mut db,target,tx)=setup(create2,0,false);
  let mut code=db.contracts[&db.accounts[&factory()].info.code_hash].original_bytes().to_vec();code.truncate(code.len()-6);
  code.extend([0x50,0x5a,0x73]);code.extend(target.as_slice());code.extend([0x31,0x50,0x5a,0x90,0x03,0x5f,0x52,0x60,32,0x5f,0xf3]);
  put(&mut db,factory(),1,100,&code);db.insert_account_storage(target,U256::from(999),U256::from(1)).unwrap();
  let r=native::execute(&db,tx).unwrap();assert!(r.result.is_success());
  // PUSH20 3 + warm BALANCE 100 + POP 2 + GAS 2. Cold would cost 2607.
  assert_eq!(U256::from_be_slice(output(&r.result)),U256::from(107));
 }
}
#[test] fn depth_and_internal_nonce_limit_precede_storage_collision(){
 use revm::interpreter::{CreateInputs,CreateScheme,InstructionResult};
 for depth in [0u64,1025]{
  let (mut db,_,_)=setup(false,0,false);let nonce=if depth==0{u64::MAX}else{1};
  db.accounts.get_mut(&factory()).unwrap().info.nonce=nonce;let target=factory().create(nonce);put(&mut db,target,0,10,&[]);
  db.insert_account_storage(target,U256::from(999),U256::from(1)).unwrap();
  let mut evm=revm::Evm::builder().with_db(db).with_spec_id(revm::primitives::SpecId::CANCUN).append_handler_register(pocol_lp4_execution::collision::register).build();
  {let inner=&mut evm.context.evm.inner;inner.journaled_state.load_account(factory(),&mut inner.db).unwrap();}
  evm.context.evm.journaled_state.depth=depth as usize;
  let handler=evm.handler.execution.create.clone();let r=handler(&mut evm.context,Box::new(CreateInputs{caller:factory(),scheme:CreateScheme::Create,value:U256::ZERO,init_code:Bytes::from(init()),gas_limit:100_000})).unwrap();
  let revm::FrameOrResult::Result(r)=r else{panic!("limit must not open create frame")};
  assert_eq!(r.interpreter_result().result,if depth==0{InstructionResult::Return}else{InstructionResult::CallTooDeep});
  assert_eq!(evm.context.evm.journaled_state.state[&factory()].info.nonce,nonce);assert!(!evm.context.evm.journaled_state.state.contains_key(&target));
 }
}
#[test] fn reverted_init_code_restores_destination_but_consumes_factory_nonce(){
 let (mut db,target,mut tx)=setup(false,0,false);db.insert_account_storage(target,U256::from(999),U256::ZERO).unwrap();
 let mut code=init();*code.last_mut().unwrap()=0xfd;tx.data=Bytes::from(code);
 let r=native::execute(&db,tx).unwrap();assert!(r.result.is_success());assert_eq!(output(&r.result),[0u8;32]);
 assert_eq!(r.state[&factory()].info.nonce,2);assert_eq!(r.state.get(&target).map(|a|a.info.nonce).unwrap_or(0),0);
 assert!(r.state.get(&target).map(|a|a.storage.values().all(|s|s.present_value()==U256::ZERO)).unwrap_or(true));
}

#[test] fn storage_collision_precedes_destination_balance_overflow(){
 let (mut db,_,mut tx)=setup(false,0,false);let target=user().create(1);put(&mut db,target,0,10,&[]);
 db.accounts.get_mut(&target).unwrap().info.balance=U256::MAX;
 db.insert_account_storage(target,U256::from(999),U256::from(1)).unwrap();tx.transact_to=TxKind::Create;tx.value=U256::from(1);
 let r=native::execute(&db,tx).unwrap();
 assert!(matches!(r.result,ExecutionResult::Halt{reason:revm::primitives::HaltReason::CreateCollision,..}));
 assert_eq!(r.state[&target].info.balance,U256::MAX);assert_eq!(r.state[&user()].info.nonce,2);
}
