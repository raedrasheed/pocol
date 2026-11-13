//! N4 (V3-20 node-local pool, V3-24 RPC): the fee-ordered selection, replace-by-fee, lowest-fee eviction and the
//! estimate's log caps, on the spec-1 devnet. Pool policy only: the blocks built from it are validated by the
//! unchanged consensus code (`Chain::validate`) and imported.
use std::collections::HashMap;
use pocold::{chain::{Chain,Spec},types::*,hashes,node::{self,TxFields,Limits,Node}};
use pocol_lp4_execution::{devnet::{self,DevnetConfig},txsign::{Key,Tx}};
use revm::primitives::{Address,B256,U256};

const CHAIN:u64=777910;const GWEI:u64=1_000_000_000;
struct Net{chain:Chain,keys:HashMap<Address,Key>}
/// The consensus suite's gate-valid test profile (consensus.rs `cfg`) with more funded senders.
fn cfg(extra:&[&str])->DevnetConfig{
    let mut c=DevnetConfig::new(CHAIN,1_700_000_000,&["a","b","c"]);c.hash_rate=1024;c.d_att=73;c.tau=40;c.c=8;
    for l in extra{c.accounts.push(((*l).into(),U256::from(10u128.pow(24))));}c
}
fn admitted(c:&DevnetConfig)->Spec{let d=devnet::build(c).unwrap();Spec::new(d.admit(&pocold::single_host_local()).unwrap_or_else(|e|panic!("{e}")).0)}
fn net_with(c:&DevnetConfig)->Net{
    let d=devnet::build(c).unwrap();let mut keys=HashMap::new();
    for m in &d.miners{keys.insert(m.id,Key::from_secret(&m.key.secret()).unwrap());}
    Net{chain:Chain::new(admitted(c)),keys}
}
fn try_seal(n:&Net,mut h:pocold::header::Header,body:Body,attempt:u32)->Option<Block>{
    let p=n.chain.full_entry(&h.parent_hash).unwrap();let ctx=n.chain.child_ctx(&p,attempt).unwrap();
    let tid=template_id(&h.ut_raw);let mut shares=vec![];let mut x=0u64;
    loop{if (x as u128)>=ctx.assignment.nonce_max(){return None}let ph=pow_hash(&tid,x);if le_target(&ph,&ctx.t_share){if le_target(&ph,&ctx.target){break}shares.push(x);}x+=1;}
    let owner=addr(&ctx.assignment.owner(x).unwrap());
    h.nonce=x;shares.truncate(256);h.shares=shares;reseal(&mut h);
    h.winner_sig=n.keys[&owner].sign_hash(&B256::from(hashes::win_msg(&tid,x,&share_root(&h))));reseal(&mut h);
    Some(Block{header:h,body})
}
/// A signed block by Draw(h,0) on `parent` carrying `txs`; the consensus code builds (and so validates) it.
fn signed(n:&Net,parent:[u8;32],txs:Vec<Vec<u8>>)->Block{
    let p=n.chain.full_entry(&parent).unwrap();let ctx=n.chain.child_ctx(&p,0).unwrap();let body=Body{txs,evidence:vec![]};
    for off in 0..u64::from(n.chain.spec.cp().D_att){let (h,_,_)=n.chain.build_signed(&p,&n.keys[&ctx.proposer],ctx.window.lower()+off,body.clone(),vec![]).unwrap();
        if let Some(b)=try_seal(n,h,body.clone(),0){return b}}panic!("no signed solution")
}
fn fallback(n:&Net,parent:[u8;32])->Block{let p=n.chain.full_entry(&parent).unwrap();
    for a in 0..16{let (h,_,_)=n.chain.build_fallback(&p,a).unwrap();if let Some(b)=try_seal(n,h,Body::default(),a){return b}}panic!("no fallback solution")}
