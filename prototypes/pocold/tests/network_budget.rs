//! A13 at the network profile (governance.md genesis parameters, profileKind=network): the model the
//! 6 GiB gate checks, itemized, for the decision package. pocold refuses to boot this profile; this
//! test records by how much and which terms carry it. It asserts nothing about admission.
use pocol_lp4_execution::devnet::{self,DevnetConfig};

fn network()->DevnetConfig{
    let labels:Vec<String>=(0..4).map(|i|format!("m{i}")).collect();let refs:Vec<&str>=labels.iter().map(|s|s.as_str()).collect();
    let mut c=DevnetConfig::new(777920,1_800_000_000,&refs);
    // chainId 777920: devnet::build refuses the network chainId 777001; chainId does not enter any A13 term.
    // governance.md: T_blk 60, tau 3600, c 2, D_att 300, dFbWait 25; m 32, kappa 2, S_max 256, D 16, W_w 64;
    // R_max 2, M_max 256, REG_RECORDS_MAX 16384; W_act 128, Theta_inact 1024; E_win 2048, U 2200;
    // caps; SYS_SLOT_MAX 131072; H_END 129600, H_CLOSE 129440, Z_close 160, RET 128, P2 2.
    c.t_blk=60;c.tau=3600;c.c=2;c.d_att=300;c.d_fb_wait=25;c.m=32;c.kappa=2;c.s_max=256;c.d=16;c.w_w=64;
    c.r_max=2;c.m_max=256;c.reg_records_max=16384;c.w_act=128;c.theta_inact=1024;c.e_win=2048;c.u=2200;
    c.code_cap=1<<30;c.txbytes_cap=1<<33;c.tx_cap=10_000_000;c.slot_cap=4_000_000;c.acct_cap=1_000_000;c.sys_slot_max=131072;
    c.h_end=129_600;c.h_close=129_440;c.t_end_offset=10_368_000;c.t_close_offset=10_368_000-9_600;c.z_close=160;c.ret=128;c.p2=2;c
}
fn print(tag:&str,b:&pocold::resources::Bound){
    let gib=|x:u128|x as f64/(1u128<<30) as f64;
    let need=pocold::resources::budget_required(b.total).unwrap();
    println!("{tag} {}",serde_json::json!({"total":b.total.to_string(),"totalGiB":gib(b.total),"budgetRequiredGiB":gib(need),"budgetGiB":6,
        "snapshotGiB":gib(b.snapshot_bytes),"snapshots":b.snapshots.to_string(),
        "groupsGiB":{"snapshots":gib(b.snapshot_total),"diffs":gib(b.diffs),"trie":gib(b.trie),"code":gib(b.code),"store":gib(b.store),"history":gib(b.history),"side":gib(b.side_blocks),"queues":gib(b.queues),"rpc":gib(b.rpc),"threads":gib(b.threads),"execution":gib(b.execution)},
        "items":b.items.iter().map(|(k,v)|serde_json::json!([k,v.to_string(),gib(*v)])).collect::<Vec<_>>()}));
}
/// C-1 E4.5: the same profile with the states in state.redb (`Limits::state_store`), with the pool at its
/// default gas bound (D-7, 4 × GAS_LIMIT), as pocold would boot it. Also state.redb's largest file, by part.
#[test] fn a13_store_model_at_the_network_profile(){
    let d=devnet::build(&network()).expect("network-profile candidate");let cp=d.profile.cp().clone();
    let l=pocold::single_host_local();
    let lim=pocold::node::Limits{state_store:true,..Default::default()};
    let b=pocold::resources::derive(&cp,32,l.side_max,&lim,2).expect("no overflow");
    print("A13-NETWORK-STORE",&b);
    let p=pocold::resources::state_file_parts(&cp,32,l.side_max,&lim).unwrap();let f:u128=p.iter().sum();
    println!("A13-NETWORK-STATE-FILE {}",serde_json::json!({"fileTiB":f as f64/(1u128<<40) as f64,"regions":f.div_ceil(pocold::resources::REDB_REGION_BYTES).to_string(),
        "regionAllocatorBytes":pocold::resources::redb_region_allocator(pocold::resources::REDB_REGION_PAGES).unwrap().to_string(),
        "partsTiB":{"versionsNodes":p[0] as f64/(1u128<<40) as f64,"preimages":p[1] as f64/(1u128<<40) as f64,"code":p[2] as f64/(1u128<<40) as f64,"checkpoints":p[3] as f64/(1u128<<40) as f64},"factor":pocold::resources::STATE_FILE_FACTOR.to_string()}));
}
/// D-7 (A13-DERIVATION §2d): the pool's terms follow its gas bound. The layer holds what that gas can write
/// (`layer_writes`); its code is its creations in both modes (in store mode its entries keep no copy of the code
/// they were read with, `Layer::drop_code_copies`). A bound past POOL_MAX_TXS ×
/// GAS_LIMIT counts as that (the count bound binds first). The block overlay is the layer of 5/4 GAS_LIMIT with
/// the SYSTEM phase's accounts and keys.
#[test] fn d7_a13_pool_terms_follow_the_gas_bound(){
    use pocold::{node::{Limits,POOL_MAX_TXS},resources::{self as r,derive}};use pocol_lp4_execution::resources::memory::code_bytes;
    let net=devnet::build(&network()).unwrap().profile.cp().clone();
    let dev=devnet::build(&DevnetConfig::new(777910,1_700_000_000,&["a","b","c"])).unwrap().profile.cp().clone();let l=pocold::single_host_local();
    let item=|b:&r::Bound,k:&str|b.items.iter().find(|x|x.0.starts_with(k)).unwrap_or_else(||panic!("{k}")).1;
    assert_eq!(r::layer_writes(32_000_000),Some((3*1523+13333+32,11034+320000)),"the counts at 4 × GAS_LIMIT");
    for cp in [&net,&dev]{
        let gl=u128::from(cp.GAS_LIMIT);let g=4*gl;let (pa,ps)=r::layer_writes(g).unwrap();
        for store in [false,true]{
            let at=|gmax:Option<u64>|derive(cp,32,l.side_max,&Limits{state_store:store,pool_gas_max:gmax,..Default::default()},2).unwrap();
            let b=at(None);
            assert_eq!(item(&b,"diffs/the pool's pending layer"),r::layer_bytes(pa,ps).unwrap());
            let spent=gl*5/4;let (oa,os)=r::layer_writes(spent).unwrap();
            // PR6: and its BLOCKHASH window (257 entries), which the template selection's overlay also carries.
            let window=257*pocol_lp4_execution::resources::memory::entry_bytes(64)+pocol_lp4_execution::resources::memory::table_bytes(64);
            assert_eq!(item(&b,"diffs/the overlay of the block being applied"),r::layer_bytes(oa+u128::from(cp.SYS_ACCT_BLOCK_MAX)+1,os+u128::from(cp.SYS_KEYS_BLOCK_MAX)).unwrap()+window);
            assert_eq!(item(&b,"code/the pool's pending layer"),code_bytes(g/r::CODE_DEPOSIT,g/r::CREATE_GAS).unwrap(),"store {store}");
            let b2=at(Some(8*cp.GAS_LIMIT));assert!(item(&b2,"diffs/the pool's pending layer")>item(&b,"diffs/the pool's pending layer")&&b2.total>b.total,"a larger bound counts more");
            assert_eq!(at(Some(u64::MAX)).total,at(Some(POOL_MAX_TXS as u64*cp.GAS_LIMIT)).total,"capped by the count bound");
            let body=u128::from(cp.BODY_MAX);assert_eq!(item(&b,"queues/an abandoned block's transactions"),body+body/r::MIN_TX_BYTES*80,"one abandoned body in revalidation");
            let e=r::check(cp,32,l.side_max,&Limits{state_store:store,pool_gas_max:Some(cp.GAS_LIMIT-1),..Default::default()},2).unwrap_err();assert!(e.contains("below GAS_LIMIT"),"{e}");
            // PR6: the pool's meta, the parked entries and the template selection's candidates, each at its bound; the
            // sender quota is checked with the gas bound.
            use pocold::node::{META_ENTRY_BYTES,PARKED_ENTRY_BYTES,POOL_PARKED_MAX};
            assert_eq!(item(&b,"queues/pool sender and declared gas"),3*POOL_MAX_TXS as u128*META_ENTRY_BYTES as u128);
            assert_eq!(item(&b,"queues/parked transactions' entries"),3*POOL_PARKED_MAX as u128*PARKED_ENTRY_BYTES as u128);
            assert_eq!(item(&b,"queues/template selection"),r::selection_bytes(cp).unwrap());assert!(r::selection_bytes(cp).unwrap()>2*body);
            // PR6 review: after a gas failure the selection's tail (at most GAS_LIMIT/TX_GAS_MIN transactions, BODY_MAX
            // bytes) joins the append round's candidates, so the term counts it beside their own bound.
            {use pocol_lp4_execution::resources::memory::{entry_bytes,table_bytes};
            let entries=body/r::MIN_TX_BYTES+1+u128::from(cp.GAS_LIMIT)/r::TX_GAS_MIN;let per=3*pocold::node::CAND_ENTRY_BYTES as u128+3*32+3*24+entry_bytes(28);
            let senders=(POOL_MAX_TXS+pocold::node::BUILD_VISIT_MAX) as u128;
            assert_eq!(r::selection_bytes(cp).unwrap(),2*body+u128::from(cp.TX_MAX)+entries*per+3*(senders*entry_bytes(20)+table_bytes(20)),"the candidates after a gas recovery");}
            let e=r::check(cp,32,l.side_max,&Limits{state_store:store,pool_sender_gas_max:Some(cp.GAS_LIMIT-1),..Default::default()},2).unwrap_err();assert!(e.contains("pool_sender_gas_max"),"{e}");
        }
    }
}
#[test] fn a13_model_at_the_network_profile(){
    let d=devnet::build(&network()).expect("network-profile candidate");let cp=d.profile.cp().clone();
    let l=pocold::single_host_local();
    // K_hist = 32 at the network profile (governance.md, local retention); side_max as pocold boots.
    let b=pocold::resources::derive(&cp,32,l.side_max,&pocold::node::Limits::default(),2).expect("no overflow");print("A13-NETWORK",&b);
    let need=pocold::resources::budget_required(b.total).unwrap();
    assert!(need>u128::from(pocold::node::MEM_BUDGET),"the network profile does not fit 6 GiB today (if this fails, the model changed: update the decision package)");
}

