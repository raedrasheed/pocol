#[path="support/admission.rs"] mod f;
use pocol_lp4_execution::{profile::{ProfileIdentity,ProfileKind,NodeKind},pool::Pool,user_phase};
#[test] fn nine_profile_signed_executions_match_independent_full_roots_receipts_and_gas(){
 let rows:Vec<serde_json::Value>=serde_json::from_str(include_str!("data/profile-execution.json")).unwrap();assert_eq!(rows.len(),9);
 for row in rows{let chain=row["chain"].as_u64().unwrap();let profile=ProfileIdentity::validate(&f::raw(row["genesisSpec"].as_str().unwrap()),&f::raw(row["genesisHash"].as_str().unwrap()).try_into().unwrap(),chain,if chain==777001{ProfileKind::Network}else{ProfileKind::Synthetic},if chain==777001{NodeKind::Node}else{NodeKind::Simnet},&[(1,0)]).unwrap();assert!(!profile.bootable());
 let(_,db,block)=f::fixture();let before=format!("{db:?}");let bytes=f::raw(row["raw"].as_str().unwrap());let candidate=user_phase::execute_for_profile(&db,&[bytes.clone()],&block,bytes.len(),1,&profile);assert_eq!(candidate.is_ok(),true);let c=candidate.unwrap();
 assert_eq!(c.state_root(3,1).unwrap().as_slice(),f::raw(row["stateRoot"].as_str().unwrap()));assert_eq!(c.receipts(),&[f::raw(row["receipt"].as_str().unwrap())]);assert_eq!(c.commitments().gas_used,row["gasUsed"].as_u64().unwrap());
 let mut pool=Pool::new(profile);let hash=pool.submit(&bytes,&db,&block).unwrap();assert_eq!(hash.as_slice(),f::raw(row["hash"].as_str().unwrap()));assert_eq!(format!("{db:?}"),before);
 }
}
