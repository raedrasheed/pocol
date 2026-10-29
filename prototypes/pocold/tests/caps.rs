//! TX_CAP and TXBYTES_CAP at exhaustion. execution.md:83: "TXBYTES_CAP and TX_CAP: the block is invalid,
//! and the builder skips the transaction"; consensus.md:170 (item 16, the counts of the branch up to the
//! parent plus this block); consensus.md:101, §4(a) (a violating template counts as absent); D49 (raw tx bytes).
//! Devnets with small caps (DevnetConfig::tx_cap / txbytes_cap) are mined past exhaustion.
use std::collections::HashMap;
use std::time::{Duration,Instant};
use pocold::{chain::{Chain,Reject},node::{Config,Event,Node},net::{Msg,NetEvent},types::*,hashes};
use pocol_lp4_execution::{devnet::{self,DevnetConfig},txsign::{Key,Tx}};
use revm::primitives::{Address,B256,U256};

const CHAIN:u64=777912;
struct Net{chain:Chain,keys:HashMap<Address,Key>}
/// The consensus.rs test profile (gate-valid with single-host timing: D_att=73, c=8), small caps.
fn cfg(tx_cap:u64,txbytes_cap:u64)->DevnetConfig{let mut c=DevnetConfig::new(CHAIN,1_700_000_000,&["a","b","c"]);c.hash_rate=1024;c.d_att=73;c.tau=40;c.c=8;c.tx_cap=tx_cap;c.txbytes_cap=txbytes_cap;c}
fn spec(c:&DevnetConfig)->pocold::chain::Spec{pocold::chain::Spec::new(devnet::build(c).unwrap().admit(&pocold::single_host_local()).unwrap_or_else(|e|panic!("{e}")).0)}
fn net(c:&DevnetConfig)->Net{let d=devnet::build(c).unwrap();Net{chain:Chain::new(spec(c)),keys:d.miners.iter().map(|m|(m.id,Key::from_secret(&m.key.secret()).unwrap())).collect()}}
fn node(c:&DevnetConfig,tag:&str,miner:Option<Key>,stop_at:Option<u64>)->(Node,std::path::PathBuf){
    let dir=std::env::temp_dir().join(format!("pocold-caps-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    (Node::open(Config{dir:dir.clone(),name:tag.into(),miner,listen:None,peers:vec![],rpc:None,stop_at,log:false,limits:Default::default()},spec(c)).unwrap(),dir)
}
fn try_seal(n:&Net,mut h:pocold::header::Header,body:Body,attempt:u32)->Option<Block>{
    let p=n.chain.full_entry(&h.parent_hash).unwrap();let ctx=n.chain.child_ctx(&p,attempt).unwrap();
    let tid=template_id(&h.ut_raw);let mut shares=vec![];let mut x=0u64;
    loop{if (x as u128)>=ctx.assignment.nonce_max(){return None}let ph=pow_hash(&tid,x);if le_target(&ph,&ctx.t_share){if le_target(&ph,&ctx.target){break}shares.push(x);}x+=1;}
    let owner=addr(&ctx.assignment.owner(x).unwrap());h.nonce=x;shares.truncate(256);h.shares=shares;reseal(&mut h);
    h.winner_sig=n.keys[&owner].sign_hash(&B256::from(hashes::win_msg(&tid,x,&share_root(&h))));reseal(&mut h);Some(Block{header:h,body})
}
fn fallback(n:&Net,parent:[u8;32])->Block{let p=n.chain.full_entry(&parent).unwrap();
    for a in 0..16{let (h,_,_)=n.chain.build_fallback(&p,a).unwrap();if let Some(b)=try_seal(n,h,Body::default(),a){return b}}panic!("no fallback solution")}
/// A signed block by Draw(h,0) on `parent` carrying `txs` (any body, valid or not: it is only built).
fn signed(n:&Net,parent:[u8;32],txs:Vec<Vec<u8>>)->Block{
    let p=n.chain.full_entry(&parent).unwrap();let ctx=n.chain.child_ctx(&p,0).unwrap();let body=Body{txs,evidence:vec![]};
    for off in 0..u64::from(n.chain.spec.cp().D_att){let (h,_,_)=n.chain.build_signed(&p,&n.keys[&ctx.proposer],ctx.window.lower()+off,body.clone(),vec![]).unwrap();
        if let Some(b)=try_seal(n,h,body.clone(),0){return b}}panic!("no signed solution")
}
fn import(n:&mut Net,b:&Block){let (e,_)=n.chain.validate(b,None,0).unwrap_or_else(|e|panic!("h={} {e:?}",b.header.h));n.chain.insert(e);}
fn pay(key:&Key,nonce:u64,data:usize)->Vec<u8>{Tx{chain_id:CHAIN,nonce,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas:21000+16*data as u64,
    to:Some(devnet::account_key("bob").address()),value:U256::from(1u64),data:vec![0x5a;data].into()}.sign_1559(key)}
fn caps_of(n:&Node)->(u64,u64){let cp=n.chain.spec.cp();(cp.TX_CAP,cp.TXBYTES_CAP)}
/// Item 16 on every canonical block, recomputed from the bodies (not from the stored counters).
fn assert_within_caps(n:&Node){
    let (tx_cap,bytes_cap)=caps_of(n);let (mut txs,mut bytes)=(0u64,0u64);
    for h in 1..=n.chain.head().height{let e=n.chain.canonical_at(h).unwrap();let b=n.body_of(&e.hash).unwrap();
        txs+=b.txs.len() as u64;bytes+=b.txs.iter().map(|t|t.len() as u64).sum::<u64>();
        assert!(txs<=tx_cap&&bytes<=bytes_cap,"block {h}: {txs} txs, {bytes} B against {tx_cap}/{bytes_cap}");assert_eq!((e.cum_txs,e.cum_tx_bytes),(txs,bytes),"stored counters at {h}");}
}
/// Mines `count` blocks: each carries exactly the node's own template selection for the head (the
/// builder under test), built and sealed on the harness, and must import as a valid head.
fn mine_templates(n:&mut Node,h:&mut Net,count:usize)->Vec<usize>{
    let mut sizes=vec![];
    for _ in 0..count{
        let head=n.chain.head().meta_clone();let txs=n.template_txs(&head);sizes.push(txs.len());
        assert!(n.chain.within_caps(&head,&txs),"h={}: the builder offers {} transactions past the room {:?}",head.height+1,txs.len(),n.chain.caps_room(&head));
        let b=signed(h,head.hash,txs);
        match n.import(b.clone(),None){Ok(true)=>{},r=>panic!("h={}: the builder's template must be a valid new head, got {r:?}",b.header.h)}
        import(h,&b);
    }
    sizes
}

#[test] fn tx_cap_exhaustion_builder_skips_and_the_chain_keeps_producing(){
    let c=cfg(5,1<<24);let (mut n,dir)=node(&c,"txcap",None,None);let mut h=net(&c);
    let (alice,bob)=(devnet::account_key("alice"),devnet::account_key("bob"));
    for i in 0..6{n.submit_tx(pay(&alice,i,0),false).unwrap();}for i in 0..4{n.submit_tx(pay(&bob,i,0),false).unwrap();}
    // 25 blocks: the cap (5) is reached in the first, then at least 20 more (validation.md:2088).
    let sizes=mine_templates(&mut n,&mut h,25);
    println!("CAPS {}",serde_json::json!({"profile":"tx_cap=5","templateSizes":sizes,"head":n.chain.head().height,"cumTxs":n.chain.head().cum_txs,"pool":n.pool.txs.len()}));
    assert_eq!(sizes[0],5,"the first template fills the remaining TX_CAP exactly");assert!(sizes[1..].iter().all(|s|*s==0),"no transaction beyond TX_CAP: {sizes:?}");
    assert_eq!(n.chain.head().height,25);assert_eq!(n.chain.head().cum_txs,5);assert_within_caps(&n);
    assert_eq!(n.pool.txs.len(),5,"the transactions that can never be included stay pooled (no pool rule is specified)");
    // A hand-built block past the cap is still invalid (item 16), on the harness and through the node.
    let head=n.chain.head;let over=signed(&h,head,vec![n.pool.txs[0].1.clone()]);
    assert!(matches!(h.chain.validate(&over,None,0),Err(Reject::Block(ref w)) if w=="16 caps"));
    assert!(matches!(n.import(over,None),Err(Reject::Block(ref w)) if w=="16 caps"));
    let _=std::fs::remove_dir_all(&dir);
}

#[test] fn txbytes_cap_exhaustion_skips_a_transaction_that_does_not_fit_and_its_senders_later_ones(){
    let (alice,bob)=(devnet::account_key("alice"),devnet::account_key("bob"));
    let (a0,a1,a2)=(pay(&alice,0,0),pay(&alice,1,600),pay(&alice,2,0));let (b0,b1,b2)=(pay(&bob,0,0),pay(&bob,1,0),pay(&bob,2,0));
    // Room for a0, b0, b1 and part of b2: a1 does not fit, a2 follows a1's nonce, b2 does not fit.
    let room=(a0.len()+b0.len()+b1.len()+b2.len()/2) as u64;let l=|t:&Vec<u8>|t.len() as u64;
    assert!(l(&a0)+l(&a1)>room&&l(&a0)+l(&b0)+l(&b1)<=room&&l(&a0)+l(&b0)+l(&b1)+l(&b2)>room);
    let c=cfg(1000,room);let (mut n,dir)=node(&c,"txbytes",None,None);let mut h=net(&c);
    for t in [&a0,&a1,&a2,&b0,&b1,&b2]{n.submit_tx(t.clone(),false).unwrap();}
    let head=n.chain.head().meta_clone();assert_eq!(n.template_txs(&head),vec![a0.clone(),b0.clone(),b1.clone()],"skips a1 (too large), a2 (after a1), keeps bob's");
    let sizes=mine_templates(&mut n,&mut h,21);
    println!("CAPS {}",serde_json::json!({"profile":format!("txbytes_cap={room}"),"templateSizes":sizes,"cumTxBytes":n.chain.head().cum_tx_bytes,"pool":n.pool.txs.len()}));
    assert_eq!(sizes[0],3);assert!(sizes[1..].iter().all(|s|*s==0),"{sizes:?}");assert_within_caps(&n);
    assert_eq!(n.chain.head().cum_tx_bytes,(a0.len()+b0.len()+b1.len()) as u64);
    let head=n.chain.head;let over=signed(&h,head,vec![b2.clone()]);
    assert!(matches!(n.import(over,None),Err(Reject::Block(ref w)) if w=="16 caps"),"b2 passes TXBYTES_CAP by itself");
    let _=std::fs::remove_dir_all(&dir);
}

