use std::io::{self,BufRead};use lp1_node::{trie,hex,fixed::U256};use serde_json::{Value,json};
fn h(v:&Value)->Vec<u8>{hex::decode(v.as_str().unwrap()).unwrap()}
fn u(v:&Value)->U256{U256::from_dec_str(v.as_str().unwrap()).unwrap()}
fn main(){for line in io::stdin().lock().lines(){let v:Value=serde_json::from_str(&line.unwrap()).unwrap();let budget=v["budget"].as_u64().unwrap() as usize;
 let r=match v["type"].as_str().unwrap(){
 "raw"=>trie::root(&v["entries"].as_array().unwrap().iter().map(|x|(h(&x[0]),h(&x[1]))).collect::<Vec<_>>(),budget),
 "storage"=>trie::storage_root(&v["entries"].as_array().unwrap().iter().map(|x|(u(&x[0]),u(&x[1]))).collect::<Vec<_>>(),budget),
 "ordered"=>trie::ordered_root(&v["entries"].as_array().unwrap().iter().map(h).collect::<Vec<_>>(),budget),
 "state"=>trie::state_root(&v["entries"].as_array().unwrap().iter().map(|x|{let mut a=[0;20];a.copy_from_slice(&h(&x[0]));let mut s=[0;32];s.copy_from_slice(&h(&x[3]));let mut c=[0;32];c.copy_from_slice(&h(&x[4]));trie::Account{address:a,nonce:x[1].as_str().unwrap().parse().unwrap(),balance:u(&x[2]),storage_root:s,code_hash:c}}).collect::<Vec<_>>(),budget),_=>panic!("type")};
 println!("{}",match r{Ok(x)=>json!({"root":hex::encode(&x)}),Err(e)=>json!({"error":format!("{:?}",e)})});}}
