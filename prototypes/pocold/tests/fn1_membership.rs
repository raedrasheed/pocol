//! FN-1 (C6 part, node-local): one-pass membership and the child-context cache give exactly what the pre-FN-1
//! algorithm gives (`Chain::membership_two_pass`, kept unchanged as the oracle, and `ctx_cache_off`, every context
//! computed from the parent's state), across registration, exit and accruing marks at block boundaries, a reorg across
//! a membership change and back, a restart (a fresh chain replaying the blocks), and a node with slim, unslim, offload,
//! checkpoints and a reopen; and at 10,000 members (v3), where the speedup is measured and printed (never asserted).
//! Private fixture chains and storage-view fixtures only.
use std::collections::HashMap;
use std::sync::Arc;
use pocol_lp4_execution::{apply,devnet::{self,DevnetConfig},txsign::{Key,Tx}};
use pocold::{chain::{Chain,ChildCtx,Entry,Reject,Spec},node::{Config,Limits,Node},hashes,types::*};
use revm::primitives::{keccak256,Address,Bytes,B256,U256};

struct Net{chain:Chain,keys:HashMap<Address,Key>,cfg:DevnetConfig,blocks:Vec<Block>}
fn config(v3:bool,full:bool,n:usize)->DevnetConfig{
    let mut cfg=DevnetConfig::new(777952,1_700_000_000,&[]);
    cfg.miners=(0..n).map(|i|format!("fn1-{i}")).collect();
    cfg.hash_rate=16;cfg.c=8;cfg.d_att=73;cfg.w_act=4;cfg.r_max=1;
    cfg.spec_version=if v3{2}else{1};cfg.nonce_mode=if full{1}else{0};cfg.earnin=true;
    cfg.m_max=cfg.m_max.max(n as u16);
    cfg.accounts.push(("carol".into(),U256::from(10u128.pow(24))));
    cfg
}
fn spec(cfg:&DevnetConfig)->Spec{Spec::new(devnet::build(cfg).unwrap().admit(&pocold::single_host_local()).unwrap().0)}
fn net(cfg:DevnetConfig)->Net{
    let candidate=devnet::build(&cfg).unwrap();
    let mut keys:HashMap<_,_>=candidate.miners.iter().map(|m|(m.id,Key::from_secret(&m.key.secret()).unwrap())).collect();
    for label in ["alice","bob","carol"]{let key=devnet::account_key(label);keys.insert(key.address(),key);}
    Net{chain:Chain::new(spec(&cfg)),keys,cfg,blocks:Vec::new()}
}
/// A block on `parent`: signed by Draw(h,0) (every fixture key is held), its winner the first member with a held key
/// whose range holds a solution.
fn solve_on(n:&Net,parent:[u8;32],txs:Vec<Vec<u8>>)->Block{
    let p=n.chain.full_entry(&parent).unwrap();let ctx=n.chain.child_ctx(&p,0).unwrap();let body=Body{txs,evidence:vec![]};
    let cp=n.chain.spec.cp();
    for ts in p.ts+u64::from(cp.T_blk)..=ctx.window.upper(){
        let (mut header,_,_)=n.chain.build_signed(&p,&n.keys[&ctx.proposer],ts,body.clone(),vec![]).unwrap();
        let tid=template_id(&header.ut_raw);
        for r in ctx.assignment.ranges(){
            let Some(key)=n.keys.get(&addr(&r.id())) else{continue};let mut shares=Vec::new();
            for nonce in r.start()..r.end_exclusive().min(r.start()+200_000){
                let hash=pow_hash(&tid,nonce as u64);
                if le_target(&hash,&ctx.target){
                    header.nonce=nonce as u64;header.shares=shares;reseal(&mut header);
                    header.winner_sig=key.sign_hash(&B256::from(hashes::win_msg(&tid,nonce as u64,&share_root(&header))));
                    reseal(&mut header);return Block{header,body};
                }
                if shares.len()<usize::from(cp.S_max)&&le_target(&hash,&ctx.t_share){shares.push(nonce as u64);}
            }
        }
    }
    panic!("the deterministic fixture has no solution")
}
fn accept(n:&mut Net,b:Block){let (e,_)=n.chain.validate(&b,None,0).unwrap();n.chain.insert(e);n.blocks.push(b);}
fn next(n:&mut Net,txs:Vec<Vec<u8>>)->[u8;32]{let b=solve_on(n,n.chain.head,txs);let h=b.hash();accept(n,b);h}
fn tx(n:&Net,key:Address,to:Address,value:U256,data:Vec<u8>)->Vec<u8>{
    let p=n.chain.full_entry(&n.chain.head).unwrap();let base=n.chain.child_ctx(&p,0).unwrap().base_fee;
    let nonce=p.state.accounts.get(&key).map_or(0,|a|a.info.nonce);let priority=U256::from(1_000_000_000u64);
    Tx{chain_id:n.chain.spec.chain_id(),nonce,max_priority:priority,max_fee:base*U256::from(2)+priority,gas:1_000_000,to:Some(to),value,data:Bytes::from(data)}.sign_1559(&n.keys[&key])
}
fn registration(n:&Net,key:Address,value:U256)->Vec<u8>{
    let mut data=keccak256("register(address)")[..4].to_vec();data.extend_from_slice(key.into_word().as_slice());tx(n,key,apply::registry(),value,data)
}
fn exit(n:&Net,key:Address)->Vec<u8>{tx(n,key,apply::registry(),U256::ZERO,keccak256("deregister()")[..4].to_vec())}

