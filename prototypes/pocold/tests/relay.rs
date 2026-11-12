//! P2P gossip of templates and shares (network.md:13 and :15; review DEC-04). A node relays a signed
//! template from Draw(h,0) once, and only after it locked on it; it relays a valid new share for its locked
//! template once, at most 4·S_max per template; it never relays an invalid template or share, and never back
//! to the peer that sent it. Before this a template or a share travelled one hop: a miner reached only
//! through another node mined an empty fallback, and its shares never reached a winner elsewhere.
//!
//! A real node B over loopback (no mining key: it never signs) with raw peers on both sides. Each test has a
//! fresh genesis, so attempt 0 at height 1 is open: window [g+1, g+D_att], and B waits for the signed
//! template until max(start + Δrel, L(0) + dFbWait) (single-host Δrel = 5 s, Φ = 0). B delivers frames to
//! one peer in order, so "the next gossip y receives is X" also shows that nothing sent before X reached y.
use std::collections::HashMap;
use std::io::{Read,Write};
use std::net::{TcpListener,TcpStream};
use std::time::{Duration,Instant};
use pocold::{chain::{Chain,ChildCtx,Spec},hashes,net::Msg,node::{Config,Limits,Node,now_s},types::*};
use pocol_lp4_execution::{admission,devnet::{self,DevnetConfig},txsign::{Key,Tx}};
use revm::primitives::{Address,B256,U256};

/// Each test runs a node whose attempt-0 window is short: one at a time.
static SERIAL:std::sync::Mutex<()>=std::sync::Mutex::new(());
fn serial()->std::sync::MutexGuard<'static,()>{SERIAL.lock().unwrap_or_else(|e|e.into_inner())}
fn frame(m:&Msg)->Vec<u8>{let b=m.encode();let mut f=(b.len() as u32).to_be_bytes().to_vec();f.extend(b);f}