#[test] fn txbytes_cap_boundary_is_byte_exact_in_the_builder_and_in_item_16(){
    // No slack: TXBYTES_CAP = a0+b0 (an exact fit, filled and valid) and a0+b0-1 (b0 one byte over: skipped by
    // the builder, and a block carrying it is "16 caps"). Raw bytes only (D49), no RLP item header.
    let (alice,bob)=(devnet::account_key("alice"),devnet::account_key("bob"));let (a0,b0,b1)=(pay(&alice,0,0),pay(&bob,0,0),pay(&bob,1,0));
    for short in [0u64,1]{
        let cap=(a0.len()+b0.len()) as u64-short;let c=cfg(1000,cap);let (mut n,dir)=node(&c,&format!("txbytes-edge{short}"),None,None);let mut h=net(&c);
        // Item 16 by itself, on a hand-built block at cum_tx_bytes = TXBYTES_CAP + short.
        let both=signed(&h,h.chain.head,vec![a0.clone(),b0.clone()]);let v=h.chain.validate(&both,None,0).map(|_|());
        assert!(if short==0{v.is_ok()}else{matches!(v,Err(Reject::Block(ref w)) if w=="16 caps")},"item 16 at TXBYTES_CAP+{short}: {v:?}");
        for t in [&a0,&b0,&b1]{n.submit_tx(t.clone(),false).unwrap();}
        let want=if short==0{vec![a0.clone(),b0.clone()]}else{vec![a0.clone()]};
        let head=n.chain.head().meta_clone();assert_eq!(n.template_txs(&head),want,"TXBYTES_CAP = a0+b0-{short}");
        let mut sizes=mine_templates(&mut n,&mut h,1);let fill=n.chain.head;
        let used:u64=want.iter().map(|t|t.len() as u64).sum();assert_eq!(n.chain.head().cum_tx_bytes,used);assert_eq!(cap-used,if short==0{0}else{b0.len() as u64-1});
        // The next transaction passes the room by its whole length (exact fit) or by one byte. It is checked
        // on the block that used the room (no empty block between: the parent's counter, not the grandparent's).
        let next=if short==0{b1.clone()}else{b0.clone()};let over=signed(&h,fill,vec![next]);
        assert!(matches!(h.chain.validate(&over,None,0),Err(Reject::Block(ref w)) if w=="16 caps"));
        assert!(matches!(n.import(over,None),Err(Reject::Block(ref w)) if w=="16 caps"),"item 16 on the block that reached cum_tx_bytes={used}");
        sizes.extend(mine_templates(&mut n,&mut h,2));assert_eq!(sizes,vec![want.len(),0,0]);assert_within_caps(&n);
        println!("CAPS {}",serde_json::json!({"profile":format!("txbytes_cap=a0+b0-{short}={cap}"),"templateSizes":sizes,"cumTxBytes":used,"roomLeft":cap-used}));
        let _=std::fs::remove_dir_all(&dir);
    }
    // Across blocks: TXBYTES_CAP = a0+b0 with a0 alone in block 1. Then b0 fills the room exactly as block 2's
    // first transaction, on a non-zero parent counter (builder and item 16), and b1 is past it on block 2.
    let cap=(a0.len()+b0.len()) as u64;let c=cfg(1000,cap);let (mut n,dir)=node(&c,"txbytes-cross",None,None);let mut h=net(&c);
    n.submit_tx(a0.clone(),false).unwrap();let head=n.chain.head().meta_clone();assert_eq!(n.template_txs(&head),vec![a0.clone()]);
    let mut sizes=mine_templates(&mut n,&mut h,1);
    for t in [&b0,&b1]{n.submit_tx(t.clone(),false).unwrap();}
    let head=n.chain.head().meta_clone();assert_eq!(n.chain.caps_room(&head).1,b0.len() as u64);assert_eq!(n.template_txs(&head),vec![b0.clone()],"b0 fits the room left by block 1 exactly");
    sizes.extend(mine_templates(&mut n,&mut h,1));let fill=n.chain.head;assert_eq!(n.chain.head().cum_tx_bytes,cap,"block 2 reaches TXBYTES_CAP and is a valid head");
    let over=signed(&h,fill,vec![b1.clone()]);
    assert!(matches!(h.chain.validate(&over,None,0),Err(Reject::Block(ref w)) if w=="16 caps"));
    assert!(matches!(n.import(over,None),Err(Reject::Block(ref w)) if w=="16 caps"),"b1 on the block that reached TXBYTES_CAP");
    sizes.extend(mine_templates(&mut n,&mut h,2));assert_eq!(sizes,vec![1,1,0,0]);assert_within_caps(&n);
    println!("CAPS {}",serde_json::json!({"profile":format!("txbytes_cap=a0+b0={cap}, a0 then b0"),"templateSizes":sizes,"cumTxBytes":n.chain.head().cum_tx_bytes,"roomLeft":0}));
    let _=std::fs::remove_dir_all(&dir);
}

