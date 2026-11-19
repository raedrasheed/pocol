//! FN-4 (FRESH-NETWORK-DESIGN.md row C7): the committed fresh-network profiles, `deploy/mainnet/v3-profile.json` and
//! `v3-rehearsal-profile.json`, through the genesis builder (`pocold init-testnet`, testnet.rs) and the full ParamGate.
//! The CP is the design's §1a table (development/decisions/FRESH-NETWORK-CP.md); the chain ids are the FN-0 draws. The
//! founders are placeholders until the genesis ceremony: the builder refuses them unless the explicit test flag fills
//! them with the public test keys of `testnet::test_founder` and the local `testnet::TEST_CHAIN_ID` (never a launch
//! genesis), so every genesis built here is a local test genesis. Only `ceremony_filled` (no flag) keeps a profile's id.
use pocol_lp4_execution::{admission::{NodeLocal,Scope},apply::{self,ProgramSet},devnet,gate::FactClass,
    genesis_spec as gs,issuance,param_gate::registry_metrics,profile::ConsensusParams};
use pocold::testnet::{self,TestPlaceholders,TimingAssumptions};
use revm::primitives::U256;
use std::{path::{Path,PathBuf},sync::atomic::{AtomicU64,Ordering}};

const MAINNET:&str=include_str!("../../../deploy/mainnet/v3-profile.json");
const REHEARSAL:&str=include_str!("../../../deploy/mainnet/v3-rehearsal-profile.json");
const RECORD:&str=include_str!("../../../development/decisions/FRESH-NETWORK-CP.md");
const RETIRED:&str=include_str!("../../../deploy/mainnet/retired-networks.json");
const PCOL:u128=1_000_000_000_000_000_000;
/// The FN-0 draws (FRESH-NETWORK-CP.md "Chain ids").
const MAINNET_CHAIN_ID:u64=2_656_089_850;
const REHEARSAL_CHAIN_ID:u64=1_656_550_731;
/// The test flag's values here: a synthetic launch second and a small synthetic H_cal, so a block is found in a few
/// hundred hashes (the CLI flag takes the current second and (1<<18)/10).
const TEST:TestPlaceholders=TestPlaceholders{genesis_time:1_800_000_000,hash_rate:16};

