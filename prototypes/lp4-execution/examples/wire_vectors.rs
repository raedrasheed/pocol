//! Real signed synthetic transaction execution, not pool/whole-block admission.
#[path="../tests/wire.rs"] #[allow(dead_code,unused_imports)] mod f;
use revm::{DatabaseCommit,InMemoryDB,primitives::{AccountInfo,Address,Bytecode,Bytes,U256,B256}};
use pocol_lp4_execution::{native::{self,NativeBlockContext},output,wire::decode_signed};
fn dump(db:&InMemoryDB)->serde_json::Value{
 let mut state=serde_json::Map::new();for (address,a) in &db.accounts{
  if a.account_state==revm::db::AccountState::NotExisting{continue}
  let code=db.contracts.get(&a.info.code_hash).map(|b|b.original_bytes()).unwrap_or_default();
  let storage=a.storage.iter().filter(|(_,v)|**v!=U256::ZERO).map(|(k,v)|(format!("0x{k:064x}"),serde_json::json!(format!("0x{v:064x}")))).collect::<serde_json::Map<_,_>>();
  state.insert(format!("{address:#x}"),serde_json::json!({"nonce":a.info.nonce,"balance":a.info.balance.to_string(),"code":format!("0x{}",code.iter().map(|b|format!("{b:02x}")).collect::<String>()),"storage":storage}));
 }serde_json::Value::Object(state)
}
fn main(){let mut rows=Vec::new();let data=f::vectors();for row in data["cases"].as_array().unwrap().iter().filter(|r|r["error"].is_null()){
 let raw=f::unhex(row["raw"].as_str().unwrap());let signed=decode_signed(&raw,raw.len()).unwrap();let mut db=InMemoryDB::default();
 let empty=Bytecode::default();db.insert_account_info(signed.sender(),AccountInfo::new(U256::from(1_000_000_000u64),1,empty.hash_slow(),empty));
 // Stores CHAINID, BASEFEE, GASPRICE and emits a nonempty topic/data log.
 let mut ops=vec![0x46,0x5f,0x55,0x48,0x60,1,0x55,0x3a,0x60,2,0x55,0x60,0x7b,0x5f,0x52,0x60,0x11,0x60,32,0x5f,0xa1];
 let jump=ops.len()+3;ops.extend([0x36,0x15,0x60,0,0x57,0x5f,0x5f,0xfd]);ops[jump]=ops.len() as u8;ops.extend([0x5b,0]);let code=Bytecode::new_raw(Bytes::from(ops));
 db.insert_account_info(Address::from([0x22;20]),AccountInfo::new(U256::ZERO,1,code.hash_slow(),code));
 let pre=dump(&db);let block=NativeBlockContext{height:U256::from(300),timestamp:U256::from(12345),gas_limit:U256::from(8_000_000),base_fee:U256::from(100),prevrandao:B256::from([0x77;32])};
 let r=native::execute_in_context(&db,signed.tx_env(),&block).unwrap();let success=r.result.is_success();let gas=r.result.gas_used();let bytes=output(&r.result).to_vec();
 let logs=r.result.logs().iter().map(|l|serde_json::json!({"address":format!("{:#x}",l.address),"topics":l.data.topics().iter().map(|t|format!("{t}")).collect::<Vec<_>>(),"data":format!("0x{}",l.data.data.iter().map(|b|format!("{b:02x}")).collect::<String>())})).collect::<Vec<_>>();
 let receipt=pocol_lp4_execution::receipts::encode(signed.kind(),success,gas,r.result.logs()).unwrap();
 let bloom=pocol_lp4_execution::receipts::bloom(r.result.logs());let (price,tip)=pocol_lp4_execution::fees::effective_fees(block.base_fee,signed.tx_env().gas_price,signed.tx_env().gas_priority_fee).unwrap();
 db.commit(r.state);rows.push(serde_json::json!({"name":row["name"],"raw":row["raw"],"decoded":signed.view(),"pre":pre,"post":dump(&db),"success":success,"gasUsed":gas,"output":bytes,"logs":logs,"receipt":receipt,"bloom":bloom.to_vec(),"effectivePrice":price.to_string(),"effectiveTip":tip.to_string()}));
 }println!("{}",serde_json::to_string_pretty(&rows).unwrap());}