/// Every position and block shape at the boundary of one counter (TX_CAP, or TXBYTES_CAP by raw bytes):
/// the parent's counter zero (genesis) or not (a0, a1 in block 1), then k = 1..3 of bob's transfers that
/// reach the cap exactly (short 0) or pass it by one transaction or one byte (short 1). Item 16 accepts a
/// block of all k only at the exact fit (harness and node); the builder, with bob's four pooled, offers
/// exactly his first k-short (each one checked against the room the ones before it left), and its block
/// is a valid head, at the cap when short is 0.
fn boundary_matrix(bytes:bool){
    let (alice,bob)=(devnet::account_key("alice"),devnet::account_key("bob"));
    let pre:Vec<Vec<u8>>=(0..2).map(|i|pay(&alice,i,0)).collect();let bs:Vec<Vec<u8>>=(0..4).map(|i|pay(&bob,i,0)).collect();
    let l=|t:&[Vec<u8>]|t.iter().map(|x|x.len() as u64).sum::<u64>();let mut cases=vec![];
    for np in [0usize,2]{for k in 1..=3usize{for short in [0usize,1]{
        let cap=if bytes{l(&pre[..np])+l(&bs[..k])-short as u64}else{(np+k-short) as u64};if cap==0{continue}// TX_CAP 0: nothing to fit
        let c=if bytes{cfg(1000,cap)}else{cfg(cap,1<<24)};let tag=format!("edge-{}{np}{k}{short}",if bytes{"b"}else{"t"});
        let (mut n,dir)=node(&c,&tag,None,None);let mut h=net(&c);
        if np>0{for t in &pre{n.submit_tx(t.clone(),false).unwrap();}assert_eq!(mine_templates(&mut n,&mut h,1),vec![np],"{tag}: block 1");}
        let (p,txs0,bytes0)=(n.chain.head,n.chain.head().cum_txs,n.chain.head().cum_tx_bytes);
        // On a zero-count parent, k > TX_CAP transactions cannot be built (build_signed applies them, and
        // apply_block refuses more than TX_CAP in one block, apply.rs:250): item 16's count is that check there.
        if bytes||np>0||short==0{
            let all=signed(&h,p,bs[..k].to_vec());let v=h.chain.validate(&all,None,0).map(|_|());
            if short==0{assert!(v.is_ok(),"{tag}: the exact fit of all {k} is valid: {v:?}")}else{
                assert!(matches!(v,Err(Reject::Block(ref w)) if w=="16 caps"),"{tag}: {k} transactions one over the cap: {v:?}");
                assert!(matches!(n.import(all,None),Err(Reject::Block(ref w)) if w=="16 caps"),"{tag}: one over the cap, through the node");}
        }
        for t in &bs{n.submit_tx(t.clone(),false).unwrap();}
        let fit=bs[..k-short].to_vec();let head=n.chain.head().meta_clone();assert_eq!(n.template_txs(&head),fit,"{tag}: the builder offers bob's first {}",k-short);
        mine_templates(&mut n,&mut h,1);let e=n.chain.head();assert_eq!((e.cum_txs,e.cum_tx_bytes),(txs0+fit.len() as u64,bytes0+l(&fit)),"{tag}");
        if short==0{assert_eq!(if bytes{e.cum_tx_bytes}else{e.cum_txs},cap,"{tag}: the builder's block is a valid head at the cap")}
        assert_within_caps(&n);cases.push(serde_json::json!([np,k,short,cap]));let _=std::fs::remove_dir_all(&dir);
    }}}
    println!("CAPS {}",serde_json::json!({"profile":format!("boundary matrix {}",if bytes{"TXBYTES_CAP"}else{"TX_CAP"}),"cases [parentTxs,k,short,cap]":cases,
        "txLen":bs.iter().chain(&pre).map(|t|t.len()).collect::<Vec<_>>()}));
}
#[test] fn tx_cap_boundary_is_exact_at_every_position_and_block_shape(){boundary_matrix(false)}
#[test] fn txbytes_cap_boundary_is_exact_at_every_position_and_block_shape(){boundary_matrix(true)}

