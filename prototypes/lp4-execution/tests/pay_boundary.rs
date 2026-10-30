//! Actual EVM calls through compiled test harnesses, not full system acceptance.
use pocol_lp4_execution::{native, pay::{registry_address,escrow_address}, pay_address, CHAIN_ID, output};
use revm::{InMemoryDB, primitives::{Address,AccountInfo,Bytecode,Bytes,TxEnv,TxKind,U256,keccak256,ResultAndState}};
pub fn user()->Address {Address::from([0x11;20])}
pub fn dest()->Address {Address::from([0x33;20])}
pub fn artifact(name:&str)->Vec<u8> {
    let j:serde_json::Value=serde_json::from_str(include_str!("data/pay-harness.json")).unwrap();
    let s=j["output"]["contracts"]["PayBoundaryHarness.sol"][name]["evm"]["deployedBytecode"]["object"].as_str().unwrap();
    (0..s.len()).step_by(2).map(|i|u8::from_str_radix(&s[i..i+2],16).unwrap()).collect()
}
fn account(db:&mut InMemoryDB,a:Address,balance:U256,code:&[u8]) {
    let code=Bytecode::new_raw(Bytes::copy_from_slice(code));
    db.insert_account_info(a,AccountInfo::new(balance,1,code.hash_slow(),code));
}
pub fn fixture(source:Address,balance:U256)->InMemoryDB {
    let mut db=InMemoryDB::default(); account(&mut db,user(),U256::from(1_000_000),&[]);
    account(&mut db,source,balance,&artifact("PayBoundaryHarness"));
    account(&mut db,dest(),U256::from(7),&artifact("RecipientTrap"));db
}
pub fn word(v:U256)->Vec<u8>{v.to_be_bytes::<32>().to_vec()}
pub fn transfer(to:Address,amount:U256)->Vec<u8>{[to.into_word().to_vec(),word(amount)].concat()}
pub fn selector(signature:&str)->Vec<u8>{keccak256(signature.as_bytes())[..4].to_vec()}
pub fn raw(data:&[u8],gas:u64,value:U256,mode:u8)->Vec<u8>{
    let mut out=selector("sendRaw(bytes,uint256,uint256,uint8)");
    for v in [U256::from(128),U256::from(gas),value,U256::from(mode),U256::from(data.len())] {out.extend(word(v));}
    out.extend(data); out.resize(4+160+data.len().div_ceil(32)*32,0);out
}
pub fn run(db:&InMemoryDB,to:Address,data:Vec<u8>)->ResultAndState {
    let tx=TxEnv{caller:user(),transact_to:TxKind::Call(to),data:Bytes::from(data),gas_limit:1_000_000,gas_price:U256::ZERO,chain_id:Some(CHAIN_ID),nonce:Some(1),..Default::default()};
    native::execute(db,tx).unwrap()
}
pub fn accepted(r:&ResultAndState)->bool {
    assert!(r.result.is_success(),"harness transaction must return normally: {:?}",r.result);
    let out=output(&r.result); assert!(out.len()>=128);U256::from_be_slice(&out[..32])==U256::from(1)
}
pub fn balance(r:&ResultAndState,db:&InMemoryDB,a:Address)->U256{
    r.state.get(&a).map(|x|x.info.balance).unwrap_or_else(||db.accounts[&a].info.balance)
}
#[test] fn both_roles_transfer_no_callback_and_no_snapshot_commit(){
    for source in [registry_address(),escrow_address()] {
        let db=fixture(source,U256::from(100));
        let r=run(&db,source,raw(&transfer(dest(),U256::from(40)),5000,U256::ZERO,0));
        assert!(accepted(&r));assert_eq!(balance(&r,&db,source),U256::from(60));assert_eq!(balance(&r,&db,dest()),U256::from(47));
        assert!(r.result.logs().is_empty()); assert_eq!(output(&r.result).len(),128);
        assert_eq!(db.accounts[&source].info.balance,U256::from(100));assert_eq!(db.accounts[&dest()].info.balance,U256::from(7));
    }
}
#[test] fn funding_zero_self_and_exact_boundaries(){
    let source=registry_address();let db=fixture(source,U256::from(100));
    for (recipient,amount,ok) in [(dest(),0,true),(dest(),100,true),(dest(),101,false),(source,100,true),(source,101,false)] {
        let r=run(&db,source,raw(&transfer(recipient,U256::from(amount)),5000,U256::ZERO,0));
        assert_eq!(accepted(&r),ok);
        if !ok || recipient==source || amount==0 {assert_eq!(balance(&r,&db,source),U256::from(100));}
    }
    for (gas,ok) in [(4999,false),(5000,true),(5001,true)] {
        assert_eq!(accepted(&run(&db,source,raw(&transfer(dest(),U256::from(1)),gas,U256::ZERO,0))),ok);
    }
}
#[test] fn recipients_overflow_and_encoding_reject_without_effects(){
    let source=registry_address();let mut db=fixture(source,U256::from(100));
    let absent=Address::from([0x44;20]);
    for recipient in [Address::ZERO,absent] {
        let r=run(&db,source,raw(&transfer(recipient,U256::from(1)),5000,U256::ZERO,0));
        assert!(!accepted(&r));assert_eq!(balance(&r,&db,source),U256::from(100));
        assert!(!r.state.get(&recipient).map(|x|x.is_touched()).unwrap_or(false));
    }
    account(&mut db,Address::ZERO,U256::from(7),&[]);
    assert!(!accepted(&run(&db,source,raw(&transfer(Address::ZERO,U256::from(1)),5000,U256::ZERO,0))));
    account(&mut db,dest(),U256::MAX,&[]);
    assert!(!accepted(&run(&db,source,raw(&transfer(dest(),U256::from(1)),5000,U256::ZERO,0))));
    account(&mut db,dest(),U256::from(7),&[]);
    let good=transfer(dest(),U256::from(1));let mut badpad=good.clone();badpad[0]=1;
    for data in [vec![],good[..63].to_vec(),[good.clone(),vec![0]].concat(),badpad] {
        let r=run(&db,source,raw(&data,5000,U256::ZERO,0)); assert!(!accepted(&r));assert_eq!(balance(&r,&db,source),U256::from(100));
    }
}
#[test] fn auth_modes_value_and_direct_reject(){
    let source=registry_address();let mut db=fixture(source,U256::from(100));
    let data=transfer(dest(),U256::from(1));
    for mode in [1,2,3] {assert!(!accepted(&run(&db,source,raw(&data,5000,U256::ZERO,mode))));}
    assert!(!accepted(&run(&db,source,raw(&data,5000,U256::from(1),0))));
    let foreign=Address::from([0x55;20]);account(&mut db,foreign,U256::from(100),&artifact("PayBoundaryHarness"));
    assert!(!accepted(&run(&db,foreign,raw(&data,5000,U256::ZERO,0))));
    let r=run(&db,pay_address(),data);assert!(!r.result.is_success());
    assert_eq!(balance(&r,&db,dest()),U256::from(7));
}
#[test] fn nested_revert_restores_both_balances(){
    let source=registry_address();let db=fixture(source,U256::from(100));
    let data=[selector("sendThenRevert(address,uint256)"),transfer(dest(),U256::from(40))].concat();
    let r=run(&db,source,data);assert!(matches!(r.result,revm::primitives::ExecutionResult::Revert{..}));
    assert_eq!(balance(&r,&db,source),U256::from(100));assert_eq!(balance(&r,&db,dest()),U256::from(7));
}
#[test] fn delegated_foreign_code_cannot_spend_registry_balance(){
    let source=registry_address();let foreign=Address::from([0x55;20]);
    let mut db=fixture(source,U256::from(100));account(&mut db,foreign,U256::ZERO,&artifact("PayBoundaryHarness"));
    let inner=raw(&transfer(dest(),U256::from(40)),5000,U256::ZERO,0);
    let mut data=selector("delegateForeign(address,bytes)");data.extend(foreign.into_word());data.extend(word(U256::from(64)));
    data.extend(word(U256::from(inner.len())));data.extend(&inner);data.resize(4+96+inner.len().div_ceil(32)*32,0);
    let r=run(&db,source,data);assert!(r.result.is_success());
    assert_eq!(balance(&r,&db,source),U256::from(100));assert_eq!(balance(&r,&db,dest()),U256::from(7));
}