fn import(n:&mut Net,b:&Block){let (e,_)=n.chain.validate(b,None,0).unwrap_or_else(|e|panic!("h={} {e:?}",b.header.h));assert!(n.chain.insert(e));}
struct Dir(std::path::PathBuf);
impl Drop for Dir{fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
fn node_with(tag:&str,limits:Limits,c:&DevnetConfig)->(Node,Dir){
    let dir=std::env::temp_dir().join(format!("pocold-n4-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    (Node::open(node::Config{dir:dir.clone(),name:tag.into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits},admitted(c)).unwrap(),Dir(dir))
}
fn key(l:&str)->Key{devnet::account_key(l)}
/// A transfer to `to` with fee caps (max fee, priority) in wei per gas.
fn tx(k:&Key,nonce:u64,cap:u64,tip:u64,gas:u64,to:Address)->Vec<u8>{
    Tx{chain_id:CHAIN,nonce,max_priority:U256::from(tip),max_fee:U256::from(cap),gas,to:Some(to),value:U256::from(1u64),data:Default::default()}.sign_1559(k)
}
fn pay(k:&Key,nonce:u64,tip:u64)->Vec<u8>{tx(k,nonce,100*GWEI,tip,21000,Address::with_last_byte(0xbb))}
fn head(n:&Node)->pocold::chain::Entry{n.chain.full_entry(&n.chain.head).unwrap()}
fn base(n:&Node)->U256{n.chain.child_ctx(&head(n),0).unwrap().base_fee}
fn rpc(n:&mut Node,method:&str,params:serde_json::Value)->serde_json::Value{
    serde_json::from_str(&pocold::rpc::handle(n,serde_json::json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string().as_bytes())).unwrap()
}
fn hex(b:&[u8])->String{format!("0x{}",b.iter().map(|x|format!("{x:02x}")).collect::<String>())}

/// N4 (1): the selection takes the highest priority first, a sender's transaction only after its predecessor. A
/// candidate's priority is the lowest effective tip among it and its sender's earlier ones. alice pools nonce 0 at
/// 1 gwei and nonce 1 at 50 gwei, bob 10 gwei, carol 5 gwei: the order is bob, carol, alice 0, alice 1 (alice's 50
/// gwei ranks at her nonce 0's 1 gwei). The effective tip is min(priority, max fee - base fee): erin's
/// priority of base + 2 gwei under a max fee of base + 2 gwei pays 2 gwei. The block built from it is valid and imported.
#[test] fn the_selection_takes_the_highest_effective_tip_first_in_nonce_order(){
    let c=cfg(&["carol","erin"]);let (mut n,_d)=node_with("order",Limits::default(),&c);let mut h=net_with(&c);
    let b=base(&n);assert!(b<=U256::from(GWEI),"the test's caps (100 gwei) are far above the base fee {b}");let b64=b.to::<u64>();
    let (alice,bob,carol,erin)=(key("alice"),key("bob"),key("carol"),key("erin"));
    let a0=pay(&alice,0,GWEI);let a1=pay(&alice,1,50*GWEI);let b0=pay(&bob,0,10*GWEI);let c0=pay(&carol,0,5*GWEI);
    let e0=tx(&erin,0,b64+2*GWEI,b64+2*GWEI,21000,Address::with_last_byte(0xbb));
    for t in [&a0,&a1,&b0,&c0,&e0]{n.submit_tx(t.clone(),false).unwrap();}n.pool_audit().unwrap();
    let f=TxFields::of(&pocol_lp4_execution::wire::decode_for_profile(&e0,n.chain.spec.profile()).unwrap().tx_env());
    assert_eq!((f.effective_tip(b),node::tx_fields(&e0)),(2*GWEI,Some(f)),"read from the bytes as the decoder reads it");
    // Pool order: a0 a1 b0 c0 e0 (keys 0..4).
    assert_eq!(n.fee_order(b),vec![2,3,4,0,1],"bob 10, carol 5, erin 2 (capped), alice 1 then her 50");
    let p=head(&n);let sel=n.template_selection(&p,pocold::node::now_s());
    assert_eq!(sel.txs,vec![b0.clone(),c0.clone(),e0.clone(),a0.clone(),a1.clone()]);
    let blk=signed(&h,h.chain.head,sel.txs.clone());import(&mut h,&blk);
    assert!(n.import(blk.clone(),None).unwrap(),"the fee-ordered block is valid on the spec-1 devnet");n.pool_settle();
    assert!(n.pool.txs.is_empty(),"all included");n.pool_audit().unwrap();
    // A fee cap at the base fee pays no tip (min(priority, 0)): it comes after a later, paying arrival.
    let b2=base(&n).to::<u64>();let low=tx(&bob,1,b2,b2,21000,Address::with_last_byte(0xbb));let ok=pay(&carol,1,GWEI/2);
    n.submit_tx(low.clone(),false).unwrap();n.submit_tx(ok.clone(),false).unwrap();
    let sel=n.template_selection(&head(&n),pocold::node::now_s());assert_eq!(sel.txs,vec![ok,low]);
}

/// N4 (1): ties are broken by candidate order (the pool's order, then the parked ones' arrival), so equal tips keep
/// the order before N4 and the result is the same on every call and on every node with the same pool.
#[test] fn equal_tips_keep_pool_order_deterministically(){
    let c=cfg(&["carol","erin"]);let (mut n,_d)=node_with("ties",Limits::default(),&c);let (mut m,_e)=node_with("ties2",Limits::default(),&c);
    let txs=vec![pay(&key("carol"),0,3*GWEI),pay(&key("bob"),0,3*GWEI),pay(&key("erin"),0,3*GWEI),pay(&key("alice"),0,3*GWEI),pay(&key("bob"),1,3*GWEI)];
    for t in &txs{n.submit_tx(t.clone(),false).unwrap();m.submit_tx(t.clone(),false).unwrap();}
    let b=base(&n);assert_eq!(n.fee_order(b),vec![0,1,2,3,4]);assert_eq!(n.fee_order(b),m.fee_order(b));
    let (s1,s2)=(n.template_selection(&head(&n),pocold::node::now_s()).txs,m.template_selection(&head(&m),pocold::node::now_s()).txs);
    assert_eq!(s1,txs);assert_eq!(s1,s2);
}

/// N4 (2): replace-by-fee at the bump boundary. Both caps must rise by POOL_PRICE_BUMP (10) percent: +10% of one
/// and +10% - 1 wei of the other is refused (POOL_REPLACE_UNDERPRICED, -32000 over RPC, nothing changes); exactly
/// +10% of both replaces the pooled transaction in place. The replacement is what the selection and the next head
/// take; the replaced one is gone. A parked transaction is replaced the same way.
#[test] fn replace_by_fee_needs_both_caps_up_by_the_bump(){
    use node::{POOL_REPLACE_UNDERPRICED,POOL_PRICE_BUMP};assert_eq!(POOL_PRICE_BUMP,10);
    let c=cfg(&["carol"]);let (mut n,_d)=node_with("rbf",Limits::default(),&c);let mut h=net_with(&c);let alice=key("alice");let to=Address::with_last_byte(0xbb);
    let old=tx(&alice,0,100*GWEI,10*GWEI,21000,to);let next=pay(&alice,1,GWEI);n.submit_tx(old.clone(),false).unwrap();n.submit_tx(next.clone(),false).unwrap();
    let gas=n.pool.gas;
    for (cap,tip) in [(110*GWEI,11*GWEI-1),(110*GWEI-1,11*GWEI),(100*GWEI,20*GWEI),(200*GWEI,10*GWEI)]{
        let e=n.submit_tx(tx(&alice,0,cap,tip,21000,to),false).unwrap_err();assert!(e.starts_with(POOL_REPLACE_UNDERPRICED),"{cap} {tip}: {e}");
        assert!(e.contains("maxFeePerGas >= 110000000000")&&e.contains("maxPriorityFeePerGas >= 11000000000"),"the answer names the least caps: {e}");
    }
    let raw=hex(&tx(&alice,0,105*GWEI,11*GWEI,21000,to));let r=rpc(&mut n,"eth_sendRawTransaction",serde_json::json!([raw]));
    assert_eq!(r["error"]["code"],-32000,"{r}");assert!(r["error"]["message"].as_str().unwrap().contains("replacement transaction underpriced"),"{r}");
    assert_eq!(n.pool.txs[0].1,old,"refused, nothing changed");assert_eq!(n.pool.gas,gas);n.pool_audit().unwrap();
    let new=tx(&alice,0,110*GWEI,11*GWEI,21000,to);let hash=n.submit_tx(new.clone(),false).unwrap();
    assert_eq!((n.pool.txs.len(),&n.pool.txs[0].1,n.pool.txs[0].0),(2,&new,hash),"replaced in place");assert_eq!(n.pool.gas,gas);n.pool_audit().unwrap();
    assert_eq!(n.stats.pool_replaced,1);
    let sel=n.template_selection(&head(&n),pocold::node::now_s());assert_eq!(sel.txs,vec![new.clone(),next.clone()],"the replacement, then its successor");
    let blk=signed(&h,h.chain.head,sel.txs);import(&mut h,&blk);assert!(n.import(blk,None).unwrap());n.pool_settle();assert!(n.pool.txs.is_empty());n.pool_audit().unwrap();
    // A replacement that declares less gas: the entry stays charged the old one's gas until the next head (N4 second
    // review: the most it declared since the last head, so the pending layer keeps it and nothing goes to ghost_gas).
    let carol=key("carol");n.submit_tx(tx(&carol,0,100*GWEI,GWEI,50_000,to),false).unwrap();
    n.submit_tx(tx(&carol,0,110*GWEI,2*GWEI,21000,to),false).unwrap();assert_eq!((n.pool.gas,n.pool.ghost_gas),(50_000,0));n.pool_audit().unwrap();
    let f=fallback(&h,h.chain.head);import(&mut h,&f);n.import(f,None).unwrap();n.pool_settle();assert_eq!((n.pool.gas,n.pool.ghost_gas),(21000,0),"recounted at the next head");n.pool_audit().unwrap();
}

