//! Actual source-built Registry/Caps/bond-bridge EVM calls; full apply NOT PASSED.
use pocol_lp4_execution::{native,output,CHAIN_ID,pay::{registry_address,escrow_address}};
use revm::{DatabaseCommit,InMemoryDB,primitives::{Address,AccountInfo,Bytecode,Bytes,TxEnv,TxKind,U256,keccak256,ResultAndState}};
pub fn addr(n:u8)->Address{Address::from([n;20])}
pub fn system()->Address{Address::from_word((U256::from(1)<<160u32).wrapping_sub(U256::from(2)).into())}
pub fn caps()->Address{Address::from_word(U256::from(0xc0c004u64).into())}
pub fn artifact(name:&str)->Bytecode{
 let j:serde_json::Value=serde_json::from_str(include_str!("data/registry-payment.json")).unwrap();
 let s=j["output"]["contracts"]["RegistryPayment.sol"][name]["evm"]["deployedBytecode"]["object"].as_str().unwrap();
 Bytecode::new_raw(Bytes::from((0..s.len()).step_by(2).map(|i|u8::from_str_radix(&s[i..i+2],16).unwrap()).collect::<Vec<_>>()))
}
pub fn insert(db:&mut InMemoryDB,a:Address,balance:U256,nonce:u64,code:Bytecode){db.insert_account_info(a,AccountInfo::new(balance,nonce,code.hash_slow(),code));}
pub fn slot(id:Address)->U256{U256::from_be_bytes(keccak256([id.into_word().as_slice(),U256::from(7).to_be_bytes::<32>().as_slice()].concat()).0)}
pub fn base(n:u64)->U256{U256::from_be_bytes(keccak256(U256::from(n).to_be_bytes::<32>()).0)}
pub fn storage(db:&InMemoryDB,a:Address,k:U256)->U256{db.accounts[&a].storage.get(&k).copied().unwrap_or_default()}
pub fn put(db:&mut InMemoryDB,a:Address,k:U256,v:U256){db.insert_account_storage(a,k,v).unwrap();}
pub fn reg(db:&InMemoryDB,k:u64)->U256{storage(db,registry_address(),U256::from(k))}
pub fn selector(s:&str)->Vec<u8>{keccak256(s.as_bytes())[..4].to_vec()}
pub fn call(db:&InMemoryDB,caller:Address,to:Address,data:Vec<u8>,value:U256,h:u64)->ResultAndState{
 let tx=TxEnv{caller,transact_to:TxKind::Call(to),data:Bytes::from(data),value,gas_limit:30_000_000,gas_price:U256::ZERO,nonce:Some(db.accounts.get(&caller).map(|a|a.info.nonce).unwrap_or(0)),chain_id:Some(CHAIN_ID),..Default::default()};
 native::execute_at(db,tx,U256::from(h)).unwrap()
}
pub fn invoke(db:&mut InMemoryDB,caller:Address,sig:&str,arg:Option<Address>,value:U256,h:u64)->ResultAndState{
 let mut data=selector(sig);if let Some(a)=arg{data.extend(a.into_word());}
 let r=call(db,caller,registry_address(),data,value,h);db.commit(r.state.clone());r
}
pub fn phase(db:&mut InMemoryDB,h:u64){
 for sig in ["markClose()","beginBlock()"]{let r=invoke(db,system(),sig,None,U256::ZERO,h);assert!(r.result.is_success(),"{sig} at{h}: {:?} {:?}",r.result,output(&r.result));}
}
pub fn fixture()->InMemoryDB{
 let mut db=InMemoryDB::default();
 for a in [system(),addr(0x11),addr(0x12),addr(0x13),addr(0x14),addr(0x15)]{insert(&mut db,a,U256::from(1_000_000u64),1,Bytecode::default());}
 for a in [addr(0x31),addr(0x32),addr(0x33)]{insert(&mut db,a,U256::ZERO,1,Bytecode::default());}
 for (a,name) in [(registry_address(),"MinerRegistry"),(escrow_address(),"RewardEscrow"),(caps(),"ChainCaps")]{insert(&mut db,a,U256::ZERO,1,artifact(name));}
 // Synthetic complete CP inventory, not a ParamGate/profile acceptance fixture.
 for n in 0..52{put(&mut db,caps(),U256::from(n),U256::ZERO);}
 for (n,v) in [(11,2),(16,10),(17,4),(18,10),(19,1),(20,20),(21,0),(22,0),(23,16),(24,2),(25,3),(26,4),(27,3),(47,1000),(48,u64::MAX),(50,2)]{put(&mut db,caps(),U256::from(n),U256::from(v));}
 // One real active/HELD genesis-style record; bond already belongs to Escrow.
 let id=addr(0x11);let r=slot(id);
 for (k,v) in [(U256::ZERO,U256::from(1)),(U256::from(1),U256::from(1)),(U256::from(4),U256::from(10)),(base(5),U256::from_be_slice(id.as_slice())),(base(6),U256::from_be_slice(id.as_slice())),(r,U256::from(1)|(U256::from(1)<<16u32)),(r+U256::from(1),U256::from_be_slice(addr(0x31).as_slice())),(r+U256::from(2),U256::from(10))]{put(&mut db,registry_address(),k,v);}
 db.accounts.get_mut(&escrow_address()).unwrap().info.balance=U256::from(10);db
}
pub fn writes(r:&ResultAndState)->usize{r.state.get(&registry_address()).map(|a|a.storage.values().filter(|s|s.is_changed()).count()).unwrap_or(0)}
pub fn success(r:&ResultAndState){assert!(r.result.is_success(),"assertion: successful EVM call required; {:?}; {:?}",r.result,output(&r.result));}
pub fn error(r:&ResultAndState,sig:&str){assert!(!r.result.is_success());assert_eq!(output(&r.result),selector(sig));}
#[test]fn full_register_reward_exit_middle_claim_use_approved_ceilings_and_escrow(){
 let mut db=fixture();phase(&mut db,1);
 let r=invoke(&mut db,addr(0x12),"register(address)",Some(addr(0x32)),U256::from(10),1);success(&r);assert_eq!(writes(&r),10);
 assert_eq!(reg(&db,0),U256::from(2));assert_eq!(reg(&db,1),U256::from(2));assert_eq!(reg(&db,4),U256::from(20));assert_eq!(db.accounts[&registry_address()].info.balance,U256::ZERO);assert_eq!(db.accounts[&escrow_address()].info.balance,U256::from(20));
 let r=invoke(&mut db,addr(0x12),"setRewardAddress(address)",Some(addr(0x33)),U256::ZERO,1);success(&r);assert_eq!(writes(&r),2);
 // Exit first record, retaining the second active record; then append a third.
 let r=invoke(&mut db,addr(0x11),"deregister()",None,U256::ZERO,1);success(&r);assert!(writes(&r)<=7);
 let r=invoke(&mut db,addr(0x13),"register(address)",Some(addr(0x32)),U256::from(10),1);success(&r);
 phase(&mut db,3);let r=invoke(&mut db,addr(0x11),"claimBond()",None,U256::ZERO,3);error(&r,"BadState()");
 phase(&mut db,4);let before=db.accounts[&escrow_address()].info.balance;
 let r=invoke(&mut db,addr(0x11),"claimBond()",None,U256::ZERO,4);success(&r);assert_eq!(writes(&r),10);
 assert_eq!(db.accounts[&escrow_address()].info.balance,before-U256::from(10));assert_eq!(db.accounts[&addr(0x31)].info.balance,U256::from(10));assert_eq!(reg(&db,4),U256::from(20));assert_eq!(reg(&db,0),U256::from(2));assert_eq!(storage(&db,registry_address(),slot(addr(0x11))),U256::ZERO);
 assert_eq!(storage(&db,registry_address(),base(5)),U256::from_be_slice(addr(0x13).as_slice()));
 assert_eq!((storage(&db,registry_address(),slot(addr(0x13)))>>24u32)&U256::from(u32::MAX),U256::ZERO);
}
#[test]fn prefix_precedence_budgets_and_durable_probe_fail_without_state_commit(){
 let mut db=fixture();phase(&mut db,1);let absent=addr(0x44);
 let before=reg(&db,3);let held=reg(&db,4);
 let r=invoke(&mut db,addr(0x12),"register(address)",Some(absent),U256::ZERO,1);error(&r,"BadValue()");
 let r=invoke(&mut db,addr(0x12),"register(address)",Some(absent),U256::from(10),1);error(&r,"RewardNotDurable()");assert_eq!(reg(&db,3),before);assert_eq!(reg(&db,4),held);
 put(&mut db,caps(),U256::from(17),U256::ZERO);
 let r=invoke(&mut db,addr(0x12),"register(address)",Some(absent),U256::from(10),1);error(&r,"RegOpsLimit()");
 put(&mut db,caps(),U256::from(47),U256::from(2));phase(&mut db,2);
 let r=invoke(&mut db,addr(0x12),"register(address)",Some(absent),U256::ZERO,2);error(&r,"BadValue()");
 let r=invoke(&mut db,addr(0x12),"register(address)",Some(absent),U256::from(10),2);error(&r,"Closing()");
 let r=call(&db,addr(0x12),registry_address(),vec![0;35],U256::from(10),2);error(&r,"BadCalldata()");
}
#[test]fn reactivation_inactivity_and_closing_refunds_actual_record_amounts(){
 let mut db=fixture();phase(&mut db,1);
 success(&invoke(&mut db,addr(0x12),"register(address)",Some(addr(0x32)),U256::from(10),1));
 success(&invoke(&mut db,addr(0x12),"deregister()",None,U256::ZERO,1));
 phase(&mut db,2);error(&invoke(&mut db,addr(0x12),"reactivate()",None,U256::ZERO,2),"BadState()");
 phase(&mut db,3);let r=invoke(&mut db,addr(0x12),"reactivate()",None,U256::ZERO,3);success(&r);assert!(writes(&r)<=5);
 // graceStart3 makes inactivity eligible at6, equality included.
 phase(&mut db,5);error(&invoke(&mut db,addr(0x13),"removeInactive(address)",Some(addr(0x12)),U256::ZERO,5),"BadState()");
 phase(&mut db,6);let r=invoke(&mut db,addr(0x13),"removeInactive(address)",Some(addr(0x12)),U256::ZERO,6);success(&r);assert!(writes(&r)<=7);
 put(&mut db,caps(),U256::from(47),U256::from(7));phase(&mut db,7);
 assert_eq!(reg(&db,4),U256::ZERO);assert_eq!(db.accounts[&addr(0x31)].info.balance,U256::from(10));assert_eq!(db.accounts[&addr(0x32)].info.balance,U256::from(10));
 let paid=storage(&db,registry_address(),slot(addr(0x12)));assert_eq!((paid>>16u32)&U256::from(255),U256::from(3));
 phase(&mut db,8);assert_eq!(db.accounts[&addr(0x32)].info.balance,U256::from(10));
}