#[test] fn counters_are_per_branch_across_a_reorg_at_the_exhaustion_boundary(){
    let c=cfg(3,1<<24);let alice=devnet::account_key("alice");let t:Vec<Vec<u8>>=(0..4).map(|i|pay(&alice,i,0)).collect();
    let (mut n,dir)=node(&c,"reorg",None,None);let mut ha=net(&c);let g=ha.chain.head;
    // Branch A exhausts TX_CAP in its first block; the node's builder then offers nothing on A.
    let a1=signed(&ha,g,t[..3].to_vec());import(&mut ha,&a1);let a2=fallback(&ha,a1.hash());import(&mut ha,&a2);
    for b in [&a1,&a2]{n.import(b.clone(),None).unwrap();}n.submit_tx(t[3].clone(),false).unwrap();
    assert_eq!(n.chain.head().cum_txs,3);let head=n.chain.head().meta_clone();assert!(n.template_txs(&head).is_empty(),"A is exhausted");
    let over=signed(&ha,a2.hash(),vec![t[3].clone()]);assert!(matches!(ha.chain.validate(&over,None,0),Err(Reject::Block(ref w)) if w=="16 caps"));
    // Branch B (empty, bob's transfer, empty; heavier) from genesis: D133 adopts it by work alone; its counters are its own.
    let mut hb=net(&c);let b1=fallback(&hb,g);import(&mut hb,&b1);let b2=signed(&hb,b1.hash(),vec![pay(&devnet::account_key("bob"),0,0)]);import(&mut hb,&b2);
    let b3=fallback(&hb,b2.hash());import(&mut hb,&b3);
    // A is the head: B's block carrying one transaction fits B's room (3), not A's (0), and is valid.
    n.import(b1.clone(),None).unwrap();assert_eq!(n.chain.head,a2.hash());
    let r=n.import(b2.clone(),None);assert!(r.is_ok(),"b2 is checked against B's count (0+1), not the head's (3+1): {r:?}");
    n.import(b3.clone(),None).unwrap();assert_eq!(n.chain.head,b3.hash(),"reorganized to B");assert_eq!(n.chain.head().cum_txs,1);
    // B is the head: A's next block fits B's room (2), not A's (0), and is "16 caps" (A's count 3+1, not the head's 1+1).
    assert!(matches!(n.import(over,None),Err(Reject::Block(ref w)) if w=="16 caps"),"item 16 on A after the reorg");
    n.pool_settle();
    assert_eq!(n.pool.txs.iter().map(|x|x.1.clone()).collect::<Vec<_>>(),t,"A's transactions are detached back into the pool (A11)");
    // On B the builder fills B's room (2) and skips the rest, which is then past the cap on B too.
    let head=n.chain.head().meta_clone();assert_eq!(n.template_txs(&head),t[..2].to_vec());
    let sizes=mine_templates(&mut n,&mut hb,3);assert_eq!(sizes,vec![2,0,0]);assert_within_caps(&n);assert_eq!(n.chain.head().cum_txs,3);
    let over=signed(&hb,n.chain.head,vec![t[2].clone()]);assert!(matches!(n.import(over,None),Err(Reject::Block(ref w)) if w=="16 caps"));
    println!("CAPS {}",serde_json::json!({"profile":"reorg tx_cap=3","cumTxsA":3,"cumTxsB":1,"roomA":0,"roomB":2,"templateSizesOnB":sizes}));
    let _=std::fs::remove_dir_all(&dir);
    // TXBYTES_CAP = big+b0-1. A (big) is left one byte short of b0; B (empty, b0, empty; heavier) forks at
    // genesis. Whichever branch is the head, a block is checked against its own branch's byte counter.
    let (bob,l)=(devnet::account_key("bob"),|t:&Vec<u8>|t.len() as u64);let (big,b0,next)=(pay(&alice,0,600),pay(&bob,0,0),pay(&alice,1,0));
    let cap=l(&big)+l(&b0)-1;assert!(l(&next)>=l(&b0)&&l(&b0)+l(&next)<=cap);
    let c=cfg(1000,cap);let (mut n,dir)=node(&c,"reorg-bytes",None,None);let (mut ha,mut hb)=(net(&c),net(&c));let g=ha.chain.head;
    let a1=signed(&ha,g,vec![big.clone()]);import(&mut ha,&a1);let a2=fallback(&ha,a1.hash());import(&mut ha,&a2);
    for b in [&a1,&a2]{n.import(b.clone(),None).unwrap();}
    let head=n.chain.head().meta_clone();assert_eq!(n.chain.caps_room(&head).1,l(&b0)-1);
    let b1=fallback(&hb,g);import(&mut hb,&b1);let b2=signed(&hb,b1.hash(),vec![b0.clone()]);import(&mut hb,&b2);let b3=fallback(&hb,b2.hash());import(&mut hb,&b3);
    // A is the head: B's block carrying b0 fits B's room (all of TXBYTES_CAP), not A's (b0-1), and is valid.
    n.import(b1.clone(),None).unwrap();assert_eq!(n.chain.head,a2.hash());
    let r=n.import(b2.clone(),None);assert!(r.is_ok(),"b2 is checked against B's counter (0), not the head's ({}): {r:?}",l(&big));
    n.import(b3.clone(),None).unwrap();assert_eq!(n.chain.head,b3.hash(),"reorganized to B");assert_eq!(n.chain.head().cum_tx_bytes,l(&b0));
    // B is the head: A's next block fits B's room (big-1), not A's (b0-1), and is "16 caps".
    let over=signed(&ha,a2.hash(),vec![next.clone()]);assert!(matches!(ha.chain.validate(&over,None,0),Err(Reject::Block(ref w)) if w=="16 caps"));
    assert!(matches!(n.import(over,None),Err(Reject::Block(ref w)) if w=="16 caps"),"item 16 on A reads A's counter ({}), not the head's ({})",l(&big),l(&b0));
    // On B the builder keeps A's detached transaction pooled: B's room is one byte short of it.
    n.pool_settle();
    assert_eq!(n.pool.txs.iter().map(|x|x.1.clone()).collect::<Vec<_>>(),vec![big.clone()],"detached (A11)");
    let head=n.chain.head().meta_clone();assert!(n.template_txs(&head).is_empty());assert_within_caps(&n);
    println!("CAPS {}",serde_json::json!({"profile":format!("reorg txbytes_cap=big+b0-1={cap}"),"cumTxBytesA":l(&big),"cumTxBytesB":l(&b0),"roomA":l(&b0)-1,"roomB":l(&big)-1}));
    let _=std::fs::remove_dir_all(&dir);
}