struct Temp(PathBuf);
impl Temp{fn new(label:&str)->Self{static N:AtomicU64=AtomicU64::new(0);Self(std::env::temp_dir().join(format!("pocol-fn4-{label}-{}-{}",std::process::id(),N.fetch_add(1,Ordering::Relaxed))))}}
impl Drop for Temp{fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
fn json(text:&str)->serde_json::Value{serde_json::from_str(text).unwrap()}
fn spec_in(dir:&Path)->gs::GenesisSpec{gs::decode(&pocold::types::unhex(std::fs::read_to_string(dir.join("genesis.spec.hex")).unwrap().trim()).unwrap()).unwrap()}
fn cp_in(dir:&Path)->ConsensusParams{ConsensusParams::from_values(&spec_in(dir).cp).unwrap()}
fn info_in(dir:&Path)->serde_json::Value{json(&std::fs::read_to_string(dir.join("devnet.json")).unwrap())}
/// `profile` built with the test flag into a new directory.
fn built(label:&str,profile:&serde_json::Value)->Temp{
    let out=Temp::new(label);testnet::prepare_with(profile.to_string().as_bytes(),&out.0,Some(&TEST)).unwrap();out
}
/// The ceremony's own inputs, filled the way the genesis ceremony fills them (a launch second, the measured aggregate
/// hash rate, each founder's public id and reward address), with the public test identities: no placeholder remains,
/// so the builder needs no test flag and H_cal keeps its measured class.
fn ceremony_filled(profile:&serde_json::Value,hash_rate:u64)->serde_json::Value{
    let mut p=profile.clone();
    p["genesisTime"]=serde_json::json!(1_800_000_000u64);p["hashRate"]["hashesPerSecond"]=serde_json::json!(hash_rate);
    for (i,f) in p["founders"].as_array_mut().unwrap().iter_mut().enumerate(){
        let t=testnet::test_founder(i);f["id"]=serde_json::json!(t.id);f["reward"]=serde_json::json!(t.reward);
    }
    p
}
/// The genesis in `dir` booted by the launcher (`pocold::load_spec_for`, the path `pocold run` takes): the full ParamGate
/// (R1-R26) at `local`, Simnet profile, Simnet node, ExperimentalDevnet scope (FRESH-NETWORK-DESIGN §0 "Launch
/// admission"), with the calibration class the launcher reads from devnet.json. Its admission is `spec.admission()`.
fn boot(dir:&Path,local:&NodeLocal)->Result<pocold::chain::Spec,String>{pocold::load_spec_for(dir,local,None)}

#[test]
fn the_two_chain_ids_differ_are_in_range_and_avoid_the_excluded_set(){
    let (m,r)=(json(MAINNET),json(REHEARSAL));
    assert_eq!(m["chainId"],MAINNET_CHAIN_ID);assert_eq!(r["chainId"],REHEARSAL_CHAIN_ID);
    assert_ne!(MAINNET_CHAIN_ID,REHEARSAL_CHAIN_ID,"rehearsal transactions must not replay on mainnet (EIP-155 binds only the chain id)");
    // The retired network's id is read from the retired list (no tracked file outside development/ pins it).
    let retired:Vec<u64>=json(RETIRED)["retired"].as_array().unwrap().iter().map(|e|e["chainId"].as_u64().unwrap()).collect();
    assert!(!retired.is_empty());
    for id in [MAINNET_CHAIN_ID,REHEARSAL_CHAIN_ID]{
        assert!((1u64<<24..1u64<<32).contains(&id),"{id}: drawn in [2^24, 2^32)");
        assert!(![777001,261597237,1234567891].contains(&id)&&!retired.contains(&id),"{id}: R22's id, the earlier public testnet, the extension placeholder and the retired network are excluded");
        assert!(!(777900..=777999).contains(&id),"{id}: 7779xx are the local devnet ids");assert_ne!(id,testnet::TEST_CHAIN_ID);
        assert!(RECORD.contains(&id.to_string()),"{id}: recorded in FN-0's record");
    }
}

#[test]
fn the_profiles_state_the_final_cp_and_differ_only_in_chain_id_and_genesis_time(){
    let (m,r)=(json(MAINNET),json(REHEARSAL));
    let strip=|v:&serde_json::Value|{let mut v=v.clone();let o=v.as_object_mut().unwrap();o.remove("chainId");o.remove("genesisTime");v};
    assert_eq!(strip(&m),strip(&r),"the rehearsal runs the mainnet profile with its own chain id and launch second");
    for p in [&m,&r]{assert!(p["genesisTime"].as_str().unwrap().starts_with(testnet::PLACEHOLDER));}
    // Every CP schema v2 field but the two the ceremony supplies (g_ts, target_g), at the §1a value.
    let cp=m["cp"].as_object().unwrap();
    let schema=gs::cp_schema(2).unwrap();assert_eq!(schema.len(),59);
    let named:Vec<&str>=schema.iter().map(|f|f.name).filter(|n|!["g_ts","target_g"].contains(n)).collect();
    assert_eq!(cp.len(),named.len());for n in &named{assert!(cp.contains_key(*n),"{n}");}
    let want:&[(&str,u128)]=&[("T_blk",10),("tau",600),("nonceMode",1),("c",2),("D_att",90),("dFbWait",8),("m",32),("kappa",2),("S_max",256),
        ("D",4),("W_w",8),("alpha_bp",1000),("gamma_bp",500),("subsidy",4*PCOL),("B_reg",20*PCOL),("R_max",4),("M_max",10_000),("M_min",1),
        ("REG_RECORDS_MAX",24_576),("SWEEP_MAX",16),("SWEEP_SCAN",64),("REG_OPS",16),("W_act",32),("Theta_inact",360),("E_win",360),("U",380),
        ("E_max",4),("EVID_BYTES_MAX",8192),("GAS_LIMIT",8_000_000),("baseFee0",1_000_000_000),("TX_MAX",131_072),("BODY_MAX",196_608),
        ("B_code_max",32_768),("CODE_CAP",1<<30),("TXBYTES_CAP",1<<24),("TX_CAP",20_000),("SLOT_CAP",4_000_000),("ACCT_CAP",1_000_000),
        ("SYS_SLOT_MAX",147_456),("SYS_KEYS_BLOCK_MAX",4096),("SYS_KEYS_USER_MAX",256),("SYS_ACCT_BLOCK_MAX",512),("G_SYS",96_000_000),
        ("H_END",0),("T_END",0),("H_CLOSE",0),("T_CLOSE",0),("Z_close",256),("RET",128),("P2",2),("H_half",12_623_040),("S_tail",PCOL/8),
        ("gamma_tip_bp",5000),("baseFeeMin",10_000_000),("R_react",4),("C_react",380),("rho_bp",2500)];
    assert_eq!(want.len(),named.len());
    let value=|v:&serde_json::Value|->u128{v.as_u64().map(u128::from).unwrap_or_else(||v.as_str().unwrap().parse().unwrap())};
    for (n,w) in want{assert_eq!(value(&cp[*n]),*w,"CP {n}");}
    assert_eq!(m["localTiming"],serde_json::json!({"delta":1,"phi":1,"tExec":1}));
    assert_eq!(m["hashRate"]["class"],"measurement");assert_eq!(m["registrationPerBlock"],4);
    let f=m["founders"].as_array().unwrap();assert_eq!(f.len(),testnet::FOUNDERS);
    for f in f{
        assert!(f["id"].as_str().unwrap().starts_with(testnet::PLACEHOLDER)&&f["reward"].as_str().unwrap().starts_with(testnet::PLACEHOLDER));
        assert_eq!(value(&f["allocationWei"])+20*PCOL,testnet::FOUNDER_TOTAL_WEI,"999,980 PCOL + the 20 PCOL HELD bond");
    }
}

#[test]
fn a_repeated_key_at_any_depth_is_refused_with_or_without_the_test_flag(){
    // JSON with a repeated key must be refused, not read as its last value (independent_keys.py checks the same for a
    // plain config): at the top, in cp and in a founder, for a ceremony-filled profile and for one the flag fills.
    for (label,p) in [("mainnet",MAINNET),("rehearsal",REHEARSAL)]{
        let filled=ceremony_filled(&json(p),1_000_000).to_string();let raw=json(p).to_string();
        for (at,from,to) in [("top","\"chainId\":","\"chainId\":1,\"chainId\":"),("cp","\"T_blk\":","\"T_blk\":10,\"T_blk\":"),("founder","\"reward\":","\"reward\":\"1\",\"reward\":")]{
            for (flag,text) in [(None,&filled),(Some(&TEST),&filled),(Some(&TEST),&raw)]{
                let dup=text.replacen(from,to,1);assert_ne!(&dup,text,"{label} {at}");
                let out=Temp::new("dup");let err=testnet::prepare_with(dup.as_bytes(),&out.0,flag).unwrap_err();
                assert!(err.contains("schema"),"{label} {at}: {err}");assert!(!out.0.exists(),"{label} {at}: nothing is written");
            }
        }
        // The same texts without the repeat build (the refusal is the repeat's).
        let out=Temp::new("nodup");testnet::prepare(filled.as_bytes(),&out.0).unwrap();
    }
}
#[test]
fn the_builder_refuses_ceremony_placeholders_without_the_test_flag(){
    for (label,p) in [("mainnet",MAINNET),("rehearsal",REHEARSAL)]{
        let out=Temp::new(label);let err=testnet::prepare(p.as_bytes(),&out.0).unwrap_err();
        for at in ["/genesisTime","/hashRate/hashesPerSecond","/founders/0/id","/founders/3/reward"]{assert!(err.contains(at),"{err}");}
        assert!(err.contains("--test-placeholders"),"{err}");assert!(!out.0.exists(),"nothing is written");
        // One placeholder left is enough to refuse, wherever it is.
        let mut one=ceremony_filled(&json(p),1_000_000);one["founders"][2]["reward"]=serde_json::json!("PLACEHOLDER: founder 3");
        let err=testnet::prepare(one.to_string().as_bytes(),&out.0).unwrap_err();assert!(err.contains("/founders/2/reward"),"{err}");
        // The flag fills only the ceremony inputs: a placeholder elsewhere is still refused.
        let mut other=json(p);other["cp"]["D_att"]=serde_json::json!("PLACEHOLDER: D_att");
        let err=testnet::prepare_with(other.to_string().as_bytes(),&out.0,Some(&TEST)).unwrap_err();assert!(err.contains("/cp/D_att"),"{err}");
        assert!(!out.0.exists());
    }
    // With the flag: a test genesis, marked as one, with the public test founders, a synthetic calibration and the local
    // test chain id (design §1b: never the profile's own id, which is recorded as profileChainId).
    for (label,p,id) in [("flagged-rehearsal",REHEARSAL,REHEARSAL_CHAIN_ID),("flagged-mainnet",MAINNET,MAINNET_CHAIN_ID)]{
        let out=built(label,&json(p));let info=info_in(&out.0);
        assert_eq!((info["chainId"].as_u64(),info["profileChainId"].as_u64()),(Some(testnet::TEST_CHAIN_ID),Some(id)),"{label}");
        assert_eq!(spec_in(&out.0).chain_id,testnet::TEST_CHAIN_ID,"{label}: the genesis itself carries the test id");
    }
    let out=built("flagged",&json(MAINNET));let info=info_in(&out.0);
    assert_eq!(info["testPlaceholders"],true);assert!(info["note"].as_str().unwrap().starts_with("TEST GENESIS"));
    assert_eq!(info["calibration"],serde_json::json!({"hashesPerSecond":TEST.hash_rate,"class":"synthetic-fixture"}));
    assert_eq!(info["genesisTime"],TEST.genesis_time);
    for i in 0..testnet::FOUNDERS{let t=testnet::test_founder(i);assert_eq!(info["founders"][i]["id"],serde_json::json!(t.id));assert_eq!(info["founders"][i]["reward"],serde_json::json!(t.reward));}
    // The CLI flag, through the binary.
    let d=Temp::new("cli");std::fs::create_dir_all(&d.0).unwrap();let config=d.0.join("v3-profile.json");std::fs::write(&config,MAINNET).unwrap();
    let run=|extra:&[&str],out:&Path|std::process::Command::new(env!("CARGO_BIN_EXE_pocold")).args(["init-testnet","--config",config.to_str().unwrap(),"--out",out.to_str().unwrap()]).args(extra).output().unwrap();
    let o=run(&[],&d.0.join("refused"));assert!(!o.status.success());assert!(String::from_utf8_lossy(&o.stderr).contains("placeholders"));assert!(!d.0.join("refused").exists());
    let o=run(&["--test-placeholders"],&d.0.join("test"));assert!(o.status.success(),"{}",String::from_utf8_lossy(&o.stderr));
    let summary:serde_json::Value=serde_json::from_slice(&o.stdout).unwrap();assert_eq!(summary["testPlaceholders"],true);
    assert_eq!((summary["chainId"].as_u64(),summary["profileChainId"].as_u64()),(Some(testnet::TEST_CHAIN_ID),Some(MAINNET_CHAIN_ID)));
    assert_eq!(info_in(&d.0.join("test"))["calibration"]["hashesPerSecond"],(1<<18)/10);
    let o=run(&["--test-placeholders","--test-placeholders"],&d.0.join("twice"));assert!(!o.status.success());
}

#[test]
fn a_ceremony_filled_profile_builds_without_the_flag_with_a_measured_calibration(){
    let out=Temp::new("ceremony");let p=ceremony_filled(&json(MAINNET),1_000_000);
    testnet::prepare(p.to_string().as_bytes(),&out.0).unwrap();
    let info=info_in(&out.0);
    assert!(info.get("testPlaceholders").is_none()&&info.get("profileChainId").is_none());
    assert_eq!(info["calibration"],serde_json::json!({"hashesPerSecond":1_000_000,"class":"measurement"}));
    assert_eq!((info["profile"].as_str(),info["record"].as_str()),(Some("pocol-v3-fresh-network"),Some("development/decisions/FRESH-NETWORK-CP.md")));
    // The ceremony path keeps the profile's own chain id, for both profiles.
    assert_eq!((info["chainId"].as_u64(),spec_in(&out.0).chain_id),(Some(MAINNET_CHAIN_ID),MAINNET_CHAIN_ID));
    let o3=Temp::new("ceremony-rehearsal");testnet::prepare(ceremony_filled(&json(REHEARSAL),1_000_000).to_string().as_bytes(),&o3.0).unwrap();
    assert_eq!(spec_in(&o3.0).chain_id,REHEARSAL_CHAIN_ID);
    // The launcher (pocold/src/lib.rs) boots it with the measured class devnet.json records.
    let spec=boot(&out.0,&testnet::local_for_network(&out.0).unwrap()).unwrap();let a=spec.admission();
    assert_eq!(spec.chain_id(),MAINNET_CHAIN_ID);
    assert_eq!(a.report().calibration_class,Some(FactClass::Measurement),"R25 with a measured class (design §1a row 1)");
    assert!(!a.report().conditions.iter().any(|c|c.id=="calibration-synthetic"));
    assert_eq!(a.profile().cp().target_g,pocol_lp4_execution::gate::calibration_target(1_000_000,10).unwrap(),"target_g = calibration_target(H_measured, 10)");
    // The converse: the launcher boots a --test-placeholders genesis with its synthetic class.
    let t=built("ceremony-converse",&json(MAINNET));let spec=boot(&t.0,&testnet::local_for_network(&t.0).unwrap()).unwrap();
    assert_eq!(spec.admission().report().calibration_class,Some(FactClass::SyntheticFixture));
    assert!(spec.admission().report().conditions.iter().any(|c|c.id=="calibration-synthetic"));
    // A measured class on a profile is required: a synthetic one is refused.
    let mut synthetic=p.clone();synthetic["hashRate"]["class"]=serde_json::json!("synthetic-fixture");
    let o2=Temp::new("ceremony-synthetic");assert!(testnet::prepare(synthetic.to_string().as_bytes(),&o2.0).unwrap_err().contains("measurement"));
}

/// V3-28 H2 (1): the admission set_network_identity.py pins for a switched-to network, readiness.rs
/// `ExpectedAdmission::FRESH_NETWORK`, is the decided launch admission (FRESH-NETWORK-CP.md §1 "Launch admission"):
/// every field of it is what the launcher's boot gives a ceremony genesis of either profile. It is not the launch
/// admission, so a pin of it keeps `admission-not-booted` in the launcher.
#[test]
fn the_pinned_fresh_network_admission_is_the_one_a_ceremony_genesis_boots_with(){
    use pocol_lp4_execution::profile::{NodeKind,ProfileKind};use pocold::readiness::ExpectedAdmission;
    let want=ExpectedAdmission::FRESH_NETWORK;
    assert_eq!((want.scope,want.profile_kind,want.node_kind),(Scope::ExperimentalDevnet,ProfileKind::Simnet,NodeKind::Simnet),"Q3 = a");
    assert_eq!(want.differing(&ExpectedAdmission::LAUNCH),["scope","conditions","waivers","profile kind","node kind"]);
    for (label,profile,chain) in [("fresh-mainnet",MAINNET,MAINNET_CHAIN_ID),("fresh-rehearsal",REHEARSAL,REHEARSAL_CHAIN_ID)]{
        let out=Temp::new(label);testnet::prepare(ceremony_filled(&json(profile),1_000_000).to_string().as_bytes(),&out.0).unwrap();
        let spec=boot(&out.0,&testnet::local_for_network(&out.0).unwrap()).unwrap();let a=spec.admission();
        assert_eq!(spec.chain_id(),chain);
        let got=ExpectedAdmission{scope:a.scope(),conditions:&[],waivers:&[],profile_kind:a.profile().kind(),node_kind:a.profile().node_kind()};
        assert_eq!(got,ExpectedAdmission{conditions:&[],waivers:&[],..want},"{label}");
        assert_eq!(a.report().conditions.iter().map(|c|c.id).collect::<Vec<_>>(),want.conditions,"{label}");
        assert_eq!(a.report().waivers.iter().map(|w|w.rule).collect::<Vec<_>>(),want.waivers,"{label}");
        // Its facts are the booted node's: readiness reports the same admission blockers from either.
        assert_eq!(want.facts(chain),pocold::readiness::AdmissionFacts::of(a),"{label}");
    }
}

#[test]
fn the_founder_allocation_check(){
    let base=ceremony_filled(&json(MAINNET),1_000_000);
    // Positive: the genesis holds each founder as a live HELD record with the 20 PCOL bond minted into Escrow, its reward
    // account durable (nonce >= 1, PAY-02) with 999,980 PCOL, and exactly 4,000,000 PCOL in all.
    let out=Temp::new("alloc");testnet::prepare(base.to_string().as_bytes(),&out.0).unwrap();
    let spec=boot(&out.0,&testnet::local_for_network(&out.0).unwrap()).unwrap();let g=spec.genesis();let state=g.state();
    for i in 0..testnet::FOUNDERS{
        let t=testnet::test_founder(i);let r=apply::miner_record(state,t.id).unwrap();
        assert_eq!((r.status,r.tomb,r.bond_state,r.bond,r.reward),(1,false,1,U256::from(20*PCOL),t.reward),"founder {i}: HELD");
        let acct=&state.accounts[&t.reward].info;assert!(acct.nonce>=1,"founder {i}: PAY-02");assert_eq!(acct.balance,U256::from(999_980*PCOL));
    }
    assert_eq!(state.accounts[&pocol_lp4_execution::pay::escrow_address()].info.balance,U256::from(80*PCOL),"the four bonds in Escrow");
    assert_eq!(g.initial_supply(),U256::from(4_000_000*PCOL));
    let info=info_in(&out.0);assert_eq!(info["supply"]["initialWei"],(4_000_000*PCOL).to_string());
    for f in info["founders"].as_array().unwrap(){assert_eq!(f["bondWei"],(20*PCOL).to_string());}
    // Negative: allocation + B_reg must be exactly 10^24 wei per founder, for four distinct founders.
    let refused=|p:serde_json::Value,why:&str|{let o=Temp::new("alloc-bad");let e=testnet::prepare(p.to_string().as_bytes(),&o.0).unwrap_err();assert!(e.contains(why),"{why}: {e}");assert!(!o.0.exists());};
    for (wei,why) in [("1000000000000000000000000","is not 1000000000000000000000000 wei"),("999979999999999999999999","is not"),("999980000000000000000001","is not"),
        ("","decimal wei"),("0x01","decimal wei"),("-1","decimal wei")]{
        let mut p=base.clone();p["founders"][1]["allocationWei"]=serde_json::json!(wei);refused(p,why);
    }
    let mut p=base.clone();p["cp"]["B_reg"]=serde_json::json!("10000000000000000000");refused(p,"is not 1000000000000000000000000 wei");
    let mut p=base.clone();p["founders"].as_array_mut().unwrap().pop();refused(p,"4 founders are required");
    let mut p=base.clone();let extra=p["founders"][0].clone();p["founders"].as_array_mut().unwrap().push(extra);refused(p,"4 founders are required");
    let mut p=base.clone();p["founders"][3]["id"]=p["founders"][0]["id"].clone();refused(p,"distinct");
    let mut p=base.clone();p["founders"][3]["reward"]=p["founders"][0]["reward"].clone();refused(p,"distinct");
    let mut p=base.clone();p["accounts"]=serde_json::json!([{"address":devnet::account_key("fn4-extra").address(),"wei":"1"}]);refused(p,"only allocation");
    let mut p=base.clone();p["registrationPerBlock"]=serde_json::json!(3);refused(p,"registrationPerBlock 3");
    let mut p=base.clone();p["cp"].as_object_mut().unwrap().remove("Z_close");refused(p,"missing Z_close");
}

#[test]
fn paramgate_r1_r26_admits_the_final_and_rehearsal_cp_at_both_declared_locals(){
    for (label,profile) in [("mainnet",MAINNET),("rehearsal",REHEARSAL)]{
        let out=built(label,&json(profile));let c=cp_in(&out.0);
        assert_eq!(spec_in(&out.0).spec_version,2);assert_eq!(spec_in(&out.0).cp.len(),59);
        // A test genesis runs on the local test id; the profile's own id is checked on the ceremony path (above).
        assert_eq!(spec_in(&out.0).chain_id,testnet::TEST_CHAIN_ID);assert_eq!(info_in(&out.0)["profileChainId"],json(profile)["chainId"]);
        // R20: Z_close >= ceil(REG_RECORDS_MAX / RET) + P2 + D + 1 = 192 + 2 + 4 + 1 = 199 <= 256; R16: 21,092 <= 24,576.
        let reg=registry_metrics(c.U,c.E_win,c.D,c.M_max,c.R_max,c.REG_RECORDS_MAX,c.RET,c.P2).unwrap();
        assert_eq!(reg.close_blocks_required,199);assert!(reg.close_blocks_required<=u128::from(c.Z_close));assert_eq!(c.Z_close,256);
        assert_eq!(reg.records_required,21_092);assert!(reg.records_required<=u128::from(c.REG_RECORDS_MAX));
        // The default (1,1,1), Δ_rel 7, and the measured fallback (2,1,1), Δ_rel 8 (design §1c): the same CP admits both.
        for ((delta,phi,t_exec),(rel,gamma)) in [((1,1,1),(7,5)),((2,1,1),(8,5))]{
            let local=testnet::local_for_v3(&c,TimingAssumptions{delta,phi,t_exec}).unwrap();
            assert_eq!((local.delta_rel,local.gamma),(rel,gamma),"{label} ({delta},{phi},{t_exec})");
            let m=pocold::timing::metrics(local.delta.into(),local.capital_phi().into(),local.t_exec.into(),local.delta_rel.into(),c.dFbWait.into());
            let (d,t,big_phi)=(u128::from(local.delta),u128::from(local.t_exec),u128::from(local.capital_phi()));
            assert!(u128::from(local.delta_rel)>=m.sigma+d+2*t,"R2");assert!(u128::from(c.dFbWait)>=big_phi+d+2*t,"R3");
            assert!(3*c.dFbWait<=c.D_att,"R5");
            assert!(u128::from(c.D_att)+u128::from(local.gamma)>=2*(m.x_w+u128::from(c.c)*u128::from(c.T_blk))+big_phi,"R7");
            let spec=boot(&out.0,&local).unwrap_or_else(|e|panic!("{label} ({delta},{phi},{t_exec}): ParamGate refused: {e}"));let a=spec.admission();
            assert_eq!(a.scope(),Scope::ExperimentalDevnet);assert!(a.report().r21_exception,"R21 through the simnet retentionStress exception (design §1c)");
            println!("FN4-ADMIT {}",serde_json::json!({"profile":label,"local":[delta,phi,t_exec],"deltaRel":rel,"gamma":gamma,
                "conditions":a.report().conditions.iter().map(|c|c.id).collect::<Vec<_>>(),"historicalBytes":a.report().historical_bytes}));
        }
        // (2,2,2) fails R3 at dFbWait 8 (2·2 + 2 + 2·2 = 10 > 8): the builder refuses it before any output.
        let mut wan=json(profile);wan["localTiming"]=serde_json::json!({"delta":2,"phi":2,"tExec":2});
        let o=Temp::new("wan");let e=testnet::prepare_with(wan.to_string().as_bytes(),&o.0,Some(&TEST)).unwrap_err();assert!(e.contains("R3"),"{e}");assert!(!o.0.exists());
    }
}

#[test]
fn issuance_at_the_era_boundaries_and_the_supply_bound_at_the_final_cp(){
    let out=built("issuance",&json(MAINNET));let c=cp_in(&out.0);
    let (s0,h)=(devnet::from_fixed(&c.subsidy),c.H_half);
    assert_eq!((s0,h,devnet::from_fixed(&c.S_tail)),(U256::from(4*PCOL),12_623_040,U256::from(PCOL/8)));
    // The builder check: S_tail = S0/32, so the S0 term meets the tail exactly at era 5.
    testnet::tail_is_s0_over_32(s0,devnet::from_fixed(&c.S_tail)).unwrap();
    assert!(testnet::tail_is_s0_over_32(s0,U256::from(PCOL/8+1)).is_err());
    // The profile builder refuses any other tail.
    let mut off=json(MAINNET);off["cp"]["S_tail"]=serde_json::json!((PCOL/8+1).to_string());
    let o=Temp::new("tail");let e=testnet::prepare_with(off.to_string().as_bytes(),&o.0,Some(&TEST)).unwrap_err();assert!(e.contains("is not S0/32"),"{e}");assert!(!o.0.exists());
    let at=|n:u64|issuance::for_cp(&c,n);
    for (n,want) in [(0,0),(1,4*PCOL),(h-1,4*PCOL),(h,2*PCOL),(2*h,PCOL),(5*h-1,PCOL/4),(5*h,PCOL/8),(6*h,PCOL/8),(255*h,PCOL/8),(u64::MAX,PCOL/8)]{
        assert_eq!(at(n),U256::from(want),"issuance({n})");
    }
    // Year 1 (8,640 blocks a day for 365.25 days, H_half/4) issues 12,623,040 PCOL.
    assert_eq!(issuance::cumulative(h/4,s0,h,devnet::from_fixed(&c.S_tail)),U256::from(12_623_040*PCOL));
    // future_bound = 2·S0·H_half + S_tail·2^64; with the 4,000,000 PCOL genesis about 2.3058·10^36 wei, 121 bits < 127.
    let future=issuance::future_bound(s0,h,devnet::from_fixed(&c.S_tail)).unwrap();
    assert_eq!(future.to_string(),"2305843009314678272000000000000000000");
    let bound=testnet::supply_bound(U256::from(4_000_000*PCOL),&c).unwrap();
    assert_eq!(bound.to_string(),"2305843009318678272000000000000000000");assert_eq!(bound.bit_len(),121);
    let info=info_in(&out.0);assert_eq!(info["supply"]["boundWei"],bound.to_string());assert_eq!(info["supply"]["boundBits"],121);
    // Every era's S0 term sums below 2·S0·H_half: the closed form at 5·H_half is within the bound's halving term.
    assert!(issuance::cumulative(5*h,s0,h,devnet::from_fixed(&c.S_tail))<s0*U256::from(h)*U256::from(2));
}

/// A devnet booted from the final profile, with the four public test founders, mines real blocks on the v3 rules
/// (specVersion 2, the v3 program set): each block won and shared in a founder's own nonce range, the escrow invariant
/// after every block, the founders' records kept, and the supply growing by exactly issuance(h) per block.
#[test]
fn a_devnet_booted_from_the_final_profile_mines_v3_blocks(){
    use pocold::{chain::Chain,hashes,types::*};
    use revm::primitives::B256;
    let out=built("devnet",&json(MAINNET));
    let local=testnet::local_for_network(&out.0).unwrap();assert_eq!((local.delta,local.phi,local.t_exec,local.delta_rel),(1,1,1,7));
    let spec=pocold::load_spec_for(&out.0,&local,None).unwrap();
    assert_eq!(spec.programs(),ProgramSet::V3);assert!(spec.v3_active(0));
    assert_eq!(spec.profile().spec().spec_version,2);assert_eq!(spec.chain_id(),testnet::TEST_CHAIN_ID);
    let mut chain=Chain::new(spec);
    let keys:Vec<_>=(0..testnet::FOUNDERS).map(testnet::test_founder).collect();
    let key_of=|a:revm::primitives::Address|keys.iter().find(|k|k.id==a).map(|k|&k.key).unwrap();
    let supply=|chain:&Chain|chain.head().state.accounts.values().fold(U256::ZERO,|s,a|s+a.info.balance);
    assert_eq!(supply(&chain),U256::from(4_000_000*PCOL));
    let blocks=8u64;
    for n in 1..=blocks{
        let winner=keys[(n as usize)%keys.len()].id;
        let parent=chain.full_entry(&chain.head).unwrap();
        let context=chain.child_ctx(&parent,0).unwrap();
        let range=context.assignment.ranges().iter().find(|r|r.id()==raw_addr(winner)).expect("every founder has a range");
        let body=Body{txs:vec![],evidence:vec![]};
        let mut mined=false;
        'ts: for timestamp in parent.ts+u64::from(chain.spec.cp().T_blk)..=context.window.upper(){
            let (mut header,_,_)=chain.build_signed(&parent,key_of(context.proposer),timestamp,body.clone(),vec![]).unwrap();
            let tid=template_id(&header.ut_raw);
            let (mut shares,mut nonce)=(Vec::new(),range.start());
            while nonce<range.end_exclusive().min(context.assignment.nonce_max()){
                let hash=pow_hash(&tid,nonce as u64);
                if le_target(&hash,&context.target){
                    header.nonce=nonce as u64;header.shares=shares;reseal(&mut header);
                    header.winner_sig=key_of(winner).sign_hash(&B256::from(hashes::win_msg(&tid,nonce as u64,&share_root(&header))));
                    reseal(&mut header);
                    let (entry,_)=chain.validate(&Block{header,body},None,0).unwrap();
                    assert!(chain.insert(entry));mined=true;break 'ts;
                }
                if shares.len()<usize::from(chain.spec.cp().S_max)&&le_target(&hash,&context.t_share){shares.push(nonce as u64);}
                nonce+=1;
            }
        }
        assert!(mined,"block {n}");
        let head=chain.head();assert_eq!(head.height,n);
        apply::check_escrow_invariant(&head.state,chain.spec.profile(),head.height,false).unwrap();
        assert_eq!(supply(&chain),U256::from(4_000_000*PCOL)+issuance::cumulative(n,U256::from(4*PCOL),12_623_040,U256::from(PCOL/8)),"block {n}: 4 PCOL per block, no other mint or burn");
    }
    // Settlement ran (blocks beyond D = 4): the founders are still live HELD members with their bonds.
    for k in &keys{let r=apply::miner_record(&chain.head().state,k.id).unwrap();assert_eq!((r.status,r.bond_state,r.bond),(1,1,U256::from(20*PCOL)));}
}