#[test]fn budgets_exact_limits_failed_calls_and_phase_replay_cannot_reset(){
 let mut db=fixture();put(&mut db,caps(),U256::from(23),U256::from(2));put(&mut db,caps(),U256::from(17),U256::from(1));phase(&mut db,1);
 error(&invoke(&mut db,addr(0x12),"register(address)",Some(addr(0x44)),U256::from(10),1),"RewardNotDurable()");
 success(&invoke(&mut db,addr(0x12),"register(address)",Some(addr(0x32)),U256::from(10),1));
 error(&invoke(&mut db,addr(0x13),"register(address)",Some(addr(0x33)),U256::from(10),1),"RegOpsLimit()");
 success(&invoke(&mut db,addr(0x11),"setRewardAddress(address)",Some(addr(0x33)),U256::ZERO,1));
 error(&invoke(&mut db,addr(0x11),"setRewardAddress(address)",Some(addr(0x31)),U256::ZERO,1),"RegOpsLimit()");
 assert_eq!(reg(&db,3)&U256::from(65535),U256::from(2));assert_eq!((reg(&db,3)>>16u32)&U256::from(255),U256::from(1));
 error(&invoke(&mut db,system(),"beginBlock()",None,U256::ZERO,1),"BadPhase()");
 error(&invoke(&mut db,system(),"markClose()",None,U256::ZERO,1),"BadPhase()");
 error(&invoke(&mut db,addr(0x11),"beginBlock()",None,U256::ZERO,2),"Unauthorized()");
 phase(&mut db,2);success(&invoke(&mut db,addr(0x13),"register(address)",Some(addr(0x33)),U256::from(10),2));
 let mut empty=fixture();put(&mut empty,caps(),U256::from(23),U256::ZERO);phase(&mut empty,1);
 error(&invoke(&mut empty,addr(0x11),"setRewardAddress(address)",Some(addr(0x33)),U256::ZERO,1),"RegOpsLimit()");
}
#[test]fn zero_bond_durable_code_only_and_balance_only_admission(){
 let mut db=fixture();put(&mut db,caps(),U256::from(16),U256::ZERO);put(&mut db,registry_address(),U256::from(4),U256::ZERO);put(&mut db,registry_address(),slot(addr(0x11))+U256::from(2),U256::ZERO);db.accounts.get_mut(&escrow_address()).unwrap().info.balance=U256::ZERO;phase(&mut db,1);
 for (dest,nonce,balance,code,ok) in [(addr(0x41),0,1,vec![],false),(addr(0x42),0,0,vec![],false),(addr(0x43),1,0,vec![],true),(addr(0x44),0,0,vec![0x5f,0x5f,0xfd],true)]{
  insert(&mut db,dest,U256::from(balance),nonce,Bytecode::new_raw(Bytes::from(code)));
  let id=if dest==addr(0x44){addr(0x13)}else{addr(0x12)};
  let before=reg(&db,3);let r=invoke(&mut db,id,"register(address)",Some(dest),U256::ZERO,1);
  if ok{success(&r);assert!(writes(&r)<=10);}else{error(&r,"RewardNotDurable()");assert_eq!(reg(&db,3),before);}
 }
 // Trap code must not be called on actual compiled-system closing refunds either.
 put(&mut db,caps(),U256::from(47),U256::from(2));phase(&mut db,2);phase(&mut db,3);assert_eq!(reg(&db,4),U256::ZERO);
}
#[test]fn bridge_authorization_and_missing_refund_destination_roll_back_all_effects(){
 let mut db=fixture();phase(&mut db,1);
 let data=[selector("releaseBond(address)"),addr(0x11).into_word().to_vec()].concat();
 for who in [addr(0x11),system()]{let r=call(&db,who,escrow_address(),data.clone(),U256::ZERO,1);error(&r,"Unauthorized()");}
 success(&invoke(&mut db,addr(0x12),"register(address)",Some(addr(0x32)),U256::from(10),1));
 success(&invoke(&mut db,addr(0x12),"deregister()",None,U256::ZERO,1));phase(&mut db,4);
 // Explicit corrupt/unadmitted record fixture: do not burn/skip/delete its bond.
 put(&mut db,registry_address(),slot(addr(0x12))+U256::from(1),U256::from_be_slice(addr(0x44).as_slice()));
 let before=db.clone();let r=invoke(&mut db,addr(0x12),"claimBond()",None,U256::ZERO,4);error(&r,"PaymentFailed()");
 assert_eq!(db.accounts[&registry_address()].storage,before.accounts[&registry_address()].storage);assert_eq!(db.accounts[&escrow_address()].info.balance,before.accounts[&escrow_address()].info.balance);
 // Closing first pays good member, then hits corrupt member: all earlier effects revert.
 put(&mut db,caps(),U256::from(47),U256::from(5));let before=db.clone();
 let r=invoke(&mut db,system(),"markClose()",None,U256::ZERO,5);error(&r,"PaymentFailed()");
 for a in [registry_address(),escrow_address(),addr(0x31),addr(0x32)]{assert_eq!(db.accounts[&a].storage,before.accounts[&a].storage);assert_eq!(db.accounts[&a].info.balance,before.accounts[&a].info.balance);}
}
#[test]fn closing_ret_counts_all_records_and_never_pays_twice(){
 let mut db=fixture();phase(&mut db,1);
 for (id,dest) in [(addr(0x12),addr(0x32)),(addr(0x13),addr(0x33)),(addr(0x14),addr(0x32))]{success(&invoke(&mut db,id,"register(address)",Some(dest),U256::from(10),1));}
 // A BURNED tomb, explicit execution fixture; its bond is already removed from heldTotal.
 let r=slot(addr(0x12));let old=storage(&db,registry_address(),r);put(&mut db,registry_address(),r,(old&!((U256::from(255))<<16u32))|(U256::from(2)<<16u32)|(U256::from(1)<<8u32));
 put(&mut db,registry_address(),U256::from(4),U256::from(30));
 put(&mut db,caps(),U256::from(47),U256::from(2));phase(&mut db,2);
 assert_eq!((reg(&db,3)>>90u32)&U256::from(u32::MAX),U256::from(2));assert_eq!(reg(&db,4),U256::from(20));
 assert_eq!(db.accounts[&addr(0x31)].info.balance,U256::from(10));assert_eq!(db.accounts[&addr(0x32)].info.balance,U256::ZERO);
 phase(&mut db,3);assert_eq!(reg(&db,4),U256::ZERO);assert_eq!(db.accounts[&addr(0x33)].info.balance,U256::from(10));assert_eq!(db.accounts[&addr(0x32)].info.balance,U256::from(10));
 phase(&mut db,4);assert_eq!(db.accounts[&addr(0x32)].info.balance,U256::from(10));
}
pub fn add_exited(db:&mut InMemoryDB,count:u64,due:u64){
 for i in 1..=count{
  let id=Address::from_word(U256::from(0x1000+i).into());let r=slot(id);
  for (k,v) in [(base(5)+U256::from(i),U256::from_be_slice(id.as_slice())),(r,U256::from(2)|(U256::from(1)<<16u32)|(U256::from(i)<<24u32)),(r+U256::from(1),U256::from_be_slice(addr(0x32).as_slice())),(r+U256::from(2),U256::from(10)),(r+U256::from(3),U256::from(due)<<64u32)]{put(db,registry_address(),k,v);}
 }
 put(db,registry_address(),U256::ZERO,U256::from(count+1));put(db,registry_address(),U256::from(4),U256::from((count+1)*10));db.accounts.get_mut(&escrow_address()).unwrap().info.balance=U256::from((count+1)*10);put(db,caps(),U256::from(20),U256::from(100));
}
#[test]fn actual_sweep_scan_and_deletion_caps_preserve_swapped_cursor(){
 let mut db=fixture();add_exited(&mut db,65,1000);put(&mut db,caps(),U256::from(21),U256::from(16));put(&mut db,caps(),U256::from(22),U256::from(64));phase(&mut db,1);assert_eq!(reg(&db,2),U256::from(64));assert_eq!(reg(&db,0),U256::from(66));
 let mut db=fixture();add_exited(&mut db,18,1);put(&mut db,caps(),U256::from(21),U256::from(16));put(&mut db,caps(),U256::from(22),U256::from(64));phase(&mut db,1);assert_eq!(reg(&db,0),U256::from(3));assert_eq!(reg(&db,4),U256::from(30));assert_eq!(db.accounts[&addr(0x32)].info.balance,U256::from(160));
 phase(&mut db,2);assert_eq!(reg(&db,0),U256::from(1));assert_eq!(reg(&db,4),U256::from(10));assert_eq!(db.accounts[&addr(0x32)].info.balance,U256::from(180));assert_eq!(reg(&db,2),U256::ZERO);
}