#[test] fn a_received_template_past_the_caps_is_treated_as_absent(){
    // consensus.md:101, §4(a): a violating template is as if absent (no lock; sticky FB at T_j). Positive control:
    // the same template shape within the caps locks.
    let alice=devnet::account_key("alice");let t:Vec<Vec<u8>>=(0..3).map(|i|pay(&alice,i,0)).collect();
    for (tag,txs,locks) in [("tpl-over",vec![t[2].clone()],false),("tpl-ok",vec![],true)]{
        let c=cfg(2,1<<24);let mut h=net(&c);let g=h.chain.head;let b1=signed(&h,g,t[..2].to_vec());import(&mut h,&b1);
        let (mut n,dir)=node(&c,tag,None,None);n.import(b1.clone(),None).unwrap();
        assert!(n.template_json().is_null(),"waiting for a template at h=2");
        let p=h.chain.get(&b1.hash()).unwrap().clone();let ctx=h.chain.child_ctx(&p,0).unwrap();let body=Body{txs,evidence:vec![]};
        let (shell,_,_)=h.chain.build_signed(&p,&h.keys[&ctx.proposer],ctx.window.lower(),body.clone(),vec![]).unwrap();
        n.handle(Event::Net(NetEvent::Message(1,Msg::Template{shell:shell.encoded.clone(),body:body.encode()})));
        let v=n.template_json();assert_eq!(!v.is_null(),locks,"{tag}: {v}");
        if locks{assert_eq!(v["fallback"],false)}
        let _=std::fs::remove_dir_all(&dir);
    }
}

