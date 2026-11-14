//! FN-3a (C6, node-local; development plan FN-3 §5): each block's member record (its member set as a delta from its
//! parent's, or a snapshot every 256 heights), from which a stored block's nonce assignment is recomputed. FN-3a keeps
//! the stored ranges and takes every verdict from them; the recomputation is compared at each reader (the shadow), at
//! import (the oracle) and by the end-of-test cold sweep. Nothing here changes a verdict: every comparison below is
//! against the verdict or the ranges the chain had before FN-3.
#[path="support/members.rs"] mod members_sweep;
use std::collections::HashMap;
use redb::ReadableTable;
use std::path::{Path,PathBuf};
use std::sync::{Arc,Mutex};
use std::sync::atomic::{AtomicU64,Ordering};
use pocol_lp4_execution::{apply::{self,EvidenceEffect},devnet::{self,DevnetConfig},txsign::{Key,Tx}};
use pocold::chain::{late_check,late_encode,BlockBytes,Chain,Entry,LateBase,LateBlock,LateRules,LazyOwners,Reject,Spec};
use pocold::members::{self,MemberRec,MemberSets,MemberStep,NonceOwners,Set};
use pocold::node::{Config,Limits,Node};
use pocold::{hashes,rlp,types::*};
use revm::primitives::{keccak256,Address,Bytes,B256,U256};

struct Net{chain:Chain,keys:HashMap<Address,Key>,cfg:DevnetConfig,ids:Vec<Address>}
impl Drop for Net{fn drop(&mut self){members_sweep::sweep_on_drop(&self.chain);}}
/// The v3 earn-in devnet (CP schema v2: the v3 program set, all-live membership, the full nonce domain) or its spec-1
/// twin (bounded nonce ranges, the activity window). E_win 80, so type-2 evidence reaches far back.
fn config(spec:u8)->DevnetConfig{
    let mut c=DevnetConfig::new(777953,1_700_000_000,&["one","two","three"]);
    c.hash_rate=16;c.c=8;c.d_att=73;c.r_max=1;c.earnin=true;c.spec_version=spec;c.e_win=80;c.u=96;c.c_react=96;c
}
fn spec_of(c:&DevnetConfig)->Spec{Spec::new(devnet::build(c).unwrap().admit(&pocold::single_host_local()).unwrap_or_else(|e|panic!("{e}")).0)}
fn net(spec:u8)->Net{
    let cfg=config(spec);let d=devnet::build(&cfg).unwrap();
    let mut keys:HashMap<Address,Key>=d.miners.iter().map(|m|(m.id,Key::from_secret(&m.key.secret()).unwrap())).collect();
    for label in ["alice","bob"]{let k=devnet::account_key(label);keys.insert(k.address(),k);}
    let ids=d.miners.iter().map(|m|m.id).collect();
    Net{chain:Chain::new(spec_of(&cfg)),keys,cfg,ids}
}
/// The nonces scanned on a template: the first SCAN of every member's range (the ranges are wide in the full domain).
const SCAN:u128=1<<12;
fn scan(ctx:&pocold::chain::ChildCtx)->impl Iterator<Item=u64>+'_{ctx.assignment.ranges().iter().flat_map(|r|(r.start()..r.end_exclusive().min(r.start()+SCAN)).map(|x|x as u64))}
/// A signed block on `parent` by Draw(h,0), carrying `txs` and `evidence` (effects applied), won by the first solution
/// scanned outside `omit`'s range; its header shares: up to 8 shares scanned outside `omit`'s range.
fn mine(n:&Net,parent:[u8;32],txs:Vec<Vec<u8>>,evidence:Vec<Vec<u8>>,omit:Option<Address>)->Block{
    let p=n.chain.full_entry(&parent).unwrap();let ctx=n.chain.child_ctx(&p,0).unwrap();
    let body=Body{txs,evidence};let effects=n.chain.evidence_effects(&p,&body,p.height+1).unwrap();
    for ts in p.ts+u64::from(n.chain.spec.cp().T_blk)..=ctx.window.upper(){
        let (mut h,_,_)=n.chain.build_signed(&p,&n.keys[&ctx.proposer],ts,body.clone(),effects.clone()).unwrap();
        let tid=template_id(&h.ut_raw);let (mut shares,mut win)=(vec![],None);
        for nonce in scan(&ctx){
            let ph=pow_hash(&tid,nonce);let owner=addr(&ctx.assignment.owner(nonce).unwrap());if omit==Some(owner){continue}
            if le_target(&ph,&ctx.target){if win.is_none(){win=Some((nonce,owner));}}else if le_target(&ph,&ctx.t_share)&&shares.len()<8{shares.push(nonce);}
        }
        let Some((nonce,owner))=win else{continue};
        shares.sort();h.nonce=nonce;h.shares=shares;reseal(&mut h);
        h.winner_sig=n.keys[&owner].sign_hash(&B256::from(hashes::win_msg(&tid,nonce,&share_root(&h))));reseal(&mut h);
        return Block{header:h,body};
    }
    panic!("no solution")
}
/// Up to 4 shares of `who` scanned on block `b`'s template that its header left out.
fn omitted(n:&Net,b:&Block,who:Address)->Vec<u64>{
    let p=n.chain.full_entry(&b.header.parent_hash).unwrap();let ctx=n.chain.child_ctx(&p,0).unwrap();let tid=template_id(&b.header.ut_raw);
    let mut v:Vec<u64>=scan(&ctx).filter(|x|*x!=b.header.nonce&&addr(&ctx.assignment.owner(*x).unwrap())==who&&le_target(&pow_hash(&tid,*x),&ctx.t_share)&&!le_target(&pow_hash(&tid,*x),&ctx.target)&&!b.header.shares.contains(x)).collect();
    v.sort();v.truncate(4);v
}
fn accept(n:&mut Net,b:&Block)->[u8;32]{let (e,_)=n.chain.validate(b,None,0).unwrap_or_else(|e|panic!("h={}: {e:?}",b.header.h));let h=e.hash;n.chain.insert(e);h}
fn grow(n:&mut Net,to:u64){while n.chain.head().height<to{let b=mine(n,n.chain.head,vec![],vec![],None);accept(n,&b);}}
fn tx(n:&Net,key:Address,data:Vec<u8>,value:U256)->Vec<u8>{
    let p=n.chain.full_entry(&n.chain.head).unwrap();let base=n.chain.child_ctx(&p,0).unwrap().base_fee;
    let nonce=p.state.accounts.get(&key).map_or(0,|a|a.info.nonce);
    let prio=U256::from(1_000_000_000u64);
    Tx{chain_id:n.chain.spec.chain_id(),nonce,max_priority:prio,max_fee:base*U256::from(2)+prio,gas:1_000_000,to:Some(apply::registry()),value,data:Bytes::from(data)}.sign_1559(&n.keys[&key])
}
fn register(n:&Net,who:Address)->Vec<u8>{let mut d=keccak256("register(address)")[..4].to_vec();d.extend_from_slice(who.into_word().as_slice());tx(n,who,d,U256::ZERO)}
fn deregister(n:&Net,who:Address)->Vec<u8>{tx(n,who,keccak256("deregister()")[..4].to_vec(),U256::ZERO)}
/// Type-2 evidence against block `b`'s winner: a second share root signed for its TemplateID and nonce.
fn type2(n:&Net,b:&Block)->(Vec<u8>,Address){
    let winner=n.chain.get(&b.hash()).and_then(|e|e.result.as_ref()).expect("the block is held whole").winner;let tid=template_id(&b.header.ut_raw);
    let mut alt=b.header.clone();alt.shares=if alt.shares.is_empty(){vec![0]}else{vec![]};reseal(&mut alt);
    let (r1,r2)=(share_root(&b.header),share_root(&alt));
    let sig2=n.keys[&winner].sign_hash(&B256::from(hashes::win_msg(&tid,b.header.nonce,&r2)));
    let mut ev=Vec::new();rlp::encode_bytes(&[2],&mut ev);rlp::encode_bytes(&tid,&mut ev);rlp::encode_bytes(&b.header.nonce.to_be_bytes(),&mut ev);
    rlp::encode_bytes(&r1,&mut ev);rlp::encode_bytes(&b.header.winner_sig,&mut ev);rlp::encode_bytes(&r2,&mut ev);rlp::encode_bytes(&sig2,&mut ev);
    let mut out=Vec::new();rlp::encode_list_payload(&ev,&mut out);(out,winner)
}
fn effects(c:&Chain,evidence:&[Vec<u8>])->Result<Vec<EvidenceEffect>,Reject>{
    let p=c.full_entry(&c.head).unwrap();c.evidence_effects(&p,&Body{txs:vec![],evidence:evidence.to_vec()},p.height+1)
}
fn entry_at(c:&Chain,h:u64)->&Entry{c.canonical_at(h).unwrap()}
fn rec(c:&Chain,h:u64)->MemberRec{c.member_rec_of(entry_at(c,h)).unwrap()}

