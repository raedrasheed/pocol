//! FN-3c (C6, node-local evidence; development plan FN-3 §5.13 and §5.14; FRESH-NETWORK-DESIGN.md risk 1, C6, M1): the
//! node at the fresh network's M_max of 10,000 members.
//! - A13 in store mode (eviction on, the files observed) at the fresh-network CP admits the node within the 6 GiB budget
//!   with its 1/8 reserve, where the pre-FN-3 per-block nonce ranges (64·M_max an entry-equivalent) alone exceeded the
//!   usable 5.25 GiB.
//! - A host-level genesis of 10,000 identities (tests/support/ten_thousand.rs) registers every one.
//! - `#[ignore]`: the growth and CPU harness. A store-mode node at 10,000 members imports about 600 blocks (two member
//!   snapshots, idle and under membership churn) and prints history.redb's growth attributable to membership, child_ctx,
//!   h_full's member record and reconstruction timings with the oracle off, the member counters of the import phase
//!   apart from the reconstruction loop's, the proposer's full evidence pool (FN-3 §5.12) at the head, and the process
//!   RSS against A13 (the three runs share one process, so each run's peak includes what earlier runs left resident; the
//!   RSS at the node's open is printed with it). FN-1b: the cold child_ctx is a new parent's (FN-1's rows cleared and the
//!   parent's own membership inputs forgotten, so they are derived from its parent's and the block's writes, which is
//!   asserted by count), and the same context with the inputs read whole (FN-1b's fallback) is timed too. Measurements
//!   are printed, never asserted;
//!   the one bound asserted is C6's (the membership part of history.redb below 100 MB a day), which is generous and
//!   deterministic (it counts bytes, not time). Run it in release on Linux:
//!     cargo +1.99.1 test --release --locked --manifest-path prototypes/pocold/Cargo.toml --test fn3_scale -- --ignored --nocapture --test-threads=1
#[path="support/ten_thousand.rs"] mod ten_thousand;
use std::collections::{HashMap,HashSet};
use std::path::{Path,PathBuf};
use std::time::Instant;
use pocol_lp4_execution::profile::ConsensusParams;
use pocold::node::{Config,Limits,Node};
use pocold::resources::{self,DiskBytes};
use pocold::members::{self,Set};
use pocold::types::*;
use pocol_lp4_execution::txsign::Key;
use revm::primitives::{Address,U256};

const GIB:u128=1<<30;
/// The model's usable share of the 6 GiB budget (7/8, resources::RESERVE_KEEP_EIGHTHS).
fn usable(l:&Limits)->u128{u128::from(l.mem_budget)*resources::RESERVE_KEEP_EIGHTHS/8}
/// Store mode, eviction on (the default depth), the files observed (`Limits::disk`, empty files: a new node).
fn store_limits()->Limits{Limits{state_store:true,disk:Some(DiskBytes::default()),..Default::default()}}
/// The canonical and side entry-equivalents that held the nonce ranges in the model before FN-3b (FN-3 plan §0, §4): the
/// resident window (K_hist), the offloaded canonical metadata (slim_depth = 2·R_keep + E_win + 257 less the window; the
/// slimmed entries below it held none), the shadow max(257, E_win + 1), the side metadata (side_cap + 2·R_keep), one full
/// side block and two views.
fn entry_equivalents(cp:&ConsensusParams,k_hist:u32,l:&Limits)->u128{
    let e=resources::evict_resolved(cp,k_hist,l).expect("eviction on");
    let window=u128::from(k_hist);let depth=u128::from(2*l.r_keep+u64::from(cp.E_win)+257).min(u128::from(e)+2);
    window+(depth-window)+u128::from(cp.E_win.max(256)+1)+l.side_cap as u128+2*u128::from(l.r_keep)+1+2
}
fn gib(x:u128)->f64{x as f64/GIB as f64}
fn show(tag:&str,cp:&ConsensusParams,b:&resources::Bound,l:&Limits){
    let members:Vec<_>=b.items.iter().filter(|(n,_)|n.contains("FN-3")||n.contains("FN-1")).map(|(n,v)|serde_json::json!([n,v.to_string()])).collect();
    let large:Vec<_>=b.items.iter().filter(|(_,v)|*v>=100_000_000).map(|(n,v)|serde_json::json!([n,v.to_string()])).collect();
    println!("FN3C-A13 {tag} {}",serde_json::json!({"M_max":cp.M_max,"E_win":cp.E_win,"D":cp.D,"E_max":cp.E_max,"R_react":cp.R_react,"REG_OPS":cp.REG_OPS,
        "REG_RECORDS_MAX":cp.REG_RECORDS_MAX,"SYS_SLOT_MAX":cp.SYS_SLOT_MAX,"total":b.total.to_string(),"totalGiB":gib(b.total),"usable":usable(l).to_string(),
        "margin":(usable(l)-b.total).to_string(),"budgetRequired":resources::budget_required(b.total).unwrap().to_string(),
        "groups":{"store":b.store.to_string(),"history":b.history.to_string(),"side":b.side_blocks.to_string(),"queues":b.queues.to_string(),"execution":b.execution.to_string(),"code":b.code.to_string()},
        "memberAndContextTerms":members,"termsOver100MB":large}));
}