/// Two child contexts agree on everything the membership, the assignment and the Draw determine (or fail alike).
fn same_ctx(a:&Result<ChildCtx,Reject>,b:&Result<ChildCtx,Reject>,what:&str){
    match (a,b){
        (Ok(x),Ok(y))=>{
            assert_eq!(*x.membership,*y.membership,"{what}: membership");assert_eq!(*x.assignment,*y.assignment,"{what}: assignment");
            assert_eq!(x.proposer,y.proposer,"{what}: Draw");assert_eq!((x.height,x.attempt,x.target,x.t_share,x.base_fee),(y.height,y.attempt,y.target,y.t_share,y.base_fee),"{what}");
        }
        (Err(x),Err(y))=>assert_eq!(format!("{x:?}"),format!("{y:?}"),"{what}"),
        _=>panic!("{what}: {:?} against {:?}",a.as_ref().err(),b.as_ref().err()),
    }
}
fn same_members(a:&Result<pocold::assignment::Membership,Reject>,b:&Result<pocold::assignment::Membership,Reject>,what:&str){
    match (a,b){(Ok(x),Ok(y))=>assert_eq!(x,y,"{what}"),(Err(x),Err(y))=>assert_eq!(format!("{x:?}"),format!("{y:?}"),"{what}"),_=>panic!("{what}: {a:?} against {b:?}")}
}
/// The context of a child of `e` computed from e's state, the cache bypassed: what child_ctx gave before FN-1.
fn uncached(c:&mut Chain,e:&Entry,a:u32)->Result<ChildCtx,Reject>{c.ctx_cache_off=true;let r=c.child_ctx(e,a);c.ctx_cache_off=false;r}
/// Whether the cache looks up and keeps a context of `e` (`Chain::ctx_cacheable`): a stored version, or a whole state
/// that is not pruned.
fn cacheable(c:&Chain,e:&Entry)->bool{!c.ctx_cache_off&&(e.stored.is_some()||(e.full&&!e.pruned))}
/// The cache's (hits, misses) added by `f`.
fn counted<T>(c:&Chain,f:impl FnOnce()->T)->(T,(u64,u64)){let (_,(h0,m0))=c.ctx_cache_stats();let r=f();let (_,(h1,m1))=c.ctx_cache_stats();(r,(h1-h0,m1-m0))}
/// Every held entry, as a whole copy (`full_entry`) and as it is held (a pruned or diff-held one reads an empty state, as
/// before FN-1): the one-pass and cached memberships equal the two-pass oracle, and cached contexts at attempts 0..3
/// (looked up twice) equal the uncached ones. The cache answered the second lookup: for a cacheable entry its membership
/// is one hit, and its context three (membership, assignment and Draw) and no miss; an entry the cache does not keep
/// is never counted. Returns the entries checked.
fn check(c:&mut Chain,what:&str)->usize{
    let hashes:Vec<[u8;32]>=c.blocks.keys().copied().collect();let (mut checked,mut hit)=(0,0);
    for h in hashes{
        let mut copies=vec![c.full_entry(&h).unwrap()];copies.push(c.get(&h).unwrap().clone());
        for e in copies{
            let k=e.height+1;let label=format!("{what} h={} pruned={} full={}",e.height,e.pruned,e.full);
            let two=c.membership_two_pass(&e,k);
            same_members(&c.membership_one_pass(&e,k),&two,&label);
            let kept=cacheable(c,&e);
            let (first,_)=counted(c,||c.membership(&e,k));same_members(&first,&two,&label);
            let (second,n)=counted(c,||c.membership(&e,k));same_members(&second,&two,&label);
            if first.is_ok(){assert_eq!(n,if kept{(1,0)}else{(0,0)},"{label}: the second membership lookup");}
            for a in 0..3{
                let want=uncached(c,&e,a);let what=format!("{label} a={a}");
                let (x,n0)=counted(c,||c.child_ctx(&e,a));same_ctx(&x,&want,&what);
                let (y,n1)=counted(c,||c.child_ctx(&e,a));same_ctx(&y,&want,&what);
                if !kept{assert_eq!((n0,n1),((0,0),(0,0)),"{what}: a context the cache does not keep is not looked up");}
                else if x.is_ok(){assert_eq!(n1,(3,0),"{what}: the second context is three hits (membership, assignment, Draw)");hit+=1;}
            }
            checked+=1;
        }
    }
    assert!(hit>0,"{what}: some contexts were answered from the cache");
    checked
}
fn ids(c:&Chain,h:[u8;32])->Vec<[u8;20]>{let e=c.full_entry(&h).unwrap();c.membership_two_pass(&e,e.height+1).unwrap().ids().to_vec()}