/// A member source a test controls: every record and set of a chain, and a fault switch (a record whose digest is off,
/// or none at all), counting reads and the faults reported to it.
#[derive(Default)]
struct TestSets{rows:Mutex<HashMap<[u8;32],([u8;32],u64,MemberRec,Option<Set>)>>,bad:AtomicU64,reads:AtomicU64,damaged:AtomicU64}
impl TestSets{fn of(c:&Chain)->Arc<TestSets>{
    let t=TestSets::default();
    {let mut r=t.rows.lock().unwrap();for e in c.blocks.values(){if let (Some(p),Some(m))=(e.parent,e.members.as_deref()){r.insert(e.hash,(p,e.height,m.rec.clone(),m.ids.clone()));}}}
    Arc::new(t)
}}
impl MemberSets for TestSets{
    fn record(&self,h:&[u8;32])->Result<Option<([u8;32],u64,MemberRec)>,Reject>{
        self.reads.fetch_add(1,Ordering::SeqCst);
        match self.bad.load(Ordering::SeqCst){1=>return Ok(None),2=>return Err(Reject::Local("the test's source fails".into())),_=>{}}
        Ok(self.rows.lock().unwrap().get(h).map(|x|{let mut r=x.2.clone();if self.bad.load(Ordering::SeqCst)==3{r.digest[0]^=1;}(x.0,x.1,r)}))
    }
    fn set(&self,h:&[u8;32])->Result<Option<Set>,Reject>{self.reads.fetch_add(1,Ordering::SeqCst);if self.bad.load(Ordering::SeqCst)==2{return Err(Reject::Local("the test's source fails".into()))}Ok(self.rows.lock().unwrap().get(h).and_then(|x|x.3.clone()))}
    fn damaged(&self,_:&str){self.damaged.fetch_add(1,Ordering::SeqCst);}
}
/// What an offload leaves of an entry's member data: nothing (the source holds it).
fn drop_held_records(c:&mut Chain,below:u64){for e in c.blocks.values_mut(){if e.height>0&&e.height<below{e.members=None;}}}

/// §5.1, §5.2, §5.5: deltas between the snapshots at 1, 257 and 513 (a registration and a deregistration change S_b);
/// type-2 evidence against block 512, whose nearest snapshot is 255 heights below, is judged the same with the records
/// held, after the caches are cleared and every held record is dropped (the walk reads 255 records and the snapshot's
/// set from the source), and a source that fails or does not add up is a local fault, never a verdict.
#[test] fn type2_against_a_block_255_deltas_above_its_snapshot_is_judged_from_the_source(){
    let mut n=net(2);let (alice,bob)=(devnet::account_key("alice").address(),devnet::account_key("bob").address());
    grow(&mut n,2);let t=register(&n,alice);let b=mine(&n,n.chain.head,vec![t],vec![],None);accept(&mut n,&b);
    grow(&mut n,300);let t=register(&n,bob);let b=mine(&n,n.chain.head,vec![t],vec![],None);accept(&mut n,&b);
    grow(&mut n,400);let t=deregister(&n,alice);let b=mine(&n,n.chain.head,vec![t],vec![],None);accept(&mut n,&b);
    grow(&mut n,511);let b512=mine(&n,n.chain.head,vec![],vec![],None);accept(&mut n,&b512);grow(&mut n,520);
    // The records: snapshots exactly at the cadence (a v3 block changes few ids), deltas of the registrations.
    let c=&n.chain;
    for h in 1..=520u64{assert_eq!(rec(c,h).is_snapshot(),h%256==1,"h={h}");}
    let changed:Vec<(u64,usize)>=(2..=520u64).filter_map(|h|{let r=rec(c,h);(r.delta_len()>0).then(||(h,r.delta_len()))}).collect();
    println!("FN3-DELTAS {changed:?}");assert!(changed.len()>=2&&changed.iter().all(|x|x.1==1),"the registrations change S_b: {changed:?}");
    let s4=c.member_set(entry_at(c,4)).unwrap();assert!(s4.contains(&alice.0.0),"alice is a member after her registration");
    let m=c.member_counters();assert_eq!(m.snapshots_written,3);assert_eq!(m.delta_max,1);assert!(m.held.0<=m.caps.0&&m.held.1<=m.caps.1&&m.held.2<=m.caps.2,"the caches keep their caps");assert!(m.oracle_checks>=520&&m.oracle_mismatch==0);
    // The verdict with the records held.
    let (ev,accused)=type2(&n,&b512);
    let want=effects(&n.chain,&[ev.clone()]).unwrap();assert_eq!(want,vec![EvidenceEffect::PunishAndInvalidate{accused,j:512}]);
    // The same from the source alone: caches cleared, every held record dropped (as `offload` drops them).
    let src=TestSets::of(&n.chain);n.chain.set_member_source(src.clone());drop_held_records(&mut n.chain,515);
    let before=n.chain.member_counters();
    assert_eq!(effects(&n.chain,&[ev.clone()]).unwrap(),want,"the verdict from the source");
    let after=n.chain.member_counters();
    assert!(after.record_reads-before.record_reads>=255,"a 255-step walk read the records: {}",after.record_reads-before.record_reads);
    assert!(after.disk_set_reads>before.disk_set_reads&&after.reconstructions>before.reconstructions);
    assert!(after.oracle_checks>before.oracle_checks&&after.oracle_mismatch==0,"the shadow compared the accused's owner");
    // A source that fails, holds nothing, or does not add up: a local fault (the source told), never a verdict.
    for (bad,what) in [(2u64,"fails"),(1,"missing"),(3,"digest")]{
        n.chain.clear_member_caches();src.bad.store(bad,Ordering::SeqCst);let d=src.damaged.load(Ordering::SeqCst);
        match effects(&n.chain,&[ev.clone()]){Err(Reject::Local(w))=>assert!(w.contains("member records")||w.contains("test's source"),"{what}: {w}"),r=>panic!("{what}: {r:?}")}
        if bad!=2{assert!(src.damaged.load(Ordering::SeqCst)>d,"{what}: the source is told it is damaged");}
    }
    src.bad.store(0,Ordering::SeqCst);n.chain.clear_member_caches();assert_eq!(effects(&n.chain,&[ev]).unwrap(),want);
    let faults=n.chain.member_counters().faults;assert!(faults>=2,"faults counted: {faults}");
    let (k,cmp)=n.chain.member_sweep().unwrap();assert!(k>=520&&cmp>=520,"the cold sweep over the source");
    // The faults above were the test's own: the drop-time sweep (whose control requires none) runs on a replay of the blocks.
    let blocks:Vec<Block>=(1..=520u64).map(|h|{let e=entry_at(&n.chain,h);Block{header:e.header.clone(),body:e.body.clone()}}).collect();
    let mut m2=Chain::new(spec_of(&n.cfg));for b in &blocks{let (e,_)=m2.validate(b,None,0).unwrap();m2.insert(e);}
    assert_eq!(m2.head,n.chain.head);for h in 1..=520u64{assert_eq!(rec(&m2,h),rec(&n.chain,h),"a replayed chain writes the same records, h={h}");}
    n.chain=m2;
}