/// The CP FN-3a and FN-3b recorded A13 at (A13-DERIVATION.md FN-3a, FN-3b): the CP of the continuous network init-testnet
/// generated from `"continuous": true` before phase 6a (the builder's defaults: CP schema v1, nonceMode 0, T_blk 10, tau
/// 600, D_att 60, dFbWait 6, R_max 1, devnet caps) with the C6 row's fields set: M_max 10,000, E_win 360, D 4, E_max 4,
/// R_react 4, REG_OPS 16, REG_RECORDS_MAX 24,576, SYS_SLOT_MAX 147,456. Built by hand (and, since phase 6a, encoded here
/// from the devnet builder's values, because no builder makes a spec-1 genesis any more): not a ParamGate-admitted CP
/// (A13 reads the values only), at the local timing (1, 1, 1, 8, 5) that network booted with.
fn c6_row_cp()->(ConsensusParams,u32,u32){
    let mut c=pocol_lp4_execution::devnet::DevnetConfig::new(777917,1_800_000_000,&[]);
    c.spec_version=1;c.nonce_mode=0;c.r_max=1;c.t_blk=10;c.tau=600;c.d_att=60;c.d_fb_wait=6;c.hash_rate=(1<<18)/10;
    c.b_reg=U256::from(100u128*10u128.pow(18));c.subsidy=U256::from(2u128*10u128.pow(18));
    let mut cp=ConsensusParams::from_values(&c.cp()).expect("a CP schema v1 encoding");assert_eq!((cp.schema_version(),cp.H_END,cp.nonceMode),(1,0,0));
    let local=pocol_lp4_execution::admission::NodeLocal{delta:1,phi:1,t_exec:1,delta_rel:8,gamma:5,..pocold::single_host_local()};
    cp.M_max=10_000;cp.E_win=360;cp.D=4;cp.E_max=4;cp.R_react=4;cp.REG_OPS=16;cp.REG_RECORDS_MAX=24_576;cp.SYS_SLOT_MAX=147_456;
    (cp,local.k_hist,local.side_max)
}
/// The fresh-network CP of FRESH-NETWORK-DESIGN.md §1a (`ten_thousand::fresh_config`: every A13 input at the network's
/// value, CODE_CAP 2^30, SLOT_CAP 4,000,000 and ACCT_CAP 1,000,000 among them), from the devnet builder with one genesis
/// member (|M_0| enters no A13 term), at the single-host local profile.
fn fresh_cp()->(ConsensusParams,u32,u32){
    let d=pocol_lp4_execution::devnet::build(&ten_thousand::fresh_config(1,16)).expect("the fresh-network candidate");
    let l=pocold::single_host_local();(d.profile.cp().clone(),l.k_hist,l.side_max)
}

