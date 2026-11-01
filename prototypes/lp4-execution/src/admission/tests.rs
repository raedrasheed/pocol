//! A02 corrected behaviour: the full gate runs on the real artifacts, every reproduced
//! violation is rejected with its rule, each rule has a boundary case, approved profiles
//! are admitted, and results are deterministic. Rule lists are asserted exactly.
use super::*;
use crate::{devnet::{self,Devnet,DevnetConfig},fixed};

fn cfg()->DevnetConfig{DevnetConfig::new(777920,1_800_000_000,&["a","b","c"])}
fn net(c:&DevnetConfig)->Devnet{devnet::build(c).expect("candidate")}
/// Timing measured once for the base profile, rebound to `bytes` so that rule
/// attribution tests isolate the rule under test (R14/R19 have their own tests).
fn rebound(t:&Timing,bytes:&[u8])->Timing{Timing{genesis_hash:gs::genesis_hash(bytes),..t.clone()}}
fn base()->(Devnet,Timing){let n=net(&cfg());let t=measure(&n.spec_bytes,&n.alloc,1).expect("measure");(n,t)}
fn run(n:&Devnet,t:&Timing,local:&NodeLocal)->Result<Admitted,Rejection>{let t=rebound(t,&n.spec_bytes);admit(&n.request(local,Some(&t)))}
fn rules(r:Result<Admitted,Rejection>)->Vec<String>{match r{Ok(_)=>vec![],Err(e)=>e.rules}}
fn v(x:&[&str])->Vec<String>{x.iter().map(|s|s.to_string()).collect()}
/// The candidate's spec with CP edits and the allocRoot re-derived for them, as a
/// careful operator would; the gate, not a stale root, must catch the edit.
fn edited(n:&Devnet,edits:&[(usize,u64)])->Vec<u8>{
    let mut spec=gs::decode(&n.spec_bytes).unwrap();for (i,x) in edits{spec.cp[*i]=fixed::U256::from_u64(*x)}
    let b=gs::encode(&spec);
    if let Ok(p)=fact_identity(&b,&gs::genesis_hash(&b),spec.chain_id){if let Ok(r)=Genesis::derive_root(&n.alloc,&p,&SystemPrograms::v1()){spec.alloc_root=r.0}}
    gs::encode(&spec)
}
fn run_bytes(n:&Devnet,t:&Timing,bytes:&[u8],local:&NodeLocal)->Vec<String>{let t=rebound(t,bytes);rules(admit(&Request{spec_bytes:bytes,..n.request(local,Some(&t))}))}
fn sh()->NodeLocal{NodeLocal::single_host()}

