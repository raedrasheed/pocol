//! Full bytecode sequence vectors. Synthetic state, not admitted CP/profile/boot.
#[path="../tests/registry_payment.rs"] #[allow(dead_code,unused_imports)] mod f;
use revm::{DatabaseCommit,InMemoryDB,primitives::{Address,U256}};
use pocol_lp4_execution::{output,pay::registry_address};
fn dump(db:&InMemoryDB)->serde_json::Value{
 let mut state=serde_json::Map::new();
 for (address,a) in &db.accounts{
  if a.account_state==revm::db::AccountState::NotExisting{continue;}
  let code=db.contracts.get(&a.info.code_hash).map(|b|b.original_bytes()).unwrap_or_default();
  let storage=a.storage.iter().filter(|(_,v)|**v!=U256::ZERO).map(|(k,v)|(format!("0x{k:064x}"),serde_json::json!(format!("0x{v:064x}")))).collect::<serde_json::Map<_,_>>();
  state.insert(format!("{address}"),serde_json::json!({"nonce":a.info.nonce,"balance":a.info.balance.to_string(),"code":format!("0x{}",code.iter().map(|b|format!("{b:02x}")).collect::<String>()),"storage":storage}));
 }serde_json::Value::Object(state)
}
fn step(rows:&mut Vec<serde_json::Value>,db:&mut InMemoryDB,name:&str,caller:Address,sig:&str,arg:Option<Address>,value:u64,h:u64){
 let pre=dump(db);let mut input=f::selector(sig);if let Some(a)=arg{input.extend(a.into_word());}
 let r=f::call(db,caller,registry_address(),input.clone(),U256::from(value),h);let changed=f::writes(&r);db.commit(r.state.clone());
 rows.push(serde_json::json!({"caseId":rows.len(),"name":name,"fixtureClass":"synthetic complete execution snapshot; NOT ParamGate/profile acceptance","caller":format!("{caller}"),"to":format!("{}",registry_address()),"height":h,"input":input,"value":value,"pre":pre,"post":dump(db),"success":r.result.is_success(),"output":output(&r.result),"gasUsed":r.result.gas_used(),"changedRegistryKeys":changed,"logs":r.result.logs().len()}));
}
fn phases(rows:&mut Vec<serde_json::Value>,db:&mut InMemoryDB,h:u64){for sig in ["markClose()","beginBlock()"]{step(rows,db,&format!("phase-{h}-{sig}"),f::system(),sig,None,0,h);}}
fn main(){
 let mut rows=vec![];let mut db=f::fixture();phases(&mut rows,&mut db,1);
 step(&mut rows,&mut db,"register-positive",f::addr(0x12),"register(address)",Some(f::addr(0x32)),10,1);
 step(&mut rows,&mut db,"reward-change",f::addr(0x12),"setRewardAddress(address)",Some(f::addr(0x33)),0,1);
 step(&mut rows,&mut db,"reward-same",f::addr(0x12),"setRewardAddress(address)",Some(f::addr(0x33)),0,1);
 step(&mut rows,&mut db,"deregister-middle",f::addr(0x11),"deregister()",None,0,1);
 step(&mut rows,&mut db,"append-after-exit",f::addr(0x13),"register(address)",Some(f::addr(0x32)),10,1);
 phases(&mut rows,&mut db,3);
 step(&mut rows,&mut db,"claim-before-due",f::addr(0x11),"claimBond()",None,0,3);
 phases(&mut rows,&mut db,4);
 step(&mut rows,&mut db,"claim-middle-due",f::addr(0x11),"claimBond()",None,0,4);
 step(&mut rows,&mut db,"claim-deleted",f::addr(0x11),"claimBond()",None,0,4);
 // Durability and prefix/budget failure sequences, sharing actual reverted state.
 step(&mut rows,&mut db,"reject-absent-reward",f::addr(0x14),"register(address)",Some(f::addr(0x44)),10,4);
 step(&mut rows,&mut db,"bad-value-before-probe",f::addr(0x14),"register(address)",Some(f::addr(0x44)),0,4);
 let mut db=f::fixture();phases(&mut rows,&mut db,1);
 step(&mut rows,&mut db,"reactivation-register",f::addr(0x12),"register(address)",Some(f::addr(0x32)),10,1);
 step(&mut rows,&mut db,"reactivation-exit",f::addr(0x12),"deregister()",None,0,1);
 phases(&mut rows,&mut db,2);step(&mut rows,&mut db,"reactivate-minus-one",f::addr(0x12),"reactivate()",None,0,2);
 phases(&mut rows,&mut db,3);step(&mut rows,&mut db,"reactivate-equality",f::addr(0x12),"reactivate()",None,0,3);
 phases(&mut rows,&mut db,5);step(&mut rows,&mut db,"inactive-minus-one",f::addr(0x13),"removeInactive(address)",Some(f::addr(0x12)),0,5);
 phases(&mut rows,&mut db,6);step(&mut rows,&mut db,"inactive-equality",f::addr(0x13),"removeInactive(address)",Some(f::addr(0x12)),0,6);
 f::put(&mut db,f::caps(),U256::from(47),U256::from(7));phases(&mut rows,&mut db,7);phases(&mut rows,&mut db,8);
 step(&mut rows,&mut db,"closing-prefix",f::addr(0x14),"register(address)",Some(f::addr(0x44)),10,8);
 let mut db=f::fixture();f::put(&mut db,f::caps(),U256::from(23),U256::from(2));f::put(&mut db,f::caps(),U256::from(17),U256::from(1));phases(&mut rows,&mut db,1);
 step(&mut rows,&mut db,"budget-probe-fail",f::addr(0x12),"register(address)",Some(f::addr(0x44)),10,1);
 step(&mut rows,&mut db,"budget-rmax-at-limit",f::addr(0x12),"register(address)",Some(f::addr(0x32)),10,1);
 step(&mut rows,&mut db,"budget-rmax-plus-one",f::addr(0x13),"register(address)",Some(f::addr(0x33)),10,1);
 step(&mut rows,&mut db,"budget-regops-at-limit",f::addr(0x11),"setRewardAddress(address)",Some(f::addr(0x33)),0,1);
 step(&mut rows,&mut db,"budget-regops-plus-one",f::addr(0x11),"setRewardAddress(address)",Some(f::addr(0x31)),0,1);
 step(&mut rows,&mut db,"reject-duplicate-begin",f::system(),"beginBlock()",None,0,1);
 step(&mut rows,&mut db,"reject-duplicate-mark",f::system(),"markClose()",None,0,1);
 phases(&mut rows,&mut db,2);step(&mut rows,&mut db,"new-height-budget",f::addr(0x13),"register(address)",Some(f::addr(0x33)),10,2);
 let mut db=f::fixture();f::add_exited(&mut db,18,1);f::put(&mut db,f::caps(),U256::from(21),U256::from(16));f::put(&mut db,f::caps(),U256::from(22),U256::from(64));phases(&mut rows,&mut db,1);phases(&mut rows,&mut db,2);
 let mut db=f::fixture();f::add_exited(&mut db,65,1000);f::put(&mut db,f::caps(),U256::from(21),U256::from(16));f::put(&mut db,f::caps(),U256::from(22),U256::from(64));phases(&mut rows,&mut db,1);
 let mut db=f::fixture();phases(&mut rows,&mut db,1);
 step(&mut rows,&mut db,"corrupt-fixture-register",f::addr(0x12),"register(address)",Some(f::addr(0x32)),10,1);
 step(&mut rows,&mut db,"corrupt-fixture-exit",f::addr(0x12),"deregister()",None,0,1);phases(&mut rows,&mut db,4);
 f::put(&mut db,pocol_lp4_execution::pay::registry_address(),f::slot(f::addr(0x12))+U256::from(1),U256::from_be_slice(f::addr(0x44).as_slice()));
 step(&mut rows,&mut db,"corrupt-missing-refund",f::addr(0x12),"claimBond()",None,0,4);
 f::put(&mut db,f::caps(),U256::from(47),U256::from(5));step(&mut rows,&mut db,"corrupt-closing-atomic-failure",f::system(),"markClose()",None,0,5);
 println!("{}",serde_json::to_string(&rows).unwrap());
}
