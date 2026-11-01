#[path="support/admission.rs"] mod f;
use pocol_lp4_execution::{code_caps::{self,CodeLimits},profile::{ProfileIdentity,ProfileKind,NodeKind},user_phase,native,genesis_spec as gs};
use revm::{InMemoryDB,primitives::{Address,AccountInfo,Bytecode,Bytes,U256,TxEnv,TxKind}};

fn load()->serde_json::Value{
 fn expand(v:&mut serde_json::Value){match v{
  serde_json::Value::Array(a)=>for x in a{expand(x)},
  serde_json::Value::Object(m) if m.len()==1 && m.contains_key("hexRuns")=>{
   let text=m["hexRuns"].as_array().unwrap().iter().map(|x|if let Some(n)=x.as_u64(){"00".repeat(n as usize)}else{x.as_str().unwrap().to_string()}).collect::<String>();*v=serde_json::Value::String(text);
  },
  serde_json::Value::Object(m)=>for x in m.values_mut(){expand(x)},_=>{}
 }}
 let mut v=serde_json::from_str(include_str!("data/code-caps-execution.json")).unwrap();expand(&mut v);v
}
fn info(code:&[u8])->AccountInfo{let b=Bytecode::new_raw(Bytes::copy_from_slice(code));AccountInfo::new(U256::ZERO,1,b.hash_slow(),b)}
fn identity(bytes:&[u8])->ProfileIdentity{ProfileIdentity::validate(bytes,&gs::genesis_hash(bytes),777902,ProfileKind::Synthetic,NodeKind::Simnet,&[(1,0)]).unwrap()}

#[test]
fn independent_signed_top_and_compiled_nested_code_limits_roots_gas_receipts_and_rollback(){
 let fixture=load();let rows=fixture["cases"].as_array().unwrap();assert!(rows.len()>=70);
 for row in rows {
  let label=row["name"].as_str().unwrap();let mut db=InMemoryDB::default();
  for (address,p) in row["pre"].as_object().unwrap(){let mut a=info(&f::raw(p["code"].as_str().unwrap()));a.balance=U256::from_str_radix(p["balance"].as_str().unwrap(),10).unwrap();a.nonce=p["nonce"].as_u64().unwrap();db.insert_account_info(Address::from_slice(&f::raw(address)),a);}
  let before=format!("{db:?}");let spec=f::raw(row["genesisSpec"].as_str().unwrap());let p=identity(&spec);assert!(!p.bootable());assert_eq!(gs::genesis_hash(&spec).as_slice(),f::raw(row["genesisHash"].as_str().unwrap()));assert_eq!(p.cp().B_code_max as u64,row["runtime"].as_u64().unwrap());assert_eq!(p.cp().CODE_CAP,row["logical"].as_u64().unwrap());
  let (_,_,block)=f::fixture();let raw=f::raw(row["raw"].as_str().unwrap());let result=user_phase::execute_for_profile(&db,&[raw.clone()],&block,raw.len(),1,&p);
  if row.get("admissionError").is_some(){assert_eq!(result.is_err(),true,"{label}: oversized initcode must reject");assert!(format!("{:?}",result.err().unwrap()).contains("CreateInitCodeSizeLimit"),"{label}");}
  else{
   let candidate=result.unwrap_or_else(|e|panic!("{label}: {e:?}"));assert_eq!(candidate.commitments().gas_used,row["gasUsed"].as_u64().unwrap(),"{label} gas");assert_eq!(candidate.receipts()[0],f::raw(row["receipt"].as_str().unwrap()),"{label} receipt");
   assert_eq!(candidate.state_root(32,0).unwrap().as_slice(),f::raw(row["stateRoot"].as_str().unwrap()),"{label} complete state root");
   assert_eq!(code_caps::inventory(candidate.state()).unwrap(),row["logicalAfter"].as_u64().unwrap(),"{label} actual logical inventory");
   for (address,expected) in row["post"].as_object().unwrap(){let a=Address::from_slice(&f::raw(address));let actual=candidate.state().accounts.get(&a).and_then(|v|v.info());assert_eq!(actual.is_some(),expected["exists"].as_bool().unwrap(),"{label}: {a:?} existence");if let Some(actual)=actual{assert_eq!(actual.balance,U256::from_str_radix(expected["balance"].as_str().unwrap(),10).unwrap(),"{label} balance");assert_eq!(actual.nonce,expected["nonce"].as_u64().unwrap(),"{label} nonce");let expected_code=Bytecode::new_raw(Bytes::from(f::raw(expected["code"].as_str().unwrap())));assert_eq!(actual.code_hash,expected_code.hash_slow(),"{label} runtime hash");}}
   if let Some(next)=row.get("followUp"){
    let second=f::raw(next["raw"].as_str().unwrap());let size=raw.len()+second.len();let sequence=user_phase::execute_for_profile(&db,&[raw,second],&block,size,2,&p).unwrap();
    assert_eq!(sequence.commitments().gas_used,next["gasUsed"].as_u64().unwrap(),"{label} subsequent gas");assert_eq!(sequence.receipts()[1],f::raw(next["receipt"].as_str().unwrap()),"{label} subsequent receipt");assert_eq!(sequence.state_root(32,0).unwrap().as_slice(),f::raw(next["stateRoot"].as_str().unwrap()),"{label} after actual commit/deletion");assert_eq!(code_caps::inventory(sequence.state()).unwrap(),next["logicalAfter"].as_u64().unwrap());
   }
  }
  assert_eq!(format!("{db:?}"),before,"{label} immutable base snapshot");
 }
}

