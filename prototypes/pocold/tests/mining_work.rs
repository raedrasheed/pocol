//! V3-30 (D-V3-30, node-local): `--mine-threads N` (disjoint slices of the own range), the localhost work interface
//! (pocol_getWork / pocol_submitWork, off by default, own range only) and networkHashrateEst.
use pocold::{chain::{self,Chain,Spec},node::{Config,Limits,Node},rpc,types::*};
use pocol_lp4_execution::{devnet::{self,DevnetConfig},txsign::Key};
use serde_json::{json,Value};

/// About 2^12 hashes per block, genesis a few seconds ago so a template built now is inside its window.
fn cfg()->DevnetConfig{let mut c=DevnetConfig::new(777910,pocold::node::now_s()-10,&["a","b","c"]);c.hash_rate=1024;c.d_att=73;c.tau=40;c.c=8;c}
fn admitted(c:&DevnetConfig)->Spec{let d=devnet::build(c).unwrap();Spec::new(d.admit(&pocold::single_host_local()).unwrap_or_else(|e|panic!("{e}")).0)}
fn dir(tag:&str)->std::path::PathBuf{let d=std::env::temp_dir().join(format!("pocold-work-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&d);d}
/// The drawn proposer of height 1's key, and the other members' keys.
fn keys(c:&DevnetConfig)->(Key,Vec<Key>){
    let d=devnet::build(c).unwrap();let ch=Chain::new(admitted(c));let g=ch.full_entry(&ch.head).unwrap();let prop=ch.child_ctx(&g,0).unwrap().proposer;
    let mut others=vec![];let mut me=None;
    for m in &d.miners{let k=Key::from_secret(&m.key.secret()).unwrap();if m.id[..]==prop[..]{me=Some(k)}else{others.push(k)}}
    (me.unwrap(),others)
}
fn node(tag:&str,c:&DevnetConfig,miner:Option<Key>,limits:Limits)->(Node,std::path::PathBuf){
    let d=dir(tag);(Node::open(Config{dir:d.clone(),name:tag.into(),miner,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits},admitted(c)).unwrap(),d)
}
fn call(n:&mut Node,m:&str,p:Value)->Value{serde_json::from_str(&rpc::handle(n,json!({"jsonrpc":"2.0","id":1,"method":m,"params":p}).to_string().as_bytes())).unwrap()}
fn q(v:&Value)->u64{u64::from_str_radix(v.as_str().unwrap().trim_start_matches("0x"),16).unwrap()}
fn hex32(v:&Value)->[u8;32]{<[u8;32]>::try_from(unhex(v.as_str().unwrap()).unwrap().as_slice()).unwrap()}
fn fixed(v:&Value)->Fixed{Fixed::from_be_slice(&unhex(v.as_str().unwrap()).unwrap()).unwrap()}
fn work_on()->Limits{Limits{work_rpc:true,..Default::default()}}

/// The work interface is off by default (an unknown method), never served to a request with an Origin header or
/// from a peer that is not loopback, alone or in a batch, and the peer check accepts loopback addresses only.
#[test] fn get_work_is_off_by_default_and_refused_from_sites_and_non_loopback_peers(){
    let c=cfg();let (me,_)=keys(&c);
    let (mut off,d1)=node("off",&c,Some(Key::from_secret(&me.secret()).unwrap()),Limits::default());
    for m in ["pocol_getWork","pocol_submitWork"]{let r=call(&mut off,m,json!([]));assert_eq!(r["error"]["code"],-32601,"{m} off by default: {r}");}
    assert_eq!(off.status_json()["mining"]["workRpc"],false);
    let (mut on,d2)=node("on",&c,Some(me),work_on());
    let body=br#"{"jsonrpc":"2.0","id":7,"method":"pocol_getWork","params":[]}"#;
    let site:Value=serde_json::from_str(&rpc::handle_with(&mut on,body,true)).unwrap();
    assert_eq!((site["error"]["code"].clone(),site["error"]["message"].clone()),(json!(-32601),json!("method not available to sites")),"{site}");
    let mut b=rpc::Budget{compute_by:std::time::Instant::now()+std::time::Duration::from_secs(30),scratch_max:usize::MAX,scratch_used:0,walled:false};
    let remote:Value=serde_json::from_str(&rpc::handle_from(&mut on,body,false,false,None,None,&mut b)).unwrap();
    assert_eq!((remote["error"]["code"].clone(),remote["error"]["message"].clone()),(json!(-32601),json!("method available to loopback callers only")),"{remote}");
    let batch=br#"[{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]},{"jsonrpc":"2.0","id":2,"method":"pocol_submitWork","params":["0x00",1]}]"#;
    let r:Value=serde_json::from_str(&rpc::handle_from(&mut on,batch,false,false,None,None,&mut b)).unwrap();
    assert!(r[0]["result"].is_string(),"{r}");assert_eq!(r[1]["error"]["code"],-32601,"{r}");
    let loopback:Value=serde_json::from_str(&rpc::handle_from(&mut on,body,false,true,None,None,&mut b)).unwrap();
    assert_eq!(loopback["error"]["code"],-32010,"served to loopback (no template locked yet): {loopback}");
    for (ip,ok) in [("127.0.0.1",true),("127.3.4.5",true),("::1",true),("::ffff:127.0.0.1",true),("10.0.0.1",false),("192.168.1.2",false),("::",false),("2001:db8::1",false),("::ffff:10.0.0.1",false)]{
        assert_eq!(rpc::work_peer_ok(Some(ip.parse().unwrap())),ok,"{ip}");
    }
    assert!(!rpc::work_peer_ok(None),"an unknown peer address is refused");
    assert!(rpc::ORIGIN_DENY.contains(&"pocol_getWork")&&rpc::ORIGIN_DENY.contains(&"pocol_submitWork"));
    drop((off,on));let _=std::fs::remove_dir_all(d1);let _=std::fs::remove_dir_all(d2);
}
/// getWork hands out the own range and disjoint slices of it; submitWork accepts a share and the solution of the own
/// range for the locked template and refuses stale work, a nonce of another member's range and a hash that is not
/// a share. The solution is sealed and imported, after which the old template's work is stale.
#[test] fn submit_work_accepts_the_own_range_and_refuses_foreign_and_stale_work(){
    let c=cfg();let (me,others)=keys(&c);let addr=me.address();
    let (mut n,d)=node("submit",&c,Some(me),Limits{mine_threads:3,..work_on()});
    n.start_production();
    let w=call(&mut n,"pocol_getWork",json!([]));let w=&w["result"];assert!(w.is_object(),"locked: {w}");
    let tid=hex32(&w["templateId"]);let (s,e)=(q(&w["rangeStart"]),q(&w["rangeEnd"]));
    assert_eq!((q(&w["sliceStart"]),q(&w["sliceEnd"])),(s,e),"no slice asked: the whole own range");
    assert_eq!(w["height"],1);assert_eq!(w["powHash"],"sha256(templateId || be64(nonce))");
    // The node's own range of the locked template, from the assignment itself.
    let ctx={let g=n.chain.full_entry(&n.chain.head).unwrap();n.chain.child_ctx(&g,0).unwrap()};
    assert_eq!(chain::assignment_range(&ctx,addr),Some((s,e)));
    let foreign=others.iter().find_map(|k|chain::assignment_range(&ctx,k.address())).expect("another member's range").0;
    // Four slices: disjoint, in order, covering the own range.
    let sl:Vec<(u64,u64)>=(0..4).map(|i|{let v=call(&mut n,"pocol_getWork",json!([i,4]));(q(&v["result"]["sliceStart"]),q(&v["result"]["sliceEnd"]))}).collect();
    assert_eq!(sl,pocold::miner::slices(s,e,4));assert_eq!((sl[0].0,sl[3].1),(s,e));
    for i in 1..4{assert_eq!(sl[i].0,sl[i-1].1+1);}
    for bad in [json!([4,4]),json!([0,0]),json!([0,65537]),json!([1])]{assert_eq!(call(&mut n,"pocol_getWork",bad.clone())["error"]["code"],-32602,"{bad}");}
    // Thread count respected (V3-30 S1).
    let st=n.status_json();assert_eq!((st["mining"]["threads"].as_u64(),st["mining"]["running"].as_u64()),(Some(3),Some(3)),"three threads run the locked template: {}",st["mining"]);
    assert!(pocold::miner::THREADS_PEAK.load(std::sync::atomic::Ordering::SeqCst)>=3);
    let (target,t_share)=(fixed(&w["target"]),fixed(&w["shareTarget"]));
    let (mut share,mut solution,mut miss)=(None,None,None);
    for nonce in s..=e{
        let h=pow_hash(&tid,nonce);
        if le_target(&h,&target){solution.get_or_insert(nonce);}else if le_target(&h,&t_share){share.get_or_insert(nonce);}else{miss.get_or_insert(nonce);}
        if share.is_some()&&solution.is_some()&&miss.is_some(){break}
    }
    let (share,solution,miss)=(share.unwrap(),solution.unwrap(),miss.unwrap());
    let sub=|n:&mut Node,t:&[u8;32],nonce:u64|call(n,"pocol_submitWork",json!([hex(t),format!("0x{nonce:x}")]));
    assert_eq!(sub(&mut n,&[0x55;32],share)["error"]["code"],-32010,"another TemplateID is stale work");
    assert_eq!(sub(&mut n,&tid,foreign)["error"]["code"],-32011,"another member's nonce");
    if e<u64::MAX{assert_eq!(sub(&mut n,&tid,e+1)["error"]["code"],-32011,"just past the own range");}
    if s>0{assert_eq!(sub(&mut n,&tid,s-1)["error"]["code"],-32011,"just before the own range");}
    assert_eq!(sub(&mut n,&tid,miss)["error"]["code"],-32012,"not a share");
    assert_eq!(call(&mut n,"pocol_submitWork",json!(["0x12",share]))["error"]["code"],-32602,"a malformed TemplateID");
    let r=sub(&mut n,&tid,share);assert_eq!(r["result"],json!({"accepted":true,"kind":"share"}),"{r}");
    let r=sub(&mut n,&tid,share);assert_eq!(r["result"],json!({"accepted":false,"kind":"share","reason":"duplicate"}),"{r}");
    let r=sub(&mut n,&tid,solution);assert_eq!(r["result"],json!({"accepted":true,"kind":"solution","imported":true}),"{r}");
    assert_eq!(n.chain.head().height,1,"the submitted solution is block 1");assert_eq!(n.chain.head().header.nonce,solution);
    assert!(n.chain.head().header.shares.contains(&share),"the submitted share is credited in the block");
    let st=n.status_json()["mining"].clone();
    assert_eq!((st["workShares"].as_u64(),st["workSolutions"].as_u64(),st["workRefused"].as_u64()),(Some(1),Some(1),Some(4+u64::from(e<u64::MAX)+u64::from(s>0))),"stale, foreign, edges, not a share and the duplicate are refused: {st}");
    // After the block the old template is stale (the node now produces on block 1, or waits).
    assert_eq!(sub(&mut n,&tid,share)["error"]["code"],-32010);
    drop(n);let _=std::fs::remove_dir_all(d);
}
/// A node without a mining key has no work; the thread count is checked at open.
#[test] fn no_key_no_work_and_the_thread_count_is_bounded(){
    let c=cfg();
    let (mut n,d)=node("nokey",&c,None,work_on());
    assert_eq!(call(&mut n,"pocol_getWork",json!([]))["error"]["code"],-32010);
    assert_eq!(call(&mut n,"pocol_submitWork",json!([hex(&[1;32]),1]))["error"]["code"],-32010);
    assert_eq!(n.status_json()["mining"]["threads"],1,"one thread by default");
    drop(n);let _=std::fs::remove_dir_all(&d);
    for bad in [0usize,pocold::miner::MINE_THREADS_MAX+1]{
        let d=dir("bad");let r=Node::open(Config{dir:d.clone(),name:"bad".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits{mine_threads:bad,..Default::default()}},admitted(&c));
        assert!(matches!(&r,Err(e) if e.contains("mine threads")),"{bad} threads refused");let _=std::fs::remove_dir_all(&d);
    }
}
/// networkHashrateEst: the window's work over its seconds. Null at genesis; after a block, exactly the block's work
/// over the seconds since genesis; the estimator itself on known points.
#[test] fn the_hashrate_estimate_is_work_over_time(){
    use revm::primitives::U256;use pocold::node::hashrate_estimate;
    assert_eq!(hashrate_estimate(100,U256::ZERO,110,U256::from(1000u64)),Some(U256::from(100u64)));
    assert_eq!(hashrate_estimate(100,U256::from(7u64),103,U256::from(17u64)),Some(U256::from(3u64)),"rounded down");
    assert_eq!(hashrate_estimate(100,U256::ZERO,100,U256::from(1000u64)),None,"no time");
    assert_eq!(hashrate_estimate(100,U256::from(5u64),120,U256::from(5u64)),None,"no work");
    assert_eq!(hashrate_estimate(120,U256::ZERO,100,U256::from(5u64)),None,"time backwards");
    let c=cfg();let (me,_)=keys(&c);
    let (mut n,d)=node("rate",&c,Some(me),work_on());
    assert!(n.status_json()["networkHashrateEst"].is_null(),"no block yet");
    n.start_production();
    let w=call(&mut n,"pocol_getWork",json!([]))["result"].clone();let tid=hex32(&w["templateId"]);let target=fixed(&w["target"]);
    let sol=(q(&w["rangeStart"])..=q(&w["rangeEnd"])).find(|x|le_target(&pow_hash(&tid,*x),&target)).unwrap();
    assert_eq!(call(&mut n,"pocol_submitWork",json!([hex(&tid),sol]))["result"]["imported"],true);
    let est=n.status_json()["networkHashrateEst"].clone();let head=n.chain.head().clone();
    let secs=head.ts-c.g_ts;assert!(secs>0);
    assert_eq!(est["blocks"],1);assert_eq!(est["fromHeight"],0);assert_eq!(est["toHeight"],1);assert_eq!(est["seconds"],secs);assert_eq!(est["window"],360);
    let want=head.total_work/U256::from(secs);assert_eq!(U256::from(est["hashesPerSecond"].as_u64().unwrap()),want,"{est}");
    // Sanity: the work of one block is the expected hashes of one solution, 2^256/(target+1).
    assert_eq!(head.total_work,chain::work(&head.header.target));
    drop(n);let _=std::fs::remove_dir_all(d);
}
