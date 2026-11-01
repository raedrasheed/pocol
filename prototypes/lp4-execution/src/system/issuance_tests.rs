//! Actual builder and signed USER/host/SYS execution; one manifest role is a
//! synthetic witness program. NOT five-program/UT/header/full LP4 acceptance.
use super::*;
use super::tests::{profile,block,raw,insert,artifact,data,storage,root,state};
use crate::{genesis_builder as gb,genesis_spec as gs,profile::{ProfileKind,NodeKind}};
use revm::primitives::{Bytecode,B256,keccak256};
fn identity(s:&gs::GenesisSpec)->ProfileIdentity{let b=gs::encode(s);ProfileIdentity::validate(&b,&gs::genesis_hash(&b),777902,ProfileKind::Synthetic,NodeKind::Simnet,&[(1,0)]).unwrap()}
fn programs()->[Bytes;5]{[
 artifact("RegistryPayment.sol","MinerRegistry").original_bytes(),artifact("PendingEscrow.sol","PendingEscrow").original_bytes(),
 artifact("SystemBoundaryWitness.sol","SystemBoundaryWitness").original_bytes(),artifact("RegistryPayment.sol","ChainCaps").original_bytes(),artifact("ChunkFactory.sol","ChunkFactory").original_bytes()]}
fn fixture(n:usize,subsidy:u64)->(ProfileIdentity,gb::GenesisImage,InMemoryDB,[Bytes;5]){
 let p=profile(300_000_000);let mut spec=p.spec().clone();spec.cp[15]=crate::fixed::U256::from_u64(subsidy);spec.cp[18]=crate::fixed::U256::from_u64(1024);spec.cp[19]=crate::fixed::U256::from_u64(1);
 spec.m0_list=(0..n).map(|i|gs::Member{id:Address::from_word(U256::from(0x10000usize+i).into()).0.0,reward_addr:Address::from_word(U256::from(0x20000usize+i).into()).0.0}).collect();
 let programs=programs();spec.sys_code_hash=gb::code_hash(&programs).0;
 let mut alloc=InMemoryDB::default();for m in &spec.m0_list{insert(&mut alloc,Address::from(m.reward_addr),Bytecode::default(),0)}
 let signed:serde_json::Value=serde_json::from_str(include_str!("../../tests/data/issuance-signed.json")).unwrap();let sender=Address::from_slice(&raw(signed["rows"][0]["sender"].as_str().unwrap()));insert(&mut alloc,sender,Bytecode::default(),1_000_000_000);
 insert(&mut alloc,Address::from([0x66;20]),artifact("SystemBoundaryWitness.sol","SystemBoundaryWitness"),0);
 insert(&mut alloc,crate::pay::escrow_address(),Bytecode::default(),1000);
 let initial=identity(&spec);spec.alloc_root=gb::derive_fixture_root(&alloc,&initial,&programs).unwrap().0;let p=identity(&spec);let image=gb::build(&alloc,&p,&programs).unwrap();assert_eq!(image.root(),root(image.state()));(p,image,alloc,programs)
}
fn phase0(c:&mut Candidate){c.call(crate::pay::registry_address(),data("markClose()")).unwrap();c.call(crate::pay::registry_address(),data("beginBlock()")).unwrap();}
fn pending(h:u64)->U256{U256::from_be_bytes(keccak256([U256::from(h).to_be_bytes::<32>(),U256::from(3).to_be_bytes::<32>()].concat()).0)}
#[test]fn actual_builder_four_and_1024_records_and_corrupt_inputs(){
 for n in [4,1024]{let(p,image,alloc,programs)=fixture(n,80000);assert!(!p.bootable());let db=image.state();let bond=U256::from_be_bytes(p.spec().cp[16].to_be_bytes());assert_eq!(storage(db,crate::pay::registry_address(),0),U256::from(n));assert_eq!(storage(db,crate::pay::registry_address(),1),U256::from(n));assert_eq!(storage(db,crate::pay::registry_address(),4),bond*U256::from(n));assert_eq!(db.accounts[&crate::pay::escrow_address()].info.balance,U256::from(1000)+bond*U256::from(n));
  let list=U256::from_be_bytes(keccak256(U256::from(6).to_be_bytes::<32>()).0);for(i,m)in p.spec().m0_list.iter().enumerate(){assert_eq!(db.accounts[&crate::pay::registry_address()].storage[&(list+U256::from(i))],U256::from_be_slice(&m.id));}
  let mut wrong=p.spec().clone();wrong.alloc_root[0]^=1;assert!(matches!(gb::build(&alloc,&identity(&wrong),&programs),Err(gb::GenesisError::Root)));
  let mut wrong_codes=programs.clone();wrong_codes[2]=Bytes::from(vec![0]);assert!(matches!(gb::build(&alloc,&p,&wrong_codes),Err(gb::GenesisError::Code)));
  let mut invalid=alloc.clone();invalid.accounts.get_mut(&Address::from(p.spec().m0_list[0].reward_addr)).unwrap().info.nonce=0;assert!(matches!(gb::build(&invalid,&p,&programs),Err(gb::GenesisError::RewardNotDurable)));
 }
}
#[test]fn signed_user_cannot_see_current_issuance_exact_pending_and_replay_rollback(){
 let rows:serde_json::Value=serde_json::from_str(include_str!("../../tests/data/issuance-signed.json")).unwrap();
 for row in rows["rows"].as_array().unwrap(){for signed in [false,true]{let(p,image,_,_)=fixture(4,80000);let b=block(&p);let proposer=if signed{Proposer::Signed(Address::from(p.spec().m0_list[0].id))}else{Proposer::Fallback};let start=root(image.state());let balance=image.state().accounts[&crate::pay::escrow_address()].info.balance;
  let mut c=Candidate::from_genesis(image.state(),&image,&p,&b,proposer).unwrap();phase0(&mut c);assert_eq!(c.issued().unwrap(),U256::ZERO);
  c.execute_users(&[raw(row["raw"].as_str().unwrap())]).unwrap();assert_eq!(storage(c.snapshot().unwrap(),Address::from([0x66;20]),1),balance);let tips=c.tips;let user=c.user.as_ref().unwrap();assert_eq!(user.receipts().len(),1);let gas=user.commitments().gas_used;assert!(tips>U256::ZERO);assert_eq!(tips,U256::from(gas)*U256::from(3));
  let used=c.used();let r=c.phase2().unwrap();assert_eq!(c.issued().unwrap(),U256::from(80000));assert_eq!(c.used(),used+r.raw_gas);let db=c.snapshot().unwrap();assert_eq!(db.accounts[&crate::pay::escrow_address()].info.balance,balance+tips+U256::from(80000));let base=pending(1);assert_eq!(db.accounts[&crate::pay::escrow_address()].storage[&base],tips+U256::from(80000));assert_eq!(db.accounts[&crate::pay::escrow_address()].storage[&(base+U256::from(1))],proposer.pending_word().unwrap());assert_eq!(db.accounts[&crate::pay::escrow_address()].storage.get(&(base+U256::from(2))).copied().unwrap_or_default(),U256::ZERO);assert!(!db.accounts.contains_key(&system_address()));
  assert!(matches!(c.phase2(),Err(SystemError::Phase)));assert!(matches!(c.snapshot(),Err(SystemError::Poisoned)));assert!(c.user.is_none());assert_eq!(root(image.state()),start);
 }}
}
#[test]fn phase_order_manifest_and_zero_subsidy_fail_closed(){
 let(p,image,_,_)=fixture(4,0);let b=block(&p);let mut c=Candidate::from_genesis(image.state(),&image,&p,&b,Proposer::Fallback).unwrap();assert!(matches!(c.phase2(),Err(SystemError::Phase)));assert!(matches!(c.snapshot(),Err(SystemError::Poisoned)));
 let mut c=Candidate::from_genesis(image.state(),&image,&p,&b,Proposer::Fallback).unwrap();assert!(matches!(c.execute_users(&[]),Err(SystemError::Phase)));
 let mut c=Candidate::from_genesis(image.state(),&image,&p,&b,Proposer::Fallback).unwrap();phase0(&mut c);c.execute_users(&[]).unwrap();c.phase2().unwrap();assert_eq!(c.issued().unwrap(),U256::ZERO);assert_eq!(c.snapshot().unwrap().accounts[&crate::pay::escrow_address()].info.balance,image.state().accounts[&crate::pay::escrow_address()].info.balance);
 let mut corrupted=image.state().clone();corrupted.accounts.get_mut(&gb::system(2)).unwrap().info.code_hash=B256::ZERO;assert!(matches!(Candidate::from_genesis(&corrupted,&image,&p,&b,Proposer::Fallback),Err(SystemError::Genesis)));
 let mut corrupted=image.state().clone();corrupted.insert_account_storage(gb::system(3),U256::from(15),U256::from(1)).unwrap();assert!(matches!(Candidate::from_genesis(&corrupted,&image,&p,&b,Proposer::Fallback),Err(SystemError::Genesis)));
}
#[test]fn real_phase2_oog_and_late_commitment_error_erase_user_and_credit(){
 let(p,image,alloc,codes)=fixture(4,80000);let b=block(&p);let mut probe=Candidate::from_genesis(image.state(),&image,&p,&b,Proposer::Fallback).unwrap();phase0(&mut probe);let budget=probe.used()+20_000;let mut spec=p.spec().clone();spec.cp[44]=crate::fixed::U256::from_u64(budget);let provisional=identity(&spec);spec.alloc_root=gb::derive_fixture_root(&alloc,&provisional,&codes).unwrap().0;let p=identity(&spec);let image=gb::build(&alloc,&p,&codes).unwrap();let b=block(&p);
 let signed:serde_json::Value=serde_json::from_str(include_str!("../../tests/data/issuance-signed.json")).unwrap();let raw_user=raw(signed["rows"][0]["raw"].as_str().unwrap());let original=root(image.state());let mut c=Candidate::from_genesis(image.state(),&image,&p,&b,Proposer::Fallback).unwrap();phase0(&mut c);c.execute_users(&[raw_user]).unwrap();assert!(storage(c.snapshot().unwrap(),Address::from([0x66;20]),1)>U256::ZERO);assert!(c.phase2().is_err());assert_eq!(root(image.state()),original);assert!(matches!(c.snapshot(),Err(SystemError::Poisoned)));assert!(matches!(c.issued(),Err(SystemError::Poisoned)));
 let(p,image,_,_)=fixture(4,80000);let b=block(&p);let mut c=Candidate::from_genesis(image.state(),&image,&p,&b,Proposer::Fallback).unwrap();phase0(&mut c);c.execute_users(&[]).unwrap();c.phase2().unwrap();let wrong=B256::ZERO;assert!(c.check_state_root(wrong).is_err());assert!(matches!(c.snapshot(),Err(SystemError::Poisoned)));
 let mut c=Candidate::from_genesis(image.state(),&image,&p,&b,Proposer::Fallback).unwrap();phase0(&mut c);c.execute_users(&[]).unwrap();c.state.accounts.get_mut(&crate::pay::escrow_address()).unwrap().info.balance=U256::from(1)<<128;assert!(matches!(c.phase2(),Err(SystemError::Genesis)));assert!(matches!(c.snapshot(),Err(SystemError::Poisoned)));
}
#[test]fn issuance_reproducible_report(){
 let txs:serde_json::Value=serde_json::from_str(include_str!("../../tests/data/issuance-signed.json")).unwrap();let mut genesis=Vec::new();
 for n in [4,1024]{let(p,image,alloc,programs)=fixture(n,80000);genesis.push(serde_json::json!({"members":n,"spec":crate::hex::encode(&gs::encode(p.spec())),"genesisHash":format!("0x{}",crate::hex::encode(&p.genesis_hash())),"alloc":state(&alloc),"programs":programs.iter().map(|b|crate::hex::encode(b)).collect::<Vec<_>>(),"root":format!("{:#x}",image.root()),"post":state(image.state())}));}
 let mut report_txs=txs["rows"].as_array().unwrap().clone();report_txs.push(serde_json::json!({"kind":0,"raw":txs["chunkStore"]}));
 let mut cases=Vec::new();for tx in &report_txs{for signed in [false,true]{let(p,image,_,_)=fixture(4,80000);let b=block(&p);let proposer=if signed{Proposer::Signed(Address::from(p.spec().m0_list[0].id))}else{Proposer::Fallback};let mut c=Candidate::from_genesis(image.state(),&image,&p,&b,proposer).unwrap();let mut calls=Vec::new();
  for method in ["markClose()","beginBlock()"]{let r=c.call(crate::pay::registry_address(),data(method)).unwrap();calls.push(serde_json::json!({"method":method,"rawGas":r.raw_gas,"refundCounter":r.refund_counter,"root":format!("{:#x}",root(c.snapshot().unwrap()))}));}
  c.execute_users(&[raw(tx["raw"].as_str().unwrap())]).unwrap();let user=c.user.as_ref().unwrap();let user_view=serde_json::json!({"root":format!("{:#x}",root(user.state())),"gasUsed":user.commitments().gas_used,"tips":user.tips().to_string(),"receipts":user.receipts().iter().map(|b|crate::hex::encode(b)).collect::<Vec<_>>()});let r=c.phase2().unwrap();cases.push(serde_json::json!({"kind":tx["kind"],"raw":tx["raw"],"origin":format!("{:#x}",proposer.origin().unwrap()),"phase0":calls,"user":user_view,"phase2":{"rawGas":r.raw_gas,"refundCounter":r.refund_counter,"input":format!("0x{}",crate::hex::encode(&[revm::primitives::keccak256(b"closeBlock(uint256)")[..4].to_vec(),c.tips.to_be_bytes::<32>().to_vec()].concat())),"issued":c.issued().unwrap().to_string(),"root":format!("{:#x}",root(c.snapshot().unwrap())),"post":state(c.snapshot().unwrap()),"sysUsed":c.used()}}));
 }}
 if let Some(path)=std::env::var_os("LP4_ISSUANCE_REPORT_PATH"){std::fs::write(path,serde_json::to_string_pretty(&serde_json::json!({"scope":"Actual GenesisBuilder/USER/SYS issuance primitive; one of five code roles is a synthetic witness; actual pending/bond Escrow still lacks complete settlement/finalization, no validated UT/header or full protocol/finality/resource acceptance","semanticVersion":["LP4-SYS-01","LP4-ISSUANCE-01"],"bootable":false,"genesis":genesis,"cases":cases})).unwrap()+"\n").unwrap();}
 assert_eq!(cases.len(),8);
}
#[test]fn derived_final_host_credit_zero_observed_by_compiled_witness_not_real_finalization(){
 let(p,_,alloc,mut codes)=fixture(4,80000);codes[1]=artifact("FinalFundingWitness.sol","FinalFundingWitness").original_bytes();let mut spec=p.spec().clone();spec.sys_code_hash=gb::code_hash(&codes).0;spec.cp[11]=crate::fixed::U256::from_u64(2);spec.cp[45]=crate::fixed::U256::from_u64(8);spec.cp[47]=crate::fixed::U256::from_u64(1);let provisional=identity(&spec);spec.alloc_root=gb::derive_fixture_root(&alloc,&provisional,&codes).unwrap().0;let p=identity(&spec);let image=gb::build(&alloc,&p,&codes).unwrap();let mut parent=image.state().clone();
 for height in 1..=8u64{let mut b=block(&p);b.height=U256::from(height);let mut c=Candidate::from_genesis(&parent,&image,&p,&b,Proposer::Fallback).unwrap();assert_eq!(c.final_block,height==8);phase0(&mut c);c.execute_users(&[]).unwrap();let before=c.snapshot().unwrap().accounts[&crate::pay::escrow_address()].info.balance;c.phase2().unwrap();
  if height<8{assert_eq!(c.issued().unwrap(),U256::from(80000));}else{assert_eq!(c.issued().unwrap(),U256::ZERO);assert_eq!(c.snapshot().unwrap().accounts[&crate::pay::escrow_address()].info.balance,before);assert_eq!(storage(c.snapshot().unwrap(),crate::pay::escrow_address(),11),before);assert!(c.snapshot().unwrap().accounts[&crate::pay::escrow_address()].storage.get(&pending(8)).copied().unwrap_or_default()==U256::ZERO);}
  parent=c.snapshot().unwrap().clone();
 }
 // The witness deliberately leaves earlier pots; this is NOT actual finalization.
 assert!(parent.accounts[&crate::pay::escrow_address()].storage.get(&pending(1)).copied().unwrap_or_default()>U256::ZERO);
}
#[test]fn actual_root_matching_supply_overflow_and_r23_bound_reject_before_acceptance(){
 let(p,image,alloc,codes)=fixture(4,80000);let txs:serde_json::Value=serde_json::from_str(include_str!("../../tests/data/issuance-signed.json")).unwrap();let sender=Address::from_slice(&raw(txs["rows"][0]["sender"].as_str().unwrap()));
 for overflow in [false,true]{let mut bad_alloc=alloc.clone();let mut bad_world=image.state().clone();let value=U256::from(1)<<if overflow{255u32}else{127u32};bad_alloc.accounts.get_mut(&sender).unwrap().info.balance=value;bad_world.accounts.get_mut(&sender).unwrap().info.balance=value;
  if overflow{let reward=Address::from(p.spec().m0_list[0].reward_addr);bad_alloc.accounts.get_mut(&reward).unwrap().info.balance=value;bad_world.accounts.get_mut(&reward).unwrap().info.balance=value;}
  let mut spec=p.spec().clone();spec.alloc_root=root(&bad_world).0;let bad=identity(&spec);assert!(matches!(gb::build(&bad_alloc,&bad,&codes),Err(gb::GenesisError::Supply)));
 }
 // Corrupt actual code bytes with an intact claimed hash cannot pass the builder.
 let mut bad=alloc.clone();let address=Address::from([0x66;20]);bad.accounts.get_mut(&address).unwrap().info.code_hash=B256::ZERO;assert!(matches!(gb::build(&bad,&p,&codes),Err(gb::GenesisError::Code)));
}
#[test]fn actual_phase2_post_storage_cap_rejects_entire_candidate(){
 // D-A03-CAPS: system storage is capped by SYS_SLOT_MAX (cp[40]); SLOT_CAP (cp[38]) now
 // bounds the user state only. Phase 2's new pending slots exceed the SYS cap here.
 let(p,image,alloc,codes)=fixture(4,80000);let slots=crate::resources::sys_slots(image.state()) as usize;let mut spec=p.spec().clone();spec.cp[40]=crate::fixed::U256::from_u64((slots+1) as u64);let initial=identity(&spec);spec.alloc_root=gb::derive_fixture_root(&alloc,&initial,&codes).unwrap().0;let p=identity(&spec);let image=gb::build(&alloc,&p,&codes).unwrap();let b=block(&p);let mut c=Candidate::from_genesis(image.state(),&image,&p,&b,Proposer::Fallback).unwrap();phase0(&mut c);c.execute_users(&[]).unwrap();assert!(matches!(c.phase2(),Err(SystemError::Genesis)));assert!(matches!(c.snapshot(),Err(SystemError::Poisoned)));assert_eq!(c.issued().err(),Some(SystemError::Poisoned));
}
#[test]fn sealed_genesis_real_factory_signed_user_creation_issuance_and_late_rollback(){
 let(p,image,_,_)=fixture(4,80000);let b=block(&p);let v:serde_json::Value=serde_json::from_str(include_str!("../../tests/data/issuance-signed.json")).unwrap();let factory=gb::system(4);let before_nonce=image.state().accounts[&factory].info.nonce;assert_eq!(image.state().accounts[&factory].info.code_hash,artifact("ChunkFactory.sol","ChunkFactory").hash_slow());
 let init=raw("61000280600a5f395ff300aa");let hash=keccak256([vec![0xff],factory.as_slice().to_vec(),keccak256([0xaa]).to_vec(),keccak256(init).to_vec()].concat());let chunk=Address::from_slice(&hash[12..]);assert!(!image.state().accounts.contains_key(&chunk));let before_balance=image.state().accounts[&crate::pay::escrow_address()].info.balance;let original=root(image.state());
 let mut c=Candidate::from_genesis(image.state(),&image,&p,&b,Proposer::Fallback).unwrap();phase0(&mut c);c.execute_users(&[raw(v["chunkStore"].as_str().unwrap())]).unwrap();let db=c.snapshot().unwrap();assert_eq!(db.accounts[&factory].info.nonce,before_nonce+1);let a=&db.accounts[&chunk].info;assert_eq!(db.contracts[&a.code_hash].original_bytes().as_ref(),[0,0xaa]);assert_eq!(a.nonce,1);assert_eq!(db.accounts[&crate::pay::escrow_address()].info.balance,before_balance+c.tips);let receipt=crate::rlp::decode(&c.user.as_ref().unwrap().receipts()[0]).unwrap();assert_eq!(receipt.list().unwrap()[0].bytes().unwrap(),[1]);
 c.phase2().unwrap();assert_eq!(c.issued().unwrap(),U256::from(80000));assert_eq!(c.snapshot().unwrap().accounts[&crate::pay::escrow_address()].info.balance,before_balance+c.tips+U256::from(80000));assert!(c.check_state_root(B256::ZERO).is_err());assert!(matches!(c.snapshot(),Err(SystemError::Poisoned)));assert_eq!(root(image.state()),original);assert!(!image.state().accounts.contains_key(&chunk));
}
#[test]fn authenticated_user_origin_never_grants_system_caller_authority(){
 let(p,image,_,_)=fixture(4,80000);let b=block(&p);let v:serde_json::Value=serde_json::from_str(include_str!("../../tests/data/issuance-signed.json")).unwrap();let sender=Address::from_slice(&raw(v["rows"][0]["sender"].as_str().unwrap()));let mut c=Candidate::from_genesis(image.state(),&image,&p,&b,Proposer::Signed(sender)).unwrap();phase0(&mut c);c.execute_users(&[raw(v["unauthorizedClose"].as_str().unwrap())]).unwrap();
 let receipt=crate::rlp::decode(&c.user.as_ref().unwrap().receipts()[0]).unwrap();let fields=receipt.list().unwrap();assert!(fields[0].bytes().unwrap().is_empty());assert_eq!(c.snapshot().unwrap().accounts[&crate::pay::escrow_address()].storage.get(&pending(1)).copied().unwrap_or_default(),U256::ZERO);c.phase2().unwrap();assert_eq!(c.snapshot().unwrap().accounts[&crate::pay::escrow_address()].storage[&pending(1)],U256::from(80000)+c.tips);
}