/// N4 (2): replacements are bounded per entry (POOL_REPLACE_SLOT_MAX) and per sender (POOL_REPLACE_SENDER_MAX) between
/// two heads; past either the answer is POOL_REPLACE_LIMIT (-32016) and nothing changes. N4 review: there is no
/// pool-wide limit on accepting them, so senders that use up POOL_REPLACE_RELAY_MAX relays do not stop another
/// sender's replacement: it is accepted and pooled, only not relayed (`replace_unrelayed`). Each accepted one is
/// offered to the peers at most once, so replacements cost at most POOL_REPLACE_RELAY_MAX relays per head. A new head
/// restores the budgets.
#[test] fn replacements_are_bounded_per_entry_per_sender_and_relays_per_head(){
    use node::{POOL_REPLACE_LIMIT,POOL_REPLACE_RELAY_MAX,POOL_REPLACE_SENDER_MAX,POOL_REPLACE_SLOT_MAX};
    let (slot,per)=(u32::from(POOL_REPLACE_SLOT_MAX),POOL_REPLACE_SENDER_MAX);assert_eq!((slot,per),(4,8));
    let spenders=POOL_REPLACE_RELAY_MAX/per as usize;
    let senders:Vec<String>=(0..=spenders).map(|i|format!("rbf-{i}")).collect();
    let refs:Vec<&str>=senders.iter().map(|s|s.as_str()).collect();let c=cfg(&refs);let (mut n,_d)=node_with("rbfspam",Limits::default(),&c);let mut h=net_with(&c);
    // Fee caps that rise by 20% at each replacement.
    let fees=|k:u32|(100*GWEI*6u64.pow(k)/5u64.pow(k),GWEI*6u64.pow(k)/5u64.pow(k));
    let t=|k:&Key,nonce:u64,j:u32|{let (c,t)=fees(j);tx(k,nonce,c,t,21000,Address::with_last_byte(1))};
    // The first sender: an entry takes POOL_REPLACE_SLOT_MAX replacements, then one more is refused (its budget).
    let first=key(&senders[0]);n.submit_tx(t(&first,0,0),true).unwrap();
    for j in 1..=slot{n.submit_tx(t(&first,0,j),true).unwrap();}
    let over=t(&first,0,slot+1);let held=n.pool.txs[0].1.clone();
    assert_eq!(n.submit_tx(over.clone(),true).unwrap_err(),POOL_REPLACE_LIMIT,"the entry's budget is spent");assert_eq!(n.pool.txs[0].1,held);
    let r=rpc(&mut n,"eth_sendRawTransaction",serde_json::json!([hex(&over)]));assert_eq!(r["error"]["code"],-32016,"{r}");
    // Its second entry takes the rest of the sender's POOL_REPLACE_SENDER_MAX; a third entry's first one is refused.
    n.submit_tx(t(&first,1,0),true).unwrap();for j in 1..=slot{n.submit_tx(t(&first,1,j),true).unwrap();}
    n.submit_tx(t(&first,2,0),true).unwrap();assert_eq!(n.submit_tx(t(&first,2,1),true).unwrap_err(),POOL_REPLACE_LIMIT,"the sender's budget is spent");
    // The other senders use up the relays: POOL_REPLACE_RELAY_MAX in all.
    for s in &senders[1..spenders]{let k=key(s);for e in 0..2{n.submit_tx(t(&k,e,0),true).unwrap();for j in 1..=slot{n.submit_tx(t(&k,e,j),true).unwrap();}}}
    assert_eq!((n.stats.pool_replaced,n.stats.replace_relayed,n.stats.replace_unrelayed),(POOL_REPLACE_RELAY_MAX as u64,POOL_REPLACE_RELAY_MAX as u64,0));
    // One more sender still replaces: accepted and pooled, not relayed.
    let last=key(&senders[spenders]);n.submit_tx(t(&last,0,0),true).unwrap();let hash=n.submit_tx(t(&last,0,1),true).unwrap();
    assert!(n.pool.txs.iter().any(|(h,_)|*h==hash),"another sender's replacement is pooled");
    assert_eq!((n.stats.pool_replaced,n.stats.replace_relayed,n.stats.replace_unrelayed),(POOL_REPLACE_RELAY_MAX as u64+1,POOL_REPLACE_RELAY_MAX as u64,1),"each offered once, at most POOL_REPLACE_RELAY_MAX per head");
    n.pool_audit().unwrap();
    // A new head (empty) restores the budgets.
    let f=fallback(&h,h.chain.head);import(&mut h,&f);n.import(f,None).unwrap();n.pool_settle();n.pool_audit().unwrap();
    n.submit_tx(over,true).unwrap();assert_eq!((n.stats.pool_replaced,n.stats.replace_relayed),(POOL_REPLACE_RELAY_MAX as u64+2,POOL_REPLACE_RELAY_MAX as u64+1));n.pool_audit().unwrap();
}

/// N4 review (finding 1): a replacement must be as strict as a new admission. The overdraft rule: a new submission and
/// a replacement must be covered by the sender's head-state balance together with its other pooled and parked
/// transactions (gas_limit x maxFeePerGas + value each). So a replacement cannot drain the balance its sender's later
/// transactions need, and a sender funded only by a pending transfer waits until that transfer is mined (taking the
/// transfer out before the next head, or ordering by fee, cannot leave it pooled but unaffordable).
#[test] fn the_overdraft_rule_covers_pooled_and_parked_spending(){
    use node::POOL_OVERDRAFT;use pocol_lp4_execution::view::View;
    let c=cfg(&[]);let (mut n,_d)=node_with("overdraft",Limits::default(),&c);let mut h=net_with(&c);
    let (alice,newbie)=(key("alice"),key("n4-newbie"));let to=Address::with_last_byte(0xbb);
    let send=|nonce:u64,cap:u64,tip:u64,to:Address,value:U256|Tx{chain_id:CHAIN,nonce,max_priority:U256::from(tip),max_fee:U256::from(cap),gas:21000,to:Some(to),value,data:Default::default()}.sign_1559(&alice);
    let fund=send(0,100*GWEI,GWEI,newbie.address(),U256::from(10u128.pow(18)));n.submit_tx(fund.clone(),false).unwrap();
    // newbie can pay on the pending state, not from its head balance (0).
    assert!(n.pool_view().account(&newbie.address()).unwrap().info.balance>=U256::from(10u128.pow(18)));
    let nb=pay(&newbie,0,GWEI);assert_eq!(n.submit_tx(nb.clone(),false).unwrap_err(),POOL_OVERDRAFT);assert_eq!(n.stats.overdraft_refused,1);
    // alice's later transactions pay 50 gwei behind her funding at 1 gwei.
    let later:Vec<Vec<u8>>=(1..4).map(|k|send(k,100*GWEI,50*GWEI,to,U256::from(1u64))).collect();for t in &later{n.submit_tx(t.clone(),false).unwrap();}
    // A replacement of the funding whose value takes the balance its later ones need is refused; nothing changes.
    let bal:U256=rpc(&mut n,"eth_getBalance",serde_json::json!([alice.address(),"latest"]))["result"].as_str().unwrap().parse().unwrap();assert_eq!(bal,U256::from(10u128.pow(24)));
    let drain=send(0,110*GWEI,2*GWEI,Address::with_last_byte(0xcc),bal-U256::from(21000u64*110*GWEI));
    assert_eq!(n.submit_tx(drain,false).unwrap_err(),POOL_OVERDRAFT);assert_eq!(n.pool.txs[0].1,fund);
    let r=rpc(&mut n,"eth_sendRawTransaction",serde_json::json!([hex(&send(0,110*GWEI,2*GWEI,to,bal))]));assert_eq!(r["error"]["code"],-32000,"{r}");
    // One its later ones stay affordable with is accepted.
    let ok=send(0,110*GWEI,2*GWEI,newbie.address(),U256::from(10u128.pow(18)));n.submit_tx(ok.clone(),false).unwrap();assert_eq!(n.pool.txs[0].1,ok);n.pool_audit().unwrap();
    // Once the funding is mined, newbie's transaction is admitted.
    let sel=n.template_selection(&head(&n),pocold::node::now_s());assert_eq!(sel.txs.len(),4);
    let blk=signed(&h,h.chain.head,sel.txs);import(&mut h,&blk);assert!(n.import(blk,None).unwrap());n.pool_settle();
    n.submit_tx(nb,false).unwrap();n.pool_audit().unwrap();
}

