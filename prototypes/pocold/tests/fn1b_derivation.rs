//! FN-1b (C6, node-local): a new parent's membership inputs (`members::MemberBase`: the ActList and the record fields
//! membership() reads) are derived from its own parent's and the block's registry writes, instead of re-reading every
//! record of its state. The one-pass reading (`Chain::membership_one_pass`) stays the oracle, and nothing here changes a
//! verdict: every comparison below is against it (and the pre-FN-1 two-pass reading).
//! - A derivation over synthetic registry storage (appends, swap-removes, shrinks, record changes and deletions, tombs,
//!   bond and clock changes, duplicates and reserved ids, unrelated and unchanged writes) equals the whole reading, and
//!   omitting a written slot makes it differ (the negative control).
//! - On chains (v3 all-live and the spec-1 activity window): registrations (bonded and an unbonded probationer, its bond
//!   filled by its wins: ACCRUING below and above B_reg/4, then HELD), an exit, a type-2 punishment, a registerReplacing at
//!   the full set, across the snapshot height 257, a reorg to a branch whose member set differs and back, and a restart:
//!   every new head's cold context derives its inputs, and equals the one-pass and two-pass readings.
//! - A store-mode node (slim, offload, checkpoints): every import's cold context derives; a rebuild derives along its
//!   rolling re-execution; after a reopen from a checkpoint (restore) the first context reads its inputs whole, after a
//!   replay it derives (the replay re-validated every block), and every import after either derives; every context
//!   equals the reference's one-pass reading.
//! - Inputs held wrong (`Chain::damage_member_base`) are caught by the next context's comparison: on a chain that tolerates
//!   it (`Chain::tolerate_base_mismatch`) the one-pass result is returned, the mismatch and a member fault are counted,
//!   every held input is dropped, the next context reads whole; on any other chain it panics, so no oracle-on test passes
//!   over one. A write set that leaves out the slots its block wrote (`Chain::withhold_next_registry_writes`) is caught
//!   at import and not kept; a write set above the cap (`Chain::set_ctx_write_slots_max`) is not kept; either way the
//!   next context reads its inputs whole.
//! The oracle (on in every test) also compares, on every block executed, the inputs derived through its writes with the
//! ones read whole from its state; the cold sweep asserts no mismatch and no fault.
#[path="support/members.rs"] mod members_sweep;
use std::collections::HashMap;
use std::path::{Path,PathBuf};
use pocol_lp4_execution::{apply,devnet::{self,DevnetConfig},txsign::{Key,Tx}};
use pocold::chain::{Chain,Spec};
use pocold::members::MemberBase;
use pocold::node::{Config,Limits,Node};
use pocold::{hashes,rlp,types::*};
use revm::primitives::{keccak256,Address,Bytes,B256,U256};

