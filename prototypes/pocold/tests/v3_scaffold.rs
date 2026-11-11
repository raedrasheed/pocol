//! V3 scaffold regressions: old encodings, isolated preview and public genesis tooling.
use pocol_lp4_execution::{admission::NodeLocal,apply::{self,ProgramSet},devnet,genesis_spec as gs,
    profile::{ConsensusParams,ForkSchedule,NodeKind,ProfileIdentity,ProfileKind}};
use revm::primitives::U256;
use sha2::{Digest,Sha256};
use std::{path::{Path,PathBuf},sync::atomic::{AtomicU64,Ordering}};

const GENESIS: &[(&str,&str)] = &[
    ("deploy/mainnet/h15000/network.pocol.json#files/genesis.spec.hex", "650ab748ef23c4906d14ec677fb0eefd0a3c384b5171827b31e12d8909dc71e7"),
    ("deploy/websites/pocol.net/downloads/network.pocol.json#files/genesis.spec.hex", "650ab748ef23c4906d14ec677fb0eefd0a3c384b5171827b31e12d8909dc71e7"),
    ("development/audit-2026-10-22/evidence/r4/accept-051dfaa/kept/e2e_browser-2026-10-28T163501406Z-1354/work/net/genesis.spec.hex", "192242af7f4289006c0cd2d9266cec74209d7f992293fa39cc2339fa82a8882f"),
    ("development/audit-2026-10-22/evidence/r4/accept-051dfaa/kept/e2e_browser-2026-10-28T163501406Z-1354/work/other/genesis.spec.hex", "8f71c221b7cb9a4d11e75214e579a5655b85e8248d5515f7a26faa5fb3564bba"),
    ("development/audit-2026-10-22/evidence/r4/accept-051dfaa/kept/e2e_browser-2026-10-28T165527020Z-19707/work/net/genesis.spec.hex", "4277c31ca87225842dd640f8185594f572084e292fbf4413def2878d1638c6b1"),
    ("development/audit-2026-10-22/evidence/r4/accept-051dfaa/kept/e2e_browser-2026-10-28T165527020Z-19707/work/other/genesis.spec.hex", "8f71c221b7cb9a4d11e75214e579a5655b85e8248d5515f7a26faa5fb3564bba"),
    ("development/audit-2026-10-22/evidence/r4/accept-719da04/kept/e2e_browser-2026-10-28T151315995Z-28289/work/net/genesis.spec.hex", "b5892451a25c4b34760a028e1b09d399eb239d010905a3c2f7cb8fc116b07b5d"),
    ("development/audit-2026-10-22/evidence/r4/accept-719da04/kept/e2e_browser-2026-10-28T151315995Z-28289/work/other/genesis.spec.hex", "8f71c221b7cb9a4d11e75214e579a5655b85e8248d5515f7a26faa5fb3564bba"),
    ("development/audit-2026-10-22/evidence/r4/accept-719da04/kept/e2e_integrated-2026-10-28T151536223Z-31457/work/net/genesis.spec.hex", "820fc8e547b98c3790a3b69a793311965adae9036815a348781d9df76f6ccdad"),
    ("development/audit-2026-10-22/evidence/r4/accept-719da04/kept/e2e_integrated-2026-10-28T152016586Z-530/work/net/genesis.spec.hex", "1a2c95ede20e545c8a783e010314c5e8c8f73464e298789c37e3ff6495df0478"),
];
fn root()->PathBuf{Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()}
struct Temp(PathBuf);
impl Temp{fn new(label:&str)->Self{static N:AtomicU64=AtomicU64::new(0);Self(std::env::temp_dir().join(format!("pocol-v3-{label}-{}-{}",std::process::id(),N.fetch_add(1,Ordering::Relaxed))))}}
impl Drop for Temp{fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
fn config()->serde_json::Value{
    let miner=devnet::miner_identity("v3-scaffold");
    serde_json::json!({"continuous":true,"earnin":true,"chainId":777931,"genesisTime":1_800_000_000u64,
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
#[test]
fn every_committed_spec1_genesis_keeps_its_original_bytes_and_hash(){
    assert_eq!(GENESIS.len(),10);
    for (name,sha) in GENESIS{
        let text=if let Some((file,key))=name.split_once('#'){
            let doc:serde_json::Value=serde_json::from_slice(&std::fs::read(root().join(file)).unwrap()).unwrap();
            assert_eq!(key,"files/genesis.spec.hex");doc["files"]["genesis.spec.hex"].as_str().unwrap().to_string()
        }else{std::fs::read_to_string(root().join(name)).unwrap()};
        let bytes=pocold::types::unhex(text.trim()).unwrap();
        assert_eq!(format!("{:x}",Sha256::digest(&bytes)),*sha,"{name}: immutable pre-scaffold bytes");
        let spec=gs::decode(&bytes).unwrap();assert_eq!(spec.spec_version,1);assert_eq!(spec.cp.len(),52);
        assert_eq!(gs::encode(&spec),bytes,"{name}");
        assert_eq!(pocold::genesis_spec::encode(&pocold::genesis_spec::decode(&bytes).unwrap()),bytes,"{name}: node codec");
        let (kind,node)=if spec.chain_id==777001{(ProfileKind::Network,NodeKind::Node)}else{(ProfileKind::Simnet,NodeKind::Simnet)};
        let id=ProfileIdentity::validate(&bytes,&gs::genesis_hash(&bytes),spec.chain_id,kind,node,&[(1,0),(2,15000)]).unwrap();
        for h in [0,1,14999,15000,15001,u64::MAX]{assert!(!id.v3_active(h),"{name} at {h}");}
        if name.starts_with("deploy/mainnet/h15000/"){
            assert_eq!(pocold::types::hex(&id.genesis_hash()),"0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca");
        }
    }
}
#[test]
fn sequential_future_forks_never_narrow_the_nonce_domain(){
    let forks=ForkSchedule::new_bounded(&[(1,0),(2,10),(3,20)],3).unwrap();
    assert!(!pocold::chain::full_nonce_rule(forks.active_version(9),0));
    for h in [10,19,20,21,u64::MAX]{assert!(pocold::chain::full_nonce_rule(forks.active_version(h),0));}
    assert_eq!(gs::SPEC_VERSION,1);assert_eq!(pocol_lp4_execution::profile::MAX_FORK_VERSION,2);
    assert_eq!(pocold::identity::CONSENSUS_RULES_VERSION,1);
}
#[test]
fn schema2_roundtrips_full_width_fields_and_respects_the_release_ceiling(){
    let cfg=devnet::DevnetConfig::new(777932,1_700_000_000,&["one"]);
    let net=devnet::build(&cfg).unwrap();let mut spec=gs::decode(&net.spec_bytes).unwrap();
    let old=ConsensusParams::from_values(&spec.cp).unwrap();assert_eq!(old.H_half,0);assert_eq!(old.F0_code,pocol_lp4_execution::fixed::U256::ZERO);
    spec.spec_version=2;spec.cp.extend(gs::CP_SCHEMA_V2[52..].iter().map(|f|pocol_lp4_execution::fixed::U256::from_u64(f.lo)));
    for name in ["S_tail","F0_acct","F0_slot","F0_code","baseFeeMin"]{let i=gs::cp_index_for(2,name).unwrap();spec.cp[i]=pocol_lp4_execution::fixed::U256::MAX;}
    let bytes=gs::encode(&spec);assert_eq!(gs::decode_bounded(&bytes,2).unwrap(),spec);
    assert_eq!(pocold::genesis_spec::encode(&pocold::genesis_spec::decode_bounded(&bytes,2).unwrap()),bytes);
    assert_eq!(gs::decode_bounded(&bytes,1).unwrap_err().code,"gsVersion");
    if pocol_lp4_execution::V3_PREVIEW{assert_eq!(gs::decode(&bytes).unwrap(),spec);}
    else{assert_eq!(gs::decode(&bytes).unwrap_err().code,"gsVersion");}
    let typed=ConsensusParams::from_values(&spec.cp).unwrap();assert_eq!(typed.F0_code,pocol_lp4_execution::fixed::U256::MAX);assert_eq!(typed.to_values(),spec.cp);
    for (name,bad) in [("gamma_tip_bp",10001),("theta_bp",10001),("rho_bp",10001),("R_react",256),("P_MAX",65536),("H_half",0),("T_PROB",0),("E_epoch",0)]{
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
    prepare(&input,&out.0).unwrap();let spec=read_spec(&out.0);assert_eq!(spec.spec_version,1);
    let cp=ConsensusParams::from_values(&spec.cp).unwrap();assert_eq!((cp.T_blk,cp.D_att,cp.dFbWait),(30,150,10));
    assert_eq!(cp.target_g,pocol_lp4_execution::gate::calibration_target((1<<18)/10,30).unwrap());
    assert_eq!(devnet::from_fixed(&cp.B_reg),U256::from(25_000_000_000_000_000_000u128));
    let local=pocold::testnet::local_for_network(&out.0).unwrap();assert_eq!((local.t_exec,local.delta_rel,local.gamma),(1,12,5));
    let info:serde_json::Value=serde_json::from_slice(&std::fs::read(out.0.join("devnet.json")).unwrap()).unwrap();assert_eq!(info["localTiming"]["deltaRel"],12);
    let admitted=pocold::load_spec_for(&out.0,&local,None).unwrap();assert_eq!(admitted.programs(),ProgramSet::EarnIn);assert!(admitted.earnin_rules());assert!(!admitted.v3_active(u64::MAX));
    let miner=devnet::miner_identity("v3-scaffold");assert_eq!(apply::miner_record(admitted.genesis().state(),miner.id).unwrap().bond,U256::from(25_000_000_000_000_000_000u128));
}
#[test]
fn v3_config_implies_the_new_schema_and_full_nonce_only_in_preview(){
    let out=Temp::new("preview-config");let mut input=config();input.as_object_mut().unwrap().remove("earnin");input.as_object_mut().unwrap().remove("continuous");input["v3"]=serde_json::json!(true);
    input["cp"]=serde_json::json!({"T_blk":30,"D_att":150,"dFbWait":10,"M_max":256,"P_MAX":64});
    let result=prepare(&input,&out.0);
    if pocol_lp4_execution::V3_PREVIEW{
        result.unwrap();let spec=read_spec(&out.0);assert_eq!(spec.spec_version,2);assert_eq!(spec.cp.len(),67);
        let cp=ConsensusParams::from_values(&spec.cp).unwrap();assert_eq!((cp.nonceMode,cp.H_END,cp.M_max,cp.P_MAX,cp.T_PROB,cp.E_epoch),(1,0,256,64,86400,2880));
        let local=pocold::testnet::local_for_network(&out.0).unwrap();let admitted=pocold::load_spec_for(&out.0,&local,None).unwrap();assert!(admitted.v3_active(0));assert!(admitted.v3_active(u64::MAX));
        // E0 adds the compiled V3 set. This scaffold honestly runs the existing earn-in artifact.
        assert_eq!(admitted.programs(),ProgramSet::EarnIn);
        for (field,value) in [("rho_bp",10001),("T_PROB",0),("E_epoch",0)]{
            let invalid=Temp::new(field);let mut config=input.clone();config["cp"][field]=serde_json::json!(value);
            assert!(prepare(&config,&invalid.0).unwrap_err().contains("gsRange"));assert!(!invalid.0.exists());
        }
    }else{assert!(result.unwrap_err().contains("requires a v3-preview build"));assert!(!out.0.exists());}
}
#[test]
fn preview_admission_rejects_every_non_simnet_context(){
    let cfg=devnet::DevnetConfig::new(777933,1_700_000_000,&["one"]);let net=devnet::build(&cfg).unwrap();
    let local=NodeLocal::single_host();let (admitted,timing)=net.admit(&local).unwrap();assert!(!admitted.profile().v3_active(0));
    for (kind,node) in [(ProfileKind::Synthetic,NodeKind::Simnet),(ProfileKind::Network,NodeKind::Node),(ProfileKind::Simnet,NodeKind::Node)]{
        let mut request=net.request(&local,Some(&timing));request.kind=kind;request.node=node;
        let result=pocol_lp4_execution::admission::admit(&request);
        if pocol_lp4_execution::V3_PREVIEW{assert_eq!(result.err().unwrap().rules,["v3-preview"]);}
        else if kind==ProfileKind::Simnet&&node==NodeKind::Node{assert!(result.is_err());}
    }
}