/// N4 (3): lowest-priority eviction at the count bound (POOL_MAX_TXS). Fifteen senders pool 256 transfers and one 255
/// at 2 gwei, except s3, whose first one pays 1 gwei and its other 255 50 gwei; vic pools one at 1 gwei. N4 review:
/// a tail ranks at the lowest tip along its sender's transactions (the fee order's rank), so s3's 50 gwei tail ranks at
/// 1 gwei. A newcomer whose own rank is no higher than the lowest (1 gwei) is refused (POOL_EVICT_UNDERPRICED, -32016
/// over RPC) and nothing changes. A 3 gwei newcomer evicts vic (the latest of the two lowest), the next one s3's 50
/// gwei tail, never an earlier nonce. N4 review: an evicted transaction's nonce goes back to its sender in the pending
/// state, so vic's pending nonce is 0 again, its next nonce is refused (not pooled to be dropped at the next head), and
/// it can submit nonce 0 again at a fee that outbids the lowest. s3 cannot outbid with its own 9 gwei: it ranks at its
/// 1 gwei first one. The pending layer keeps the evicted gas (`ghost_gas`) until the next head, which revalidates the
/// pool without the evicted transactions.
#[test] fn a_higher_priority_evicts_the_lowest_tail_at_the_count_bound(){
    use node::{POOL_EVICT_UNDERPRICED,POOL_MAX_TXS};use pocol_lp4_execution::view::View;
    let senders:Vec<String>=(0..16).map(|i|format!("s{i}")).collect();let mut labels:Vec<&str>=senders.iter().map(|s|s.as_str()).collect();labels.extend(["zed","yan","vic"]);
    let c=cfg(&labels);let gas_max=POOL_MAX_TXS as u64*21000+10*21000;
    let (mut n,_d)=node_with("evict",Limits{pool_gas_max:Some(gas_max),..Default::default()},&c);let mut h=net_with(&c);
    let per=POOL_MAX_TXS/senders.len();
    for (i,s) in senders.iter().enumerate(){let k=key(s);for j in 0..per-usize::from(i==15){n.submit_tx(pay(&k,j as u64,match (i,j){(3,0)=>GWEI,(3,_)=>50*GWEI,_=>2*GWEI}),false).unwrap();}}
    let v0=pay(&key("vic"),0,GWEI);n.submit_tx(v0.clone(),false).unwrap();
    assert_eq!(n.pool.txs.len(),POOL_MAX_TXS);n.pool_audit().unwrap();
    let held=|n:&Node,who:&str,nonce:u64|{let a=key(who).address();n.pool.txs.iter().any(|(_,r)|{let e=pocol_lp4_execution::wire::decode_for_profile(r,n.chain.spec.profile()).unwrap();e.sender()==a&&e.tx_env().nonce==Some(nonce)})};
    let pending=|n:&mut Node,who:&str|rpc(n,"eth_getTransactionCount",serde_json::json!([key(who).address(),"pending"]))["result"].clone();
    // A lower or equal rank is refused.
    for tip in [GWEI,GWEI/2]{let e=n.submit_tx(pay(&key("yan"),0,tip),false).unwrap_err();assert!(e.starts_with(POOL_EVICT_UNDERPRICED)&&e.contains("1000000000 wei"),"{e}");}
    let r=rpc(&mut n,"eth_sendRawTransaction",serde_json::json!([hex(&pay(&key("yan"),0,GWEI))]));assert_eq!(r["error"]["code"],-32016,"{r}");
    assert!(r["error"]["message"].as_str().unwrap().contains("tip too low to evict"),"{r}");
    assert_eq!((n.pool.txs.len(),n.pool.ghost_gas,n.stats.pool_evicted),(POOL_MAX_TXS,0,0),"nothing changed");
    // zed evicts vic's (the latest of the two lowest ranks); vic's nonce is free again in the pending state.
    let z0=pay(&key("zed"),0,3*GWEI);n.submit_tx(z0.clone(),false).unwrap();
    assert!(!held(&n,"vic",0)&&held(&n,"s3",per as u64-1));assert_eq!((n.pool.txs.len(),n.pool.ghost_gas,n.stats.pool_evicted),(POOL_MAX_TXS,21000,1));
    assert_eq!(pending(&mut n,"vic"),"0x0");assert_eq!(n.pool_view().account(&key("vic").address()).unwrap().info.nonce,0);
    assert_eq!(n.submit_tx(pay(&key("vic"),1,9*GWEI),false).unwrap_err(),"-32016 pool full","its next nonce would follow a gap");n.pool_audit().unwrap();
    // zed's next one evicts s3's 50 gwei tail (ranked at 1 gwei), never an earlier nonce, and never zed's own.
    n.submit_tx(pay(&key("zed"),1,3*GWEI),false).unwrap();assert!(!held(&n,"s3",per as u64-1)&&held(&n,"s3",per as u64-2)&&held(&n,"zed",0));
    assert_eq!(pending(&mut n,"s3"),serde_json::json!(format!("{:#x}",per-1)));
    let e=n.submit_tx(pay(&key("s3"),per as u64-1,9*GWEI),false).unwrap_err();assert!(e.starts_with(POOL_EVICT_UNDERPRICED)&&e.contains("ranks at 1000000000 wei"),"{e}");
    // vic outbids the lowest with its nonce 0 again.
    let v1=pay(&key("vic"),0,4*GWEI);n.submit_tx(v1.clone(),false).unwrap();assert!(held(&n,"vic",0)&&!held(&n,"s3",per as u64-2));
    assert_eq!((n.pool.txs.len(),n.pool.ghost_gas,n.stats.pool_evicted),(POOL_MAX_TXS,63000,3));n.pool_audit().unwrap();
    // The selection takes the higher ranks first: vic's 4 gwei, then zed's two.
    let sel=n.template_selection(&head(&n),pocold::node::now_s());assert_eq!(sel.txs[..2],[v1.clone(),z0.clone()]);
    // The next head re-executes the pool from its state: the evicted ones are gone, the counters are recounted.
    let f=fallback(&h,h.chain.head);import(&mut h,&f);n.import(f,None).unwrap();n.pool_settle();
    assert_eq!((n.pool.txs.len(),n.pool.ghost_gas,n.pool.gas),(POOL_MAX_TXS,0,POOL_MAX_TXS as u64*21000));n.pool_audit().unwrap();
    assert!(!held(&n,"s3",per as u64-2)&&held(&n,"vic",0));
}

/// N4 (3) with PR6 parking and the sender quota. At the gas bound a valid transaction is parked (PR6); when the parked
/// set is full and PR6's rules make no room, the last parked transaction of another sender with the lowest effective
/// tip makes room for one paying strictly more; a lower payer is refused (POOL_FULL_GAS). A parked transaction is
/// replaced by fee in place. The sender quota still holds for replacements.
#[test] fn parking_quota_and_fee_eviction_of_the_parked_set(){
    use node::{POOL_FULL_GAS,POOL_FULL_SENDER,POOL_PARKED,POOL_REPLACE_UNDERPRICED};
    let c=cfg(&["carol","erin","frank","gina","hal","ivy"]);let (mut n,_d)=node_with("parked",Limits::default(),&c);let gl=n.chain.spec.cp().GAS_LIMIT;
    let to=Address::with_last_byte(0xbb);
    // alice holds her whole quota (GAS_LIMIT) in two transactions; with erin's, frank's and hal's the gas bound is full.
    for k in 0..2{n.submit_tx(tx(&key("alice"),k,100*GWEI,GWEI,gl/2,to),false).unwrap();}
    for l in ["erin","frank","hal"]{n.submit_tx(tx(&key(l),0,100*GWEI,GWEI,gl,to),false).unwrap();}assert_eq!(n.pool.gas,n.pool.gas_max,"the gas bound is full");
    n.set_pool_parked_max(2);
    let (b0,c0)=(pay(&key("bob"),0,GWEI),pay(&key("carol"),0,2*GWEI));
    assert_eq!(n.submit_tx(b0.clone(),false).unwrap_err(),POOL_PARKED);assert_eq!(n.submit_tx(c0.clone(),false).unwrap_err(),POOL_PARKED);
    // The parked set is full; ivy at 1 gwei pays no more than bob's: refused. gina at 5 gwei evicts bob's (the lowest).
    assert_eq!(n.submit_tx(pay(&key("ivy"),0,GWEI),false).unwrap_err(),POOL_FULL_GAS);
    let g0=pay(&key("gina"),0,5*GWEI);assert_eq!(n.submit_tx(g0.clone(),false).unwrap_err(),POOL_PARKED);
    assert_eq!(n.pool_parked(),vec![c0.clone(),g0.clone()]);assert_eq!(n.stats.parked_fee_evicted,1);n.pool_audit().unwrap();
    // A parked transaction is replaced by fee in place (the replacement is answered with its hash); underpriced is refused.
    let e=n.submit_tx(tx(&key("carol"),0,100*GWEI,2*GWEI+1,21000,to),false).unwrap_err();assert!(e.starts_with(POOL_REPLACE_UNDERPRICED),"{e}");
    let c1=tx(&key("carol"),0,110*GWEI,3*GWEI,21000,to);n.submit_tx(c1.clone(),false).unwrap();assert_eq!(n.pool_parked(),vec![c1.clone(),g0.clone()]);n.pool_audit().unwrap();
    // The sender quota (GAS_LIMIT): alice's replacement may not declare more than her quota allows.
    assert_eq!(n.submit_tx(tx(&key("alice"),0,110*GWEI,2*GWEI,gl/2+1,to),false).unwrap_err(),POOL_FULL_SENDER);
    // And the gas bound: the same declared gas replaces (the bound is full, nothing grows).
    n.submit_tx(tx(&key("alice"),0,110*GWEI,2*GWEI,gl/2,to),false).unwrap();assert_eq!(n.pool.gas,n.pool.gas_max);n.pool_audit().unwrap();
    // The selection: the highest tips first among pooled and parked (each of the four GAS_LIMIT ones needs a block).
    let sel=n.template_selection(&head(&n),pocold::node::now_s());assert!(sel.txs.contains(&c1)&&sel.txs.contains(&g0),"the parked ones are candidates");
}