/// Registration (a bonded member and an unbonded probationer, accruing), exit, a reorg across the membership change and
/// back, and a restart, each block checked at its boundary.
fn scenario(v3:bool,full:bool){
    let mut n=net(config(v3,full,3));let tag=format!("v3={v3} full={full}");
    let (alice,bob)=(devnet::account_key("alice").address(),devnet::account_key("bob").address());
    let b_reg=pocold::chain::from_fixed_lp4(&n.chain.spec.cp().B_reg);
    for _ in 0..2{next(&mut n,vec![]);check(&mut n.chain,&tag);}
    let fork=n.chain.head;let before=ids(&n.chain,fork);
    // R_max 1: one registration per block, so the two join at consecutive boundaries.
    let t=vec![registration(&n,alice,b_reg)];next(&mut n,t);check(&mut n.chain,&tag);
    let t=vec![registration(&n,bob,U256::ZERO)];next(&mut n,t);check(&mut n.chain,&tag);
    let joined=ids(&n.chain,n.chain.head);
    assert_ne!(before,joined,"{tag}: the registrations changed the membership at a block boundary");
    let e=n.chain.full_entry(&n.chain.head).unwrap();let m=n.chain.membership(&e,e.height+1).unwrap();
    assert!(m.ids().contains(&raw_addr(bob))&&m.accruing().contains(&raw_addr(bob)),"{tag}: the unbonded probationer is a member marked accruing");
    for _ in 0..2{next(&mut n,vec![]);check(&mut n.chain,&tag);}
    let t=vec![exit(&n,alice)];next(&mut n,t);check(&mut n.chain,&tag);
    for _ in 0..2{next(&mut n,vec![]);check(&mut n.chain,&tag);}
    let main_tip=n.chain.head;let main_len=n.chain.head().height-n.chain.get(&fork).unwrap().height;
    // A heavier branch from before the registrations: the head moves across the membership change.
    let mut p=fork;for _ in 0..=main_len{let b=solve_on(&n,p,vec![]);p=b.hash();accept(&mut n,b);}
    assert_eq!(n.chain.head,p,"{tag}: the longer side branch is the head");
    assert!(n.chain.get(&main_tip).is_some(),"the abandoned branch is still held");
    let side_head=n.chain.full_entry(&n.chain.head).unwrap();let act=apply::active_list(side_head.state.as_ref());
    assert!(!act.contains(&alice)&&!act.contains(&bob),"{tag}: the new head branch never had the registrations");
    assert!(!ids(&n.chain,n.chain.head).contains(&raw_addr(bob)));
    let entries=check(&mut n.chain,&format!("{tag} after the reorg"));assert!(entries>=2*(2*main_len as usize+3),"{tag}: both branches are checked");
    // And back: the old branch grows heavier again.
    let mut q=main_tip;for _ in 0..3{let b=solve_on(&n,q,vec![]);q=b.hash();accept(&mut n,b);}
    assert_eq!(n.chain.head,q,"{tag}: the reorg back");check(&mut n.chain,&format!("{tag} after the reorg back"));
    let ((parents,assignments),(hits,misses))=n.chain.ctx_cache_stats();
    assert!(parents<=pocold::chain::CTX_PARENTS_MAX&&assignments<=pocold::chain::CTX_ASSIGNMENTS_MAX,"bounded: {parents} {assignments}");
    assert!(hits>0&&misses>0,"{tag}: the cache was used ({hits} hits, {misses} misses)");
    // A restart: a fresh chain (empty cache) replaying the same blocks gives every entry the same contexts.
    let mut r=Chain::new(spec(&n.cfg));for b in &n.blocks{let (e,_)=r.validate(b,None,0).unwrap();r.insert(e);}
    assert_eq!(r.head,n.chain.head);
    for h in n.chain.blocks.keys().copied().collect::<Vec<_>>(){
        let (x,y)=(n.chain.full_entry(&h).unwrap(),r.full_entry(&h).unwrap());let k=x.height+1;
        same_members(&n.chain.membership(&x,k),&r.membership(&y,k),&format!("{tag} restart"));
        for a in 0..2{same_ctx(&n.chain.child_ctx(&x,a),&r.child_ctx(&y,a),&format!("{tag} restart a={a}"));}
    }
    check(&mut r,&format!("{tag} restarted"));
}
#[test] fn legacy_one_pass_membership_and_cache_equal_the_two_pass_oracle_bounded(){scenario(false,false)}
#[test] fn legacy_one_pass_membership_and_cache_equal_the_two_pass_oracle_full_nonce(){scenario(false,true)}
#[test] fn v3_one_pass_membership_and_cache_equal_the_two_pass_oracle(){scenario(true,true)}
#[test] fn v3_bounded_one_pass_membership_and_cache_equal_the_two_pass_oracle(){scenario(true,false)}