#[test] fn approved_single_host_devnet_is_admitted_with_reported_provenance(){
    let (n,t)=base();let a=admit(&n.request(&sh(),Some(&t))).unwrap_or_else(|e|panic!("{e}"));
    assert_eq!(a.genesis_hash(),n.profile.genesis_hash());assert_eq!(a.genesis().root(),n.genesis.root());
    let r=a.report();assert_eq!(r.calibration_class,Some(FactClass::SyntheticFixture));assert_eq!(r.resource_class,Some(FactClass::Measurement));
    // D-A13-PROFILE: the retained-state bound fits HIST_TRIE_BUDGET; no R21 exception is used.
    assert!(!r.r21_exception&&r.historical_bytes<=HIST_TRIE_BUDGET,"{} {:?}",r.historical_bytes,r.notes);
    let a=admit(&n.request(&NodeLocal{retention_stress:false,..sh()},Some(&t)));assert!(a.is_ok(),"admitted without retentionStress");
    assert!(t.phase_ms.iter().all(|ms|*ms<=1000),"t_exec=1 s must cover every measured phase: {t:?}");
}
#[test] fn approved_p11_timing_profile_is_admitted(){
    // P11: T_blk=10, tau=600, c=2, D_att=60, dFbWait=6 with P11 local timing.
    let mut c=cfg();c.t_blk=10;c.tau=600;c.c=2;c.d_att=60;c.d_fb_wait=6;c.hash_rate=1<<14;
    let n=net(&c);let t=measure(&n.spec_bytes,&n.alloc,1).unwrap();
    admit(&n.request(&NodeLocal::p11(),Some(&t))).unwrap_or_else(|e|panic!("{e}"));
}
/// PR #27 review: with a long closing sequence (here 33 blocks to the final one, D = 30) the London base fee
/// of full blocks passed the old fixed fee cap (16·baseFee0) after about 24 blocks, and the closing and
/// final samples were then empty blocks. Every sample now carries the GAS_LIMIT workload (measure
/// refuses a block that does not).
#[test] fn calibration_keeps_the_full_gas_workload_through_a_long_closing_sequence(){
    let mut c=cfg();c.d=30;let n=net(&c);
    let (t,gas)=measure_with_gas(&n.spec_bytes,&n.alloc,1).unwrap_or_else(|e|panic!("{e}"));
    assert!(t.samples>=30,"the sequence runs past the ~24 blocks the old fee cap covered: {t:?}");
    // Independently of measure's guard: every sampled block, closing and final included, ran the workload.
    let gl=n.profile.cp().GAS_LIMIT;
    assert!(gas.len()>=30&&gas.iter().all(|g|*g>=gl),"every sample used GAS_LIMIT ({gl}): {gas:?}");
}
/// R14/R19 for a store-mode node (C-1): the calibration through a node store runs the same blocks as the one in
/// memory (the same state root and gas at every height, every phase class reached), commits each block, and
/// releases every version it pinned. The timing marked stored is reported as such by the admission.
#[test] fn the_store_calibration_runs_the_same_blocks_through_the_node_store_and_releases_its_pins(){
    use crate::node_trie::{Batch,Id,Node,NodeStore,SharedMem,TrieStore};
    /// The test store: SharedMem, counting commits.
    #[derive(Default)] struct Counting(SharedMem,std::sync::atomic::AtomicU64);
    impl NodeStore for Counting{fn node(&self,id:&Id)->Option<std::sync::Arc<Node>>{self.0.node(id)}fn code(&self,h:&B256)->Option<Bytecode>{self.0.code(h)}}
    impl TrieStore for Counting{
        fn commit(&self,b:&Batch,root:Option<Id>)->Result<(),String>{self.1.fetch_add(1,std::sync::atomic::Ordering::SeqCst);self.0.commit(b,root)}
        fn unpin(&self,root:&Id)->Result<(),String>{self.0.unpin(root)}
        fn pinned(&self)->Result<u64,String>{self.0.pinned()}}
    let n=net(&cfg());let store=Counting::default();
    let (tm,gm,rm)=measure_run(&n.spec_bytes,&n.alloc,1,None).unwrap_or_else(|e|panic!("{e}"));
    let (ts,gs_,rs)=measure_run(&n.spec_bytes,&n.alloc,1,Some(&store)).unwrap_or_else(|e|panic!("{e}"));
    assert!(rm.len()>=3&&rm==rs,"the same state root at every height: {} vs {}",rm.len(),rs.len());
    assert_eq!(gm,gs_,"the same gas at every height");assert_eq!(tm.samples,ts.samples);
    assert!(!tm.stored&&ts.stored&&ts.genesis_hash==tm.genesis_hash);
    assert_eq!(store.1.load(std::sync::atomic::Ordering::SeqCst),rs.len() as u64+1,"the genesis and every block committed once");
    assert_eq!(store.pinned().unwrap(),0,"every version the calibration pinned is released");
    let a=admit(&n.request(&sh(),Some(&tm.with_store(&ts).unwrap()))).unwrap_or_else(|e|panic!("{e}"));
    let c=a.report().conditions.iter().find(|c|c.id=="timing-not-a-bound").unwrap();
    assert!(c.detail.contains("through a scratch node store"),"{}",c.detail);
    let a=admit(&n.request(&sh(),Some(&tm))).unwrap();
    assert!(!a.report().conditions.iter().any(|c|c.detail.contains("node store")),"an in-memory timing is not reported as stored");
}
/// A block's durable commit is inside its timed span: with a store whose commit takes at least 25 ms, every phase
/// of the store measurement is at least that, and the in-memory one (no commit) is not affected.
#[test] fn the_store_calibration_times_each_block_commit(){
    use crate::node_trie::{Batch,Id,Node,NodeStore,SharedMem,TrieStore};
    #[derive(Default)] struct Slow(SharedMem);
    impl NodeStore for Slow{fn node(&self,id:&Id)->Option<std::sync::Arc<Node>>{self.0.node(id)}fn code(&self,h:&B256)->Option<Bytecode>{self.0.code(h)}}
    impl TrieStore for Slow{
        fn commit(&self,b:&Batch,root:Option<Id>)->Result<(),String>{std::thread::sleep(std::time::Duration::from_millis(25));self.0.commit(b,root)}
        fn unpin(&self,root:&Id)->Result<(),String>{self.0.unpin(root)}
        fn pinned(&self)->Result<u64,String>{self.0.pinned()}}
    let n=net(&cfg());let t=measure_on_store(&n.spec_bytes,&n.alloc,1,&Slow::default()).unwrap_or_else(|e|panic!("{e}"));
    assert!(t.phase_ms.iter().all(|p|*p>=25),"every phase includes its commit: {:?}",t.phase_ms);
}
/// A store-mode node's timing takes, per phase class, the larger of its in-memory and store measurements, so a
/// store path slower than t_exec is refused by R14/R19 even when memory is fast; a store measurement for another
/// genesis is refused.
#[test] fn a_store_mode_timing_takes_the_larger_phase_and_gates_r14_r19(){
    let (n,t)=base();
    let slow=Timing{phase_ms:[3,4,1001],samples:5,stored:true,..t.clone()};
    let c=t.with_store(&slow).unwrap();
    assert_eq!(c.phase_ms,[t.phase_ms[0].max(3),t.phase_ms[1].max(4),1001]);assert!(c.stored);assert_eq!(c.samples,t.samples+5);
    assert_eq!(rules(admit(&n.request(&sh(),Some(&c)))),v(&["R14","R19"]),"a store path above t_exec is refused");
    let fast=Timing{phase_ms:[1,1,1],..slow.clone()};let c=t.with_store(&fast).unwrap();assert_eq!(c.phase_ms,t.phase_ms,"memory slower: memory's times");
    assert!(admit(&n.request(&sh(),Some(&c))).is_ok());
    assert!(t.with_store(&Timing{genesis_hash:[7;32],..fast}).is_err(),"another genesis");
    let j=c.to_json();assert_eq!(Timing::from_json(&j).unwrap().stored,true);
    let mut old=t.to_json();old.as_object_mut().unwrap().remove("stored");assert!(!Timing::from_json(&old).unwrap().stored,"a record without the field is in memory");
}
#[test] fn audit_reproductions_are_rejected_with_their_rules(){
    let (_,t)=base();
    let mut c=cfg();c.s_max=1;assert_eq!(rules(run(&net(&c),&t,&sh())),v(&["R9"]),"A02-R9 S_max=1");
    let mut c=cfg();c.h_close=c.h_end;assert_eq!(rules(run(&net(&c),&t,&sh())),v(&["R13"]),"A02-R13 H_CLOSE=H_END");
    // As audited: R_max=2 (the audited devnet default), so the genesis holds 69 system slots.
    let mut c=cfg();c.r_max=2;c.sys_slot_max=1;let n=net(&c);
    let e=run(&n,&t,&sh()).err().expect("SYS_SLOT_MAX=1 must be rejected");assert_eq!(e.rules,v(&["R18"]));
    let g=Genesis::build(&n.alloc,&n.profile,&SystemPrograms::v1()).unwrap();assert_eq!(resources::sys_slots(g.state()),69,"genesis needs 69 system slots");
}
#[test] fn timing_rules_r1_to_r7_at_their_boundaries(){
    let (n,t)=base();
    assert_eq!(rules(run(&n,&t,&NodeLocal{delta_rel:4,..sh()})),v(&["R2"]));
    assert!(run(&n,&t,&NodeLocal{delta_rel:5,..sh()}).is_ok());
    let mut c=cfg();c.d_fb_wait=2;assert_eq!(rules(run(&net(&c),&t,&sh())),v(&["R3"]));
    assert_eq!(rules(run(&n,&t,&NodeLocal{phi:2,..sh()})),v(&["R2","R3","R4","R7"]),"Φ=4 ≥ Δ_fb_wait=3");
    let mut c=cfg();c.d_att=8;assert_eq!(rules(run(&net(&c),&t,&sh())),v(&["R5","R7"]));
    let mut c=cfg();c.d_att=24;assert_eq!(rules(run(&net(&c),&t,&sh())),v(&["R7"]));
    let mut c=cfg();c.d_att=25;assert!(run(&net(&c),&t,&sh()).is_ok());
    assert!(rules(run(&n,&t,&NodeLocal{phi:u32::MAX,..sh()})).contains(&"R1".to_string()),"an overflowing Φ=2φ fails R1");
}
#[test] fn size_membership_and_end_rules_r8_to_r13(){
    let (n,t)=base();
    let mut c=cfg();c.u=19;assert_eq!(rules(run(&net(&c),&t,&sh())),v(&["R8"]),"U < E_win+D");
    let mut c=cfg();c.e_win=2049;c.u=2053;assert_eq!(rules(run(&net(&c),&t,&sh())),v(&["R8"]),"E_win > R_keep");
    // Genesis construction itself refuses B_code_max < 24577 (code_caps), so the identity
    // facts are absent as well; the note names the construction failure.
    assert_eq!(run_bytes(&n,&t,&edited(&n,&[(34,24576)]),&sh()),v(&["R9","R23","R24"]),"B_code_max");
    assert!(run_bytes(&n,&t,&edited(&n,&[(34,24577)]),&sh()).is_empty());
    assert_eq!(run_bytes(&n,&t,&edited(&n,&[(32,196608-16384+1)]),&sh()),v(&["R10"]));
    assert!(run_bytes(&n,&t,&edited(&n,&[(32,196608-16384)]),&sh()).is_empty());
    assert_eq!(rules(run(&n,&t,&NodeLocal{k_hist:12,..sh()})),v(&["R11"]),"k_view < K_hist");
    let r=run_bytes(&n,&t,&edited(&n,&[(18,2)]),&sh());assert_eq!(r.first().map(|s|s.as_str()),Some("R12"),"M_max < |M_0|: {r:?}");
    let mut c=cfg();c.t_close_offset=c.t_end_offset+1;assert_eq!(rules(run(&net(&c),&t,&sh())),v(&["R13"]),"T_CLOSE > T_END");
    let mut c=cfg();c.h_close=c.h_end-160;assert!(run(&net(&c),&t,&sh()).is_ok(),"H_CLOSE+Z_close = H_END");
}
#[test] fn resource_rules_r14_to_r21(){
    let (n,t)=base();
    assert_eq!(rules(admit(&n.request(&sh(),None))),v(&["R14","R19"]),"no timing measurement");
    let other=Timing{genesis_hash:[7;32],..t.clone()};assert_eq!(rules(admit(&n.request(&sh(),Some(&other)))),v(&["R14","R19"]),"measured for another genesis");
    let slow=Timing{phase_ms:[10,10,1001],..t.clone()};assert_eq!(rules(run(&n,&slow,&sh())),v(&["R14","R19"]),"final phase above t_exec");
    let mut g=DEFAULT_GOSSIP;g[1]=196608+685; // template = BODY_MAX + 686-byte envelope
    assert_eq!(rules(run(&n,&t,&NodeLocal{gossip:g,..sh()})),v(&["R15"]));
    // R16 with R_max=2: M_max + R_max*(Lmax+LAG) = 64 + 2*(max(40, 2*16+4+1)+2048) = 4240.
    let mut c=cfg();c.r_max=2;c.reg_records_max=4239;assert_eq!(rules(run(&net(&c),&t,&sh())),v(&["R16"]));
    let mut c=cfg();c.r_max=2;c.reg_records_max=4240;assert!(run(&net(&c),&t,&sh()).is_ok());
    // R16 with the devnet's R_max=0: REG_RECORDS_MAX >= M_max = 64.
    let mut c=cfg();c.reg_records_max=63;assert_eq!(rules(run(&net(&c),&t,&sh())),v(&["R16"]));
    assert_eq!(run_bytes(&n,&t,&edited(&n,&[(21,3)]),&sh()),v(&["R17"]),"SWEEP_MAX < R_max+E_max = 4");
    assert!(run_bytes(&n,&t,&edited(&n,&[(21,4)]),&sh()).is_empty());
    let b=resources::derive(n.profile.cp());
    let mut c=cfg();c.sys_slot_max=b.slot_min as u32-1;assert_eq!(rules(run(&net(&c),&t,&sh())),v(&["R18"]));
    let mut c=cfg();c.sys_slot_max=b.slot_min as u32;assert!(run(&net(&c),&t,&sh()).is_ok());
    let keys=(b.keys_normal+u64::from(n.profile.cp().SYS_KEYS_USER_MAX)).max(b.keys_closing).max(b.keys_final);
    assert_eq!(run_bytes(&n,&t,&edited(&n,&[(41,keys-1)]),&sh()),v(&["R18"]),"system+user keys above SYS_KEYS_BLOCK_MAX");
    assert!(run_bytes(&n,&t,&edited(&n,&[(41,keys)]),&sh()).is_empty());
    assert_eq!(run_bytes(&n,&t,&edited(&n,&[(44,30_000_000)]),&sh()),v(&["R18"]),"phase gas above G_SYS");
    // R20: Z_close >= ceil(8192/128)+P2+D+1 = 71.
    assert_eq!(run_bytes(&n,&t,&edited(&n,&[(49,70)]),&sh()),v(&["R20"]));
    assert!(run_bytes(&n,&t,&edited(&n,&[(49,71)]),&sh()).is_empty());
    // R21 at network-scale caps: pocold's full snapshots exceed HIST_TRIE_BUDGET; only the
    // simnet retentionStress exception admits it, and a node never gets that exception.
    let mut c=cfg();c.slot_cap=4_000_000;c.acct_cap=1_000_000;let big=net(&c);
    assert_eq!(rules(run(&big,&t,&NodeLocal{retention_stress:false,..sh()})),v(&["R21"]));
    let a=run(&big,&t,&sh()).unwrap();assert!(a.report().r21_exception&&a.report().notes.iter().any(|x|x.contains("retentionStress")));
    // Outside the measured SYS gas domain the gas facts are absent: R18, never assumed.
    let mut c=cfg();c.d=17;c.u=41;assert_eq!(rules(run(&net(&c),&t,&sh())),v(&["R18"]),"D=17 outside the gas domain");
}
#[test] fn identity_rules_r22_to_r26(){
    let (n,t)=base();let tt=rebound(&t,&n.spec_bytes);
    // The caps-test build refuses every non-simnet request first (validation.md:2065).
    let r22=if cfg!(feature="caps-test"){"caps-test"}else{"R22"};
    assert_eq!(rules(admit(&Request{node:NodeKind::Node,..n.request(&sh(),Some(&tt))})),v(&[r22]),"simnet profile on a node: [R22] (governance.md)");
    // R24 accepts the implemented full-range v2 while preserving the original
    // genesis identity and applying the new version only at its scheduled height.
    let upgraded=admit(&Request{forks:&[(1,0),(2,50)],..n.request(&sh(),Some(&tt))}).expect("scheduled full-range v2 is implemented");
    assert_eq!(upgraded.genesis_hash(),n.profile.genesis_hash());
    assert_eq!(upgraded.genesis().root(),n.genesis.root());
    assert_eq!(upgraded.profile().forks().entries(),&[(1,0),(2,50)]);
    for (height,version) in [(49,1),(50,2),(51,2)]{assert_eq!(upgraded.profile().forks().active_version(height),version);}
    for unknown in [3,u16::MAX]{
        assert_eq!(rules(admit(&Request{forks:&[(1,0),(unknown,50)],..n.request(&sh(),Some(&tt))})),v(&["R24"]),"unknown version {unknown} remains rejected");
    }
    let mut alloc=n.alloc.clone();let a=devnet::account_key("alice").address();alloc.accounts.get_mut(&a).unwrap().info.balance+=revm::primitives::U256::from(1);
    assert_eq!(rules(admit(&Request{alloc:&alloc,..n.request(&sh(),Some(&tt))})),v(&["R24"]),"alloc does not match allocRoot");
    let mut big=n.alloc.clone();big.accounts.get_mut(&a).unwrap().info.balance=revm::primitives::U256::from(1)<<127;
    let r=rules(admit(&Request{alloc:&big,..n.request(&sh(),Some(&tt))}));assert!(r.contains(&"R23".to_string()),"{r:?}");
    assert_eq!(rules(admit(&Request{calibration:Some((n.hash_rate+1,FactClass::SyntheticFixture)),..n.request(&sh(),Some(&tt))})),v(&["R25"]),"target_g is not the calibrated value");
    let e=admit(&Request{calibration:None,..n.request(&sh(),Some(&tt))}).err().unwrap();assert_eq!(e.rules,v(&["R25"]));assert!(!e.missing.is_empty());
    let mut rpc=RpcLimits::default();rpc.conn_max=1000;assert_eq!(rules(run(&n,&t,&NodeLocal{rpc:RpcFacts::fixture(rpc),..sh()})),v(&["R26"]));
    let mut bytes=n.spec_bytes.clone();bytes.pop();assert_eq!(rules(admit(&Request{spec_bytes:&bytes,..n.request(&sh(),Some(&tt))})),v(&["R24"]),"undecodable GenesisSpec");
}
#[test] fn rejections_are_deterministic_and_name_their_causes(){
    let (_,t)=base();let mut c=cfg();c.s_max=1;c.h_close=c.h_end;c.d_att=8;let n=net(&c);
    let a=run(&n,&t,&sh()).err().unwrap();let b=run(&n,&t,&sh()).err().unwrap();
    assert_eq!(a,b);assert_eq!(a.rules,v(&["R5","R7","R9","R13"]));
    assert!(a.to_string().starts_with("-32018 ParamGate rejected [R5, R7, R9, R13]"),"{a}");
}

