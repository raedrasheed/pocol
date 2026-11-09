//! Inbound eviction's credits (VA-SPEC §1b, net::Net::establish): the node credits a peer for its first delivery of a
//! new block, a template that locked, a transaction that entered the pool and a valid new share, and never for a
//! duplicate. In-process: a real handshake (support/auth_peer.rs), then handler events; read through the status.
use std::collections::HashMap;
use pocold::{chain::Chain,node::{Config,Event,Node},net::{Msg,NetEvent},types::*,hashes};
use pocol_lp4_execution::{devnet::{self,DevnetConfig},txsign::{Key,Tx}};
use revm::primitives::{Address,B256,U256};
#[path="support/auth_peer.rs"] mod auth_peer;

const CHAIN:u64=777913;
/// The caps.rs test profile (gate-valid with single-host timing: D_att=73, c=8).
fn cfg()->DevnetConfig{let mut c=DevnetConfig::new(CHAIN,1_700_000_000,&["a","b","c"]);c.hash_rate=1024;c.d_att=73;c.tau=40;c.c=8;c}
fn spec(c:&DevnetConfig)->pocold::chain::Spec{pocold::chain::Spec::new(devnet::build(c).unwrap().admit(&pocold::single_host_local()).unwrap_or_else(|e|panic!("{e}")).0)}
/// The block producer's chain and every miner's key, apart from the node under test (tests/caps.rs).
struct Producer{chain:Chain,keys:HashMap<Address,Key>}
fn producer(c:&DevnetConfig)->Producer{let d=devnet::build(c).unwrap();Producer{chain:Chain::new(spec(c)),keys:d.miners.iter().map(|m|(m.id,Key::from_secret(&m.key.secret()).unwrap())).collect()}}
fn try_seal(n:&Producer,mut h:pocold::header::Header,body:Body,attempt:u32)->Option<Block>{
    let p=n.chain.full_entry(&h.parent_hash).unwrap();let ctx=n.chain.child_ctx(&p,attempt).unwrap();
    let tid=template_id(&h.ut_raw);let mut shares=vec![];let mut x=0u64;
    loop{if (x as u128)>=ctx.assignment.nonce_max(){return None}let ph=pow_hash(&tid,x);if le_target(&ph,&ctx.t_share){if le_target(&ph,&ctx.target){break}shares.push(x);}x+=1;}
    let owner=addr(&ctx.assignment.owner(x).unwrap());h.nonce=x;shares.truncate(256);h.shares=shares;reseal(&mut h);
    h.winner_sig=n.keys[&owner].sign_hash(&B256::from(hashes::win_msg(&tid,x,&share_root(&h))));reseal(&mut h);Some(Block{header:h,body})
}
/// A signed block by Draw(h,0) on `parent`, empty (tests/caps.rs `signed`).
fn signed(n:&Producer,parent:[u8;32])->Block{
    let p=n.chain.full_entry(&parent).unwrap();let ctx=n.chain.child_ctx(&p,0).unwrap();
    for off in 0..u64::from(n.chain.spec.cp().D_att){let (h,_,_)=n.chain.build_signed(&p,&n.keys[&ctx.proposer],ctx.window.lower()+off,Body::default(),vec![]).unwrap();
        if let Some(b)=try_seal(n,h,Body::default(),0){return b}}panic!("no signed solution")
}
fn pay(key:&Key,nonce:u64)->Vec<u8>{Tx{chain_id:CHAIN,nonce,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas:21000,
    to:Some(devnet::account_key("bob").address()),value:U256::from(1u64),data:vec![].into()}.sign_1559(key)}
/// stats.credits{Block,Template,Tx,Share}.
fn credits(n:&Node)->[u64;4]{let s=n.status_json();["creditsBlock","creditsTemplate","creditsTx","creditsShare"].map(|k|s["stats"][k].as_u64().unwrap_or_else(||panic!("stats.{k} missing: {}",s["stats"])))}

#[test] fn new_blocks_templates_and_transactions_credit_their_sender_once(){
    let c=cfg();let mut h=producer(&c);let g=h.chain.head;let b1=signed(&h,g);
    let dir=std::env::temp_dir().join(format!("pocold-eviction-credit-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let mut n=Node::open(Config{dir:dir.clone(),name:"credit".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Default::default()},spec(&c)).unwrap();
    let (peer,_socket)=auth_peer::connect(&mut n);
    let deliver=|n:&mut Node,m:Msg|n.handle(Event::Net(NetEvent::Message(peer,m)));
    assert_eq!(credits(&n),[0;4]);
    // A new block, then the same block again.
    let block=Msg::Block{header:b1.header.encoded.clone(),body:b1.body.encode()};
    deliver(&mut n,block.clone());assert_eq!(n.chain.head,b1.hash(),"imported");assert_eq!(credits(&n),[1,0,0,0],"a new block credits its sender");
    deliver(&mut n,block);assert_eq!(credits(&n),[1,0,0,0],"a duplicate earns nothing");
    // Draw(2,0)'s template on b1 locks, then a copy of it arrives.
    let (e1,_)=h.chain.validate(&b1,None,0).unwrap();h.chain.insert(e1);
    let p=h.chain.get(&b1.hash()).unwrap().clone();let ctx=h.chain.child_ctx(&p,0).unwrap();
    let (shell,_,_)=h.chain.build_signed(&p,&h.keys[&ctx.proposer],ctx.window.lower(),Body::default(),vec![]).unwrap();
    let template=Msg::Template{shell:shell.encoded.clone(),body:Body::default().encode()};
    deliver(&mut n,template.clone());assert!(!n.template_json().is_null(),"the template locks");assert_eq!(credits(&n),[1,1,0,0],"a template that locks credits its sender");
    deliver(&mut n,template);assert_eq!(credits(&n),[1,1,0,0],"a copy of a locked template earns nothing");
    // A valid new share for the locked template, then the same share.
    let tid=template_id(&shell.ut_raw);
    let share=(0u64..).take_while(|x|u128::from(*x)<ctx.assignment.nonce_max()).find(|x|le_target(&pow_hash(&tid,*x),&ctx.t_share)).expect("a share below nonce_max");
    deliver(&mut n,Msg::Share{tid,nonce:share});assert_eq!(credits(&n),[1,1,0,1],"a valid new share credits its sender");
    deliver(&mut n,Msg::Share{tid,nonce:share});assert_eq!(credits(&n),[1,1,0,1],"a duplicate share earns nothing");
    // A transaction that enters the pool, then the same transaction.
    n.pool_settle();let tx=pay(&devnet::account_key("alice"),0);
    deliver(&mut n,Msg::Tx{raw:tx.clone()});assert_eq!(n.status_json()["pool"],1,"pooled");assert_eq!(credits(&n),[1,1,1,1],"a pooled transaction credits its sender");
    deliver(&mut n,Msg::Tx{raw:tx});assert_eq!((n.status_json()["pool"].as_u64(),credits(&n)),(Some(1),[1,1,1,1]),"a transaction already pooled earns nothing");
    let _=std::fs::remove_dir_all(&dir);
}
