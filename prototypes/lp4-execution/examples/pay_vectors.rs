//! Synthetic compiled-harness vectors; not complete PoCol system acceptance.
#[path="../tests/pay_boundary.rs"]
#[allow(dead_code,unused_imports)]
mod f;
use revm::primitives::{U256,Address};
use pocol_lp4_execution::{pay::{registry_address,escrow_address},output};
fn main(){
    let mut rows=Vec::new();
    for source in [registry_address(),escrow_address()] {
        for (name,recipient,amount,gas,value,mode) in [
            ("valid",f::dest(),40u64,5000u64,0u64,0u8),
            ("zero",f::dest(),0,5000,0,0),("exact-balance",f::dest(),100,5000,0,0),
            ("underfunded",f::dest(),101,5000,0,0),("self",source,100,5000,0,0),
            ("gas-minus-one",f::dest(),40,4999,0,0),("gas-plus-one",f::dest(),40,5001,0,0),
            ("absent",Address::from([0x44;20]),40,5000,0,0),("zero-address",Address::ZERO,0,5000,0,0),
            ("value",f::dest(),40,5000,1,0),("static",f::dest(),40,5000,0,1),
            ("delegate",f::dest(),40,5000,0,2),("callcode",f::dest(),40,5000,0,3)] {
            let db=f::fixture(source,U256::from(100));
            let input=f::raw(&f::transfer(recipient,U256::from(amount)),gas,U256::from(value),mode);
            let r=f::run(&db,source,input.clone());
            rows.push(serde_json::json!({"name":name,"source":format!("{source}"),"input":input,"output":output(&r.result),"gasUsed":r.result.gas_used(),"sourceBalance":f::balance(&r,&db,source).to_string(),"destBalance":f::balance(&r,&db,f::dest()).to_string(),"logs":r.result.logs().len()}));
        }
    }
    for source in [registry_address(),escrow_address()] {
        let db=f::fixture(source,U256::from(100));let payload=f::transfer(f::dest(),U256::from(40));
        let mut input=f::selector("twoCalls(bytes,uint256)");
        input.extend(f::word(U256::from(64)));input.extend(f::word(U256::from(5000)));input.extend(f::word(U256::from(payload.len())));input.extend(payload);
        let r=f::run(&db,source,input.clone());
        assert!(r.result.is_success());assert_eq!(output(&r.result),[f::word(U256::from(1)),f::word(U256::from(1))].concat());
        assert_eq!(f::balance(&r,&db,source),U256::from(20));assert_eq!(f::balance(&r,&db,f::dest()),U256::from(87));
        rows.push(serde_json::json!({"name":"warm-second-call","source":format!("{source}"),"input":input,"output":output(&r.result),"gasUsed":r.result.gas_used(),"sourceBalance":f::balance(&r,&db,source).to_string(),"destBalance":f::balance(&r,&db,f::dest()).to_string(),"logs":r.result.logs().len()}));
    }
    for source in [registry_address(),escrow_address()] {
        for (name,nonce,code,balance) in [
            ("pay02-balance-only",0,vec![],7u64),
            ("pay02-empty",0,vec![],0),
            ("pay02-nonce-only",1,vec![],0),
            ("pay02-code-only",0,f::artifact("RecipientTrap"),0),
        ] {
            for amount in [0u64,1] {
                let mut db=f::fixture(source,U256::from(100));
                let bytecode=revm::primitives::Bytecode::new_raw(revm::primitives::Bytes::from(code.clone()));
                db.insert_account_info(f::dest(),revm::primitives::AccountInfo::new(U256::from(balance),nonce,bytecode.hash_slow(),bytecode));
                let input=f::raw(&f::transfer(f::dest(),U256::from(amount)),5000,U256::ZERO,0);
                let r=f::run(&db,source,input.clone());
                assert_eq!(f::accepted(&r),nonce>0 || !code.is_empty());
                rows.push(serde_json::json!({"name":format!("{name}-{amount}"),"source":format!("{source}"),"recipientAccount":{"nonce":nonce,"balance":balance,"code":code},"input":input,"output":output(&r.result),"gasUsed":r.result.gas_used(),"sourceBalance":f::balance(&r,&db,source).to_string(),"destBalance":f::balance(&r,&db,f::dest()).to_string(),"logs":r.result.logs().len()}));
            }
        }
    }
    println!("{}",serde_json::to_string_pretty(&rows).unwrap());
}
