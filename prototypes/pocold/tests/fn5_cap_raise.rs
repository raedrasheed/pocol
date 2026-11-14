//! FN-5 (C10, D-V3-23 restricted to the cap fields; rehearsal step S2 in miniature): a cap raise scheduled at fork 3
//! beside the fork schedule (`devnet.json` `capRaise`). On private fixture chains: a filled ACCT_CAP stops funding a
//! newcomer (so it cannot `register`) and new accounts until the raise height, from which both succeed; SLOT_CAP and
//! CODE_CAP likewise; replay and a reorg across the height give the same states; a node without the raise agrees on
//! every block below the height and partitions only from it; the pool, eth_estimateGas and pocol_getCaps follow the
//! caps of the next block; the consensus identity commits to the raise; the configuration fails closed. Dormant: with
//! no raise nothing here differs from the chain before FN-5 (the existing suites run unchanged).
#[path="support/members.rs"] mod members_sweep;
use std::sync::OnceLock;
use pocol_lp4_execution::{apply,devnet::{self,DevnetConfig},profile::CapRaise,state_summary::Summary,txsign::{Key,Tx},view::View};
use pocold::{chain::{Chain,Reject,Spec},identity,types::*};
use revm::primitives::{keccak256,Address,Bytes,B256,U256};
use serde_json::json;

const CHAIN:u64=777962;
/// Fork 2 (a nonce-rule no-op under nonceMode 1) and fork 3, the cap raise.
const V2:u64=3;const H:u64=6;
/// The account headroom the admission's timing workload needs (its sender and its spin contract) and its code (4 bytes).
const ROOM:u64=2;const SPIN:u64=4;

fn base()->DevnetConfig{
    let mut c=DevnetConfig::new(CHAIN,1_700_000_000,&["execution"]);c.spec_version=2;c.earnin=true;c.nonce_mode=1;c.hash_rate=16;c.c=8;c.d_att=91;c.r_max=1;c
}
fn genesis_counts()->Summary{static S:OnceLock<Summary>=OnceLock::new();*S.get_or_init(||{let d=devnet::build(&base()).unwrap();Summary::scan(d.genesis.state(),32768)})}
/// ACCT_CAP with only the timing workload's headroom above the genesis accounts.
fn acct_cfg()->DevnetConfig{let mut c=base();c.acct_cap=genesis_counts().user_accounts+ROOM;c}
/// SLOT_CAP at the genesis slots and CODE_CAP with only the timing workload's code above the genesis code.
fn store_cfg()->DevnetConfig{let mut c=base();c.slot_cap=genesis_counts().user_slots;c.code_cap=genesis_counts().code_total as u64+SPIN;c}
fn same_caps(c:&DevnetConfig)->CapRaise{CapRaise{code_cap:c.code_cap,slot_cap:c.slot_cap,acct_cap:c.acct_cap,sys_slot_max:c.sys_slot_max}}
fn acct_raise()->CapRaise{let c=acct_cfg();CapRaise{acct_cap:c.acct_cap+8,..same_caps(&c)}}
fn store_raise()->CapRaise{let c=store_cfg();CapRaise{slot_cap:c.slot_cap+8,code_cap:c.code_cap+64,..same_caps(&c)}}
/// The fixture admitted with [(1,0),(2,V2)] and, with a raise, (3,H).
fn spec_of(c:&DevnetConfig,raise:Option<CapRaise>)->Spec{
    let mut d=devnet::build(c).unwrap();d.forks=vec![(1,0),(2,V2)];if raise.is_some(){d.forks.push((3,H));}d.cap_raise=raise;
    crate::members_sweep::oracle();Spec::new(d.admit(&pocold::single_host_local()).unwrap_or_else(|e|panic!("{e}")).0)
}
struct Net{chain:Chain}
impl Drop for Net{fn drop(&mut self){crate::members_sweep::sweep_on_drop(&self.chain);}}
fn net(c:&DevnetConfig,raise:Option<CapRaise>)->Net{Net{chain:Chain::new(spec_of(c,raise))}}

