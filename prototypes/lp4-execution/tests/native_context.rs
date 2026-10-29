//! Actual execution-field plumbing controls, not validated header/apply acceptance.
use pocol_lp4_execution::{native::{self,NativeBlockContext},output,pay::escrow_address,CHAIN_ID};
use revm::{InMemoryDB,primitives::{Address,AccountInfo,Bytecode,Bytes,TxEnv,TxKind,U256,B256}};
pub fn fixture(code:&[u8])->(InMemoryDB,TxEnv){
 let caller=Address::from([0x11;20]);let target=Address::from([0x22;20]);let mut db=InMemoryDB::default();
 let empty=Bytecode::default();db.insert_account_info(caller,AccountInfo::new(U256::from(1_000_000_000u64),1,empty.hash_slow(),empty));
 let bytecode=Bytecode::new_raw(Bytes::copy_from_slice(code));
 db.insert_account_info(target,AccountInfo::new(U256::ZERO,1,bytecode.hash_slow(),bytecode));
 let tx=TxEnv{caller,transact_to:TxKind::Call(target),gas_limit:100_000,gas_price:U256::from(200),nonce:Some(1),chain_id:Some(CHAIN_ID),..Default::default()};(db,tx)
}
pub fn context(height:u64)->NativeBlockContext{NativeBlockContext{height:U256::from(height),timestamp:U256::from(1_234_567),gas_limit:U256::from(8_000_000),base_fee:U256::from(123),prevrandao:B256::from([0x77;32])}}
pub fn fields_code()->Vec<u8>{
 let mut code=Vec::new();
 for (i,op) in [0x43,0x42,0x41,0x44,0x45,0x46,0x48,0x4a].iter().enumerate(){code.extend([*op,0x60,(i*32) as u8,0x52]);}
 // BLOBHASH(0), no blob transaction is supplied in this control.
 code.extend([0x5f,0x49,0x61,1,0,0x52,0x61,1,32,0x5f,0xf3]);code
}
#[test] fn explicit_fields_execute_and_snapshot_is_immutable(){
 let (db,tx)=fixture(&fields_code());let b=context(300);
 let r=native::execute_in_context(&db,tx.clone(),&b).unwrap();assert!(r.result.is_success());
 let words=[b.height,b.timestamp,U256::from_be_slice(escrow_address().as_slice()),U256::from_be_slice(b.prevrandao.as_slice()),b.gas_limit,U256::from(CHAIN_ID),b.base_fee,U256::from(1),U256::ZERO];
 let expected:Vec<u8>=words.iter().flat_map(|w|w.to_be_bytes::<32>()).collect();assert_eq!(output(&r.result),expected);
 assert_eq!(db.accounts[&tx.caller].info.nonce,1);assert!(db.block_hashes.is_empty());
}
pub fn blockhash_code(n:u64)->Vec<u8>{let mut code=vec![0x67];code.extend(n.to_be_bytes());code.extend([0x40,0x5f,0x52,0x60,32,0x5f,0xf3]);code}
#[test] fn blockhash_exact_256_window_genesis_and_current_boundaries(){
 for (h,n,visible) in [(10,0,true),(257,0,false),(257,1,true),(300,44,true),(300,43,false),(300,300,false),(300,301,false),(0,0,false)]{
  let (mut db,tx)=fixture(&blockhash_code(n));let value=B256::from([0x66;32]);db.block_hashes.insert(U256::from(n),value);
  let r=native::execute_in_context(&db,tx,&context(h)).unwrap();assert!(r.result.is_success());
  assert_eq!(output(&r.result),if visible{value.as_slice()}else{B256::ZERO.as_slice()},"h={h}, n={n}");
  assert_eq!(db.block_hashes[&U256::from(n)],value);
 }
}
#[test] fn explicit_base_fee_gas_limit_and_chain_rejections_do_not_mutate(){
 let (db,tx)=fixture(&[0]);let b=context(300);
 let mut wrong=tx.clone();wrong.gas_price=U256::from(122);assert!(native::execute_in_context(&db,wrong,&b).is_err());
 let mut wrong=tx.clone();wrong.gas_limit=8_000_001;assert!(native::execute_in_context(&db,wrong,&b).is_err());
 let mut wrong=tx.clone();wrong.chain_id=Some(777001);assert!(native::execute_in_context(&db,wrong,&b).is_err());
 let mut equal=tx.clone();equal.gas_price=b.base_fee;equal.gas_limit=8_000_000;assert!(native::execute_in_context(&db,equal,&b).unwrap().result.is_success());
 assert_eq!(db.accounts[&tx.caller].info.nonce,1);
}
