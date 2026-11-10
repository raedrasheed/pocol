//! PR7 (C-1 S5, §7, T21): what `Node::open` holds while it replays the journal, beyond what the node holds once
//! open, is counted by `resources::open_transient`. Its own test binary, for a counting global allocator that also
//! keeps the peak (as tests/memory_window.rs and tests/eviction_memory.rs).
use std::alloc::{GlobalAlloc,Layout,System};
use std::sync::atomic::{AtomicUsize,Ordering};
use std::collections::HashMap;
use pocold::{chain::{Chain,Spec},node::{Config,Limits,Node},types::*};
use pocol_lp4_execution::{devnet::{self,DevnetConfig},txsign::Key};
use revm::primitives::{Address,B256};

struct Meter;
static LIVE:AtomicUsize=AtomicUsize::new(0);
static PEAK:AtomicUsize=AtomicUsize::new(0);
fn grew(by:usize){let n=LIVE.fetch_add(by,Ordering::SeqCst)+by;PEAK.fetch_max(n,Ordering::SeqCst);}
unsafe impl GlobalAlloc for Meter{
    unsafe fn alloc(&self,l:Layout)->*mut u8{let p=System.alloc(l);if !p.is_null(){grew(l.size());}p}
    unsafe fn dealloc(&self,p:*mut u8,l:Layout){System.dealloc(p,l);LIVE.fetch_sub(l.size(),Ordering::SeqCst);}
    unsafe fn realloc(&self,p:*mut u8,l:Layout,n:usize)->*mut u8{let q=System.realloc(p,l,n);
        if !q.is_null(){if n>=l.size(){grew(n-l.size())}else{LIVE.fetch_sub(l.size()-n,Ordering::SeqCst);}}q}
}
#[global_allocator]
static ALLOC:Meter=Meter;

fn cfg()->DevnetConfig{let mut c=DevnetConfig::new(777910,1_700_000_000,&["a","b","c"]);c.hash_rate=1024;c.d_att=73;c.tau=40;c.c=8;c}
fn admitted()->Spec{let d=devnet::build(&cfg()).unwrap();Spec::new(d.admit(&pocold::single_host_local()).unwrap_or_else(|e|panic!("{e}")).0)}
/// A fallback block on the builder's head (attempt 0, or the next one with a solution).
fn next(c:&Chain,keys:&HashMap<Address,Key>)->Block{
    let p=c.full_entry(&c.head).unwrap();
    for a in 0..16{let (mut h,_,_)=c.build_fallback(&p,a).unwrap();let ctx=c.child_ctx(&p,a).unwrap();let tid=template_id(&h.ut_raw);let mut shares=vec![];let mut x=0u64;
        let found=loop{if (x as u128)>=ctx.assignment.nonce_max(){break false}let ph=pow_hash(&tid,x);if le_target(&ph,&ctx.t_share){if le_target(&ph,&ctx.target){break true}shares.push(x);}x+=1;};
        if !found{continue}
        let owner=addr(&ctx.assignment.owner(x).unwrap());h.nonce=x;shares.truncate(256);h.shares=shares;reseal(&mut h);
        h.winner_sig=keys[&owner].sign_hash(&B256::from(pocold::hashes::win_msg(&tid,x,&share_root(&h))));reseal(&mut h);
        return Block{header:h,body:Body::default()}}
    panic!("no fallback solution")
}

/// PR7 (T21): a whole-state node (R_keep 8, slim depth 20; no checkpoints, so every open replays from genesis, as
/// `pocold run` does) opened on journals of 1,000 and 2,000 records, with eviction off and with the default one. What
/// the open holds at its peak beyond what the open node holds is within `open_transient(records)`, the canonical
/// index's page cache (filled as replay writes the rows below the window, at most `canon::CANON_CACHE`, a fixed term
/// of the bound) and a fixed slack (the replay's execution and retention scratch, redb's write buffers, and the
/// tables `Chain::shrink` gives back at the end of open). With eviction off nothing but the replay tables (`node::Ev`
/// events, the journal's frame offsets, the sampled rolling hashes) grows with the journal, and it grows by at most
/// the model's growth. Before PR7 §7 the replay index was a map of a queue per hash, with every prefix's rolling hash
/// and a second offset vector beside the journal's: it grew 188,815 B from 1,000 to 2,000 records (measured).
#[test] fn the_open_transient_is_counted(){
    let d=devnet::build(&cfg()).unwrap();let keys:HashMap<Address,Key>=d.miners.iter().map(|m|(m.id,Key::from_secret(&m.key.secret()).unwrap())).collect();
    let blocks={let mut b=Chain::new(admitted());let mut v=Vec::with_capacity(2000);
        for _ in 0..2000{let x=next(&b,&keys);let (en,_)=b.validate(&x,None,0).unwrap();b.insert(en);b.retain();v.push(x);}v};
    // The slack: none of it depends on the journal's length (measured below 1.4 MB).
    const SLACK:u128=4<<20;let cache=pocold::node::canon::CANON_CACHE as u128;
    let mut log=vec![];
    for evict in [Some(0),None]{
        let dir=std::env::temp_dir().join(format!("pocold-t21-{}-{}",evict.is_some(),std::process::id()));let _=std::fs::remove_dir_all(&dir);
        let limits=Limits{r_keep:8,side_cap:8,slim_depth:Some(20),evict_depth:evict,..Default::default()};
        let conf=||Config{dir:dir.clone(),name:"t21".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:limits.clone()};
        let (mut done,mut got)=(0usize,vec![]);
        for upto in [1000usize,2000]{
            {let mut n=Node::open(conf(),admitted()).unwrap();for b in &blocks[done..upto]{n.import(b.clone(),None).unwrap();}}done=upto;
            PEAK.store(LIVE.load(Ordering::SeqCst),Ordering::SeqCst);
            let n=Node::open(conf(),admitted()).unwrap();let (peak,steady)=(PEAK.load(Ordering::SeqCst),LIVE.load(Ordering::SeqCst));
            assert_eq!((n.journal_records(),n.stats.resumed_from),(upto as u64,None),"positive control: the open replayed every record");
            assert_eq!(n.chain.canonical.base()>1,evict.is_none(),"positive control: the window evicts only by default");
            let model=pocold::resources::open_transient(n.chain.spec.cp(),&n.cfg.limits,upto as u64).unwrap();
            let t=peak.saturating_sub(steady) as u128;
            assert!(t<=model+cache+SLACK,"{evict:?}, {upto} records: the open held {t} B beyond the open node; the model {model} B, the index cache {cache} B and {SLACK} B");
            got.push((upto,t,model));drop(n);
        }
        let _=std::fs::remove_dir_all(&dir);
        let ((_,t1,m1),(_,t2,m2))=(got[0],got[1]);log.push((evict,got.clone(),(t2 as f64-t1 as f64)/1000.0,(m2-m1)/1000));
        if evict==Some(0){assert!(t2<=t1+(m2-m1),"FAIL-ON-ORIGINAL: the open transient grew {} B from 1,000 to 2,000 records, the model {} B",t2 as i128-t1 as i128,m2-m1);}
    }
    // Measured: about 89 B a record with eviction off and 107 B by default (the tables at their capacity, and the index's
    // cache filling), the model 160 B (the tables at twice their length).
    println!("PR7-OPEN-TRANSIENT (evict, [(records, measured B, model B)], measured B a record, model B a record) {log:?}");
}
