//! D27 compiled factory controls; not complete five-program/apply acceptance.
use super::*;
use super::tests::{insert,root,state,raw};
use revm::primitives::{Bytecode,keccak256};
fn hex(b:&[u8])->String{format!("0x{}",b.iter().map(|x|format!("{x:02x}")).collect::<String>())}
fn factory()->Address{crate::genesis_builder::system(4)}
fn code()->Bytecode{
 let j:serde_json::Value=serde_json::from_str(include_str!("../../tests/data/chunk-factory.json")).unwrap();
 Bytecode::new_raw(Bytes::from(raw(j["output"]["contracts"]["ChunkFactory.sol"]["ChunkFactory"]["evm"]["deployedBytecode"]["object"].as_str().unwrap())))
}
fn input(data:&[u8])->Bytes{
 let mut out=keccak256(b"store(bytes)")[..4].to_vec();out.extend(U256::from(32).to_be_bytes::<32>());out.extend(U256::from(data.len()).to_be_bytes::<32>());out.extend(data);out.resize(68+data.len().div_ceil(32)*32,0);Bytes::from(out)
}
fn target(data:&[u8])->(Address,Vec<u8>){
 let mut init=vec![0x61];init.extend(u16::try_from(data.len()+1).unwrap().to_be_bytes());init.extend([0x80,0x60,0x0a,0x5f,0x39,0x5f,0xf3,0]);init.extend(data);
 let hash=keccak256([vec![0xff],factory().as_slice().to_vec(),keccak256(data).to_vec(),keccak256(&init).to_vec()].concat());(Address::from_slice(&hash[12..]),init)
}
fn fixture()->InMemoryDB{let mut db=InMemoryDB::default();insert(&mut db,Address::from([0x11;20]),Bytecode::default(),1_000_000_000);insert(&mut db,factory(),code(),0);db}
fn run(db:&InMemoryDB,data:&[u8],value:u64)->revm::primitives::ResultAndState{
 let sender=Address::from([0x11;20]);crate::native::execute_at(db,TxEnv{caller:sender,transact_to:TxKind::Call(factory()),gas_limit:8_000_000,gas_price:U256::ZERO,nonce:Some(db.accounts[&sender].info.nonce),chain_id:Some(crate::CHAIN_ID),data:input(data),value:U256::from(value),..Default::default()},U256::from(1)).unwrap()
}
#[test]fn compiled_d27_boundaries_exact_initcode_idempotence_and_no_storage(){
 let mut reports=Vec::new();
 for len in [1,31,24575]{let data=vec![0xaa;len];let mut db=fixture();let before=root(&db);let(address,init)=target(&data);let first=run(&db,&data,0);assert!(first.result.is_success());assert!(first.result.logs().is_empty());assert_eq!(crate::output(&first.result),address.into_word().as_slice());assert!(first.state.values().all(|a|a.storage.values().all(|s|!s.is_changed())));let first_gas=first.result.gas_used();commit_epoch(&mut db,first.state);let account=&db.accounts[&address];assert_eq!(account.info.nonce,1);assert_eq!(db.contracts[&account.info.code_hash].original_bytes().as_ref(),[vec![0],data.clone()].concat());assert_eq!(db.accounts[&factory()].info.nonce,2);let after_first=root(&db);let second=run(&db,&data,0);assert!(second.result.is_success());assert_eq!(crate::output(&second.result),address.into_word().as_slice());let second_gas=second.result.gas_used();assert!(second_gas<first_gas);commit_epoch(&mut db,second.state);assert_eq!(db.accounts[&factory()].info.nonce,2);assert_eq!(db.accounts[&address].info.nonce,1);assert!(db.accounts[&factory()].storage.values().all(|v|*v==U256::ZERO));
 reports.push(serde_json::json!({"bytes":len,"initcode":hex(&init),"address":hex(address.as_slice()),"initialRoot":hex(before.as_slice()),"firstRoot":hex(after_first.as_slice()),"secondRoot":hex(root(&db).as_slice()),"firstGas":first_gas,"secondGas":second_gas,"post":state(&db)}));
 }
 if let Ok(path)=std::env::var("LP4_CHUNK_REPORT_PATH"){std::fs::write(path,serde_json::to_vec_pretty(&serde_json::json!({"scope":"Actual D27 factory, unsigned caller-controlled native execution only; not validated apply or full GenesisBuilder","artifact":serde_json::from_str::<serde_json::Value>(include_str!("../../tests/data/chunk-factory.json")).unwrap(),"cases":reports,"bootable":false})).unwrap()).unwrap();}
}
#[test]fn compiled_d27_invalid_lengths_value_and_storage_collision_reject(){
 for (len,value) in [(0,0),(24576,0),(1,1)]{let db=fixture();let r=run(&db,&vec![0xaa;len],value);assert!(!r.result.is_success());assert_eq!(r.state.get(&factory()).map(|a|a.info.nonce).unwrap_or(1),1);}
 let data=vec![0xaa];let(address,_)=target(&data);let mut db=fixture();insert(&mut db,address,Bytecode::default(),0);db.accounts.get_mut(&address).unwrap().info.nonce=0;db.insert_account_storage(address,U256::ZERO,U256::from(1)).unwrap();let r=run(&db,&data,0);assert!(!r.result.is_success());assert!(!r.state.get(&address).map(|a|a.is_created()).unwrap_or(false));
}
#[test]fn d27_length_limit_is_independent_of_approved_larger_runtime_cap(){
 let old=super::tests::profile(300_000_000);let mut spec=old.spec().clone();spec.cp[34]=crate::fixed::U256::from_u64(32768);
 let bytes=crate::genesis_spec::encode(&spec);let p=crate::profile::ProfileIdentity::validate(&bytes,&crate::genesis_spec::genesis_hash(&bytes),777902,crate::profile::ProfileKind::Synthetic,crate::profile::NodeKind::Simnet,&[(1,0)]).unwrap();
 assert_eq!(p.cp().B_code_max,32768);let db=fixture();let b=super::tests::block(&p);
 let r=crate::native::execute_for_profile(&db,TxEnv{caller:Address::from([0x11;20]),transact_to:TxKind::Call(factory()),gas_limit:8_000_000,gas_price:U256::from(100),nonce:Some(1),chain_id:Some(777902),data:input(&vec![0xaa;24576]),..Default::default()},&b,&p).unwrap();
 assert!(!r.result.is_success(),"assertion: D27 data cap remains24575 even when CP runtime cap is32768");
}
