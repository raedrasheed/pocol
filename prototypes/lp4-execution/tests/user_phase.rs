use pocol_lp4_execution::{native::NativeBlockContext,user_phase::{self,UserError,UserCommitments}};
use revm::{InMemoryDB,primitives::{U256,B256,Address,Bytecode,Bytes,AccountInfo}};
fn raw(s:&str)->Vec<u8>{s.as_bytes().chunks(2).map(|c|u8::from_str_radix(std::str::from_utf8(c).unwrap(),16).unwrap()).collect()}
fn fixture()->(serde_json::Value,InMemoryDB,Vec<Vec<u8>>,NativeBlockContext){
 let v:serde_json::Value=serde_json::from_str(include_str!("data/user-phase.json")).unwrap();let mut db=InMemoryDB::default();
 let sender=Address::from_slice(&raw(v["sender"].as_str().unwrap()));let code=Bytecode::new_raw(Bytes::from(raw(v["code"].as_str().unwrap())));let empty=Bytecode::default();
 db.insert_account_info(sender,AccountInfo::new(U256::from(1_000_000_000u64),1,empty.hash_slow(),empty));db.insert_account_info(Address::from([0x22;20]),AccountInfo::new(U256::ZERO,1,code.hash_slow(),code));
 let txs=v["raw"].as_array().unwrap().iter().map(|r|raw(r.as_str().unwrap())).collect();
 (v,db,txs,NativeBlockContext{height:U256::from(300),timestamp:U256::from(12345),gas_limit:U256::from(8_000_000),base_fee:U256::from(100),prevrandao:B256::from([0x77;32])})
}
fn commitments(v:&serde_json::Value)->UserCommitments{UserCommitments{transactions_root:B256::from_slice(&raw(v["transactionsRoot"].as_str().unwrap())),receipts_root:B256::from_slice(&raw(v["receiptsRoot"].as_str().unwrap())),gas_used:v["gasUsed"].as_u64().unwrap(),logs_bloom:[0;256]}}
#[test] fn sequential_signed_execution_refund_revert_and_commitments(){
 let(v,db,txs,block)=fixture();let before=format!("{db:?}");let bytes=txs.iter().map(Vec::len).sum();
 let calculated=user_phase::execute_user_phase(&db,&txs,&block,bytes,4);assert_eq!(calculated.is_ok(),true);let calculated=calculated.unwrap();assert_eq!(calculated.commitments(),&commitments(&v));let c=user_phase::validate_user_phase(&db,&txs,&block,bytes,4,&commitments(&v)).unwrap();assert_eq!(format!("{db:?}"),before);
 assert_eq!(c.state_root(3,1).is_ok(),true);assert_eq!(c.state_root(3,1).unwrap().as_slice(),raw(v["postStateRoot"].as_str().unwrap()));assert!(matches!(c.state_root(2,1),Err(UserError::Budget)));assert!(matches!(c.state_root(3,0),Err(UserError::Budget)));
 assert_eq!(c.tips().to_string(),v["tips"].as_str().unwrap());assert!(v["refund"][1].as_u64().unwrap()>0);
 let expected=v["receipts"].as_array().unwrap().iter().map(|r|raw(r.as_str().unwrap())).collect::<Vec<_>>();assert_eq!(c.receipts(),expected);
 let actual=c.state().accounts.iter().filter(|(_,a)|a.account_state!=revm::db::AccountState::NotExisting).collect::<Vec<_>>();assert_eq!(actual.len(),v["post"].as_object().unwrap().len());
 for (address,a) in actual{let key=format!("{address:#x}");let e=&v["post"][&key[2..]];assert_eq!(a.info.nonce,e["nonce"].as_u64().unwrap());assert_eq!(a.info.balance.to_string(),e["balance"].as_str().unwrap());assert_eq!(a.info.code_hash.as_slice(),raw(e["codeHash"].as_str().unwrap()));assert!(a.storage.iter().all(|(k,v)|*k==U256::ZERO||*v==U256::ZERO));assert_eq!(a.storage.get(&U256::ZERO).copied().unwrap_or_default().to_string(),e["slot0"].as_str().unwrap());}
}
#[test] fn wrong_commitments_and_late_invalid_transaction_leave_snapshot_unchanged(){
 let(v,db,txs,block)=fixture();let before=format!("{db:?}");let bytes=txs.iter().map(Vec::len).sum();let good=commitments(&v);
 for i in 0..4{let mut wrong=good.clone();match i{0=>wrong.transactions_root=B256::ZERO,1=>wrong.receipts_root=B256::ZERO,2=>wrong.gas_used+=1,_=>wrong.logs_bloom[0]=1};assert!(matches!(user_phase::validate_user_phase(&db,&txs,&block,bytes,4,&wrong),Err(UserError::Commitments)));assert_eq!(format!("{db:?}"),before);}
 let mut late=txs.clone();late.push(txs[0].clone());assert!(matches!(user_phase::execute_user_phase(&db,&late,&block,usize::MAX,5),Err(UserError::Execution(4,_))));assert_eq!(format!("{db:?}"),before);
 assert!(matches!(user_phase::execute_user_phase(&db,&txs,&block,bytes-1,4),Err(UserError::Budget)));assert!(matches!(user_phase::execute_user_phase(&db,&txs,&block,bytes,3),Err(UserError::Budget)));
 let mut tight=block;tight.gas_limit=U256::from(v["gasUsed"].as_u64().unwrap());assert!(matches!(user_phase::execute_user_phase(&db,&txs,&tight,bytes,4),Err(UserError::Gas)));assert_eq!(format!("{db:?}"),before);
}

#[test] fn late_wrong_chain_type_and_unprotected_are_not_included(){
 let(_,db,txs,block)=fixture();let before=format!("{db:?}");
 let all:serde_json::Value=serde_json::from_str(include_str!("data/wire-envelopes.json")).unwrap();let mut checked=0;
 for row in all["cases"].as_array().unwrap().iter().filter(|r|matches!(r["error"].as_str(),Some("ChainId"|"UnprotectedLegacy"|"UnsupportedType"))){
  let mut bad=txs.clone();bad.push(raw(row["raw"].as_str().unwrap()));let result=user_phase::execute_user_phase(&db,&bad,&block,usize::MAX,5);
  assert!(matches!(result,Err(UserError::Wire(4,_))));assert_eq!(format!("{db:?}"),before);checked+=1;
 }assert_eq!(checked,8);
 let empty=user_phase::execute_user_phase(&db,&[],&block,0,0).unwrap();assert_eq!(empty.commitments().gas_used,0);assert_eq!(empty.tips(),U256::ZERO);assert_eq!(empty.commitments().transactions_root,empty.commitments().receipts_root);assert_eq!(format!("{:?}",empty.state()),before);
}
