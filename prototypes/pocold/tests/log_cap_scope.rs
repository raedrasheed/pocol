//! LOG_CAP (execution.md:82; LOG-CAP-AGGREGATE, chain scope, provisional) at its boundary, in the caps-test
//! build (LOG_CAP = 2^24, validation.md:2065): accumulation inside one block, the per-branch scope and the
//! re-execution of a pruned state. Written by the independent caps review of 2026-10-24 (R1-2: mutants L1, L2
//! and L4 survived every earlier test). One LOGN fill of 32,768 charged bytes per block.
//! Run with `cargo test --release --features caps-test --test log_cap_scope`.
#![cfg(feature="caps-test")]
use std::collections::HashMap;
use pocold::{chain::{Chain,Reject},types::*,hashes,rlp};
use pocol_lp4_execution::{devnet::{self,DevnetConfig},txsign::{Key,Tx},user_phase::caps};
use revm::primitives::{Address,B256,Bytes,U256};

const CHAIN:u64=777913;
struct Net{chain:Chain,keys:HashMap<Address,Key>}
fn cfg()->DevnetConfig{let mut c=DevnetConfig::new(CHAIN,1_700_000_000,&["a","b","c"]);c.hash_rate=1024;c.d_att=73;c.tau=40;c.c=8;c}
fn spec(c:&DevnetConfig)->pocold::chain::Spec{pocold::chain::Spec::new(devnet::build(c).unwrap().admit(&pocold::single_host_local()).unwrap_or_else(|e|panic!("{e}")).0)}
fn try_seal(n:&Net,mut h:pocold::header::Header,body:Body,attempt:u32)->Option<Block>{
    let p=n.chain.full_entry(&h.parent_hash).unwrap();let ctx=n.chain.child_ctx(&p,attempt).unwrap();
    let tid=template_id(&h.ut_raw);let mut shares=vec![];let mut x=0u64;
    loop{if (x as u128)>=ctx.assignment.nonce_max(){return None}let ph=pow_hash(&tid,x);if le_target(&ph,&ctx.t_share){if le_target(&ph,&ctx.target){break}shares.push(x);}x+=1;}
    let owner=addr(&ctx.assignment.owner(x).unwrap());h.nonce=x;shares.truncate(256);h.shares=shares;reseal(&mut h);
    h.winner_sig=n.keys[&owner].sign_hash(&B256::from(hashes::win_msg(&tid,x,&share_root(&h))));reseal(&mut h);Some(Block{header:h,body})
}
fn signed(n:&Net,parent:[u8;32],txs:Vec<Vec<u8>>)->Block{
    let p=n.chain.full_entry(&parent).unwrap();let ctx=n.chain.child_ctx(&p,0).unwrap();let body=Body{txs,evidence:vec![]};
    for off in 3..u64::from(n.chain.spec.cp().D_att){let (h,_,_)=n.chain.build_signed(&p,&n.keys[&ctx.proposer],ctx.window.lower()+off,body.clone(),vec![]).unwrap();
        if let Some(b)=try_seal(n,h,body.clone(),0){return b}}panic!("no signed solution")
}
fn fallback(n:&Net,parent:[u8;32])->Block{let p=n.chain.full_entry(&parent).unwrap();
    for a in 0..16{let (h,_,_)=n.chain.build_fallback(&p,a).unwrap();if let Some(b)=try_seal(n,h,Body::default(),a){return b}}panic!("no fallback solution")}
fn import(n:&mut Net,b:&Block)->pocold::chain::Entry{let (e,_)=n.chain.validate(b,None,0).unwrap_or_else(|e|panic!("h={} {e:?}",b.header.h));n.chain.insert(e.clone());e}
fn tx(key:&Key,nonce:u64,to:Option<Address>,data:Vec<u8>,gas:u64)->Vec<u8>{
    Tx{chain_id:CHAIN,nonce,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas,to,value:U256::ZERO,data:Bytes::from(data)}.sign_1559(key)}