struct Net{chain:Chain,keys:HashMap<Address,Key>,cfg:DevnetConfig,ids:Vec<Address>,blocks:Vec<Block>}
impl Drop for Net{fn drop(&mut self){members_sweep::sweep_on_drop(&self.chain);}}
/// The earn-in devnet, v3 (CP schema v2: all-live membership, full nonce domain) or spec 1 (bounded nonces, the W_act
/// activity window). M_max `m_max`; E_win 80.
fn config(spec:u8,m_max:u16)->DevnetConfig{
    // Phase 5a: spec 2 is the v3 devnet default; spec 1 its retired earn-in twin (Phase 6a deletes the spec-1 rule set).
    let mut c=if spec>=2{DevnetConfig::new(777954,1_700_000_000,&["one","two","three"])}
        else{DevnetConfig::spec1(777954,1_700_000_000,&["one","two","three"],devnet::Spec1Programs::EarnIn)};
    c.hash_rate=16;c.c=8;c.d_att=73;c.r_max=1;c.e_win=80;c.u=96;c.c_react=96;c.w_act=4;c.m_max=m_max;
    c.accounts.push(("carol".into(),U256::from(10u128.pow(24))));c
}
fn spec_of(c:&DevnetConfig)->Spec{members_sweep::oracle();Spec::new(devnet::build(c).unwrap().admit(&pocold::single_host_local()).unwrap_or_else(|e|panic!("{e}")).0)}
fn net(cfg:DevnetConfig)->Net{
    let d=devnet::build(&cfg).unwrap();
    let mut keys:HashMap<Address,Key>=d.miners.iter().map(|m|(m.id,Key::from_secret(&m.key.secret()).unwrap())).collect();
    for label in ["alice","bob","carol"]{let k=devnet::account_key(label);keys.insert(k.address(),k);}
    let ids=d.miners.iter().map(|m|m.id).collect();
    Net{chain:Chain::new(spec_of(&cfg)),keys,cfg,ids,blocks:vec![]}
}
const SCAN:u128=1<<12;
fn scan(ctx:&pocold::chain::ChildCtx)->impl Iterator<Item=u64>+'_{ctx.assignment.ranges().iter().flat_map(|r|(r.start()..r.end_exclusive().min(r.start()+SCAN)).map(|x|x as u64))}
/// A signed block on `parent` by Draw(h,0) carrying `txs` and `evidence`, won by the first solution scanned whose owner
/// `keep` accepts (with up to 8 of its shares).
fn mine_where(n:&Net,parent:[u8;32],txs:Vec<Vec<u8>>,evidence:Vec<Vec<u8>>,keep:&dyn Fn(Address)->bool)->Option<Block>{
    let p=n.chain.full_entry(&parent).unwrap();let ctx=n.chain.child_ctx(&p,0).unwrap();
    let body=Body{txs,evidence};let effects=n.chain.evidence_effects(&p,&body,p.height+1).unwrap();
    for ts in p.ts+u64::from(n.chain.spec.cp().T_blk)..=ctx.window.upper(){
        let (mut h,_,_)=n.chain.build_signed(&p,&n.keys[&ctx.proposer],ts,body.clone(),effects.clone()).unwrap();
        let tid=template_id(&h.ut_raw);let (mut shares,mut win)=(vec![],None);
        for nonce in scan(&ctx){
            let ph=pow_hash(&tid,nonce);let owner=addr(&ctx.assignment.owner(nonce).unwrap());if !keep(owner){continue}
            if le_target(&ph,&ctx.target){if win.is_none(){win=Some((nonce,owner));}}else if le_target(&ph,&ctx.t_share)&&shares.len()<8{shares.push(nonce);}
        }
        let Some((nonce,owner))=win else{continue};
        shares.sort();h.nonce=nonce;h.shares=shares;reseal(&mut h);
        h.winner_sig=n.keys[&owner].sign_hash(&B256::from(hashes::win_msg(&tid,nonce,&share_root(&h))));reseal(&mut h);
        return Some(Block{header:h,body});
    }
    None
}
fn mine(n:&Net,parent:[u8;32],txs:Vec<Vec<u8>>,evidence:Vec<Vec<u8>>)->Block{mine_where(n,parent,txs,evidence,&|_|true).expect("a solution")}
fn tx(n:&Net,key:Address,data:Vec<u8>,value:U256)->Vec<u8>{
    let p=n.chain.full_entry(&n.chain.head).unwrap();let base=n.chain.child_ctx(&p,0).unwrap().base_fee;
    let nonce=p.state.accounts.get(&key).map_or(0,|a|a.info.nonce);let prio=U256::from(1_000_000_000u64);
    Tx{chain_id:n.chain.spec.chain_id(),nonce,max_priority:prio,max_fee:base*U256::from(2)+prio,gas:1_000_000,to:Some(apply::registry()),value,data:Bytes::from(data)}.sign_1559(&n.keys[&key])
}
fn register(n:&Net,who:Address,value:U256)->Vec<u8>{let mut d=keccak256("register(address)")[..4].to_vec();d.extend_from_slice(who.into_word().as_slice());tx(n,who,d,value)}
fn deregister(n:&Net,who:Address)->Vec<u8>{tx(n,who,keccak256("deregister()")[..4].to_vec(),U256::ZERO)}
fn replacing(n:&Net,who:Address,victim:Address)->Vec<u8>{
    let mut d=keccak256("registerReplacing(address,address)")[..4].to_vec();d.extend_from_slice(who.into_word().as_slice());d.extend_from_slice(victim.into_word().as_slice());tx(n,who,d,U256::ZERO)
}
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
/// Every receipt of the head carries status 1 (each test transaction took effect).
fn all_succeeded(n:&Net,what:&str){
    let e=n.chain.head();assert!(!e.receipts.is_empty(),"{what}: a transaction");
    for r in &e.receipts{assert_eq!(rlp::decode(&r[1..]).unwrap().list().unwrap()[0].bytes().unwrap(),&[1u8][..],"{what}: a receipt failed");}
}
fn quarter(c:&Chain)->U256{pocold::chain::from_fixed_lp4(&c.spec.cp().B_reg)/U256::from(4)}
/// Imports `b`, then the cold context of the new head (the FN-1 rows cleared, as for a new parent): its inputs are
/// derived (one derivation, no whole reading), and its membership is the one-pass and the two-pass reading's. Returns
/// the head.
fn accept(n:&mut Net,b:Block,what:&str)->[u8;32]{
    let (e,_)=n.chain.validate(&b,None,0).unwrap_or_else(|x|panic!("{what} h={}: {x:?}",b.header.h));let h=e.hash;n.chain.insert(e);n.blocks.push(b);
    cold_derives(&n.chain,&h,what);h
}
fn cold_derives(c:&Chain,h:&[u8;32],what:&str){
    let p=c.full_entry(h).unwrap();let k=p.height+1;
    let before=c.member_counters();c.clear_ctx_cache();
    let ctx=c.child_ctx(&p,0).unwrap_or_else(|x|panic!("{what} h={}: {x:?}",p.height));
    let after=c.member_counters();
    assert_eq!((after.base_derived-before.base_derived,after.base_read-before.base_read),(1,0),"{what} h={}: the new parent's inputs are derived, not read whole",p.height);
    assert_eq!(after.base_mismatch,0,"{what}: no derivation differed");
    assert_eq!(*ctx.membership,c.membership_one_pass(&p,k).unwrap(),"{what} h={}: the one-pass reading",p.height);
    assert_eq!(*ctx.membership,c.membership_two_pass(&p,k).unwrap(),"{what} h={}: the two-pass reading",p.height);
    assert_eq!(*MemberBase::read(&*p.state,quarter(c)).list,*apply::active_list(&*p.state).iter().map(|a|a.0.0).collect::<Vec<_>>(),"{what}: the inputs' list is the ActList");
}