/// C-1 E4.5: the devnet profile pocold boots, whole-state and store mode with the default limits: both pass
/// the boot gate. Store mode is not the smaller bound here: the devnet's states are small, and its fixed
/// caches and worst-case batch and file terms outweigh them.
#[test] fn a13_store_model_at_the_devnet_profile(){
    let c=DevnetConfig::new(777910,1_700_000_000,&["a","b","c"]);let d=devnet::build(&c).unwrap();let cp=d.profile.cp().clone();let l=pocold::single_host_local();
    let (m,s)=(pocold::node::Limits::default(),pocold::node::Limits{state_store:true,..Default::default()});
    let bm=pocold::resources::check(&cp,l.k_hist,l.side_max,&m,0).expect("whole-state mode boots");let bs=pocold::resources::check(&cp,l.k_hist,l.side_max,&s,0).expect("store mode boots");
    print("A13-DEVNET-MEM",&bm);print("A13-DEVNET-STORE",&bs);
}

/// Review 2026-10-27: the stored versions' handles and their bytes are counted from one inventory, and it holds
/// every version open keeps at once: genesis, the window of K_hist with the block being imported, the abandoned
/// window, the side states, and a rolling re-execution with the version it commits next (2·K_hist + side_max + 4).
#[test] fn a13_counts_a_handle_for_every_held_version(){
    use pocold::resources as r;
    for (k,side) in [(32u32,2u32),(32,8),(1,0),(4096,64)]{
        let want=2*u128::from(k)+u128::from(side)+4;
        assert_eq!(r::held_runs(k,side).iter().sum::<u128>(),want,"the runs at K_hist {k}, side_max {side}");
        assert_eq!(r::stored_versions(k,side),want,"a handle per held version at K_hist {k}, side_max {side}");
    }
    let net=devnet::build(&network()).unwrap().profile.cp().clone();let one=r::chain_versions_bytes(&net,1).unwrap();
    assert_eq!(r::held_versions_bytes(&net,32,2).unwrap(),one+r::chain_versions_bytes(&net,33).unwrap()+r::chain_versions_bytes(&net,32).unwrap()+2*one+r::chain_versions_bytes(&net,2).unwrap(),
        "the versions' bytes are the same runs");
}