/// N4 review: the parked set's fee eviction is planned before anything is evicted. With three parked (bob and carol
/// at 1 gwei, erin at 9 gwei) and the bound lowered to one, a 5 gwei newcomer needs three places but only two parked
/// ones rank below it: it is refused (POOL_FULL_GAS) and all three stay. With one place needed, the lowest-ranked
/// latest one goes. A parked transaction ranks at the lowest tip of its sender's pooled ones too: frank's parked 9
/// gwei, behind his pooled 1 gwei ones, ranks at 1 gwei and goes for a 2 gwei newcomer.
#[test] fn the_parked_fee_eviction_evicts_only_when_it_frees_the_room(){
    use node::{POOL_FULL_GAS,POOL_PARKED};
    let c=cfg(&["carol","erin","frank","gina","hal","ivy","kim","lee"]);let (mut n,_d)=node_with("parkplan",Limits::default(),&c);let gl=n.chain.spec.cp().GAS_LIMIT;
    let to=Address::with_last_byte(0xbb);
    for k in 0..2{n.submit_tx(tx(&key("alice"),k,100*GWEI,GWEI,gl/2,to),false).unwrap();}
    for l in ["hal","ivy"]{n.submit_tx(tx(&key(l),0,100*GWEI,GWEI,gl,to),false).unwrap();}
    n.submit_tx(tx(&key("frank"),0,100*GWEI,GWEI,gl/2,to),false).unwrap();n.submit_tx(tx(&key("frank"),1,100*GWEI,GWEI,gl/2-21000,to),false).unwrap();
    n.submit_tx(pay(&key("kim"),0,GWEI),false).unwrap();assert_eq!(n.pool.gas,n.pool.gas_max,"the gas bound is full");
    let parked=[pay(&key("bob"),0,GWEI),pay(&key("carol"),0,GWEI),pay(&key("erin"),0,9*GWEI)];
    for t in &parked{assert_eq!(n.submit_tx(t.clone(),false).unwrap_err(),POOL_PARKED);}
    n.set_pool_parked_max(1);let before=n.pool_parked();
    assert_eq!(n.submit_tx(pay(&key("gina"),0,5*GWEI),false).unwrap_err(),POOL_FULL_GAS);
    assert_eq!((n.pool_parked(),n.stats.parked_fee_evicted),(before,0),"nothing evicted when the room cannot be made");
    // One place needed: the latest of the lowest rank (carol's) goes for gina's.
    n.set_pool_parked_max(3);n.pool_audit().unwrap();let g0=pay(&key("gina"),0,5*GWEI);assert_eq!(n.submit_tx(g0.clone(),false).unwrap_err(),POOL_PARKED);
    let p=n.pool_parked();assert_eq!((p.len(),n.stats.parked_fee_evicted),(3,1));assert!(!p.contains(&parked[1])&&p.contains(&g0));
    // frank's parked nonce 2 at 9 gwei ranks at his pooled 1 gwei: it goes (the latest of bob's rank) for lee's 2 gwei.
    n.set_pool_parked_max(4);let f2=pay(&key("frank"),2,9*GWEI);assert_eq!(n.submit_tx(f2.clone(),false).unwrap_err(),POOL_PARKED);
    let l0=pay(&key("lee"),0,2*GWEI);assert_eq!(n.submit_tx(l0.clone(),false).unwrap_err(),POOL_PARKED);
    let p=n.pool_parked();assert_eq!((p.len(),n.stats.parked_fee_evicted),(4,2));assert!(!p.contains(&f2)&&p.contains(&parked[0])&&p.contains(&l0));
    n.pool_audit().unwrap();
}

/// N4 (V3-24): eth_estimateGas models the log caps active in the next block. On spec-1 (32 KiB / 546 per block) a call
/// whose one LOG0 carries 32 KiB of data (36 + 32,768 charged bytes) halts in every block, so it gets no estimate
/// (-32000, LogBytes) while one of 1 KiB does; eth_call is not an estimate and still runs it. The pool refuses the
/// former the same way.
#[test] fn estimate_gas_models_the_spec1_log_caps(){
    let c=cfg(&[]);let (mut n,_d)=node_with("estimate",Limits::default(),&c);let mut h=net_with(&c);let alice=key("alice");
    // PUSH3 size; PUSH1 0; LOG0; STOP, deployed by a plain init code.
    let runtime=|size:u32|vec![0x62,(size>>16) as u8,(size>>8) as u8,size as u8,0x60,0,0xa0,0];
    let init=|r:&[u8]|{let len=r.len() as u8;let mut b=vec![0x60,len,0x60,12,0x60,0,0x39,0x60,len,0x60,0,0xf3];b.extend_from_slice(r);b};
    let deploy=|nonce:u64,size:u32|Tx{chain_id:CHAIN,nonce,max_priority:U256::from(GWEI),max_fee:U256::from(100*GWEI),gas:300_000,to:None,value:U256::ZERO,data:init(&runtime(size)).into()}.sign_1559(&alice);
    let (big,small)=(deploy(0,32768),deploy(1,1024));for t in [&big,&small]{n.submit_tx(t.clone(),false).unwrap();}
    let blk=signed(&h,h.chain.head,vec![big,small]);import(&mut h,&blk);assert!(n.import(blk,None).unwrap());n.pool_settle();
    let (to_big,to_small)=(alice.address().create(0),alice.address().create(1));
    let est=|n:&mut Node,to:Address|rpc(n,"eth_estimateGas",serde_json::json!([{"from":alice.address(),"to":to},"latest"]));
    let r=est(&mut n,to_big);assert_eq!(r["error"]["code"],-32000,"{r}");assert!(r["error"]["message"].as_str().unwrap().contains("LogBytes"),"{r}");assert_eq!(r["error"]["data"]["reason"],"LogBytes");
    let r=est(&mut n,to_small);assert!(r.get("error").is_none(),"{r}");
    let call=rpc(&mut n,"eth_call",serde_json::json!([{"from":alice.address(),"to":to_big},"latest"]));assert!(call.get("error").is_none(),"{call}");
    let e=n.submit_tx(Tx{chain_id:CHAIN,nonce:2,max_priority:U256::from(GWEI),max_fee:U256::from(100*GWEI),gas:400_000,to:Some(to_big),value:U256::ZERO,data:Default::default()}.sign_1559(&alice),false).unwrap_err();
    assert!(e.contains("LogBytes"),"{e}");
    // N4 review: a pooled replacement passes the same execution checks (run on the head, not committed). A plain
    // transfer at nonce 2 is pooled; replacing it with the call that halts on its own logs is refused, nothing changes.
    let plain=pay(&alice,2,GWEI);n.submit_tx(plain.clone(),false).unwrap();
    let e=n.submit_tx(Tx{chain_id:CHAIN,nonce:2,max_priority:U256::from(2*GWEI),max_fee:U256::from(110*GWEI),gas:400_000,to:Some(to_big),value:U256::ZERO,data:Default::default()}.sign_1559(&alice),false).unwrap_err();
    assert!(e.contains("LogBytes"),"{e}");assert_eq!((n.pool.txs[0].1.clone(),n.stats.pool_replaced),(plain,0));n.pool_audit().unwrap();
}