/// A signed block on `parent` by the fixture miner (the only drawable member: a newcomer registered with no bond is
/// ACCRUING below B_reg/4), its solution searched in the miner's own range; `late` shifts the timestamp.
fn solve(chain:&Chain,parent:&[u8;32],txs:Vec<Vec<u8>>,late:u64)->Result<Block,Reject>{
    let p=chain.full_entry(parent).unwrap();let key=devnet::miner_identity("execution").key;let body=Body{txs,evidence:vec![]};
    let (mut h,_,ctx)=chain.build_signed(&p,&key,p.ts+u64::from(chain.spec.cp().T_blk)+late,body.clone(),vec![])?;let tid=template_id(&h.ut_raw);
    let (lo,hi)=pocold::chain::assignment_range(&ctx,key.address()).expect("the fixture miner holds a range");
    for nonce in lo..=hi.min(lo.saturating_add(1_000_000)){if le_target(&pow_hash(&tid,nonce),&ctx.target){h.nonce=nonce;reseal(&mut h);
        h.winner_sig=key.sign_hash(&B256::from(pocold::hashes::win_msg(&tid,nonce,&share_root(&h))));reseal(&mut h);return Ok(Block{header:h,body})}}
    panic!("private fixture solution not found");
}
fn import(chain:&mut Chain,b:&Block){let (e,_)=chain.validate(b,None,0).unwrap_or_else(|e|panic!("h={} {e:?}",b.header.h));chain.insert(e);}
/// Solve on the head and import.
fn next(n:&mut Net,txs:Vec<Vec<u8>>)->Block{let head=n.chain.head;let b=solve(&n.chain,&head,txs,0).unwrap();import(&mut n.chain,&b);b}
fn alice()->Key{devnet::account_key("alice")}
fn newcomer()->Key{devnet::account_key("fn5-newcomer")}
fn pcol(n:u64)->U256{U256::from(n)*U256::from(10u64.pow(18))}
fn tx(key:&Key,nonce:u64,to:Option<Address>,value:U256,data:Vec<u8>,gas:u64)->Vec<u8>{
    Tx{chain_id:CHAIN,nonce,max_priority:U256::from(7),max_fee:U256::from(100_000_000_000u64),gas,to,value,data:Bytes::from(data)}.sign_1559(key)
}
fn fresh(i:u8)->Address{Address::from([0xf5,i,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0x05])}
fn registration(key:&Key,nonce:u64)->Vec<u8>{
    let mut data=keccak256("register(address)")[..4].to_vec();data.extend_from_slice(key.address().into_word().as_slice());
    tx(key,nonce,Some(apply::registry()),U256::ZERO,data,1_000_000)
}
fn balance(c:&Chain,a:Address)->U256{c.view_of(&c.head).unwrap().account(&a).map(|x|x.info.balance).unwrap_or_default()}
fn accounts(c:&Chain)->u64{c.head().summary.as_ref().unwrap().user_accounts}
fn registered(c:&Chain,a:Address)->bool{apply::miner_record(c.view_of(&c.head).unwrap().v(),a).is_some()}
/// Init code that returns `runtime` (under 256 bytes).
fn init(runtime:&[u8])->Vec<u8>{let len=runtime.len() as u8;let mut b=vec![0x60,len,0x60,12,0x60,0,0x39,0x60,len,0x60,0,0xf3];b.extend_from_slice(runtime);b}

