//! Pure encoding/root fixtures, including repeated envelopes; NOT valid blocks.
use revm::primitives::{Address,B256,Bytes,Log};
use pocol_lp4_execution::receipts;
fn raw(s:&str)->Vec<u8>{s.trim_start_matches("0x").as_bytes().chunks(2).map(|p|u8::from_str_radix(std::str::from_utf8(p).unwrap(),16).unwrap()).collect()}
fn main(){let path=std::env::args().nth(1).unwrap();let rows:Vec<serde_json::Value>=serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();let mut results=Vec::new();
 for count in [0usize,1,2,18,130]{let mut txs=Vec::new();let mut encoded=Vec::new();let mut cumulative=0u64;let mut all_logs=Vec::new();
  for i in 0..count{let row=&rows[i%rows.len()];txs.push(raw(row["raw"].as_str().unwrap()));cumulative=cumulative.checked_add(row["gasUsed"].as_u64().unwrap()).unwrap();
   let logs=row["logs"].as_array().unwrap().iter().map(|l|Log::new(Address::from_slice(&raw(l["address"].as_str().unwrap())),l["topics"].as_array().unwrap().iter().map(|t|B256::from_slice(&raw(t.as_str().unwrap()))).collect(),Bytes::from(raw(l["data"].as_str().unwrap()))).unwrap()).collect::<Vec<_>>();
   encoded.push(receipts::encode(row["decoded"]["kind"].as_u64().unwrap() as u8,row["success"].as_bool().unwrap(),cumulative,&logs).unwrap());all_logs.extend(logs);
  }
  assert!(count==0 || receipts::ordered_root(&txs,count-1).is_err());
  results.push(serde_json::json!({"count":count,"txRoot":format!("{}",receipts::ordered_root(&txs,count).unwrap()),"receiptsRoot":format!("{}",receipts::ordered_root(&encoded,count).unwrap()),"cumulativeGas":cumulative,"bloom":receipts::bloom(&all_logs).to_vec(),"receipts":encoded}));
 }println!("{}",serde_json::to_string_pretty(&results).unwrap());}