#[test]
fn inventory_charges_shared_code_per_address_and_system_accounts_without_physical_dedup(){
 let mut db=InMemoryDB::default();let a=info(&[0;10]);for n in 1..=5{db.insert_account_info(Address::from_word(U256::from(0xc0c000u64+n).into()),a.clone());}
 assert_eq!(code_caps::inventory(&db).unwrap(),50);assert_eq!(db.contracts.values().filter(|b|!b.is_empty()).count(),1);
 assert!(code_caps::validate_snapshot(&db,CodeLimits{runtime:24577,logical:50}).is_ok());assert!(code_caps::validate_snapshot(&db,CodeLimits{runtime:24577,logical:49}).is_err());
 db.insert_account_info(pocol_lp4_execution::pay_address(),AccountInfo::default());assert_eq!(code_caps::inventory(&db).unwrap(),50);
}

#[test]
fn missing_or_mismatched_code_and_below_r9_profile_never_execute(){
 let (_,mut db,block)=f::fixture();let address=Address::from([0x22;20]);let hash=db.accounts[&address].info.code_hash;db.accounts.get_mut(&address).unwrap().info.code=None;
 assert_eq!(code_caps::inventory(&db).unwrap(),4);db.contracts.remove(&hash);assert_eq!(code_caps::inventory(&db).unwrap_err(),"missing runtime bytes");
 db.contracts.insert(hash,Bytecode::new_raw(Bytes::from(vec![0])));let bad=code_caps::inventory(&db);assert_eq!(bad.is_err(),true,"mismatched runtime must be rejected");assert_eq!(bad.unwrap_err(),"runtime code hash mismatch");
 let (_,good,_)=f::fixture();let mut s=gs::decode(&f::raw(include_str!("data/execution-profile.hex").trim())).unwrap();s.chain_id=777902;s.cp[34]=pocol_lp4_execution::fixed::U256::from_u64(24576);let p=identity(&gs::encode(&s));
 let caller=*good.accounts.keys().find(|a|**a!=address).unwrap();let tx=TxEnv{caller,nonce:Some(1),chain_id:Some(777902),transact_to:TxKind::Call(address),gas_limit:100000,gas_price:U256::from(200),..Default::default()};
 let r=native::execute_for_profile(&good,tx,&block,&p);assert!(r.is_err());let error=r.unwrap_err();assert!(format!("{error:?}").contains("R9"));assert!(!p.bootable());
}

#[test]
fn profile_code_rule_keeps_compiled_registry_escrow_pay_results_and_rollback_identical(){
 use pocol_lp4_execution::{pay::{registry_address,escrow_address},output};
 let artifacts:serde_json::Value=serde_json::from_str(include_str!("data/pay-harness.json")).unwrap();
 let code=|name:&str|f::raw(artifacts["output"]["contracts"]["PayBoundaryHarness.sol"][name]["evm"]["deployedBytecode"]["object"].as_str().unwrap());
 let caller=Address::from([0x11;20]);let dest=Address::from([0x33;20]);let profile=f::profile(777902,None);let (_,_,mut block)=f::fixture();block.base_fee=U256::ZERO;
 for source in [registry_address(),escrow_address()] {
  let mut db=InMemoryDB::default();let mut user=info(&[]);user.balance=U256::from(1000000);db.insert_account_info(caller,user);let mut system=info(&code("PayBoundaryHarness"));system.balance=U256::from(100);db.insert_account_info(source,system);let mut recipient=info(&code("RecipientTrap"));recipient.balance=U256::from(7);db.insert_account_info(dest,recipient);
  for (gas,undo) in [(4999,false),(5000,false),(5000,true)] {
   let selector=|name:&str|revm::primitives::keccak256(name.as_bytes())[..4].to_vec();
   let mut data=if undo{selector("sendThenRevert(address,uint256)")}else{selector("sendRaw(bytes,uint256,uint256,uint8)")};
   if !undo{for n in [128,gas,0,0,64]{data.extend(U256::from(n).to_be_bytes::<32>());}}
   data.extend(dest.into_word());data.extend(U256::from(40).to_be_bytes::<32>());
   let tx=TxEnv{caller,nonce:Some(1),chain_id:Some(777902),transact_to:TxKind::Call(source),data:Bytes::from(data),gas_limit:1000000,gas_price:U256::ZERO,..Default::default()};
   let baseline=native::execute_in_context(&db,tx.clone(),&block).unwrap();let amended=native::execute_for_profile(&db,tx,&block,&profile).unwrap();
   assert_eq!(amended.result,baseline.result);assert_eq!(amended.state,baseline.state);if undo{assert!(!amended.result.is_success());}else{assert_eq!(U256::from_be_slice(&output(&amended.result)[..32]),U256::from((gas>=5000) as u8));}
  }
 }
}