/// The S2 run: blocks 1..=H+1 of a chain whose ACCT_CAP is filled at block 1, with the funding of a newcomer at 2
/// and H-1 (halted, AcctCap) and at H (created), its `register` and one more new account at H+1.
fn s2(n:&mut Net)->Vec<Block>{
    let cap=acct_cfg().acct_cap;let (a,c)=(alice(),newcomer());let mut blocks=Vec::new();let mut nonce=0;
    let mut send=|to:Address,value:U256|{let t=tx(&a,nonce,Some(to),value,vec![],50_000);nonce+=1;t};
    blocks.push(next(n,(0..ROOM as u8).map(|i|send(fresh(i),U256::from(1))).collect()));
    assert_eq!(accounts(&n.chain),cap,"positive control: ACCT_CAP reached at block 1");
    blocks.push(next(n,vec![send(c.address(),pcol(1000))]));
    assert_eq!(balance(&n.chain,c.address()),U256::ZERO,"h=2: funding a newcomer halts at the full ACCT_CAP");assert_eq!(accounts(&n.chain),cap);
    // Unfunded, the newcomer cannot register: its transaction cannot be included.
    let head=n.chain.head;assert!(solve(&n.chain,&head,vec![registration(&c,0)],0).is_err(),"an unfunded registration has no block");
    while n.chain.head().height<H-2{blocks.push(next(n,vec![]));}
    blocks.push(next(n,vec![send(c.address(),pcol(1000))]));
    assert_eq!(n.chain.head().height,H-1);assert_eq!(balance(&n.chain,c.address()),U256::ZERO,"h=H-1: still the genesis ACCT_CAP");
    blocks.push(next(n,vec![send(c.address(),pcol(1000))]));
    assert_eq!(n.chain.head().header.protocol_version,3,"block H runs fork version 3");
    assert_eq!(balance(&n.chain,c.address()),pcol(1000),"h=H: the raise admits the newcomer's account");assert_eq!(accounts(&n.chain),cap+1);
    assert!(!registered(&n.chain,c.address()));
    blocks.push(next(n,vec![registration(&c,0),send(fresh(9),U256::from(1))]));
    assert!(registered(&n.chain,c.address()),"h=H+1: the newcomer registers");assert_eq!(accounts(&n.chain),cap+2,"and another new account is created");
    blocks
}

#[test]
fn a_filled_account_cap_stops_new_accounts_and_register_until_the_raise_height(){
    let mut n=net(&acct_cfg(),Some(acct_raise()));let blocks=s2(&mut n);
    assert_eq!(n.chain.spec.cp_at(H-1).ACCT_CAP,acct_cfg().acct_cap);assert_eq!(n.chain.spec.cp_at(H).ACCT_CAP,acct_raise().acct_cap);
    // Replay: a second node importing the same blocks reaches every state exactly.
    let mut replay=net(&acct_cfg(),Some(acct_raise()));
    for b in &blocks{import(&mut replay.chain,b);assert_eq!(replay.chain.head().header.state_root,n.chain.canonical_at(b.header.h).unwrap().header.state_root);}
    assert_eq!(replay.chain.head,n.chain.head);
    assert_eq!(apply::state_root(&replay.chain.head().state).unwrap(),apply::state_root(&n.chain.head().state).unwrap());
}

#[test]
fn a_node_without_the_raise_agrees_below_the_height_and_partitions_from_it(){
    let mut n=net(&acct_cfg(),Some(acct_raise()));let blocks=s2(&mut n);
    let mut old=net(&acct_cfg(),None);
    for b in blocks.iter().filter(|b|b.header.h<H){import(&mut old.chain,b);assert_eq!(old.chain.head().header.state_root,b.header.state_root,"h={}",b.header.h);}
    let at=&blocks[(H-1) as usize];assert_eq!(at.header.h,H);
    assert!(matches!(old.chain.validate(at,None,0),Err(Reject::Block(ref w)) if w=="netVersion"),"the raised block H is refused by a node without the raise");
    // That node's own block H still runs the genesis caps (the funding halts) and the raised node refuses it.
    let head=old.chain.head;let own=solve(&old.chain,&head,vec![tx(&alice(),ROOM+2,Some(newcomer().address()),pcol(1000),vec![],50_000)],0).unwrap();
    import(&mut old.chain,&own);assert_eq!(balance(&old.chain,newcomer().address()),U256::ZERO);assert_eq!(own.header.protocol_version,2);
    let mut raised=net(&acct_cfg(),Some(acct_raise()));for b in blocks.iter().filter(|b|b.header.h<H){import(&mut raised.chain,b);}
    assert!(matches!(raised.chain.validate(&own,None,0),Err(Reject::Block(ref w)) if w=="netVersion"));
    // They do not even peer: the identity commits to the schedule and the raise, and names the missing activation.
    let (new,was)=(identity::Consensus::of(&n.chain.spec),identity::Consensus::of(&old.chain.spec));
    assert_ne!(new.id,was.id);
    assert_eq!(identity::explain(&new,&was.id),format!("peer has not scheduled activation (3,{H}): it must apply the published network upgrade"));
}