struct Fixture{chain:Chain,keys:HashMap<Address,Key>,p2p:u16,rpc:u16,chain_id:u64,genesis:[u8;32]}
/// A fresh devnet (genesis now, adjusted by `tune`), node B started on it, and a local chain on the same
/// admitted profile (timing measured once, admitted twice: a Spec is not Clone) that builds the drawn
/// proposer's template.
fn fixture(tag:&str,chain_id:u64,tune:impl FnOnce(&mut DevnetConfig))->Fixture{
    let mut c=DevnetConfig::new(chain_id,now_s(),&["a","b","c"]);tune(&mut c);
    let d=devnet::build(&c).unwrap();
    let local=pocold::single_host_local();
    let (admitted,timing)=d.admit(&local).unwrap_or_else(|e|panic!("{e}"));
    let spec_b=Spec::new(admitted);
    let spec_local=Spec::new(admission::admit(&d.request(&local,Some(&timing))).unwrap_or_else(|e|panic!("{e}")));
    let mut keys=HashMap::new();for m in &d.miners{keys.insert(m.id,Key::from_secret(&m.key.secret()).unwrap());}
    // The ports stay held by listeners until B binds them, so no other socket can take them in between.
    let held=[TcpListener::bind("127.0.0.1:0").unwrap(),TcpListener::bind("127.0.0.1:0").unwrap()];
    let [p2p,rpc]=held.each_ref().map(|l|l.local_addr().unwrap().port());let (chain_id,genesis)=(spec_b.chain_id(),spec_b.genesis_hash());
    let dir=std::env::temp_dir().join(format!("pocold-relay-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let cfg=Config{dir,name:"b".into(),miner:None,listen:Some(format!("127.0.0.1:{p2p}").parse().unwrap()),peers:vec![],
        rpc:Some(format!("127.0.0.1:{rpc}").parse().unwrap()),stop_at:None,log:false,limits:Limits::default()};
    let mut node=Node::open(cfg,spec_b).unwrap();drop(held);node.start_network().unwrap();std::thread::spawn(move||node.run());
    Fixture{chain:Chain::new(spec_local),keys,p2p,rpc,chain_id,genesis}
}
/// The next frame from B within `wait` (None on timeout or close).
fn read(s:&mut TcpStream,wait:Duration)->Option<Msg>{
    s.set_read_timeout(Some(wait.max(Duration::from_millis(1)))).unwrap();
    let mut len=[0u8;4];s.read_exact(&mut len).ok()?;
    let mut b=vec![0u8;u32::from_be_bytes(len) as usize];s.read_exact(&mut b).ok()?;
    Msg::decode(&b).ok()
}
/// The next gossip from B (anything but a Hello) within `wait`, or None.
fn gossip(s:&mut TcpStream,wait:Duration)->Option<Msg>{
    let t=Instant::now();
    while t.elapsed()<wait{match read(s,wait.saturating_sub(t.elapsed())){Some(Msg::Hello{..})=>continue,m=>return m}}
    None
}
/// True once B has closed the connection within `wait`.
fn closed(s:&mut TcpStream,wait:Duration)->bool{
    s.set_read_timeout(Some(wait)).unwrap();let mut b=[0u8;4096];
    loop{match s.read(&mut b){Ok(0)=>return true,Ok(_)=>continue,Err(e)=>return !matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)}}
}
/// A raw peer that has completed the handshake with B (B's Hello read, ours sent).
fn peer(f:&Fixture,name:&str)->TcpStream{
    let mut s=TcpStream::connect(("127.0.0.1",f.p2p)).unwrap();let _=s.set_nodelay(true);
    s.write_all(&frame(&Msg::Hello{chain_id:f.chain_id,genesis:f.genesis,head:f.chain.head().hash,height:0,work:[0;32],node:name.into(),
        protocol:pocold::identity::PROTOCOL_VERSION,compatibility:pocold::identity::Consensus::of(&f.chain.spec).id,listen_port:0,nonce:0})).unwrap();
    assert!(matches!(read(&mut s,Duration::from_secs(5)),Some(Msg::Hello{..})),"{name}: B's Hello");
    s
}
/// `n` raw peers, returned once B has accepted all their Hellos (B gossips only to compatible peers).
fn peers<const N:usize>(f:&Fixture,names:[&str;N])->[TcpStream;N]{
    let ps=names.map(|n|peer(f,n));
    let t=Instant::now();
    while status(f)["compatiblePeers"].as_u64()!=Some(N as u64){assert!(t.elapsed()<Duration::from_secs(5),"B did not accept {N} peers");std::thread::sleep(Duration::from_millis(20));}
    ps
}
/// Waits until attempt 0 at height 1 is open (C >= L(0)), then has Draw(1,0) sign a template with `body`
/// at C. Returns its TemplateID, the Template message as the proposer sends it, and the child context.
fn template(f:&Fixture,body:Body)->([u8;32],Msg,ChildCtx){
    let p=f.chain.full_entry(&f.chain.head().hash).unwrap();let ctx=f.chain.child_ctx(&p,0).unwrap();
    while now_s()<ctx.window.lower(){std::thread::sleep(Duration::from_millis(50));}
    let (h,_,ctx)=f.chain.build_signed(&p,&f.keys[&ctx.proposer],now_s(),body.clone(),vec![]).unwrap();
    (template_id(&h.ut_raw),Msg::Template{shell:h.encoded.clone(),body:body.encode()},ctx)
}
/// The nonces below nMax (when `below`) or from nMax on (otherwise) whose PoW for `tid` meets T_share.
fn valid_shares<'a>(tid:&'a [u8;32],ctx:&'a ChildCtx,below:bool)->impl Iterator<Item=u64>+'a{
    let n_max=ctx.assignment.nonce_max();let start=if below{0}else{u64::try_from(n_max).unwrap()};
    (start..).filter(move|n|((*n as u128)<n_max)==below&&le_target(&pow_hash(tid,*n),&ctx.t_share))
}
/// The first nonce below nMax whose PoW does not meet T_share.
fn invalid_share(tid:&[u8;32],ctx:&ChildCtx)->u64{(0u64..).find(|n|!le_target(&pow_hash(tid,*n),&ctx.t_share)).unwrap()}
/// B's pocol_status result.
fn status(f:&Fixture)->serde_json::Value{
    let body=r#"{"jsonrpc":"2.0","id":1,"method":"pocol_status","params":[]}"#;
    let mut s=TcpStream::connect(("127.0.0.1",f.rpc)).unwrap();s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.write_all(format!("POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",body.len()).as_bytes()).unwrap();
    let mut out=String::new();let _=s.read_to_string(&mut out);
    serde_json::from_str::<serde_json::Value>(out.split("\r\n\r\n").nth(1).unwrap_or("null")).unwrap()["result"].clone()
}
fn stats(f:&Fixture,keys:&[&str])->Vec<Option<u64>>{let s=status(f);keys.iter().map(|k|s["stats"][*k].as_u64()).collect()}