// ---------------- the derivation over synthetic registry storage ----------------
struct Rng(u64);
impl Rng{fn next(&mut self)->u64{self.0^=self.0<<13;self.0^=self.0>>7;self.0^=self.0<<17;self.0}fn below(&mut self,n:u64)->u64{self.next()%n.max(1)}}
fn act_base()->U256{U256::from_be_bytes(keccak256(U256::from(6).to_be_bytes::<32>()).0)}
fn sid(i:u64)->Address{let mut a=[0u8;20];a[0]=0x5e;a[1]=0x1b;a[12..].copy_from_slice(&i.to_be_bytes());Address::from(a)}
/// One storage write of the registry, recorded in the write set.
fn put(db:&mut revm::InMemoryDB,w:&mut Vec<U256>,k:U256,v:U256){db.insert_account_storage(apply::registry(),k,v).unwrap();w.push(k);}
fn get(db:&revm::InMemoryDB,k:U256)->U256{db.accounts.get(&apply::registry()).and_then(|a|a.storage.get(&k).copied()).unwrap_or_default()}
fn record(db:&mut revm::InMemoryDB,w:&mut Vec<U256>,r:&mut Rng,id:Address,b_reg:U256,height:u64){
    let s=apply::record_slot(id);let tomb=r.below(9)==0;let state:u64=[1,2,4,4,3][r.below(5) as usize];
    let last=height.saturating_sub(r.below(8));
    put(db,w,s,U256::from(1u64)|(U256::from(u8::from(tomb))<<8)|(U256::from(state)<<16)|(U256::from(last)<<88));
    put(db,w,s+U256::from(1),U256::from_be_slice(id.as_slice()));
    put(db,w,s+U256::from(2),[U256::ZERO,b_reg/U256::from(8),b_reg/U256::from(4),b_reg][r.below(4) as usize]);
    put(db,w,s+U256::from(3),if r.below(3)==0{U256::from(height.saturating_sub(r.below(6)))<<192}else{U256::ZERO});
}
/// Random registry changes on `db`, as MinerRegistry makes them and beyond (a duplicate or reserved id on the list, a
/// length past the written elements), each step's write set the slots it wrote (some unchanged) and unrelated slots.
/// After each step the inputs derived from the previous ones equal the whole reading, and the membership from them the
/// one-pass reading (or the same error), at several heights; dropping a changed slot from the write set gives inputs
/// that differ (the negative control).
fn synthetic(spec:u8,seed:u64){
    let n=net(config(spec,64));let c=&n.chain;let g=c.full_entry(&c.head).unwrap();let b_reg=pocold::chain::from_fixed_lp4(&c.spec.cp().B_reg);let q=quarter(c);
    let mut db=(*g.state).clone();let mut r=Rng(seed);let (base,one)=(act_base(),U256::from(1));
    let mut prev=MemberBase::read(&db,q);let (mut negatives,mut steps,mut errors)=(0,0,0);let mut longest=0;let mut next_id=1u64;
    for step in 0..400u64{
        let mut w=Vec::new();let len=get(&db,one).as_limbs()[0];let height=step+3;
        match r.below(10){
            0|1=>{let id=sid(next_id);next_id+=1;record(&mut db,&mut w,&mut r,id,b_reg,height);put(&mut db,&mut w,base+U256::from(len),U256::from_be_slice(id.as_slice()));put(&mut db,&mut w,one,U256::from(len+1));}
            2 if len>1=>{// swap-remove at i (removeActive): the last id moves to i and its record word is rewritten
                let i=r.below(len);let last=get(&db,base+U256::from(len-1));
                put(&mut db,&mut w,base+U256::from(i),last);put(&mut db,&mut w,base+U256::from(len-1),U256::ZERO);put(&mut db,&mut w,one,U256::from(len-1));
                let moved=Address::from_word(B256::from(last));let s=apply::record_slot(moved);let w0=get(&db,s);put(&mut db,&mut w,s,w0^(U256::from(r.below(255)+1)<<56));}
            3 if len>0=>{let id=Address::from_word(B256::from(get(&db,base+U256::from(r.below(len)))));record(&mut db,&mut w,&mut r,id,b_reg,height);}
            4 if len>0=>{let id=Address::from_word(B256::from(get(&db,base+U256::from(r.below(len)))));let s=apply::record_slot(id);let k=s+U256::from(r.below(4));put(&mut db,&mut w,k,U256::from(r.next())<<64);}
            5 if len>0=>{let id=Address::from_word(B256::from(get(&db,base+U256::from(r.below(len)))));put(&mut db,&mut w,apply::record_slot(id),U256::ZERO);}
            6 if len>2&&r.below(4)==0=>{// a duplicate on the list (DuplicateMember), repaired by a later step
                let x=get(&db,base);put(&mut db,&mut w,base+U256::from(1+r.below(len-1)),x);}
            7 if r.below(6)==0=>{put(&mut db,&mut w,one,U256::from(len+1+r.below(2)));}// past the written elements: id 0 (ReservedMember)
            8 if len>1=>{let n2=len-1-r.below(len.min(3));put(&mut db,&mut w,one,U256::from(n2));}// a shrink
            _=>{// an unchanged write and an unrelated slot
                let k=U256::from(r.next())<<128;let v=get(&db,one);put(&mut db,&mut w,one,v);put(&mut db,&mut w,k,U256::from(r.next()));}
        }
        if r.below(3)==0{w.push(U256::from(r.next()));}
        if r.below(4)==0{let len=get(&db,one).as_limbs()[0];w.push(base+U256::from(len+5));}
        let whole=MemberBase::read(&db,q);
        let (derived,_)=prev.derive(&db,&w,q);
        assert_eq!(derived,whole,"spec {spec} step {step}: the derived inputs are the whole reading");
        let mut p=g.clone();p.state=std::sync::Arc::new(db.clone());p.hash=[0xb1;32];p.trie=None;p.summary=None;
        // Parents at several heights: the activity window's cutoff moves (spec 1).
        for ph in [height.saturating_sub(1),height+3,height+40]{
            p.height=ph;let h=ph+1;
            let (a,b)=(c.membership_from_base(&p,h,&derived),c.membership_one_pass(&p,h));
            assert_eq!(a,b,"spec {spec} step {step} h={h}: the membership from the derived inputs is the one-pass reading");if b.is_err(){errors+=1;}
        }
        // The negative control: each changed slot left out of the write set leaves a difference.
        let prev_db_changed:Vec<U256>=w.iter().copied().filter(|k|{let x=MemberBase::read(&db,q);let mut ww=w.clone();ww.retain(|y|y!=k);prev.derive(&db,&ww,q).0!=x}).collect();
        if !prev_db_changed.is_empty(){negatives+=1;}
        longest=longest.max(whole.list.len());prev=whole;steps+=1;
    }
    assert!(steps==400&&negatives>100&&errors>0,"spec {spec}: positive controls ({negatives} steps whose write set was needed, {errors} error readings)");
    assert!(longest>8,"spec {spec}: the list grew ({longest} ids at most)");
}
#[test] fn derived_inputs_equal_the_whole_reading_on_synthetic_registry_changes_v3(){synthetic(2,0x9e37_79b9_7f4a_7c15)}
#[test] fn derived_inputs_equal_the_whole_reading_on_synthetic_registry_changes_spec1(){synthetic(1,0x2545_f491_4f6c_dd1d)}

