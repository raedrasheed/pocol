#![allow(dead_code)]
use pocol_lp4_execution::{profile::{ProfileIdentity,ProfileKind,NodeKind},genesis_spec as gs,native::NativeBlockContext};
use revm::{InMemoryDB,primitives::{U256,B256,AccountInfo,Address,Bytecode,Bytes}};
pub fn raw(s:&str)->Vec<u8>{s.as_bytes().chunks(2).map(|p|u8::from_str_radix(std::str::from_utf8(p).unwrap(),16).unwrap()).collect()}
pub fn profile(chain:u64,txmax:Option<u32>)->ProfileIdentity{
 let mut s=gs::decode(&raw(include_str!("../data/execution-profile.hex").trim())).unwrap();s.chain_id=chain;if let Some(n)=txmax{s.cp[gs::cp_index("TX_MAX").unwrap()]=pocol_lp4_execution::fixed::U256::from_u64(n as u64);}
 let bytes=gs::encode(&s);ProfileIdentity::validate(&bytes,&gs::genesis_hash(&bytes),chain,if chain==777001{ProfileKind::Network}else{ProfileKind::Synthetic},if chain==777001{NodeKind::Node}else{NodeKind::Simnet},&[(1,0)]).unwrap()
}
pub fn fixture()->(serde_json::Value,InMemoryDB,NativeBlockContext){
 let v:serde_json::Value=serde_json::from_str(include_str!("../data/wire-envelopes.json")).unwrap();let row=&v["cases"][0];let mut db=InMemoryDB::default();let code=Bytecode::new_raw(Bytes::from(raw("465f5500")));let empty=Bytecode::default();
 db.insert_account_info(Address::from_slice(&raw(&row["view"]["sender"].as_str().unwrap()[2..])),AccountInfo::new(U256::from(1_000_000_000u64),1,empty.hash_slow(),empty));db.insert_account_info(Address::from([0x22;20]),AccountInfo::new(U256::ZERO,1,code.hash_slow(),code));
 (v,db,NativeBlockContext{height:U256::from(300),timestamp:U256::from(12345),gas_limit:U256::from(8_000_000),base_fee:U256::from(100),prevrandao:B256::from([0x77;32])})
}
