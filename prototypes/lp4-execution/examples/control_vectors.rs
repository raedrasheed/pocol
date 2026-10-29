use pocol_lp4_execution::{Sandbox,CHAIN_ID,output};
use revm::primitives::{Address,AccountInfo,Bytecode,Bytes,U256};
fn main() {
    let caller=Address::from([0x11;20]); let target=Address::from([0x22;20]);
    let cases=[("store",vec![0x60,1,0x60,0,0x55,0x60,0,0x60,0,0xf3]),("revert",vec![0x60,1,0x60,0,0x55,0x60,0,0x60,0,0xfd]),("chainid",vec![0x46,0x60,0,0x52,0x60,32,0x60,0,0xf3])];
    let mut rows=Vec::new();
    for (name,code) in cases {
        let mut s=Sandbox::default();
        s.snapshot.insert_account_info(caller,AccountInfo::from_balance(U256::from(1_000_000u64)));
        let b=Bytecode::new_raw(Bytes::copy_from_slice(&code));
        s.snapshot.insert_account_info(target,AccountInfo::new(U256::ZERO,0,b.hash_slow(),b));
        let r=s.execute(caller,target,100_000,CHAIN_ID).unwrap();
        rows.push(serde_json::json!({"name":name,"code":code,"success":r.result.is_success(),"gas":r.result.gas_used(),"output":output(&r.result),"slot0":r.state.get(&target).and_then(|a|a.storage.get(&U256::ZERO)).map(|v|v.present_value().to_string()).unwrap_or("0".into())}));
    }
    println!("{}",serde_json::to_string_pretty(&rows).unwrap());
}