/// FN-3 §5.13: A13 in store mode with eviction on and the files observed admits a node at M_max 10,000, at both CPs, and
/// the pre-FN-3 ranges term alone (64·M_max over every entry-equivalent) exceeds the usable 5.25 GiB. The totals are
/// pinned on Linux x86-64 (the Entry's size is the platform's).
#[test] fn a13_admits_the_fresh_network_cp_at_ten_thousand_members(){
    let l=store_limits();
    // The C6-row CP keeps the continuous builder's CP schema v1 and bounded nonces (so its member terms take the
    // every-block-a-snapshot branch); the fresh-network CP is schema v2 with the full nonce domain (the cadence branch).
    for (tag,(cp,k,side),pin,modes) in [("c6-row",c6_row_cp(),2_917_910_217u128,(0,1)),("fresh-1a",fresh_cp(),5_485_767_089u128,(1,2))]{
        assert_eq!((cp.M_max,cp.E_win,cp.D,cp.E_max,cp.R_react,cp.REG_OPS,cp.REG_RECORDS_MAX,cp.SYS_SLOT_MAX,cp.H_END,cp.nonceMode,cp.schema_version()),
            (10_000,360,4,4,4,16,24_576,147_456,0,modes.0,modes.1),"{tag}: the C6 row's values");
        assert_eq!(pocold::members::all_live_everywhere(&cp),modes.1==2,"{tag}");
        let b=resources::check(&cp,k,side,&l,2).unwrap_or_else(|e|panic!("{tag}: A13 admits the node: {e}"));
        show(tag,&cp,&b,&l);
        assert!(resources::budget_required(b.total).unwrap()<=u128::from(l.mem_budget)&&b.total<usable(&l),"{tag}: within the budget with its reserve");
        // The member terms are all present and small: none is M_max per block any more.
        for name in ["member records on entries not offloaded","member sets cache","recomputed assignments cache","member reconstruction scratch","child-context cache","membership inputs and write sets"]{
            let v=b.items.iter().find(|(n,_)|n.contains(name)).unwrap_or_else(||panic!("{tag}: the term {name}")).1;
            assert!(v<64<<20,"{tag}: {name} is {v} B");
        }
        // The sibling: the ranges term FN-3b removed, 64·M_max over every entry-equivalent, alone exceeds the usable share.
        // Today's total plus that term is a lower bound on FN-3a's model (after FN-3a, before FN-3b): it leaves out the
        // ranges rows' allocator share and the Entry's 16 B. It is not a pre-FN-3 figure, since today's total holds the
        // member terms FN-3a added; less those five terms it estimates the pre-FN-3 model (it still leaves out the
        // ranges rows' allocator share and the Entry's 16 B, and keeps the member payload's allocator share), which
        // A13-DERIVATION FN-3a recorded at the C6-row CP as 10,035,259,610 B.
        let eq=entry_equivalents(&cp,k,&l);let ranges=64*u128::from(cp.M_max)*eq;
        let fn3_terms:u128=b.items.iter().filter(|(n,_)|n.contains("(FN-3:")).map(|(_,v)|*v).sum();
        assert_eq!(b.items.iter().filter(|(n,_)|n.contains("(FN-3:")).count(),5,"{tag}: FN-3's five member terms (records, sets, assignments, owner memo, scratch)");
        println!("FN3C-A13-PRE {tag} {}",serde_json::json!({"entryEquivalents":eq.to_string(),"rangesTerm":ranges.to_string(),"rangesGiB":gib(ranges),
            "fn3aModelAtLeast":(b.total+ranges).to_string(),"fn3aModelGiB":gib(b.total+ranges),"fn3MemberTerms":fn3_terms.to_string(),
            "preFn3Estimate":(b.total-fn3_terms+ranges).to_string(),"preFn3EstimateGiB":gib(b.total-fn3_terms+ranges)}));
        assert_eq!(eq,11_221,"{tag}: the entry-equivalents of FN-3 plan §0");
        assert!(ranges>usable(&l),"{tag}: the pre-FN-3 ranges term alone exceeds 5.25 GiB");
        if cfg!(all(target_os="linux",target_arch="x86_64")){assert_eq!(b.total,pin,"{tag}: the A13 total (re-pin with the recorded delta)");}
    }
}
/// The fresh-network CP's headroom against the observed files (printed): A13 counts redb's allocator over each observed
/// file, so the bound rises as history.redb and state.redb grow. The largest state.redb (history.redb empty) and
/// history.redb (state.redb empty) at which `check` still admits, found by bisection to 1 MiB. Never asserted.
#[test] fn a13_fresh_network_headroom_against_the_observed_files(){
    let (cp,k,side)=fresh_cp();
    let fits=|d:DiskBytes|resources::check(&cp,k,side,&Limits{state_store:true,disk:Some(d),..Default::default()},2).is_ok();
    assert!(fits(DiskBytes::default()),"positive control: an empty node is admitted");
    let largest=|f:&dyn Fn(u64)->DiskBytes|{let (mut lo,mut hi)=(0u64,1u64<<50);if fits(f(hi)){return hi}while hi-lo>1<<20{let m=lo+(hi-lo)/2;if fits(f(m)){lo=m}else{hi=m}}lo};
    let state=largest(&|x|DiskBytes{state:x,..Default::default()});let history=largest(&|x|DiskBytes{history:x,..Default::default()});
    println!("FN3C-A13-HEADROOM {}",serde_json::json!({"stateRedbMaxBytes":state.to_string(),"stateRedbMaxGiB":state as f64/(1u64<<30) as f64,
        "historyRedbMaxBytes":history.to_string(),"historyRedbMaxGiB":history as f64/(1u64<<30) as f64}));
}

/// FN-3 §5.14 harness support: the 10,000-identity genesis registers every member (its ActList holds 10,000 ids, each
/// with the key the helper derived), at the fresh-network CP.
#[test] fn ten_thousand_member_genesis_registers_every_identity(){
    let t=Instant::now();let g=ten_thousand::genesis(&ten_thousand::fresh_config(10_000,16));let built=t.elapsed();
    let act=ten_thousand::genesis_active(&g);
    assert_eq!(act.len(),10_000,"the genesis ActList holds every identity");
    let mut sorted=act.clone();sorted.sort();sorted.dedup();assert_eq!(sorted,g.ids,"the ActList is the M_0 list (distinct ids)");
    assert!(g.ids.iter().all(|id|g.keys.contains_key(id)),"the helper holds every member's key");
    assert_eq!(g.candidate.profile.cp().M_max,10_000);
    println!("FN3C-GENESIS {}",serde_json::json!({"members":act.len(),"buildMs":built.as_millis().to_string(),"specBytes":g.candidate.spec_bytes.len()}));
}

