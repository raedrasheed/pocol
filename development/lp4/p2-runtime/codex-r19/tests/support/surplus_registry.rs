#![allow(dead_code)]
//! Actual source-built Registry/Caps/bond-bridge EVM calls; full apply NOT PASSED.
use pocol_lp4_execution::{native,output,CHAIN_ID,pay::{registry_address,escrow_address}};
use revm::{DatabaseCommit,InMemoryDB,primitives::{Address,AccountInfo,Bytecode,Bytes,TxEnv,TxKind,U256,keccak256,ResultAndState}};
pub fn addr(n:u8)->Address{Address::from([n;20])}
pub fn system()->Address{Address::from_word((U256::from(1)<<160u32).wrapping_sub(U256::from(2)).into())}
pub fn caps()->Address{Address::from_word(U256::from(0xc0c004u64).into())}
pub fn artifact(name:&str)->Bytecode{
 let j:serde_json::Value=serde_json::from_str(include_str!("../data/registry-payment.json")).unwrap();
 let s=j["output"]["contracts"]["RegistryPayment.sol"][name]["evm"]["deployedBytecode"]["object"].as_str().unwrap();
 Bytecode::new_raw(Bytes::from((0..s.len()).step_by(2).map(|i|u8::from_str_radix(&s[i..i+2],16).unwrap()).collect::<Vec<_>>()))
}
pub fn insert(db:&mut InMemoryDB,a:Address,balance:U256,nonce:u64,code:Bytecode){db.insert_account_info(a,AccountInfo::new(balance,nonce,code.hash_slow(),code));}
pub fn slot(id:Address)->U256{U256::from_be_bytes(keccak256([id.into_word().as_slice(),U256::from(7).to_be_bytes::<32>().as_slice()].concat()).0)}
pub fn base(n:u64)->U256{U256::from_be_bytes(keccak256(U256::from(n).to_be_bytes::<32>()).0)}
pub fn storage(db:&InMemoryDB,a:Address,k:U256)->U256{db.accounts[&a].storage.get(&k).copied().unwrap_or_default()}
pub fn put(db:&mut InMemoryDB,a:Address,k:U256,v:U256){db.insert_account_storage(a,k,v).unwrap();}
pub fn reg(db:&InMemoryDB,k:u64)->U256{storage(db,registry_address(),U256::from(k))}
pub fn selector(s:&str)->Vec<u8>{keccak256(s.as_bytes())[..4].to_vec()}
pub fn call(db:&InMemoryDB,caller:Address,to:Address,data:Vec<u8>,value:U256,h:u64)->ResultAndState{
 let tx=TxEnv{caller,transact_to:TxKind::Call(to),data:Bytes::from(data),value,gas_limit:30_000_000,gas_price:U256::ZERO,nonce:Some(db.accounts.get(&caller).map(|a|a.info.nonce).unwrap_or(0)),chain_id:Some(CHAIN_ID),..Default::default()};
 native::execute_at(db,tx,U256::from(h)).unwrap()
}
pub fn invoke(db:&mut InMemoryDB,caller:Address,sig:&str,arg:Option<Address>,value:U256,h:u64)->ResultAndState{
 let mut data=selector(sig);if let Some(a)=arg{data.extend(a.into_word());}
 let r=call(db,caller,registry_address(),data,value,h);db.commit(r.state.clone());r
}
pub fn phase(db:&mut InMemoryDB,h:u64){
 for sig in ["markClose()","beginBlock()"]{let r=invoke(db,system(),sig,None,U256::ZERO,h);assert!(r.result.is_success(),"{sig} at{h}: {:?} {:?}",r.result,output(&r.result));}
}
pub fn fixture()->InMemoryDB{
 let mut db=InMemoryDB::default();
 for a in [system(),addr(0x11),addr(0x12),addr(0x13),addr(0x14),addr(0x15)]{insert(&mut db,a,U256::from(1_000_000u64),1,Bytecode::default());}
 for a in [addr(0x31),addr(0x32),addr(0x33)]{insert(&mut db,a,U256::ZERO,1,Bytecode::default());}
 for (a,name) in [(registry_address(),"MinerRegistry"),(escrow_address(),"RewardEscrow"),(caps(),"ChainCaps")]{insert(&mut db,a,U256::ZERO,1,artifact(name));}
 // Synthetic complete CP inventory, not a ParamGate/profile acceptance fixture.
 for n in 0..52{put(&mut db,caps(),U256::from(n),U256::ZERO);}
 for (n,v) in [(11,2),(16,10),(17,4),(18,10),(19,1),(20,20),(21,0),(22,0),(23,16),(24,2),(25,3),(26,4),(27,3),(47,1000),(48,u64::MAX),(50,2)]{put(&mut db,caps(),U256::from(n),U256::from(v));}
 // One real active/HELD genesis-style record; bond already belongs to Escrow.
 let id=addr(0x11);let r=slot(id);
 for (k,v) in [(U256::ZERO,U256::from(1)),(U256::from(1),U256::from(1)),(U256::from(4),U256::from(10)),(base(5),U256::from_be_slice(id.as_slice())),(base(6),U256::from_be_slice(id.as_slice())),(r,U256::from(1)|(U256::from(1)<<16u32)),(r+U256::from(1),U256::from_be_slice(addr(0x31).as_slice())),(r+U256::from(2),U256::from(10))]{put(&mut db,registry_address(),k,v);}
 db.accounts.get_mut(&escrow_address()).unwrap().info.balance=U256::from(10);db
}
pub fn writes(r:&ResultAndState)->usize{r.state.get(&registry_address()).map(|a|a.storage.values().filter(|s|s.is_changed()).count()).unwrap_or(0)}
pub fn success(r:&ResultAndState){assert!(r.result.is_success(),"assertion: successful EVM call required; {:?}; {:?}",r.result,output(&r.result));}
pub fn error(r:&ResultAndState,sig:&str){assert!(!r.result.is_success());assert_eq!(output(&r.result),selector(sig));}