#[test] fn a_locked_template_and_a_valid_share_are_relayed_once_and_never_back(){let _g=serial();
    let f=fixture("relay",778101,|_|{});
    let [mut x,mut y]=peers(&f,["x","y"]);
    let (tid,tmsg,ctx)=template(&f,Body::default());
    let s:Vec<u64>=valid_shares(&tid,&ctx,true).take(5).collect();let (early,s1,s2,s3,s4)=(s[0],s[1],s[2],s[3],s[4]);
    let share=|n:u64|Msg::Share{tid,nonce:n};
    let next=|p:&mut TcpStream|gossip(p,Duration::from_secs(5));
    // A share for a template B has not locked (it is still waiting) is dropped, not kept for later; then the
    // template locks and goes to y, and nothing before it.
    x.write_all(&frame(&share(early))).unwrap();
    x.write_all(&frame(&tmsg)).unwrap();
    assert_eq!(next(&mut y),Some(tmsg.clone()),"B locks the drawn proposer's template from x and relays it to y");
    // Barriers instead of timeouts: B handles one sender's frames in order and sends to one peer in order, so
    // when a fresh valid share sent after some frames is the receiver's next gossip, none of those frames
    // reached it. y sends a second copy of the template, then s1: x must get s1 first.
    y.write_all(&frame(&tmsg)).unwrap();y.write_all(&frame(&share(s1))).unwrap();
    assert_eq!(next(&mut x),Some(share(s1)),"x got neither its own template back nor y's second copy, and y's new share once");
    // Invalid PoW, nonce >= nMax and a foreign TemplateID are dropped; the next valid share goes to y only.
    let mut other=tid;other[0]^=1;
    for m in [share(invalid_share(&tid,&ctx)),share(valid_shares(&tid,&ctx,false).next().unwrap()),
        Msg::Share{tid:other,nonce:valid_shares(&other,&ctx,true).next().unwrap()},share(s2)]{x.write_all(&frame(&m)).unwrap();}
    assert_eq!(next(&mut y),Some(share(s2)),"only the valid share reached y (and the early share was never applied)");
    y.write_all(&frame(&share(s3))).unwrap();
    assert_eq!(next(&mut x),Some(share(s3)),"x did not get its own share s2 back");
    y.write_all(&frame(&share(s2))).unwrap();y.write_all(&frame(&share(s4))).unwrap();
    assert_eq!(next(&mut x),Some(share(s4)),"a duplicate share (s2 from y) is not relayed");
    // Every frame above was handled before s4 was relayed, so the counts are final.
    assert_eq!(stats(&f,&["relayedTemplates","relayedShares","peerShares","templatePenalties"]),[Some(1),Some(4),Some(4),Some(0)]);
}

#[test] fn a_template_with_a_foreign_signature_is_ignored_and_never_relayed(){let _g=serial();
    let f=fixture("foreign",778102,|_|{});
    let [mut x,mut y]=peers(&f,["x","y"]);
    let (tid,tmsg,ctx)=template(&f,Body::default());
    let Msg::Template{shell,body}=tmsg.clone() else{unreachable!()};
    // The drawn proposer's TemplateID signed with another member's key: the signature does not recover to the
    // proposer, so the template does not apply here (no lock, no relay), and its sender is not punished.
    // Both templates come from x: B handles one sender's frames in order, so the forged one is checked while
    // B is still waiting (from two senders their order would be a race).
    let other=f.keys.iter().find(|(a,_)|**a!=ctx.proposer).map(|(_,k)|k).unwrap();
    let mut h=decode_header(&shell).unwrap();h.sig=other.sign_hash(&B256::from(hashes::sig_msg(&tid))).to_vec();reseal(&mut h);
    x.write_all(&frame(&Msg::Template{shell:h.encoded.clone(),body})).unwrap();
    x.write_all(&frame(&tmsg)).unwrap();
    assert_eq!(gossip(&mut y,Duration::from_secs(5)),Some(tmsg),"y's next gossip is the honest template: the foreign one neither locked nor was relayed");
    // Barrier: x's next frame, a valid share for the locked template, reaches y. So B handled x's foreign
    // template before it and did not close x for it.
    let s=valid_shares(&tid,&ctx,true).next().unwrap();
    x.write_all(&frame(&Msg::Share{tid,nonce:s})).unwrap();
    assert_eq!(gossip(&mut y,Duration::from_secs(5)),Some(Msg::Share{tid,nonce:s}),"its sender is not punished");
    assert_eq!(stats(&f,&["relayedTemplates","templatePenalties"]),[Some(1),Some(0)]);
}

