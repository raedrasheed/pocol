use pocol_lp4_execution::{pool::{Pool,PoolError},native,wire};
use revm::{InMemoryDB,primitives::{U256,Address}};
#[path="support/admission.rs"] mod f;
use f::{raw,profile,fixture};
#[test] fn actual_admission_identity_size_capacity_and_reorg_recheck(){
 let(v,db,block)=fixture();let before=format!("{db:?}");let bytes=raw(v["cases"][0]["raw"].as_str().unwrap());let mut p=Pool::with_budget(profile(777902,None),bytes.len());assert!(!p.identity().bootable());
 let hash=p.submit(&bytes,&db,&block).unwrap();assert_eq!(hash.as_slice(),raw(&v["cases"][0]["view"]["hash"].as_str().unwrap()[2..]));assert_eq!(p.bytes(),bytes.len());assert_eq!(p.pending().len(),1);
 assert_eq!(p.submit(&bytes,&db,&block),Ok(hash));assert_eq!(p.bytes(),bytes.len());assert_eq!(format!("{db:?}"),before);
 let mut wrong=0;for row in v["cases"].as_array().unwrap().iter().filter(|r|matches!(r["error"].as_str(),Some("ChainId"|"UnsupportedType"|"UnprotectedLegacy"))){let error=p.submit(&raw(row["raw"].as_str().unwrap()),&db,&block).unwrap_err();assert_eq!(error.rpc_code(),-32015);wrong+=1;}assert_eq!(wrong,8);
 let another=raw(v["cases"][1]["raw"].as_str().unwrap());assert_eq!(p.submit(&another,&db,&block),Err(PoolError::Full));
 let mut small=Pool::new(profile(777902,Some((bytes.len()-1) as u32)));assert_eq!(small.submit(&bytes,&db,&block),Err(PoolError::Size));
 let mut exact=Pool::new(profile(777902,Some(bytes.len() as u32)));assert!(exact.submit(&bytes,&db,&block).is_ok());
 let changed=InMemoryDB::default();assert!(matches!(p.submit(&bytes,&changed,&block),Err(PoolError::Execution(_))));let rejected=p.revalidate(&changed,&block);assert_eq!(rejected.len(),1);assert!(p.pending().is_empty());assert_eq!(p.bytes(),0);assert_eq!(format!("{db:?}"),before);
}
#[test] fn network_identity_drives_both_signature_domain_and_actual_chainid_opcode(){
 let(v,db,block)=fixture();let before=format!("{db:?}");let row=v["cases"].as_array().unwrap().iter().find(|r|r["name"]=="wrong-chain-0").unwrap();let bytes=raw(row["raw"].as_str().unwrap());let identity=profile(777001,None);
 let tx=wire::decode_for_profile(&bytes,&identity);assert_eq!(tx.is_ok(),true);let tx=tx.unwrap();assert_eq!(tx.tx_env().chain_id,Some(777001));let result=native::execute_for_profile(&db,tx.tx_env(),&block,&identity);assert_eq!(result.is_ok(),true);let result=result.unwrap();
 assert!(result.result.is_success());assert_eq!(result.state[&Address::from([0x22;20])].storage[&U256::ZERO].present_value(),U256::from(777001));
 let candidate=pocol_lp4_execution::user_phase::execute_for_profile(&db,&[bytes.clone()],&block,bytes.len(),1,&identity);assert_eq!(candidate.is_ok(),true);let candidate=candidate.unwrap();assert_eq!(candidate.state().accounts[&Address::from([0x22;20])].storage[&U256::ZERO],U256::from(777001));
 let mut pool=Pool::new(identity);assert!(pool.submit(&bytes,&db,&block).is_ok());assert_eq!(format!("{db:?}"),before);
}

#[test] fn source_identity_hash_and_changed_profile_cannot_reuse_admission(){
 use pocol_lp4_execution::{genesis_spec as gs,profile::{ProfileIdentity,ProfileKind,NodeKind}};
 let bytes=raw(include_str!("data/execution-profile.hex").trim());let hash=gs::genesis_hash(&bytes);assert_eq!(hash.as_slice(),raw("c4fd041f93fef7c445345cb614ac7a1ffe2e56332cedfe652397afb98c529372"));
 let mut wrong=hash;wrong[0]^=1;assert!(ProfileIdentity::validate(&bytes,&wrong,777002,ProfileKind::Synthetic,NodeKind::Simnet,&[(1,0)]).is_err());
 let(v,db,block)=fixture();let tx=raw(v["cases"][0]["raw"].as_str().unwrap());let mut network=Pool::new(profile(777001,None));assert_eq!(network.submit(&tx,&db,&block).unwrap_err().rpc_code(),-32015);
 let mut badblock=block;badblock.gas_limit=U256::from(8_000_001);let mut p=Pool::new(profile(777902,None));assert_eq!(p.submit(&tx,&db,&badblock),Err(PoolError::Context));assert_eq!(p.bytes(),0);
}
