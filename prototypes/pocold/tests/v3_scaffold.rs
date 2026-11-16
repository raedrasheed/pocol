//! V3 scaffold regressions: old encodings (decoded, and refused at admission since phase 6a), the spec-2 release decoder
//! and public genesis tooling.
use pocol_lp4_execution::{admission::NodeLocal,apply::{self,ProgramSet},devnet,genesis_spec as gs,
    profile::{ConsensusParams,ForkSchedule,NodeKind,ProfileKind}};
use revm::primitives::U256;
use std::{path::{Path,PathBuf},sync::atomic::{AtomicU64,Ordering}};

struct Temp(PathBuf);
impl Temp{fn new(label:&str)->Self{static N:AtomicU64=AtomicU64::new(0);Self(std::env::temp_dir().join(format!("pocol-v3-{label}-{}-{}",std::process::id(),N.fetch_add(1,Ordering::Relaxed))))}}
impl Drop for Temp{fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
fn config()->serde_json::Value{
    let miner=devnet::miner_identity("v3-scaffold");
    serde_json::json!({"chainId":777931,"genesisTime":1_800_000_000u64,
        "registrationPerBlock":1,"miners":[{"id":miner.id,"reward":miner.reward}],
        "accounts":[{"address":devnet::account_key("v3-scaffold-funder").address(),"wei":"1000000000000000000000000"}]})
}
fn prepare(config:&serde_json::Value,out:&Path)->Result<(),String>{pocold::testnet::prepare(config.to_string().as_bytes(),out)}
fn read_spec(path:&Path)->gs::GenesisSpec{
    let text=std::fs::read_to_string(path.join("genesis.spec.hex")).unwrap();
    gs::decode(&pocold::types::unhex(text.trim()).unwrap()).unwrap()
}

#[test]
fn shared_codec_and_profile_sources_stay_byte_identical(){
    assert_eq!(include_bytes!("../../lp1-node/src/genesis_spec.rs").as_slice(),include_bytes!("../../lp4-execution/src/p1_genesis_spec.rs").as_slice());
    assert_eq!(include_bytes!("../../lp1-node/src/profile.rs").as_slice(),include_bytes!("../../lp4-execution/src/p1_profile.rs").as_slice());
}

/// The sequential schedule's versions; an admitted spec keeps the full nonce domain at each of them (nonceMode 1 is
/// required by admission: tests/nonce_domain.rs the_full_nonce_domain_holds_at_every_fork_version).
#[test]
fn sequential_future_forks_never_narrow_the_nonce_domain(){
    let forks=ForkSchedule::new_bounded(&[(1,0),(2,10),(3,20)],3).unwrap();
    for (h,version) in [(0,1),(9,1),(10,2),(19,2),(20,3),(21,3),(u64::MAX,3)]{assert_eq!(forks.active_version(h),version,"h={h}");}
    assert_eq!(gs::SPEC_VERSION,2);assert_eq!(pocol_lp4_execution::profile::MAX_FORK_VERSION,3);
    assert_eq!(pocold::identity::CONSENSUS_RULES_VERSION,2);
}
#[test]
fn schema2_roundtrips_full_width_fields_and_respects_the_release_ceiling(){
    // From a 52-field (CP schema v1) GenesisSpec extended to schema v2 by hand: the devnet's CP encoded at specVersion 1
    // (the decoder still reads CP schema v1, so that a spec-1 genesis is refused at admission with its reason).
    let mut cfg=devnet::DevnetConfig::new(777932,1_700_000_000,&["one"]);cfg.spec_version=1;cfg.nonce_mode=0;
    let m=devnet::miner_identity("one");
    let mut spec=gs::GenesisSpec{spec_version:1,chain_id:777932,cp:cfg.cp(),alloc_root:[0;32],m0_list:vec![gs::Member{id:m.id.0.0,reward_addr:m.reward.0.0}],sys_code_hash:[0;32]};
    assert_eq!(gs::decode(&gs::encode(&spec)).unwrap(),spec,"a CP schema v1 GenesisSpec decodes");
    let old=ConsensusParams::from_values(&spec.cp).unwrap();assert_eq!(old.H_half,0);assert_eq!(old.baseFeeMin,pocol_lp4_execution::fixed::U256::ZERO);
    spec.spec_version=2;spec.cp.extend(gs::CP_SCHEMA_V2[52..].iter().map(|f|pocol_lp4_execution::fixed::U256::from_u64(f.lo)));
    for name in ["S_tail","baseFeeMin"]{let i=gs::cp_index_for(2,name).unwrap();spec.cp[i]=pocol_lp4_execution::fixed::U256::MAX;}
    let bytes=gs::encode(&spec);assert_eq!(gs::decode_bounded(&bytes,2).unwrap(),spec);
    assert_eq!(pocold::genesis_spec::encode(&pocold::genesis_spec::decode_bounded(&bytes,2).unwrap()),bytes);
    assert_eq!(gs::decode_bounded(&bytes,1).unwrap_err().code,"gsVersion");
    assert_eq!(gs::decode(&bytes).unwrap(),spec);
    let typed=ConsensusParams::from_values(&spec.cp).unwrap();assert_eq!(typed.baseFeeMin,pocol_lp4_execution::fixed::U256::MAX);
    // FN-2a (owner decision 10d) deleted the three F0_* creation-fee fields from CP schema v2, FN-2b (owner decisions
    // 10b/10c) P_MAX, T_PROB, E_epoch, theta_bp and k_sigma: 59 fields.
    assert_eq!(spec.cp.len(),59);for gone in ["F0_acct","F0_slot","F0_code","P_MAX","T_PROB","E_epoch","theta_bp","k_sigma"]{assert_eq!(gs::cp_index_for(2,gone),None);}assert_eq!(typed.to_values(),spec.cp);
    for (name,bad) in [("gamma_tip_bp",10001),("rho_bp",10001),("R_react",256),("C_react",1u64<<32),("H_half",0)]{
        let mut invalid=spec.clone();invalid.cp[gs::cp_index_for(2,name).unwrap()]=pocol_lp4_execution::fixed::U256::from_u64(bad);
        let err=gs::decode_bounded(&gs::encode(&invalid),2).unwrap_err();assert_eq!((err.code,err.detail.as_str()),("gsRange",name));
    }
}
#[test]
fn public_cp_overrides_fail_closed_before_writing_any_output(){
    let cases=[("unknown",serde_json::json!({"unknown":1}),"unknown CP field"),
        ("range",serde_json::json!({"R_max":256}),"gsRange"),
        ("derived",serde_json::json!({"target_g":1}),"derived"),
        ("float",serde_json::json!({"T_blk":1.5}),"unsigned integer"),
        ("negative",serde_json::json!({"T_blk":-1}),"unsigned integer"),
        ("zero",serde_json::json!({"T_blk":0}),"gsRange"),
        ("huge",serde_json::json!({"B_reg":"115792089237316195423570985008687907853269984665640564039457584007913129639936"}),"outside uint256"),
        ("nonce",serde_json::json!({"nonceMode":0}),"contradicts")];
    for (name,cp,want) in cases{let out=Temp::new(name);let mut input=config();input["cp"]=cp;
        let err=prepare(&input,&out.0).unwrap_err();assert!(err.contains(want),"{name}: {err}");assert!(!out.0.exists());}
}
#[test]
fn resolved_cp_drives_bond_target_timing_and_the_actual_program_registry(){
    let out=Temp::new("overrides");let mut input=config();input["cp"]=serde_json::json!({"T_blk":30,"D_att":150,"dFbWait":10,"B_reg":"25000000000000000000"});
    prepare(&input,&out.0).unwrap();let spec=read_spec(&out.0);assert_eq!(spec.spec_version,2,"every genesis is v3 (phase 6a)");
    let cp=ConsensusParams::from_values(&spec.cp).unwrap();assert_eq!((cp.T_blk,cp.D_att,cp.dFbWait),(30,150,10));
    assert_eq!(cp.target_g,pocol_lp4_execution::gate::calibration_target((1<<18)/10,30).unwrap());
    assert_eq!(devnet::from_fixed(&cp.B_reg),U256::from(25_000_000_000_000_000_000u128));
    // V3-05 at the V3_DEFAULT assumptions (1, 1, 1): Δ_rel 7, recorded in devnet.json and derived again by the launcher.
    let local=pocold::testnet::local_for_network(&out.0).unwrap();assert_eq!((local.t_exec,local.delta_rel),(1,7));
    let info:serde_json::Value=serde_json::from_slice(&std::fs::read(out.0.join("devnet.json")).unwrap()).unwrap();
    assert_eq!((info["localTiming"]["deltaRel"].as_u64(),info["localTiming"]["gamma"].as_u64()),(Some(u64::from(local.delta_rel)),Some(u64::from(local.gamma))));
    let admitted=pocold::load_spec_for(&out.0,&local,None).unwrap();assert_eq!(admitted.programs(),ProgramSet::V3);assert!(admitted.v3_active(0));
    assert_eq!(spec.sys_code_hash,apply::SystemPrograms::v3().sys_code_hash().0);
    let miner=devnet::miner_identity("v3-scaffold");assert_eq!(apply::miner_record(admitted.genesis().state(),miner.id).unwrap().bond,U256::from(25_000_000_000_000_000_000u128));
}
#[test]
fn v3_config_implies_the_new_schema_and_full_nonce(){
    let out=Temp::new("v3-config");let mut input=config();input["v3"]=serde_json::json!(true);
    input["cp"]=serde_json::json!({"T_blk":30,"D_att":150,"dFbWait":10,"M_max":256});
    prepare(&input,&out.0).unwrap();
    let spec=read_spec(&out.0);assert_eq!(spec.spec_version,2);assert_eq!(spec.cp.len(),59);
    let cp=ConsensusParams::from_values(&spec.cp).unwrap();assert_eq!((cp.nonceMode,cp.H_END,cp.M_max),(1,0,256));
    let local=pocold::testnet::local_for_network(&out.0).unwrap();let admitted=pocold::load_spec_for(&out.0,&local,None).unwrap();assert!(admitted.v3_active(0));assert!(admitted.v3_active(u64::MAX));
    // V3-PR-E0: a v3 config selects the compiled v3 set, which has no probation pool or deadline since FN-2b.
    assert_eq!(admitted.programs(),ProgramSet::V3);
    assert_eq!(spec.sys_code_hash,apply::SystemPrograms::v3().sys_code_hash().0);
    for (field,value) in [("rho_bp",10001),("H_half",0)]{
        let invalid=Temp::new(field);let mut config=input.clone();config["cp"][field]=serde_json::json!(value);
        assert!(prepare(&config,&invalid.0).unwrap_err().contains("gsRange"));assert!(!invalid.0.exists());
    }
    // The deleted fields are unknown CP fields of a v3 config (FN-2b).
    for gone in ["P_MAX","T_PROB","E_epoch","theta_bp","k_sigma"]{
        let invalid=Temp::new(gone);let mut config=input.clone();config["cp"][gone]=serde_json::json!(1);
        assert!(prepare(&config,&invalid.0).unwrap_err().contains("unknown CP field"),"{gone}");assert!(!invalid.0.exists());
    }
}
/// Phase 6a: a spec-1 network's files still decode (CP schema v1), so the launcher and the boot refuse them with their
/// reason rather than as unreadable files: the local timing (`local_for_network`) and the boot admission (R24) both name
/// the retired spec-1 rule set, and nothing boots.
#[test]
fn a_spec1_network_decodes_and_is_refused_with_its_reason(){
    let out=Temp::new("spec1-net");std::fs::create_dir_all(&out.0).unwrap();
    let mut cfg=devnet::DevnetConfig::new(777935,1_700_000_000,&["one"]);cfg.spec_version=1;cfg.nonce_mode=0;
    let m=devnet::miner_identity("one");
    let spec=gs::GenesisSpec{spec_version:1,chain_id:777935,cp:cfg.cp(),alloc_root:[0;32],m0_list:vec![gs::Member{id:m.id.0.0,reward_addr:m.reward.0.0}],sys_code_hash:[0;32]};
    std::fs::write(out.0.join("genesis.spec.hex"),pocold::types::hex(&gs::encode(&spec))).unwrap();
    std::fs::write(out.0.join("alloc.json"),"{}").unwrap();
    std::fs::write(out.0.join("devnet.json"),serde_json::json!({"calibration":{"hashesPerSecond":1024}}).to_string()).unwrap();
    assert_eq!(pocold::testnet::spec_version_of(&out.0),Ok(1),"it decodes");
    assert!(pocold::testnet::local_for_network(&out.0).unwrap_err().contains("spec-1 rule set is retired"));
    let e=pocold::load_spec_for(&out.0,&pocold::single_host_local(),None).err().expect("a spec-1 genesis never boots");
    assert!(e.starts_with("-32018 ParamGate rejected [R24]")&&e.contains("GenesisSpec version 1: the spec-1 rule set is retired"),"{e}");
}
/// V3-05 on a v3 genesis: Δ_rel = σ + Δ + 2·t_exec from the declared assumptions, V3_DEFAULT (1,1,1: 7) by default
/// since FN-4 (FRESH-NETWORK-DESIGN §1c; V3_WAN, 13, before); devnet.json records them and the launcher derives the same;
/// assumptions the CP cannot carry (R3) are refused by admission before any output.
#[test]
fn v3_05_derives_delta_rel_for_v3_profiles_from_declared_timing(){
    let v3=|cp:serde_json::Value,timing:Option<serde_json::Value>|{let mut input=config();
        input["v3"]=serde_json::json!(true);input["cp"]=cp;if let Some(t)=timing{input["localTiming"]=t;}input};
    let ob=serde_json::json!({"T_blk":30,"D_att":150,"dFbWait":10});
    let out=Temp::new("v3-default");prepare(&v3(ob.clone(),None),&out.0).unwrap();
    let info:serde_json::Value=serde_json::from_slice(&std::fs::read(out.0.join("devnet.json")).unwrap()).unwrap();
    assert_eq!(info["localTiming"],serde_json::json!({"delta":1,"phi":1,"tExec":1,"deltaRel":7,"gamma":5,"source":"v3-default"}));
    let l=pocold::testnet::local_for_network(&out.0).unwrap();assert_eq!((l.delta,l.phi,l.t_exec,l.delta_rel,l.gamma),(1,1,1,7,5));
    pocold::load_spec_for(&out.0,&l,None).unwrap();
    // The O-6=b WAN assumptions declared (the pre-FN-4 default): Δ_rel 13, Γ 8.
    let wan=serde_json::json!({"delta":2,"phi":2,"tExec":2});
    let out=Temp::new("v3-wan");prepare(&v3(ob.clone(),Some(wan.clone())),&out.0).unwrap();
    let info:serde_json::Value=serde_json::from_slice(&std::fs::read(out.0.join("devnet.json")).unwrap()).unwrap();
    assert_eq!(info["localTiming"],serde_json::json!({"delta":2,"phi":2,"tExec":2,"deltaRel":13,"gamma":8,"source":"declared"}));
    let l=pocold::testnet::local_for_network(&out.0).unwrap();assert_eq!((l.delta,l.phi,l.t_exec,l.delta_rel,l.gamma),(2,2,2,13,8));
    pocold::load_spec_for(&out.0,&l,None).unwrap();
    // Without devnet.json the launcher falls back to the V3_DEFAULT assumptions.
    std::fs::remove_file(out.0.join("devnet.json")).unwrap();let l=pocold::testnet::local_for_network(&out.0).unwrap();assert_eq!((l.delta_rel,l.gamma),(7,5));
    // Declared single-host assumptions: Δ_rel 7 (the R2 minimum), Γ 5; the launcher reads them back.
    let out=Temp::new("v3-declared");prepare(&v3(ob.clone(),Some(serde_json::json!({"delta":1,"phi":1,"tExec":1}))),&out.0).unwrap();
    let info:serde_json::Value=serde_json::from_slice(&std::fs::read(out.0.join("devnet.json")).unwrap()).unwrap();
    assert_eq!((info["localTiming"]["deltaRel"].as_u64(),info["localTiming"]["gamma"].as_u64(),info["localTiming"]["source"].as_str()),(Some(7),Some(5),Some("declared")));
    let l=pocold::testnet::local_for_network(&out.0).unwrap();assert_eq!((l.delta_rel,l.gamma),(7,5));pocold::load_spec_for(&out.0,&l,None).unwrap();
    // Without devnet.json the launcher falls back to the V3_DEFAULT assumptions, the ones declared here.
    std::fs::remove_file(out.0.join("devnet.json")).unwrap();let l=pocold::testnet::local_for_network(&out.0).unwrap();assert_eq!((l.delta_rel,l.gamma),(7,5));
    // The governance WAN locals need dFbWait ≥ Φ + Δ + 2·t_exec = 21 (R3): refused at the O-6=b CP, admitted at (c).
    let gov=serde_json::json!({"delta":5,"phi":5,"tExec":3});
    let out=Temp::new("v3-r3");let err=prepare(&v3(ob.clone(),Some(gov.clone())),&out.0).unwrap_err();assert!(err.contains("R3"),"{err}");assert!(!out.0.exists());
    let out=Temp::new("v3-60s");prepare(&v3(serde_json::json!({"T_blk":60,"D_att":300,"dFbWait":25}),Some(gov)),&out.0).unwrap();
    let l=pocold::testnet::local_for_network(&out.0).unwrap();assert_eq!(l.delta_rel,25);
    // A v3 devnet.json written before V3-05 (1/1/1 with deltaRel = dFbWait + 2 and no source) or edited by hand is
    // refused, not silently re-derived; recorded values equal to the derivation, or none, are accepted.
    let out=Temp::new("v3-pre-n1");prepare(&v3(ob.clone(),Some(serde_json::json!({"delta":1,"phi":1,"tExec":1}))),&out.0).unwrap();
    let path=out.0.join("devnet.json");let mut info:serde_json::Value=serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    for (recorded,refused) in [(serde_json::json!({"delta":1,"phi":1,"tExec":1,"deltaRel":12,"gamma":5}),Some("records deltaRel 12 and gamma 5, but V3-05 derives deltaRel 7 and gamma 5")),
        (serde_json::json!({"delta":1,"phi":1,"tExec":1,"deltaRel":7,"gamma":6,"source":"declared"}),Some("records deltaRel 7 and gamma 6")),
        (serde_json::json!({"delta":1,"phi":1,"tExec":1,"deltaRel":-1}),Some("localTiming.deltaRel must be an unsigned integer")),
        (serde_json::json!({"delta":1,"phi":1,"tExec":1,"deltaRel":7,"gamma":5}),None),(serde_json::json!({"delta":1,"phi":1,"tExec":1}),None)]{
        info["localTiming"]=recorded.clone();std::fs::write(&path,info.to_string()).unwrap();
        match (pocold::testnet::local_for_network(&out.0),refused){
            (Err(e),Some(want))=>assert!(e.contains(want),"{recorded}: {e}"),
            (Ok(l),None)=>assert_eq!((l.delta_rel,l.gamma),(7,5),"{recorded}"),
            (r,_)=>panic!("{recorded}: {r:?}"),
        }
    }
    // `pocold status` (and `run`) on a v3 network take the network's V3-05 timing, as the launcher does, with the
    // command-line overrides on top (declared WAN assumptions, distinct from the single-host locals).
    let out=Temp::new("v3-cli");prepare(&v3(ob.clone(),Some(wan)),&out.0).unwrap();
    let local=|extra:&[&str]|{let data=Temp::new("v3-cli-data");
        let o=std::process::Command::new(env!("CARGO_BIN_EXE_pocold")).args(["status","--network",out.0.to_str().unwrap(),"--data",data.0.to_str().unwrap()]).args(extra).output().unwrap();
        assert!(o.status.success(),"{}",String::from_utf8_lossy(&o.stderr));
        let st:serde_json::Value=serde_json::from_slice(&o.stdout).unwrap();st["templateLatency"]["local"].clone()};
    let l=local(&[]);assert_eq!((l["delta"].as_u64(),l["phi"].as_u64(),l["tExec"].as_u64(),l["deltaRel"].as_u64(),l["gamma"].as_u64()),(Some(2),Some(2),Some(2),Some(13),Some(8)),"{l}");
    // (An override is applied as given and admitted as usual: a larger Γ keeps R7.)
    let l=local(&["--gamma","20"]);assert_eq!((l["deltaRel"].as_u64(),l["gamma"].as_u64()),(Some(13),Some(20)),"{l}");
    // Malformed declarations fail closed.
    for (name,bad) in [("zero",serde_json::json!({"delta":0,"phi":1,"tExec":1})),("huge",serde_json::json!({"delta":3601,"phi":1,"tExec":1})),
        ("unknown",serde_json::json!({"delta":1,"phi":1,"tExec":1,"deltaRel":4})),("missing",serde_json::json!({"delta":1,"phi":1}))]{
        let out=Temp::new(name);assert!(prepare(&v3(ob.clone(),Some(bad)),&out.0).is_err(),"{name}");assert!(!out.0.exists());
    }
}
#[test]
fn a_simnet_devnet_is_refused_in_every_non_simnet_context(){
    let cfg=devnet::DevnetConfig::new(777933,1_700_000_000,&["one"]);let net=devnet::build(&cfg).unwrap();
    let local=NodeLocal::single_host();let (admitted,timing)=net.admit(&local).unwrap();assert!(admitted.profile().v3_active(0),"the v3 devnet");
    for (kind,node) in [(ProfileKind::Synthetic,NodeKind::Simnet),(ProfileKind::Network,NodeKind::Node),(ProfileKind::Simnet,NodeKind::Node)]{
        let mut request=net.request(&local,Some(&timing));request.kind=kind;request.node=node;
        let result=pocol_lp4_execution::admission::admit(&request);
        // No build-policy guard precedes R22 any more (the v3-preview guard is gone with the feature), except caps-test's.
        let want=if pocol_lp4_execution::CAPS_TEST{"caps-test"}else{"R22"};
        assert_eq!(result.err().expect("a simnet profile outside a simnet node is refused").rules,[want],"{kind:?} on {node:?}");
    }
}
