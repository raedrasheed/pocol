//! Upstream Cancun control runner, not PoCol apply or its exception acceptance.
use std::io::{self,BufRead};
use std::str::FromStr;
use revm::{Evm,InMemoryDB,DatabaseCommit};
use revm::primitives::{Address,AccountInfo,Bytecode,Bytes,U256,SpecId,TxKind,AccessListItem,B256};
use serde_json::{Value,json};
fn number(v:&Value)->U256 {U256::from_str(v.as_str().unwrap()).unwrap()}
fn u64n(v:&Value)->u64 {number(v).try_into().unwrap()}
fn bytes(s:&str)->Bytes {let s=s.strip_prefix("0x").unwrap_or(s);assert!(s.len()%2==0);Bytes::from((0..s.len()).step_by(2).map(|i|u8::from_str_radix(&s[i..i+2],16).unwrap()).collect::<Vec<_>>())}
fn hex(b:&[u8])->String {format!("0x{}",b.iter().map(|x|format!("{x:02x}")).collect::<String>())}
fn run(v:Value)->Value {
    let mut db=InMemoryDB::default();
    for (a,info) in v["pre"].as_object().unwrap() {
        let a=Address::from_str(a).unwrap();let code=Bytecode::new_raw(bytes(info["code"].as_str().unwrap()));
        db.insert_account_info(a,AccountInfo::new(number(&info["balance"]),u64n(&info["nonce"]),code.hash_slow(),code));
        for (k,val) in info["storage"].as_object().unwrap(){db.insert_account_storage(a,U256::from_str(k).unwrap(),number(val)).unwrap();}
    }
    let tx=&v["transaction"];let indexes=&v["indexes"];let data=indexes["data"].as_u64().unwrap() as usize;
    let to=tx["to"].as_str().unwrap();let env=&v["env"];
    if let Err(e)=pocol_lp4_execution::native::validate_nonce(&db,Address::from_str(tx["sender"].as_str().unwrap()).unwrap(),Some(u64n(&tx["nonce"]))) {return json!({"error":format!("{e:?}")});}

    // This unsigned control runner does not replace real wire-transaction validation.
    let chosen_value=&tx["value"][indexes["value"].as_u64().unwrap() as usize];
    if U256::from_str(chosen_value.as_str().unwrap()).is_err(){return json!({"error":"ControlInputValueOverflow"});}
    let mut evm=Evm::builder().with_db(db).with_spec_id(SpecId::CANCUN)
        .modify_cfg_env(|c|c.chain_id=1)
        .modify_block_env(|b|{
            b.number=number(&env["currentNumber"]);b.coinbase=Address::from_str(env["currentCoinbase"].as_str().unwrap()).unwrap();
            b.timestamp=number(&env["currentTimestamp"]);b.gas_limit=number(&env["currentGasLimit"]);
            b.basefee=env.get("currentBaseFee").map(number).unwrap_or_default();
            b.difficulty=number(&env["currentDifficulty"]);
            b.prevrandao=Some(env.get("currentRandom").map(|v|B256::from_str(v.as_str().unwrap()).unwrap()).unwrap_or_default());
            b.set_blob_excess_gas_and_price(env.get("currentExcessBlobGas").map(u64n).unwrap_or_default(),false);
        }).modify_tx_env(|t|{
            t.caller=Address::from_str(tx["sender"].as_str().unwrap()).unwrap();
            t.transact_to=if to.is_empty(){TxKind::Create}else{TxKind::Call(Address::from_str(to).unwrap())};
            t.data=bytes(tx["data"][data].as_str().unwrap());t.value=number(&tx["value"][indexes["value"].as_u64().unwrap() as usize]);
            t.gas_limit=u64n(&tx["gasLimit"][indexes["gas"].as_u64().unwrap() as usize]);t.nonce=Some(u64n(&tx["nonce"]));t.chain_id=Some(1);
            if let Some(price)=tx.get("gasPrice"){t.gas_price=number(price)}else{
                t.gas_price=number(&tx["maxFeePerGas"]);t.gas_priority_fee=Some(number(&tx["maxPriorityFeePerGas"]));}
            if let Some(lists)=tx.get("accessLists").and_then(|x|x.get(data)).and_then(|x|x.as_array()){
                t.access_list=lists.iter().map(|a|AccessListItem{address:Address::from_str(a["address"].as_str().unwrap()).unwrap(),storage_keys:a["storageKeys"].as_array().unwrap().iter().map(|k|B256::from_str(k.as_str().unwrap()).unwrap()).collect()}).collect();
            }
            if let Some(blobs)=tx.get("blobVersionedHashes"){t.blob_hashes=blobs.as_array().unwrap().iter().map(|b|B256::from_str(b.as_str().unwrap()).unwrap()).collect();}
            t.max_fee_per_blob_gas=tx.get("maxFeePerBlobGas").map(number);
        }).append_handler_register(pocol_lp4_execution::collision::register).build();
    let r=match evm.transact(){Ok(r)=>r,Err(e)=>return json!({"error":format!("{e:?}")})};
    let removed:Vec<_>=r.state.iter().filter(|(_,a)|a.is_selfdestructed()||(a.is_touched()&&a.is_empty())).map(|(a,_)|*a).collect();
    let gas=r.result.gas_used();let logs:Vec<_>=r.result.logs().iter().map(|l|json!({"address":hex(l.address.as_slice()),"topics":l.data.topics().iter().map(|x|hex(x.as_slice())).collect::<Vec<_>>(),"data":hex(&l.data.data)})).collect();
    evm.context.evm.db.commit(r.state);
    let db=&evm.context.evm.db;let mut state=serde_json::Map::new();
    for (addr,a) in &db.accounts {
        if removed.contains(addr)||a.account_state==revm::db::AccountState::NotExisting {continue;}
        let code=db.contracts.get(&a.info.code_hash).map(|c|c.original_bytes()).unwrap_or_default();
        let storage:serde_json::Map<String,Value>=a.storage.iter().filter(|(_,v)|**v!=U256::ZERO).map(|(k,v)|(format!("{k:#x}"),json!(format!("{v:#x}")))).collect();
        state.insert(hex(addr.as_slice()),json!({"balance":format!("{:#x}",a.info.balance),"nonce":a.info.nonce,"code":hex(&code),"storage":storage}));
    }
    json!({"state":state,"gasUsed":gas,"logs":logs,"success":r.result.is_success()})
}
fn main(){for line in io::stdin().lock().lines(){let line=line.unwrap();let result=std::panic::catch_unwind(||run(serde_json::from_str(&line).unwrap()));let mut result=result.unwrap_or_else(|_|json!({"runnerFailure":true}));result.as_object_mut().unwrap().insert("collisionRule".into(),json!("LP4-CREATE-01"));println!("{}",result);}}