// ---------------- chains ----------------
/// The bond state and whether the bond is below B_reg/4 of `who` on the head (None: no record or not on the list).
fn bond_of(n:&Net,who:Address)->Option<(u8,bool)>{let e=n.chain.full_entry(&n.chain.head).unwrap();MemberBase::read(&*e.state,quarter(&n.chain)).rec(&who.0.0).map(|r|(r.bond_state,r.below_quarter))}
fn grow(n:&mut Net,to:u64,what:&str){while n.chain.head().height<to{let b=mine(n,n.chain.head,vec![],vec![]);accept(n,b,what);}}
/// Registrations to the full set (M_max 5: three genesis members, alice bonded, bob unbonded: an accruing probationer whose
/// wins fill his bond through B_reg/4 to B_reg, ACCRUING to HELD), carol's registerReplacing of an inactive member (v3),
/// a type-2 punishment, alice's exit, across the snapshot height 257; a side branch from before the registrations (each
/// block validated on its own parent) growing heavier (a reorg across the membership change) and the main branch heavier
/// again (the reorg back); a restart. Every new head's cold context
/// derives its inputs and equals the one-pass and two-pass readings.
fn scenario(spec:u8){
    let tag=format!("spec {spec}");let mut cfg=config(spec,5);cfg.theta_inact=16;
    let mut n=net(cfg);let b_reg=pocold::chain::from_fixed_lp4(&n.chain.spec.cp().B_reg);
    let (alice,bob,carol)=(devnet::account_key("alice").address(),devnet::account_key("bob").address(),devnet::account_key("carol").address());
    grow(&mut n,3,&tag);let fork=n.chain.head;
    let t=vec![register(&n,alice,b_reg)];let b=mine(&n,n.chain.head,t,vec![]);accept(&mut n,b,&tag);all_succeeded(&n,"alice registers");
    let t=vec![register(&n,bob,U256::ZERO)];let b=mine(&n,n.chain.head,t,vec![]);accept(&mut n,b,&tag);all_succeeded(&n,"bob registers");
    let start=bond_of(&n,bob).expect("bob's record");assert_eq!(start,(4,true),"{tag}: bob is ACCRUING below B_reg/4");
    // Bob's wins fill his bond (the accruing mark goes at B_reg/4 on v3; the state turns HELD at B_reg); the third
    // genesis member neither wins nor shares, so it turns inactive.
    let victim=n.ids[2];let mut seen=vec![start];
    for _ in 0..160{
        let b=mine_where(&n,n.chain.head,vec![],vec![],&|o|o==bob).or_else(||mine_where(&n,n.chain.head,vec![],vec![],&|o|o!=victim)).expect("a solution");accept(&mut n,b,&tag);
        let s=bond_of(&n,bob).unwrap();if seen.last()!=Some(&s){seen.push(s);}if s.0==1&&n.chain.head().height>30{break}
    }
    assert!(seen.contains(&(4,false))&&seen.last().map(|s|s.0)==Some(1),"{tag}: bob crossed B_reg/4 and became HELD ({seen:?})");
    if spec==2{
        let t=vec![replacing(&n,carol,victim)];let b=mine_where(&n,n.chain.head,t,vec![],&|o|o!=victim).unwrap();accept(&mut n,b,&tag);
        all_succeeded(&n,"carol's registerReplacing");
        let act=apply::active_list(&*n.chain.full_entry(&n.chain.head).unwrap().state);assert!(act.contains(&carol)&&!act.contains(&victim),"{tag}: carol replaced the victim");
    }
    // A type-2 punishment of an earlier winner, then alice's exit.
    let target=n.blocks[n.blocks.len()-3].clone();let (ev,accused)=type2(&n,&target);
    let before=apply::miner_record(&*n.chain.full_entry(&n.chain.head).unwrap().state,accused).unwrap();
    let b=mine(&n,n.chain.head,vec![],vec![ev]);accept(&mut n,b,&tag);
    let after=apply::miner_record(&*n.chain.full_entry(&n.chain.head).unwrap().state,accused);
    assert!(after.map_or(true,|a|(a.tomb,a.status,a.bond_state,a.bond)!=(before.tomb,before.status,before.bond_state,before.bond)),"{tag}: the punishment changed {accused}'s record");
    let t=vec![deregister(&n,alice)];let b=mine(&n,n.chain.head,t,vec![]);accept(&mut n,b,&tag);all_succeeded(&n,"alice exits");
    // Across the snapshot height 257.
    grow(&mut n,262,&tag);
    let rec=n.chain.member_rec_of(n.chain.canonical_at(257).unwrap()).unwrap();assert!(rec.is_snapshot(),"{tag}: 257 is a snapshot");
    // A side branch from before the registrations: each of its blocks is validated on its own parent (a side state) and
    // derived; then the main branch grows again.
    // It grows heavier: a reorg across the membership change (the head's member set loses alice, bob and carol), and back.
    let main_tip=n.chain.head;let main_len=n.chain.head().height-n.chain.get(&fork).unwrap().height;let mut p=fork;
    for _ in 0..3{let b=mine(&n,p,vec![],vec![]);p=b.hash();accept_side(&mut n,b,&format!("{tag} side"));}
    assert_eq!(n.chain.head,main_tip,"{tag}: the short side branch is not the head");
    while n.chain.head==main_tip{let b=mine(&n,p,vec![],vec![]);p=b.hash();accept_side(&mut n,b,&format!("{tag} side"));}
    assert_eq!(n.chain.head,p,"{tag}: the reorg to the side branch ({} blocks above the fork, the main branch {main_len})",n.chain.head().height-3);
    let side=n.chain.full_entry(&p).unwrap();let act=apply::active_list(&*side.state);
    assert!(!act.contains(&bob)&&!act.contains(&alice),"{tag}: the new head's branch never had the registrations");
    let mut q=main_tip;while n.chain.head!=q||n.chain.head().height<=side.height{let b=mine(&n,q,vec![],vec![]);q=b.hash();accept_side(&mut n,b,&format!("{tag} main"));}
    assert_eq!(n.chain.head,q,"{tag}: the reorg back");
    let m=n.chain.member_counters();
    assert!(m.base_derived>=n.blocks.len() as u64&&m.base_checks>=n.blocks.len() as u64&&m.base_mismatch==0,"{tag}: every head derived and was compared ({m:?})");
    // A restart: a fresh chain replaying the blocks reads the genesis inputs whole, then derives each block's.
    let r=Chain::new(spec_of(&n.cfg));let mut r=r;
    for b in &n.blocks{let (e,_)=r.validate(b,None,0).unwrap();r.insert(e);}
    let rm=r.member_counters();assert!(rm.base_read<=4&&rm.base_derived+4>=n.blocks.len() as u64,"{tag}: the replay derives ({rm:?})");
    for h in n.chain.blocks.keys().copied().collect::<Vec<_>>(){
        let (x,y)=(n.chain.full_entry(&h).unwrap(),r.full_entry(&h).unwrap());let k=x.height+1;
        assert_eq!(n.chain.membership(&x,k),r.membership(&y,k),"{tag} restart h={}",x.height);
        assert_eq!(r.membership(&y,k),r.membership_one_pass(&y,k),"{tag} restart h={}",x.height);
    }
    members_sweep::sweep(&r);
}
/// A side block: imported, and its cold context derived as a head's (its parent is a side state the chain keeps whole).
fn accept_side(n:&mut Net,b:Block,what:&str){let (e,_)=n.chain.validate(&b,None,0).unwrap_or_else(|x|panic!("{what}: {x:?}"));let h=e.hash;n.chain.insert(e);n.blocks.push(b);cold_derives(&n.chain,&h,what);}
#[test] fn v3_contexts_derive_through_registrations_bonds_punishment_exit_replacement_reorg_and_restart(){scenario(2)}
#[test] fn spec1_contexts_derive_through_registrations_bonds_punishment_exit_replacement_reorg_and_restart(){scenario(1)}