/// §5.10: a bare chain that evicts below head − E keeps its MemSets down to the floor − WALK_MAX only, and type-2
/// evidence against a block whose snapshot was evicted is judged from them.
#[test] fn a_bare_chain_judges_evidence_whose_snapshot_was_evicted(){
    struct NoIndex;impl BlockBytes for NoIndex{fn record_at(&self,_:u64)->Result<(Vec<u8>,Vec<u8>,Vec<u8>),Reject>{Err(Reject::Local("no journal".into()))}}
    let mut n=net(2);let mut r=net(2);
    n.chain.r_keep=8;n.chain.side_cap=8;n.chain.evict_depth=Some(258);n.chain.evict_batch=8+2*8+2;
    let mut b512=None;
    while n.chain.head().height<525{
        let b=mine(&n,n.chain.head,vec![],vec![],None);if b.header.h==512{b512=Some(b.clone());}
        accept(&mut n,&b);n.chain.retain_unsettled();n.chain.window(&NoIndex).unwrap();n.chain.evicted_out.clear();n.chain.settle();
        accept(&mut r,&b);
    }
    assert!(n.chain.canonical.base()>257,"the snapshot at 257 was evicted (base {})",n.chain.canonical.base());
    assert!(n.chain.get(&r.chain.canonical_at(257).unwrap().hash).is_none());
    let (ev,accused)=type2(&n,&b512.unwrap());n.chain.clear_member_caches();
    let got=effects(&n.chain,&[ev.clone()]).unwrap();assert_eq!(got,vec![EvidenceEffect::PunishAndInvalidate{accused,j:512}]);
    assert_eq!(got,effects(&r.chain,&[ev]).unwrap(),"the reference holds every entry");
    let m=n.chain.member_counters();assert!(m.record_reads>0&&m.disk_set_reads>0,"the walk read below the window: {m:?}");
}

/// §5.3: owner(nonce) from the records equals the stored ranges' on random nonces, the domain's edges and beyond nMax,
/// bounded (spec 1) and full (spec 2); kind-3's cap check runs before any lookup, so a capped item recomputes nothing.
#[test] fn recomputed_owners_equal_the_stored_ranges_and_the_cap_check_comes_first(){
    struct Counting<'a>(&'a Vec<(Address,u128,u128)>,AtomicU64);
    impl NonceOwners for Counting<'_>{fn owner(&self,n:u64)->Result<Option<Address>,Reject>{self.1.fetch_add(1,Ordering::SeqCst);self.0.owner(n)}}
    for spec in [1u8,2]{
        let mut n=net(spec);grow(&mut n,12);n.chain.clear_member_caches();
        let mut x=0x9e3779b97f4a7c15u64;let mut nonces=vec![0,1,u64::MAX,u64::MAX-1];
        for _ in 0..200{x^=x<<13;x^=x>>7;x^=x<<17;nonces.push(x);nonces.push(x%4096);}
        for h in 1..=12u64{
            let e=entry_at(&n.chain,h);let mut edges=nonces.clone();for r in e.ranges.iter(){for v in [r.1,r.2]{edges.extend([v.saturating_sub(1) as u64,v.min(u128::from(u64::MAX)) as u64]);}}
            let lazy=LazyOwners::new(&n.chain,e);
            for v in &edges{assert_eq!(lazy.owner(*v).unwrap(),e.ranges.to_vec().owner(*v).unwrap(),"spec {spec} h={h} nonce {v}");}
            if spec==1{assert!(e.ranges.last().unwrap().2<u128::from(u64::MAX),"positive control: a bounded domain")}
        }
        // The cap: C_header(j) + C_late(j) + k > κm refuses before the first owner lookup (none recomputed or read).
        let cp=n.chain.spec.cp();let rules=LateRules{h:13,d:u64::from(cp.D),s_max:u64::from(cp.S_max),kappa_m:u64::from(cp.kappa)*u64::from(cp.m)};
        let ranges=entry_at(&n.chain,12).ranges.to_vec();let probe=Counting(&ranges,AtomicU64::new(0));let credited:std::collections::BTreeSet<u64>=Default::default();
        let base=LateBase{j:12,tid:[0;32],nonce:0,t_share:Fixed::from_u64(1),ranges:&ranges,owners:&probe,header:&[],credited:&credited,total:rules.kappa_m};
        assert_eq!(late_check(&rules,&base,&[5],&mut LateBlock::default()),Err(Reject::Block("15 ls cap".into())));
        assert_eq!(probe.1.load(Ordering::SeqCst),0,"no owner was looked up");
        let base=LateBase{total:0,..base};let _=late_check(&rules,&base,&[5],&mut LateBlock::default());assert_eq!(probe.1.load(Ordering::SeqCst),1,"positive control");
    }
}

/// §5.3 on a chain, kind 3: a valid late-share item is judged with its owners recomputed and compared (the shadow), and
/// an item past the no-dilution cap is refused with "15 ls cap" with nothing recomputed.
#[test] fn kind3_owners_are_compared_and_a_capped_item_recomputes_nothing(){
    let mut n=net(2);let two=n.ids[1];grow(&mut n,3);
    let b4=mine(&n,n.chain.head,vec![],vec![],Some(two));accept(&mut n,&b4);let late=omitted(&n,&b4,two);
    assert!(late.len()>=2,"two's shares were left out: {late:?}");
    n.chain.clear_member_caches();let before=n.chain.member_counters();
    assert_eq!(effects(&n.chain,&[late_encode(4,&late[..2])]).unwrap(),vec![EvidenceEffect::Late{j:4,owners:vec![two,two]}]);
    let after=n.chain.member_counters();assert!(after.oracle_checks>=before.oracle_checks+2&&after.assign_misses>before.assign_misses,"each owner recomputed and compared");
    // Fill j = 5's ledger to κm through late items is not needed: a template with κm header shares caps any item.
    let cp=n.chain.spec.cp();let km=usize::from(cp.kappa)*usize::from(cp.m);
    let p=n.chain.full_entry(&n.chain.head).unwrap();let ctx=n.chain.child_ctx(&p,0).unwrap();
    let body=Body::default();let (mut h,_,_)=n.chain.build_signed(&p,&n.keys[&ctx.proposer],p.ts+u64::from(cp.T_blk),body.clone(),vec![]).unwrap();
    let tid=template_id(&h.ut_raw);let (mut shares,mut win)=(vec![],None);
    for x in ctx.assignment.ranges().iter().flat_map(|r|(r.start()..r.end_exclusive().min(r.start()+(1<<16))).map(|x|x as u64)){
        let ph=pow_hash(&tid,x);if le_target(&ph,&ctx.target){if win.is_none(){win=Some(x);}}else if le_target(&ph,&ctx.t_share){shares.push(x);}}
    shares.sort();assert!(shares.len()>km,"κm + 1 shares scanned: {}",shares.len());let w=win.expect("a solution");
    h.nonce=w;h.shares=shares[..km].to_vec();reseal(&mut h);let owner=addr(&ctx.assignment.owner(w).unwrap());
    h.winner_sig=n.keys[&owner].sign_hash(&B256::from(hashes::win_msg(&tid,w,&share_root(&h))));reseal(&mut h);let b5=Block{header:h,body};accept(&mut n,&b5);
    let spare=shares[km];
    n.chain.clear_member_caches();let before=n.chain.member_counters();
    assert_eq!(effects(&n.chain,&[late_encode(5,&[spare])]),Err(Reject::Block("15 ls cap".into())));
    let after=n.chain.member_counters();assert_eq!((after.assign_misses,after.reconstructions,after.oracle_checks),(before.assign_misses,before.reconstructions,before.oracle_checks),"the cap refused first");
}

