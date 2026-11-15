//! V3-26 / R21 (local retention, never validity): a node answers a state query at every canonical height inside the
//! K_hist trie window, (head − K_hist, head], and at `safe` = max(head − k_safe, finalized_h) while that is inside it
//! (node.rs `Limits::k_safe`), with the state of that canonical block. Measured on the 3-node devnet (memory mode, a
//! partition then a heal): a block adopted after its state had been pruned as a side block (the other side's branch at
//! the heal, or a node's own block that lost a race and won again) answered −32017 "state outside the retained trie
//! window" at heights well inside the window, so `safe` did too. The node under test receives the blocks of three
//! builders in the order a partition and a heal deliver them: its own branch, the heavier other side, its own side
//! overtaking again, the other side again, then a linear stretch that slides the window past every fork. After every
//! import each height of the window (and `safe`, `latest`) must answer eth_getCode and eth_getBalance, and the state it
//! answers with must be the canonical block's: each miner's reward balance equals the one in the state of the builder
//! that made that block (the branches pay different miners), and in memory mode the state's root is the block's
//! state_root. Memory mode and store mode (`--state-store`): live through the whole schedule, then once reopened; and
//! reopened after every import (journal replay, or a checkpoint and the records after it), importing on into the
//! reopened node, so a restart happens while the forks are inside the window. After a restart the window is the whole
//! (head − K_hist, head] again.
#[path="support/members.rs"] mod members_sweep;
use std::collections::HashMap;
use pocold::{chain::{Chain,Spec},types::*,hashes};
use pocol_lp4_execution::{apply,devnet::{self,DevnetConfig},txsign::Key};
use revm::primitives::{Address,B256,U256};
use serde_json::json;

/// A builder: its chain, the miners' signing keys, its miners' reward addresses, and each reward address's balance in
/// the state of every block it imported (the reference the node under test is compared with).
struct Net{chain:Chain,keys:HashMap<Address,Key>,rewards:Vec<Address>,seen:HashMap<[u8;32],Vec<U256>>}
impl Drop for Net{fn drop(&mut self){crate::members_sweep::sweep_on_drop(&self.chain);}}
fn cfg()->DevnetConfig{let mut c=DevnetConfig::new(777910,1_700_000_000,&["a","b","c"]);c.hash_rate=1024;c.d_att=73;c.tau=40;c.c=8;c}
fn admitted()->Spec{members_sweep::oracle();let d=devnet::build(&cfg()).unwrap();Spec::new(d.admit(&pocold::single_host_local()).unwrap_or_else(|e|panic!("{e}")).0)}
fn net()->Net{let d=devnet::build(&cfg()).unwrap();let keys=d.miners.iter().map(|m|(m.id,Key::from_secret(&m.key.secret()).unwrap())).collect();
    let rewards:Vec<Address>=d.miners.iter().map(|m|m.reward).collect();let chain=Chain::new(admitted());
    let genesis:Vec<U256>={let s=chain.state_of(&chain.head).unwrap();rewards.iter().map(|a|balance(&s,a)).collect()};
    let seen=HashMap::from([(chain.head,genesis)]);Net{chain,keys,rewards,seen}}
fn balance(s:&revm::InMemoryDB,a:&Address)->U256{s.accounts.get(a).map_or(U256::ZERO,|x|x.info.balance)}
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
fn import(n:&mut Net,b:&Block){let (e,_)=n.chain.validate(b,None,0).unwrap_or_else(|e|panic!("h={} {e:?}",b.header.h));n.chain.insert(e);n.chain.retain();
    let v:Vec<U256>={let s=n.chain.state_of(&b.hash()).expect("the builder's new head holds its state");n.rewards.iter().map(|a|balance(&s,a)).collect()};
    n.seen.insert(b.hash(),v);}
/// `count` blocks on the builder's head (fallbacks at attempt 0).
fn grow(n:&mut Net,count:usize)->Vec<Block>{(0..count).map(|_|{let b=fallback(n,n.chain.head,0);import(n,&b);b}).collect()}
/// Blocks on the builder's head until its work exceeds `than`, then `extra` more.
fn past(n:&mut Net,than:U256,extra:usize)->Vec<Block>{let mut v=vec![];while n.chain.head().total_work<=than{v.extend(grow(n,1));}v.extend(grow(n,extra));v}