// D-A02-SCOPE: an absent RpcGuard field is never nominal; specification scope refuses every open
// condition; the experimental-devnet scope waives only R26 predicates reading absent fields.
/// pocold's enforced/equivalent RpcGuard fields (pocold::RPC_ENFORCED) at its running values
/// (RpcGuard round 2). heavy_active/light_active = 1 are the single-consensus-thread equivalent;
/// send_sock_buf, mem_total and timer_eps_ms stay absent.
fn pocold_like()->RpcFacts{RpcFacts::enforced(&[("conn_max",32),("req_buf",1<<20),("conn_buf",64<<10),("heavy_active",1),("light_active",1),("light_queue",32),
    ("heavy_resp_mem",48<<20),("light_resp_mem",33*(512<<10)),("resp_max",8<<20),("light_resp_max",512<<10),("batch_resp_max",16<<20),("scratch_heavy",16<<20),("scratch_light",1<<20),
    ("batch_max",20),("err_item_max",256),("logs_count_max",10000),("logs_scan_bytes",32<<20),("rate",10_000),("bucket_cap",10_000),("queue_wait_ms",2000),("compute_heavy_ms",5000),
    ("cancel_max_ms",100),("grace_ms",2000),("min_send_rate",65536),("batch_wall_ms",420000),("enforce_slack_ms",200)]).unwrap()}
