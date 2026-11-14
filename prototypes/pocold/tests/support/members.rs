//! FN-3a (C6): the end-of-test cold sweep and the oracle's positive control, shared by the suites that run chains.
#![allow(dead_code)]
use pocold::chain::Chain;
/// Clears every member cache, recomputes every held entry's assignment from its member records and compares it with the
/// stored ranges (`Chain::member_sweep`); the oracle found no mismatch and no member fault. With a block held, the
/// positive control: the sweep compared at least one entry.
pub fn sweep(c:&Chain)->(usize,usize){
    let (n,k)=c.member_sweep().unwrap_or_else(|e|panic!("FN-3 cold sweep: {e}"));
    let m=c.member_counters();
    assert_eq!(m.oracle_mismatch,0,"FN-3: the oracle found a mismatch");assert_eq!(m.faults,0,"FN-3: a member fault");
    if c.head().height>0{assert!(k>0,"FN-3 positive control: the sweep compared an entry");}
    (n,k)
}
/// The sweep at the end of a test, when the test did not already fail (a panic while unwinding would abort). A chain
/// that validated its blocks (every bare chain here): the import oracle compared too.
pub fn sweep_on_drop(c:&Chain){
    if std::thread::panicking(){return}
    sweep(c);
    if c.head().height>0{let m=c.member_counters();assert!(m.oracle_checks>0,"FN-3 positive control: the oracle compared ({} checks)",m.oracle_checks);}
}