#[test]
fn a_reorg_across_the_raise_height_judges_each_branch_at_its_own_heights(){
    let mut n=net(&acct_cfg(),Some(acct_raise()));let (a,c)=(alice(),newcomer());
    let first=next(&mut n,(0..ROOM as u8).map(|i|tx(&a,u64::from(i),Some(fresh(i)),U256::from(1),vec![],50_000)).collect());
    let mut prefix=vec![first];while n.chain.head().height<H-2{prefix.push(next(&mut n,vec![]));}
    let fork=n.chain.head;let funding=|nonce:u64,to:Address|tx(&a,nonce,Some(to),pcol(1000),vec![],50_000);
    // Branch A: funds the newcomer at H-1 (halts) and at H (created).
    let mut a_blocks=Vec::new();let mut parent=fork;
    for (i,txs) in [vec![funding(ROOM,c.address())],vec![funding(ROOM+1,c.address())]].into_iter().enumerate(){
        let b=solve(&n.chain,&parent,txs,0).unwrap();parent=b.hash();import(&mut n.chain,&b);a_blocks.push(b);let _=i;}
    assert_eq!(n.chain.head().height,H);assert_eq!(balance(&n.chain,c.address()),pcol(1000));
    // Branch B from the same parent, heavier: funds another address at H only, and one block later still.
    let other=fresh(7);let mut b_blocks=Vec::new();let mut parent=fork;
    for (i,txs) in [vec![],vec![funding(ROOM,other)],vec![],vec![]].into_iter().enumerate(){
        let b=solve(&n.chain,&parent,txs,if i==0{1}else{0}).unwrap();parent=b.hash();import(&mut n.chain,&b);b_blocks.push(b);}
    assert_eq!(n.chain.head,parent,"the heavier branch B is the head");assert!(!n.chain.is_canonical(&a_blocks[1].hash()));
    assert_eq!(balance(&n.chain,c.address()),U256::ZERO,"branch B never funded the newcomer");assert_eq!(balance(&n.chain,other),pcol(1000),"B's funding at H is created");
    assert_eq!(accounts(&n.chain),acct_cfg().acct_cap+1);
    // Back to A, made heavier: its states are those it had, the raise judged at A's own heights.
    let mut parent=a_blocks[1].hash();for _ in 0..3{let b=solve(&n.chain,&parent,vec![],0).unwrap();parent=b.hash();import(&mut n.chain,&b);a_blocks.push(b);}
    assert_eq!(n.chain.head,parent);assert_eq!(balance(&n.chain,c.address()),pcol(1000));assert_eq!(balance(&n.chain,other),U256::ZERO);
    let mut replay=net(&acct_cfg(),Some(acct_raise()));for b in prefix.iter().chain(a_blocks.iter()){import(&mut replay.chain,b);}
    assert_eq!(replay.chain.head,n.chain.head);
    assert_eq!(apply::state_root(&replay.chain.head().state).unwrap(),apply::state_root(&n.chain.head().state).unwrap());
}

