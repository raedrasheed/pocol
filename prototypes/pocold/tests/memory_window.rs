//! PR7 (C-1 S5, T2): the chain's heap stays flat as it grows, once the eviction window holds E entries. Its own
//! test binary, for a counting global allocator (as tests/eviction_memory.rs).
use std::alloc::{GlobalAlloc,Layout,System};
use std::sync::atomic::{AtomicUsize,Ordering};
use std::collections::HashMap;
use pocold::{chain::{BlockBytes,Chain,Reject,Spec},types::*};
use pocol_lp4_execution::{devnet::{self,DevnetConfig},txsign::Key};
use revm::primitives::{Address,B256};

struct Meter;
static LIVE:AtomicUsize=AtomicUsize::new(0);
unsafe impl GlobalAlloc for Meter{
    unsafe fn alloc(&self,l:Layout)->*mut u8{let p=System.alloc(l);if !p.is_null(){LIVE.fetch_add(l.size(),Ordering::SeqCst);}p}
    unsafe fn dealloc(&self,p:*mut u8,l:Layout){System.dealloc(p,l);LIVE.fetch_sub(l.size(),Ordering::SeqCst);}
    unsafe fn realloc(&self,p:*mut u8,l:Layout,n:usize)->*mut u8{let q=System.realloc(p,l,n);if !q.is_null(){LIVE.fetch_add(n,Ordering::SeqCst);LIVE.fetch_sub(l.size(),Ordering::SeqCst);}q}
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
/// A block source with no index: the chain under test never falls back below its window.
struct NoIndex;
impl BlockBytes for NoIndex{fn record_at(&self,_:u64)->Result<(Vec<u8>,Vec<u8>,Vec<u8>),Reject>{Err(Reject::Local("no journal".into()))}}

/// PR7 (T2): a whole-state chain evicting below head - E (R_keep 8, slim depth 20, the default E) holds the same heap
/// at E + 1,000 and E + 2,000 blocks, within 64 KiB. Before PR7 each canonical entry stayed in memory (about 1.3 KB
/// each even slimmed), so the heap grew by about 1.3 MB between the two.
#[test] fn long_chain_heap_is_flat(){
    let d=devnet::build(&cfg()).unwrap();let keys:HashMap<Address,Key>=d.miners.iter().map(|m|(m.id,Key::from_secret(&m.key.secret()).unwrap())).collect();
    let limits=pocold::node::Limits{r_keep:8,side_cap:8,slim_depth:Some(20),..Default::default()};
    let e=pocold::resources::evict_depth_of(admitted().cp(),admitted().local().k_hist,&limits,8).unwrap().unwrap();let n=e as usize+2000;
    let blocks={let mut b=Chain::new(admitted());let mut v=Vec::with_capacity(n);
        for _ in 0..n{let x=next(&b,&keys);let (en,_)=b.validate(&x,None,0).unwrap();b.insert(en);b.retain();v.push(x);}v};
    let mut c=Chain::new(admitted());c.r_keep=8;c.side_cap=8;c.slim_depth=Some(20);c.history_window=Some(32);c.evict_depth=Some(e);c.evict_batch=8+2*8+2;
    let mut heap=vec![];
    for (i,x) in blocks.iter().enumerate(){
        let (en,_)=c.validate(x,None,0).unwrap();c.insert(en);c.retain_unsettled();c.window(&NoIndex).unwrap();c.evicted_out.clear();c.settle();
        if i+1==e as usize+1000||i+1==n{heap.push(LIVE.load(Ordering::SeqCst))}
    }
    assert!(c.window_done()&&c.evict_stalls==0&&c.blocks.len() as u64<=e+2);
    // Measured 60,566 B: the block table's buckets doubling once more (to 1,024 for E + 2 entries; insertions and
    // removals of other keys leave tombstones), after which it rehashes in place. A13 counts 4 slots per entry.
    let grew=heap[1] as i64-heap[0] as i64;println!("PR7-HEAP {heap:?} grew {grew} B over 1,000 blocks");
    assert!(grew.abs()<64<<10,"FAIL-ON-ORIGINAL: the heap grew {grew} B over 1,000 blocks");
}