/// C-1 E4.5 (2026-10-27): state.redb's versions and a block's batch, re-derived from node_trie's bounds. The file
/// counts the versions held at once (genesis; the K_hist window and the block being imported; the abandoned window
/// until settle; side_max side states; a rolling re-execution and the version it commits next), once for the
/// entries and once for each kept checkpoint. A run of n consecutive versions is the oldest whole and n − 1 blocks'
/// additions, at most n whole versions.
#[test] fn a13_store_file_and_batch_terms_follow_the_node_trie_bounds(){
    use pocol_lp4_execution::node_trie::{batch_nodes,block_bytes,version_bytes,Id,Node,BRANCH_ENC,ROW};use pocold::resources as r;
    let net=devnet::build(&network()).unwrap().profile.cp().clone();
    let dev=devnet::build(&DevnetConfig::new(777910,1_700_000_000,&["a","b","c"])).unwrap().profile.cp().clone();let l=pocold::single_host_local();
    for cp in [&net,&dev]{
        let (ka,ks)=(u128::from(cp.ACCT_CAP)+5,u128::from(cp.SLOT_CAP)+u128::from(cp.SYS_SLOT_MAX));assert_eq!(r::state_keys(cp),Some((ka,ks)));
        let (wa,ws)=r::block_writes(cp).unwrap();let (whole,add)=(version_bytes(ka,ks).unwrap(),block_bytes(wa,ws,ka,ks).unwrap());
        let run=|n:u128|(n*whole).min(whole+(n-1)*add);
        for n in [1u128,2,5,33]{assert_eq!(r::chain_versions_bytes(cp,n),Some(run(n)),"a run of {n}");}
        let k=32u128;let sm=u128::from(l.side_max);
        let held=whole+run(k+1)+run(k)+sm*whole+run(2);assert_eq!(r::held_versions_bytes(cp,32,l.side_max),Some(held));
        let lim=pocold::node::Limits{state_store:true,..Default::default()};let p=r::state_file_parts(cp,32,l.side_max,&lim).unwrap();
        // D-6 (2026-10-27): and one dropped checkpoint whose nodes are owed, with a release row for at most each of its
        // nodes (each counted at ROW + BRANCH_ENC bytes or more).
        let owed_rows=ROW*(held/(ROW+BRANCH_ENC)+1);
        assert_eq!(p[0],r::STATE_FILE_FACTOR*((pocold::checkpoint::KEPT as u128+2)*held+owed_rows),"the entries' versions, each kept checkpoint's and one owed");
        // The batch: its branches and leaves behind their Arcs and map slots, every branch's children, the write set.
        let (b,lv,kids)=batch_nodes(wa,ws,ka,ks).unwrap();let slot=|n:usize|pocol_lp4_execution::resources::memory::entry_bytes(n as u128);
        let node=(2*std::mem::size_of::<usize>()+std::mem::size_of::<Node>()) as u128+slot(std::mem::size_of::<(Id,std::sync::Arc<Node>)>());
        let ws_bytes=wa*slot(std::mem::size_of::<(revm::primitives::Address,Option<Vec<revm::primitives::U256>>)>())+ws*pocol_lp4_execution::resources::memory::VEC*32;
        assert_eq!(r::batch_max(cp),Some((b+lv)*node+kids*std::mem::size_of::<(Id,pocol_lp4_execution::state_trie::Ref)>() as u128+ws_bytes));
        assert!(b<=64*(wa+ws)&&lv<=2*(wa+ws)&&kids<=16*b,"within the paths");
    }
    // At the network profile the window's run is the oldest version and its blocks' additions, not K_hist + 1 whole versions.
    assert!(r::chain_versions_bytes(&net,33).unwrap()<33*r::chain_versions_bytes(&net,1).unwrap());
}