#[test] fn r26_predicates_are_the_gate_conjunction(){
    // Deterministic configurations around every boundary; the split must equal the gate's verdict.
    let mut x=0x9e3779b97f4a7c15u64;let mut next=||{x^=x<<13;x^=x>>7;x^=x<<17;x};
    let d=RpcLimits::default();let (mut agree,mut pass,mut fail)=(0,0,0);
    let mut fails_alone=vec![false;R26_PREDICATES.len()];
    for i in 0..20000u32{
        let mut l=d.clone();
        for f in RPC_FIELDS{if next()%6==0{let v=scope_slot(&mut l,f);*v=match next()%6{0=>0,1=>*v/2,2=>v.saturating_mul(2),3=>v.saturating_mul(3),4=>v.saturating_add(1),_=>v.saturating_sub(1)}}}
        if i%1000==0{l=d.clone()}
        let each:Vec<bool>=R26_PREDICATES.iter().map(|p|(p.2)(&l)).collect();
        assert_eq!(each.iter().all(|b|*b),l.satisfies_r26(),"{l:?}");agree+=1;
        if l.satisfies_r26(){pass+=1}else{fail+=1}
        if each.iter().filter(|b|!**b).count()==1{fails_alone[each.iter().position(|b|!*b).unwrap()]=true}
    }
    assert!(agree==20000&&pass>100&&fail>100,"{pass} {fail}");
    // Every predicate except the derive guard can decide the verdict alone (none is dead code).
    let dead:Vec<&str>=R26_PREDICATES.iter().zip(&fails_alone).filter(|(p,f)|!**f&&p.0!="derive").map(|(p,_)|p.0).collect();
    assert!(dead.is_empty(),"never the only failing predicate: {dead:?}");
}
fn scope_slot<'a>(l:&'a mut RpcLimits,f:&str)->&'a mut u64{
    match f{"conn_max"=>&mut l.conn_max,"req_buf"=>&mut l.req_buf,"conn_buf"=>&mut l.conn_buf,"send_sock_buf"=>&mut l.send_sock_buf,"heavy_active"=>&mut l.heavy_active,
        "light_active"=>&mut l.light_active,"light_queue"=>&mut l.light_queue,"heavy_resp_mem"=>&mut l.heavy_resp_mem,"light_resp_mem"=>&mut l.light_resp_mem,
        "resp_max"=>&mut l.resp_max,"light_resp_max"=>&mut l.light_resp_max,"batch_resp_max"=>&mut l.batch_resp_max,"scratch_heavy"=>&mut l.scratch_heavy,
        "scratch_light"=>&mut l.scratch_light,"mem_total"=>&mut l.mem_total,"batch_max"=>&mut l.batch_max,"err_item_max"=>&mut l.err_item_max,
        "logs_count_max"=>&mut l.logs_count_max,"logs_scan_bytes"=>&mut l.logs_scan_bytes,"rate"=>&mut l.rate,"bucket_cap"=>&mut l.bucket_cap,
        "queue_wait_ms"=>&mut l.queue_wait_ms,"compute_heavy_ms"=>&mut l.compute_heavy_ms,"cancel_max_ms"=>&mut l.cancel_max_ms,"grace_ms"=>&mut l.grace_ms,
        "min_send_rate"=>&mut l.min_send_rate,"batch_wall_ms"=>&mut l.batch_wall_ms,"enforce_slack_ms"=>&mut l.enforce_slack_ms,"timer_eps_ms"=>&mut l.timer_eps_ms,_=>unreachable!()}
}
#[test] fn absent_rpc_fields_enter_the_gate_as_zero_never_nominal(){
    let f=pocold_like();let v=f.values();
    // Only the three unenforced fields enter the gate as 0; the enforced/equivalent ones carry values.
    assert_eq!((v.send_sock_buf,v.mem_total,v.timer_eps_ms),(0,0,0));
    assert_eq!((v.heavy_resp_mem,v.rate,v.min_send_rate,v.compute_heavy_ms),(48<<20,10_000,65536,5000));
    assert_eq!((v.conn_max,v.resp_max,v.batch_max),(32,8<<20,20));assert_eq!(f.absent().len(),3);assert!(!f.is_fixture());
    assert!(!v.satisfies_r26(),"absent mechanisms (mem_total, timer_eps) cannot satisfy R26");
    assert!(RpcFacts::enforced(&[("rate_limit",1)]).is_err());assert!(RpcFacts::enforced(&[("rate",1),("rate",2)]).is_err());
}
#[test] fn specification_scope_refuses_every_open_condition(){
    let (n,t)=base();let tt=rebound(&t,&n.spec_bytes);
    // The lp4 fixture: complete RpcGuard values, synthetic calibration, a measured timing sample.
    let e=admit(&Request{scope:Scope::Specification,..n.request(&sh(),Some(&tt))}).err().expect("not specification evidence");
    assert_eq!(e.rules,v(&["R14","R18","R19","R21","R25","R26"]),"{e}");
    for id in ["rpc-fixture","calibration-synthetic","timing-not-a-bound","gas-monotonicity-assumed","gate-deferred-general-R18","gate-deferred-general-R21","gate-deferred-caps64"]{
        assert!(e.missing.contains(&format!("specification:{id}")),"{id}: {:?}",e.missing)}
    // pocold's facts: the absent mechanisms are named.
    let local=NodeLocal{rpc:pocold_like(),..sh()};
    let e=admit(&Request{scope:Scope::Specification,..n.request(&local,Some(&tt))}).err().unwrap();
    assert!(e.missing.contains(&"specification:rpc-absent".to_string())&&!e.missing.contains(&"specification:rpc-fixture".to_string()));
    assert!(e.notes.iter().any(|x|x.contains("send_sock_buf")&&x.contains("mem_total")&&x.contains("timer_eps_ms")),"{:?}",e.notes);
    // A gate violation is still reported with the conditions.
    let e=admit(&Request{scope:Scope::Specification,..n.request(&NodeLocal{delta_rel:4,..sh()},Some(&tt))}).err().unwrap();assert!(e.rules.contains(&"R2".to_string()));
}
#[test] fn experimental_devnet_waives_only_r26_predicates_reading_absent_fields(){
    let (n,t)=base();let tt=rebound(&t,&n.spec_bytes);
    let a=admit(&n.request(&NodeLocal{rpc:pocold_like(),..sh()},Some(&tt))).unwrap_or_else(|e|panic!("{e}"));
    assert_eq!(a.scope(),Scope::ExperimentalDevnet);assert!(!a.specification_compliant());
    let w=&a.report().waivers;assert_eq!(w.len(),1);assert_eq!(w[0].rule,"R26");
    // 14 predicates read only enforced/equivalent fields; the 4 reading mem_total, timer_eps_ms or
    // send_sock_buf are waived.
    assert_eq!(w[0].checked,vec!["derive","heavyRespMem>=respMax","lightRespMem>=connMax*lightRespMax","batchRespMax>=respMax","lightQueue>=connMax",
        "batchMax*errItemMax+64<=connBuf","respMax>=2*32768+546*512","logsCountMax>=546","logsScanBytes>=32768","lightRespMax>=385KiB","batchWallMin<=batchWallMs",
        "rate>=1","bucketCap>=batchMax","enforceSlack<=500"]);
    assert_eq!(w[0].waived,vec!["memoryRequired<=memTotal","timerEps>0","timerEps<enforceSlack","sendSockBuf<=64KiB"]);
    assert_eq!(w[0].waived.len()+w[0].checked.len(),R26_PREDICATES.len());
    let ids:Vec<&str>=a.report().conditions.iter().map(|c|c.id).collect();
    assert!(ids.contains(&"rpc-absent")&&ids.contains(&"calibration-synthetic")&&ids.contains(&"timing-not-a-bound"),"{ids:?}");
    // An enforced field below its R26 bound is not waivable.
    let bad=RpcFacts::enforced(&[("resp_max",1000),("batch_resp_max",16<<20)]).unwrap();
    let e=admit(&n.request(&NodeLocal{rpc:bad,..sh()},Some(&tt))).err().unwrap();assert_eq!(e.rules,v(&["R26"]));assert!(e.notes.iter().any(|x|x.contains("respMax>=2*32768+546*512")));
    let bad=RpcFacts::enforced(&[("logs_count_max",545)]).unwrap();assert_eq!(rules(admit(&n.request(&NodeLocal{rpc:bad,..sh()},Some(&tt)))),v(&["R26"]));
    // Fixture values are evaluated in full and reported as a fixture.
    let a=admit(&n.request(&sh(),Some(&tt))).unwrap();assert!(a.report().waivers.is_empty());assert!(a.report().conditions.iter().any(|c|c.id=="rpc-fixture"));
    // The scope is restricted to a simnet profile on a simnet node.
    let r22=if cfg!(feature="caps-test"){"caps-test"}else{"R22"};
    let e=admit(&Request{node:NodeKind::Node,..n.request(&NodeLocal{rpc:pocold_like(),..sh()},Some(&tt))}).err().unwrap();assert_eq!(e.rules,v(&[r22]));
    // Review F5: a Synthetic (or Network) profile on a simnet node does not get the waiver.
    for kind in [ProfileKind::Synthetic,ProfileKind::Network]{
        let e=admit(&Request{kind,..n.request(&NodeLocal{rpc:pocold_like(),..sh()},Some(&tt))}).err().unwrap();assert_eq!(e.rules,v(&[r22]),"{kind:?}");
    }
}
/// validation.md:2065: the caps-test build (LOG_CAP=2^24) admits simnet profiles on simnet nodes only.
#[cfg(feature="caps-test")]
#[test] fn the_caps_test_build_admits_only_simnet(){
    let (n,t)=base();assert!(run(&n,&t,&sh()).is_ok(),"a simnet devnet boots the caps-test build");
    for (kind,node) in [(ProfileKind::Network,NodeKind::Node),(ProfileKind::Network,NodeKind::Simnet),(ProfileKind::Simnet,NodeKind::Node),(ProfileKind::Synthetic,NodeKind::Simnet)]{
        for scope in [Scope::Specification,Scope::ExperimentalDevnet]{
            let tt=rebound(&t,&n.spec_bytes);let e=admit(&Request{kind,node,scope,..n.request(&sh(),Some(&tt))}).err().expect("refused");
            assert_eq!(e.rules,v(&["caps-test"]),"{kind:?} on {node:?}, {scope:?}");
        }
    }
}
