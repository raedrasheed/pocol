extern crate lp1_node;
extern crate serde_json;
use lp1_node::assignment::*;
use lp1_node::fixed::U256;
use lp1_node::genesis_spec::Address;
use lp1_node::hex;
use serde_json::{Value,json};
use std::io::{self,BufRead};
fn address(v:&Value)->Address { hex::decode(v.as_str().unwrap()).unwrap().try_into().unwrap() }
fn run(v:Value)->Value {
    let seal:[u8;32]=hex::decode(v["seal"].as_str().unwrap()).unwrap().try_into().unwrap();
    let height=v["height"].as_u64().unwrap();let attempt=v["attempt"].as_u64().unwrap() as u32;
    let act:Vec<_>=v["act"].as_array().unwrap().iter().map(|r|ParentMember{id:address(&r[0]),tomb:r[1].as_bool().unwrap(),last_share:r[2].as_u64().unwrap(),grace_start:r[3].as_u64().unwrap()}).collect();
    let parent=ParentView{height:v["parentHeight"].as_u64().unwrap(),seal:&seal,act:&act};
    let members=match Membership::from_parent(&parent,height,v["wAct"].as_u64().unwrap() as u32,v["mMax"].as_u64().unwrap() as u16) {Ok(m)=>m,Err(e)=>return json!({"error":format!("{:?}",e)})};
    let target=U256::from_be_slice(&hex::decode(v["target"].as_str().unwrap()).unwrap()).unwrap();
    let mode=if v["mode"].as_u64()==Some(0) {NonceMode::Bounded}else{NonceMode::Unbounded};let c=v["c"].as_u64().unwrap() as u8;
    let max=match nonce_max(&target,mode,c){Ok(n)=>n,Err(e)=>return json!({"error":format!("{:?}",e)})};
    let map=Assignment::new(&members,attempt,&target,mode,c).ok();
    let rows:Vec<_>=v["history"].as_array().unwrap().iter().map(|r|ShareCount{height:r[0].as_u64().unwrap(),id:address(&r[1]),count:r[2].as_u64().unwrap()}).collect();
    let weights=DrawWeights::new(&members,v["ww"].as_u64().unwrap() as u16,&rows).ok();
    let ranges:Vec<_>=map.as_ref().map(|m|m.ranges().iter().map(|r|json!([hex::encode(&r.id()),hex::encode(&r.ticket()),r.start().to_string(),r.end_exclusive().to_string()])).collect()).unwrap_or_default();
    let owners:Vec<_>=v["queries"].as_array().unwrap().iter().map(|n|map.as_ref().and_then(|m|m.owner(n.as_u64().unwrap())).map(|id|hex::encode(&id))).collect();
    let w:Option<Vec<_>>=weights.as_ref().map(|w|w.weights().iter().map(|(id,n)|json!([hex::encode(id),n.to_string()])).collect());
    let choices:Vec<_>=v["indices"].as_array().unwrap().iter().map(|s|weights.as_ref().and_then(|w|w.choose(s.as_str().unwrap().parse().unwrap()).ok()).map(|id|hex::encode(&id))).collect();
    let draw=weights.as_ref().map(|w|{let (id,k)=w.draw().unwrap();json!([hex::encode(&id),k])});
    let samples:Vec<_>=v["samples"].as_array().unwrap().iter().map(|s|{
        let sampler=UnbiasedSampler::new(s[0].as_str().unwrap().parse().unwrap()).unwrap();
        let bytes:[u8;32]=hex::decode(s[1].as_str().unwrap()).unwrap().try_into().unwrap();sampler.sample(&bytes).map(|n|n.to_string())
    }).collect();
    let digests:Vec<_>=[0u32,1,u32::MAX].iter().map(|k|hex::encode(&proposer_digest(&seal,height,*k))).collect();
    json!({"members":members.ids().iter().map(|id|hex::encode(id)).collect::<Vec<_>>(),"seed":hex::encode(&nonce_seed(&seal,height,attempt)),"nonceMax":max.to_string(),"ranges":ranges,"owners":owners,"weights":w,"choices":choices,"draw":draw,"samples":samples,"digests":digests})
}
fn main(){for line in io::stdin().lock().lines(){println!("{}",run(serde_json::from_str(&line.unwrap()).unwrap()));}}
