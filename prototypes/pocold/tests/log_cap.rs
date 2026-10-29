//! LOG_CAP exhaustion with the caps-test build constants (validation.md:2065: LOG_CAP=2^24, simnet only,
//! absent from the release; T18b L6 and its criterion :2087–2088). Run with `--features caps-test`.
//! execution.md:82: reaching LOG_CAP is an exceptional halt of the transaction (all gas charged, state
//! reverted except the nonce and the fee, status 0, no logs; the block stays valid); LOG-CAP-AGGREGATE's
//! provisional "chain" reading: the charged log bytes of the branch up to the parent, plus this block's.
#![cfg(feature="caps-test")]
use std::collections::HashMap;
use pocold::{chain::Chain,node::{Config,Node},types::*,hashes,rlp};
use pocol_lp4_execution::{apply,devnet::{self,DevnetConfig},txsign::{Key,Tx},user_phase::caps};
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
/// Signed by Draw(h,0), timestamped T_blk after the parent where possible (ASERT stays near target_g).
fn signed(n:&Net,parent:[u8;32],txs:Vec<Vec<u8>>)->Block{
    let p=n.chain.full_entry(&parent).unwrap();let ctx=n.chain.child_ctx(&p,0).unwrap();let body=Body{txs,evidence:vec![]};
    for off in 3..u64::from(n.chain.spec.cp().D_att){let (h,_,_)=n.chain.build_signed(&p,&n.keys[&ctx.proposer],ctx.window.lower()+off,body.clone(),vec![]).unwrap();
        if let Some(b)=try_seal(n,h,body.clone(),0){return b}}panic!("no signed solution")
}
fn fallback(n:&Net,parent:[u8;32])->Block{let p=n.chain.full_entry(&parent).unwrap();
    for a in 0..16{let (h,_,_)=n.chain.build_fallback(&p,a).unwrap();if let Some(b)=try_seal(n,h,Body::default(),a){return b}}panic!("no fallback solution")}
fn import(n:&mut Net,b:&Block)->pocold::chain::Entry{let (e,_)=n.chain.validate(b,None,0).unwrap_or_else(|e|panic!("h={} {e:?}",b.header.h));assert!(n.chain.insert(e.clone()),"new head");e}
fn tx(key:&Key,nonce:u64,to:Option<Address>,data:Vec<u8>,gas:u64,value:u64)->Vec<u8>{
    Tx{chain_id:CHAIN,nonce,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas,to,value:U256::from(value),data:Bytes::from(data)}.sign_1559(key)}
/// (status, cumulative gas, logs) of each typed receipt.
fn receipts(e:&pocold::chain::Entry)->Vec<(bool,u64,usize)>{e.receipts.iter().map(|r|{let it=rlp::decode(&r[1..]).unwrap();let l=it.list().unwrap();
    let g=l[1].bytes().unwrap().iter().fold(0u64,|n,b|n<<8|u64::from(*b));(l[0].bytes().unwrap()==[1],g,l[3].list().unwrap().len())}).collect()}
fn word(v:u64)->[u8;32]{U256::from(v).to_be_bytes()}
/// LOG0 x calldata[0..32] times, each with calldata[32..64] zero bytes (lp4 apply tests' LOGN).
const LOGN:&[u8]=&[0x5f,0x35,0x5b,0x80,0x15,0x61,0x00,0x15,0x57,0x60,0x20,0x35,0x5f,0xa0,0x60,0x01,0x90,0x03,0x60,0x02,0x56,0x5b,0x00];
/// Initcode returning `code` (at most 32 bytes): PUSHn code, MSTORE at 0, RETURN its last n bytes.
fn deploy(code:&[u8])->Vec<u8>{let n=code.len();let mut v=vec![0x5f+n as u8];v.extend_from_slice(code);v.extend([0x5f,0x52,0x60,n as u8,0x60,(32-n) as u8,0xf3]);v}

#[test] fn the_caps_test_build_sets_log_cap_to_2_pow_24(){assert_eq!(caps::LOG_CAP,1<<24)}