// ---------------- nodes: restart, migration, damage, the proposer's pool ----------------
fn copy_dir(from:&Path,to:&Path){std::fs::create_dir_all(to).unwrap();for e in std::fs::read_dir(from).unwrap(){let e=e.unwrap();let t=to.join(e.file_name());
    if e.file_type().unwrap().is_dir(){copy_dir(&e.path(),&t)}else{std::fs::copy(e.path(),&t).unwrap();}}}
fn dir(label:&str)->PathBuf{let d=std::env::temp_dir().join(format!("pocol-fn3-{label}-{}",std::process::id()));let _=std::fs::remove_dir_all(&d);d}
fn conf(d:&Path,limits:Limits,miner:Option<Key>)->Config{Config{name:"fn3".into(),dir:d.to_path_buf(),listen:None,peers:vec![],miner,stop_at:None,log:false,rpc:None,limits}}
fn store()->Limits{Limits{state_store:true,checkpoint_every:64,..Default::default()}}
/// The following block, carrying type-2 evidence against `old` and a kind-3 item for the head, judged by `node` as the
/// reference judges it.
fn judge_next(r:&mut Net,node:&mut Node,old:&Block,late:&[u64],what:&str){
    let (ev2,_)=type2(r,old);let j=r.chain.head().height;
    let b=mine(r,r.chain.head,vec![],vec![ev2,late_encode(j,late)],None);
    let want=r.chain.validate(&b,None,0).map(|x|x.0.hash);assert!(want.is_ok(),"{what}: the reference accepts the carrier");
    node.chain.clear_member_caches();
    assert_eq!(node.import(b.clone(),None).map(|x|{let _=x;b.hash()}),want.clone().map_err(|e|e),"{what}: the node judges the carrier as the reference");
    accept(r,&b);members_sweep::sweep_node(node);
}

/// §5.6: a node restarted three ways (checkpoint restore; replay; history.redb deleted and rebuilt) holds the same chain
/// and member records, and judges a following block carrying type-2 and kind-3 evidence as the reference does. Also the
/// I3 crash point at a snapshot height: a snapshot's rows written to history.redb before the journal append are never
/// read, and the block imported after the crash writes nothing new.
#[test] fn restart_three_ways_keeps_the_records_and_the_verdicts(){
    let mut r=net(2);let two=r.ids[1];let alice=devnet::account_key("alice").address();
    grow(&mut r,5);let t=register(&r,alice);let b=mine(&r,r.chain.head,vec![t],vec![],None);accept(&mut r,&b);
    grow(&mut r,256);
    let blocks:Vec<Block>=(1..=256u64).map(|h|{let e=entry_at(&r.chain,h);Block{header:e.header.clone(),body:e.body.clone()}}).collect();
    let d=dir("restart");
    {let mut a=Node::open(conf(&d,store(),None),spec_of(&r.cfg)).unwrap();for b in &blocks{assert!(a.import(b.clone(),None).unwrap());}members_sweep::sweep_node(&a);
        // The A13 term's premise: an offloaded entry holds no member record, at most the window + 2 entries hold one.
        let (off,holding)=members_sweep::held_records(&a.chain);assert!(off>0&&holding>0,"positive control: {off} offloaded entries, {holding} holding a record");}
    // I3 at the snapshot height 257: its rows (record and set) go to history.redb, then the process stops before the
    // journal append.
    let b257=mine(&r,r.chain.head,vec![],vec![],None);let x=r.chain.validate(&b257,None,0).unwrap().0;assert!(x.members.as_ref().unwrap().rec.is_snapshot());
    {let a=Node::open(conf(&d,store(),None),spec_of(&r.cfg)).unwrap();assert!(a.history.put(&x).unwrap());}
    {let mut a=Node::open(conf(&d,store(),None),spec_of(&r.cfg)).unwrap();assert_eq!(a.chain.head().height,256,"never journaled");assert!(a.history.has(&x.hash).unwrap());
        assert!(a.import(b257.clone(),None).unwrap());}
    accept(&mut r,&b257);grow(&mut r,300);
    let b300=mine(&r,r.chain.head,vec![],vec![],Some(two));accept(&mut r,&b300);let late=omitted(&r,&b300,two);assert!(late.len()>=2);
    let old=Block{header:entry_at(&r.chain,280).header.clone(),body:entry_at(&r.chain,280).body.clone()};
    {let mut a=Node::open(conf(&d,store(),None),spec_of(&r.cfg)).unwrap();for h in 258..=301u64{let e=entry_at(&r.chain,h);assert!(a.import(Block{header:e.header.clone(),body:e.body.clone()},None).unwrap());}
        a.checkpoint_now().unwrap();}
    let mut live=None;
    let reference=||{let mut rr=net(2);for h in 1..=301u64{let e=entry_at(&r.chain,h);accept(&mut rr,&Block{header:e.header.clone(),body:e.body.clone()});}rr};
    // Three copies of the data directory, one per way.
    for (way,what) in ["checkpoint restore","replay (the checkpoints deleted with state.redb)","history.redb deleted and rebuilt"].iter().enumerate(){
        let c=dir(&format!("restart-{way}"));copy_dir(&d,&c);
        match way{1=>std::fs::remove_file(c.join(pocold::state_store::FILE)).unwrap(),2=>std::fs::remove_file(c.join(pocold::history::FILE)).unwrap(),_=>{}}
        let mut a=Node::open(conf(&c,store(),None),spec_of(&r.cfg)).unwrap();
        assert_eq!(a.stats.resumed_from.is_some(),way==0,"{what}: resumed from a checkpoint (refused {}, released {})",a.stats.checkpoints_refused,a.stats.pins_released);
        if way==2{assert!(a.stats.history_refilled>=301,"{what}: every block refilled ({})",a.stats.history_refilled);}
        let f=pocold::checkpoint::fingerprint(&a.chain).unwrap();match &live{None=>live=Some(f),Some(l)=>assert_eq!(&f,l,"{what}: the chain's fields, as restored")}
        let mut rr=reference();
        for h in 1..=301u64{let x=entry_at(&r.chain,h).hash;assert!(a.history.has(&x).unwrap(),"{what}: h={h} stored");
            assert_eq!(a.history.members(&x).unwrap().unwrap().2,rec(&rr.chain,h),"{what}: the stored record at {h}");
            if let Some(e)=a.chain.get(&x){if e.height>0{assert_eq!(a.chain.member_rec_of(e).unwrap(),rec(&rr.chain,h),"{what}: the held record at {h}");}}}
        judge_next(&mut rr,&mut a,&old,&late[..2],what);
        drop(a);let _=std::fs::remove_dir_all(&c);
    }
    let _=std::fs::remove_dir_all(&d);
}

