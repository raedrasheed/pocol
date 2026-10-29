//! P2 native execution views, NOT Rust signed-wire authentication or genesis.
use crate::{native::{self,NativeBlockContext},code_caps::{self,CodeLimits},profile::{ProfileIdentity,ProfileKind,NodeKind},genesis_spec as gs};
use revm::{InMemoryDB,DatabaseCommit,db::AccountState,primitives::{Address,AccountInfo,Bytecode,Bytes,U256,B256,TxEnv,TxKind}};
fn raw(s:&str)->Vec<u8>{s.as_bytes().chunks(2).map(|p|u8::from_str_radix(std::str::from_utf8(p).unwrap(),16).unwrap()).collect()}
fn num(v:&str)->U256{U256::from_str_radix(v,10).unwrap()}
fn load()->serde_json::Value{
 fn expand(v:&mut serde_json::Value){match v{
  serde_json::Value::Array(a)=>for x in a{expand(x)},
  serde_json::Value::Object(m) if m.len()==1&&m.contains_key("hexRuns")=>{let s=m["hexRuns"].as_array().unwrap().iter().map(|x|if let Some(n)=x.as_u64(){"00".repeat(n as usize)}else{x.as_str().unwrap().to_string()}).collect::<String>();*v=serde_json::Value::String(s);},
  serde_json::Value::Object(m)=>for x in m.values_mut(){expand(x)},_=>{}
 }}let mut v=serde_json::from_str(include_str!("../tests/data/code-caps-native-views.json")).unwrap();expand(&mut v);v
}
fn info(code:&[u8])->AccountInfo{let b=Bytecode::new_raw(Bytes::copy_from_slice(code));AccountInfo::new(U256::ZERO,1,b.hash_slow(),b)}
fn identity(bytes:&[u8])->ProfileIdentity{ProfileIdentity::validate(bytes,&gs::genesis_hash(bytes),777902,ProfileKind::Synthetic,NodeKind::Simnet,&[(1,0)]).unwrap()}
fn root(db:&InMemoryDB)->B256{
 let number=|n:U256|crate::fixed::U256::from_be_slice(&n.to_be_bytes::<32>()).unwrap();let mut accounts=Vec::new();
 for (address,a) in &db.accounts{if a.account_state==AccountState::NotExisting{continue}assert!(a.storage.values().all(|v|*v==U256::ZERO));accounts.push(crate::trie::Account{address:address.0.0,nonce:a.info.nonce,balance:number(a.info.balance),storage_root:crate::trie::storage_root(&[],0).unwrap(),code_hash:a.info.code_hash.0});}
 B256::from(crate::trie::state_root(&accounts,32).unwrap())
}
fn tx(view:&serde_json::Value)->TxEnv{let to=raw(view["to"].as_str().unwrap());TxEnv{caller:Address::from_slice(&raw(view["caller"].as_str().unwrap())),transact_to:if to.is_empty(){TxKind::Create}else{TxKind::Call(Address::from_slice(&to))},nonce:Some(view["nonce"].as_u64().unwrap()),chain_id:Some(view["chain"].as_u64().unwrap()),gas_limit:view["gas"].as_u64().unwrap(),gas_price:num(view["gasPrice"].as_str().unwrap()),gas_priority_fee:view["priority"].as_str().map(num),data:Bytes::from(raw(view["input"].as_str().unwrap())),value:num(view["value"].as_str().unwrap()),..Default::default()}}
fn commit(db:&mut InMemoryDB,state:revm::primitives::EvmState){let removed:Vec<_>=state.iter().filter(|(_,a)|a.is_selfdestructed()||(a.is_touched()&&a.is_empty())).map(|(a,_)|*a).collect();db.commit(state);for address in removed{let a=db.accounts.entry(address).or_default();a.account_state=AccountState::NotExisting;a.storage.clear();a.info=Default::default();}}
#[test]fn independent_engine_limits_against_complete_state_roots(){
 let data=load();let rows=data["cases"].as_array().unwrap();assert_eq!(rows.len(),76);
 for row in rows{
  let name=row["name"].as_str().unwrap();let mut db=InMemoryDB::default();for (address,p) in row["pre"].as_object().unwrap(){let mut a=info(&raw(p["code"].as_str().unwrap()));a.balance=num(p["balance"].as_str().unwrap());a.nonce=p["nonce"].as_u64().unwrap();db.insert_account_info(Address::from_slice(&raw(address)),a);}
  assert_eq!(root(&db).as_slice(),raw(row["preRoot"].as_str().unwrap()));let before=format!("{db:?}");let spec=raw(row["genesisSpec"].as_str().unwrap());assert_eq!(gs::genesis_hash(&spec).as_slice(),raw(row["genesisHash"].as_str().unwrap()));let profile=identity(&spec);assert_eq!(profile.cp().B_code_max as u64,row["runtime"].as_u64().unwrap());assert_eq!(profile.cp().CODE_CAP,row["logical"].as_u64().unwrap());assert!(!profile.bootable());let block=NativeBlockContext{height:U256::from(300),timestamp:U256::from(12345),gas_limit:U256::from(profile.cp().GAS_LIMIT),base_fee:U256::from(100),prevrandao:B256::from([0x77;32])};
  let result=native::execute_for_profile(&db,tx(&row["nativeView"]),&block,&profile);
  if row.get("admissionError").is_some(){assert_eq!(result.is_err(),true,"{name} initcode limit");assert!(format!("{:?}",result.unwrap_err()).contains("CreateInitCodeSizeLimit"));}
  else{let r=result.unwrap();assert_eq!(r.result.is_success(),row["success"].as_bool().unwrap(),"{name} status");assert_eq!(r.result.gas_used(),row["gasUsed"].as_u64().unwrap(),"{name} gas");assert_eq!(crate::output(&r.result),raw(row["output"].as_str().unwrap()),"{name} output");let mut candidate=db.clone();commit(&mut candidate,r.state);assert_eq!(root(&candidate).as_slice(),raw(row["stateRoot"].as_str().unwrap()),"{name} complete root");assert_eq!(code_caps::inventory(&candidate).unwrap(),row["logicalAfter"].as_u64().unwrap());
   if let Some(next)=row.get("followUp"){let r=native::execute_for_profile(&candidate,tx(&next["nativeView"]),&block,&profile).unwrap();assert_eq!(r.result.gas_used()+row["gasUsed"].as_u64().unwrap(),next["gasUsed"].as_u64().unwrap());commit(&mut candidate,r.state);assert_eq!(root(&candidate).as_slice(),raw(next["stateRoot"].as_str().unwrap()));assert_eq!(code_caps::inventory(&candidate).unwrap(),next["logicalAfter"].as_u64().unwrap());}
  }assert_eq!(format!("{db:?}"),before);
 }
}
fn known_total(db:&InMemoryDB,tx:TxEnv,block:&NativeBlockContext,profile:&ProfileIdentity)->Result<revm::primitives::ResultAndState,revm::primitives::EVMError<core::convert::Infallible>>{
 native::execute_on_validated(db,code_caps::inventory(db).unwrap(),tx,block,profile)
}
/// Decision package C-2: the same 76 independent vectors through the path that takes the state's code
/// total instead of rescanning (USER phase and pool): identical status, gas, output and roots. Rows
/// whose pre-state fails the limits are skipped here (that path is only taken on a checked state; the
/// row above still covers them).
#[test]fn independent_engine_limits_with_a_known_code_total(){
 let data=load();let rows=data["cases"].as_array().unwrap();assert_eq!(rows.len(),76);let (mut checked,mut skipped)=(0,0);
 for row in rows{
  let name=row["name"].as_str().unwrap();let mut db=InMemoryDB::default();for (address,p) in row["pre"].as_object().unwrap(){let mut a=info(&raw(p["code"].as_str().unwrap()));a.balance=num(p["balance"].as_str().unwrap());a.nonce=p["nonce"].as_u64().unwrap();db.insert_account_info(Address::from_slice(&raw(address)),a);}
  assert_eq!(root(&db).as_slice(),raw(row["preRoot"].as_str().unwrap()));let before=format!("{db:?}");let spec=raw(row["genesisSpec"].as_str().unwrap());assert_eq!(gs::genesis_hash(&spec).as_slice(),raw(row["genesisHash"].as_str().unwrap()));let profile=identity(&spec);assert_eq!(profile.cp().B_code_max as u64,row["runtime"].as_u64().unwrap());assert_eq!(profile.cp().CODE_CAP,row["logical"].as_u64().unwrap());assert!(!profile.bootable());let block=NativeBlockContext{height:U256::from(300),timestamp:U256::from(12345),gas_limit:U256::from(profile.cp().GAS_LIMIT),base_fee:U256::from(100),prevrandao:B256::from([0x77;32])};
  if code_caps::validate_snapshot(&db,CodeLimits{runtime:profile.cp().B_code_max,logical:profile.cp().CODE_CAP}).is_err(){skipped+=1;continue}checked+=1;
  let result=known_total(&db,tx(&row["nativeView"]),&block,&profile);
  if row.get("admissionError").is_some(){assert_eq!(result.is_err(),true,"{name} initcode limit");assert!(format!("{:?}",result.unwrap_err()).contains("CreateInitCodeSizeLimit"));}
  else{let r=result.unwrap();assert_eq!(r.result.is_success(),row["success"].as_bool().unwrap(),"{name} status");assert_eq!(r.result.gas_used(),row["gasUsed"].as_u64().unwrap(),"{name} gas");assert_eq!(crate::output(&r.result),raw(row["output"].as_str().unwrap()),"{name} output");let mut candidate=db.clone();commit(&mut candidate,r.state);assert_eq!(root(&candidate).as_slice(),raw(row["stateRoot"].as_str().unwrap()),"{name} complete root");assert_eq!(code_caps::inventory(&candidate).unwrap(),row["logicalAfter"].as_u64().unwrap());
   if let Some(next)=row.get("followUp"){let r=known_total(&candidate,tx(&next["nativeView"]),&block,&profile).unwrap();assert_eq!(r.result.gas_used()+row["gasUsed"].as_u64().unwrap(),next["gasUsed"].as_u64().unwrap());commit(&mut candidate,r.state);assert_eq!(root(&candidate).as_slice(),raw(next["stateRoot"].as_str().unwrap()));assert_eq!(code_caps::inventory(&candidate).unwrap(),next["logicalAfter"].as_u64().unwrap());}
  }assert_eq!(format!("{db:?}"),before);
 }
 assert!(checked>=60,"checked {checked}, skipped {skipped}");
}