#[test] fn a_template_whose_rebuild_differs_punishes_its_sender_and_is_never_relayed(){let _g=serial();
    let f=fixture("rebuild",778103,|_|{});
    let [mut x,mut y,mut z]=peers(&f,["x","y","z"]);
    let (_,tmsg,_)=template(&f,Body::default());
    let Msg::Template{shell,..}=tmsg.clone() else{unreachable!()};
    // The proposer's signed shell with a body it does not commit to: the rebuild differs, so the template is
    // as if absent (consensus.md:101); its sender is closed and nothing is relayed.
    let tx=Tx{chain_id:f.chain_id,nonce:0,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas:21000,
        to:Some(devnet::account_key("bob").address()),value:U256::from(1u64),data:Default::default()}.sign_1559(&devnet::account_key("alice"));
    x.write_all(&frame(&Msg::Template{shell,body:Body{txs:vec![tx],evidence:vec![]}.encode()})).unwrap();
    assert!(closed(&mut x,Duration::from_secs(5)),"the sender of a template whose rebuild differs is closed");
    z.write_all(&frame(&tmsg)).unwrap();
    assert_eq!(gossip(&mut y,Duration::from_secs(5)),Some(tmsg),"y's next gossip is the honest template: the invalid one was not relayed");
    assert_eq!(stats(&f,&["relayedTemplates","templatePenalties"]),[Some(1),Some(1)]);
}

/// The share relay bound: at most 4·S_max shares per locked template are relayed (a block carries at most
/// S_max; more only cost bandwidth). Full-nonce mode has unboundedly many valid shares per template, so the
/// bound is reached there; a low hash rate makes 4·S_max + 1 of them cheap to find.
#[test] fn share_relay_stops_at_4_s_max_shares_per_template(){let _g=serial();
    let f=fixture("cap",778104,|c|{c.nonce_mode=1;c.hash_rate=1024;});
    let [mut x,mut y]=peers(&f,["x","y"]);
    let (tid,tmsg,ctx)=template(&f,Body::default());
    x.write_all(&frame(&tmsg)).unwrap();
    assert_eq!(gossip(&mut y,Duration::from_secs(5)),Some(tmsg),"locked and relayed");
    let cap=4*usize::from(f.chain.spec.cp().S_max);
    let sent:Vec<u64>=valid_shares(&tid,&ctx,true).take(cap+1).collect();
    let mut batch=Vec::new();for n in &sent{batch.extend(frame(&Msg::Share{tid,nonce:*n}));}
    x.write_all(&batch).unwrap();
    let mut got=Vec::new();
    while let Some(m)=gossip(&mut y,Duration::from_millis(1500)){match m{Msg::Share{tid:t,nonce} if t==tid=>got.push(nonce),m=>panic!("unexpected {m:?}")}}
    assert_eq!(got,sent[..cap],"the first 4·S_max valid shares are relayed, in order, and no more");
    assert_eq!(stats(&f,&["relayedShares","peerShares"]),[Some(cap as u64),Some(cap as u64+1)],"the last one is kept but not relayed");
}