#[test] fn log_cap_exhaustion_halts_logging_only_and_the_chain_keeps_producing(){
    let c=cfg();let d=devnet::build(&c).unwrap();let mut n=Net{chain:Chain::new(spec(&c)),keys:d.miners.iter().map(|m|(m.id,Key::from_secret(&m.key.secret()).unwrap())).collect()};
    let (alice,bob)=(devnet::account_key("alice"),devnet::account_key("bob"));let mut blocks=vec![];
    // h=1: deploy LOGN. Then one LOGN(1, 32732) per block: 36+32732 = 32768 charged bytes, so 512 blocks reach 2^24.
    let logn=alice.address().create(0);let b=signed(&n,n.chain.head,vec![tx(&alice,0,None,deploy(LOGN),200_000,0)]);let mut e=import(&mut n,&b);blocks.push(b);
    assert!(receipts(&e)[0].0,"LOGN deployed");let fill=|nonce:u64|tx(&alice,nonce,Some(logn),[word(1),word(32732)].concat(),400_000,0);
    let per_block=(caps::LOG_CAP/32768) as u64;assert_eq!(per_block,512);
    for i in 0..per_block{
        let b=signed(&n,n.chain.head,vec![fill(1+i)]);e=import(&mut n,&b);blocks.push(b);
        assert_eq!(receipts(&e)[0],(true,receipts(&e)[0].1,1),"fill {i} succeeds");assert_eq!(e.cum_log_bytes,(i+1)*32768);
        if i%64==63{n.chain.retain();}
    }
    assert_eq!(e.cum_log_bytes,caps::LOG_CAP,"the branch's charged log bytes equal LOG_CAP at h={}",e.height);
    // The exhausted block: a logging call, a plain transfer, a CREATE without logs, a CREATE whose
    // constructor logs, and a call to a contract that emits none.
    let p=n.chain.head().clone();let exhausted_at=p.height;let (an,bn)=(1+per_block,0u64);let carol=devnet::account_key("carol").address();
    let quiet_init=deploy(&[0x00]);let noisy_init=vec![0x5f,0x5f,0xa0,0x60,0x01,0x5f,0xf3];// LOG0(0,0), then RETURN(0,1) = code 0x00
    let (quiet,noisy)=(bob.address().create(bn+1),bob.address().create(bn+2));
    let txs=vec![tx(&alice,an,Some(logn),[word(1),word(0)].concat(),300_000,0),tx(&bob,bn,Some(carol),vec![],21_000,7),
        tx(&bob,bn+1,None,quiet_init,200_000,0),tx(&bob,bn+2,None,noisy_init,200_000,0),tx(&alice,an+1,Some(quiet),vec![],50_000,0)];
    let b=signed(&n,p.hash,txs);let e=import(&mut n,&b);blocks.push(b.clone());let r=receipts(&e);
    println!("LOGCAP {}",serde_json::json!({"exhaustedAt":p.height,"cumLogBytes":p.cum_log_bytes,"logCap":caps::LOG_CAP,"receipts":r.iter().map(|x|serde_json::json!({"status":x.0,"cumGas":x.1,"logs":x.2})).collect::<Vec<_>>()}));
    assert_eq!(r.iter().map(|x|(x.0,x.2)).collect::<Vec<_>>(),vec![(false,0),(true,0),(true,0),(false,0),(true,0)]);
    assert_eq!(r[0].1,300_000,"the halted logging call is charged its whole gas limit");assert_eq!(r[3].1-r[2].1,200_000,"so is the halted CREATE");
    assert_eq!(e.cum_log_bytes,caps::LOG_CAP,"halts charge no log bytes");assert_eq!(e.header.logs_bloom,vec![0u8;256]);
    let (s0,s1)=(&p.state,&e.state);let price=from_fixed(&b.header.base_fee)+U256::from(1_000_000_000u64);
    let (a0,a1)=(&s0.accounts[&alice.address()].info,&s1.accounts[&alice.address()].info);
    assert_eq!(a1.nonce,a0.nonce+2,"the halt keeps the nonce increment");assert_eq!(a0.balance-a1.balance,price*U256::from(300_000u64+21_000),"the halt keeps the fee (gas limit x price); the quiet call costs 21000");
    assert_eq!(s1.accounts[&carol].info.balance,U256::from(7u64),"a plain transfer succeeds");
    assert!(s1.accounts.get(&quiet).and_then(|a|a.info()).map_or(false,|i|i.code_hash!=revm::primitives::KECCAK_EMPTY),"a CREATE without logs deploys");
    assert!(s1.accounts.get(&noisy).and_then(|a|a.info()).is_none(),"a CREATE whose constructor logs halts: nothing is created");
    assert_eq!(s1.accounts[&bob.address()].info.nonce,3,"the halted CREATE still consumes bob's nonce (its address is never used)");
    // T18b: at least 20 more blocks pass T17 (apply checks the Escrow invariant each block); system phases and
    // settlement continue: the miners' reward balances grow. Every logging call keeps halting.
    let rewards=|s:&revm::InMemoryDB|->U256{d.miners.iter().map(|m|s.accounts.get(&m.reward).map(|a|a.info.balance).unwrap_or_default()).fold(U256::ZERO,|x,y|x+y)};
    let before=rewards(&e.state);let mut nonce=an+2;
    for i in 0..20{
        let b=if i%2==0{let b=signed(&n,n.chain.head,vec![fill(nonce)]);nonce+=1;b}else{fallback(&n,n.chain.head)};let x=import(&mut n,&b);blocks.push(b.clone());
        if i%2==0{assert_eq!(receipts(&x)[0],(false,400_000,0),"logging halts after exhaustion");assert_eq!(x.cum_log_bytes,caps::LOG_CAP);}
        apply::check_escrow_invariant(&x.state,n.chain.spec.profile(),x.height,x.is_final).expect("T17");
    }
    let head=n.chain.head().clone();assert!(rewards(&head.state)>before,"settlement keeps paying the miners");
    // Block production through a node: the whole chain replays into a node, and its builder offers a
    // logging transaction, which the block carries as a halt (a valid block).
    let dir=std::env::temp_dir().join(format!("pocold-logcap-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let mut node=Node::open(Config{dir:dir.clone(),name:"logcap".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Default::default()},spec(&c)).unwrap();
    for b in &blocks{node.import(b.clone(),None).unwrap();}assert_eq!(node.chain.head,head.hash);
    node.submit_tx(fill(nonce),false).unwrap();let p=node.chain.head().meta_clone();let txs=node.template_txs(&p);assert_eq!(txs.len(),1,"the builder includes it");
    let b=signed(&n,head.hash,txs);assert!(node.import(b,None).unwrap());let x=node.chain.head();
    assert_eq!((receipts(x)[0].0,x.cum_log_bytes),(false,caps::LOG_CAP));assert!(node.pool.txs.is_empty());
    println!("LOGCAP {}",serde_json::json!({"head":x.height,"exhaustedAt":exhausted_at,"blocksAfterExhaustion":x.height-exhausted_at,"rewardsBefore":before.to_string(),"rewardsAfter":rewards(&head.state).to_string()}));
    let _=std::fs::remove_dir_all(&dir);
}
