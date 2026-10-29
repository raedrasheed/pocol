//! pocol-sign: signs EIP-1559 transactions with a deterministic disposable
//! devnet account key (never a funded key). Prints one 0x-hex envelope per line.
use pocol_lp4_execution::{devnet,txsign::Tx};
use revm::primitives::{Address,Bytes,U256};
fn arg(a:&[String],k:&str)->Option<String>{a.iter().position(|x|x==k).and_then(|i|a.get(i+1)).cloned()}
fn main(){
    let a:Vec<String>=std::env::args().collect();
    let chain_id:u64=arg(&a,"--chain-id").and_then(|v|v.parse().ok()).expect("--chain-id");
    let key=devnet::account_key(&arg(&a,"--account").expect("--account"));
    let to:Option<Address>=arg(&a,"--to").map(|s|s.parse().expect("--to address"));
    let value:U256=arg(&a,"--value").map(|s|s.parse().expect("--value")).unwrap_or_default();
    let count:u64=arg(&a,"--count").and_then(|v|v.parse().ok()).unwrap_or(1);
    let nonce:u64=arg(&a,"--nonce").and_then(|v|v.parse().ok()).unwrap_or(0);
    let gas:u64=arg(&a,"--gas").and_then(|v|v.parse().ok()).unwrap_or(21000);
    let data=arg(&a,"--data").map(|s|pocold::types::unhex(&s).expect("--data hex")).unwrap_or_default();
    let max_fee:U256=arg(&a,"--max-fee").map(|s|s.parse().unwrap()).unwrap_or(U256::from(100_000_000_000u64));
    if a.iter().any(|x|x=="--address"){println!("{:#x}",key.address());return}
    for i in 0..count{
        let tx=Tx{chain_id,nonce:nonce+i,max_priority:U256::from(1_000_000_000u64),max_fee,gas,to,value,data:Bytes::from(data.clone())};
        println!("{}",pocold::types::hex(&tx.sign_1559(&key)));
    }
}