#[test] fn inherited_static_call_cannot_write_balances(){
    let source=registry_address();let db=fixture(source,U256::from(100));
    let inner=raw(&transfer(dest(),U256::from(40)),5000,U256::ZERO,0);
    let mut data=selector("staticVia(address,bytes)");data.extend(source.into_word());data.extend(word(U256::from(64)));
    data.extend(word(U256::from(inner.len())));data.extend(&inner);data.resize(4+96+inner.len().div_ceil(32)*32,0);
    let r=run(&db,source,data);assert!(r.result.is_success());let out=output(&r.result);
    assert_eq!(U256::from_be_slice(&out[..32]),U256::from(1));
    assert_eq!(U256::from_be_slice(&out[96..128]),U256::ZERO);
    assert_eq!(balance(&r,&db,source),U256::from(100));assert_eq!(balance(&r,&db,dest()),U256::from(7));
}

#[test] fn maximum_nonce_rejected_before_execution(){
    let source=registry_address();let mut db=fixture(source,U256::from(100));
    let mut tx=TxEnv{caller:user(),transact_to:TxKind::Call(source),gas_limit:100_000,gas_price:U256::ZERO,nonce:Some(u64::MAX),chain_id:Some(CHAIN_ID),..Default::default()};
    db.accounts.get_mut(&user()).unwrap().info.nonce=u64::MAX;
    assert!(native::execute(&db,tx.clone()).is_err());
    tx.transact_to=TxKind::Create;assert!(native::execute(&db,tx.clone()).is_err());
    tx.nonce=Some(u64::MAX-1);tx.transact_to=TxKind::Call(source);db.accounts.get_mut(&user()).unwrap().info.nonce=u64::MAX-1;
    assert!(native::execute(&db,tx).is_ok());
}