// ---------------- the growth and CPU harness (#[ignore]) ----------------
fn dir(label:&str)->PathBuf{let d=std::env::temp_dir().join(format!("pocold-fn3c-{label}-{}",std::process::id()));let _=std::fs::remove_dir_all(&d);d}
fn conf(d:&Path,limits:Limits)->Config{Config{name:"fn3c".into(),dir:d.to_path_buf(),listen:None,peers:vec![],miner:None,stop_at:None,log:false,rpc:None,limits}}
fn ms(t:Instant)->f64{t.elapsed().as_secs_f64()*1000.0}
fn pct(v:&[f64],p:usize)->f64{if v.is_empty(){return 0.0}let mut s=v.to_vec();s.sort_by(|a,b|a.partial_cmp(b).unwrap());s[(s.len()*p/100).min(s.len()-1)]}
fn stats(v:&[f64])->serde_json::Value{serde_json::json!({"n":v.len(),"p50":pct(v,50),"p99":pct(v,99),"max":pct(v,100)})}
fn proc_kb(field:&str)->Option<u64>{std::fs::read_to_string("/proc/self/status").ok()?.lines().find(|l|l.starts_with(field))?.split_whitespace().nth(1)?.parse().ok()}
/// The bytes of each table of a closed history.redb (keys and values) and redb's own counts.
fn table_bytes(path:&Path)->serde_json::Value{
    use redb::{ReadableTable,ReadableTableMetadata,TableDefinition};
    let db=redb::Database::open(path).expect("history.redb opens");let r=db.begin_read().unwrap();
    let sum32=|name:&str|->(u64,u64){let t:TableDefinition<&[u8;32],&[u8]>=TableDefinition::new(name);
        match r.open_table(t){Ok(t)=>t.iter().unwrap().map(|x|{let (_,v)=x.unwrap();(1u64,32+v.value().len() as u64)}).fold((0,0),|a,b|(a.0+b.0,a.1+b.1)),Err(_)=>(0,0)}};
    let txidx:(u64,u64)={let t:TableDefinition<&[u8;64],u32>=TableDefinition::new("txidx");match r.open_table(t){Ok(t)=>(t.len().unwrap(),t.len().unwrap()*68),Err(_)=>(0,0)}};
    let (m,s,rc)=(sum32("members"),sum32("member_sets"),sum32("receipts"));
    drop(r);let w=db.begin_write().unwrap();let st=w.stats().unwrap();
    let out=serde_json::json!({"members":{"rows":m.0,"bytes":m.1},"memberSets":{"rows":s.0,"bytes":s.1},"receipts":{"rows":rc.0,"bytes":rc.1},"txidx":{"rows":txidx.0,"bytes":txidx.1},
        "redb":{"storedBytes":st.stored_bytes(),"metadataBytes":st.metadata_bytes(),"fragmentedBytes":st.fragmented_bytes()}});
    w.abort().unwrap();out
}
/// What one run of the harness measured.
struct Run{blocks:u64,history_file:Vec<(u64,u64)>,tables:serde_json::Value,deltas:Vec<usize>,snapshots:Vec<u64>,ctx_cold:Vec<f64>,ctx_warm:Vec<f64>,ctx_read:Vec<f64>,membership:Vec<f64>,validate:Vec<f64>,import:Vec<f64>,
    record:Vec<f64>,record_snap:Vec<f64>,encode:Vec<f64>,recon_cold:Vec<f64>,recon_warm:Vec<f64>,counters_import:serde_json::Value,counters_recon:serde_json::Value,counters:String,
    pool:serde_json::Value,evidence:Vec<usize>,members_end:usize,a13:u128,rss_open_kb:Option<u64>,hwm_kb:Option<u64>,rss_kb:Option<u64>}