// ---------------- V3-05 template-latency telemetry (network.md, "Template latency") ----------------
/// The `result` of a parameterless RPC `method` on the node at `rpc`.
fn call(rpc:u16,method:&str)->serde_json::Value{
    let body=format!(r#"{{"jsonrpc":"2.0","id":1,"method":"{method}","params":[]}}"#);
    let mut s=TcpStream::connect(("127.0.0.1",rpc)).unwrap();s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.write_all(format!("POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",body.len()).as_bytes()).unwrap();
    let mut out=String::new();let _=s.read_to_string(&mut out);
    serde_json::from_str::<serde_json::Value>(out.split("\r\n\r\n").nth(1).unwrap_or("null")).unwrap()["result"].clone()
}
fn wait_for(ms:u64,mut f:impl FnMut()->bool)->bool{let t=Instant::now();while t.elapsed()<Duration::from_millis(ms){if f(){return true}std::thread::sleep(Duration::from_millis(50));}f()}

/// B waits in the attempt-0 slot at height 1 for Draw(1,0); the drawn proposer's template arrives before T_j and
/// locks. pocol_templateLatency (and pocol_status.templateLatency) then hold one slot, one arrival and one lock for
/// that proposer, none late; a relayed second copy is not a second sample.
#[test] fn template_latency_records_the_drawn_proposers_arrival_and_lock_once(){let _g=serial();
    let f=fixture("latency",778105,|_|{});
    let [mut x,mut y]=peers(&f,["x","y"]);
    let before=call(f.rpc,"pocol_templateLatency");
    assert_eq!((before["tracked"].as_u64(),before["all"]["slots"].as_u64(),before["all"]["received"].as_u64()),(Some(1),Some(1),Some(0)),"B waits in one slot: {before}");
    let (tid,tmsg,ctx)=template(&f,Body::default());
    x.write_all(&frame(&tmsg)).unwrap();
    assert_eq!(gossip(&mut y,Duration::from_secs(5)),Some(tmsg.clone()),"locked and relayed");
    // y's copy, then a share from y: once x gets the share, B has handled the copy.
    let s=valid_shares(&tid,&ctx,true).next().unwrap();
    y.write_all(&frame(&tmsg)).unwrap();y.write_all(&frame(&Msg::Share{tid,nonce:s})).unwrap();
    assert_eq!(gossip(&mut x,Duration::from_secs(5)),Some(Msg::Share{tid,nonce:s}));
    let t=call(f.rpc,"pocol_templateLatency");
    assert_eq!(t["tracked"],1);assert_eq!(t["trackedMax"],256);assert_eq!(t["local"]["deltaRel"],5);
    let p=&t["proposers"][0];
    assert_eq!(p["proposer"],format!("{:#x}",ctx.proposer),"keyed by Draw(1,0)");
    let n=|k:&str|p[k].as_u64().unwrap();
    assert_eq!((n("slots"),n("received"),n("locked"),n("late"),n("missing")),(1,1,1,0,0),"{p}");
    for h in ["sinceParentMs","sinceWindowMs","lockMs"]{
        assert_eq!(p[h]["count"],1,"{h}");let m=p[h]["max"].as_u64().unwrap();
        assert!(p[h]["p50"].as_u64().unwrap()<=m&&p[h]["p99"].as_u64().unwrap()<=m&&p[h]["mean"].as_u64().unwrap()<=m,"{h}: {}",p[h]);
    }
    assert!(n("late")==0&&p["lockMs"]["max"].as_u64()>=p["sinceParentMs"]["max"].as_u64(),"the lock follows the arrival: {p}");
    assert!(p["sinceWindowMs"]["max"].as_u64().unwrap()<25_000,"inside the attempt window: {p}");
    assert_eq!(t["all"]["received"],1);assert_eq!(t["all"]["locked"],1);
    assert_eq!(status(&f)["templateLatency"],t,"pocol_status carries the same counters");
}

/// A template that arrives after T_j (B already on the attempt-0 fallback) is a late arrival and never locks.
#[test] fn a_template_after_the_deadline_is_counted_late(){let _g=serial();
    let f=fixture("latency-late",778106,|_|{});
    let [mut x]=peers(&f,["x"]);
    assert!(wait_for(30_000,||status(&f)["production"].as_str().is_some_and(|m|m.starts_with("FB h=1 a=0"))),"B passes T_j and locks the fallback");
    let (_,tmsg,ctx)=template(&f,Body::default());
    x.write_all(&frame(&tmsg)).unwrap();
    assert!(wait_for(5000,||call(f.rpc,"pocol_templateLatency")["all"]["received"]==1),"the late template is recorded");
    let t=call(f.rpc,"pocol_templateLatency");let p=&t["proposers"][0];
    assert_eq!(p["proposer"],format!("{:#x}",ctx.proposer));
    let n=|k:&str|p[k].as_u64().unwrap();
    assert_eq!((n("slots"),n("received"),n("late"),n("locked")),(1,1,1,0),"{p}");
    assert!(p["sinceParentMs"]["max"].as_u64().unwrap()>=5000,"after b_j + Δ_rel (5 s): {p}");
    assert!(status(&f)["production"].as_str().unwrap().starts_with("FB"),"B stays on its fallback");
}