/// N4 second review (finding 1): a pooled entry is charged the most gas it or a replacement of it declared since the
/// last head (`Pool::gas` and the sender quota), so replacing up and down cannot charge the same gas twice. Before the
/// fix one sender with two 21000-gas transfers replaced each up to its quota and back (4 replacements each, within
/// every replacement limit and the quota) and left `gas` at 31,874,000 of the 32,000,000 bound until the next head,
/// with 42,000 declared: another sender's 200k-gas call was parked and an honest replacement raising its gas refused.
/// Now the second entry's raise is refused by the quota and the sender holds at most its quota of the gas bound.
#[test] fn replacement_cycles_charge_the_most_declared_gas_once(){
    use node::{POOL_FULL_SENDER};
    let c=cfg(&["carol"]);let (mut n,_d)=node_with("rrgas",Limits::default(),&c);let mut h=net_with(&c);
    let to=Address::with_last_byte(0xbb);let (q,gmax)=(n.pool.sender_gas_max,n.pool.gas_max);assert_eq!((q,gmax),(8_000_000,32_000_000));
    let f=|k:u32|(100*GWEI*6u64.pow(k)/5u64.pow(k),GWEI*6u64.pow(k)/5u64.pow(k));
    let alice=key("alice");let t=|nonce:u64,j:u32,gas:u64|{let (c,p)=f(j);tx(&alice,nonce,c,p,gas,to)};
    n.submit_tx(t(0,0,21000),false).unwrap();n.submit_tx(t(1,0,21000),false).unwrap();
    // The first entry: up to the quota, down, up, down. It stays charged q - 21000 (its most), nothing goes to ghost.
    for (j,g) in [(1,q-21000),(2,21000),(3,q-21000),(4,21000)]{n.submit_tx(t(0,j,g),false).unwrap();n.pool_audit().unwrap();
        assert_eq!((n.pool.gas,n.pool.ghost_gas),(q,0),"charged once: {j}");}
    assert_eq!(node::tx_fields(&n.pool.txs[0].1).unwrap().gas,21000,"the pooled bytes declare 21000");
    // The second entry cannot raise: the sender's quota is held by the first one's charge until the next head.
    assert_eq!(n.submit_tx(t(1,1,q-21000),false).unwrap_err(),POOL_FULL_SENDER);
    n.pool_audit().unwrap();assert!(n.pool.gas<=42_000+q,"a sender holds at most its quota of the gas bound: {}",n.pool.gas);
    // Another sender's ordinary call and an honest replacement raising its gas are pooled.
    n.submit_tx(tx(&key("carol"),0,100*GWEI,10*GWEI,200_000,to),false).unwrap();
    let bob=key("bob");n.submit_tx(tx(&bob,0,100*GWEI,GWEI,21000,to),false).unwrap();
    n.submit_tx(tx(&bob,0,120*GWEI,2*GWEI,300_000,to),false).unwrap();n.pool_audit().unwrap();
    assert_eq!(n.pool.gas,q+200_000+300_000);
    // Raising back to the most it declared charges nothing more; the overdraft rule reads the current declaration.
    n.submit_tx(tx(&bob,0,150*GWEI,3*GWEI,21000,to),false).unwrap();n.submit_tx(tx(&bob,0,180*GWEI,4*GWEI,300_000,to),false).unwrap();
    assert_eq!(n.pool.gas,q+500_000);n.pool_audit().unwrap();
    // The selection takes the replacements at their declared gas; the next head recounts the declared gas.
    let sel=n.template_selection(&head(&n),pocold::node::now_s());assert_eq!(sel.txs.len(),4);
    let fb=fallback(&h,h.chain.head);import(&mut h,&fb);n.import(fb,None).unwrap();n.pool_settle();
    assert_eq!((n.pool.gas,n.pool.ghost_gas),(42_000+200_000+300_000,0));n.pool_audit().unwrap();
}

/// N4 second review (finding 2): a transaction a new head revalidates (returned from an abandoned block, or left in
/// the previous pool) keeps the pending-state check alone, so it can depend on another sender's pending funds. The
/// fee order ranks a candidate its sender's head balance does not cover at 0, so it comes after its funding (pool
/// order) and the block takes both, as the pool order did before N4. Before the fix newbie's 10 gwei transfer was
/// tried first, failed on the parent and was skipped with its sender. A replacement of the funding can still leave it
/// unfunded (the overdraft guarantee covers new submissions only): it is then tried last and skipped.
#[test] fn a_revalidated_dependent_is_ordered_after_its_funding(){
    let c=cfg(&[]);let (mut n,_d)=node_with("rr1",Limits::default(),&c);let mut h=net_with(&c);
    let (alice,newbie)=(key("alice"),key("n4-newbie"));
    let fund=Tx{chain_id:CHAIN,nonce:0,max_priority:U256::from(GWEI),max_fee:U256::from(100*GWEI),gas:21000,to:Some(newbie.address()),value:U256::from(10u128.pow(18)),data:Default::default()}.sign_1559(&alice);
    let nb=pay(&newbie,0,10*GWEI);let b0=pay(&key("bob"),0,5*GWEI);
    // As a new head re-queues an abandoned block's transactions, in block order; bob's is a new submission.
    n.requeue_for_revalidation(fund.clone());n.requeue_for_revalidation(nb.clone());n.submit_tx(b0.clone(),false).unwrap();
    assert_eq!(n.pool.txs.len(),3,"pooled on the pending state");n.pool_audit().unwrap();
    assert_eq!(n.fee_order(base(&n)),vec![2,0,1],"bob 5, alice's funding 1, newbie's uncovered 10 last");
    let sel=n.template_selection(&head(&n),pocold::node::now_s());
    assert_eq!(sel.txs,vec![b0.clone(),fund.clone(),nb.clone()],"the dependent follows its funding");
    let blk=signed(&h,h.chain.head,sel.txs);import(&mut h,&blk);assert!(n.import(blk,None).unwrap(),"valid on the spec-1 devnet");n.pool_settle();
    assert!(n.pool.txs.is_empty());n.pool_audit().unwrap();
    // A replacement of a funding (its own sender is covered) is accepted and may leave the dependent unfunded.
    let fund2=Tx{chain_id:CHAIN,nonce:1,max_priority:U256::from(GWEI),max_fee:U256::from(100*GWEI),gas:21000,to:Some(key("n4-other").address()),value:U256::from(10u128.pow(18)),data:Default::default()}.sign_1559(&alice);
    let other=pay(&key("n4-other"),0,10*GWEI);n.requeue_for_revalidation(fund2);n.requeue_for_revalidation(other.clone());
    let repl=Tx{chain_id:CHAIN,nonce:1,max_priority:U256::from(2*GWEI),max_fee:U256::from(110*GWEI),gas:21000,to:Some(key("n4-other").address()),value:U256::ZERO,data:Default::default()}.sign_1559(&alice);
    n.submit_tx(repl.clone(),false).unwrap();n.pool_audit().unwrap();assert!(n.pool.txs.iter().any(|(_,r)|*r==other),"still pooled, unfunded");
    assert_eq!(n.fee_order(base(&n)),vec![0,1],"the unfunded one ranks last");
    let sel=n.template_selection(&head(&n),pocold::node::now_s());assert_eq!(sel.txs,vec![repl],"tried last, it fails on the parent and is skipped");
}