/// Why the network profile does not boot, gate by gate. The CP is network() (governance.md; chainId 777920
/// only because the builder refuses 777001). The local parameters are governance.md's for a network node
/// (Δ=5, φ=5, Φ=10, t_exec=3, Δ_rel=26, Γ=30; K_hist=32; retentionStress off) with pocold's RPC facts. The timing
/// is measured here. Each R1–R26 line prints the rule's inputs, what it requires and what it gets; the rules
/// re-derived here must agree with gate::check's own verdict (`admit` on a simnet request, which returns the
/// pure violations), so the table cannot drift from the gate. Then the Specification-scope conditions, pocold's
/// A13 check in both modes, and the boot path's structural refusals. It asserts nothing about what should boot.
#[test] fn network_profile_boot_gates_itemized(){
    use pocol_lp4_execution::{admission::{self,NodeLocal,Request,Scope},gate::{FactClass,calibration_target},param_gate::registry_metrics,
        profile::{NodeKind,ProfileKind},timing::metrics};
    let d=devnet::build(&network()).expect("network-profile candidate");let c=d.profile.cp().clone();
    let local=NodeLocal{delta:5,phi:5,t_exec:3,delta_rel:26,gamma:30,k_hist:32,retention_stress:false,..pocold::single_host_local()};
    let timing=admission::measure(&d.spec_bytes,&d.alloc,1).expect("the calibration blocks apply");
    let req=|kind,node,scope|Request{spec_bytes:&d.spec_bytes,alloc:&d.alloc,forks:&d.forks,kind,node,local:&local,
        calibration:Some((d.hash_rate,FactClass::SyntheticFixture)),timing:Some(&timing),scope};
    let gate=admission::admit(&req(ProfileKind::Simnet,NodeKind::Simnet,Scope::ExperimentalDevnet)).err().map(|e|e.rules).unwrap_or_default();
    let (t,big_phi,dl)=(u128::from(local.t_exec),u128::from(local.capital_phi()),u128::from(local.delta));
    let m=metrics(local.delta.into(),big_phi as u64,local.t_exec.into(),local.delta_rel.into(),c.dFbWait.into());
    let reg=registry_metrics(c.U,c.E_win,c.D,c.M_max,c.R_max,c.REG_RECORDS_MAX,c.RET,c.P2).ok();
    let p99=timing.phase_ms.iter().copied().max().unwrap();
    let hist=admission::historical_bytes(&c,&local);
    // (rule, fails, inputs, required, actual)
    let rows:Vec<(&str,bool,String,String,String)>=vec![
        ("R2",u128::from(local.delta_rel)<m.sigma+dl+2*t,format!("Δ_rel={} σ={} Δ={} t_exec={}",local.delta_rel,m.sigma,dl,t),format!("Δ_rel ≥ σ+Δ+2t_exec = {}",m.sigma+dl+2*t),local.delta_rel.to_string()),
        ("R3",u128::from(c.dFbWait)<big_phi+dl+2*t,format!("dFbWait={} Φ={big_phi} Δ={dl} t_exec={t}",c.dFbWait),format!("dFbWait ≥ Φ+Δ+2t_exec = {}",big_phi+dl+2*t),c.dFbWait.to_string()),
        ("R4",big_phi>=u128::from(c.dFbWait)||u128::from(local.delta_rel)<t,format!("Φ={big_phi} dFbWait={} Δ_rel={} t_exec={t}",c.dFbWait,local.delta_rel),"Φ < dFbWait and Δ_rel ≥ t_exec".into(),format!("{big_phi} < {}, {} ≥ {t}",c.dFbWait,local.delta_rel)),
        ("R5",3*u128::from(c.dFbWait)>u128::from(c.D_att),format!("dFbWait={} D_att={}",c.dFbWait,c.D_att),format!("3·dFbWait ≤ D_att = {}",c.D_att),(3*u128::from(c.dFbWait)).to_string()),
        ("R7",u128::from(c.D_att)+u128::from(local.gamma)<2*(m.x_w+u128::from(c.c)*u128::from(c.T_blk))+big_phi,format!("D_att={} Γ={} X^W={} c={} T_blk={} Φ={big_phi}",c.D_att,local.gamma,m.x_w,c.c,c.T_blk),format!("D_att+Γ ≥ 2(X^W+c·T_blk)+Φ = {}",2*(m.x_w+u128::from(c.c)*u128::from(c.T_blk))+big_phi),(u128::from(c.D_att)+u128::from(local.gamma)).to_string()),
        ("R8",c.D<2||c.U<c.E_win+u32::from(c.D)||c.E_win>admission::R_KEEP,format!("D={} U={} E_win={} R_keep={}",c.D,c.U,c.E_win,admission::R_KEEP),"D ≥ 2, U ≥ E_win+D, E_win ≤ R_keep".into(),format!("{}, {} vs {}, {}",c.D,c.U,c.E_win+u32::from(c.D),c.E_win)),
        ("R9",u128::from(c.S_max)<4*u128::from(c.kappa)*u128::from(c.m)||c.B_code_max<24577,format!("S_max={} κ={} m={} B_code_max={}",c.S_max,c.kappa,c.m,c.B_code_max),format!("S_max ≥ 4κm = {}, B_code_max ≥ 24577",4*u128::from(c.kappa)*u128::from(c.m)),format!("{}, {}",c.S_max,c.B_code_max)),
        ("R10",u128::from(c.BODY_MAX)<u128::from(c.TX_MAX)+16384,format!("BODY_MAX={} TX_MAX={}",c.BODY_MAX,c.TX_MAX),format!("BODY_MAX ≥ TX_MAX+16384 = {}",u128::from(c.TX_MAX)+16384),c.BODY_MAX.to_string()),
        ("R13",c.H_END<=u64::from(c.Z_close)||u128::from(c.H_CLOSE)+u128::from(c.Z_close)>u128::from(c.H_END)||c.T_CLOSE>c.T_END,format!("H_END={} H_CLOSE={} Z_close={}",c.H_END,c.H_CLOSE,c.Z_close),"H_END > Z_close, H_CLOSE+Z_close ≤ H_END, T_CLOSE ≤ T_END".into(),format!("{} ≤ {}",u128::from(c.H_CLOSE)+u128::from(c.Z_close),c.H_END)),
        ("R14",u128::from(p99)>t*1000,format!("validate_p99 = max phase ms measured here {:?}",timing.phase_ms),format!("≤ t_exec·1000 = {}",t*1000),p99.to_string()),
        ("R16",reg.as_ref().map_or(true,|r|u128::from(c.REG_RECORDS_MAX)<r.records_required),format!("REG_RECORDS_MAX={} U={} E_win={} D={} M_max={} R_max={} RET={} P2={}",c.REG_RECORDS_MAX,c.U,c.E_win,c.D,c.M_max,c.R_max,c.RET,c.P2),format!("≥ records_required = {:?}",reg.as_ref().map(|r|r.records_required)),c.REG_RECORDS_MAX.to_string()),
        ("R17",u128::from(c.SWEEP_MAX)<u128::from(c.R_max)+u128::from(c.E_max),format!("SWEEP_MAX={} R_max={} E_max={}",c.SWEEP_MAX,c.R_max,c.E_max),format!("≥ R_max+E_max = {}",u128::from(c.R_max)+u128::from(c.E_max)),c.SWEEP_MAX.to_string()),
        ("R19",timing.phase_ms.iter().any(|ms|u128::from(*ms)>t*1000),format!("phase ms measured here {:?}",timing.phase_ms),format!("each ≤ {}",t*1000),format!("{:?}",timing.phase_ms)),
        ("R20",reg.as_ref().map_or(true,|r|u128::from(c.Z_close)<r.close_blocks_required),format!("Z_close={}",c.Z_close),format!("≥ close_blocks_required = {:?}",reg.as_ref().map(|r|r.close_blocks_required)),c.Z_close.to_string()),
        ("R21",u64::from(local.k_hist).min(c.H_END)<u64::from(local.k_hist)||hist>admission::HIST_TRIE_BUDGET,format!("(K_hist+side_max)·snapshot(ACCT_CAP+5, SLOT_CAP+SYS_SLOT_MAX), K_hist={} side_max={}",local.k_hist,local.side_max),format!("≤ HIST_TRIE_BUDGET = {} (no retentionStress exception off simnet)",admission::HIST_TRIE_BUDGET),hist.to_string()),
        ("R25",calibration_target(d.hash_rate,c.T_blk)!=Some(c.target_g),format!("H_cal={} (synthetic) T_blk={}",d.hash_rate,c.T_blk),"target_g = 2^256/(H_cal·T_blk)".into(),"equal (target_g derived from the same H_cal)".into()),
    ];
    for (rule,fails,inputs,required,actual) in &rows{
        println!("NETWORK-GATE {}",serde_json::json!({"rule":rule,"fails":fails,"inputs":inputs,"required":required,"actual":actual}));
        assert_eq!(*fails,gate.iter().any(|g|g==rule),"{rule}: the re-derivation disagrees with gate::check ({gate:?})");
    }
    println!("NETWORK-GATE-VERDICT {}",serde_json::json!({"gateViolations":gate,"t_execMs":t*1000,"phaseMs":timing.phase_ms,"sigma":m.sigma.to_string(),"x_h":m.x_h.to_string(),"x_w":m.x_w.to_string()}));
    // Specification scope (what a network node must pass): the gate plus every condition that keeps an admission
    // from being specification evidence.
    let spec=admission::admit(&req(ProfileKind::Synthetic,NodeKind::Simnet,Scope::Specification)).err().expect("refused in Specification scope");
    println!("NETWORK-SPEC-SCOPE {}",serde_json::json!({"rules":spec.rules,"missing":spec.missing,"notes":spec.notes}));
    // R21's input is (K_hist + side_max) whole snapshots (`admission::historical_bytes`: pocold's whole-state
    // mode keeps full states). In store mode the retained versions share nodes in state.redb: what the K_hist
    // versions behind the live one add is at most K_hist block additions (resources::chain_versions_bytes).
    {
        use pocol_lp4_execution::node_trie::version_bytes;
        let (ka,ks)=pocold::resources::state_keys(&c).unwrap();let live=version_bytes(ka,ks).unwrap();
        let window=pocold::resources::chain_versions_bytes(&c,u128::from(local.k_hist)+1).unwrap();
        println!("NETWORK-R21-STORE {}",serde_json::json!({"wholeStateInput":hist.to_string(),"liveVersionBytes":live.to_string(),
            "windowBytes":window.to_string(),"historicalAdditionsBytes":(window-live).to_string(),"histTrieBudget":admission::HIST_TRIE_BUDGET,
            "additionsFit":window-live<=u128::from(admission::HIST_TRIE_BUDGET)}));
    }
    // pocold's A13 check (resources::check at open), both modes.
    for store in [false,true]{
        let lim=pocold::node::Limits{state_store:store,..Default::default()};
        let b=pocold::resources::derive(&c,local.k_hist,local.side_max,&lim,2).unwrap();let need=pocold::resources::budget_required(b.total).unwrap();
        let r=pocold::resources::check(&c,local.k_hist,local.side_max,&lim,2);
        println!("NETWORK-A13 {}",serde_json::json!({"stateStore":store,"modelBytes":b.total.to_string(),"budgetRequiredBytes":need.to_string(),"budgetBytes":lim.mem_budget,"passes":r.is_ok()}));
        assert_eq!(r.is_ok(),need<=u128::from(lim.mem_budget));
    }
}
/// D-3 O2 (2026-10-28; a fact the decision table lacked): the network store-mode A13 model at the caps O2
/// proposes (TX_CAP 5·10^7, TXBYTES_CAP 24·2^30, LOG_CAP 2^32) beside today's (10^7, 2^33, 2^30). TX_CAP and
/// TXBYTES_CAP are CP fields and enter `derive` through the profile. LOG_CAP is a build constant, so its share
/// is added from the formula the history term uses (`resources.rs`: LOG_CAP·33/32 of history.redb payload, its
/// allocator at HISTORY_FILE_FACTOR / 4096 bytes per byte; the resident window's logs are bounded by the window,
/// not by LOG_CAP). Prints; asserts only that both are computed. Nothing here adopts O2.
#[test] fn network_store_model_at_the_o2_caps(){
    let local=pocold_local();let lim=pocold::node::Limits{state_store:true,..Default::default()};
    let model=|c:&DevnetConfig|{let d=devnet::build(c).expect("network-profile candidate");pocold::resources::derive(d.profile.cp(),local.k_hist,local.side_max,&lim,2).unwrap().total};
    let o1=model(&network());let mut o2c=network();o2c.tx_cap=50_000_000;o2c.txbytes_cap=24<<30;let o2_cp=model(&o2c);
    let log_delta=((1u128<<32)-(1u128<<30))/32*33*pocold::resources::HISTORY_FILE_FACTOR/4096;
    let o2=o2_cp+log_delta;let gib=|x:u128|x as f64/(1u128<<30) as f64;
    let need=|x:u128|pocold::resources::budget_required(x).unwrap();
    println!("NETWORK-A13-O2 {}",serde_json::json!({"o1ModelBytes":o1.to_string(),"o1GiB":gib(o1),"o1RequiredGiB":gib(need(o1)),
        "o2CpCapsModelBytes":o2_cp.to_string(),"o2LogCapDeltaBytes":log_delta.to_string(),"o2ModelBytes":o2.to_string(),"o2GiB":gib(o2),"o2RequiredGiB":gib(need(o2)),
        "budgetGiB":gib(u128::from(lim.mem_budget)),"o2FitsBudget":need(o2)<=u128::from(lim.mem_budget)}));
    assert!(o2>=o1);
}
fn pocold_local()->pocol_lp4_execution::admission::NodeLocal{pocol_lp4_execution::admission::NodeLocal{delta:5,phi:5,t_exec:3,delta_rel:26,gamma:30,k_hist:32,retention_stress:false,..pocold::single_host_local()}}

