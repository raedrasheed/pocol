use lp1_node::{asert::asert,fixed::U256};
use serde_json::{json,Value};
use std::io::{self,BufRead};
fn main(){for l in io::stdin().lock().lines(){let v:Value=serde_json::from_str(&l.unwrap()).unwrap();let n=|k:&str|v[k].as_str().unwrap().parse::<i128>().unwrap();let tg=U256::from_dec_str(v["target"].as_str().unwrap()).unwrap();let o=asert(&tg,n("g"),n("T"),n("tau"),n("ts"),n("h")).unwrap();println!("{}",json!({"target":o.target.to_dec_string(),"dt":o.trace.delta_t.to_string(),"e":o.trace.e.to_string(),"s":o.trace.s.to_string(),"f":o.trace.f.to_string(),"F":o.trace.factor.map(|f|f.to_string()),"early":match o.early {lp1_node::asert::Early::No=>None,lp1_node::asert::Early::Low=>Some("low"),lp1_node::asert::Early::High=>Some("high")},"maxBits":o.max_bits}));}}