/// A storage-view fixture whose hash, height and seal stay the genesis' while its state changes (as
/// tests/v3_membership.rs builds them): the cache keys the state by identity, so a changed state is never answered
/// with the context of another.
#[test] fn a_changed_state_under_the_same_hash_is_never_answered_from_the_cache(){
    let mut n=net(config(false,true,3));let g=n.chain.full_entry(&n.chain.head).unwrap();
    assert!(cacheable(&n.chain,&g),"the genesis context is one the cache keeps");
    let (first,cold)=counted(&n.chain,||n.chain.child_ctx(&g,0).unwrap());assert_eq!(first.membership.ids().len(),3);
    assert_eq!(cold,(0,3),"the first context is computed");
    let (rows,_)=n.chain.ctx_cache_stats();assert_eq!(rows,(1,1),"and left a parent row and an assignment row");
    let (_,warm)=counted(&n.chain,||n.chain.child_ctx(&g,0).unwrap());assert_eq!(warm,(3,0),"the same state is answered from the cache");
    let mut db=(*g.state).clone();let reg=apply::registry();
    // Tomb the first ActList member: the membership loses it.
    let victim=apply::active_list(&db)[0];let slot=apply::record_slot(victim);let w0=db.accounts[&reg].storage[&slot];
    db.insert_account_storage(reg,slot,w0|(U256::from(1)<<8)).unwrap();
    let mut p=g.clone();p.state=Arc::new(db);
    assert!(cacheable(&n.chain,&p));
    let (changed,n_changed)=counted(&n.chain,||n.chain.child_ctx(&p,0).unwrap());assert_eq!(changed.membership.ids().len(),2,"the changed state's own membership");
    assert_eq!(n_changed,(0,3),"a changed state under the same hash is a miss, never a hit");
    same_ctx(&Ok(changed),&uncached(&mut n.chain,&p,0),"changed state");
    let (again,n_again)=counted(&n.chain,||n.chain.child_ctx(&g,0));same_ctx(&again,&Ok(first),"the original state is still answered with its own");
    assert_eq!(n_again,(3,0),"from its own rows");
}