/// N4 second review (finding 3): the sender quota comes before the eviction's refusals, so the answer does not depend
/// on the count or byte bound: q holds its whole quota (one GAS_LIMIT transaction) and its next one is refused
/// -32016 "pool full (sender gas)" below the bound and at it (before the fix -32000, the overdraft refusal, at it).
#[test] fn the_sender_quota_comes_before_the_eviction_refusals(){
    use node::{POOL_FULL_SENDER,POOL_MAX_TXS};
    let senders:Vec<String>=(0..16).map(|i|format!("s{i}")).collect();let labels:Vec<&str>=senders.iter().map(|s|s.as_str()).collect();
    let mut c=cfg(&labels);let gl=admitted(&c).cp().GAS_LIMIT;
    // q can pay one GAS_LIMIT transfer at 100 gwei (value 1), nothing more.
    c.accounts.push(("rr2-q".into(),U256::from(gl)*U256::from(100*GWEI)+U256::from(1u64)));
    let (mut n,_d)=node_with("rr2",Limits{pool_gas_max:Some(POOL_MAX_TXS as u64*21000+2*gl),pool_sender_gas_max:Some(gl),..Default::default()},&c);
    let q=key("rr2-q");let to=Address::with_last_byte(0xbb);
    n.submit_tx(tx(&q,0,100*GWEI,GWEI,gl,to),false).unwrap();
    let q1=pay(&q,1,5*GWEI);assert_eq!(n.submit_tx(q1.clone(),false).unwrap_err(),POOL_FULL_SENDER,"below the bound");
    let per=POOL_MAX_TXS/senders.len();
    for (i,s) in senders.iter().enumerate(){let k=key(s);for j in 0..per-usize::from(i==15){n.submit_tx(pay(&k,j as u64,2*GWEI),false).unwrap();}}
    assert_eq!(n.pool.txs.len(),POOL_MAX_TXS);n.pool_audit().unwrap();let before=n.stats.clone();
    assert_eq!(n.submit_tx(q1.clone(),false).unwrap_err(),POOL_FULL_SENDER,"at the count bound");
    let r=rpc(&mut n,"eth_sendRawTransaction",serde_json::json!([hex(&q1)]));assert_eq!(r["error"]["code"],-32016,"{r}");
    assert_eq!((n.stats.evict_refused,n.stats.overdraft_refused,n.stats.pool_evicted),(before.evict_refused,before.overdraft_refused,0),"counted as a quota refusal only");
}

/// N4 second review (finding 4, blocking): A13 counts the pool's layer at min(gas bound, POOL_MAX_TXS x GAS_LIMIT)
/// (`resources::pool_gas`). An eviction leaves its victim's writes in the layer, so with a gas bound above that term
/// evictions took the layer past it. The node applies the same minimum as its gas bound (`pool_gas_in_effect`): with
/// the pool at the count bound and its gas at the term, a newcomer that needs gas room is refused -32016 "pool full"
/// and nothing is evicted; with room left, eviction works and `gas` (which bounds the layer) stays within the term.
#[test] fn evictions_keep_the_pending_layer_within_the_a13_term(){
    use node::POOL_MAX_TXS;
    let senders:Vec<String>=(0..16).map(|i|format!("s{i}")).collect();let mut labels:Vec<&str>=senders.iter().map(|s|s.as_str()).collect();labels.push("zed");
    let c=cfg(&labels);let spec=admitted(&c);let gl=spec.cp().GAS_LIMIT;
    let gas_max=POOL_MAX_TXS as u64*gl+4*gl;
    let limits=Limits{pool_gas_max:Some(gas_max),pool_sender_gas_max:Some(gas_max),mem_budget:u64::MAX,..Default::default()};
    let counted=pocold::resources::pool_gas(spec.cp(),&limits).unwrap();assert_eq!(counted,POOL_MAX_TXS as u128*u128::from(gl),"A13 counts the count term");
    let (mut n,_d)=node_with("rrmem",limits,&c);assert_eq!(u128::from(n.pool.gas_max),counted,"the bound in effect is A13's term");
    let to=Address::with_last_byte(0xbb);let per=POOL_MAX_TXS/senders.len();
    // All but the last transaction declare GAS_LIMIT; the last 21000, so gas room is left for 21000-gas newcomers.
    for (i,s) in senders.iter().enumerate(){let k=key(s);for j in 0..per{let g=if i==15&&j==per-1{21000}else{gl};n.submit_tx(tx(&k,j as u64,100*GWEI,GWEI,g,to),false).unwrap();}}
    assert_eq!(n.pool.txs.len(),POOL_MAX_TXS);n.pool_audit().unwrap();
    let z=key("zed");
    // A GAS_LIMIT newcomer does not fit the term: refused, nothing evicted.
    assert_eq!(n.submit_tx(tx(&z,0,100*GWEI,3*GWEI,gl,to),false).unwrap_err(),"-32016 pool full");assert_eq!(n.stats.pool_evicted,0);
    // 21000-gas newcomers evict (each victim's GAS_LIMIT stays counted as ghost gas) while the term has room.
    for j in 0..3{n.submit_tx(tx(&z,j,100*GWEI,3*GWEI,21000,to),false).unwrap();n.pool_audit().unwrap();
        assert!(u128::from(n.pool.gas)<=counted,"the layer stays within A13's term: {} > {counted}",n.pool.gas);}
    assert_eq!(n.stats.pool_evicted,3);assert_eq!(n.pool.txs.len(),POOL_MAX_TXS);
    assert!(u128::from(n.pool.gas)+u128::from(gl)>counted,"no room for one more GAS_LIMIT execution");
    assert_eq!(n.submit_tx(tx(&z,3,100*GWEI,3*GWEI,gl,to),false).unwrap_err(),"-32016 pool full");assert_eq!(n.stats.pool_evicted,3);
}

/// N4 second review (finding 5): the replacement bounds. At a full gas bound a pooled replacement declaring more gas
/// is refused POOL_FULL_GAS (and declaring its most again replaces); nothing changes on a refusal.
#[test] fn a_pooled_replacement_declaring_more_gas_respects_the_gas_bound(){
    use node::POOL_FULL_GAS;
    let c=cfg(&["erin","frank","hal","kim"]);let (mut n,_d)=node_with("rgas",Limits::default(),&c);let gl=n.chain.spec.cp().GAS_LIMIT;
    let to=Address::with_last_byte(0xbb);
    let k0=pay(&key("kim"),0,GWEI);n.submit_tx(k0.clone(),false).unwrap();
    for j in 0..2{n.submit_tx(tx(&key("alice"),j,100*GWEI,GWEI,gl/2,to),false).unwrap();}
    for l in ["erin","frank"]{n.submit_tx(tx(&key(l),0,100*GWEI,GWEI,gl,to),false).unwrap();}
    n.submit_tx(tx(&key("hal"),0,100*GWEI,GWEI,gl-21000,to),false).unwrap();assert_eq!(n.pool.gas,n.pool.gas_max,"the gas bound is full");
    assert_eq!(n.submit_tx(tx(&key("kim"),0,110*GWEI,2*GWEI,42_000,to),false).unwrap_err(),POOL_FULL_GAS,"within kim's quota, past the gas bound");
    assert_eq!((n.pool.txs[0].1.clone(),n.pool.gas,n.stats.pool_replaced),(k0,n.pool.gas_max,0));n.pool_audit().unwrap();
    n.submit_tx(tx(&key("kim"),0,110*GWEI,2*GWEI,21000,to),false).unwrap();assert_eq!(n.pool.gas,n.pool.gas_max);n.pool_audit().unwrap();
}