// ---------------- a store-mode node ----------------
fn dir(label:&str)->PathBuf{let d=std::env::temp_dir().join(format!("pocol-fn1b-{label}-{}",std::process::id()));let _=std::fs::remove_dir_all(&d);d}
fn conf(d:&Path,limits:Limits)->Config{Config{name:"fn1b".into(),dir:d.to_path_buf(),listen:None,peers:vec![],miner:None,stop_at:None,log:false,rpc:None,limits}}
fn store()->Limits{Limits{state_store:true,checkpoint_every:16,r_keep:8,side_cap:2,slim_depth:Some(8),history_window:Some(4),..Default::default()}}
/// The node's cold context of its head derives (true) or reads its inputs whole, and equals the reference's one-pass
/// reading of the same block.
fn node_head(node:&Node,r:&Net,what:&str)->bool{
    let h=node.chain.head;let p=node.chain.full_entry(&h).unwrap();let k=p.height+1;
    let c0=node.chain.member_counters();node.chain.clear_ctx_cache();node.chain.forget_member_base(&p);let ctx=node.chain.child_ctx(&p,0).unwrap();let c1=node.chain.member_counters();
    let y=r.chain.full_entry(&h).unwrap();
    assert_eq!(*ctx.membership,r.chain.membership_one_pass(&y,k).unwrap(),"{what} h={}: the reference's one-pass reading",p.height);
    assert_eq!(*ctx.membership,node.chain.membership_one_pass(&p,k).unwrap(),"{what} h={}: the node's one-pass reading",p.height);
    assert_eq!(c1.base_mismatch,0);
    assert_eq!(c1.base_derived-c0.base_derived+c1.base_read-c0.base_read,1,"{what}: the inputs were derived or read whole, once");
    c1.base_derived>c0.base_derived
}
/// A store-mode node importing a chain with membership changes: every import's cold context derives; a rebuild of an
/// old block derives along its rolling re-execution; after a reopen from a checkpoint the first inputs are read whole, after
/// a replay (state.redb deleted) they are derived, and every later import derives; every context equals the reference's.
#[test] fn a_store_mode_node_derives_on_import_rebuild_and_after_a_checkpoint_restore_or_replay(){
    let cfg=config(2,8);let mut r=net(cfg);let b_reg=pocold::chain::from_fixed_lp4(&r.chain.spec.cp().B_reg);
    let (alice,bob,carol)=(devnet::account_key("alice").address(),devnet::account_key("bob").address(),devnet::account_key("carol").address());
    for i in 0..40u64{
        let txs=match i{5=>vec![register(&r,alice,b_reg)],6=>vec![register(&r,bob,U256::ZERO)],12=>vec![register(&r,carol,b_reg/U256::from(2))],20=>vec![deregister(&r,alice)],_=>vec![]};
        let ev=if i==25{vec![type2(&r,&r.blocks[20].clone()).0]}else{vec![]};
        let b=mine(&r,r.chain.head,txs,ev);accept(&mut r,b,"reference");
    }
    let d=dir("node");
    {
        let mut node=Node::open(conf(&d,store()),spec_of(&r.cfg)).unwrap();assert!(node.chain.member_oracle);
        let mut derived=0;
        for b in &r.blocks{node.chain.clear_ctx_cache();assert!(node.import(b.clone(),None).unwrap());if node_head(&node,&r,"import"){derived+=1;}}
        assert!(derived+1>=r.blocks.len(),"every import but the first derived ({derived} of {})",r.blocks.len());
        // A rebuild: an old block's state is gone (pruned below the window); its rebuild re-executes from an anchor or
        // genesis, deriving along the rolling entries (the oracle compares each).
        let (c0,mut rebuilt)=(node.chain.member_counters(),0);
        for h in 2..36u64{
            let old=r.chain.canonical_at(h).unwrap().hash;
            if !node.chain.get(&old).is_some_and(|e|e.pruned&&e.stored.is_none()){continue}
            node.chain.clear_member_caches();node.ensure_state(&old).unwrap();rebuilt+=1;
            let x=node.chain.full_entry(&old).unwrap();let y=r.chain.full_entry(&old).unwrap();
            assert_eq!(node.chain.membership(&x,x.height+1).unwrap(),r.chain.membership_one_pass(&y,y.height+1).unwrap(),"the rebuilt block at {h}");
        }
        let c1=node.chain.member_counters();
        assert!(rebuilt>0&&c1.base_derived>c0.base_derived&&c1.base_checks>c0.base_checks,"{rebuilt} rebuilds derived along their re-execution ({c0:?} -> {c1:?})");
        node.checkpoint_now().unwrap();members_sweep::sweep_node(&node);
    }
    // More blocks for the reopened nodes to import.
    for _ in 0..4{let b=mine(&r,r.chain.head,vec![],vec![]);accept(&mut r,b,"reference");}
    let tail:Vec<Block>=r.blocks[40..].to_vec();
    for way in 0..2{
        let c=dir(&format!("node-{way}"));copy_dir(&d,&c);
        if way==1{std::fs::remove_file(c.join(pocold::state_store::FILE)).unwrap();}
        let mut node=Node::open(conf(&c,store()),spec_of(&r.cfg)).unwrap();
        assert_eq!(node.stats.resumed_from.is_some(),way==0,"way {way}: a checkpoint restore, or a replay");
        // A checkpoint restore holds no inputs and no write set: the first context reads its inputs whole. A replay
        // re-validated every block, deriving along it, so it holds the head's write set and its parent's inputs.
        assert_eq!(node_head(&node,&r,"reopened"),way==1,"way {way}: the first context after the reopen derives only after a replay");
        for b in &tail{node.chain.clear_ctx_cache();assert!(node.import(b.clone(),None).unwrap());assert!(node_head(&node,&r,"after the reopen"),"way {way}: an import after the reopen derives");}
        members_sweep::sweep_node(&node);drop(node);let _=std::fs::remove_dir_all(&c);
    }
    let _=std::fs::remove_dir_all(&d);
}
fn copy_dir(from:&Path,to:&Path){std::fs::create_dir_all(to).unwrap();for e in std::fs::read_dir(from).unwrap(){let e=e.unwrap();let t=to.join(e.file_name());
    if e.file_type().unwrap().is_dir(){copy_dir(&e.path(),&t)}else{std::fs::copy(e.path(),&t).unwrap();}}}
