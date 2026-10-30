//! Lower-bound witnesses only, NOT full Registry execution/acceptance.
use pocol_lp4_execution::{native,pay::registry_address,output,CHAIN_ID};
use revm::{InMemoryDB,primitives::{Address,AccountInfo,Bytecode,Bytes,TxEnv,TxKind,U256,keccak256}};
fn slot(id:Address)->U256 {U256::from_be_bytes(keccak256([id.into_word().as_slice(),U256::from(7).to_be_bytes::<32>().as_slice()].concat()).0)}
fn base(n:u64)->U256 {U256::from_be_bytes(keccak256(U256::from(n).to_be_bytes::<32>()).0)}
fn hex(v:U256)->String {format!("0x{v:064x}")}
fn main(){
 let j:serde_json::Value=serde_json::from_str(include_str!("../tests/data/registry-write-budget.json")).unwrap();
 let text=j["output"]["contracts"]["RegistryWriteBudgetWitness.sol"]["RegistryWriteBudgetWitness"]["evm"]["deployedBytecode"]["object"].as_str().unwrap();
 let code=Bytecode::new_raw(Bytes::from((0..text.len()).step_by(2).map(|i|u8::from_str_radix(&text[i..i+2],16).unwrap()).collect::<Vec<_>>()));
 let user=Address::from([0x11;20]);let old=Address::from([0x22;20]);let reward=Address::from([0x33;20]);let moved=Address::from([0x44;20]);let source=registry_address();
 let bond=100_000_000_000_000_000_000u128; // M1 governance.md network B_reg=10^20
 let funding=1_000_000_000_000_000_000_000_000u128;
 let active=Address::from([0x55;20]);
 let mut rows=vec![];
 for (name,sig,original_limit,observed,value) in [("register","registerWrites(address)",9,10,bond),("setRewardAddress","rewardWrites(address)",1,2,0),("claimBond-middle","middleClaimWrites()",9,10,0)] {
  let mut db=InMemoryDB::default();
  db.insert_account_info(user,AccountInfo::new(U256::from(funding),1,revm::primitives::KECCAK_EMPTY,Bytecode::default()));
  db.insert_account_info(source,AccountInfo::new(U256::ZERO,1,code.hash_slow(),code.clone()));
  db.insert_account_info(reward,AccountInfo::new(U256::ZERO,1,revm::primitives::KECCAK_EMPTY,Bytecode::default()));
  db.insert_account_info(old,AccountInfo::new(U256::ZERO,1,revm::primitives::KECCAK_EMPTY,Bytecode::default()));
  let mut initial=vec![];
  let r=slot(user);
  assert_ne!(r+U256::from(1),U256::from(3));
  if name=="register" {
   for (k,v) in [(U256::ZERO,U256::from(1)),(U256::from(1),U256::from(1)),(U256::from(4),U256::from(bond)),(base(5),U256::from_be_slice(active.as_slice())),(base(6),U256::from_be_slice(active.as_slice())),(slot(active),U256::from(0x101)),(slot(active)+U256::from(1),U256::from_be_slice(reward.as_slice())),(slot(active)+U256::from(2),U256::from(bond))] {initial.push((k,v));}
  }
  if name=="setRewardAddress" {
   for (k,v) in [(U256::ZERO,U256::from(1)),(U256::from(1),U256::from(1)),(U256::from(4),U256::from(bond)),(base(5),U256::from_be_slice(user.as_slice())),(base(6),U256::from_be_slice(user.as_slice())),(r,U256::from(0x101)),(r+U256::from(1),U256::from_be_slice(old.as_slice())),(r+U256::from(2),U256::from(bond))] {initial.push((k,v));}
  }
  if name=="claimBond-middle" {
   for (k,v) in [(U256::ZERO,U256::from(3)),(U256::from(1),U256::from(1)),(base(6),U256::from_be_slice(active.as_slice())),(U256::from(4),U256::from(bond*3)),(base(5),U256::from_be_slice(user.as_slice())),(base(5)+U256::from(1),U256::from_be_slice(active.as_slice())),(base(5)+U256::from(2),U256::from_be_slice(moved.as_slice())),(r,U256::from(0x201)),(r+U256::from(1),U256::from_be_slice(reward.as_slice())),(r+U256::from(2),U256::from(bond)),(r+U256::from(3),U256::from(0x10001)),(slot(moved),U256::from(0x20201)),(slot(moved)+U256::from(1),U256::from_be_slice(reward.as_slice())),(slot(moved)+U256::from(2),U256::from(bond)),(slot(moved)+U256::from(3),U256::from(0x10001)),(slot(active),U256::from(0x10101)),(slot(active)+U256::from(1),U256::from_be_slice(reward.as_slice())),(slot(active)+U256::from(2),U256::from(bond))] {initial.push((k,v));}
  }
  for (k,v) in &initial {db.insert_account_storage(source,*k,*v).unwrap();}
  let mut input=keccak256(sig.as_bytes())[..4].to_vec();if name!="claimBond-middle" {input.extend(reward.into_word());}
  let tx=TxEnv{caller:user,transact_to:TxKind::Call(source),data:Bytes::from(input.clone()),value:U256::from(value),gas_limit:1_000_000,gas_price:U256::ZERO,nonce:Some(1),chain_id:Some(CHAIN_ID),..Default::default()};
  let result=native::execute_at(&db,tx,U256::from(1)).unwrap();assert!(result.result.is_success());
  let mut changes=result.state[&source].storage.iter().filter(|(_,s)|s.is_changed()).map(|(k,s)|(hex(*k),hex(s.present_value()))).collect::<Vec<_>>();changes.sort();
  assert_eq!(changes.len(),observed);assert!(changes.len()>original_limit);
  rows.push(serde_json::json!({"case":name,"scope":"mandatory-write-set witness only; original gate NOT PASSED","input":input,"value":value.to_string(),"preStorage":initial.into_iter().map(|(k,v)|(hex(k),hex(v))).collect::<Vec<_>>(),"changedStorage":changes,"gasUsed":result.result.gas_used(),"output":output(&result.result),"originalCeiling":original_limit,"actualChangedSlots":observed,"registryBalance":result.state[&source].info.balance.to_string(),"userBalance":result.state[&user].info.balance.to_string()}));
 }
 println!("{}",serde_json::to_string_pretty(&rows).unwrap());
}
