//! V3-26 / R21 (local retention, never validity): a node answers a state query at every canonical height inside the
//! K_hist trie window, (head − K_hist, head], and at `safe` = max(head − k_safe, finalized_h) while that is inside it
//! (node.rs `Limits::k_safe`). Measured on the 3-node devnet (memory mode, a partition then a heal): a block adopted
//! after its state had been pruned as a side block (the other side's branch at the heal, or a node's own block that
//! lost a race and won again) answered −32017 "state outside the retained trie window" at heights well inside the
//! window, so `safe` did too. The node under test receives the blocks of three builders in the order a partition and
//! a heal deliver them: its own branch, the heavier other side, its own side overtaking again, the other side again,
//! then a linear stretch that slides the window past every fork. After every import each height of the window (and
//! `safe`) must answer eth_getCode and eth_getBalance. Memory mode and store mode (`--state-store`), live and after a
//! reopen (journal replay, or a checkpoint resume).
#[path="support/members.rs"] mod members_sweep;
use std::collections::HashMap;
use pocold::{chain::{Chain,Spec},types::*,hashes};
use pocol_lp4_execution::{devnet::{self,DevnetConfig},txsign::Key};
use revm::primitives::{Address,B256,U256};
use serde_json::json;

struct Net{chain:Chain,keys:HashMap<Address,Key>}
impl Drop for Net{fn drop(&mut self){crate::members_sweep::sweep_on_drop(&self.chain);}}
fn cfg()->DevnetConfig{let mut c=DevnetConfig::new(777910,1_700_000_000,&["a","b","c"]);c.hash_rate=1024;c.d_att=73;c.tau=40;c.c=8;c}
fn admitted()->Spec{members_sweep::oracle();let d=devnet::build(&cfg()).unwrap();Spec::new(d.admit(&pocold::single_host_local()).unwrap_or_else(|e|panic!("{e}")).0)}
fn net()->Net{let d=devnet::build(&cfg()).unwrap();let keys=d.miners.iter().map(|m|(m.id,Key::from_secret(&m.key.secret()).unwrap())).collect();Net{chain:Chain::new(admitted()),keys}}
/// A fallback block on `parent` (attempt `first`, or the next attempt whose nonce space holds a solution).
fn fallback(n:&Net,parent:[u8;32],first:u32)->Block{
    let p=n.chain.full_entry(&parent).unwrap();
    for a in first..first+16{let (mut h,_,_)=n.chain.build_fallback(&p,a).unwrap();let ctx=n.chain.child_ctx(&p,a).unwrap();let tid=template_id(&h.ut_raw);let mut shares=vec![];let mut x=0u64;
        let found=loop{if (x as u128)>=ctx.assignment.nonce_max(){break false}let ph=pow_hash(&tid,x);if le_target(&ph,&ctx.t_share){if le_target(&ph,&ctx.target){break true}shares.push(x);}x+=1;};
        if !found{continue}
        let owner=addr(&ctx.assignment.owner(x).unwrap());h.nonce=x;shares.truncate(256);h.shares=shares;reseal(&mut h);
        h.winner_sig=n.keys[&owner].sign_hash(&B256::from(hashes::win_msg(&tid,x,&share_root(&h))));reseal(&mut h);
        return Block{header:h,body:Body::default()}}
    panic!("no fallback solution")
}
fn import(n:&mut Net,b:&Block){let (e,_)=n.chain.validate(b,None,0).unwrap_or_else(|e|panic!("h={} {e:?}",b.header.h));n.chain.insert(e);n.chain.retain();}
/// `count` blocks on the builder's head (fallbacks at attempt 0).
fn grow(n:&mut Net,count:usize)->Vec<Block>{(0..count).map(|_|{let b=fallback(n,n.chain.head,0);import(n,&b);b}).collect()}
/// Blocks on the builder's head until its work exceeds `than`, then `extra` more.
fn past(n:&mut Net,than:U256,extra:usize)->Vec<Block>{let mut v=vec![];while n.chain.head().total_work<=than{v.extend(grow(n,1));}v.extend(grow(n,extra));v}

