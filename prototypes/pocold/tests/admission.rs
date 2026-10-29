//! A02 boot path: `init-devnet` and `load_spec` (used by `run` and `status`) run the full
//! ParamGate on the real files and refuse every violation with its rules (-32018). Timing
//! facts are measured on the booting machine.
use pocol_lp4_execution::devnet::{self,DevnetConfig};
use std::path::{Path,PathBuf};

fn tmp(tag:&str)->PathBuf{let d=std::env::temp_dir().join(format!("pocold-adm-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&d);d}
/// Writes a candidate's published files exactly as init-devnet does, with or without
/// its calibration and timing records.
fn publish(c:&DevnetConfig,dir:&Path,timing:bool){
    let n=devnet::build(c).unwrap();std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("genesis.spec.hex"),pocold::types::hex(&n.spec_bytes)).unwrap();
    std::fs::write(dir.join("alloc.json"),devnet::alloc_to_json(&n.alloc).to_string()).unwrap();
    let mut info=serde_json::json!({"calibration":{"hashesPerSecond":n.hash_rate}});
    // As init-devnet: a profile whose blocks cannot be applied has no timing record.
    if timing{if let Ok(t)=pocol_lp4_execution::admission::measure(&n.spec_bytes,&n.alloc,1){info["timing"]=t.to_json();}}
    std::fs::write(dir.join("devnet.json"),info.to_string()).unwrap();
}
fn load(dir:&Path)->Result<(),String>{pocold::load_spec(dir,&pocold::single_host_local()).map(|_|())}
fn base()->DevnetConfig{DevnetConfig::new(777930,1_700_000_000,&["a","b","c"])}

#[test] fn load_refuses_the_audited_profiles_and_admits_the_valid_one(){
    let d=tmp("load");
    for (name,edit,rule) in [("s_max",(|c:&mut DevnetConfig|c.s_max=1) as fn(&mut DevnetConfig),"[R9]"),
        ("h_close",|c|c.h_close=c.h_end,"[R13]"),
        // Its genesis already exceeds SYS_SLOT_MAX, so no block applies and no timing exists.
        ("sys_slot_max",|c|c.sys_slot_max=1,"[R14, R18, R19]")]{
        let mut c=base();edit(&mut c);let dir=d.join(name);publish(&c,&dir,true);
        let e=load(&dir).expect_err(name);assert!(e.starts_with(&format!("-32018 ParamGate rejected {rule}")),"{name}: {e}");
    }
    let dir=d.join("ok");publish(&base(),&dir,true);load(&dir).unwrap();
    // R14/R19 use a measurement taken at load on this machine (review finding): a devnet.json
    // without a timing record still boots, and a forged record changes nothing.
    let dir=d.join("untimed");publish(&base(),&dir,false);load(&dir).unwrap();
    let dir=d.join("forged");publish(&base(),&dir,true);
    let mut info:serde_json::Value=serde_json::from_str(&std::fs::read_to_string(dir.join("devnet.json")).unwrap()).unwrap();
    info["timing"]["phaseMs"]=serde_json::json!([3_600_000,3_600_000,3_600_000]);std::fs::write(dir.join("devnet.json"),info.to_string()).unwrap();
    load(&dir).unwrap();
    let _=std::fs::remove_dir_all(&d);
}
#[test] fn init_devnet_refuses_gate_invalid_timing_and_writes_nothing(){
    let d=tmp("init");let out=d.join("net");
    let o=std::process::Command::new(env!("CARGO_BIN_EXE_pocold")).args(["init-devnet","--out",out.to_str().unwrap(),"--miners","a,b,c","--genesis-time","1700000000",
        "--t-blk","1","--d-att","4","--fb-wait","1","--work-bits","15"]).output().unwrap();
    // The old LP5-LP10 e2e flags: Δ_fb_wait=1 < Φ+Δ+2t_exec=3 (R3); 4+5 < 2(7+2·1) (R7).
    let err=String::from_utf8_lossy(&o.stderr);
    assert!(!o.status.success()&&err.contains("-32018 ParamGate rejected [R3, R7]"),"{err}");
    assert!(!out.exists(),"nothing is written for a rejected profile");
    let o=std::process::Command::new(env!("CARGO_BIN_EXE_pocold")).args(["init-devnet","--out",out.to_str().unwrap(),"--miners","a,b,c","--genesis-time","1700000000"]).output().unwrap();
    assert!(o.status.success(),"{}",String::from_utf8_lossy(&o.stderr));load(&out).unwrap();
    // A node whose local timing breaks the gate refuses to boot the same files.
    let e=pocold::load_spec(&out,&pocol_lp4_execution::admission::NodeLocal{delta_rel:4,..pocold::single_host_local()}).map(|_|()).unwrap_err();
    assert!(e.starts_with("-32018 ParamGate rejected [R2]"),"{e}");
    let _=std::fs::remove_dir_all(&d);
}
/// D-A02-SCOPE: pocold boots only an experimental-devnet admission. Its R26 facts name only the
/// RpcGuard limits it enforces; the unimplemented mechanisms are absent (0 in the gate), waived
/// and reported, never nominal. The same files under the specification scope are refused.
#[test] fn pocold_admission_is_an_experimental_devnet_with_absent_rpc_mechanisms(){
    use pocol_lp4_execution::admission::{self,Request,Scope};
    let d=tmp("scope");let dir=d.join("ok");publish(&base(),&dir,true);
    let spec=pocold::load_spec(&dir,&pocold::single_host_local()).unwrap();let a=spec.admission();
    assert_eq!(a.scope(),Scope::ExperimentalDevnet);assert!(!a.specification_compliant());
    // RpcGuard (rpcguard track, round 2 fixes, and SEND_SOCK_BUF at integration): 25 enforced/equivalent
    // fields; 4 absent (scratch_heavy, scratch_light, mem_total, cancel_max_ms).
    let f=&a.local().rpc;assert!(!f.is_fixture());
    let mut absent=f.absent().to_vec();absent.sort();assert_eq!(absent,["cancel_max_ms","mem_total","scratch_heavy","scratch_light"]);
    for x in pocold::RPC_ENFORCED{assert!(!f.absent().contains(&x),"{x} is enforced")}
    let v=f.values();
    assert_eq!((v.scratch_heavy,v.scratch_light,v.mem_total,v.cancel_max_ms),(0,0,0,0),"absent fields are not nominal");
    assert_eq!(v.send_sock_buf,pocold::rpc::SEND_SOCK_BUF as u64,"SO_SNDBUF is set on every RPC connection");
    assert_eq!((v.heavy_resp_mem,v.rate,v.bucket_cap,v.min_send_rate,v.timer_eps_ms,v.queue_wait_ms),(48<<20,20,20,65536,1,2000),"enforced fields carry their running values");
    assert_eq!((v.req_buf,v.conn_buf),(pocold::rpc::REQ_BUF as u64,pocold::rpc::SMALL_REPLY_MAX as u64));
    // Review 2 (NF1): every enforced fact is its running value, field by field, against the limits the
    // node runs with (written out here, not taken from rpc_facts), and the rows are RPC_ENFORCED.
    use pocold::rpc as r;let l=r::Limits::default();let ms=|d:std::time::Duration|d.as_millis() as u64;
    assert_eq!(pocold::node::Limits::default().rpc,l,"the node runs the default RPC limits");
    let rows:[(&str,u64,u64);25]=[("conn_max",v.conn_max,l.conn_max as u64),("req_buf",v.req_buf,r::REQ_BUF as u64),("conn_buf",v.conn_buf,r::SMALL_REPLY_MAX as u64),
        ("send_sock_buf",v.send_sock_buf,r::SEND_SOCK_BUF as u64),("heavy_active",v.heavy_active,1),("light_active",v.light_active,1),("light_queue",v.light_queue,l.light_queue as u64),
        ("heavy_resp_mem",v.heavy_resp_mem,r::HEAVY_RESP_POOL as u64),("light_resp_mem",v.light_resp_mem,((l.conn_max+1)*r::LIGHT_RESP_MAX) as u64),("resp_max",v.resp_max,r::RESP_MAX as u64),
        ("light_resp_max",v.light_resp_max,r::LIGHT_RESP_MAX as u64),("batch_resp_max",v.batch_resp_max,r::BATCH_RESP_MAX as u64),("batch_max",v.batch_max,r::BATCH_MAX as u64),
        ("err_item_max",v.err_item_max,r::ERR_ITEM_MAX as u64),("logs_count_max",v.logs_count_max,r::LOGS_COUNT_MAX as u64),("logs_scan_bytes",v.logs_scan_bytes,r::LOGS_SCAN_BYTES as u64),
        ("rate",v.rate,u64::from(l.rate)),("bucket_cap",v.bucket_cap,u64::from(l.bucket_cap)),("queue_wait_ms",v.queue_wait_ms,ms(l.queue_wait)),("compute_heavy_ms",v.compute_heavy_ms,ms(l.compute_heavy)),
        ("grace_ms",v.grace_ms,ms(l.grace)),("min_send_rate",v.min_send_rate,l.min_send_rate),("batch_wall_ms",v.batch_wall_ms,ms(l.batch_wall)),
        ("enforce_slack_ms",v.enforce_slack_ms,ms(l.enforce_slack)),("timer_eps_ms",v.timer_eps_ms,ms(l.timer_eps))];
    assert_eq!(rows.map(|x|x.0),pocold::RPC_ENFORCED,"one row per enforced fact, in order");
    for (k,fact,running) in rows{assert_eq!(fact,running,"{k}: the fact is the running value");}
    // Review 2 (m1): rate is equivalent and vacuous per connection, and the notes say so.
    let j=pocold::admission_json(a);let notes:Vec<&str>=j["notes"].as_array().unwrap().iter().filter_map(|x|x.as_str()).collect();
    for (f,_) in pocold::RPC_EQUIVALENT{assert!(pocold::RPC_ENFORCED.contains(f)&&notes.iter().any(|n|n.starts_with(&format!("R26 {f}: equivalent"))),"{f}: {notes:?}");}
    assert!(notes.iter().any(|n|n.starts_with("R26 rate: equivalent, vacuous per connection")),"{notes:?}");
    // 15 of the 18 R26 predicates read only enforced fields and are checked (all pass, sendSockBuf<=64KiB
    // among them); 3 read an absent field and are waived: derive and batchWallMin<=batchWallMs
    // (scratch, cancel_max), memoryRequired<=memTotal.
    let w=&a.report().waivers;assert_eq!(w.len(),1);assert_eq!(w[0].checked.len(),15);assert!(w[0].checked.iter().any(|c|*c=="sendSockBuf<=64KiB"));
    let mut waived=w[0].waived.clone();waived.sort();assert_eq!(waived,["batchWallMin<=batchWallMs","derive","memoryRequired<=memTotal"]);
    assert_eq!(j["scope"],"experimental-devnet");assert_eq!(j["specificationCompliant"],false);
    assert!(j["conditions"].as_array().unwrap().iter().any(|c|c["id"]=="rpc-absent"));
    // Specification scope on the same artifacts and local facts.
    let n=devnet::build(&base()).unwrap();let t=admission::measure(&n.spec_bytes,&n.alloc,1).unwrap();let local=pocold::single_host_local();
    let e=admission::admit(&Request{scope:Scope::Specification,..n.request(&local,Some(&t))}).err().expect("not specification compliance");
    assert!(e.missing.contains(&"specification:rpc-absent".to_string()),"{e}");
    let _=std::fs::remove_dir_all(&d);
}
/// R14/R19 in store mode (C-1): a node about to open a data directory in store mode is admitted on a timing that
/// includes the calibration blocks applied through a scratch state store in that directory (each block's nodes
/// committed and fsynced), and its timing condition says so. The scratch store is removed before the node opens,
/// one a crash left there is replaced, and the node then opens in store mode on that spec. Whole-state
/// admission is unchanged.
#[test] fn a_store_mode_boot_is_timed_through_a_scratch_state_store_in_its_data_directory(){
    use pocold::node::{Config,Limits,Node};
    let d=tmp("store-timing");let dir=d.join("net");publish(&base(),&dir,true);let data=d.join("data");
    let scratch=data.join(pocold::CALIBRATION_STORE);std::fs::create_dir_all(&scratch).unwrap();std::fs::write(scratch.join("state.redb"),b"left by a crash").unwrap();
    let detail=|s:&pocold::chain::Spec|s.admission().report().conditions.iter().find(|c|c.id=="timing-not-a-bound").map(|c|c.detail.clone()).expect("the timing condition");
    let spec=pocold::load_spec_for(&dir,&pocold::single_host_local(),Some(&data)).unwrap();
    assert!(detail(&spec).contains("through a scratch node store"),"{}",detail(&spec));
    assert!(!scratch.exists(),"the scratch store is removed before the node opens");
    // 2026-10-28 (Windows run 40680583220): the whole-state admission was refused once on R14/R19 with no times to
    // show why; a refusal now prints the timing of a calibration run made right after it.
    let whole=pocold::load_spec(&dir,&pocold::single_host_local()).unwrap_or_else(|e|{let n=devnet::build(&base()).unwrap();
        panic!("{e}; a calibration run right after it: {:?}",pocol_lp4_execution::admission::measure(&n.spec_bytes,&n.alloc,1).map(|t|t.phase_ms))});
    assert!(!detail(&whole).contains("node store"),"{}",detail(&whole));
    let n=Node::open(Config{dir:data.clone(),name:"st".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits{state_store:true,..Default::default()}},spec).unwrap();
    assert!(n.state_store().is_some()&&n.status_json()["stateStore"]==true);
    drop(n);let _=std::fs::remove_dir_all(&d);
}
