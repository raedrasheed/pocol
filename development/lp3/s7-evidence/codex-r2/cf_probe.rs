use std::io::{self,BufRead};
use lp1_node::finality_core::*;
use serde_json::{Value,json};
fn main(){for line in io::stdin().lock().lines(){let v:Value=serde_json::from_str(&line.unwrap()).unwrap();let a:Vec<u64>=v["a"].as_array().unwrap().iter().map(|x|x.as_str().unwrap().parse().unwrap()).collect();let p=ParentFacts{closing:v["closing"].as_bool().unwrap(),close_h:a[0],cursor_done:v["done"].as_bool().unwrap(),is_final:v["final"].as_bool().unwrap()};let first=if v["first"].is_null(){None}else{Some(v["first"].as_str().unwrap().parse().unwrap())};
let cursor=match cursor_bound(a[0],a[1],a[2] as u32){Ok(n)=>json!(n.to_string()),Err(s)=>json!({"error":s})};
let gate=match gate2(a[3],a[4],a[5],a[6],a[7] as u32,a[8] as u32){Ok(n)=>json!(n),Err(s)=>json!({"error":s})};
println!("{}",json!({"closing":closing(first,a[3],a[4],a[9],a[10]).map(|n|n.to_string()),"cursor":cursor,"gate":gate,"final":final_predicate(p,a[3],a[4],a[11] as u32,a[5],a[6]),"child":child_allowed(p.is_final,a[3],a[5])}));}}