#[test]fn timestamp_closing_height_overflow_and_capacity_boundaries(){
 let mut db=fixture();put(&mut db,caps(),U256::from(48),U256::from(10));
 for (timestamp,closed) in [(9,false),(10,true),(11,true)]{
  let tx=TxEnv{caller:system(),transact_to:TxKind::Call(registry_address()),data:Bytes::from(selector("markClose()")),gas_limit:30_000_000,gas_price:U256::ZERO,nonce:Some(1),chain_id:Some(CHAIN_ID),..Default::default()};
  let r=native::execute_at_time(&db,tx,U256::from(1),U256::from(timestamp)).unwrap();success(&r);let mut post=db.clone();post.commit(r.state);
  assert_eq!((reg(&post,3)&(U256::from(1)<<89u32))!=U256::ZERO,closed);
  assert_eq!(reg(&post,4),if closed{U256::ZERO}else{U256::from(10)});
 }
 let tx=TxEnv{caller:system(),transact_to:TxKind::Call(registry_address()),data:Bytes::from(selector("markClose()")),gas_limit:30_000_000,gas_price:U256::ZERO,nonce:Some(1),chain_id:Some(CHAIN_ID),..Default::default()};
 error(&native::execute_at(&db,tx,U256::from(1)<<64u32).unwrap(),"HeightOverflow()");
 let mut db=fixture();phase(&mut db,1);put(&mut db,caps(),U256::from(18),U256::from(1));
 error(&invoke(&mut db,addr(0x12),"register(address)",Some(addr(0x32)),U256::from(10),1),"Capacity()");
 put(&mut db,caps(),U256::from(18),U256::from(10));put(&mut db,caps(),U256::from(20),U256::from(1));
 error(&invoke(&mut db,addr(0x12),"register(address)",Some(addr(0x32)),U256::from(10),1),"Capacity()");
 error(&invoke(&mut db,addr(0x11),"deregister()",None,U256::ZERO,1),"BadState()"); // M_min
}
#[test]fn zero_delay_and_tomb_cleanup_equality_preserve_unchanged_limits(){
 let mut db=fixture();put(&mut db,caps(),U256::from(24),U256::ZERO);put(&mut db,caps(),U256::from(27),U256::ZERO);phase(&mut db,1);
 success(&invoke(&mut db,addr(0x12),"register(address)",Some(addr(0x32)),U256::from(10),1));
 success(&invoke(&mut db,addr(0x12),"deregister()",None,U256::ZERO,1));success(&invoke(&mut db,addr(0x12),"reactivate()",None,U256::ZERO,1));
 success(&invoke(&mut db,addr(0x12),"deregister()",None,U256::ZERO,1));success(&invoke(&mut db,addr(0x12),"claimBond()",None,U256::ZERO,1));
 let mut db=fixture();add_exited(&mut db,1,1000);let id=Address::from_word(U256::from(0x1001).into());let r=slot(id);let w=storage(&db,registry_address(),r);
 put(&mut db,registry_address(),r,(w&!(U256::from(255)<<16u32))|(U256::from(2)<<16u32)|(U256::from(1)<<8u32));
 put(&mut db,registry_address(),r+U256::from(3),U256::from(1)<<128u32);put(&mut db,registry_address(),U256::from(4),U256::from(10));
 put(&mut db,caps(),U256::from(21),U256::from(16));put(&mut db,caps(),U256::from(22),U256::from(64));
 phase(&mut db,7);assert_eq!(reg(&db,0),U256::from(2));phase(&mut db,8);assert_eq!(reg(&db,0),U256::from(1));assert_eq!(reg(&db,4),U256::from(10)); // burn1+E4+D2+1=8
}

