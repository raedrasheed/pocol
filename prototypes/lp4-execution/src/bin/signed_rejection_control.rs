//! Signed rejection controls only, not PoCol full-block or boot admission.
use std::{io::{self,BufRead},str::FromStr};
use revm::{InMemoryDB,primitives::{U256,B256,Address,AccountInfo,Bytecode,Bytes}};
use pocol_lp4_execution::{wire,native::{self,NativeBlockContext}};
fn number(v:&serde_json::Value)->U256{U256::from_str(v.as_str().unwrap()).unwrap()}
fn bytes(s:&str)->Vec<u8>{let s=s.strip_prefix("0x").unwrap_or(s);s.as_bytes().chunks(2).map(|p|u8::from_str_radix(std::str::from_utf8(p).unwrap(),16).unwrap()).collect()}
fn run(v:serde_json::Value)->serde_json::Value{
 let raw=bytes(v["raw"].as_str().unwrap());let signed=match wire::decode_signed(&raw,131072){Ok(t)=>t,Err(e)=>return serde_json::json!({"stage":"wire","error":format!("{e:?}")})};let mut db=InMemoryDB::default();
 for (a,info) in v["pre"].as_object().unwrap(){let address=Address::from_str(a).unwrap();let code=Bytecode::new_raw(Bytes::from(bytes(info["code"].as_str().unwrap())));db.insert_account_info(address,AccountInfo::new(number(&info["balance"]),number(&info["nonce"]).try_into().unwrap(),code.hash_slow(),code));for(k,value)in info["storage"].as_object().unwrap(){db.insert_account_storage(address,U256::from_str(k).unwrap(),number(value)).unwrap();}}
 let env=&v["env"];let block=NativeBlockContext{height:number(&env["currentNumber"]),timestamp:number(&env["currentTimestamp"]),gas_limit:number(&env["currentGasLimit"]),base_fee:env.get("currentBaseFee").map(number).unwrap_or_default(),prevrandao:env.get("currentRandom").map(|r|B256::from_str(r.as_str().unwrap()).unwrap()).unwrap_or_default()};
 let before=format!("{db:?}");let outcome=native::execute_in_context(&db,signed.tx_env(),&block);assert_eq!(format!("{db:?}"),before);
 match outcome{Ok(r)=>serde_json::json!({"stage":"execution","accepted":true,"success":r.result.is_success(),"sender":format!("{:#x}",signed.sender())}),Err(e)=>serde_json::json!({"stage":"execution","accepted":false,"error":format!("{e:?}"),"sender":format!("{:#x}",signed.sender()),"hash":format!("{}",signed.hash()),"snapshotUnchanged":true})}
}
fn main(){for line in io::stdin().lock().lines(){let line=line.unwrap();let result=std::panic::catch_unwind(||run(serde_json::from_str(&line).unwrap())).unwrap_or_else(|_|serde_json::json!({"runnerFailure":true}));println!("{result}");}}
