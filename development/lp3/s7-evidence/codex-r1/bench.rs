use lp1_node::{asert::asert,fixed::U256};
use std::time::Instant;
fn main(){let t=U256::pow2(240).unwrap();let inputs=[(t,0i128,10i128,600i128,0i128,0i128),(t,0,60,3600,1i128<<62,7),(t,0,u32::MAX as i128,1,0,u64::MAX as i128),(U256::ONE,0,10,600,153000,0),(U256::ONE,0,10,600,153600,0),(U256::MAX,0,10,600,0,15420),(U256::MAX,0,10,600,0,15360)];let names=["AS1","AS14","AS15","AS16","AS17","AS18","AS19"];let mut medians=Vec::new();let mut checksum=0u64;
 for (name,input) in names.iter().zip(inputs.iter()){for _ in 0..10000{let a=unsafe{std::ptr::read_volatile(input)};checksum^=asert(&a.0,a.1,a.2,a.3,a.4,a.5).unwrap().target.0[0];}
 let mut ns=Vec::new();for _ in 0..11{let start=Instant::now();for _ in 0..100000{let a=unsafe{std::ptr::read_volatile(input)};let o=asert(&a.0,a.1,a.2,a.3,a.4,a.5).unwrap();checksum^=unsafe{std::ptr::read_volatile(&o.target.0[0])};}ns.push(start.elapsed().as_nanos() as f64/100000.0);}ns.sort_by(|a,b|a.partial_cmp(b).unwrap());println!("{{\"case\":\"{}\",\"medianNs\":{},\"minNs\":{},\"maxNs\":{},\"rounds\":11,\"callsPerRound\":100000}}",name,ns[5],ns[0],ns[10]);medians.push(ns[5]);}
 assert!(medians[1..].iter().all(|n|*n<=1_000_000.0&&*n<=10.0*medians[0]));println!("{{\"passed\":true,\"checksum\":{}}}",checksum);
}