#[test]fn strict_calldata_status_exclusions_and_maximum_bond_failure_are_atomic(){
 let mut db=fixture();phase(&mut db,1);
 for state in [2u64,3]{
  let mut bad=db.clone();let r=slot(addr(0x11));let w=storage(&bad,registry_address(),r);
  put(&mut bad,registry_address(),r,(w&!(U256::from(255)<<16u32))|(U256::from(state)<<16u32));
  for sig in ["setRewardAddress(address)","deregister()","reactivate()","claimBond()"]{
   let arg=if sig.contains("address"){Some(addr(0x32))}else{None};error(&invoke(&mut bad,addr(0x11),sig,arg,U256::ZERO,1),"BadState()");
  }
 }
 let mut data=[selector("register(address)"),addr(0x32).into_word().to_vec()].concat();
 for malformed in [data[..35].to_vec(),[data.clone(),vec![0]].concat()]{error(&call(&db,addr(0x12),registry_address(),malformed,U256::ZERO,1),"BadCalldata()");}
 data[4]=1;error(&call(&db,addr(0x12),registry_address(),data,U256::ZERO,1),"BadCalldata()");
 error(&invoke(&mut db,system(),"register(address)",Some(addr(0x32)),U256::from(10),1),"BadState()");
 // Explicit unadmitted maximum-value execution fixture, not accepted supply/profile.
 put(&mut db,caps(),U256::from(16),U256::MAX);db.accounts.get_mut(&addr(0x12)).unwrap().info.balance=U256::MAX;
 let before=db.clone();let r=invoke(&mut db,addr(0x12),"register(address)",Some(addr(0x32)),U256::MAX,1);error(&r,"PaymentFailed()");
 for a in [registry_address(),escrow_address(),addr(0x12)]{assert_eq!(db.accounts[&a].info.balance,before.accounts[&a].info.balance);assert_eq!(db.accounts[&a].storage,before.accounts[&a].storage);}
}