/// §5.9: a history.redb written before FN-3 (its receipts, transaction index and ranges, no member tables) is not set
/// aside: a checkpoint restore finds no member rows (`has`), falls back to replay, and replay refills them. A flipped
/// byte in a member row marks the store damaged when it is read, and the next open rebuilds it.
#[test] fn a_pre_fn3_history_is_refilled_by_replay_and_a_damaged_member_row_rebuilds(){
    let mut r=net(2);grow(&mut r,70);
    let blocks:Vec<Block>=(1..=70u64).map(|h|{let e=entry_at(&r.chain,h);Block{header:e.header.clone(),body:e.body.clone()}}).collect();
    let d=dir("s4");
    let live={let mut a=Node::open(conf(&d,store(),None),spec_of(&r.cfg)).unwrap();for b in &blocks{assert!(a.import(b.clone(),None).unwrap());}assert!(a.stats.checkpoints_written>0);pocold::checkpoint::fingerprint(&a.chain).unwrap()};
    // The S4 format: the member tables removed (an older build never made them).
    {let db=redb::Database::open(d.join(pocold::history::FILE)).unwrap();let w=db.begin_write().unwrap();
        for t in ["members","member_sets"]{let def:redb::TableDefinition<&[u8;32],&[u8]>=redb::TableDefinition::new(t);assert!(w.delete_table(def).unwrap(),"{t} existed");}w.commit().unwrap();}
    {let a=Node::open(conf(&d,store(),None),spec_of(&r.cfg)).unwrap();
        assert!(!a.history.rebuilt,"not set aside");assert!(a.stats.resumed_from.is_none(),"no checkpoint restores without member rows");
        assert!(a.stats.history_refilled>=70,"replay refilled every block ({})",a.stats.history_refilled);
        for b in &blocks{assert!(a.history.has(&b.hash()).unwrap());assert!(a.history.members(&b.hash()).unwrap().is_some());}
        assert_eq!(pocold::checkpoint::fingerprint(&a.chain).unwrap(),live);members_sweep::sweep_node(&a);}
    // A later open resumes from a checkpoint again.
    {let a=Node::open(conf(&d,store(),None),spec_of(&r.cfg)).unwrap();assert!(a.stats.resumed_from.is_some());assert_eq!(pocold::checkpoint::fingerprint(&a.chain).unwrap(),live);}
    // A flipped byte in block 5's member record (an offloaded block: no restore holds it, a member walk reads it).
    let h5=blocks[4].hash();
    {let db=redb::Database::open(d.join(pocold::history::FILE)).unwrap();let w=db.begin_write().unwrap();
        {let def:redb::TableDefinition<&[u8;32],&[u8]>=redb::TableDefinition::new("members");let mut t=w.open_table(def).unwrap();
         let mut v=t.get(&h5).unwrap().unwrap().value().to_vec();let k=v.len()-1;v[k]^=0x40;t.insert(&h5,v.as_slice()).unwrap();}w.commit().unwrap();}
    // Either the open's replay meets it (a local failure: the open stops) or a reader does; the store is marked damaged.
    match Node::open(conf(&d,store(),None),spec_of(&r.cfg)){
        Err(e)=>assert!(e.contains("member"),"{e}"),
        Ok(a)=>{a.chain.clear_member_caches();let mut held=a.chain.canonical_at(5).map(|e|e.clone()).unwrap_or_else(||{let e=entry_at(&r.chain,5);e.clone()});held.members=None;
            assert!(matches!(a.chain.member_rec_of(&held),Err(Reject::Local(_))),"the row fails its seal: local");assert!(a.history.is_damaged());}
    }
    assert!(d.join(pocold::history::DAMAGED).exists(),"the store is marked damaged");
    {let a=Node::open(conf(&d,store(),None),spec_of(&r.cfg)).unwrap();assert!(a.history.rebuilt,"the next open rebuilds it");assert_eq!(a.history.members(&h5).unwrap().unwrap().2,rec(&r.chain,5));
        assert_eq!(pocold::checkpoint::fingerprint(&a.chain).unwrap(),live);members_sweep::sweep_node(&a);}
    let _=std::fs::remove_dir_all(&d);
}

/// §5.9 (io-fault): an I/O error while history.redb's tables (the member tables among them) are opened keeps the file
/// and fails the open; the next open refills the member rows of a pre-FN-3 file without setting it aside.
#[cfg(feature="io-fault")]
#[test] fn an_io_fault_opening_the_member_tables_keeps_the_file(){
    use pocold::io_fault::{self,Op};
    let mut r=net(2);grow(&mut r,20);
    let d=dir("iof");let blocks:Vec<Block>=(1..=20u64).map(|h|{let e=entry_at(&r.chain,h);Block{header:e.header.clone(),body:e.body.clone()}}).collect();
    {let mut a=Node::open(conf(&d,store(),None),spec_of(&r.cfg)).unwrap();for b in &blocks{a.import(b.clone(),None).unwrap();}}
    {let db=redb::Database::open(d.join(pocold::history::FILE)).unwrap();let w=db.begin_write().unwrap();
        for t in ["members","member_sets"]{let def:redb::TableDefinition<&[u8;32],&[u8]>=redb::TableDefinition::new(t);w.delete_table(def).unwrap();}w.commit().unwrap();}
    let hist=d.join(pocold::history::FILE);let size=std::fs::metadata(&hist).unwrap().len();
    for op in [Op::Write,Op::Sync]{
        io_fault::arm(&hist,"history-open-tables",op);
        let e=match Node::open(conf(&d,store(),None),spec_of(&r.cfg)){Err(e)=>e,Ok(_)=>panic!("the open succeeded")};
        let fired=io_fault::fired(&hist);io_fault::disarm(&hist);assert!(!fired.is_empty(),"positive control: the fault fired");
        assert!(e.contains("I/O failure"),"{e}");assert!(hist.exists()&&std::fs::metadata(&hist).unwrap().len()>=size,"the file is kept");
    }
    let a=Node::open(conf(&d,store(),None),spec_of(&r.cfg)).unwrap();assert!(!a.history.rebuilt);
    for b in &blocks{assert!(a.history.members(&b.hash()).unwrap().is_some(),"refilled");}
    drop(a);let _=std::fs::remove_dir_all(&d);
}