fn status(e:&pocold::chain::Entry)->Vec<bool>{e.receipts.iter().map(|r|{let it=rlp::decode(&r[1..]).unwrap();it.list().unwrap()[0].bytes().unwrap()==[1]}).collect()}
fn word(v:u64)->[u8;32]{U256::from(v).to_be_bytes()}
const LOGN:&[u8]=&[0x5f,0x35,0x5b,0x80,0x15,0x61,0x00,0x15,0x57,0x60,0x20,0x35,0x5f,0xa0,0x60,0x01,0x90,0x03,0x60,0x02,0x56,0x5b,0x00];
fn deploy(code:&[u8])->Vec<u8>{let n=code.len();let mut v=vec![0x5f+n as u8];v.extend_from_slice(code);v.extend([0x5f,0x52,0x60,n as u8,0x60,(32-n) as u8,0xf3]);v}

#[test] fn log_cap_accumulates_in_a_block_follows_the_branch_and_survives_a_rebuild(){
    let c=cfg();let d=devnet::build(&c).unwrap();let mut n=Net{chain:Chain::new(spec(&c)),keys:d.miners.iter().map(|m|(m.id,Key::from_secret(&m.key.secret()).unwrap())).collect()};
    let (alice,bob)=(devnet::account_key("alice"),devnet::account_key("bob"));let logn=alice.address().create(0);
    let b=signed(&n,n.chain.head,vec![tx(&alice,0,None,deploy(LOGN),200_000)]);import(&mut n,&b);
    // charge c = 36 + d for one LOG0 of d bytes.
    let fill=|k:&Key,nonce:u64,charge:u64|tx(k,nonce,Some(logn),[word(1),word(charge-36)].concat(),400_000);
    let mut an=1u64;
    for _ in 0..(caps::LOG_CAP/32768-1){let b=signed(&n,n.chain.head,vec![fill(&alice,an,32768)]);an+=1;import(&mut n,&b);}
    let b=signed(&n,n.chain.head,vec![fill(&alice,an,16384)]);an+=1;let x=import(&mut n,&b);
    assert_eq!(x.cum_log_bytes,caps::LOG_CAP-16384);let xh=x.hash;
    // (1) In-block accumulation: two charges of 10,000 on room 16,384: the first fits, the second halts (LogCap).
    let y=signed(&n,xh,vec![fill(&alice,an,10_000),fill(&alice,an+1,10_000)]);let ye=import(&mut n,&y);
    println!("LOGCAP Y status={:?} cum={}",status(&ye),ye.cum_log_bytes);
    assert_eq!(status(&ye),vec![true,false],"the second transaction passes LOG_CAP with the first one's bytes");
    assert_eq!(ye.cum_log_bytes,caps::LOG_CAP-6384);assert_eq!(n.chain.head,ye.hash);
    // (2) Branch scope: a sibling of Y on X (room 16,384) carrying an exact 16,384 fit succeeds, although the
    // head's room is 6,384.
    let z=signed(&n,xh,vec![fill(&bob,0,16384)]);let (ze,_)=n.chain.validate(&z,None,0).unwrap();
    println!("LOGCAP Z status={:?} cum={} head cum={}",status(&ze),ze.cum_log_bytes,n.chain.head().cum_log_bytes);
    assert_eq!(status(&ze),vec![true],"Z is checked against its own branch (X), not the head (Y)");assert_eq!(ze.cum_log_bytes,caps::LOG_CAP);
    n.chain.insert(ze);
    // (3) Rebuild: extend Y past K_hist, prune, then validate a child of Y (its state is rebuilt by re-execution).
    let mut tip=ye.hash;for _ in 0..40{let b=fallback(&n,tip);tip=import(&mut n,&b).hash;}
    n.chain.retain();assert!(n.chain.get(&ye.hash).unwrap().pruned,"Y's state is pruned");
    match n.chain.ensure_state(&ye.hash){Ok(())=>{},Err(e)=>panic!("rebuild of Y by re-execution: {e:?}")}
    let w=signed(&n,ye.hash,vec![fill(&bob,0,6384)]);let (we,_)=n.chain.validate(&w,None,0).unwrap_or_else(|e:Reject|panic!("{e:?}"));
    println!("LOGCAP W status={:?} cum={} rebuilds={}",status(&we),we.cum_log_bytes,n.chain.rebuilds);
    assert_eq!(status(&we),vec![true]);assert_eq!(we.cum_log_bytes,caps::LOG_CAP);
}