#[test]fn shared_code_charged_per_address(){let mut db=InMemoryDB::default();for n in 1..=5{db.insert_account_info(Address::from_word(U256::from(0xc0c000u64+n).into()),info(&[0;10]));}assert_eq!(code_caps::inventory(&db).unwrap(),50);assert_eq!(db.contracts.values().filter(|b|!b.is_empty()).count(),1);assert!(code_caps::validate_snapshot(&db,CodeLimits{runtime:24577,logical:50}).is_ok());assert!(code_caps::validate_snapshot(&db,CodeLimits{runtime:24577,logical:49}).is_err());}
#[test]fn missing_hash_or_r9_reject(){let mut db=InMemoryDB::default();let address=Address::from([0x22;20]);db.insert_account_info(address,info(&[0;10]));let hash=db.accounts[&address].info.code_hash;db.accounts.get_mut(&address).unwrap().info.code=None;db.contracts.remove(&hash);assert!(code_caps::inventory(&db).is_err());db.contracts.insert(hash,Bytecode::new_raw(Bytes::from(vec![0])));let r=code_caps::inventory(&db);assert_eq!(r.is_err(),true);let mut good=InMemoryDB::default();good.insert_account_info(address,info(&[]));assert!(code_caps::validate_snapshot(&good,CodeLimits{runtime:24576,logical:1<<30}).is_err());}
/// Decision package C-2, item 2: the USER phase keeps the code total across its transactions. Three
/// creates of 10 runtime bytes under a CODE_CAP of 29 (a vector profile): the first two fit, the third
/// does not (30 > 29). A total not updated after each commit would let the third through. The same
/// transactions run one by one through the full-scan path give the same outcome.
#[test]fn user_phase_keeps_the_code_total_across_transactions(){
 let data=load();let row=data["cases"].as_array().unwrap().iter().find(|r|r["logical"].as_u64()==Some(29)).expect("a CODE_CAP 29 vector").clone();
 let spec=raw(row["genesisSpec"].as_str().unwrap());let profile=identity(&spec);assert_eq!(profile.cp().CODE_CAP,29);
 let key=crate::txsign::Key::devnet("code-total");let caller=key.address();
 let mut db=InMemoryDB::default();db.insert_account_info(caller,AccountInfo{balance:U256::from(10u64).pow(U256::from(18u64)),..Default::default()});
 let init=Bytes::from(vec![0x60,10,0x60,0,0xf3]);// RETURN 10 zero bytes
 let txs:Vec<Vec<u8>>=(0..3u64).map(|n|crate::txsign::Tx{chain_id:profile.spec().chain_id,nonce:n,max_priority:U256::ZERO,max_fee:U256::from(1u64),gas:200_000,to:None,value:U256::ZERO,data:init.clone()}.sign_1559(&key)).collect();
 let block=NativeBlockContext{height:U256::from(1u64),timestamp:U256::from(1u64),gas_limit:U256::from(profile.cp().GAS_LIMIT),base_fee:U256::ZERO,prevrandao:B256::ZERO};
 let budget=profile.cp().BODY_MAX as usize;
 let c=crate::user_phase::execute_for_profile(&db,&txs,&block,budget,budget,&profile).unwrap();
 let status:Vec<u8>=c.receipts().iter().map(|r|{let it=crate::rlp::decode(&r[1..]).unwrap();it.list().unwrap()[0].bytes().unwrap().first().copied().unwrap_or(0)}).collect();
 assert_eq!(status,vec![1,1,0],"the third create passes CODE_CAP 29 and fails");
 assert_eq!(code_caps::inventory(c.state()).unwrap(),20);
 // Full-scan control, one transaction at a time.
 let mut s=db.clone();let mut full=vec![];
 for t in &txs{let env=crate::wire::decode_for_profile(t,&profile).unwrap().tx_env();let r=native::execute_for_profile(&s,env,&block,&profile).unwrap();full.push(u8::from(r.result.is_success()));commit(&mut s,r.state);}
 assert_eq!(full,status,"same outcome as the full-scan path");
}
/// Decision package C-2, item 1: the USER phase checks its snapshot once, at the first transaction, with
/// the error the per-transaction check gave (index 0); a phase without transactions checks nothing,
/// as before.
#[test]fn user_phase_checks_its_snapshot_once_at_the_first_transaction(){
 let data=load();let row=data["cases"].as_array().unwrap().iter().find(|r|r["logical"].as_u64()==Some(29)).unwrap().clone();
 let profile=identity(&raw(row["genesisSpec"].as_str().unwrap()));
 let key=crate::txsign::Key::devnet("snapshot-once");let caller=key.address();
 let mut db=InMemoryDB::default();db.insert_account_info(caller,AccountInfo{balance:U256::from(10u64).pow(U256::from(18u64)),..Default::default()});
 db.insert_account_info(Address::from([0x42;20]),info(&[0u8;30]));// 30 logical bytes > CODE_CAP 29
 let block=NativeBlockContext{height:U256::from(1u64),timestamp:U256::from(1u64),gas_limit:U256::from(profile.cp().GAS_LIMIT),base_fee:U256::ZERO,prevrandao:B256::ZERO};
 let budget=profile.cp().BODY_MAX as usize;
 let t=crate::txsign::Tx{chain_id:profile.spec().chain_id,nonce:0,max_priority:U256::ZERO,max_fee:U256::from(1u64),gas:21_000,to:Some(Address::from([0x43;20])),value:U256::ZERO,data:Bytes::new()}.sign_1559(&key);
 let e=crate::user_phase::execute_for_profile(&db,&[t],&block,budget,budget,&profile).err().expect("a snapshot over CODE_CAP is refused");
 assert_eq!(format!("{e:?}"),format!("{:?}",crate::user_phase::UserError::Execution(0,format!("{:?}",revm::primitives::EVMError::<core::convert::Infallible>::Custom("snapshot exceeds CODE_CAP".into())))));
 assert!(crate::user_phase::execute_for_profile(&db,&[],&block,budget,budget,&profile).is_ok(),"no transaction, no check (as before)");
}
/// Decision package C-2, item 2: `revalidate_touched` gives the Ok/Err (same texts) and the inventory
/// of a full `validate_snapshot` + `inventory` for a checked state changed only at the touched
/// accounts, across code growth, shrinkage, B_code_max, CODE_CAP and a missing runtime.
#[test]fn revalidate_touched_equals_the_full_check(){
 let limits=CodeLimits{runtime:24577,logical:100};
 let (a,b,c)=(Address::from([1;20]),Address::from([2;20]),Address::from([3;20]));
 let mut base=InMemoryDB::default();base.insert_account_info(a,info(&[0;30]));base.insert_account_info(b,info(&[0;20]));
 assert_eq!(code_caps::validate_snapshot(&base,limits),Ok(()));let total=code_caps::inventory(&base).unwrap();
 let cases:Vec<(&str,Box<dyn Fn(&mut InMemoryDB)>,Vec<Address>)>=vec![
  ("grow within CODE_CAP",Box::new(move|d|d.insert_account_info(c,info(&[0;40]))),vec![c]),
  ("grow past CODE_CAP",Box::new(move|d|d.insert_account_info(c,info(&[0;51]))),vec![c]),
  ("exactly CODE_CAP",Box::new(move|d|d.insert_account_info(c,info(&[0;50]))),vec![c]),
  ("replace with larger",Box::new(move|d|d.insert_account_info(b,info(&[0;71]))),vec![b]),
  ("remove one",Box::new(move|d|{let x=d.accounts.get_mut(&a).unwrap();x.account_state=AccountState::NotExisting;x.info=Default::default();}),vec![a]),
  ("past B_code_max",Box::new(move|d|d.insert_account_info(c,info(&vec![0;24578]))),vec![c]),
  ("missing runtime",Box::new(move|d|{d.insert_account_info(c,info(&[1,2,3]));let h=d.accounts[&c].info.code_hash;d.accounts.get_mut(&c).unwrap().info.code=None;d.contracts.remove(&h);}),vec![c]),
  ("untouched by a balance change",Box::new(move|d|{d.accounts.get_mut(&a).unwrap().info.balance=U256::from(5u64);}),vec![a]),
 ];
 for (name,change,touched) in cases{
  let before=code_caps::code_of(&base,&touched).unwrap();let mut d=base.clone();change(&mut d);
  let full=code_caps::validate_snapshot(&d,CodeLimits{logical:u64::MAX,..limits}).and_then(|_|code_caps::inventory(&d)).and_then(|t|if t>limits.logical{Err("snapshot exceeds CODE_CAP".to_string())}else{Ok(t)});
  let full=code_caps::validate_snapshot(&d,limits).and(full);
  assert_eq!(code_caps::revalidate_touched(&d,limits,&touched,total,before),full,"{name}");
 }
}