/// PR2a (plan §9): peer discovery's four queue terms are counted only with discovery on, each at its formula: the
/// book's BOOK_MAX entries reserved once; the links table at its churn high-water (the live peers and one stale link
/// per Closed still queued); the transient scratch; the file's read and save buffers. Off, no term is counted and the
/// total is the on-total less exactly their sum. Local mode changes nothing.
#[test] fn a13_discovery_terms_are_counted_only_when_enabled(){
    use pocold::{addrbook::{self as ab,BOOK_MAX,CACHE_MAX,SELF_MAX,ANCHORS_MAX,FILE_LINE_MAX},node::{self,Discovery,Limits},net,resources::derive};use std::mem::size_of;
    let cp=devnet::build(&DevnetConfig::new(777910,1_700_000_000,&["a","b","c"])).unwrap().profile.cp().clone();let l=pocold::single_host_local();
    let disc=|b:&pocold::resources::Bound|b.items.iter().filter(|x|x.0.starts_with("queues/discovery")).cloned().collect::<Vec<_>>();
    let link=((node::LINK_BYTES as u128+1)*32).div_ceil(7);
    for (peers_max,dials) in [(net::Limits::default().peers_max,2usize),(16,5),(1,0)]{
        let at=|d:Discovery|derive(&cp,32,l.side_max,&Limits{net:net::Limits{peers_max,..Default::default()},discovery:d,..Default::default()},dials).unwrap();
        let (on,off,local)=(at(Discovery::default()),at(Discovery{enabled:false,..Default::default()}),at(Discovery{allow_local:true,..Default::default()}));
        assert!(disc(&off).is_empty(),"off: {:?}",disc(&off));
        let high=(net::Limits{peers_max,..Default::default()}.inbound_max()+dials+node::EVENT_QUEUE+(1+dials)*(net::NET_EVENT_QUEUE+1)) as u128;
        let want=[("queues/discovery: address book (BOOK_MAX, reserved once)",BOOK_MAX as u128*size_of::<ab::Entry>() as u128+512),
            ("queues/discovery: per-connection links (churn)",high*2*link+512),
            ("queues/discovery: selection, answer and self scratch",(2*BOOK_MAX*(size_of::<ab::Source>()+size_of::<ab::Key>()+8)+2*CACHE_MAX*size_of::<std::net::SocketAddr>()+(SELF_MAX+ANCHORS_MAX+7)*64+1024) as u128),
            ("queues/discovery: address-book file buffers",(pocold::launcher::MAX_INVITATION+1+FILE_LINE_MAX*(BOOK_MAX+ANCHORS_MAX+3)) as u128)];
        assert_eq!(disc(&on),want,"peers_max {peers_max}, dials {dials}");
        let sum:u128=want.iter().map(|x|x.1).sum();
        assert_eq!((on.total-sum,on.queues-sum),(off.total,off.queues),"off is on less exactly the discovery terms");
        assert_eq!(local,on,"local mode costs nothing more");
    }
    assert_eq!(BOOK_MAX,1024);
    if cfg!(all(target_os="linux",target_arch="x86_64")){assert_eq!((size_of::<ab::Entry>(),node::LINK_BYTES),(152,128),"the type sizes the pinned total was derived with");}
}