/// The A13 term: the inputs and write sets held stay within CTX_BASES_MAX and CTX_WRITES_MAX, and the term at M_max
/// 10,000 is what `member_bases_bytes` counts.
#[test] fn the_inputs_cache_stays_within_its_a13_term(){
    let mut n=net(config(2,8));for _ in 0..12{let b=mine(&n,n.chain.head,vec![],vec![]);accept(&mut n,b,"bounded");
        let (b,w)=n.chain.member_bases_held();assert!(b<=pocold::chain::CTX_BASES_MAX&&w<=pocold::chain::CTX_WRITES_MAX,"{b} {w}");}
    let (b,w)=n.chain.member_bases_held();assert!(b>0&&w>0,"positive control: {b} inputs, {w} write sets held");
    let mut cp=n.chain.spec.cp().clone();cp.M_max=10_000;
    let one=pocold::resources::member_base_bytes(&cp).unwrap();let all=pocold::resources::member_bases_bytes(&cp).unwrap();
    println!("FN1B-A13 member_base_bytes(10,000)={one} member_bases_bytes(10,000)={all}");
    assert!(all>(pocold::chain::CTX_BASES_MAX as u128+1)*one,"the term counts CTX_BASES_MAX + 1 inputs and the write sets");
}
/// The oracle's negative control: inputs held wrong (as a faulty derivation would leave them) are caught by the
/// comparison with the one-pass reading on the next context; the one-pass result is the one returned (no verdict follows
/// the derivation), the difference is counted as a derivation and an oracle mismatch and a member fault, and every held
/// input and write set is dropped, so the next context reads its inputs whole. On a replayed chain (no end-of-test sweep:
/// the mismatch is wanted here).
#[test] fn a_wrong_derivation_is_caught_and_the_one_pass_reading_wins(){
    let mut n=net(config(2,8));for _ in 0..6{let b=mine(&n,n.chain.head,vec![],vec![]);accept(&mut n,b,"grow");}
    let r=Chain::new(spec_of(&n.cfg));let mut r=r;assert!(r.member_oracle,"the oracle is on in tests");r.tolerate_base_mismatch();
    for b in &n.blocks{let (e,_)=r.validate(b,None,0).unwrap();r.insert(e);}
    let p=r.full_entry(&r.head).unwrap();let k=p.height+1;
    let right=r.child_ctx(&p,0).unwrap().membership.clone();assert_eq!(*right,r.membership_one_pass(&p,k).unwrap());
    let c0=r.member_counters();assert_eq!((c0.base_mismatch,c0.oracle_mismatch,c0.faults),(0,0,0));
    r.clear_ctx_cache();assert!(r.damage_member_base(&p),"positive control: the head's inputs were held");
    let ctx=r.child_ctx(&p,0).unwrap();
    assert_eq!(*ctx.membership,*right,"the one-pass reading wins over the damaged inputs");
    assert_eq!(*ctx.membership,r.membership_one_pass(&p,k).unwrap());
    let c1=r.member_counters();
    assert_eq!((c1.base_mismatch,c1.oracle_mismatch,c1.faults,c1.base_hits-c0.base_hits),(1,1,1,1),"counted once, from the held (damaged) inputs ({c1:?})");
    assert_eq!(r.member_bases_held(),(0,0),"every held input and write set was dropped");
    // The next cold context reads its inputs whole, agrees, and is not a new mismatch.
    r.clear_ctx_cache();let again=r.child_ctx(&p,0).unwrap();let c2=r.member_counters();
    assert_eq!(*again.membership,*right);assert_eq!((c2.base_read-c1.base_read,c2.base_mismatch),(1,1),"read whole after the drop ({c2:?})");
}
/// Without `tolerate_base_mismatch`, the same damaged inputs make the next context panic where the comparison fails, so a
/// test with the oracle on fails even when it reads no counter (the one-pass substitution cannot hide a wrong derivation
/// from it).
#[test] #[should_panic(expected="FN-1b the oracle: a parent's membership from its derived inputs")]
fn a_wrong_derivation_panics_on_a_chain_that_does_not_tolerate_it(){
    let mut n=net(config(2,8));for _ in 0..3{let b=mine(&n,n.chain.head,vec![],vec![]);accept(&mut n,b,"grow");}
    let mut r=Chain::new(spec_of(&n.cfg));for b in &n.blocks{let (e,_)=r.validate(b,None,0).unwrap();r.insert(e);}
    let p=r.full_entry(&r.head).unwrap();r.child_ctx(&p,0).unwrap();
    r.clear_ctx_cache();assert!(r.damage_member_base(&p),"positive control: the head's inputs were held");
    let _=r.child_ctx(&p,0);
}
/// A chain of `n`'s blocks but the last `keep_back`, replayed (no end-of-test sweep: the hooks below change what it keeps).
fn replay_but(n:&Net,keep_back:usize)->Chain{
    let mut r=Chain::new(spec_of(&n.cfg));for b in &n.blocks[..n.blocks.len()-keep_back]{let (e,_)=r.validate(b,None,0).unwrap();r.insert(e);}r
}
/// The cold context of `h` on `c` (FN-1's rows cleared): the (derived, read whole) inputs it took, and its membership is
/// the one-pass reading.
fn cold(c:&Chain,h:&[u8;32],what:&str)->(u64,u64){
    let p=c.full_entry(h).unwrap();let k=p.height+1;let c0=c.member_counters();c.clear_ctx_cache();
    let ctx=c.child_ctx(&p,0).unwrap_or_else(|x|panic!("{what}: {x:?}"));let c1=c.member_counters();
    assert_eq!(*ctx.membership,c.membership_one_pass(&p,k).unwrap(),"{what}: the one-pass reading");
    (c1.base_derived-c0.base_derived,c1.base_read-c0.base_read)
}
/// Four blocks, a registration (it writes registry slots: the list, its length and the new record) and one more block.
fn with_registration()->Net{
    let mut n=net(config(2,8));for _ in 0..4{let b=mine(&n,n.chain.head,vec![],vec![]);accept(&mut n,b,"grow");}
    let alice=devnet::account_key("alice").address();let b_reg=pocold::chain::from_fixed_lp4(&n.chain.spec.cp().B_reg);
    let t=vec![register(&n,alice,b_reg)];let b=mine(&n,n.chain.head,t,vec![]);accept(&mut n,b,"register");all_succeeded(&n,"alice registers");
    let b=mine(&n,n.chain.head,vec![],vec![]);accept(&mut n,b,"after");n
}
fn import(c:&mut Chain,b:&Block)->[u8;32]{let (e,_)=c.validate(b,None,0).unwrap();let h=e.hash;c.insert(e);h}
/// A registry write set above the cap is not kept (the cap lowered to 0 for the registration's block, which wrote registry
/// slots): no write set is added, and the next context of that block reads its inputs whole; with the cap back, the next
/// block's write set is kept and its context derives again.
#[test] fn a_write_set_above_the_cap_is_not_kept_and_the_next_context_reads_whole(){
    let n=with_registration();let mut r=replay_but(&n,2);let (reg,after)=(&n.blocks[n.blocks.len()-2],&n.blocks[n.blocks.len()-1]);
    let head=r.head;assert_eq!(cold(&r,&head,"before"),(1,0),"positive control: the replayed head derives");
    r.set_ctx_write_slots_max(0);let (_,w0)=r.member_bases_held();
    let h=import(&mut r,reg);
    assert_eq!(r.member_bases_held().1,w0,"the registration's write set (above the cap) is not kept");
    assert_eq!(cold(&r,&h,"above the cap"),(0,1),"its context reads the inputs whole");
    r.set_ctx_write_slots_max(pocold::chain::CTX_WRITE_SLOTS_MAX);
    let h=import(&mut r,after);assert_eq!(cold(&r,&h,"within the cap"),(1,0),"within the cap again: the next context derives");
    members_sweep::sweep(&r);
}
/// The oracle's negative control at import: a write set that leaves out every slot its block wrote (as one missing a
/// changed slot would) gives derived inputs that differ from the block's state read whole. `note_writes` counts one
/// mismatch, keeps no write set (every held one is dropped), and the next context of that block reads its inputs whole;
/// the next block's write set is kept again and its context derives.
#[test] fn a_write_set_missing_its_slots_is_caught_at_import_and_not_kept(){
    let n=with_registration();let mut r=replay_but(&n,2);r.tolerate_base_mismatch();let (reg,after)=(&n.blocks[n.blocks.len()-2],&n.blocks[n.blocks.len()-1]);
    let c0=r.member_counters();assert_eq!((c0.base_mismatch,c0.oracle_mismatch,c0.faults),(0,0,0));
    let (_,w0)=r.member_bases_held();assert!(w0>0,"positive control: write sets were held");
    r.withhold_next_registry_writes();let h=import(&mut r,reg);
    let c1=r.member_counters();
    assert_eq!((c1.base_mismatch,c1.oracle_mismatch,c1.faults),(1,1,1),"counted once, at the import ({c1:?})");
    assert_eq!(r.member_bases_held().1,0,"the wrong write set is not kept, and the held ones were dropped");
    assert_eq!(cold(&r,&h,"after the import mismatch"),(0,1),"its context reads the inputs whole");
    let h=import(&mut r,after);assert_eq!(cold(&r,&h,"the next block"),(1,0),"the next block's write set is kept: its context derives");
    let c2=r.member_counters();assert_eq!(c2.base_mismatch,1,"no new mismatch ({c2:?})");
}
