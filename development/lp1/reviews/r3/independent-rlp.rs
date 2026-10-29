extern crate lp1_node;
use lp1_node::rlp::{decode,decode_with_depth,encode,ErrKind};
fn wrap(p: &[u8])->Vec<u8>{let mut v=Vec::new();if p.len()<56{v.push(0xc0+p.len() as u8)}else{let b=(p.len() as u64).to_be_bytes();let s=b.iter().position(|x|*x!=0).unwrap();v.push(0xf7+(8-s) as u8);v.extend_from_slice(&b[s..]);}v.extend_from_slice(p);v}
fn main(){let mut checks=0;let mut v=vec![0x80];for d in 0..=2000 {if d<=20 {for limit in 0..=16 {let result=decode_with_depth(&v,limit);assert_eq!(result.is_ok(),d<=limit);if let Ok(x)=result {assert_eq!(encode(&x),v)}checks+=1;}} if d==2000 {assert_eq!(decode(&v).unwrap_err().kind,ErrKind::TooDeep);checks+=1;}v=wrap(&v);}
for limit in [17,32,2001,usize::MAX] {for b in [&v[..],&[0x80][..],&[][..]] {assert_eq!(decode_with_depth(b,limit).unwrap_err().kind,ErrKind::LimitAboveCeiling);checks+=1;}}
for b in [vec![0x81,0],vec![0x80,0x80],vec![0xb8,1,0],vec![0xc2,0x83,1],vec![0xf9,0,56]] {assert!(decode(&b).is_err());checks+=1;}
println!("{{\"independentRlpChecks\":{},\"ok\":true}}",checks);}
