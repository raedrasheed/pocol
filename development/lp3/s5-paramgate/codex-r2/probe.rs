use serde_json::{json,Value};
mod fixture {
 include!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/gate.rs"));
 pub fn evaluate(v:&serde_json::Value)->serde_json::Value {
  let end=v["end"].as_bool().unwrap_or(false);let mut p=profile(end);let original=facts(&p);
  if let Some(cp)=v["cp"].as_object() {let mut values=p.cp.to_values();for (name,n) in cp {let idx=gs::cp_index(name).unwrap();values[idx]=U256::from_u64(n.as_u64().unwrap());}p.cp=ConsensusParams::from_values(&values).unwrap();}
  macro_rules! local {($($f:ident),*)=>{$(if let Some(n)=v["local"].get(stringify!($f)){p.local.$f=n.as_u64().unwrap() as _;})*};}
  local!(delta,phi,capital_phi,t_exec,delta_rel,gamma,x_h,x_w,r_keep,k_hist,k_final,k_view,hist_trie_budget);
  if v["stress"].as_bool().unwrap_or(false){p.local.retention_stress=true;}
  if let Some(n)=v.get("gossip") {p.local.gossip[n[0].as_u64().unwrap() as usize]=n[1].as_u64().unwrap() as u32;}
  if v["synthetic"].as_bool().unwrap_or(false){p.kind=ProfileKind::Synthetic;}
  if let Some(n)=v.get("chain"){p.chain_id=n.as_u64().unwrap();}
  if v["v2"].as_bool().unwrap_or(false){p.forks.push((2,50));}
  if let Some(n)=v.get("rpcwall"){p.rpc.batch_wall_ms=n.as_u64().unwrap();}
  refresh(&mut p);
  let (mut r,mut i,mut cal)=if v["stale"].as_bool().unwrap_or(false){original}else{facts(&p)};
  if let Some(n)=v.get("p99"){r.validate_p99_ms=n.as_u64().unwrap();}
  if let Some(n)=v.get("hist"){r.historical_trie_bytes=n.as_u64().unwrap();}
  if let Some(n)=v.get("retained"){r.retained_blocks=n.as_u64().unwrap();}
  if let Some(f)=v["phase"].as_object(){let phase=&mut r.phases[f["index"].as_u64().unwrap() as usize];
   macro_rules! phasefields{($($f:ident),*)=>{$(if let Some(n)=f.get(stringify!($f)){phase.$f=n.as_u64().unwrap() as _;})*};}
   phasefields!(system_keys,user_keys,system_accounts,user_accounts,live_slots,duration_ms,retired_records);
   if let Some(n)=f.get("gas"){phase.call_gas=n.as_array().unwrap().iter().map(|n|n.as_u64().unwrap()).collect();}
  }
  if v["modelBad"].as_bool().unwrap_or(false){r.model.push('x');}
  if v["rootBad"].as_bool().unwrap_or(false){i.alloc_root=[0;32];}
  if v["supplyBad"].as_bool().unwrap_or(false){i.alloc_balances=vec![U256::pow2(127).unwrap()];}
  if v["calBad"].as_bool().unwrap_or(false){cal.hashes_per_second=0;}
  if v["omitPhase"].as_bool().unwrap_or(false){r.phases.pop();}
  let node=if v["node"].as_bool().unwrap_or(!end){NodeKind::Node}else{NodeKind::Simnet};
  let report=check(&p,if v["resourceMissing"].as_bool().unwrap_or(false){None}else{Some(&r)},if v["identityMissing"].as_bool().unwrap_or(false){None}else{Some(&i)},if v["calMissing"].as_bool().unwrap_or(false){None}else{Some(&cal)},node);
  serde_json::json!({"violations":report.violations,"accepted":report.accepted(),"bootable":report.bootable()})
 }
}
fn main(){use std::io::{self,BufRead};for line in io::stdin().lock().lines(){let v:Value=serde_json::from_str(&line.unwrap()).unwrap();println!("{}",fixture::evaluate(&v));}}