/// A node with slim, offload, checkpoints and the stored state (both modes), then reopened: every canonical block's
/// context, its state restored (`ensure_state`: unslim, rebuild), equals the one a bare reference chain computes.
fn node_scenario(store:bool){
    let mut n=net(config(false,true,3));let (alice,bob)=(devnet::account_key("alice").address(),devnet::account_key("bob").address());
    let b_reg=pocold::chain::from_fixed_lp4(&n.chain.spec.cp().B_reg);
    for i in 0..14{
        let txs=match i{3=>vec![registration(&n,alice,b_reg)],4=>vec![registration(&n,bob,U256::ZERO)],8=>vec![exit(&n,alice)],_=>vec![]};
        next(&mut n,txs);
    }
    // The reference is the pre-FN-1 computation: every context from the parent's state, the cache bypassed.
    n.chain.ctx_cache_off=true;
    let d=std::env::temp_dir().join(format!("pocold-fn1-node-{store}-{}",std::process::id()));let _=std::fs::remove_dir_all(&d);
    let conf=||Config{name:"fn1".into(),dir:d.clone(),listen:None,peers:vec![],miner:None,stop_at:None,log:false,rpc:None,
        limits:Limits{r_keep:4,side_cap:2,state_store:store,checkpoint_every:2,slim_depth:Some(4),history_window:Some(2),..Default::default()}};
    let compare=|node:&mut Node,n:&Net,what:&str|{
        let (mut slim,mut hit)=(0,0);
        for h in 0..=node.chain.head().height{
            let hash=node.chain.canonical_at(h).map(|e|e.hash).unwrap_or_else(||n.chain.canonical_at(h).unwrap().hash);
            if node.chain.get(&hash).is_some_and(|e|e.slim||e.offloaded||e.pruned){slim+=1;}
            node.ensure_state(&hash).unwrap();let Some(x)=node.chain.full_entry(&hash) else{continue};
            if x.pruned{continue}
            let y=n.chain.full_entry(&hash).unwrap();
            let kept=cacheable(&node.chain,&x);
            for a in 0..2{
                let want=n.chain.child_ctx(&y,a);let label=format!("{what} h={h} a={a}");
                let (first,_)=counted(&node.chain,||node.chain.child_ctx(&x,a));same_ctx(&first,&want,&label);
                let (second,n1)=counted(&node.chain,||node.chain.child_ctx(&x,a));same_ctx(&second,&want,&label);
                if kept&&first.is_ok(){assert_eq!(n1,(3,0),"{label}: the node's second context is answered from its cache");hit+=1;}
            }
        }
        assert!(hit>0,"{what}: the node's cache answered some contexts");
        slim
    };
    {let mut node=Node::open(conf(),spec(&n.cfg)).unwrap();
        for b in &n.blocks{assert!(node.import(b.clone(),None).unwrap());}
        assert_eq!(node.chain.head,n.chain.head);
        let slim=compare(&mut node,&n,"live");assert!(slim>0,"store={store}: some canonical entries were slimmed, offloaded or pruned");
        compare(&mut node,&n,"live, again");}
    let mut node=Node::open(conf(),spec(&n.cfg)).unwrap();assert_eq!(node.chain.head,n.chain.head,"reopened");
    compare(&mut node,&n,"reopened");drop(node);let _=std::fs::remove_dir_all(&d);
}
#[test] fn contexts_survive_slim_unslim_offload_and_reopen(){node_scenario(false)}
#[test] fn contexts_survive_slim_unslim_offload_and_reopen_in_store_mode(){node_scenario(true)}

