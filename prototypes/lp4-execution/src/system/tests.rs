//! Synthetic profile controls, not complete block/GenesisBuilder acceptance.
use super::*;
use crate::{profile::{ProfileKind,NodeKind},genesis_spec as gs};
use revm::primitives::{AccountInfo,Bytecode,B256,keccak256};
pub(super) fn raw(s:&str)->Vec<u8>{s.as_bytes().chunks(2).map(|p|u8::from_str_radix(std::str::from_utf8(p).unwrap(),16).unwrap()).collect()}
pub(super) fn profile(budget:u64)->ProfileIdentity{
 let v:serde_json::Value=serde_json::from_str(include_str!("../../tests/data/code-caps-native-views.json")).unwrap();
 let mut spec=gs::decode(&raw(v["cases"][0]["genesisSpec"].as_str().unwrap())).unwrap();
 spec.cp[44]=crate::fixed::U256::from_u64(budget);
 let bytes=gs::encode(&spec);ProfileIdentity::validate(&bytes,&gs::genesis_hash(&bytes),777902,ProfileKind::Synthetic,NodeKind::Simnet,&[(1,0)]).unwrap()
}
pub(super) fn block(p:&ProfileIdentity)->NativeBlockContext{NativeBlockContext{height:U256::from(1),timestamp:U256::ZERO,gas_limit:U256::from(p.cp().GAS_LIMIT),base_fee:U256::from(100),prevrandao:B256::ZERO}}
pub(super) fn insert(db:&mut InMemoryDB,a:Address,code:Bytecode,balance:u64){db.insert_account_info(a,AccountInfo::new(U256::from(balance),1,code.hash_slow(),code));}
pub(super) fn artifact(file:&str,name:&str)->Bytecode{
 let v:serde_json::Value=serde_json::from_str(match file{"ChunkFactory.sol"=>include_str!("../../tests/data/chunk-factory.json"),"FinalFundingWitness.sol"=>include_str!("../../tests/data/pending-escrow.json"),"PendingEscrow.sol"=>include_str!("../../tests/data/pending-escrow.json"),"SystemBoundaryWitness.sol"=>include_str!("../../tests/data/system-boundary-witness.json"),"SystemEpochWitness.sol"=>include_str!("../../tests/data/system-epoch-witness.json"),_=>include_str!("../../tests/data/registry-payment.json")}).unwrap();
 Bytecode::new_raw(Bytes::from(raw(v["output"]["contracts"][file][name]["evm"]["deployedBytecode"]["object"].as_str().unwrap())))
}
fn target()->Address{Address::from([0x22;20])}
fn fixture()->InMemoryDB{let mut db=InMemoryDB::default();insert(&mut db,target(),artifact("SystemEpochWitness.sol","SystemEpochWitness"),0);db.insert_account_storage(target(),U256::ZERO,U256::from(11)).unwrap();db}
pub(super) fn data(s:&str)->Bytes{Bytes::from(keccak256(s.as_bytes())[..4].to_vec())}
pub(super) fn storage(db:&InMemoryDB,to:Address,slot:u64)->U256{db.accounts[&to].storage.get(&U256::from(slot)).copied().unwrap_or_default()}
pub(super) fn root(db:&InMemoryDB)->B256{
 let n=|v:U256|crate::fixed::U256::from_be_slice(&v.to_be_bytes::<32>()).unwrap();let mut accounts=Vec::new();
 for (addr,a) in &db.accounts{if a.account_state==revm::db::AccountState::NotExisting{continue}
  let slots:Vec<_>=a.storage.iter().filter(|(_,v)|**v!=U256::ZERO).map(|(k,v)|(n(*k),n(*v))).collect();
  accounts.push(crate::trie::Account{address:addr.0.0,nonce:a.info.nonce,balance:n(a.info.balance),storage_root:crate::trie::storage_root(&slots,slots.len()).unwrap(),code_hash:a.info.code_hash.0});
 }B256::from(crate::trie::state_root(&accounts,accounts.len()).unwrap())
}
#[test]fn fresh_epochs_match_independent_compiled_witness_gas_and_root(){
 let p=profile(300_000_000);assert!(!p.bootable());let b=block(&p);let mut db=fixture();insert(&mut db,system_address(),Bytecode::default(),1_000_000);
 let before=format!("{db:?}");let mut c=Candidate::new(&db,&p,&b,Proposer::Signed(Address::from([0x11;20]))).unwrap();
 let cases=[("readSlot()",2323,0),("readSlot()",2323,0),("setTransient()",0,0),("readTransient()",0,0),("persistTransient()",0,0),("clearSlot()",5158,4800),("restoreSlot()",22281,0)];
 for (name,gas,refund) in cases{let result=c.call(target(),data(name)).unwrap();if gas!=0{assert_eq!(result.raw_gas,gas,"{name}")}assert_eq!(result.refund_counter,refund,"{name}");}
 assert_eq!(storage(c.snapshot().unwrap(),target(),1),U256::ZERO);
 assert_eq!(c.snapshot().unwrap().accounts[&system_address()].info.nonce,1);
 assert_eq!(c.snapshot().unwrap().accounts[&system_address()].info.balance,U256::from(1_000_000));
 assert_eq!(root(c.snapshot().unwrap()),B256::from_slice(&raw("499340f755645e065240edf8be10c9b87e4d9d90200c9deadcb14a450fb0ed01")));
 assert_eq!(format!("{db:?}"),before);
}
#[test]fn system_origin_and_caller_do_not_create_origin_or_system_accounts(){
 let p=profile(30_000_000);let b=block(&p);let db=fixture();
 for proposer in [Proposer::Fallback,Proposer::Signed(Address::from([0x11;20])),Proposer::Signed(system_address())]{
  let origin=proposer.origin().unwrap();let mut c=Candidate::new(&db,&p,&b,proposer).unwrap();let r=c.call(target(),data("originAndCaller()")).unwrap();
  assert_eq!(r.output.len(),64);assert_eq!(&r.output[..32],origin.into_word().as_slice());assert_eq!(&r.output[32..],system_address().into_word().as_slice());
  assert!(!c.snapshot().unwrap().accounts.contains_key(&system_address()));
  if origin!=Address::ZERO{assert!(!c.snapshot().unwrap().accounts.contains_key(&origin));}
 }
 assert!(matches!(Candidate::new(&db,&p,&b,Proposer::Signed(Address::ZERO)),Err(SystemError::Metadata)));
}
#[test]fn raw_budget_exact_minus_one_remaining_zero_and_refund_no_credit(){
 let db=fixture();
 for budget in [2322,2323,4645,4646]{let p=profile(budget);let b=block(&p);let mut c=Candidate::new(&db,&p,&b,Proposer::Fallback).unwrap();
  let first=c.call(target(),data("readSlot()"));if budget<2323{assert!(first.is_err());continue}assert_eq!(first.unwrap().raw_gas,2323);
  let second=c.call(target(),data("readSlot()"));if budget<4646{assert!(second.is_err());assert!(matches!(c.snapshot(),Err(SystemError::Poisoned)));}else{assert_eq!(second.unwrap().raw_gas,2323);assert_eq!(c.used(),4646);assert!(matches!(c.call(target(),data("readSlot()")),Err(SystemError::Budget)));}
 }
 let p=profile(5158);let b=block(&p);let mut c=Candidate::new(&db,&p,&b,Proposer::Fallback).unwrap();let r=c.call(target(),data("clearSlot()")).unwrap();assert_eq!((r.raw_gas,r.refund_counter),(5158,4800));assert_eq!(c.used(),5158);assert!(matches!(c.call(target(),data("readSlot()")),Err(SystemError::Budget)));
}
#[test]fn later_revert_or_log_invalidates_all_candidate_effects(){
 let p=profile(30_000_000);let b=block(&p);let db=fixture();let original=root(&db);
 for code in [vec![0x5f,0x5f,0xfd],vec![0x5f,0x5f,0xa0,0x00]]{
  let bad=Address::from([0x33;20]);let mut base=db.clone();insert(&mut base,bad,Bytecode::new_raw(Bytes::from(code)),0);
  let mut c=Candidate::new(&base,&p,&b,Proposer::Fallback).unwrap();c.call(target(),data("clearSlot()")).unwrap();assert_eq!(storage(c.snapshot().unwrap(),target(),0),U256::ZERO);
  assert!(c.call(bad,Bytes::new()).is_err());assert!(matches!(c.snapshot(),Err(SystemError::Poisoned)));assert!(matches!(c.call(target(),data("restoreSlot()")),Err(SystemError::Poisoned)));
 }assert_eq!(root(&db),original);
}
#[test]fn independent_pending_encoder_and_invalid_context(){
 let a=Address::from([0x11;20]);let signed=Proposer::Signed(a).pending_word().unwrap().to_be_bytes::<32>();let mut expected=[0u8;32];expected[11..31].copy_from_slice(a.as_slice());assert_eq!(signed,expected);
 let fallback=Proposer::Fallback.pending_word().unwrap().to_be_bytes::<32>();let mut expected=[0u8;32];expected[31]=1;assert_eq!(fallback,expected);
 let p=profile(30_000_000);let mut b=block(&p);b.gas_limit+=U256::from(1);assert!(matches!(Candidate::new(&fixture(),&p,&b,Proposer::Fallback),Err(SystemError::Context)));b=block(&p);b.height=U256::ZERO;assert!(matches!(Candidate::new(&fixture(),&p,&b,Proposer::Fallback),Err(SystemError::Context)));
}
fn load_state(v:&serde_json::Value)->InMemoryDB{
 let mut db=InMemoryDB::default();for (address,a) in v.as_object().unwrap(){let to=Address::from_slice(&raw(address.trim_start_matches("0x")));let code=Bytecode::new_raw(Bytes::from(raw(a["code"].as_str().unwrap().trim_start_matches("0x"))));
 let number=|v:&serde_json::Value|{let s=v.as_str().map(str::to_owned).unwrap_or_else(||v.to_string());if let Some(s)=s.strip_prefix("0x"){U256::from_str_radix(s,16).unwrap()}else{U256::from_str_radix(&s,10).unwrap()}};
 db.insert_account_info(to,AccountInfo::new(number(&a["balance"]),number(&a["nonce"]).try_into().unwrap(),code.hash_slow(),code));
 for (key,value) in a["storage"].as_object().unwrap(){db.insert_account_storage(to,U256::from_str_radix(key.trim_start_matches("0x"),16).unwrap(),number(value)).unwrap();}
 }db
}
pub(super) fn state(db:&InMemoryDB)->serde_json::Value{
 let mut out=serde_json::Map::new();for (address,a) in &db.accounts{if a.account_state==revm::db::AccountState::NotExisting{continue}
 let code=a.info.code.as_ref().or_else(||db.contracts.get(&a.info.code_hash)).unwrap().original_bytes();
 let slots:serde_json::Map<String,serde_json::Value>=a.storage.iter().filter(|(_,v)|**v!=U256::ZERO).map(|(k,v)|(format!("{k:#x}"),serde_json::json!(format!("{v:#x}")))).collect();
 out.insert(format!("{address:#x}"),serde_json::json!({"nonce":a.info.nonce,"balance":a.info.balance.to_string(),"code":format!("0x{}",code.iter().map(|b|format!("{b:02x}")).collect::<String>()),"storage":slots}));
 }serde_json::Value::Object(out)
}
fn registry_pre(p:&ProfileIdentity)->InMemoryDB{
 let v:serde_json::Value=serde_json::from_str(include_str!("../../tests/data/system-registry-pre.json")).unwrap();
 let mut db=load_state(&v);assert_eq!(db.accounts[&crate::pay::registry_address()].info.code_hash,artifact("RegistryPayment.sol","MinerRegistry").hash_slow());
 // Historical diagnostic supplied source-built code/record structure only.
 // Replace ALL52 CP slots, not merely parameters read by the selected method.
 let caps=Address::from_word(U256::from(0xc0c004u64).into());
 for (i,v) in p.spec().cp.iter().enumerate(){db.insert_account_storage(caps,U256::from(i),U256::from_be_bytes(v.to_be_bytes())).unwrap();}
 let bond=U256::from_be_bytes(p.spec().cp[16].to_be_bytes());
 let rec=U256::from_be_bytes(keccak256([Address::from([0x11;20]).into_word().as_slice(),U256::from(7).to_be_bytes::<32>().as_slice()].concat()).0);
 db.insert_account_storage(crate::pay::registry_address(),rec+U256::from(2),bond).unwrap();
 db.insert_account_storage(crate::pay::registry_address(),U256::from(4),bond).unwrap();
 db.accounts.get_mut(&crate::pay::escrow_address()).unwrap().info.balance=bond;db
}
#[test]fn actual_registry_system_calls_pay_and_caller_authorization(){
 let p=profile(300_000_000);let mut b=block(&p);b.height=U256::from(p.cp().H_CLOSE);let db=registry_pre(&p);let before=root(&db);
 let mut c=Candidate::new(&db,&p,&b,Proposer::Signed(Address::from([0x11;20]))).unwrap();
 c.call(crate::pay::registry_address(),data("markClose()")).unwrap();c.call(crate::pay::registry_address(),data("beginBlock()")).unwrap();
 assert_eq!(c.snapshot().unwrap().accounts[&crate::pay::escrow_address()].info.balance,U256::ZERO);
 assert_eq!(c.snapshot().unwrap().accounts[&Address::from([0x31;20])].info.balance,U256::from_be_bytes(p.spec().cp[16].to_be_bytes()));
 assert_eq!(c.snapshot().unwrap().accounts[&system_address()].info.nonce,1);
 assert_eq!(root(&db),before);
 // A USER whose ORIGIN is SYSTEM still has a non-SYSTEM CALLER in a nested
 // Registry call. It cannot borrow authorization from ORIGIN.
 let proxy=Address::from([0x44;20]);let mut spoof=db.clone();
 // Store markClose selector in memory and forward it via CALL, return success.
 let mut code=vec![0x63];code.extend(&data("markClose()"));code.extend([0x60,0xe0,0x1b,0x5f,0x52,0x5f,0x5f,0x60,4,0x5f,0x5f,0x62,0xc0,0xc0,1,0x62,0x0f,0x42,0x40,0xf1,0x5f,0x52,0x60,32,0x5f,0xf3]);
 insert(&mut spoof,proxy,Bytecode::new_raw(Bytes::from(code)),0);
 // SYSTEM here is just the old unsigned USER control's sender, not host SYS.
 let tx=TxEnv{caller:system_address(),transact_to:TxKind::Call(proxy),gas_limit:1_000_000,gas_price:U256::from(100),nonce:Some(1),chain_id:Some(777902),..Default::default()};
 spoof.accounts.get_mut(&system_address()).unwrap().info.balance=U256::from(1_000_000_000u64);
 let result=native::execute_for_profile(&spoof,tx,&b,&p).unwrap();assert!(result.result.is_success());assert_eq!(crate::output(&result.result),[0u8;32]);
}
#[test]fn user_origin_nonce_and_fees_remain_normal(){
 let p=profile(30_000_000);let b=block(&p);let mut db=fixture();let sender=Address::from([0x11;20]);insert(&mut db,sender,Bytecode::default(),1_000_000_000);
 let tx=TxEnv{caller:sender,transact_to:TxKind::Call(target()),gas_limit:100_000,gas_price:U256::from(1000),nonce:Some(1),chain_id:Some(777902),data:data("originAndCaller()"),..Default::default()};
 let result=native::execute_for_profile(&db,tx,&b,&p).unwrap();assert!(result.result.is_success());let output=crate::output(&result.result);assert_eq!(&output[..32],sender.into_word().as_slice());assert_eq!(&output[32..],sender.into_word().as_slice());assert_eq!(result.state[&sender].info.nonce,2);
 let gas=result.result.gas_used();assert_eq!(result.state[&sender].info.balance,U256::from(1_000_000_000u64-gas*1000));assert_eq!(result.state[&crate::pay::escrow_address()].info.balance,U256::from(gas*900));assert_eq!(db.accounts[&sender].info.nonce,1);
}
#[test]fn independent_execution_report(){
 let p=profile(300_000_000);let b=block(&p);let mut db=fixture();insert(&mut db,system_address(),Bytecode::default(),1_000_000);
 let mut sequences=Vec::new();
 for (name,pre,proposer,methods,block) in [
  ("fresh-epochs",db.clone(),Proposer::Signed(Address::from([0x11;20])),vec!["readSlot()","readSlot()","setTransient()","readTransient()","persistTransient()","clearSlot()","restoreSlot()"],block(&p)),
  ("signed-origin",fixture(),Proposer::Signed(Address::from([0x11;20])),vec!["originAndCaller()"],block(&p)),
  ("fallback-origin",fixture(),Proposer::Fallback,vec!["originAndCaller()"],block(&p)),
  ("compiled-registry",registry_pre(&p),Proposer::Signed(Address::from([0x11;20])),vec!["markClose()","beginBlock()"],block(&p)),
  ("compiled-registry-closing-pay",registry_pre(&p),Proposer::Fallback,vec!["markClose()","beginBlock()"],{let mut b=block(&p);b.height=U256::from(p.cp().H_CLOSE);b}),
 ]{
  let to=if name.starts_with("compiled-registry"){crate::pay::registry_address()}else{target()};let mut c=Candidate::new(&pre,&p,&block,proposer).unwrap();let mut rows=Vec::new();
  for method in methods{let input=data(method);let result=c.call(to,input.clone()).unwrap();let after=c.snapshot().unwrap();rows.push(serde_json::json!({"method":method,"to":format!("{to:#x}"),"input":format!("0x{}",input.iter().map(|b|format!("{b:02x}")).collect::<String>()),"rawGas":result.raw_gas,"refundCounter":result.refund_counter,"output":format!("0x{}",result.output.iter().map(|b|format!("{b:02x}")).collect::<String>()),"stateRoot":format!("{:#x}",root(after)),"post":state(after),"systemUsed":c.used()}));}
  sequences.push(serde_json::json!({"name":name,"pre":state(&pre),"origin":format!("{:#x}",proposer.origin().unwrap()),"height":block.height.to_string(),"gasLimit":block.gas_limit.to_string(),"baseFee":block.base_fee.to_string(),"budget":p.cp().G_SYS,"calls":rows}));
 }
 let report=serde_json::json!({"scope":"LP4-SYS-01 internal frame primitive, actual Registry/bridge and synthetic profile controls; validated UT/phases/complete programs/GenesisBuilder/resources NOT PASSED","semanticVersion":"LP4-SYS-01","genesisHash":format!("0x{}",crate::hex::encode(&p.genesis_hash())),"profileKind":"Synthetic","nodeKind":"Simnet","forkSchedule":[[1,0]],"resourceModel":"sys-frame-measurement-v1","genesisSpec":crate::hex::encode(&gs::encode(p.spec())),"bootable":false,"sequences":sequences});
 assert_eq!(report["sequences"].as_array().unwrap().iter().map(|s|s["calls"].as_array().unwrap().len()).sum::<usize>(),13);
 if let Some(path)=std::env::var_os("LP4_SYS_REPORT_PATH"){std::fs::write(path,serde_json::to_string_pretty(&report).unwrap()+"\n").unwrap();}
 let _=b;
}
#[test]fn allocation_is_minimum_of_30m_and_remaining_not_block_gaslimit(){
 // GAS returns allowance-2 before memory/store/return. Synthetic CP GAS_LIMIT8M
 // stays visible, even when message allocation is30M and BASEFEE is100.
 for budget in [0,99_999,30_000_000,30_000_001,u64::MAX]{
  let p=profile(budget);let b=block(&p);let mut db=fixture();let probe=Address::from([0x55;20]);
  // GAS; MSTORE(0); GASLIMIT; MSTORE(32); BASEFEE; MSTORE(64); GASPRICE; MSTORE(96)
  let code=raw("5a5f5245602052486040523a60605260805ff3");insert(&mut db,probe,Bytecode::new_raw(Bytes::from(code)),0);
  let mut c=Candidate::new(&db,&p,&b,Proposer::Fallback).unwrap();let r=c.call(probe,Bytes::new());
  if budget==0{assert!(matches!(r,Err(SystemError::Budget)));continue}
  let r=r.unwrap();assert_eq!(U256::from_be_slice(&r.output[..32]),U256::from(budget.min(30_000_000)-2));assert_eq!(U256::from_be_slice(&r.output[32..64]),b.gas_limit);assert_eq!(U256::from_be_slice(&r.output[64..96]),b.base_fee);assert_eq!(U256::from_be_slice(&r.output[96..]),U256::ZERO);
  let remaining=budget-r.raw_gas;let second=c.call(probe,Bytes::new()).unwrap();assert_eq!(U256::from_be_slice(&second.output[..32]),U256::from(remaining.min(30_000_000)-2));
 }
}
fn boundary_target()->Address{Address::from([0x66;20])}
fn boundary_db()->InMemoryDB{let mut db=fixture();insert(&mut db,boundary_target(),artifact("SystemBoundaryWitness.sol","SystemBoundaryWitness"),0);db}
fn forward(requested:u64,required:u64,abort_child:bool,abort_outer:bool)->Bytes{
 let mut input=data("forward(uint256,uint256,bool,bool)").to_vec();for n in [requested,required,abort_child as u64,abort_outer as u64]{input.extend(U256::from(n).to_be_bytes::<32>());}Bytes::from(input)
}
fn word(bytes:&[u8],n:usize)->U256{U256::from_be_slice(&bytes[n*32..(n+1)*32])}
#[test]fn compiled_nested_eip150_and_nested_revert_boundaries(){
 let db=boundary_db();let original=root(&db);let input=forward(30_000_000,100_000,false,false);
 let succeeds=|gas|{let p=profile(gas);let b=block(&p);Candidate::new(&db,&p,&b,Proposer::Fallback).unwrap().call(boundary_target(),input.clone()).map(|r|word(&r.output,0)==U256::from(1)).unwrap_or(false)};
 let(mut lo,mut hi)=(30_000u64,200_000u64);assert!(!succeeds(lo));assert!(succeeds(hi));
 while lo<hi{let mid=lo+(hi-lo)/2;if succeeds(mid){hi=mid}else{lo=mid+1}}
 let minimum=lo;let expected:serde_json::Value=serde_json::from_str(include_str!("../../tests/data/system-boundary-independent.json")).unwrap();assert_eq!(minimum,expected["minimumGas"].as_u64().unwrap());assert!(succeeds(minimum));assert!(!succeeds(minimum-1));
 let p=profile(30_000_000);let b=block(&p);let mut c=Candidate::new(&db,&p,&b,Proposer::Fallback).unwrap();
 let r=c.call(boundary_target(),forward(30_000_000,0,true,false)).unwrap();assert_eq!(word(&r.output,0),U256::ZERO);assert_eq!(word(&r.output,3),U256::ZERO);assert_eq!(word(&r.output,4),U256::ZERO);
 let r=c.call(boundary_target(),forward(30_000_000,100_000,false,false)).unwrap();assert_eq!(word(&r.output,0),U256::from(1));assert!(word(&r.output,2)>=U256::from(100_000));assert!(word(&r.output,2)<word(&r.output,1));assert_eq!(word(&r.output,3),U256::from(9));assert_eq!(word(&r.output,4),U256::from(7));
 let r=c.call(boundary_target(),forward(30_000_000,0,true,false)).unwrap();assert_eq!(word(&r.output,3),U256::from(9));assert_eq!(word(&r.output,4),U256::ZERO);
 assert!(c.call(boundary_target(),forward(30_000_000,0,false,true)).is_err());assert!(matches!(c.snapshot(),Err(SystemError::Poisoned)));assert_eq!(root(&db),original);
}
#[test]fn issuance_timing_is_observable_without_selecting_a_rule(){
 // Two countermodels only: no host mint operation is installed. Both credit the
 // same subsidy once. The USER instruction BALANCE sees different phase timing.
 let p=profile(30_000_000);let mut b=block(&p);b.base_fee=U256::ZERO;let mut base=boundary_db();let sender=Address::from([0x11;20]);insert(&mut base,sender,Bytecode::default(),1_000_000);
 insert(&mut base,crate::pay::escrow_address(),artifact("RegistryPayment.sol","RewardEscrow"),1000);
 let subsidy=U256::from_be_bytes(p.spec().cp[15].to_be_bytes());let mut rows=Vec::new();
 for early in [true,false]{let mut pre=base.clone();if early{let a=pre.accounts.get_mut(&crate::pay::escrow_address()).unwrap();a.info.balance=a.info.balance.checked_add(subsidy).unwrap();}
  let tx=TxEnv{caller:sender,transact_to:TxKind::Call(boundary_target()),gas_limit:100_000,gas_price:U256::ZERO,nonce:Some(1),chain_id:Some(777902),data:data("captureCoinbaseBalance()"),..Default::default()};
  let result=native::execute_for_profile(&pre,tx,&b,&p).unwrap();assert!(result.result.is_success());let mut post=pre.clone();commit_epoch(&mut post,result.state);
  let observed=storage(&post,boundary_target(),1);assert_eq!(observed,U256::from(1000)+if early{subsidy}else{U256::ZERO});
  if !early{let a=post.accounts.get_mut(&crate::pay::escrow_address()).unwrap();a.info.balance=a.info.balance.checked_add(subsidy).unwrap();}
  rows.push(serde_json::json!({"model":if early{"credit-before-USER"}else{"credit-after-USER-before-closeBlock"},"pre":state(&pre),"caller":format!("{sender:#x}"),"to":format!("{:#x}",boundary_target()),"input":format!("0x{}",crate::hex::encode(&data("captureCoinbaseBalance()"))),"gas":100000,"gasUsed":result.result.gas_used(),"subsidy":subsidy.to_string(),"observed":observed.to_string(),"post":state(&post),"stateRoot":format!("{:#x}",root(&post)),"approved":false}));
 }
 assert_ne!(rows[0]["stateRoot"],rows[1]["stateRoot"]);assert_eq!(rows[0]["gasUsed"],rows[1]["gasUsed"]);
 if let Some(path)=std::env::var_os("LP4_ISSUANCE_DIAGNOSTIC_PATH"){std::fs::write(path,serde_json::to_string_pretty(&serde_json::json!({"scope":"Unapproved issuance timing countermodels only; ordinary unsigned EVM transaction control, not verified PoCol block admission","semanticBase":"LP4-SYS-01","bootable":false,"genesisSpec":crate::hex::encode(&gs::encode(p.spec())),"genesisHash":format!("0x{}",crate::hex::encode(&p.genesis_hash())),"rows":rows})).unwrap()+"\n").unwrap();}
}
#[test]fn compiled_nested_matches_independent_complete_roots_and_exact_gas(){
 let expected:serde_json::Value=serde_json::from_str(include_str!("../../tests/data/system-boundary-independent.json")).unwrap();let db=boundary_db();assert_eq!(format!("{:#x}",root(&db)),expected["preRoot"].as_str().unwrap());
 for row in expected["cases"].as_array().unwrap(){let p=profile(row["gas"].as_u64().unwrap());let b=block(&p);let mut c=Candidate::new(&db,&p,&b,Proposer::Fallback).unwrap();let result=c.call(boundary_target(),forward(30_000_000,row["required"].as_u64().unwrap(),row["abortChild"].as_bool().unwrap(),row["abortOuter"].as_bool().unwrap()));
  if !row["frameSuccess"].as_bool().unwrap(){assert!(result.is_err());assert!(matches!(c.snapshot(),Err(SystemError::Poisoned)));continue}
  let result=result.unwrap();assert_eq!(result.raw_gas,row["rawGas"].as_u64().unwrap());assert_eq!(result.refund_counter,row["refundCounter"].as_i64().unwrap());assert_eq!(result.output.as_ref(),raw(row["output"].as_str().unwrap().trim_start_matches("0x")));assert_eq!(format!("{:#x}",root(c.snapshot().unwrap())),row["stateRoot"].as_str().unwrap());
 }assert_eq!(expected["cases"].as_array().unwrap().len(),5);
}
