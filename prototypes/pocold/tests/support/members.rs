//! FN-3a (C6): the end-of-test cold sweep and the oracle's positive control, shared by the suites that run chains.
#![allow(dead_code)]
use pocold::chain::{ranges_equal,Chain};
use pocold::node::Node;
/// The held entries the cold sweep recomputes: every one but genesis whose parent is held.
fn swept(c:&Chain)->impl Iterator<Item=&pocold::chain::Entry>{c.blocks.values().map(|b|&**b).filter(move|e|e.height>0&&e.parent.is_some_and(|p|c.get(&p).is_some()))}
/// Clears every member cache, recomputes every held entry's assignment from its member records and compares it with the
/// stored ranges (`Chain::member_sweep`); the oracle found no mismatch and no member fault. Every unslimmed entry the
/// sweep recomputes is compared (a slimmed one holds no ranges: `sweep_node` compares it with history.redb's). With a
/// block held, the positive control: the sweep compared at least one entry.
pub fn sweep(c:&Chain)->(usize,usize){
    let (n,k)=c.member_sweep().unwrap_or_else(|e|panic!("FN-3 cold sweep: {e}"));
    let m=c.member_counters();
    assert_eq!(m.oracle_mismatch,0,"FN-3: the oracle found a mismatch");assert_eq!(m.faults,0,"FN-3: a member fault");
    let (all,unslim)=swept(c).fold((0,0),|(a,u),e|(a+1,u+usize::from(!e.slim)));
    assert_eq!((n,k),(all,unslim),"FN-3: the sweep recomputed every held entry and compared every unslimmed one");
    if c.head().height>0{assert!(k>0,"FN-3 positive control: the sweep compared an entry");}
    held_records(c);
    (n,k)
}
/// `sweep` on a node's chain, and each slimmed entry's recomputed assignment compared with the ranges history.redb
/// restores for it (`Node::restored_entry`). Returns (recomputed, compared, slimmed compared).
pub fn sweep_node(node:&Node)->(usize,usize,usize){
    let (n,k)=sweep(&node.chain);let c=&node.chain;let mut s=0;
    let mut slim:Vec<&pocold::chain::Entry>=swept(c).filter(|e|e.slim).collect();slim.sort_by_key(|e|(e.height,e.hash));
    for e in slim{
        let r=node.restored_entry(&e.hash).unwrap_or_else(||panic!("FN-3: the slimmed entry at height {} restores",e.height));
        assert!(!r.ranges.is_empty(),"FN-3: the slimmed entry at height {} restores its ranges",e.height);
        let a=c.assignment_of(e).unwrap_or_else(|x|panic!("FN-3: the slimmed entry at height {}: {x:?}",e.height));
        assert!(ranges_equal(&r.ranges,&a),"FN-3: the slimmed entry at height {}: the recomputed assignment differs from history.redb's ranges",e.height);s+=1;
    }
    c.clear_member_caches();
    assert_eq!(k+s,n,"FN-3: every recomputed entry was compared");
    (n,k,s)
}
/// The A13 term "member records on entries not offloaded" (`resources`, FN-3): `offload` drops an entry's member record
/// (and a snapshot's ids), so an offloaded entry holds none, and on a node (a resident window) at most the window plus
/// two entries (one full side block, one being validated) hold one. Returns (offloaded, holding a record).
pub fn held_records(c:&Chain)->(usize,usize){
    let offloaded=c.blocks.values().filter(|e|e.offloaded).count();
    for e in c.blocks.values().filter(|e|e.offloaded){assert!(e.members.is_none(),"FN-3: the offloaded entry at height {} holds its member record",e.height);}
    let holding=c.blocks.values().filter(|e|e.height>0&&e.members.is_some()).count();
    if let Some(w)=c.history_window{assert!(holding as u64<=w+2,"FN-3: {holding} entries hold a member record, above the window {w} + 2");}
    (offloaded,holding)
}
/// The sweep at the end of a test, when the test did not already fail (a panic while unwinding would abort). A chain
/// that validated its blocks (every bare chain here): the import oracle compared too.
pub fn sweep_on_drop(c:&Chain){
    if std::thread::panicking(){return}
    sweep(c);
    if c.head().height>0{let m=c.member_counters();assert!(m.oracle_checks>0,"FN-3 positive control: the oracle compared ({} checks)",m.oracle_checks);}
}