#[test] fn pay02_requires_actual_pretransaction_nonce_or_code_even_for_zero(){
    let source=registry_address();
    for (nonce,code,balance_before,eligible) in [
        (0,vec![],0,false),(0,vec![],7,false),(1,vec![],0,true),
        (u64::MAX,vec![],0,true),(0,artifact("RecipientTrap"),0,true),
    ] {
        let mut db=fixture(source,U256::from(100));
        let code=Bytecode::new_raw(Bytes::from(code));
        db.insert_account_info(dest(),AccountInfo::new(U256::from(balance_before),nonce,code.hash_slow(),code));
        for amount in [0,1] {
            let r=run(&db,source,raw(&transfer(dest(),U256::from(amount)),5000,U256::ZERO,0));
            assert_eq!(accepted(&r),eligible,"nonce={nonce} balance={balance_before} amount={amount}");
            assert_eq!(balance(&r,&db,source),U256::from(if eligible {100-amount}else{100}));
            assert_eq!(db.accounts[&dest()].info.nonce,nonce);
        }
    }
}

#[test] fn pay02_existing_code_survives_selfdestruct_and_stays_payable_next_transaction(){
    use revm::DatabaseCommit;
    let source=registry_address();let mut db=fixture(source,U256::from(100));
    let code=Bytecode::new_raw(Bytes::from(vec![0x33,0xff])); // CALLER; SELFDESTRUCT
    db.insert_account_info(dest(),AccountInfo::new(U256::from(7),0,code.hash_slow(),code.clone()));
    let erased=run(&db,dest(),vec![]);assert!(erased.result.is_success());
    assert!(!erased.state[&dest()].is_selfdestructed(),"EIP-6780 preserves old account identity/code");
    db.commit(erased.state);
    assert_eq!(db.accounts[&dest()].info.nonce,0);
    assert_eq!(db.accounts[&dest()].info.code_hash,code.hash_slow());
    let tx=TxEnv{caller:user(),transact_to:TxKind::Call(source),data:Bytes::from(raw(&transfer(dest(),U256::from(40)),5000,U256::ZERO,0)),gas_limit:1_000_000,gas_price:U256::ZERO,chain_id:Some(CHAIN_ID),nonce:Some(2),..Default::default()};
    let paid=native::execute(&db,tx).unwrap();assert!(accepted(&paid));
    assert_eq!(balance(&paid,&db,dest()),U256::from(40));
    assert_eq!(balance(&paid,&db,source),U256::from(60));
}
