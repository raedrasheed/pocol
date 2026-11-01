//! Synthetic field plumbing with independent state-root comparisons, not apply.
#[path="../tests/native_context.rs"] #[allow(dead_code,unused_imports)] mod f;
use revm::{DatabaseCommit,InMemoryDB,primitives::{B256,U256}};
use pocol_lp4_execution::{native,output};
fn dump(db:&InMemoryDB)->serde_json::Value{
 let mut state=serde_json::Map::new();
 for (address,a) in &db.accounts{
  if a.account_state==revm::db::AccountState::NotExisting{continue;}
  let code=db.contracts.get(&a.info.code_hash).map(|b|b.original_bytes()).unwrap_or_default();
  state.insert(format!("{address}"),serde_json::json!({"nonce":a.info.nonce,"balance":a.info.balance.to_string(),"code":format!("0x{}",code.iter().map(|b|format!("{b:02x}")).collect::<String>()),"storage":{}}));
 }serde_json::Value::Object(state)
}
fn main(){
 let mut rows=Vec::new();
 let cases=[("fields",300,0,true),("genesis",10,0,false),("expired-genesis",257,0,false),("oldest",257,1,false),("inclusive256",300,44,false),("expired257",300,43,false),("current",300,300,false),("future",300,301,false),("height0",0,0,false)];
 for (name,h,n,fields) in cases{
  let code=if fields{f::fields_code()}else{f::blockhash_code(n)};
  let (mut db,tx)=f::fixture(&code);db.block_hashes.insert(U256::from(n),B256::from([0x66;32]));let block=f::context(h);let pre=dump(&db);
  let r=native::execute_in_context(&db,tx.clone(),&block).unwrap();let success=r.result.is_success();let gas=r.result.gas_used();let bytes=output(&r.result).to_vec();db.commit(r.state);
  rows.push(serde_json::json!({"name":name,"height":h,"timestamp":1234567,"gasLimit":8000000,"baseFee":123,"prevrandao":format!("{}",block.prevrandao),"cachedNumber":n,"cachedHash":format!("{}",B256::from([0x66;32])),"caller":format!("{}",tx.caller),"to":format!("{}",tx.transact_to.to().unwrap()),"pre":pre,"post":dump(&db),"success":success,"output":bytes,"gasUsed":gas}));
 }
 println!("{}",serde_json::to_string_pretty(&rows).unwrap());
}
