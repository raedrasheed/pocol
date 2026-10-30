//! DIAGNOSTIC alternatives only. Neither configurable interpretation is approved.
#[path="../tests/support/admission.rs"] #[allow(dead_code,unused_imports)] mod f;
use revm::{Evm,primitives::{U256,TxEnv,TxKind,Bytes,SpecId}};
use pocol_lp4_execution::native;
fn summary(result:&Result<revm::primitives::ResultAndState,revm::primitives::EVMError<core::convert::Infallible>>)->String{
 match result{Err(e)=>format!("{e:?}"),Ok(r)=>match &r.result{revm::primitives::ExecutionResult::Success{gas_used,..}=>format!("Success gas={gas_used}"),revm::primitives::ExecutionResult::Revert{gas_used,..}=>format!("Revert gas={gas_used}"),revm::primitives::ExecutionResult::Halt{reason,gas_used}=>format!("Halt {reason:?} gas={gas_used}")}}
}
fn main(){let(_,db,block)=f::fixture();let profile=f::profile(777902,None);assert_eq!(profile.cp().B_code_max,32768);let sender=profile;
 let mut rows=Vec::new();for mode in ["current-native","diagnostic-revm-cp-override"]{
  for size in [24576usize,24577,32768,32769]{
   let init=vec![0x61,(size>>8) as u8,size as u8,0x5f,0xf3];let tx=TxEnv{caller:f::fixture().0["cases"][0]["view"]["sender"].as_str().unwrap().parse().unwrap(),nonce:Some(1),gas_limit:8_000_000,gas_price:U256::ZERO,transact_to:TxKind::Create,data:Bytes::from(init),chain_id:Some(777902),..Default::default()};
   let result=if mode=="current-native"{native::execute_for_profile(&db,tx,&native::NativeBlockContext{base_fee:U256::ZERO,..f::fixture().2},&sender)}else{Evm::builder().with_db(db.clone()).with_spec_id(SpecId::CANCUN).modify_cfg_env(|c|{c.chain_id=777902;c.limit_contract_code_size=Some(32768)}).modify_block_env(|b|{b.gas_limit=U256::from(8_000_000);b.basefee=U256::ZERO}).modify_tx_env(|t|*t=tx).build().transact()};
   rows.push(serde_json::json!({"mode":mode,"test":"runtime-code","size":size,"outcome":summary(&result),"success":result.as_ref().map(|r|r.result.is_success()).unwrap_or(false),"acceptedRule":false}));
  }
  for size in [49152usize,49153,65536,65537]{let mut init=vec![0x5f,0x5f,0xf3];init.resize(size,0);let tx=TxEnv{caller:f::fixture().0["cases"][0]["view"]["sender"].as_str().unwrap().parse().unwrap(),nonce:Some(1),gas_limit:8_000_000,gas_price:U256::ZERO,transact_to:TxKind::Create,data:Bytes::from(init),chain_id:Some(777902),..Default::default()};
   let result=if mode=="current-native"{native::execute_for_profile(&db,tx,&native::NativeBlockContext{base_fee:U256::ZERO,..f::fixture().2},&sender)}else{Evm::builder().with_db(db.clone()).with_spec_id(SpecId::CANCUN).modify_cfg_env(|c|{c.chain_id=777902;c.limit_contract_code_size=Some(32768)}).modify_block_env(|b|{b.gas_limit=U256::from(8_000_000);b.basefee=U256::ZERO}).modify_tx_env(|t|*t=tx).build().transact()};
   rows.push(serde_json::json!({"mode":mode,"test":"initcode","size":size,"outcome":summary(&result),"success":result.as_ref().map(|r|r.result.is_success()).unwrap_or(false),"acceptedRule":false}));
  }
 }let _=block;println!("{}",serde_json::to_string_pretty(&rows).unwrap());}
