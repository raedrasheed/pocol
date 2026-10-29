//! Reproduces a missing lifecycle policy, not completed Registry/closing acceptance.
use pocol_lp4_execution::{native,output,CHAIN_ID,pay::registry_address};
use revm::{InMemoryDB,primitives::{Address,AccountInfo,Bytecode,Bytes,TxEnv,TxKind,U256,keccak256}};
#[test] fn pay02_rejects_both_ephemeral_reward_attacks(){
 let j:serde_json::Value=serde_json::from_str(include_str!("data/reward-durability.json")).unwrap();
 let text=j["output"]["contracts"]["RewardDurabilityWitness.sol"]["RewardDurabilityWitness"]["evm"]["deployedBytecode"]["object"].as_str().unwrap();
 let code=Bytecode::new_raw(Bytes::from((0..text.len()).step_by(2).map(|i|u8::from_str_radix(&text[i..i+2],16).unwrap()).collect::<Vec<_>>()));
 for prefunded in [false,true] {
 let mut db=InMemoryDB::default();let user=Address::from([0x11;20]);db.insert_account_info(user,AccountInfo::from_balance(U256::from(1_000_000)));
 db.insert_account_info(registry_address(),AccountInfo::new(U256::from(100),1,code.hash_slow(),code.clone()));
 let reward=registry_address().create(1);
 if prefunded {db.insert_account_info(reward,AccountInfo::from_balance(U256::from(1)));}
 assert_eq!(db.accounts.contains_key(&reward),prefunded);
 let tx=TxEnv{caller:user,transact_to:TxKind::Call(registry_address()),data:Bytes::copy_from_slice(&keccak256("probeThenErase()".as_bytes())[..4]),gas_limit:800_000,gas_price:U256::ZERO,nonce:Some(0),chain_id:Some(CHAIN_ID),..Default::default()};
 let r=native::execute(&db,tx).unwrap();assert!(r.result.is_success());let bytes=output(&r.result);
 assert_eq!(U256::from_be_slice(&bytes[..32]),U256::ZERO,"PAY-02 rejects a recipient without pre-transaction nonce/code");
 assert_eq!(&bytes[44..64],reward.as_slice());
 assert!(r.state[&reward].is_selfdestructed(),"newly-created reward can disappear later in the same Cancun transaction");
 assert_eq!(db.accounts.contains_key(&reward),prefunded);
 }
}