/// §5.7: a reorg between sibling branches whose member sets differ (a registration on one), with the fork at the
/// snapshot height 257 and with a branch crossing it: the new branch's evidence against ancestors above and below the
/// fork is judged as on a chain that only ever held that branch.
#[test] fn a_reorg_across_the_snapshot_height_judges_as_the_new_branch_alone(){
    for fork in [257u64,250]{
        let mut n=net(2);let alice=devnet::account_key("alice").address();grow(&mut n,fork);
        let f=n.chain.head;
        // Branch A: a registration right after the fork (S_b + alice from fork + 2), 6 blocks.
        let mut a=vec![];let t=register(&n,alice);let b=mine(&n,f,vec![t],vec![],None);accept(&mut n,&b);a.push(b);
        for _ in 0..5{let b=mine(&n,n.chain.head,vec![],vec![],None);accept(&mut n,&b);a.push(b);}
        let head_a=n.chain.head;
        // Branch B from the fork: 9 blocks, heavier; it takes the head.
        let mut b_blocks=vec![];let mut p=f;
        for _ in 0..9{let b=mine(&n,p,vec![],vec![],None);p=b.hash();let (e,_)=n.chain.validate(&b,None,0).unwrap();n.chain.insert(e);b_blocks.push(b);}
        assert_ne!(n.chain.head,head_a);assert_eq!(n.chain.head,p,"fork choice took branch B");n.chain.retain();
        assert_ne!(rec(&n.chain,fork+3),n.chain.member_rec_of(n.chain.get(&a[2].hash()).unwrap()).unwrap(),"the branches' records differ");
        // The reference: the shared prefix and branch B only.
        let mut r=net(2);for h in 1..=fork{let e=entry_at(&n.chain,h);accept(&mut r,&Block{header:e.header.clone(),body:e.body.clone()});}for b in &b_blocks{accept(&mut r,b);}
        assert_eq!(r.chain.head,n.chain.head);
        n.chain.clear_member_caches();
        for j in [fork-20,fork-1,fork,fork+1,fork+5]{
            let x=entry_at(&n.chain,j);let blk=Block{header:x.header.clone(),body:x.body.clone()};let (ev,_)=type2(&n,&blk);
            assert_eq!(effects(&n.chain,&[ev.clone()]),effects(&r.chain,&[ev]),"fork {fork}: evidence against {j}");
        }
        // A block on branch B judged the same.
        let next=mine(&r,r.chain.head,vec![],vec![],None);assert_eq!(n.chain.validate(&next,None,0).map(|x|x.0.hash),r.chain.validate(&next,None,0).map(|x|x.0.hash));
        // Back to branch A, the registration's, extended until it is heavier: its member sets differ from B's from
        // fork + 2. The caches hold what B's readers filled while B was canonical and nothing is cleared before the first
        // pass, so the new branch's readers meet them; the second pass compares again from cleared caches.
        let mut tip=a.last().unwrap().hash();let mut a_ext=vec![];
        while n.chain.head!=tip{n.chain.ensure_state(&tip).unwrap();let b=mine(&n,tip,vec![],vec![],None);tip=b.hash();let (e,_)=n.chain.validate(&b,None,0).unwrap();n.chain.insert(e);a_ext.push(b);
            assert!(a_ext.len()<=12,"fork {fork}: branch A takes the head back");}
        n.chain.retain();
        let mut r2=net(2);for h in 1..=fork{let e=entry_at(&n.chain,h);accept(&mut r2,&Block{header:e.header.clone(),body:e.body.clone()});}for b in a.iter().chain(&a_ext){accept(&mut r2,b);}
        assert_eq!(r2.chain.head,n.chain.head);
        assert!(n.chain.member_set(entry_at(&n.chain,fork+2)).unwrap().contains(&alice.0.0)&&!r.chain.member_set(entry_at(&r.chain,fork+2)).unwrap().contains(&alice.0.0),"positive control: the new head's member set differs from B's at fork + 2");
        let top=n.chain.head().height;
        for pass in ["with the caches filled under branch B","from cleared caches"]{
            for j in [fork-20,fork-1,fork,fork+1,fork+2,fork+3,top-1,top]{
                let x=entry_at(&n.chain,j);let blk=Block{header:x.header.clone(),body:x.body.clone()};let (ev,_)=type2(&n,&blk);
                assert_eq!(effects(&n.chain,&[ev.clone()]),effects(&r2.chain,&[ev]),"fork {fork}, {pass}: evidence against {j} on branch A");
            }
            let next=mine(&r2,r2.chain.head,vec![],vec![],None);assert_eq!(n.chain.validate(&next,None,0).map(|x|x.0.hash),r2.chain.validate(&next,None,0).map(|x|x.0.hash),"fork {fork}, {pass}: a block on branch A");
            n.chain.clear_member_caches();
        }
        let m=n.chain.member_counters();assert!(m.oracle_mismatch==0&&m.faults==0);
    }
}

/// §5.7 (store mode): a deep rebuild with the caches cleared re-executes blocks whose member record was replaced by one
/// that is sealed but does not describe the block (its count off): the re-execution's check is a local failure, the
/// store is marked damaged, and the next open rebuilds it.
#[test] fn a_wrong_member_record_found_by_a_rebuild_is_local_and_rebuilt(){
    let mut r=net(2);grow(&mut r,40);
    let blocks:Vec<Block>=(1..=40u64).map(|h|{let e=entry_at(&r.chain,h);Block{header:e.header.clone(),body:e.body.clone()}}).collect();
    let d=dir("wrong");
    let live={let mut a=Node::open(conf(&d,Limits{state_store:true,checkpoint_every:0,..Default::default()},None),spec_of(&r.cfg)).unwrap();for b in &blocks{assert!(a.import(b.clone(),None).unwrap());}pocold::checkpoint::fingerprint(&a.chain).unwrap()};
    let h5=blocks[4].hash();
    {let db=redb::Database::open(d.join(pocold::history::FILE)).unwrap();let w=db.begin_write().unwrap();
        {let def:redb::TableDefinition<&[u8;32],&[u8]>=redb::TableDefinition::new("members");let mut t=w.open_table(def).unwrap();
         let v=t.get(&h5).unwrap().unwrap().value().to_vec();let (p,h,mut m)=members::decode_rec(&v[32..]).unwrap();m.n+=1;
         let rec=members::encode_rec(&p,h,&m);let mut b=h5.to_vec();b.extend_from_slice(&rec);let mut val=keccak256(&b).to_vec();val.extend_from_slice(&rec);t.insert(&h5,val.as_slice()).unwrap();}
        w.commit().unwrap();}
    // The import oracle's cold walk during the open's replay, or a rebuild's re-execution, meets it: either way a local
    // failure (the open stops, or the rebuild fails), never a verdict, and the store is marked damaged.
    match Node::open(conf(&d,Limits{state_store:true,checkpoint_every:0,..Default::default()},None),spec_of(&r.cfg)){
        Err(e)=>assert!(e.contains("Local")&&e.contains("member"),"{e}"),
        Ok(mut a)=>{a.chain.clear_member_caches();
            let target=a.chain.canonical_at(6).unwrap().hash;assert!(a.chain.canonical_at(6).unwrap().pruned,"positive control: block 6's state was pruned");
            for e in a.chain.blocks.values_mut(){if e.height>0&&e.height<30{e.members=None;}}
            match a.rebuild_state(&target){Err(Reject::Local(w))=>assert!(w.contains("member record"),"{w}"),r=>panic!("{r:?}")}
            assert!(a.history.is_damaged(),"marked damaged");}
    }
    assert!(d.join(pocold::history::DAMAGED).exists(),"the store is marked damaged");
    {let a=Node::open(conf(&d,Limits{state_store:true,checkpoint_every:0,..Default::default()},None),spec_of(&r.cfg)).unwrap();assert!(a.history.rebuilt,"rebuilt");
        assert_eq!(a.history.members(&h5).unwrap().unwrap().2,rec(&r.chain,5),"the true record again");
        assert_eq!(pocold::checkpoint::fingerprint(&a.chain).unwrap(),live);members_sweep::sweep_node(&a);}
    let _=std::fs::remove_dir_all(&d);
}