/// The registry storage of `m` synthetic members on `db`'s ActList (records as MinerRegistry writes them): every 97th a
/// tomb, every 7th ACCRUING below B_reg/4, the rest bonded; recent shares on most, grace on some.
fn synthetic_members(db:&mut revm::InMemoryDB,m:usize,b_reg:U256,height:u64){
    let reg=apply::registry();let base=U256::from_be_bytes(keccak256(U256::from(6).to_be_bytes::<32>()).0);
    let put=|db:&mut revm::InMemoryDB,k:U256,v:U256|db.insert_account_storage(reg,k,v).unwrap();
    put(db,U256::from(1),U256::from(m));
    for i in 0..m{
        let mut a=[0u8;20];a[0]=0x5e;a[1]=0x11;a[2]=(i>>8) as u8;a[3]=i as u8;a[19]=1;let id=Address::from(a);
        put(db,base+U256::from(i),U256::from_be_slice(id.as_slice()));
        let tomb=i%97==96;let accruing=!tomb&&i%7==3;let state:u64=if tomb{3}else if accruing{4}else{1};
        let last=if i%5==0{0}else{height.saturating_sub((i%9) as u64)};
        let w0=U256::from(1u64)|(U256::from(u8::from(tomb))<<8)|(U256::from(state)<<16)|(U256::from(last)<<88);
        let slot=apply::record_slot(id);put(db,slot,w0);put(db,slot+U256::from(1),U256::from_be_slice(id.as_slice()));
        put(db,slot+U256::from(2),if accruing{b_reg/U256::from(8)}else if tomb{U256::ZERO}else{b_reg});
        put(db,slot+U256::from(3),if i%11==0{U256::from(height)<<192}else{U256::ZERO});
    }
}
/// A block's stored nonce ranges, as `Entry::ranges` keeps them (chain.rs, the entry a validated block makes).
fn stored_ranges(a:&pocold::assignment::Assignment)->Vec<(Address,u128,u128)>{a.ranges().iter().map(|r|(addr(&r.id()),r.start(),r.end_exclusive())).collect()}
/// The pre-FN-1 lookup over stored ranges (type-2 evidence, `late_check`), kept as the oracle of `range_owner`.
fn stored_linear(ranges:&[(Address,u128,u128)],n:u128)->Option<Address>{ranges.iter().find(|(_,s0,e0)|*s0<=n&&n<*e0).map(|r|r.0)}
/// `range_owner` against the linear scan on contiguous sorted ranges [i·nMax/m, (i+1)·nMax/m): empty ranges (nMax < m),
/// one member, every range bound and its neighbours, and nonces at and past nMax; and on the ranges real assignments
/// keep (bounded and full nonce modes).
#[test] fn range_owner_over_stored_ranges_equals_the_linear_scan(){
    for (m,n_max) in [(1u128,1u128),(1,7),(3,2),(5,3),(10,1),(7,1000),(64,63),(64,65),(256,1u128<<40),(1000,999)]{
        let ranges:Vec<(Address,u128,u128)>=(0..m).map(|i|{let mut a=[0u8;20];a[18]=(i>>8) as u8;a[19]=i as u8;(Address::from(a),i*n_max/m,(i+1)*n_max/m)}).collect();
        let mut probes=vec![0,n_max,n_max+1,u128::MAX];for r in &ranges{for x in [r.1,r.2]{probes.extend([x.saturating_sub(1),x,x+1]);}}
        for x in probes{assert_eq!(pocold::chain::range_owner(&ranges,x),stored_linear(&ranges,x),"m={m} nMax={n_max} n={x}");}
    }
    for full in [false,true]{
        let mut n=net(config(false,full,3));next(&mut n,vec![]);
        for h in n.chain.blocks.keys().copied().collect::<Vec<_>>(){
            let e=n.chain.full_entry(&h).unwrap();if e.height==0{continue}
            assert!(!e.ranges.is_empty(),"a validated block keeps its ranges");
            let mut probes=vec![0u128,u128::from(u64::MAX)];for r in &e.ranges{for x in [r.1,r.2]{probes.extend([x.saturating_sub(1),x]);}}
            for x in probes{assert_eq!(pocold::chain::range_owner(&e.ranges,x),stored_linear(&e.ranges,x),"full={full} n={x}");}
        }
    }
}
fn ms(d:std::time::Duration)->f64{d.as_secs_f64()*1000.0}
fn time<T>(runs:u32,mut f:impl FnMut()->T)->(f64,T){let t=std::time::Instant::now();let mut out=None;for _ in 0..runs{out=Some(f());}(ms(t.elapsed())/f64::from(runs),out.unwrap())}
/// `m` synthetic members on the genesis state of `cfg`'s chain: the one-pass, cached and two-pass paths agree, the
/// binary-search owner equals a linear scan, and each path is timed (printed as FN1-BENCH; never asserted).
fn full_set(cfg:DevnetConfig,m:usize,label:&str){
    let mut c=Chain::new(spec(&cfg));let g=c.full_entry(&c.head).unwrap();let b_reg=pocold::chain::from_fixed_lp4(&c.spec.cp().B_reg);
    let mut db=(*g.state).clone();synthetic_members(&mut db,m,b_reg,0);
    let mut p=g.clone();p.state=Arc::new(db);p.hash=[0xf1;32];p.trie=None;p.summary=None;
    let h=p.height+1;
    let (two_ms,two)=time(3,||c.membership_two_pass(&p,h).unwrap());
    let (one_ms,one)=time(3,||c.membership_one_pass(&p,h).unwrap());
    assert_eq!(one,two,"{label}: one pass equals the two-pass oracle");
    assert!(two.ids().len()>m*9/10&&!two.accruing().is_empty(),"{label}: the fixture has {} members, {} accruing",two.ids().len(),two.accruing().len());
    // The pre-FN-1 child context's membership, assignment and Draw: the two-pass membership, then the same two steps.
    let (pre_ms,_)=time(3,||{let m=c.membership_two_pass(&p,h).unwrap();let pre=c.child_pre(&p,0).unwrap();
        let a=pocold::assignment::Assignment::new(&m,0,&pre.target,c.spec.effective_nonce_mode(h),c.spec.cp().c).unwrap();(a,c.draw(&p,&m,h).unwrap())});
    c.ctx_cache_off=true;let (cold_ms,cold)=time(3,||c.child_ctx(&p,0).unwrap());c.ctx_cache_off=false;
    c.clear_ctx_cache();let (first_ms,first)=time(1,||c.child_ctx(&p,0).unwrap());
    let (warm_ms,warm)=time(20,||c.child_ctx(&p,0).unwrap());
    same_ctx(&Ok(first),&Ok(cold),label);let want=uncached(&mut c,&p,0);same_ctx(&Ok(warm),&want,label);
    for a in 1..4{let want=uncached(&mut c,&p,a);same_ctx(&c.child_ctx(&p,a),&want,&format!("{label} a={a}"));same_ctx(&c.child_ctx(&p,a),&want,&format!("{label} a={a} hit"));}
    // owner(): the binary search against the pre-FN-1 linear scan, on every range bound and 10,000 random nonces.
    let ctx=c.child_ctx(&p,0).unwrap();let ranges=ctx.assignment.ranges();
    let linear=|x:u64|{let x=x as u128;if x>=ctx.assignment.nonce_max(){return None}ranges.iter().find(|r|r.start()<=x&&x<r.end_exclusive()).map(|r|r.id())};
    let mut rng=0x2545_f491_4f6c_dd1du64;let mut nonces:Vec<u64>=(0..10_000).map(|_|{rng^=rng<<13;rng^=rng>>7;rng^=rng<<17;rng}).collect();
    for r in ranges{for x in [r.start(),r.end_exclusive()]{for d in [x.wrapping_sub(1),x]{if d<=u128::from(u64::MAX){nonces.push(d as u64)}}}}
    nonces.extend([0,u64::MAX]);
    let (lin_ms,lin)=time(1,||nonces.iter().map(|x|linear(*x)).collect::<Vec<_>>());
    let (bin_ms,bin)=time(1,||nonces.iter().map(|x|ctx.assignment.owner(*x)).collect::<Vec<_>>());
    assert_eq!(bin,lin,"{label}: owner() equals the linear scan");
    // The stored ranges' lookup (type-2 evidence and kind-3 nonces read them): the same answers.
    let stored=stored_ranges(&ctx.assignment);
    let (stored_ms,by_stored)=time(1,||nonces.iter().map(|x|pocold::chain::range_owner(&stored,u128::from(*x))).collect::<Vec<_>>());
    assert_eq!(by_stored.into_iter().map(|o|o.map(raw_addr)).collect::<Vec<_>>(),lin,"{label}: range_owner over the stored ranges equals the linear scan");
    let (stored_lin_ms,_)=time(1,||nonces.iter().map(|x|stored_linear(&stored,u128::from(*x))).collect::<Vec<_>>());
    let per=|t:f64|t*1e6/nonces.len() as f64;
    println!("FN1-BENCH {label} members={} two_pass_membership_ms={two_ms:.2} one_pass_membership_ms={one_ms:.2} pre_fn1_members_assignment_draw_ms={pre_ms:.2} child_ctx_uncached_ms={cold_ms:.2} child_ctx_first_ms={first_ms:.2} child_ctx_cached_ms={warm_ms:.3} owner_linear_ns={:.0} owner_binary_ns={:.0} stored_linear_ns={:.0} stored_binary_ns={:.0} lookups={}",
        two.ids().len(),per(lin_ms),per(bin_ms),per(stored_lin_ms),per(stored_ms),nonces.len());
}
/// Spec-1 admission keeps M_max <= 256 (admission.rs GAS_DOMAIN): the legacy set at its full 256.
#[test] fn legacy_full_set_of_256_members_agrees_and_is_timed(){
    let mut cfg=config(false,true,3);cfg.m_max=256;full_set(cfg,256,"spec1-256");
}
/// M_max 10,000 (owner decision 2026-11-13), the v3 set (no M_max term in its gas domain, FN-2b), continuous.
#[test] fn v3_full_set_of_ten_thousand_members_agrees_and_is_timed(){
    let mut cfg=config(true,true,3);cfg.m_max=10_000;cfg.reg_records_max=24_576;cfg.sys_slot_max=147_456;cfg.h_end=0;
    // R20: Z_close >= ceil(REG_RECORDS_MAX/RET) + P2 + D + 1 = 192 + 2 + 4 + 1.
    cfg.z_close=199;
    full_set(cfg,10_000,"v3-10000");
}