/// R1 (PR3): the live continuous chain (deploy/mainnet/h15000, T_blk 10 s) has no finite lifetime, so A13 projects
/// from its journal and the node stops once the journal holds more records than the budget admits (node.rs re-runs
/// `resources::check` every 256 records). The largest admitted record count R, found by binary search for each
/// (peers_max, dials), as the seeds boot: store mode with checkpoints every 128 records, discovery on, node_operator.py's
/// LOCAL_ARGS. The bundle's files are written out and loaded with `load_spec_for` (`Invitation::parse` refuses another
/// build's digest). Pinned for the target the evidence is produced on, so a term that moves the halt is seen.
#[test] fn a13_live_chain_halt_records(){
    use pocold::{net,node::Limits};
    let bundle:serde_json::Value=serde_json::from_slice(&std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../deploy/mainnet/h15000/network.pocol.json")).unwrap()).unwrap();
    let dir=std::env::temp_dir().join(format!("pocold-live-halt-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);std::fs::create_dir_all(&dir).unwrap();
    for (name,text) in bundle["files"].as_object().unwrap(){std::fs::write(dir.join(name),text.as_str().unwrap()).unwrap();}
    let local=pocol_lp4_execution::admission::NodeLocal{delta:1,phi:1,t_exec:1,delta_rel:8,gamma:5,..pocold::single_host_local()};
    let spec=pocold::load_spec_for(&dir,&local,None).expect("the live bundle loads");let _=std::fs::remove_dir_all(&dir);
    let (cp,k,side)=(spec.cp(),spec.local().k_hist,spec.local().side_max);assert_eq!(cp.H_END,0,"the live chain is continuous");
    let passes=|peers_max:usize,dials:usize,r:u64|pocold::resources::check(cp,k,side,&Limits{state_store:true,checkpoint_every:128,continuous_history:Some(r),
        net:net::Limits{peers_max,..Default::default()},..Default::default()},dials).is_ok();
    let halt=|peers_max:usize,dials:usize|->u64{
        assert!(passes(peers_max,dials,0),"an empty journal boots at peers_max {peers_max}, dials {dials}");
        let mut hi=1u64;while passes(peers_max,dials,hi){hi*=2;assert!(hi<1<<40,"no halt below 2^40 records");}
        let mut lo=hi/2;while hi-lo>1{let mid=lo+(hi-lo)/2;if passes(peers_max,dials,mid){lo=mid}else{hi=mid}}lo
    };
    let mut got=Vec::new();
    for (p,d) in [(8usize,2usize),(8,7),(32,2),(32,7),(64,2),(64,7)]{let r=halt(p,d);got.push(r);
        println!("A13-LIVE-HALT {}",serde_json::json!({"peersMax":p,"dials":d,"records":r,"days":r as f64/8640.0}));}
    // At main b83b584: 1,170,090 and 1,145,444 records (135.4 and 132.6 days). Main e3d6f07, PR4's post-state verdicts
    // (239,520 B): 1,169,988 and 1,145,342 (-102, -102). PR3 R10, the node's per-connection maps, on that main: 1,169,713
    // and 1,144,871 (-275, -471). PR3 inbound eviction, every physical inbound place counted (8 established and 8
    // waiting for a Hello) and the eviction registry: 1,158,957 and 1,134,336 (-10,756, -10,535).
    // PR3 commit 5, the default of 32 established (40 places): 1,127,165 at 2 dials and 1,102,318 at 7 (130.5 and 127.6
    // days; -42,548 and -42,553 against R10's (8, d) halts, within the week of records, 60,480, the plan's rule allows).
    // PR6, the pool's meta, the parked entries, the template selection's candidates and the block overlay's BLOCKHASH
    // window: 1,157,187 and 1,132,366 at 8 established (-1,770, -1,970), 1,125,239 and 1,100,548 at 32 (-1,926, -1,770),
    // 1,082,629 and 1,058,008 at 64 (-1,996, -1,770); the seed rule's outcome is unchanged. PR6 review, the selection's
    // tail after a gas failure (GAS_LIMIT/TX_GAS_MIN more candidate entries and BODY_MAX more raw bytes): 1,157,024 and
    // 1,132,366 at 8 (-163, 0), 1,125,232 and 1,100,385 at 32 (-7, -163), 1,082,476 and 1,057,845 at 64 (-153, -163).
    // A seed at 64 established: 1,084,625 at 2 dials and 1,059,778 at 7 (125.5 and 122.7 days); R(64, 7) is 85,093
    // records (9.8 days) below R10's R(8, 7), more than a week, so by the plan's seed rule the seeds stay at the default 32.
    if cfg!(all(target_os="linux",target_arch="x86_64")){
        assert_eq!(got,[1_157_024,1_132_366,1_125_232,1_100_385,1_082_476,1_057_845],"the live chain's halt moved (record it in the PR)");
        // The seed rule (plan §4, commit 5), against R10's halts at (8, 2) and (8, 7): seeds ship at 64 only while R(64, 7)
        // stays within a week of records of R(8, 7); the default of 32 only while R(32, 2) stays within a week of R(8, 2).
        const WEEK:u64=7*8640;const R10:[u64;2]=[1_169_713,1_144_871];
        assert!(R10[0]-got[2]<=WEEK,"the default of 32 moves the halt by more than a week: ask the owner");
        assert!(R10[1]-got[5]>WEEK,"a seed at 64 now moves the halt by at most a week: the seeds may take POCOL_PEERS_MAX=64 (deploy/public-testnet/prepare.py)");
    }
}

/// PR3: A13 counts every physical inbound place, S = inbound_max = peers_max + handshaking_max (the established ones and
/// those waiting for a Hello; an evicted peer keeps its place until its reader releases it), in each per-place term: the
/// send queues and frames, the group entries, the connection high-water (`high`, with the node's maps at 2,718 B an
/// entry and each connection's activity record), the I/O threads and the exiting ones; and the eviction registry (a
/// slot and a selection candidate per place, reserved once, and the ring). At the defaults one more established place
/// costs 3,424,474 B of model.
#[test] fn a13_inbound_terms_count_every_inbound_place_and_the_eviction_registry(){
    use pocold::{net,node::{self,Limits},resources::derive,threads::IO_STACK};
    let cp=devnet::build(&DevnetConfig::new(777910,1_700_000_000,&["a","b","c"])).unwrap().profile.cp().clone();let l=pocold::single_host_local();
    let item=|b:&pocold::resources::Bound,k:&str|b.items.iter().find(|x|x.0.starts_with(k)).unwrap_or_else(||panic!("{k}")).1;
    let at=|peers_max:usize,handshaking_max:usize,dials:usize|derive(&cp,32,l.side_max,&Limits{net:net::Limits{peers_max,handshaking_max,..Default::default()},..Default::default()},dials).unwrap();
    let conn_max=Limits::default().rpc.conn_max as u128;assert_eq!(conn_max,32,"the RPC connections the io and exiting terms were derived with");
    let (q,f)=(net::Limits::default().queue_bytes_max as u128,net::MAX_FRAME as u128);
    for (p,h,d) in [(8usize,8usize,2usize),(32,8,2),(64,8,7),(1,1,0)]{
        let b=at(p,h,d);let (s,d)=((p+h) as u128,d as u128);
        let high=s+d+node::EVENT_QUEUE as u128+(1+d)*(net::NET_EVENT_QUEUE as u128+1);
        assert_eq!(item(&b,"queues/inbound eviction"),s*192+high*64+3072,"the registry at ({p}, {h}, {d})");
        assert_eq!(item(&b,"queues/node peer and status maps"),high*2718+1024,"the maps at ({p}, {h}, {d})");
        assert_eq!(item(&b,"queues/peer send queues and frames"),(s+d)*(q+2*f+4096),"the queues at ({p}, {h}, {d})");
        assert_eq!(item(&b,"threads/I/O thread stacks and heap"),(2*s+4*d+conn_max+6)*(IO_STACK as u128+4096),"the io threads at ({p}, {h}, {d})");
        assert_eq!(item(&b,"threads/exiting not-yet-joined"),(s+d+conn_max)*IO_STACK as u128,"the exiting threads at ({p}, {h}, {d})");
    }
    let d=net::Limits::default();assert_eq!(at(d.peers_max+1,d.handshaking_max,2).total-at(d.peers_max,d.handshaking_max,2).total,3_424_474,"one more established place");
    assert_eq!((net::SLOT_BYTES,net::CAND_BYTES,net::ACTIVITY_BYTES,net::EVICT_RING,net::EVICT_ENTRY_BYTES),(96,96,64,64,32),"the constants the pinned totals were derived with");
}
/// PR3 A2: the devnet profile pocold boots passes the A13 gate at a seed's settings (64 established, 8 waiting for a
/// Hello, 7 configured peers) and at the largest established count (128), in both modes.
#[test] fn a13_admits_seed_inbound_settings_at_the_devnet_profile(){
    let cp=devnet::build(&DevnetConfig::new(777910,1_700_000_000,&["a","b","c"])).unwrap().profile.cp().clone();let l=pocold::single_host_local();
    for peers_max in [64usize,128]{for state_store in [false,true]{
        let lim=pocold::node::Limits{state_store,net:pocold::net::Limits{peers_max,handshaking_max:8,..Default::default()},..Default::default()};
        let b=pocold::resources::check(&cp,l.k_hist,l.side_max,&lim,7).unwrap_or_else(|e|panic!("peers_max {peers_max}, store {state_store}: {e}"));
        println!("A13-DEVNET-INBOUND {}",serde_json::json!({"peersMax":peers_max,"stateStore":state_store,"total":b.total.to_string()}));
    }}
}