/// §5.12: a pool of type-2 items against distinct ancestors: the second template's evaluation is served by the owner
/// memo; and an item whose check fails locally (the member source failing) stays in the pool.
#[test] fn the_proposers_pool_uses_the_owner_memo_and_keeps_items_on_a_local_failure(){
    let mut r=net(2);grow(&mut r,70);
    let blocks:Vec<Block>=(1..=70u64).map(|h|{let e=entry_at(&r.chain,h);Block{header:e.header.clone(),body:e.body.clone()}}).collect();
    let p=r.chain.full_entry(&r.chain.head).unwrap();let proposer=r.chain.child_ctx(&p,0).unwrap().proposer;
    let d=dir("pool");let mut a=Node::open(conf(&d,Limits::default(),Some(Key::from_secret(&r.keys[&proposer].secret()).unwrap())),spec_of(&r.cfg)).unwrap();
    for b in &blocks{assert!(a.import(b.clone(),None).unwrap());}
    let items:Vec<Vec<u8>>=(70-63..=70u64).map(|j|type2(&r,&blocks[j as usize-1]).0).collect();assert_eq!(items.len(),64);
    a.evidence_pool=items.clone();a.chain.clear_member_caches();
    let pe=a.chain.full_entry(&a.chain.head).unwrap();
    let t=std::time::Instant::now();let first=a.build_template_unrecorded(&pe,Default::default(),pe.ts+4).expect("a template");let cold=t.elapsed();
    let m1=a.chain.member_counters();
    let t=std::time::Instant::now();let second=a.build_template_unrecorded(&pe,Default::default(),pe.ts+4).expect("a template");let warm=t.elapsed();
    let m2=a.chain.member_counters();
    println!("FN3-POOL {}",serde_json::json!({"items":64,"coldMs":cold.as_secs_f64()*1e3,"warmMs":warm.as_secs_f64()*1e3,"ownerHits":m2.owner_hits-m1.owner_hits}));
    assert!(m2.owner_hits>m1.owner_hits,"the second template's owners come from the memo");
    assert_eq!(first.1.evidence,second.1.evidence);assert!(!first.1.evidence.is_empty()&&a.evidence_pool.len()==64);
    // The member source fails: every item's check is local, and each is kept.
    let src=TestSets::of(&a.chain);src.bad.store(2,Ordering::SeqCst);a.chain.set_member_source(src);
    for e in a.chain.blocks.values_mut(){if e.height>0&&e.height<69{e.members=None;}}
    let kept=a.stats.evidence_kept_local;let _=a.build_template_unrecorded(&pe,Default::default(),pe.ts+4);
    assert_eq!(a.evidence_pool.len(),64,"no item dropped on a local failure");assert!(a.stats.evidence_kept_local>=kept+64);
    drop(a);let _=std::fs::remove_dir_all(&d);
}

/// §5.2 (on a chain): a record round-trips through history.redb exactly as the chain holds it, and its set adds up.
#[test] fn history_member_rows_are_the_chains(){
    let mut r=net(2);let alice=devnet::account_key("alice").address();grow(&mut r,3);let t=register(&r,alice);let b=mine(&r,r.chain.head,vec![t],vec![],None);accept(&mut r,&b);grow(&mut r,10);
    let d=dir("rows");let mut a=Node::open(conf(&d,Limits::default(),None),spec_of(&r.cfg)).unwrap();
    for h in 1..=10u64{let e=entry_at(&r.chain,h);assert!(a.import(Block{header:e.header.clone(),body:e.body.clone()},None).unwrap());}
    for h in 1..=10u64{let e=entry_at(&r.chain,h);let (p,hh,m)=a.history.members(&e.hash).unwrap().unwrap();assert_eq!((p,hh,&m),(e.parent.unwrap(),h,&rec(&r.chain,h)));
        let set=a.history.member_set(&e.hash).unwrap();assert_eq!(set.is_some(),m.is_snapshot());
        if let Some(s)=set{assert_eq!(s,r.chain.member_set(e).unwrap());}
        if let MemberStep::Delta{adds,..}=&m.step{if !adds.is_empty(){assert_eq!(&adds[..],&[alice.0.0][..]);}}}
    members_sweep::sweep_node(&a);drop(a);let _=std::fs::remove_dir_all(&d);
}

/// §5.4: a carrier at h whose kind-3 item is for template j = h − D and whose type-2 item is against an older block, on a
/// node slimming at depth 2 (below D): both ancestors are slimmed, restored for the evidence walk, and the carrier is
/// accepted with the effects and state root of a node that never slims; the shadow compared at the restored ancestors.
#[test] fn kind3_at_h_minus_d_and_type2_against_slimmed_ancestors_restore_them_first(){
    let mut r=net(2);let two=r.ids[1];let d=u64::from(r.chain.spec.cp().D);assert!(d>2,"positive control: D {d} above the slim depth 2");
    grow(&mut r,20);let bj=mine(&r,r.chain.head,vec![],vec![],Some(two));accept(&mut r,&bj);let j=bj.header.h;
    let late=omitted(&r,&bj,two);assert!(late.len()>=2,"two's shares were left out: {late:?}");
    grow(&mut r,j+d-1);let h=j+d;
    // A type-2 item against a block whose winner is not the carrier's proposer.
    let p=r.chain.full_entry(&r.chain.head).unwrap();let proposer=r.chain.child_ctx(&p,0).unwrap().proposer;
    let (ev2,j2)=(10..j).rev().find_map(|x|{let e=entry_at(&r.chain,x);let blk=Block{header:e.header.clone(),body:e.body.clone()};let (ev,acc)=type2(&r,&blk);(acc!=proposer).then_some((ev,x))}).expect("a winner other than the proposer");
    let carrier=mine(&r,r.chain.head,vec![],vec![ev2,late_encode(j,&late[..2])],None);assert_eq!(carrier.header.h,h);
    let want=r.chain.evidence_effects(&p,&carrier.body,h).unwrap();assert_eq!(want.len(),2,"both items have an effect: {want:?}");
    let blocks:Vec<Block>=(1..h).map(|x|{let e=entry_at(&r.chain,x);Block{header:e.header.clone(),body:e.body.clone()}}).collect();
    let open=|tag:&str,depth:Option<u64>|{let dd=dir(tag);(Node::open(conf(&dd,Limits{slim_depth:depth,..Default::default()},None),spec_of(&r.cfg)).unwrap(),dd)};
    let (mut a,da)=open("k3slim",Some(2));let (mut b,db)=open("k3ref",None);
    for x in &blocks[..blocks.len()-1]{assert!(a.import(x.clone(),None).unwrap());assert!(b.import(x.clone(),None).unwrap());}
    let c0=a.chain.member_counters().oracle_checks;let x=blocks.last().unwrap().clone();assert!(a.import(x.clone(),None).unwrap());assert!(b.import(x,None).unwrap());
    let plain=a.chain.member_counters().oracle_checks-c0;assert!(plain>=1,"positive control: the import oracle compared");
    for (x,what) in [(j,"the kind-3 template"),(j2,"the type-2 ancestor")]{assert!(a.chain.canonical_at(x).unwrap().slim,"positive control: {what} at {x} is slimmed");assert!(!b.chain.canonical_at(x).unwrap().slim);}
    a.chain.clear_member_caches();let (u,c1)=(a.stats.unslims,a.chain.member_counters().oracle_checks);
    assert!(a.import(carrier.clone(),None).unwrap());assert!(b.import(carrier.clone(),None).unwrap());
    assert!(a.stats.unslims>u,"the slimmed ancestors were restored for the evidence walk");
    assert_eq!((a.chain.head,b.chain.head),(carrier.hash(),carrier.hash()));
    assert_eq!(apply::state_root(&a.chain.head().state).unwrap(),apply::state_root(&b.chain.head().state).unwrap());
    // The shadow: kind-3's two owners and its template's assignment, type-2's owner and assignment, beyond the import's own.
    let got=a.chain.member_counters().oracle_checks-c1;assert!(got>=plain+5,"the shadow compared at the restored ancestors: {got} checks, an import alone {plain}");
    let (_,_,s)=members_sweep::sweep_node(&a);assert!(s>0,"positive control: slimmed entries compared");members_sweep::sweep_node(&b);
    drop(a);drop(b);for dd in [da,db]{let _=std::fs::remove_dir_all(&dd);}
}