/// A local devnet of three mining nodes over loopback: each one's counters fill from the other members' templates,
/// and never list the node's own proposals.
#[test] fn template_latency_populates_on_a_local_multi_node_devnet(){let _g=serial();
    use std::sync::{Arc,atomic::{AtomicBool,Ordering}};
    struct Live{rpc:u16,me:Address,stop:Arc<AtomicBool>,run:Option<std::thread::JoinHandle<()>>}
    impl Drop for Live{fn drop(&mut self){self.stop.store(true,Ordering::SeqCst);if let Some(r)=self.run.take(){let _=r.join();}}}
    let d=devnet::build(&DevnetConfig::new(778107,now_s(),&["a","b","c"])).unwrap();
    let local=pocold::single_host_local();
    let (first,timing)=d.admit(&local).unwrap_or_else(|e|panic!("{e}"));let mut first=Some(first);
    let held:Vec<[TcpListener;2]>=(0..3).map(|_|[TcpListener::bind("127.0.0.1:0").unwrap(),TcpListener::bind("127.0.0.1:0").unwrap()]).collect();
    let ports:Vec<(u16,u16)>=held.iter().map(|h|(h[0].local_addr().unwrap().port(),h[1].local_addr().unwrap().port())).collect();
    drop(held);
    let root=std::env::temp_dir().join(format!("pocold-latency-live-{}",std::process::id()));let _=std::fs::remove_dir_all(&root);
    let nodes:Vec<Live>=d.miners.iter().enumerate().map(|(i,m)|{
        let spec=first.take().map(Spec::new).unwrap_or_else(||Spec::new(admission::admit(&d.request(&local,Some(&timing))).unwrap_or_else(|e|panic!("{e}"))));
        let peers=ports[..i].iter().map(|(p,_)|format!("127.0.0.1:{p}").parse().unwrap()).collect();
        let cfg=Config{dir:root.join(i.to_string()),name:format!("n{i}"),miner:Some(Key::from_secret(&m.key.secret()).unwrap()),listen:Some(format!("127.0.0.1:{}",ports[i].0).parse().unwrap()),
            peers,rpc:Some(format!("127.0.0.1:{}",ports[i].1).parse().unwrap()),stop_at:None,log:false,limits:Limits::default()};
        let mut n=Node::open(cfg,spec).unwrap();n.start_network().unwrap();
        let stop=Arc::new(AtomicBool::new(false));let s=stop.clone();
        Live{rpc:ports[i].1,me:m.id,stop,run:Some(std::thread::spawn(move||n.run_controlled(|_|s.load(Ordering::SeqCst))))}
    }).collect();
    let filled=|n:&Live|{let t=call(n.rpc,"pocol_templateLatency");t["all"]["received"].as_u64().unwrap_or(0)>=1&&t["all"]["locked"].as_u64().unwrap_or(0)>=1};
    assert!(wait_for(240_000,||nodes.iter().all(filled)),"every node recorded another member's template: {:?}",nodes.iter().map(|n|call(n.rpc,"pocol_templateLatency")["all"].clone()).collect::<Vec<_>>());
    for n in &nodes{
        let t=call(n.rpc,"pocol_templateLatency");
        let ps=t["proposers"].as_array().unwrap();assert!(!ps.is_empty()&&ps.len()<=2,"{t}");
        let mut sum=(0,0,0);
        for p in ps{
            assert_ne!(p["proposer"],format!("{:#x}",n.me),"a node does not time its own templates");
            assert!(p["received"].as_u64()<=p["slots"].as_u64()&&p["locked"].as_u64()<=p["received"].as_u64(),"{p}");
            sum.0+=p["slots"].as_u64().unwrap();sum.1+=p["received"].as_u64().unwrap();sum.2+=p["locked"].as_u64().unwrap();
        }
        assert_eq!((t["all"]["slots"].as_u64().unwrap(),t["all"]["received"].as_u64().unwrap(),t["all"]["locked"].as_u64().unwrap()),sum,"the aggregate sums the proposers (none evicted)");
        assert_eq!(t["all"]["sinceParentMs"]["count"],t["all"]["received"]);
        let st=call(n.rpc,"pocol_status")["templateLatency"].clone();assert_eq!(st["trackedMax"],256);assert!(st["all"]["received"].as_u64()>=t["all"]["received"].as_u64());
    }
    drop(nodes);let _=std::fs::remove_dir_all(&root);
}