fn node(tag:&str,store:bool)->(pocold::node::Node,std::path::PathBuf,impl Fn()->pocold::node::Config){
    use pocold::node::{Config,Limits};
    let dir=std::env::temp_dir().join(format!("pocold-memret-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let (d,name)=(dir.clone(),tag.to_string());
    let conf=move||Config{dir:d.clone(),name:name.clone(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,
        limits:Limits{state_store:store,checkpoint_every:if store{8}else{0},..Default::default()}};
    (pocold::node::Node::open(conf(),admitted()).unwrap(),dir,conf)
}
/// One request answered in process (rpc::answer), under compute deadlines as long as its work deadline: the subject
/// is which heights answer, never how fast (a loaded machine must not turn an answer into −32020).
fn rpc(n:&mut pocold::node::Node,method:&str,params:serde_json::Value)->serde_json::Value{
    let long=std::time::Duration::from_secs(120);let kept=(n.cfg.limits.rpc.compute_light,n.cfg.limits.rpc.compute_heavy);
    (n.cfg.limits.rpc.compute_light,n.cfg.limits.rpc.compute_heavy)=(long,long);
    let body=json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string();let (t,_r)=std::sync::mpsc::channel();
    let a=pocold::rpc::answer(n,&pocold::rpc::Request::local(body.clone().into_bytes(),t,std::time::Instant::now()+long,false));
    (n.cfg.limits.rpc.compute_light,n.cfg.limits.rpc.compute_heavy)=kept;
    let v:serde_json::Value=serde_json::from_str(&a.body).unwrap();
    assert!(v["error"]["code"]!=-32020&&v["error"]["code"]!=-32021,"{body}: no answer to judge: {v}");v
}
/// Every canonical height of the window and `safe`, by number and by tag, that does not answer a state query. The
/// window is (max(head, top) − K_hist, head], `top` the highest head the node has had: its floor never moves down
/// (node.rs `Limits::k_safe`), so after a heavier, shorter branch lowered the head the heights the old head had pruned
/// are not asked for. `safe` is always asked for, and must be inside the window here (every fall-back of the schedule
/// is shallower than K_hist − k_safe).
fn unanswered(n:&mut pocold::node::Node,top:u64)->Vec<String>{
    let head=n.chain.head().height;let k=n.chain.k_hist;let top=top.max(head);let lo=if top>=k{top-k+1}else{0};
    let who=format!("{:#x}",devnet::account_key("alice").address());let mut bad=vec![];
    let safe=n.safe_height();assert!(safe>=lo,"safe {safe} is inside the window [{lo}, {head}] (k_safe {}, K_hist {k})",n.cfg.limits.k_safe);
    let tags:Vec<String>=(lo..=head).map(|h|format!("{h:#x}")).chain(["safe".to_string(),"latest".to_string()]).collect();
    for t in &tags{let mut codes=vec![];for m in ["eth_getCode","eth_getBalance"]{let v=rpc(n,m,json!([who,t]));
            if v.get("result").map_or(true,|r|!r.is_string()){codes.push(format!("{m} {}",v["error"]["code"]))}}
        if !codes.is_empty(){bad.push(format!("{t} ({})",codes.join(", ")))}}
    bad
}
/// The blocks of a partition and a heal, in the order the node of the minority side receives them, and the
/// positive controls on the shape: three reorganizations, the deepest more than side_max blocks deep.
fn schedule()->Vec<Block>{
    let mut x=net();let prefix=grow(&mut x,20);
    let mut y=net();for b in &prefix{import(&mut y,b);}
    // The node's own side first (it is canonical there), then the other side, heavier; then its own side overtakes
    // again (its blocks were side blocks meanwhile), then the other side again; then one branch, linearly, long
    // enough to slide the window past every fork.
    // Its first block is a later attempt, so the two sides differ from the fork on (attempt 0 is deterministic).
    let own:Vec<Block>={let b=fallback(&y,y.chain.head,1);import(&mut y,&b);std::iter::once(b).chain(grow(&mut y,9)).collect()};
    let other=past(&mut x,y.chain.head().total_work,2);
    let own2=past(&mut y,x.chain.head().total_work,1);
    let other2=past(&mut x,y.chain.head().total_work,3);
    let tail=grow(&mut x,40);
    assert!(other.len()>4&&own2.len()>2&&other2.len()>2,"positive control: every reorganization is deeper than side_max ({}, {}, {})",other.len(),own2.len(),other2.len());
    prefix.into_iter().chain(own).chain(other).chain(own2).chain(other2).chain(tail).collect()
}
fn window_after_every_import(store:bool,tag:&str){
    let blocks=schedule();let (mut n,dir,conf)=node(tag,store);let mut failures=vec![];let (mut top,mut fell)=(0u64,0u64);
    for b in &blocks{n.import(b.clone(),None).unwrap_or_else(|e|panic!("h={}: {e:?}",b.header.h));
        let head=n.chain.head().height;top=top.max(head);fell=fell.max(top-head);
        let bad=unanswered(&mut n,top);if !bad.is_empty(){failures.push(format!("head {head}: {}",bad.join(", ")));}}
    assert!(n.stats.reorgs>=3,"positive control: {} reorganizations",n.stats.reorgs);assert_eq!(n.chain.head,blocks.last().unwrap().hash());
    assert!(fell>0,"positive control: a heavier, shorter branch lowered the head");
    // Positive control: below the window a state query still answers −32017 (the window is a bound, not a promise of more).
    let who=format!("{:#x}",devnet::account_key("alice").address());
    assert_eq!(rpc(&mut n,"eth_getBalance",json!([who,"0x1"]))["error"]["code"],json!(-32017),"height 1 is below the window");
    assert!(failures.is_empty(),"FAIL-ON-ORIGINAL ({tag}): canonical heights inside the K_hist window answered no state:\n{}",failures.join("\n"));
    // Reopened (journal replay, or a checkpoint and the records after it): the same window answers.
    drop(n);let mut r=pocold::node::Node::open(conf(),admitted()).unwrap();
    let bad=unanswered(&mut r,top);assert!(bad.is_empty(),"after reopen ({tag}): {}",bad.join(", "));
    crate::members_sweep::sweep_node(&r);drop(r);let _=std::fs::remove_dir_all(&dir);
}
#[test] fn memory_mode_answers_every_height_of_the_window_through_partition_and_heal(){window_after_every_import(false,"mem")}
#[test] fn store_mode_answers_every_height_of_the_window_through_partition_and_heal(){window_after_every_import(true,"store")}