/// §5.5, risk 7 [C1-D7]: a store-mode node at exactly `evict_depth_of`'s minimum E, restarted from a checkpoint of a
/// window based above a snapshot height.
/// (a) After the restore, with the caches cleared, the walk of the lowest window entry whose parent is held crosses
///     canon_base: it reads the records of evicted blocks and the evicted snapshot's set from history.redb
///     (`member_src`), and its set and assignment are the reference's and history.redb's ranges.
/// (b) A fork child 40 below the head (within R_keep, its parent's state pruned) is accepted after a rebuild from the
///     newest anchor below it, which re-executes a block carrying type-2 evidence against an ancestor E_win − 1 below
///     it: that ancestor's parent (its seal) is held, its walk reads offloaded records and its snapshot's set from
///     history.redb, and the shadow compared. An anchor rebuild's evidence walks stay above the floor: the oldest block
///     re-executed is above the newest anchor below the fork (at least head − 2·R_keep), its evidence reaches E_win
///     below that, a walk 255 below that, and E_min ≥ 2·R_keep + E_win + 258. A walk below the floor is (a)'s, or a
///     bare chain's (`a_bare_chain_judges_evidence_whose_snapshot_was_evicted`).
/// (c) A following carrier with type-2 and kind-3 evidence is judged as the reference judges it.
#[test] fn a_minimum_depth_store_node_walks_below_its_window_after_a_restore_and_rebuilds_from_an_anchor(){
    let mut r=net(2);let two=r.ids[1];let rk=64u64;
    let spec=spec_of(&r.cfg);let (e_win,k_hist)=(u64::from(spec.cp().E_win),spec.local().k_hist);
    // The default slim depth (2·R_keep + E_win + 257): an evidence walk never meets a slimmed entry, and E_min is 2·R_keep +
    // E_win + 258 (above the store-mode anchor term at K_hist 32).
    let base_l=Limits{r_keep:rk,side_cap:rk as usize,state_store:true,checkpoint_every:7,..Default::default()};
    // E_min: the least depth `evict_depth_of` accepts (the default, None, is the slim depth + R_keep, above it).
    let e_min=2*rk+e_win+258;let depth=|n:u64|pocold::resources::evict_depth_of(spec.cp(),k_hist,&Limits{evict_depth:Some(n),..base_l.clone()},rk);
    assert_eq!(depth(e_min),Ok(Some(e_min)));assert!(depth(e_min-1).is_err(),"positive control: {e_min} is the minimum at R_keep {rk}, E_win {e_win}");
    let l=Limits{evict_depth:Some(e_min),..base_l};
    // The carrier x of type-2 evidence against x − (E_win − 1), at the first height from 700 whose proposer is not the
    // accused; the head 40 above it, so the snapshot at 257 is below the window.
    grow(&mut r,699);
    let x=loop{let p=r.chain.full_entry(&r.chain.head).unwrap();let ctx=r.chain.child_ctx(&p,0).unwrap();let h=p.height+1;
        let e=entry_at(&r.chain,h-(e_win-1));let (ev,accused)=type2(&r,&Block{header:e.header.clone(),body:e.body.clone()});
        if ctx.proposer!=accused{let b=mine(&r,r.chain.head,vec![],vec![ev],None);accept(&mut r,&b);break h}
        assert!(h<720,"an eligible proposer");let b=mine(&r,r.chain.head,vec![],vec![],None);accept(&mut r,&b);};
    let j=x-(e_win-1);let top=x+40;grow(&mut r,top-1);
    let bt=mine(&r,r.chain.head,vec![],vec![],Some(two));accept(&mut r,&bt);let late=omitted(&r,&bt,two);assert!(late.len()>=2);
    assert!(top-e_min>257&&(top-e_min+1)%256!=1,"positive control: the window's base {} is above the snapshot at 257",top-e_min);
    let blocks:Vec<Block>=(1..=top).map(|h|{let e=entry_at(&r.chain,h);Block{header:e.header.clone(),body:e.body.clone()}}).collect();
    let d=dir("emin");
    {let mut a=Node::open(conf(&d,l.clone(),None),spec_of(&r.cfg)).unwrap();assert_eq!(a.cfg.limits.evict_depth,Some(e_min));
        for b in &blocks{assert!(a.import(b.clone(),None).unwrap());}assert!(a.stats.checkpoints_written>0);}
    let mut a=Node::open(conf(&d,l.clone(),None),spec_of(&r.cfg)).unwrap();
    assert!(a.stats.resumed_from.is_some(),"positive control: resumed from a checkpoint");assert_eq!(a.chain.head().height,top);
    let base=a.chain.canonical.base();assert!(base>257&&a.chain.get(&entry_at(&r.chain,257).hash).is_none(),"positive control: the window's base {base} is above the evicted snapshot at 257");
    let (off,_)=members_sweep::held_records(&a.chain);assert!(off>0,"positive control: offloaded entries");
    // (a) The lowest held canonical entry whose parent is held: its walk goes through its parent, then below canon_base.
    let lo=(base..top).find(|h|{let x=entry_at(&r.chain,*h);a.chain.get(&x.hash).is_some()&&a.chain.get(&x.parent.unwrap()).is_some()}).unwrap();
    assert!(a.chain.get(&entry_at(&r.chain,lo-1).parent.unwrap()).is_none(),"positive control: the walk from {lo} leaves the window");
    a.chain.clear_member_caches();let m0=a.chain.member_counters();
    let e=a.chain.get(&entry_at(&r.chain,lo).hash).unwrap().clone();
    assert_eq!(a.chain.member_set(&e).unwrap(),r.chain.member_set(entry_at(&r.chain,lo)).unwrap(),"the set walked below the window");
    let got=a.chain.assignment_of(&e).unwrap();let restored=a.restored_entry(&e.hash).unwrap();
    assert!(pocold::chain::ranges_equal(&restored.ranges,&got)&&pocold::chain::ranges_equal(&entry_at(&r.chain,lo).ranges,&got),"the assignment recomputed across canon_base");
    let m1=a.chain.member_counters();
    assert!(m1.record_reads-m0.record_reads>=lo-257&&m1.disk_set_reads>m0.disk_set_reads,"the walk read {} records and {} sets from history.redb",m1.record_reads-m0.record_reads,m1.disk_set_reads-m0.disk_set_reads);
    // (b) The anchor rebuild.
    let canon=entry_at(&r.chain,x+1);let winner=canon.result.as_ref().unwrap().winner;let child=mine(&r,entry_at(&r.chain,x).hash,vec![],vec![],Some(winner));
    assert_ne!(child.hash(),canon.hash);assert!(r.chain.validate(&child,None,0).is_ok(),"the reference accepts the fork child");
    let root=a.chain.anchors().iter().map(|a|a.0).filter(|h|*h<x).max().expect("an anchor below the fork");
    assert!(root<x&&x-root<=rk&&j>root.saturating_sub(e_win)&&j>base,"positive control: the rebuild from {root} re-executes the carrier at {x}, whose ancestor {j} is in the window");
    assert!(a.chain.canonical_at(x).unwrap().pruned,"positive control: the fork point's state is pruned");
    a.chain.clear_member_caches();let (m0,rb)=(a.chain.member_counters(),a.chain.rebuilds);
    a.import(child.clone(),None).unwrap();assert!(a.chain.get(&child.hash()).is_some(),"the fork child is accepted");assert_eq!(a.chain.rebuilds-rb,x-root,"the rebuild starts at the newest anchor below the fork");
    let m1=a.chain.member_counters();
    assert!(m1.record_reads>m0.record_reads&&m1.disk_set_reads>m0.disk_set_reads,"the re-executed evidence's walk read history.redb: {m1:?}");
    assert!(m1.oracle_checks>=m0.oracle_checks+3&&m1.oracle_mismatch==0&&m1.faults==0,"the shadow compared at the re-executed evidence: {m1:?}");
    // (c) A following carrier of type-2 and kind-3 evidence.
    let old=Block{header:entry_at(&r.chain,top-60).header.clone(),body:entry_at(&r.chain,top-60).body.clone()};
    judge_next(&mut r,&mut a,&old,&late[..2],"after the restore and the anchor rebuild");
    let (off,_)=members_sweep::held_records(&a.chain);assert!(off>0);
    drop(a);let _=std::fs::remove_dir_all(&d);
}