#[test]
fn filled_slot_and_code_caps_halt_and_fail_until_the_raise_height(){
    let cfg=store_cfg();let mut n=net(&cfg,Some(store_raise()));let a=alice();let (slots,code)=(cfg.slot_cap,cfg.code_cap);
    let sum=|c:&Chain|**c.head().summary.as_ref().unwrap();
    // Block 1: a 4-byte deployment fills CODE_CAP exactly.
    next(&mut n,vec![tx(&a,0,None,U256::ZERO,init(&[0;SPIN as usize]),200_000)]);
    assert_eq!(sum(&n.chain).code_total as u64,code,"positive control: CODE_CAP reached");assert_eq!(sum(&n.chain).user_slots,slots,"and SLOT_CAP is full from genesis");
    // A constructor storing a slot (SLOT_CAP halt) and a 32-byte runtime (CODE_CAP fails the CREATE), at 2 and at H-1.
    let store=vec![0x60,1,0x60,0,0x55,0x60,0,0x60,0,0xf3];let mut nonce=1;
    let mut pair=||{let t=vec![tx(&a,nonce,None,U256::ZERO,store.clone(),200_000),tx(&a,nonce+1,None,U256::ZERO,init(&[0;32]),200_000)];nonce+=2;t};
    next(&mut n,pair());assert_eq!((sum(&n.chain).user_slots,sum(&n.chain).code_total as u64),(slots,code),"h=2: both stopped");
    while n.chain.head().height<H-2{next(&mut n,vec![]);}
    next(&mut n,pair());assert_eq!((sum(&n.chain).user_slots,sum(&n.chain).code_total as u64),(slots,code),"h=H-1: both stopped");
    next(&mut n,pair());assert_eq!(n.chain.head().height,H);
    assert_eq!((sum(&n.chain).user_slots,sum(&n.chain).code_total as u64),(slots+1,code+32),"h=H: the slot is stored and the code deployed");
    let mut replay=net(&cfg,Some(store_raise()));
    for h in 1..=H{import(&mut replay.chain,&Block{header:n.chain.canonical_at(h).unwrap().header.clone(),body:n.chain.canonical_at(h).unwrap().body.clone()});}
    assert_eq!(replay.chain.head,n.chain.head);
}

mod node {
    use super::*;
    use pocold::node::{Config,Limits,Node};
    struct Temp(std::path::PathBuf);
    impl Temp{fn new(tag:&str)->Self{Self(std::env::temp_dir().join(format!("pocol-fn5-{tag}-{}",std::process::id())))}}
    impl Drop for Temp{fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
    fn config(dir:&Temp,store:bool)->Config{Config{dir:dir.0.clone(),name:"fn5".into(),miner:None,listen:None,rpc:None,peers:vec![],stop_at:None,log:false,
        limits:Limits{state_store:store,..Default::default()}}}
    fn rpc(node:&mut Node,method:&str,params:serde_json::Value)->serde_json::Value{
        serde_json::from_str(&pocold::rpc::handle(node,json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string().as_bytes())).unwrap()
    }
    fn mine(node:&mut Node,txs:Vec<Vec<u8>>){let head=node.chain.head;let b=solve(&node.chain,&head,txs,0).unwrap();assert!(node.import(b,None).unwrap());}

