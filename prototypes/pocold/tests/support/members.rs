//! FN-3 (C6): the oracle switch, the end-of-test cold sweep and the oracle's positive control, shared by the suites that
//! run chains.
#![allow(dead_code)]
use pocold::chain::Chain;
use pocold::node::Node;
/// FN-3b: the oracle on for every chain this process creates from now on, and the capture of each validated block's
/// assignment (`pocold::members::test_oracle`). A suite calls it where it builds a spec, so every chain and node of every
/// test runs under it whatever order the tests run in.
pub fn oracle(){pocold::members::test_oracle();}
/// The held entries the cold sweep recomputes: every one but genesis whose parent is held.
fn swept(c:&Chain)->impl Iterator<Item=&pocold::chain::Entry>{c.blocks.values().map(|b|&**b).filter(move|e|e.height>0&&e.parent.is_some_and(|p|c.get(&p).is_some()))}
/// Clears every member cache, recomputes every held entry's assignment from its member records and compares it with the
/// assignment the block was validated under (`Chain::member_sweep`, against the capture); the oracle was on, found no
/// mismatch and no member fault. Every entry the sweep recomputes, slimmed ones included, is compared. With a block
/// held, the positive control: the sweep compared at least one entry.
pub fn sweep(c:&Chain)->(usize,usize){
    assert!(c.member_oracle,"FN-3: the chain runs the oracle (members_sweep::oracle)");
    let (n,k)=c.member_sweep().unwrap_or_else(|e|panic!("FN-3 cold sweep: {e}"));
    let m=c.member_counters();
    assert_eq!(m.oracle_mismatch,0,"FN-3: the oracle found a mismatch");assert_eq!(m.faults,0,"FN-3: a member fault");
    let all=swept(c).count();
    assert_eq!((n,k),(all,all),"FN-3: the sweep recomputed every held entry and compared each with its captured assignment");
    if c.head().height>0{assert!(k>0,"FN-3 positive control: the sweep compared an entry");}
    held_records(c);
    (n,k)
}
/// `sweep` on a node's chain, and each slimmed entry restored whole from the journal and history.redb
/// (`Node::restored_entry`). Returns (recomputed, compared, slimmed compared).
pub fn sweep_node(node:&Node)->(usize,usize,usize){
    let (n,k)=sweep(&node.chain);let c=&node.chain;let mut s=0;
    let mut slim:Vec<&pocold::chain::Entry>=swept(c).filter(|e|e.slim).collect();slim.sort_by_key(|e|(e.height,e.hash));
    for e in slim{
        let r=node.restored_entry(&e.hash).unwrap_or_else(||panic!("FN-3: the slimmed entry at height {} restores",e.height));
        assert!(!r.slim&&r.result.is_some(),"FN-3: the slimmed entry at height {} restores whole",e.height);
        let a=c.assignment_of(e).unwrap_or_else(|x|panic!("FN-3: the slimmed entry at height {}: {x:?}",e.height));
        assert_eq!(c.captured(e),Some(pocold::members::ranges_digest(&a)),"FN-3: the slimmed entry at height {}",e.height);s+=1;
    }
    c.clear_member_caches();
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