/// A single-miner devnet whose genesis is now, mined through the whole production path (pool,
/// build_signed_template, miner, import) until `stop_at`, or for at most 240 s.
fn live(tag:&str,tx_cap:u64,txbytes_cap:u64,pool:u64,stop_at:u64)->Node{
    let g=pocold::node::now_s();let mut c=DevnetConfig::new(CHAIN,g,&["a"]);c.hash_rate=1024;c.d_att=73;c.tau=40;c.c=8;c.tx_cap=tx_cap;c.txbytes_cap=txbytes_cap;
    // Own data directory: the tag is shared with an in-process test of the same binary (same PID).
    let (mut n,dir)=node(&c,&format!("live-{tag}"),Some(devnet::miner_identity("a").key),Some(stop_at));let alice=devnet::account_key("alice");
    n.cfg.log=true;// the production trace (locks, own-block rejections, attempt expiry) goes to stderr
    for i in 0..pool{n.submit_tx(pay(&alice,i,0),false).unwrap();}
    let t=Instant::now();let run=std::thread::spawn(move||{n.run();n});
    while !run.is_finished()&&t.elapsed()<Duration::from_secs(240){std::thread::sleep(Duration::from_millis(100));}
    assert!(run.is_finished(),"{tag}: the node did not reach height {stop_at} within 240 s (production stalled)");
    let n=run.join().unwrap();let _=std::fs::remove_dir_all(&dir);
    let signed:Vec<bool>=(1..=n.chain.head().height).map(|h|n.chain.canonical_at(h).unwrap().signed).collect();
    println!("CAPS {}",serde_json::json!({"profile":format!("live {tag}"),"head":n.chain.head().height,"cumTxs":n.chain.head().cum_txs,"cumTxBytes":n.chain.head().cum_tx_bytes,
        "signed":signed,"seconds":t.elapsed().as_secs_f64(),"rejects":n.rejects.iter().map(|r|r.1.clone()).collect::<Vec<_>>()}));
    assert_within_caps(&n);assert!(n.rejects.iter().all(|r|!r.1.contains("16 caps")),"{tag}: an own block was rejected for the caps: {:?}",n.rejects);n
}
#[test] fn a_mining_devnet_node_includes_up_to_tx_cap_and_keeps_producing(){
    // Six pooled transfers, TX_CAP = 3. Before the fix the template carried all six, apply_block refused more
    // than TX_CAP transactions in one block, and the builder fell back to an empty body every time: nothing
    // was ever included although the cap had room.
    let n=live("txcap",3,1<<24,6,8);assert_eq!(n.chain.head().cum_txs,3,"TX_CAP reached exactly");
}
#[test] fn a_mining_devnet_node_keeps_producing_past_txbytes_cap(){
    // Six pooled transfers, room for two and a half by TXBYTES_CAP. Before the fix the template carried all
    // six, the node's own block was invalid (item 16), and every height waited for the attempt to expire
    // (D_att+Γ = 78 s) before a fallback block.
    let t=pay(&devnet::account_key("alice"),0,0);let room=(5*t.len()/2) as u64;
    let n=live("txbytes",1000,room,6,8);assert_eq!(n.chain.head().cum_txs,2,"two transfers fit TXBYTES_CAP");
}