/// N4 second review (finding 5): at the count bound with no gas room a newcomer is refused -32016 "pool full", not
/// parked and evicting nothing; a newcomer that fails execution (gas below the intrinsic 21000) evicts nothing.
#[test] fn the_count_bound_refuses_without_gas_room_and_an_invalid_newcomer_evicts_nothing(){
    use node::POOL_MAX_TXS;
    let senders:Vec<String>=(0..16).map(|i|format!("s{i}")).collect();let mut labels:Vec<&str>=senders.iter().map(|s|s.as_str()).collect();labels.push("zed");
    let c=cfg(&labels);
    let (mut n,_d)=node_with("cbound",Limits{pool_gas_max:Some(POOL_MAX_TXS as u64*21000+21000),..Default::default()},&c);
    let per=POOL_MAX_TXS/senders.len();
    for s in &senders{let k=key(s);for j in 0..per{n.submit_tx(pay(&k,j as u64,GWEI),false).unwrap();}}
    assert_eq!(n.pool.txs.len(),POOL_MAX_TXS);n.pool_audit().unwrap();let z=key("zed");let to=Address::with_last_byte(0xbb);
    // Gas room for 21000: an invalid newcomer (gas 20999) passes the cheap checks, fails execution and evicts nothing.
    let e=n.submit_tx(tx(&z,0,100*GWEI,3*GWEI,20_999,to),false).unwrap_err();assert!(e.starts_with("-32000"),"{e}");
    assert_eq!((n.pool.txs.len(),n.pool.ghost_gas,n.stats.pool_evicted),(POOL_MAX_TXS,0,0),"nothing evicted");n.pool_audit().unwrap();
    // A valid one evicts; then there is no gas room left (the victim's gas stays counted): refused, not parked.
    n.submit_tx(tx(&z,0,100*GWEI,3*GWEI,21000,to),false).unwrap();assert_eq!(n.stats.pool_evicted,1);
    assert_eq!(n.submit_tx(tx(&z,1,100*GWEI,4*GWEI,21000,to),false).unwrap_err(),"-32016 pool full");
    assert!(n.pool_parked().is_empty(),"not parked");assert_eq!(n.stats.pool_evicted,1);n.pool_audit().unwrap();
}

/// N4 second review (finding 5): a parked newcomer ranks at its sender's chain (its pooled 1 gwei one), not its own 9
/// gwei, so it cannot evict a parked 2 gwei transaction; a newcomer with nothing pooled at 9 gwei can.
#[test] fn a_parked_newcomer_ranks_at_its_chain_for_the_parked_fee_eviction(){
    use node::{POOL_FULL_GAS,POOL_PARKED};
    let c=cfg(&["erin","frank","gina","hal","ivy"]);let (mut n,_d)=node_with("parkchain",Limits::default(),&c);let gl=n.chain.spec.cp().GAS_LIMIT;
    let to=Address::with_last_byte(0xbb);
    for j in 0..2{n.submit_tx(tx(&key("alice"),j,100*GWEI,GWEI,gl/2,to),false).unwrap();}
    for l in ["erin","hal"]{n.submit_tx(tx(&key(l),0,100*GWEI,GWEI,gl,to),false).unwrap();}
    n.submit_tx(pay(&key("frank"),0,GWEI),false).unwrap();n.submit_tx(tx(&key("ivy"),0,100*GWEI,GWEI,gl-21000,to),false).unwrap();
    assert_eq!(n.pool.gas,n.pool.gas_max,"the gas bound is full");
    n.set_pool_parked_max(1);let b0=pay(&key("bob"),0,2*GWEI);assert_eq!(n.submit_tx(b0.clone(),false).unwrap_err(),POOL_PARKED);
    assert_eq!(n.submit_tx(pay(&key("frank"),1,9*GWEI),false).unwrap_err(),POOL_FULL_GAS,"ranks at frank's pooled 1 gwei");
    assert_eq!((n.pool_parked(),n.stats.parked_fee_evicted),(vec![b0.clone()],0));n.pool_audit().unwrap();
    let g0=pay(&key("gina"),0,9*GWEI);assert_eq!(n.submit_tx(g0.clone(),false).unwrap_err(),POOL_PARKED);
    assert_eq!((n.pool_parked(),n.stats.parked_fee_evicted),(vec![g0],1));n.pool_audit().unwrap();
}

/// N4 second review (finding 5): the byte bound. With the pool's bytes near POOL_MAX_BYTES (large calldata
/// transactions): a pooled replacement whose bytes would pass it is refused -32016 "pool full"; a newcomer at the byte
/// bound skips the lowest-ranked tail (vic's small 1 gwei one) when it is smaller than the bytes needed and evicts
/// the lowest-ranked tail large enough (the latest 2 gwei one); the pool's bytes stay within the bound.
#[test] fn the_byte_bound_evicts_a_victim_large_enough_and_holds_for_replacements(){
    use node::POOL_MAX_BYTES;
    let senders:Vec<String>=(0..16).map(|i|format!("s{i}")).collect();let mut labels:Vec<&str>=senders.iter().map(|s|s.as_str()).collect();labels.extend(["zed","vic","pad"]);
    let c=cfg(&labels);let spec=admitted(&c);let tx_max=spec.cp().TX_MAX as usize;
    let limits=Limits{pool_gas_max:Some(2_000_000_000),pool_sender_gas_max:Some(2_000_000_000),mem_budget:u64::MAX,..Default::default()};
    let (mut n,_d)=node_with("bytes",limits,&c);let to=Address::with_last_byte(0xbb);
    let data=|k:&Key,nonce:u64,cap:u64,tip:u64,len:usize|Tx{chain_id:CHAIN,nonce,max_priority:U256::from(tip),max_fee:U256::from(cap),gas:21000+16*len as u64,to:Some(to),value:U256::from(1u64),data:vec![0u8;len].into()}.sign_1559(k);
    let len=tx_max-512;
    let (vic,alice)=(pay(&key("vic"),0,GWEI),pay(&key("alice"),0,5*GWEI));n.submit_tx(vic.clone(),false).unwrap();n.submit_tx(alice.clone(),false).unwrap();
    // Fill with large transactions at 2 gwei, round robin over the senders, while the next one fits.
    let mut count=0usize;
    loop{let raw=data(&key(&senders[count%16]),(count/16) as u64,100*GWEI,2*GWEI,len);if n.pool.bytes+raw.len()>POOL_MAX_BYTES{break}n.submit_tx(raw,false).unwrap();count+=1;}
    // The latest large tail (all rank at 2 gwei; ties take the latest).
    let victim=n.pool.txs.last().unwrap().1.clone();assert!(count>16&&victim.len()>=len);
    // Then pads (5 gwei) that leave between about 500 and 1,000 bytes of room.
    let mut p=0;while POOL_MAX_BYTES-n.pool.bytes>1000{let r=POOL_MAX_BYTES-n.pool.bytes;n.submit_tx(data(&key("pad"),p,100*GWEI,5*GWEI,(r-700).min(len)),false).unwrap();p+=1;}
    let room=POOL_MAX_BYTES-n.pool.bytes;assert!(room<=1000&&room>vic.len(),"room {room}");n.pool_audit().unwrap();
    // A replacement whose bytes pass the bound is refused; nothing changes.
    let bigger=data(&key("alice"),0,110*GWEI,6*GWEI,len);
    assert_eq!(n.submit_tx(bigger,false).unwrap_err(),"-32016 pool full");assert!(n.pool.txs.iter().any(|(_,r)|*r==alice));assert_eq!(n.stats.pool_replaced,0);
    // zed's needs more bytes than vic's (the lowest rank) holds: the latest 2 gwei tail goes.
    let z=data(&key("zed"),0,100*GWEI,3*GWEI,len-300);assert!(z.len()-room>vic.len());
    n.submit_tx(z.clone(),false).unwrap();n.pool_audit().unwrap();
    assert_eq!(n.stats.pool_evicted,1);assert!(n.pool.txs.iter().any(|(_,r)|*r==vic),"vic's is too small to make room: kept");
    assert!(!n.pool.txs.iter().any(|(_,r)|*r==victim),"the latest large 2 gwei tail is evicted");
    assert!(n.pool.bytes<=POOL_MAX_BYTES,"{} bytes",n.pool.bytes);
}