/// The member counters that tell where member data came from, as `b` less `a` (a phase's own counts).
fn counter_delta(a:&pocold::chain::MemberCounters,b:&pocold::chain::MemberCounters)->serde_json::Value{
    serde_json::json!({"setHits":b.set_hits-a.set_hits,"setMisses":b.set_misses-a.set_misses,"diskSetReads":b.disk_set_reads-a.disk_set_reads,"recordReads":b.record_reads-a.record_reads,
        "assignHits":b.assign_hits-a.assign_hits,"assignMisses":b.assign_misses-a.assign_misses,"ownerHits":b.owner_hits-a.owner_hits,"reconstructions":b.reconstructions-a.reconstructions,
        "faults":b.faults-a.faults,"inputHits":b.base_hits-a.base_hits,"inputsDerived":b.base_derived-a.base_derived,"inputsRead":b.base_read-a.base_read,
        "derivedListReads":b.derive_list_reads-a.derive_list_reads,"derivedRecordReads":b.derive_record_reads-a.derive_record_reads})
}
/// A store-mode node (eviction on, the default window, the oracle off: `Chain::member_oracle`'s default) importing
/// `blocks` blocks of the genesis `g`. With `churn`, 64 blocks (201..=264, or 4..=67 on a run shorter than 300 blocks)
/// change the member set by as much as transactions and evidence can: 4 registrations (R_max), 12 deregistrations
/// (REG_OPS 16 in all) of members alice funded at blocks 1..=3, from the 33rd on 4 reactivations of those (R_react, after
/// the W_act cooldown) and up to 4 type-2 punishments (E_max) of earlier winners still active. Each block is
/// imported with the child-context cache cleared (as a node that did not build it), after its parent's child_ctx was
/// timed cold (the FN-1 cache cleared: FN-1b derives the parent's membership inputs from its own parent's and the block's
/// writes), warm, and with the inputs read whole (FN-1b's fallback: a restart, a checkpoint restore, a deep reorg). Without churn, the proposer's pool case of FN-3 §5.12 follows at the
/// head: 64 type-2 items against distinct ancestors, most of them offloaded below the window.
fn run(g:&ten_thousand::Genesis,spec:pocold::chain::Spec,blocks:u64,churn:bool,label:&str)->Run{
    let d=dir(label);let limits=Limits{state_store:true,..Default::default()};
    // The peak RSS from here (Linux: "5" to clear_refs resets VmHWM to the current RSS, not to zero), so the
    // admission's measurement's peak is not counted. The process RSS at this point (the genesis, what the admission and
    // earlier runs left resident) is recorded too: the run's own increase is its peak less that.
    let _=std::fs::write("/proc/self/clear_refs","5");let rss_open=proc_kb("VmRSS:");
    let mut node=Node::open(conf(&d,limits),spec).unwrap_or_else(|e|panic!("{label}: the node opens: {e}"));
    assert!(!node.chain.member_oracle,"{label}: the oracle is off (its default)");
    let keys=&g.keys;let alice=pocol_lp4_execution::devnet::account_key("alice").address();
    let mut nonces:HashMap<Address,u64>=HashMap::new();let mut next=|a:Address|{let n=nonces.entry(a).or_insert(0);*n+=1;*n-1};
    let churners:Vec<Address>=(0..256).map(|i|pocol_lp4_execution::devnet::account_key(&format!("churn-{i}")).address()).collect();
    // Deregistered at churn block j (genesis members, funded by alice at blocks 1..=3), reactivated at j + 32.
    let leavers:Vec<Address>=g.ids.iter().rev().take(12*64).copied().collect();
    let mut gone:HashSet<Address>=leavers.iter().copied().collect();
    let mut won:Vec<(Block,Address)>=Vec::new();let mut punished=0usize;
    // The churn's 64 blocks: from 201 (a backlog of earlier winners to punish 4 a block, and the snapshot at 257 among
    // them), or from 4 on a short run.
    let c0:u64=if blocks>=300{201}else{4};
    let mut r=Run{blocks,history_file:vec![],tables:serde_json::Value::Null,deltas:vec![],snapshots:vec![],ctx_cold:vec![],ctx_warm:vec![],ctx_read:vec![],membership:vec![],validate:vec![],import:vec![],
        record:vec![],record_snap:vec![],encode:vec![],recon_cold:vec![],recon_warm:vec![],counters_import:serde_json::Value::Null,counters_recon:serde_json::Value::Null,counters:String::new(),
        pool:serde_json::Value::Null,evidence:vec![],members_end:0,a13:0,rss_open_kb:rss_open,hwm_kb:None,rss_kb:None};
    r.history_file.push((0,DiskBytes::observe(&d).history));
    let m_open=node.chain.member_counters();
    // The parent's member set, for the record h_full builds (the previous block's S_b).
    let mut prev_set:Option<Set>=None;
    for h in 1..=blocks{
        let head=node.chain.head;let p=node.chain.full_entry(&head).unwrap();
        let m0=node.chain.member_counters();
        // The cold path of a new parent: FN-1's rows cleared and the parent's own inputs forgotten (the node may have
        // derived them after the import), its block's write set and its parent's inputs kept.
        node.chain.clear_ctx_cache();node.chain.forget_member_base(&p);let t=Instant::now();let ctx=node.chain.child_ctx(&p,0).unwrap();r.ctx_cold.push(ms(t));
        // FN-1b positive control (counts, not times): from the second block on, the cold context derived its parent's
        // membership inputs, reading none whole.
        let m1=node.chain.member_counters();
        if h>=2{assert_eq!((m1.base_derived-m0.base_derived,m1.base_read-m0.base_read),(1,0),"{label} h={h}: the cold context derives its parent's inputs");}
        let t=Instant::now();let _=node.chain.child_ctx(&p,0).unwrap();r.ctx_warm.push(ms(t));
        // FN-1b's fallback: the inputs read whole from the parent's state (kept again for the next block's derivation).
        node.chain.clear_ctx_cache();node.chain.clear_member_bases();let t=Instant::now();let again=node.chain.child_ctx(&p,0).unwrap();r.ctx_read.push(ms(t));
        assert_eq!(*again.membership,*ctx.membership,"{label} h={h}: the derived and the whole reading agree");
        // The part of a cold child_ctx that reads the parent's ActList and every record (FN-1's one pass, uncached).
        let t=Instant::now();let _=node.chain.membership_one_pass(&p,h).unwrap();r.membership.push(ms(t));
        // h_full's member record (FN-3 §1.4), built here as h_full builds it from the same inputs: S_b (the context's
        // membership), the parent's set and digest (h_full takes the set from the sets cache, a hit in steady state, and
        // the digest from the parent's held record; both are read before the timer). Then the row's encoding for
        // history.redb (the record; the set too at a snapshot). After the import, the record is checked to be the one
        // the chain built.
        let d_p=node.chain.member_rec_of(&p).unwrap().digest;
        let t=Instant::now();let s_b:Set=std::sync::Arc::from(ctx.membership.ids());
        let (my_rec,_)=members::record_for(h,prev_set.as_ref().map(|x|(&x[..],&d_p)),&s_b);let built=ms(t);
        let t=Instant::now();let row=members::encode_rec(&p.hash,h,&my_rec);let set_row=if my_rec.is_snapshot(){members::encode_set(&s_b).len()}else{0};r.encode.push(ms(t));
        assert!(row.len()+set_row>0);
        if my_rec.is_snapshot(){r.record_snap.push(built);}else{r.record.push(built);}
        let base=ctx.base_fee;let c=&node.chain;
        let mut txs=Vec::new();let mut evidence=Vec::new();
        if churn&&(1..=3).contains(&h){for who in leavers.iter().skip((h as usize-1)*256).take(256){txs.push(ten_thousand::transfer(c,keys,alice,next(alice),base,*who,U256::from(10u128.pow(18))));}}
        if churn&&(c0..c0+64).contains(&h){
            let j=(h-c0) as usize;
            for who in &leavers[j*12..j*12+12]{txs.push(ten_thousand::registry_call(c,keys,*who,next(*who),base,"deregister()",None));}
            for who in &churners[j*4..j*4+4]{txs.push(ten_thousand::registry_call(c,keys,*who,next(*who),base,"register(address)",Some(*who)));}
            if j>=32{for who in &leavers[(j-32)*12..(j-32)*12+4]{txs.push(ten_thousand::registry_call(c,keys,*who,next(*who),base,"reactivate()",None));}}
            while evidence.len()<4&&punished<won.len(){let (b,w)=&won[punished];punished+=1;if gone.insert(*w){evidence.push(ten_thousand::type2(keys,b,*w));}}
        }
        if churn&&(c0..c0+64).contains(&h){r.evidence.push(evidence.len());}
        let (b,winner)=ten_thousand::mine(c,keys,&head,Body{txs,evidence});
        // Validation alone (no insert, nothing written), then the import (validation, journal, state.redb, history.redb,
        // checkpoints), each with the child-context cache cleared.
        node.chain.clear_ctx_cache();let t=Instant::now();node.chain.validate(&b,None,0).unwrap_or_else(|e|panic!("{label} h={h}: validate: {e:?}"));r.validate.push(ms(t));
        node.chain.clear_ctx_cache();let t=Instant::now();
        assert!(node.import(b.clone(),None).unwrap_or_else(|e|panic!("{label} h={h}: import: {e:?}")),"{label} h={h}: the block is new");r.import.push(ms(t));
        assert_eq!(node.chain.head().height,h);
        let rec=node.chain.member_rec_of(node.chain.head()).unwrap();
        assert_eq!(rec,my_rec,"{label} h={h}: the harness's record is the one h_full built");prev_set=Some(s_b);
        if rec.is_snapshot(){r.snapshots.push(h);}else{r.deltas.push(rec.delta_len());}
        if h>=4{won.push((b,winner));}
        // The evidence window: a type-2 item reaches back at most E_win blocks.
        while punished<won.len()&&won[punished].0.header.h+u64::from(node.chain.spec.cp().E_win)<=h+1{punished+=1;}
        if h%32==0||h==blocks{r.history_file.push((h,DiskBytes::observe(&d).history));}
        if h%100==0{eprintln!("{label}: h={h} import p50 {:.1} ms ctx cold p50 {:.1} ms",pct(&r.import,50),pct(&r.ctx_cold,50));}
    }
    // The import phase's own member counters (validation, import, and in the churn run the type-2 evidence against
    // ancestors up to about 200 blocks deep), before the reconstruction loop below adds its own.
    let m_import=node.chain.member_counters();r.counters_import=counter_delta(&m_open,&m_import);
    // Reconstruction: every 5th height's assignment recomputed from its member records with every cache cleared (cold: a
    // walk to its snapshot, from history.redb below the window), then again (warm).
    for h in (1..=blocks).step_by(5){
        let e=node.chain.canonical_at(h).unwrap().clone();
        node.chain.clear_member_caches();let t=Instant::now();let a=node.chain.assignment_of(&e).unwrap();r.recon_cold.push(ms(t));
        let t=Instant::now();let b=node.chain.assignment_of(&e).unwrap();r.recon_warm.push(ms(t));assert_eq!(a.ranges(),b.ranges());
    }
    let m=node.chain.member_counters();r.counters=format!("{m:?}");r.counters_recon=counter_delta(&m_import,&m);
    // FN-3 §5.12 at this M: the proposer's pool full (EVIDENCE_POOL_MAX = 64 type-2 items against distinct ancestors,
    // every 5th height back from the head, so all but the newest few are offloaded), a signed template on the head
    // built with every member cache cleared (cold: each item's owner recomputed from the records) and again (warm: the
    // owner memo), against the same template with the pool empty. Idle runs only (the churn run punished its winners).
    let step=((blocks.saturating_sub(6))/64).min(5);
    if !churn&&step>=1{
        let pe=node.chain.full_entry(&node.chain.head).unwrap();let proposer=node.chain.child_ctx(&pe,0).unwrap().proposer;
        node.cfg.miner=Some(Key::from_secret(&keys[&proposer].secret()).unwrap());let clock=pe.ts+u64::from(node.chain.spec.cp().T_blk);
        let heights:Vec<u64>=(0..64).map(|j|blocks-2-step*j).collect();
        let items:Vec<Vec<u8>>=heights.iter().map(|hh|{let (b,w)=won.iter().find(|(b,_)|b.header.h==*hh).expect("a winner at that height");ten_thousand::type2(keys,b,*w)}).collect();
        let offloaded=heights.iter().filter(|hh|node.chain.canonical_at(**hh).unwrap().offloaded).count();
        let timed=|node:&mut Node|{let t=Instant::now();let x=node.build_template_unrecorded(&pe,Default::default(),clock).expect("a template");(ms(t),x)};
        node.evidence_pool.clear();node.chain.clear_member_caches();
        let (empty_cold,e1)=timed(&mut node);let (empty_warm,_)=timed(&mut node);assert!(e1.1.evidence.is_empty());
        node.evidence_pool=items;node.chain.clear_member_caches();let c0=node.chain.member_counters();
        let (pool_cold,first)=timed(&mut node);let c1=node.chain.member_counters();
        let (pool_warm,second)=timed(&mut node);let c2=node.chain.member_counters();
        assert_eq!(node.evidence_pool.len(),64,"{label}: every item stays pooled");
        assert!(!first.1.evidence.is_empty()&&first.1.evidence==second.1.evidence,"{label}: the same evidence both times");
        assert!(c2.owner_hits>c1.owner_hits,"{label}: the second template's owners come from the memo");
        r.pool=serde_json::json!({"items":64,"ancestorHeights":[heights[63],heights[0]],"offloadedAncestors":offloaded,"included":first.1.evidence.len(),
            "templateEmptyPoolMs":{"cold":empty_cold,"warm":empty_warm},"templateFullPoolMs":{"cold":pool_cold,"warm":pool_warm},
            "poolCostMs":{"cold":pool_cold-empty_cold,"warm":pool_warm-empty_warm},"countersCold":counter_delta(&c0,&c1),"countersWarm":counter_delta(&c1,&c2)});
        node.cfg.miner=None;node.evidence_pool.clear();
    }
    r.members_end=node.chain.membership(&node.chain.full_entry(&node.chain.head).unwrap(),blocks+1).unwrap().ids().len();
    let disk=DiskBytes::observe(&d);let spec=&node.chain.spec;
    r.a13=resources::derive(spec.cp(),spec.local().k_hist,spec.local().side_max,&Limits{state_store:true,disk:Some(disk),..Default::default()},2).map_or(0,|b|b.total);
    r.hwm_kb=proc_kb("VmHWM:");r.rss_kb=proc_kb("VmRSS:");
    drop(node);r.tables=table_bytes(&d.join(pocold::history::FILE));let _=std::fs::remove_dir_all(&d);
    r
}
fn report(label:&str,g:&ten_thousand::Genesis,r:&Run){
    let file=|h:u64|r.history_file.iter().find(|x|x.0==h).map(|x|x.1);
    let mut dist:std::collections::BTreeMap<usize,usize>=Default::default();for d in &r.deltas{*dist.entry(*d).or_default()+=1;}
    println!("FN3C-RUN {label} {}",serde_json::json!({"genesisMembers":g.ids.len(),"blocks":r.blocks,"membersAtEnd":r.members_end,"snapshotsAt":r.snapshots,
        "churnBlocksEvidence":{"total":r.evidence.iter().sum::<usize>(),"perBlockHistogram":r.evidence.iter().fold(std::collections::BTreeMap::<usize,usize>::new(),|mut m,n|{*m.entry(*n).or_default()+=1;m})},
        "deltaLenHistogram":dist,"historyFile":r.history_file,"historyFileEnd":file(r.blocks),"tables":r.tables,
        "childCtxColdMs":stats(&r.ctx_cold),"childCtxWarmMs":stats(&r.ctx_warm),"childCtxInputsReadMs":stats(&r.ctx_read),"membershipOnePassMs":stats(&r.membership),"validateMs":stats(&r.validate),"importMs":stats(&r.import),
        "hFullRecordMs":{"delta":stats(&r.record),"snapshot":stats(&r.record_snap)},"recordEncodeMs":stats(&r.encode),
        "reconstructionColdMs":stats(&r.recon_cold),"reconstructionWarmMs":stats(&r.recon_warm),"memberCountersImportPhase":r.counters_import,"memberCountersReconstructionLoop":r.counters_recon,"memberCounters":r.counters,
        "proposerPool":r.pool,"a13ModelAtEnd":r.a13.to_string(),
        "processRssAtOpenKiB":r.rss_open_kb,"processVmHwmKiB":r.hwm_kb,"processVmRssKiB":r.rss_kb,"peakAboveOpenKiB":r.hwm_kb.zip(r.rss_open_kb).map(|(a,b)|a.saturating_sub(b))}));
}
/// FN-3 §5.14 / design M1: history.redb's growth attributable to membership and the CPU of child_ctx and reconstruction
/// at 10,000 members, with the oracle off; a 4-member chain of the same CP and length is the baseline. Prints FN3C-RUN
/// (each run) and FN3C-M1 (the per-day projection at T_blk 10 s, 8,640 blocks a day). Asserts only C6's byte bound: the
/// membership tables of history.redb (records and snapshot sets, keys included) grow by less than 100 MB a day, idle
/// and under the churn the harness produces, projected from what was written.
#[test] #[ignore] fn growth_and_cpu_at_ten_thousand_members(){
    let blocks:u64=std::env::var("FN3C_BLOCKS").ok().and_then(|x|x.parse().ok()).unwrap_or(600);
    let mut cfg=ten_thousand::fresh_config(10_000,16);
    cfg.accounts=vec![("alice".into(),U256::from(10u128.pow(24)))];
    for i in 0..256{cfg.accounts.push((format!("churn-{i}"),U256::from(10u128.pow(21))));}
    let t=Instant::now();let g=ten_thousand::genesis(&cfg);let built=ms(t);
    let t=Instant::now();let (spec,timing)=ten_thousand::admit(&g);let admitted=ms(t);
    println!("FN3C-SETUP {}",serde_json::json!({"members":g.ids.len(),"buildMs":built,"admitMs":admitted,"timing":format!("{timing:?}")}));
    let churn=run(&g,spec,blocks,true,"churn");report("10k-churn",&g,&churn);
    let idle=run(&g,ten_thousand::readmit(&g,&timing),blocks,false,"idle");report("10k-idle",&g,&idle);
    let mut small=ten_thousand::fresh_config(4,16);small.accounts=cfg.accounts.clone();
    let gs=ten_thousand::genesis(&small);let (ss,_)=ten_thousand::admit(&gs);
    let base=run(&gs,ss,blocks,false,"base");report("4-idle",&gs,&base);
    // C6: the membership tables' bytes a block, projected to a day.
    let member_bytes=|r:&Run|r.tables["members"]["bytes"].as_u64().unwrap()+r.tables["memberSets"]["bytes"].as_u64().unwrap();
    let per_day=|r:&Run|member_bytes(r) as f64/r.blocks as f64*8640.0;
    let rec_bytes=|r:&Run|r.tables["members"]["bytes"].as_u64().unwrap() as f64/r.blocks as f64;
    let snap_rows=|r:&Run|r.tables["memberSets"]["rows"].as_u64().unwrap();
    let snap_bytes=|r:&Run|r.tables["memberSets"]["bytes"].as_u64().unwrap() as f64/snap_rows(r).max(1) as f64;
    let file_end=|r:&Run|r.history_file.last().unwrap().1;
    // The file's growth attributable to membership: the 10k idle chain less the 4-member chain of the same length.
    // redb preallocates the file, so over 600 blocks its length moves in steps (or not at all); redb's stored bytes do not.
    let attributable=file_end(&idle) as f64-file_end(&base) as f64;
    let stored=|r:&Run|r.tables["redb"]["storedBytes"].as_u64().unwrap() as f64;
    let stored_attributable=stored(&idle)-stored(&base);
    // The steady state a day: 8,640 records and 8,640/256 = 33.75 snapshots (a run of 600 blocks holds 3).
    let steady=|rec:f64,snap:f64|rec*8640.0+snap*8640.0/256.0;
    println!("FN3C-M1 {}",serde_json::json!({"blocksPerDay":8640,
        "recordBytesPerBlock":{"idle":rec_bytes(&idle),"churn":rec_bytes(&churn),"base4":rec_bytes(&base)},
        "snapshotRowBytes":{"idle":snap_bytes(&idle),"base4":snap_bytes(&base)},
        "membershipTablesPerDayBytes":{"idle":per_day(&idle),"churn":per_day(&churn),"base4":per_day(&base)},
        "historyFileEnd":{"idle":file_end(&idle),"churn":file_end(&churn),"base4":file_end(&base)},
        "fileGrowthAttributableToMembershipPerDayBytes":attributable/blocks as f64*8640.0,
        "redbStoredBytesAttributableToMembership":stored_attributable,"redbStoredAttributablePerDayBytes":stored_attributable/blocks as f64*8640.0,
        "steadyStatePerDayBytes":{"idle":steady(rec_bytes(&idle),snap_bytes(&idle)),"churn":steady(rec_bytes(&churn),snap_bytes(&idle)),
            "everyBlockAtDelta24":steady(144.0+20.0*24.0,snap_bytes(&idle))},
        "membershipOnePassP99Ms":{"idle":pct(&idle.membership,99),"churn":pct(&churn.membership,99)},
        "validateP99Ms":{"idle":pct(&idle.validate,99),"churn":pct(&churn.validate,99),"base4":pct(&base.validate,99)},
        "childCtxColdP99Ms":{"idle":pct(&idle.ctx_cold,99),"churn":pct(&churn.ctx_cold,99),"base4":pct(&base.ctx_cold,99)},
        "childCtxColdP50Ms":{"idle":pct(&idle.ctx_cold,50),"churn":pct(&churn.ctx_cold,50),"base4":pct(&base.ctx_cold,50)},
        "childCtxInputsReadP99Ms":{"idle":pct(&idle.ctx_read,99),"churn":pct(&churn.ctx_read,99),"base4":pct(&base.ctx_read,99)},
        "importP99Ms":{"idle":pct(&idle.import,99),"churn":pct(&churn.import,99)},
        "reconstructionColdP99Ms":{"idle":pct(&idle.recon_cold,99),"churn":pct(&churn.recon_cold,99)}}));
    let max_delta=churn.deltas.iter().copied().max().unwrap_or(0);
    println!("FN3C-CHURN max delta {max_delta}, members at end {}",churn.members_end);
    assert!(max_delta>0&&churn.members_end<10_000,"positive control: the churn run changed the member set");
    for (what,r) in [("idle",&idle),("churn",&churn)]{
        assert!(snap_rows(r)>=2,"{what}: two snapshots written ({:?})",r.snapshots);
        assert!(per_day(r)<100e6,"{what}: C6: the membership tables grow {} B a day, not below 100 MB",per_day(r));
    }
}