    /// The pool, eth_estimateGas and pocol_getCaps use the caps of the next block: at a full ACCT_CAP a creating transfer
    /// is refused (AcctCap) while the next block is below H, and admitted once it is H; a reopened node replays the
    /// journal across H to the same head and state. Whole-state and store mode.
    #[test]
    fn the_pool_the_estimate_and_get_caps_follow_the_next_blocks_caps_and_a_restart_replays_across_the_height(){
        for store in [false,true]{
            let dir=Temp::new(if store{"store"}else{"mem"});let cfg=acct_cfg();let r=acct_raise();
            let mut node=Node::open(config(&dir,store),spec_of(&cfg,Some(r))).unwrap();let (a,c)=(alice(),newcomer());
            mine(&mut node,(0..ROOM as u8).map(|i|tx(&a,u64::from(i),Some(fresh(i)),U256::from(1),vec![],50_000)).collect());
            let funding=tx(&a,ROOM,Some(c.address()),pcol(1000),vec![],50_000);
            let request=json!([{"from":a.address(),"to":c.address(),"value":format!("{:#x}",pcol(1000)),"maxFeePerGas":"0x174876e800","maxPriorityFeePerGas":"0x7"},"latest"]);
            while node.chain.head().height<H-2{mine(&mut node,vec![]);}
            // The head is H-2: its child is below H.
            assert_eq!(node.submit_tx(funding.clone(),false).unwrap_err(),"-32000 pending state would exceed a cap (AcctCap)",);
            assert_eq!(rpc(&mut node,"eth_estimateGas",request.clone())["error"]["data"]["reason"],"AcctCap");
            let caps=rpc(&mut node,"pocol_getCaps",json!([]))["result"].clone();
            assert_eq!(caps["accountsCap"],format!("{:#x}",cfg.acct_cap));assert_eq!(caps["occupancy"]["accounts"]["remaining"],"0x0");
            assert_eq!(caps["capRaise"],json!({"ACCT_CAP":format!("{:#x}",r.acct_cap),"SLOT_CAP":format!("{:#x}",r.slot_cap),"CODE_CAP":format!("{:#x}",r.code_cap),
                "SYS_SLOT_MAX":format!("{:#x}",r.sys_slot_max),"height":format!("{H:#x}"),"active":false}),"pocol_getCaps capRaise: hex quantities");
            let params=rpc(&mut node,"pocol_getParams",json!([]))["result"].clone();
            assert_eq!(params["capRaise"],json!({"ACCT_CAP":r.acct_cap,"SLOT_CAP":r.slot_cap,"CODE_CAP":r.code_cap,"SYS_SLOT_MAX":r.sys_slot_max,"height":H}));assert_eq!(params["maxForkVersion"],3);assert_eq!(params["forkSchedule"],json!([[1,0],[2,V2],[3,H]]));
            mine(&mut node,vec![]);
            // The head is H-1: its child is H.
            let e=rpc(&mut node,"eth_estimateGas",request.clone());assert!(e.get("error").is_none(),"{e}");
            node.submit_tx(funding.clone(),false).unwrap();let p=node.chain.full_entry(&node.chain.head).unwrap();
            let sel=node.template_selection(&p,p.ts+u64::from(node.chain.spec.cp().T_blk));assert_eq!(sel.txs,vec![funding]);
            let caps=rpc(&mut node,"pocol_getCaps",json!([]))["result"].clone();
            assert_eq!(caps["accountsCap"],format!("{:#x}",r.acct_cap));assert_eq!(caps["capRaise"]["active"],true);
            mine(&mut node,sel.txs);assert_eq!(node.chain.head().height,H);assert_eq!(balance(&node.chain,c.address()),pcol(1000));
            mine(&mut node,vec![registration(&c,0)]);assert!(registered(&node.chain,c.address()));
            let (head,root)=(node.chain.head,node.chain.head().header.state_root);drop(node);
            let reopened=Node::open(config(&dir,store),spec_of(&cfg,Some(r))).unwrap();
            assert_eq!((reopened.chain.head,reopened.chain.head().header.state_root),(head,root),"store={store}: the journal replays across H");
            assert!(registered(&reopened.chain,c.address()));
            drop(reopened);
            // The same journal under a node without the raise conflicts with its schedule from block H.
            let e=Node::open(config(&dir,store),spec_of(&cfg,None)).err().expect("the journal holds fork-3 blocks");
            assert!(e.contains("conflicts with the configured consensus fork schedule"),"{e}");
        }
    }
}

mod a13 {
    use super::*;
    /// A13 is computed at the largest caps the schedule reaches (`Spec::cp_max`): a node with a raise is bounded above
    /// one without; without a raise `cp_max` is the genesis CP.
    #[test]
    fn the_resident_bound_is_taken_at_the_raised_caps(){
        let cfg=acct_cfg();let big=CapRaise{acct_cap:cfg.acct_cap*1000,..acct_raise()};
        let (raised,plain)=(spec_of(&cfg,Some(big)),spec_of(&cfg,None));
        assert_eq!(plain.cp_max(),plain.cp());assert_eq!(raised.cp_max(),raised.cp_at(H));assert_ne!(raised.cp_max(),raised.cp());
        let bound=|s:Spec,tag:&str|{let dir=std::env::temp_dir().join(format!("pocol-fn5-a13-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
            let n=pocold::node::Node::open(pocold::node::Config{dir:dir.clone(),name:"fn5-a13".into(),miner:None,listen:None,rpc:None,peers:vec![],stop_at:None,log:false,limits:Default::default()},s).unwrap();
            let b:u128=n.status_json()["a13"]["boundBytes"].as_str().unwrap().parse().unwrap();drop(n);let _=std::fs::remove_dir_all(&dir);b};
        let (r,p)=(bound(raised,"raised"),bound(plain,"plain"));assert!(r>p,"{r} > {p}");
        println!("A13 resident bound: {p} B without the raise, {r} B with ACCT_CAP x1000 from H");
    }
}

mod configuration {
    use super::*;
    use pocold::{configured_cap_raise,configured_forks};
    fn info(raise:serde_json::Value)->serde_json::Value{json!({"forks":[[1,0],[2,V2],[3,H]],"capRaise":raise})}
    #[test]
    fn the_cap_raise_is_carried_beside_its_fork_3_entry_and_fails_closed(){
        let ok=json!({"ACCT_CAP":10,"SLOT_CAP":20,"CODE_CAP":30,"SYS_SLOT_MAX":40});
        assert_eq!(configured_cap_raise(&info(ok.clone())).unwrap(),Some(CapRaise{acct_cap:10,slot_cap:20,code_cap:30,sys_slot_max:40}));
        assert_eq!(pocold::cap_raise_json(&configured_cap_raise(&info(ok.clone())).unwrap().unwrap()),ok);
        assert_eq!(configured_forks(&info(ok.clone())).unwrap(),vec![(1,0),(2,V2),(3,H)]);
        // Dormant: no fork 3 and no capRaise.
        for i in [json!({}),json!({"forks":[[1,0],[2,V2]]})]{assert_eq!(configured_cap_raise(&i).unwrap(),None);}
        // Fork 3 needs capRaise and capRaise needs fork 3.
        assert!(configured_forks(&json!({"forks":[[1,0],[2,V2],[3,H]]})).unwrap_err().contains("needs capRaise"));
        assert!(configured_forks(&json!({"forks":[[1,0],[2,V2]],"capRaise":ok})).unwrap_err().contains("needs a fork version 3"));
        assert!(configured_cap_raise(&json!({"capRaise":ok})).is_err());
        // Its form: four integers, no other field, SYS_SLOT_MAX within 32 bits.
        for bad in [json!(null),json!([10,20,30,40]),json!({"ACCT_CAP":10,"SLOT_CAP":20,"CODE_CAP":30}),
            json!({"ACCT_CAP":10,"SLOT_CAP":20,"CODE_CAP":30,"SYS_SLOT_MAX":40,"TX_CAP":1}),json!({"ACCT_CAP":10,"SLOT_CAP":20,"CODE_CAP":30,"SYS_SLOTS":40}),
            json!({"ACCT_CAP":10.5,"SLOT_CAP":20,"CODE_CAP":30,"SYS_SLOT_MAX":40}),json!({"ACCT_CAP":"10","SLOT_CAP":20,"CODE_CAP":30,"SYS_SLOT_MAX":40}),
            json!({"ACCT_CAP":-1,"SLOT_CAP":20,"CODE_CAP":30,"SYS_SLOT_MAX":40}),json!({"ACCT_CAP":10,"SLOT_CAP":20,"CODE_CAP":30,"SYS_SLOT_MAX":1u64<<32})]{
            assert!(configured_cap_raise(&info(bad.clone())).unwrap_err().starts_with("invalid capRaise"),"{bad}");
        }
        // Version 4 is not implemented.
        assert!(configured_forks(&json!({"forks":[[1,0],[2,V2],[3,H],[4,H+1]],"capRaise":ok})).is_err());
    }

    /// A network directory with capRaise in devnet.json boots through the full admission with the raise; one that
    /// lowers a cap is refused (R24), and one whose raise breaks a rule at the raised CP is refused with that rule.
    #[test]
    fn a_network_directory_carries_the_raise_to_the_admission(){
        let dir=std::env::temp_dir().join(format!("pocol-fn5-load-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);std::fs::create_dir_all(&dir).unwrap();
        let cfg=acct_cfg();let d=devnet::build(&cfg).unwrap();
        std::fs::write(dir.join("genesis.spec.hex"),hex(&d.spec_bytes)).unwrap();
        std::fs::write(dir.join("alloc.json"),devnet::alloc_to_json(&d.alloc).to_string()).unwrap();
        let write=|raise:&CapRaise|std::fs::write(dir.join("devnet.json"),json!({"calibration":{"hashesPerSecond":d.hash_rate},"forks":[[1,0],[2,V2],[3,H]],
            "capRaise":pocold::cap_raise_json(raise)}).to_string()).unwrap();
        let r=acct_raise();write(&r);
        let spec=pocold::load_spec(&dir,&pocold::single_host_local()).unwrap();
        assert_eq!(spec.profile().cap_raise(),Some((H,r)));assert_eq!(spec.cp_at(H).ACCT_CAP,r.acct_cap);assert_eq!(spec.cp_at(H-1).ACCT_CAP,cfg.acct_cap);
        assert_eq!(spec.genesis_hash(),d.profile.genesis_hash(),"the raise is outside the genesis");
        write(&CapRaise{acct_cap:cfg.acct_cap-1,slot_cap:cfg.slot_cap+1,..r});
        let e=pocold::load_spec(&dir,&pocold::single_host_local()).err().unwrap();assert!(e.starts_with("-32018 ParamGate rejected [R24]")&&e.contains("may not lower a cap"),"{e}");
        let strict=pocol_lp4_execution::admission::NodeLocal{retention_stress:false,..pocold::single_host_local()};
        write(&CapRaise{acct_cap:1<<40,..r});
        let e=pocold::load_spec(&dir,&strict).err().unwrap();assert!(e.contains("cap raise: the raised CP violates [R21]"),"{e}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The consensus identity: unchanged byte for byte without a raise; with one it commits to the values after the
    /// schedule ("cap-raise" ‖ CODE_CAP ‖ SLOT_CAP ‖ ACCT_CAP u64 ‖ SYS_SLOT_MAX u32), so another value set does not peer.
    #[test]
    fn the_identity_commits_to_the_raise_and_is_unchanged_without_one(){
        let g=[3u8;32];let forks=[(1u16,0u64),(2,V2),(3,H)];let r=CapRaise{acct_cap:10,slot_cap:20,code_cap:30,sys_slot_max:40};
        let plain=identity::consensus_id(CHAIN,&g,2,0,&forks[..2],None);
        assert_eq!(identity::consensus_id_raised(CHAIN,&g,2,0,&forks[..2],None,None),plain);
        let mut b=b"pocol-consensus-identity-v1\0".to_vec();b.extend(CHAIN.to_be_bytes());b.extend(g);b.extend(2u32.to_be_bytes());b.extend([0u8,3]);
        for (v,h) in forks{b.extend(v.to_be_bytes());b.extend(h.to_be_bytes());}
        b.extend(b"cap-raise");b.extend(30u64.to_be_bytes());b.extend(20u64.to_be_bytes());b.extend(10u64.to_be_bytes());b.extend(40u32.to_be_bytes());
        use sha2::Digest;let want:[u8;32]=sha2::Sha256::digest(&b).into();
        assert_eq!(identity::consensus_id_raised(CHAIN,&g,2,0,&forks,Some(&r),None),want);
        // Pinned with deploy/public-testnet/test_pocol_identity.py (test_a_cap_raise_is_committed_after_the_schedule).
        assert_eq!(identity::Consensus::raised(CHAIN,g,&forks,Some(r),2,0,None).hex(),"1f28be8d438ea0b98309dab3710d20702e2410e027cba01d3067e492f720edb0");
        let other=identity::Consensus::raised(CHAIN,g,&forks,Some(CapRaise{acct_cap:11,..r}),2,0,None);
        let ours=identity::Consensus::raised(CHAIN,g,&forks,Some(r),2,0,None);assert_ne!(ours.id,other.id);
        assert_eq!(identity::explain(&ours,&identity::Consensus::new(CHAIN,g,&forks[..2],2,0,None).id),format!("peer has not scheduled activation (3,{H}): it must apply the published network upgrade"));
        assert_ne!(identity::consensus_id_raised(CHAIN,&g,2,0,&forks,None,None),want,"the same schedule without the raise is another identity");
        assert!(std::panic::catch_unwind(||identity::consensus_id_raised(CHAIN,&g,2,0,&forks[..2],Some(&r),None)).is_err(),"a raise goes with its fork-3 entry");
    }
}