fn node(tag:&str,store:bool,checkpoint_every:u64)->(pocold::node::Node,std::path::PathBuf,impl Fn()->pocold::node::Config){
    use pocold::node::{Config,Limits};
    let dir=std::env::temp_dir().join(format!("pocold-memret-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let (d,name)=(dir.clone(),tag.to_string());
    let conf=move||Config{dir:d.clone(),name:name.clone(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,
        limits:Limits{state_store:store,checkpoint_every,..Default::default()}};
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
/// The partition and heal schedule: the blocks in the order the node of the minority side receives them, the miners'
/// reward addresses, and each block's reward balances in its builder's state.
struct Schedule{blocks:Vec<Block>,rewards:Vec<Address>,expect:HashMap<[u8;32],Vec<U256>>}
/// Every canonical height of the window and `safe`, by number and by tag, that does not answer a state query, or
/// answers with a state that is not that canonical block's. The window is (max(head, top) − K_hist, head], `top` the
/// highest head the node has had since it opened: its floor never moves down while it runs (node.rs
/// `Limits::k_safe`), so after a heavier, shorter branch lowered the head the heights the old head had pruned are not
/// asked for (`top` = head after a restart asks for all of them). `safe` is always asked for, and must be inside the
/// window here (every fall-back of the schedule is shallower than K_hist − k_safe).
fn unanswered(n:&mut pocold::node::Node,top:u64,s:&Schedule)->Vec<String>{
    let head=n.chain.head().height;let k=n.chain.k_hist;let top=top.max(head);let lo=if top>=k{top-k+1}else{0};
    let who=format!("{:#x}",devnet::account_key("alice").address());let mut bad=vec![];
    let safe=n.safe_height();assert!(safe>=lo,"safe {safe} is inside the window [{lo}, {head}] (k_safe {}, K_hist {k})",n.cfg.limits.k_safe);
    let tags:Vec<(String,u64)>=(lo..=head).map(|h|(format!("{h:#x}"),h)).chain([("safe".to_string(),safe),("latest".to_string(),head)]).collect();
    for (t,h) in &tags{let mut codes=vec![];
        let hash=n.chain.canonical_at(*h).unwrap_or_else(|| panic!("canonical height {h} is resident")).hash;
        let want=s.expect.get(&hash).unwrap_or_else(||panic!("height {h}: a block of the schedule"));
        let v=rpc(n,"eth_getCode",json!([who,t]));if v.get("result").map_or(true,|r|!r.is_string()){codes.push(format!("eth_getCode {}",v["error"]["code"]))}
        for (i,a) in s.rewards.iter().enumerate(){let v=rpc(n,"eth_getBalance",json!([format!("{a:#x}"),t]));
            match v.get("result").and_then(|r|r.as_str()){
                None=>codes.push(format!("eth_getBalance {}",v["error"]["code"])),
                Some(r)=>{let got=U256::from_str_radix(r.trim_start_matches("0x"),16).unwrap();if got!=want[i]{codes.push(format!("reward {i} balance {got}, the block's state has {}",want[i]))}}}}
        // Memory mode: the state itself, by its root (store mode root-checks every version it makes, `reexec`; the
        // genesis header commits no root, its state is the admitted genesis image).
        if !n.cfg.limits.state_store&&*h>0{if let Some(st)=n.chain.state_of(&hash){
            let root=apply::state_root(&st).unwrap().0;let e=n.chain.canonical_at(*h).unwrap();
            if root!=e.header.state_root{codes.push(format!("state root {} is not the block's {}",B256::from(root),B256::from(e.header.state_root)))}}}
        if !codes.is_empty(){bad.push(format!("{t} ({})",codes.join(", ")))}}
    bad
}
/// The blocks of a partition and a heal, in the order the node of the minority side receives them, and the
/// positive controls on the shape: three reorganizations, the deepest more than side_max blocks deep, and branches
/// whose states differ at the same height.
fn schedule()->Schedule{
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
    let at=|v:&[Block],h:u64|v.iter().find(|b|b.header.h==h).map(|b|b.hash());
    let differ=own.iter().filter(|b|at(&other,b.header.h).is_some_and(|o|x.seen[&o]!=y.seen[&b.hash()])).count();
    assert!(differ>0,"positive control: the two sides' reward balances differ at some height of the fork");
    let rewards=x.rewards.clone();let mut expect=std::mem::take(&mut x.seen);expect.extend(std::mem::take(&mut y.seen));
    Schedule{blocks:prefix.into_iter().chain(own).chain(other).chain(own2).chain(other2).chain(tail).collect(),rewards,expect}
}
/// `reopen`: drop and reopen the node after every import, and check the window there with top = head.
fn window_after_every_import(store:bool,checkpoint_every:u64,reopen:bool,tag:&str){
    let s=schedule();let (mut n,dir,conf)=node(tag,store,checkpoint_every);let (mut live,mut reopened)=(vec![],vec![]);
    let (mut top,mut fell,mut reorgs)=(0u64,0u64,0u64);
    for (i,b) in s.blocks.iter().enumerate(){n.import(b.clone(),None).unwrap_or_else(|e|panic!("h={}: {e:?}",b.header.h));
        let head=n.chain.head().height;top=top.max(head);fell=fell.max(top-head);
        let bad=unanswered(&mut n,top,&s);if !bad.is_empty(){live.push(format!("i{i} head {head}: {}",bad.join(", ")));}
        if reopen{let was=n.chain.head;reorgs+=n.stats.reorgs;drop(n);n=pocold::node::Node::open(conf(),admitted()).unwrap();
            assert_eq!(n.chain.head,was,"a reopen keeps the head");top=head;
            let bad=unanswered(&mut n,head,&s);if !bad.is_empty(){reopened.push(format!("i{i} head {head}: {}",bad.join(", ")));}}}
    reorgs+=n.stats.reorgs;
    assert!(reorgs>=3,"positive control: {reorgs} reorganizations");assert_eq!(n.chain.head,s.blocks.last().unwrap().hash());
    assert!(fell>0,"positive control: a heavier, shorter branch lowered the head");
    // Positive control: below the window a state query still answers −32017 (the window is a bound, not a promise of more).
    let who=format!("{:#x}",devnet::account_key("alice").address());
    assert_eq!(rpc(&mut n,"eth_getBalance",json!([who,"0x1"]))["error"]["code"],json!(-32017),"height 1 is below the window");
    assert!(live.is_empty()&&reopened.is_empty(),"FAIL-ON-ORIGINAL ({tag}): canonical heights inside the K_hist window answered no state, or another state\nlive ({}):\n{}\nafter a reopen ({}):\n{}",
        live.len(),live.join("\n"),reopened.len(),reopened.join("\n"));
    // Reopened (journal replay, or a checkpoint and the records after it): the whole window answers.
    drop(n);let mut r=pocold::node::Node::open(conf(),admitted()).unwrap();let head=r.chain.head().height;
    let bad=unanswered(&mut r,head,&s);assert!(bad.is_empty(),"after the last reopen ({tag}): {}",bad.join(", "));
    crate::members_sweep::sweep_node(&r);drop(r);let _=std::fs::remove_dir_all(&dir);
}
#[test] fn memory_mode_answers_every_height_of_the_window_through_partition_and_heal(){window_after_every_import(false,0,false,"mem")}
#[test] fn store_mode_answers_every_height_of_the_window_through_partition_and_heal(){window_after_every_import(true,8,false,"store")}
#[test] fn memory_mode_answers_every_height_of_the_window_reopened_after_every_import(){window_after_every_import(false,0,true,"mem-reopen")}
#[test] fn store_mode_answers_every_height_of_the_window_reopened_after_every_import_journal(){window_after_every_import(true,0,true,"store-reopen-0")}
#[test] fn store_mode_answers_every_height_of_the_window_reopened_after_every_import_checkpoints(){window_after_every_import(true,8,true,"store-reopen-8")}