// ---------------- PR #27 review R1-1: a skipped transaction's dependents ----------------
fn send(key:&Key,nonce:u64,to:Address,value:U256,data:usize)->Vec<u8>{Tx{chain_id:CHAIN,nonce,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),
    gas:21000+16*data as u64,to:Some(to),value,data:vec![0x5a;data].into()}.sign_1559(key)}
/// alice's large transfer funds dave, who has nothing on the head; dave's small transfers depend on it
/// (they were admitted on the pool's pending state, which includes alice's). The byte room is below
/// alice's transfer, so the builder skips it. Before the fix the builder then offered dave's transfers,
/// which cannot execute on the head, and not bob's independent one; with more than 8 such dependents
/// no signed template was built at all. Now the selection is executed on a running copy of the parent
/// state: dave's are skipped with alice's, and bob's transfer is selected and the template builds.
#[test] fn the_builder_skips_the_dependents_of_a_skipped_transaction(){
    let (alice,bob,dave)=(devnet::account_key("alice"),devnet::account_key("bob"),devnet::account_key("dave-unfunded"));
    let big=send(&alice,0,dave.address(),U256::from(10u128.pow(18)),2000);
    let ds:Vec<Vec<u8>>=(0..9u64).map(|i|send(&dave,i,bob.address(),U256::from(1u64),0)).collect();
    let b0=send(&bob,0,alice.address(),U256::from(1u64),0);
    let room:u64=ds.iter().map(|t|t.len() as u64).sum();assert!(room<big.len() as u64&&b0.len() as u64<=room);
    let c=cfg(1000,room);let (mut n,dir)=node(&c,"r1dep",None,None);let h=net(&c);
    for t in std::iter::once(&big).chain(ds.iter()).chain(std::iter::once(&b0)){n.submit_tx(t.clone(),false).unwrap();}
    assert_eq!(n.pool.txs.len(),11,"all admitted on the pending state");
    let head=n.chain.head().clone();let offered=n.template_txs(&head);
    assert_eq!(offered,vec![b0.clone()],"only bob's independent transfer: dave's depend on alice's skipped one");
    let ctx=h.chain.child_ctx(&head,0).unwrap();
    assert!(h.chain.build_signed(&head,&h.keys[&ctx.proposer],ctx.window.lower(),Body{txs:offered,evidence:vec![]},vec![]).is_ok(),"the selection executes on the head");
    let _=std::fs::remove_dir_all(&dir);
}
/// Review R1-4: a block of TX_CAP+1 transactions on a parent with no transactions (here TX_CAP 2, three
/// transfers on genesis) is refused, whether the builder refuses to build it or validation refuses it.
/// Item 16 and the USER budget check each hold this alone, so removing either one alone changed no test;
/// this pins the case itself.
#[test] fn a_block_over_tx_cap_on_an_empty_parent_is_refused(){
    let c=cfg(2,1<<24);let n=net(&c);let alice=devnet::account_key("alice");
    let txs:Vec<Vec<u8>>=(0..3).map(|i|pay(&alice,i,0)).collect();
    let p=n.chain.head().clone();assert_eq!(p.cum_txs,0);let ctx=n.chain.child_ctx(&p,0).unwrap();let body=Body{txs,evidence:vec![]};
    for off in 0..u64::from(n.chain.spec.cp().D_att){
        match n.chain.build_signed(&p,&n.keys[&ctx.proposer],ctx.window.lower()+off,body.clone(),vec![]){
            Err(_)=>return,// refused at build: the same checks as validation
            Ok((h,_,_))=>if let Some(b)=try_seal(&n,h,body.clone(),0){
                assert!(n.chain.validate(&b,None,0).is_err(),"three transactions at TX_CAP 2 on an empty parent must be invalid");return}}
    }
    panic!("no sealed block to check");
}
