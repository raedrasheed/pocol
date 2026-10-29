//! A05 resource bounds (P2P and RPC, separately) and A10 ORIGIN_DENY per batch member,
//! on a real node over loopback sockets. Limits are shrunk so every case stays small.
use std::io::{Read,Write};
use std::net::{TcpListener,TcpStream};
use std::sync::{Arc,atomic::{AtomicBool,Ordering}};
use std::time::{Duration,Instant};
use pocold::{chain::Spec,net::{self,Msg},node::{Config,Limits,Node}};
use pocol_lp4_execution::{devnet::{self,DevnetConfig},txsign::Tx};
use revm::primitives::U256;

/// Thread counts are process-wide: these tests run one at a time.
static SERIAL:std::sync::Mutex<()>=std::sync::Mutex::new(());
fn serial()->std::sync::MutexGuard<'static,()>{SERIAL.lock().unwrap_or_else(|e|e.into_inner())}
fn free_port()->u16{TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()}
/// One devnet per test binary: its genesis is now (with a genesis in the past every node's
/// production attempts are overdue, and each tick builds a fallback template, so the nodes the
/// earlier tests leave running would saturate the CPU), and its timing is measured once
/// (R14/R19 are measured on this host and fail closed under CPU contention).
fn spec()->Spec{
    static GENESIS:std::sync::OnceLock<u64>=std::sync::OnceLock::new();
    static TIMING:std::sync::OnceLock<pocol_lp4_execution::admission::Timing>=std::sync::OnceLock::new();
    let n=devnet::build(&DevnetConfig::new(777940,*GENESIS.get_or_init(pocold::node::now_s),&["a","b","c"])).unwrap();
    let t=TIMING.get_or_init(||pocol_lp4_execution::admission::measure(&n.spec_bytes,&n.alloc,1).unwrap());
    Spec::new(pocol_lp4_execution::admission::admit(&n.request(&pocold::single_host_local(),Some(t))).unwrap_or_else(|e|panic!("{e}")))
}
struct Running{p2p:u16,rpc:u16,net:net::Net,chain_id:u64,genesis:[u8;32]}
fn start(tag:&str,limits:Limits)->Running{start_with(tag,limits,vec![])}
fn start_with(tag:&str,limits:Limits,peers:Vec<std::net::SocketAddr>)->Running{
    let (p2p,rpc)=(free_port(),free_port());let spec=spec();let (chain_id,genesis)=(spec.chain_id(),spec.genesis_hash());
    let dir=std::env::temp_dir().join(format!("pocold-net-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let cfg=Config{dir,name:tag.into(),miner:None,listen:Some(format!("127.0.0.1:{p2p}").parse().unwrap()),peers,rpc:Some(format!("127.0.0.1:{rpc}").parse().unwrap()),stop_at:None,log:false,limits};
    let mut node=Node::open(cfg,spec).unwrap();let net=node.net.clone();
    node.start_network().unwrap();std::thread::spawn(move||node.run());
    std::thread::sleep(Duration::from_millis(100));
    Running{p2p,rpc,net,chain_id,genesis}
}
fn frame(m:&Msg)->Vec<u8>{let b=m.encode();let mut f=(b.len() as u32).to_be_bytes().to_vec();f.extend(b);f}
fn hello(r:&Running,name:&str)->Vec<u8>{frame(&Msg::Hello{chain_id:r.chain_id,genesis:r.genesis,head:r.genesis,height:0,work:[0;32],node:name.into()})}
fn connect(r:&Running)->TcpStream{let s=TcpStream::connect(("127.0.0.1",r.p2p)).unwrap();s.set_read_timeout(Some(Duration::from_millis(3000))).unwrap();s}
/// True if the node closed the connection (EOF or reset) within the read timeout.
fn closed(s:&mut TcpStream)->bool{let mut b=[0u8;4096];loop{match s.read(&mut b){Ok(0)=>return true,Ok(_)=>continue,Err(e)=>return e.kind()!=std::io::ErrorKind::WouldBlock&&e.kind()!=std::io::ErrorKind::TimedOut}}}
fn live(r:&Running)->usize{r.net.counters.live.load(Ordering::SeqCst)}
/// Process threads (Linux /proc). Elsewhere thread counts are not measured: 0, and every
/// thread-count assertion holds vacuously; the notice lets a platform run report that.
fn threads()->usize{
    #[cfg(target_os="linux")]{std::fs::read_to_string("/proc/self/status").unwrap().lines().find(|l|l.starts_with("Threads:")).unwrap()[8..].trim().parse().unwrap()}
    #[cfg(not(target_os="linux"))]{eprintln!("THREADS-NOT-MEASURED");0}
}
fn wait_until(ms:u64,f:impl Fn()->bool)->bool{let t=Instant::now();while t.elapsed()<Duration::from_millis(ms){if f(){return true}std::thread::sleep(Duration::from_millis(20));}f()}
fn rpc(port:u16,body:&str,origin:bool)->(u16,String){
    let mut s=TcpStream::connect(("127.0.0.1",port)).unwrap();s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();let _=s.set_nodelay(true);
    let o=if origin{"Origin: https://site.example\r\n"}else{""};
    // A refused connection is closed unread (RPC_CONN_MAX): the write or the read may then see a reset (code 0).
    // One write: write! would send a write per piece, and Nagle plus delayed ACKs (Windows) adds about
    // 200 ms to the measured times.
    let _=s.write_all(format!("POST / HTTP/1.1\r\nHost: x\r\n{o}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",body.len()).as_bytes());
    let mut out=String::new();let _=s.read_to_string(&mut out);
    let code=out.get(9..12).and_then(|c|c.parse().ok()).unwrap_or(0);(code,out.split("\r\n\r\n").nth(1).unwrap_or("").to_string())
}
/// A client that does not read, with the default socket options. pocold must bound what the kernel
/// takes for it: SO_SNDBUF (SEND_SOCK_BUF) and writes of at most SEND_SOCK_BUF. Before the write cap,
/// Windows loopback took a whole ~14 MiB unread answer within a second (Windows runs 3, 5 and 7), so
/// an unread client could not be told from a fast one by the bytes the server's socket accepted.
/// `unread_default_client_absorption` measures it on every platform.
fn unread_client(port:u16)->TcpStream{let s=TcpStream::connect(("127.0.0.1",port)).unwrap();let _=s.set_nodelay(true);s}
fn small()->Limits{
    let mut l=Limits::default();
    l.net=net::Limits{peers_max:6,handshaking_max:3,hello_deadline:Duration::from_millis(500),frame_deadline:Duration::from_millis(500),
        idle_deadline:Duration::from_secs(60),queue_bytes_max:512*1024,write_deadline:Duration::from_millis(500)};
    l
}

#[test] fn idle_peers_without_hello_are_bounded_and_expire(){let _g=serial();
    let r=start("idle",small());let base=threads();
    let mut idle:Vec<TcpStream>=(0..10).map(|_|connect(&r)).collect();
    assert!(wait_until(500,||r.net.counters.refused.load(Ordering::SeqCst)>=7),"only 3 handshake slots: refused={}",r.net.counters.refused.load(Ordering::SeqCst));
    assert!(live(&r)<=3,"live={}",live(&r));
    assert!(threads()<=base+3*2,"two threads per admitted connection only: {} vs {base}",threads());
    // The Hello deadline (500 ms) closes every admitted idle connection.
    assert!(wait_until(2000,||live(&r)==0),"live={}",live(&r));
    assert!(idle.iter_mut().all(closed),"every idle connection is closed");
    // A legitimate peer is served afterwards: its Hello is accepted and answered.
    let mut s=connect(&r);s.write_all(&hello(&r,"legit")).unwrap();
    let mut len=[0u8;4];s.read_exact(&mut len).unwrap();let mut b=vec![0u8;u32::from_be_bytes(len) as usize];s.read_exact(&mut b).unwrap();
    assert!(matches!(Msg::decode(&b).unwrap(),Msg::Hello{..}));assert!(wait_until(500,||live(&r)==1));
    std::thread::sleep(Duration::from_millis(700));assert_eq!(live(&r),1,"an established peer is not subject to the Hello deadline");
}
#[test] fn only_a_hello_is_accepted_before_the_handshake(){let _g=serial();
    let r=start("prehello",small());
    let mut s=connect(&r);s.write_all(&frame(&Msg::GetRange{start:1,count:512})).unwrap();
    assert!(closed(&mut s),"a non-Hello first frame closes the connection");assert!(wait_until(500,||live(&r)==0));
}
#[test] fn oversized_and_stalled_frames_close_the_peer(){let _g=serial();
    let r=start("frames",small());
    let mut s=connect(&r);s.write_all(&hello(&r,"big")).unwrap();s.write_all(&((net::MAX_FRAME+1) as u32).to_be_bytes()).unwrap();
    assert!(closed(&mut s),"a length above MAX_FRAME closes before any allocation");
    let mut s=connect(&r);s.write_all(&hello(&r,"slow")).unwrap();std::thread::sleep(Duration::from_millis(100));
    s.write_all(&1000u32.to_be_bytes()).unwrap();s.write_all(&[0u8;10]).unwrap();
    let t=Instant::now();assert!(closed(&mut s));assert!(t.elapsed()<Duration::from_millis(2500),"the frame deadline (500 ms) closes a stalled frame");
    assert!(wait_until(1000,||live(&r)==0));
}
#[test] fn a_peer_that_stops_reading_is_dropped_and_consensus_keeps_serving(){let _g=serial();
    // 120 relayed transactions that declare 2M gas each: the pool's gas bound (D-7, 4 × GAS_LIMIT by
    // default) is raised to hold them, since this test is about the slow peer's outbound queue.
    let mut l=small();l.pool_gas_max=Some(120*2_000_000);let r=start("slow",l);
    // The slow peer completes the handshake and never reads again.
    let mut slow=connect(&r);slow.write_all(&hello(&r,"slow")).unwrap();assert!(wait_until(500,||live(&r)==1));
    // Large valid transactions are relayed to every peer (about 100 KiB each).
    let alice=devnet::account_key("alice");let t0=Instant::now();let mut worst=Duration::ZERO;
    for nonce in 0..120u64{
        let tx=Tx{chain_id:r.chain_id,nonce,max_priority:U256::from(1u64),max_fee:U256::from(10_000_000_000u64),gas:2_000_000,to:Some(alice.address()),value:U256::ZERO,data:vec![1u8;100_000].into()}.sign_1559(&alice);
        let body=format!(r#"{{"jsonrpc":"2.0","id":{nonce},"method":"eth_sendRawTransaction","params":["0x{}"]}}"#,pocold::types::hex(&tx).trim_start_matches("0x"));
        let t=Instant::now();let (code,_)=rpc(r.rpc,&body,false);worst=worst.max(t.elapsed());assert_eq!(code,200);
        if r.net.counters.dropped_slow.load(Ordering::SeqCst)>0{break}
    }
    assert!(r.net.counters.dropped_slow.load(Ordering::SeqCst)>=1,"the full outbound queue drops the slow peer");
    assert!(wait_until(1000,||live(&r)==0));
    assert!(worst<Duration::from_secs(2),"RPC stayed responsive while broadcasting: worst {worst:?} in {:?}",t0.elapsed());
    let (code,b)=rpc(r.rpc,r#"{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[]}"#,false);assert_eq!(code,200,"{b}");
    drop(slow);
}
#[test] fn reconnect_churn_keeps_threads_and_slots_bounded(){let _g=serial();
    let r=start("churn",small());let base=threads();
    for i in 0..200{let mut s=connect(&r);if i%2==0{let _=s.write_all(&hello(&r,"churn"));}drop(s);}
    assert!(wait_until(3000,||live(&r)==0),"live={}",live(&r));
    assert!(wait_until(3000,||threads()<=base+2),"threads {} vs {base}",threads());
    // live==0 is not an accept-queue barrier: connections already accepted by TCP can still
    // be waiting for the listener. Under single-core scheduling the original test observed
    // live=0, then 79 more refusals before its final socket was processed. A bounded refusal
    // during that backlog is valid. Require a Hello on OUR socket within the original 500 ms,
    // retrying only closed/refused probes; another churn peer's live count cannot pass this.
    let refused_before=r.net.counters.refused.load(Ordering::SeqCst);
    let until=Instant::now()+Duration::from_millis(500);let mut attempts=0;let mut accepted=false;
    while let Some(left)=until.checked_duration_since(Instant::now()){
        attempts+=1;
        if let Ok(mut s)=TcpStream::connect_timeout(&format!("127.0.0.1:{}",r.p2p).parse().unwrap(),left){
            let _=s.set_read_timeout(Some(left));let _=s.set_write_timeout(Some(left));
            if s.write_all(&hello(&r,"after")).is_ok(){
                let mut len=[0;4];
                if s.read_exact(&mut len).is_ok(){let n=u32::from_be_bytes(len) as usize;
                    if n<=net::MAX_FRAME{let mut body=vec![0;n];
                        if s.read_exact(&mut body).is_ok(){accepted=Instant::now()<=until&&matches!(Msg::decode(&body),Ok(Msg::Hello{chain_id,genesis,..}) if chain_id==r.chain_id&&genesis==r.genesis);}
                    }
                }
            }
        }
        if accepted{break}
        std::thread::yield_now();
    }
    assert!(accepted,"still accepts peers within 500 ms: attempts={attempts}, live={}, handshaking={}, refused before={refused_before}, after={}, threads={} base={base}",live(&r),r.net.counters.handshaking.load(Ordering::SeqCst),r.net.counters.refused.load(Ordering::SeqCst),threads());
}
#[test] fn rpc_connection_cap_refuses_excess_unread_and_keeps_serving(){let _g=serial();
    // network.md RPC_CONN_MAX: an excess connection is closed without being read (503).
    let mut l=small();l.rpc.conn_max=4;l.rpc.read_deadline=Duration::from_millis(1500);
    let r=start("rpccap",l);let base=threads();let refused0=pocold::rpc::COUNTERS.refused.load(Ordering::SeqCst);
    let mut idle:Vec<TcpStream>=(0..12).map(|_|TcpStream::connect(("127.0.0.1",r.rpc)).unwrap()).collect();
    std::thread::sleep(Duration::from_millis(300));
    assert!(threads()<=base+4,"at most conn_max request threads: {} vs {base}",threads());
    assert!(pocold::rpc::COUNTERS.refused.load(Ordering::SeqCst)-refused0>=8,"the 8 excess connections are refused");
    // While the slots are held, a client is refused at once rather than queued.
    let t=Instant::now();let (code,_)=rpc(r.rpc,r#"{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[]}"#,false);
    assert!(code==503||code==0,"refused (503 or reset): {code}");assert!(t.elapsed()<Duration::from_secs(1),"refused promptly");
    // The whole-request deadline (1.5 s) closes the idle readers, even a client trickling bytes; then clients are served.
    for s in idle.iter_mut(){s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();assert!(closed(s));}
    let (code,b)=rpc(r.rpc,r#"{"jsonrpc":"2.0","id":2,"method":"eth_blockNumber","params":[]}"#,false);assert_eq!(code,200,"{b}");
    // Review R2-5: the deadline runs over the whole request, not per read. A client sending one byte
    // every 400 ms for 3 s (twice the deadline) is closed while it still trickles, about 1.5 s after
    // its first byte; a per-read idle timeout would never close it.
    let mut trickle=TcpStream::connect(("127.0.0.1",r.rpc)).unwrap();trickle.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
    let t0=Instant::now();let mut closed_at=None;
    while t0.elapsed()<Duration::from_secs(3)&&closed_at.is_none(){
        let _=trickle.write_all(b"P");let next=Instant::now()+Duration::from_millis(400);
        while Instant::now()<next{let mut b=[0u8;64];match trickle.read(&mut b){
            Ok(0)=>{closed_at=Some(t0.elapsed());break}
            Err(e) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)=>{}
            _=>{closed_at=Some(t0.elapsed());break}}}
    }
    let at=closed_at.expect("a client trickling for twice REQ_READ_DEADLINE is closed while it trickles");
    assert!(at>=Duration::from_millis(1400)&&at<=Duration::from_millis(2300),"closed about REQ_READ_DEADLINE after the first byte: {at:?}");
    // Many concurrent clients each finish promptly: served (200) or refused (503 or reset), none hangs.
    let t=Instant::now();
    let hs:Vec<_>=(0..24).map(|i|{let p=r.rpc;std::thread::spawn(move||rpc(p,&format!(r#"{{"jsonrpc":"2.0","id":{i},"method":"eth_chainId","params":[]}}"#),false).0)}).collect();
    let codes:Vec<u16>=hs.into_iter().map(|h|h.join().unwrap()).collect();
    assert!(codes.iter().all(|c|[200,503,0].contains(c)),"{codes:?}");assert!(codes.iter().any(|c|*c==200));assert!(t.elapsed()<Duration::from_secs(4));
}
#[test] fn rpc_batches_answer_every_member_in_order_and_cap_their_length(){let _g=serial();
    // BATCH_RESP_MAX itself is checked by the rpc unit test (batch_members_beyond_the_byte_limit_are_not_run);
    // here the streamed batch keeps member order and ids end to end, and BATCH_MAX holds.
    let r=start("batch",small());
    let batch:Vec<String>=(0..20).map(|i|format!(r#"{{"jsonrpc":"2.0","id":{i},"method":"eth_chainId","params":[]}}"#)).collect();
    let (code,b)=rpc(r.rpc,&format!("[{}]",batch.join(",")),false);assert_eq!(code,200);
    let v:serde_json::Value=serde_json::from_str(&b).unwrap();let a=v.as_array().unwrap();
    assert_eq!(a.len(),20);for (i,x) in a.iter().enumerate(){assert_eq!(x["id"],i as u64);assert!(x["result"].is_string());}
    let (_,b)=rpc(r.rpc,&format!("[{}]",(0..21).map(|_|batch[0].clone()).collect::<Vec<_>>().join(",")),false);assert!(b.contains("-32600"),"{b}");
}
#[test] fn a10_origin_deny_applies_to_every_batch_member(){let _g=serial();
    let r=start("origin",small());
    let batch=r#"[{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[]},{"jsonrpc":"2.0","id":2,"method":"pocol_getTemplate","params":[]},
        {"jsonrpc":"2.0","id":3,"method":"pocol_submitShare","params":["0x00",1]},5,{"jsonrpc":"2.0","id":4}]"#;
    let (code,b)=rpc(r.rpc,batch,true);assert_eq!(code,200);
    let v:serde_json::Value=serde_json::from_str(&b).unwrap();let a=v.as_array().unwrap();
    assert!(a[0]["result"].is_string(),"permitted member runs: {}",a[0]);
    for i in [1,2]{assert_eq!(a[i]["error"]["code"],-32601,"{}",a[i]);assert_eq!(a[i]["error"]["message"],"method not available to sites");assert!(a[i].get("result").is_none());}
    assert_eq!(a[3]["error"]["code"],-32600);assert_eq!(a[4]["error"]["code"],-32600);
    // The singleton form is equally denied; without an Origin the local miner keeps its interface.
    let (_,b)=rpc(r.rpc,r#"{"jsonrpc":"2.0","id":9,"method":"pocol_getTemplate","params":[]}"#,true);assert!(b.contains("-32601"),"{b}");
    let (_,b)=rpc(r.rpc,r#"[{"jsonrpc":"2.0","id":9,"method":"pocol_getTemplate","params":[]}]"#,false);assert!(b.contains("\"result\""),"{b}");
}

/// Review R2-3: every error item the guard or the dispatcher produces stays within ERR_ITEM_MAX
/// (network.md:276), whatever the size of the member's id or method name: ORIGIN_DENY, an unknown
/// method and a member without a method. A short id is still echoed. In process.
#[test] fn guard_and_dispatch_error_items_stay_within_err_item_max(){let _g=serial();
    let dir=std::env::temp_dir().join(format!("pocold-net-erritem-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let cfg=Config{dir,name:"erritem".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits::default()};
    let mut n=Node::open(cfg,spec()).unwrap();
    let (big,name)=(format!("\"{}\"","i".repeat(100_000)),"m\\u0000".repeat(20_000));
    let batch=format!(r#"[{{"jsonrpc":"2.0","id":{big},"method":"pocol_getTemplate","params":[]}},{{"jsonrpc":"2.0","id":{big},"method":"{name}","params":[]}},{{"jsonrpc":"2.0","id":{big}}},
        {{"jsonrpc":"2.0","id":7,"method":"pocol_getTemplate","params":[]}},{{"jsonrpc":"2.0","id":8,"method":"no_such","params":[]}}]"#);
    let v:serde_json::Value=serde_json::from_str(&pocold::rpc::handle_with(&mut n,batch.as_bytes(),true)).unwrap();let a=v.as_array().unwrap();
    for (i,code) in [(0,-32601),(1,-32601),(2,-32600),(3,-32601),(4,-32601)]{
        assert_eq!(a[i]["error"]["code"],code,"{}",a[i]);let len=a[i].to_string().len();assert!(len<=pocold::rpc::ERR_ITEM_MAX,"item {i}: {len} B");}
    for i in 0..3{assert!(a[i]["id"].is_null(),"an id over ID_ECHO_MAX is not echoed");}
    assert_eq!((a[3]["id"].clone(),a[4]["id"].clone()),(serde_json::json!(7),serde_json::json!(8)),"short ids are echoed");
    assert_eq!(a[4]["error"]["message"],"method no_such not found");assert_eq!(a[1]["error"]["message"],"method not found");
    // The single-request forms are bounded the same way.
    for body in [format!(r#"{{"jsonrpc":"2.0","id":{big},"method":"{name}","params":[]}}"#),format!(r#"{{"jsonrpc":"2.0","id":{big},"method":"pocol_getTemplate"}}"#)]{
        let s=pocold::rpc::handle_with(&mut n,body.as_bytes(),true);assert!(s.len()<=pocold::rpc::ERR_ITEM_MAX&&s.contains("-32601"),"{} B",s.len());}
}

#[test] fn inbound_floods_cannot_take_the_slots_for_configured_peers(){let _g=serial();
    // Review finding (A05): dials used to share peers_max/handshaking_max with inbound connections.
    let target=TcpListener::bind("127.0.0.1:0").unwrap();let taddr=target.local_addr().unwrap();target.set_nonblocking(true).unwrap();
    let accept=|t:&TcpListener|{let s=Instant::now();loop{if let Ok((c,_))=t.accept(){return Some(c)}if s.elapsed()>Duration::from_secs(3){return None}std::thread::sleep(Duration::from_millis(20));}};
    let mut l=small();l.net.hello_deadline=Duration::from_secs(10);
    let r=start_with("outbound",l,vec![taddr]);
    let first=accept(&target).expect("the node dials its configured peer");
    // Every inbound handshake slot is taken by connections that never send a Hello.
    let idle:Vec<TcpStream>=(0..8).map(|_|connect(&r)).collect();
    assert!(wait_until(1000,||r.net.counters.handshaking.load(Ordering::SeqCst)==3),"inbound handshakes full");
    assert_eq!(r.net.counters.outbound.load(Ordering::SeqCst),1);
    // The configured peer drops; the node reconnects although inbound is full.
    drop(first);
    assert!(accept(&target).is_some(),"outbound reconnects despite a full inbound set");
    assert!(wait_until(1000,||r.net.counters.outbound.load(Ordering::SeqCst)==1));drop(idle);
}
/// RpcGuard REQ_BUF (network.md:229): request line, headers and body together. A request that
/// fills REQ_BUF exactly is served; one byte more is 413 without reaching the node.
#[test] fn rpc_request_buffer_covers_headers_and_body(){let _g=serial();
    let r=start("reqbuf",Limits::default());
    let call=r#"{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}"#;
    let head=|n:usize|format!("POST / HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {n}\r\n\r\n");
    let send=|extra:usize|->u16{
        let n=pocold::rpc::REQ_BUF-head(1_000_000).len()+extra;assert_eq!(head(n).len(),head(1_000_000).len());
        let mut body=call.to_string();body.push_str(&" ".repeat(n-call.len()));
        let mut s=TcpStream::connect(("127.0.0.1",r.rpc)).unwrap();s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let _=s.write_all(head(n).as_bytes());let _=s.write_all(body.as_bytes());
        let mut out=String::new();let _=s.read_to_string(&mut out);out.get(9..12).and_then(|c|c.parse().ok()).unwrap_or(0)
    };
    assert_eq!(send(0),200,"headers + body == REQ_BUF is served");
    assert_eq!(send(1),413,"one byte over REQ_BUF");
}
/// A13 thread inventory under a stalled consensus thread: heavy eth_calls (30M gas) keep it
/// busy while peers fill the event queue and churn. Every reader joins its writer and delivers
/// Closed before it releases its slot, and the listener joins finished readers before reusing a
/// slot, so the threads alive never exceed two per peer slot plus the RPC connection in use and at
/// most one that has finished but is not yet joined (the acceptor joins it at the next accept); the
/// A13 inventory counts conn_max RPC threads (and this test's own threads).
#[test] fn peer_threads_stay_within_the_inventory_while_consensus_is_stalled(){let _g=serial();
    let r=start("inventory",small());let base=threads();let peers_max=6usize;
    let stop=std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let heavy={let (port,stop)=(r.rpc,stop.clone());std::thread::spawn(move||{let mut n=0;while !stop.load(Ordering::SeqCst){
        let (code,_)=rpc(port,r#"{"jsonrpc":"2.0","id":1,"method":"eth_call","params":[{"data":"0x5b600056","gas":"0x1c9c380"},"latest"]}"#,false);if code==200{n+=1}}n})};
    let churn:Vec<_>=(0..3).map(|i|{let (port,stop,h)=(r.p2p,stop.clone(),hello(&r,&format!("churn{i}")));std::thread::spawn(move||{let mut k=0;while !stop.load(Ordering::SeqCst){
        if let Ok(mut s)=TcpStream::connect(("127.0.0.1",port)){let _=s.set_write_timeout(Some(Duration::from_millis(200)));let _=s.write_all(&h);
            for _ in 0..100{if s.write_all(&frame(&Msg::GetRange{start:1,count:1})).is_err(){break}}k+=1;}
    }k})}).collect();
    let own=1+3;let t=Instant::now();let mut worst=0usize;
    while t.elapsed()<Duration::from_secs(6){worst=worst.max(threads().saturating_sub(base+own));std::thread::sleep(Duration::from_millis(3));}
    stop.store(true,Ordering::SeqCst);let calls=heavy.join().unwrap();let conns:usize=churn.into_iter().map(|h|h.join().unwrap()).sum();
    assert!(calls>=2&&conns>=10,"the scenario ran: {calls} heavy calls, {conns} peer connections");
    assert!(worst<=2*peers_max+2,"threads beyond the inventory: {worst} > {}",2*peers_max+2);
    assert!(wait_until(5000,||live(&r)==0&&threads()<=base+2),"live={} threads {} vs {base}",live(&r),threads());
    println!("INVENTORY {}",serde_json::json!({"worstExtraThreads":worst,"bound":2*peers_max+2,"heavyCalls":calls,"peerConnections":conns}));
}

// ---------------- round 2: the RPC work deadline, tested on its own ----------------
const HEAVY:&str=r#"{"jsonrpc":"2.0","id":1,"method":"eth_call","params":[{"data":"0x5b600056","gas":"0x1c9c380"},"latest"]}"#;
const LIGHT:&str=r#"{"jsonrpc":"2.0","id":2,"method":"eth_blockNumber","params":[]}"#;
fn timed(port:u16,body:&str)->(u16,Duration){let t=Instant::now();let (c,_)=rpc(port,body,false);(c,t.elapsed())}
fn counter(c:&std::sync::atomic::AtomicUsize)->usize{c.load(Ordering::SeqCst)}
/// While a 30M-gas eth_call holds the consensus thread, requests queued behind it get 504 at the
/// work deadline (not when the work would end), are never run once it has passed, and release
/// their connection threads; a batch stops starting members at the deadline.
/// The heavy call's duration on this host and build: the shorter of two warm runs on a node with
/// a wide deadline. Deadlines under test are fractions of it, so the tests do not depend on how
/// fast the build is.
fn heavy_duration()->Duration{
    let mut wide=small();wide.rpc.work_deadline=Duration::from_secs(60);let w=start("heavy-duration",wide);
    (0..2).map(|_|{let (code,t)=timed(w.rpc,HEAVY);assert_eq!(code,200);t}).min().unwrap()
}
/// pocold's RPC counters. A heavy request: while it runs it holds its own RESP_MAX reservation.
fn stats(port:u16)->serde_json::Value{let (_,b)=rpc(port,r#"{"jsonrpc":"2.0","id":9,"method":"pocold_rpcCounters","params":[]}"#,false);serde_json::from_str::<serde_json::Value>(&b).unwrap()["result"].clone()}
/// Review F8: the work deadline, independently of build speed. The deadline under test is a
/// quarter of the heavy call's measured duration d, so the heavy call always outlasts it:
/// - the heavy caller is answered 504 at the deadline, while the work runs on;
/// - light requests sent meanwhile are run (200) when they are drained before the heavy event,
///   or wait behind it and expire (504); one dequeued after its deadline is skipped, never run,
///   so the skipped count is at most the 504s, and at least one is skipped;
/// - in a batch whose first member runs past the deadline, the 19 later members are not started;
/// - afterwards no connection thread, queued body or response reservation is left.
#[test] fn rpc_work_deadline_skips_expired_work_and_releases_resources(){let _g=serial();
    let c=&pocold::rpc::COUNTERS;let d=heavy_duration();let dl=(d/4).max(Duration::from_millis(2));
    let mut l=small();l.rpc.work_deadline=dl;let r=start("deadline",l);
    let base=threads();let (e0,m0)=(counter(&c.expired),counter(&c.expired_members));let (ha0,lq0)=(counter(&c.heavy_active),counter(&c.light_queued));
    let stop=Arc::new(AtomicBool::new(false));
    let h=std::thread::spawn({let p=r.rpc;move||timed(p,HEAVY)});
    let lights:Vec<_>=(0..8).map(|_|{let (p,stop)=(r.rpc,stop.clone());std::thread::spawn(move||{let mut v=vec![];while !stop.load(Ordering::SeqCst){v.push(rpc(p,LIGHT,false).0)}v})}).collect();
    let (hc,ht)=h.join().unwrap();std::thread::sleep(d);stop.store(true,Ordering::SeqCst);
    let codes:Vec<u16>=lights.into_iter().flat_map(|t|t.join().unwrap()).collect();
    assert_eq!(hc,504);assert!(ht>=dl&&ht<d,"the heavy caller is answered at the deadline ({dl:?}), not after the work ({d:?}): {ht:?}");
    // A full light lane (requests that expired behind the heavy call and are not yet skipped) answers
    // -32021 {busy} with HTTP 200 (network.md:214); 503 is only the RPC_CONN_MAX refusal.
    assert!(codes.iter().all(|c|matches!(*c,200|503|504)),"{codes:?}");let late=codes.iter().filter(|c|**c==504).count();
    // Review B5: the skipped count covers exactly the window of `codes`. Wait until the heavy call has
    // finished and every light request of the window has left the lane (run or skipped), sending
    // nothing meanwhile; only then snapshot, before the serve-again polling below (whose own requests
    // could expire and inflate the count past `late`).
    assert!(wait_until(30_000,||counter(&c.heavy_active)<=ha0&&counter(&c.light_queued)<=lq0),"the window's requests are all run or skipped");
    let skipped=counter(&c.expired)-e0;
    assert!(wait_until(10_000,||rpc(r.rpc,LIGHT,false).0==200),"the node serves again");
    assert!(late>=1&&skipped>=1&&skipped<=late,"light 504s {late}, skipped {skipped} (of {} light requests)",codes.len());
    // A batch: the first member runs past the deadline; the 19 after it are not started.
    let mut b=vec![HEAVY.to_string()];b.extend((0..19).map(|_|LIGHT.to_string()));let body=format!("[{}]",b.join(","));
    let (code,t)=timed(r.rpc,&body);assert_eq!(code,504);assert!(t<d,"{t:?}");
    assert!(wait_until(10_000,||counter(&c.expired_members)-m0==19),"members after the deadline are not started: {}",counter(&c.expired_members)-m0);
    // Resources: connection threads, queued bodies and reservations are all released.
    assert!(wait_until(5000,||counter(&c.active)==0&&threads()<=base+2),"active={} threads {} vs {base}",counter(&c.active),threads());
    let st=stats(r.rpc);assert_eq!(st["queued"],1,"only the stats request itself is queued");assert_eq!(st["heavyReserved"],pocold::rpc::RESP_MAX,"only the stats request itself is reserved");assert_eq!(st["lightReserved"],0);
    println!("DEADLINE {}",serde_json::json!({"heavyMs":d.as_millis() as u64,"workDeadlineMs":dl.as_millis() as u64,"heavyAnsweredMs":ht.as_millis() as u64,
        "lightRequests":codes.len(),"light504":late,"light503":codes.iter().filter(|c|**c==503).count(),"skipped":skipped,"batchMembersNotRun":counter(&c.expired_members)-m0}));
}
/// Review F4: the body of a heavy request whose caller already got 504 stays queued until the
/// node reaches it. At most queued_max (HEAVY_QUEUE) are held at any moment, a further heavy
/// request is -32021 busy (counted), and every held body is released when the node skips it.
#[test] fn abandoned_queued_bodies_are_capped_and_released(){let _g=serial();
    let c=&pocold::rpc::COUNTERS;let d=heavy_duration();
    // A small HEAVY_QUEUE and a short QUEUE_WAIT so a full-for-QUEUE_WAIT queue reliably answers busy
    // while the consensus thread is stalled on the first heavy call (network.md:214, :234).
    let mut l=small();l.rpc.conn_max=2;l.rpc.heavy_queue=2;l.rpc.queue_wait=Duration::from_millis(5);l.rpc.work_deadline=(d/8).max(Duration::from_millis(1));let qmax=pocold::rpc::queued_max(&l.rpc);let r=start("queued",l);
    let (f0,q0)=(counter(&c.queue_full),counter(&c.queued));
    let stop=Arc::new(AtomicBool::new(false));let peak=Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let sampler={let (stop,peak)=(stop.clone(),peak.clone());std::thread::spawn(move||while !stop.load(Ordering::SeqCst){peak.fetch_max(counter(&pocold::rpc::COUNTERS.queued).saturating_sub(q0),Ordering::SeqCst);std::thread::yield_now();})};
    let mut rounds=0;
    // Windows run 20 (49ae9fa): a queue of 4 with a 20 ms wait reached its cap without a busy answer in
    // 5 rounds; a queue of 2, a 5 ms wait and up to 12 rounds make a full-for-QUEUE_WAIT queue likely.
    while counter(&c.queue_full)==f0&&rounds<12{rounds+=1;
        // One heavy call holds the consensus thread for about d; two senders keep abandoning
        // heavy requests (each answered 504 at d/8, its body left queued).
        let h=std::thread::spawn({let p=r.rpc;move||rpc(p,HEAVY,false).0});std::thread::sleep(d/16);
        let senders:Vec<_>=(0..2).map(|_|{let p=r.rpc;std::thread::spawn(move||{let t=Instant::now();let mut v=vec![];while t.elapsed()<d{v.push(rpc(p,HEAVY,false).0)}v})}).collect();
        for s in senders{s.join().unwrap();}h.join().unwrap();
        assert!(wait_until(30_000,||counter(&c.queued)==q0),"queued bodies are released once skipped: {}",counter(&c.queued));
    }
    stop.store(true,Ordering::SeqCst);sampler.join().unwrap();
    let full=counter(&c.queue_full)-f0;let peak=peak.load(Ordering::SeqCst);
    assert!(full>=1,"HEAVY_QUEUE was reached and a heavy request answered -32021 busy ({rounds} rounds, peak {peak})");
    assert!(peak<=qmax,"at most HEAVY_QUEUE ({qmax}) bodies are held: {peak}");
    assert_eq!(stats(r.rpc)["queued"],1,"only the stats request itself");
    println!("QUEUED {}",serde_json::json!({"queuedMax":qmax,"peakQueued":peak,"refusedBusy":full,"rounds":rounds}));
}
/// Response pools on a node: with the heavy pool held full, a heavy batch carrying a transaction
/// is answered -32021 busy for every member and nothing runs (the transaction is not pooled);
/// with the pool free the same batch runs. An answer's reservation is its size while the reply
/// lives and is released when the reply drops.
#[test] fn a_full_heavy_pool_answers_busy_before_anything_runs(){let _g=serial();
    use pocold::rpc::{answer,Request,HEAVY_RESP_POOL};
    let dir=std::env::temp_dir().join(format!("pocold-net-pools-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let cfg=Config{dir,name:"pools".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits::default()};
    let mut n=Node::open(cfg,spec()).unwrap();let pools=n.resp_pools();
    let tx=Tx{chain_id:777940,nonce:0,max_priority:U256::from(1u64),max_fee:U256::from(10_000_000_000u64),gas:21000,to:Some(revm::primitives::Address::ZERO),value:U256::ZERO,data:Default::default()}.sign_1559(&devnet::account_key("alice"));
    let body=format!(r#"[{{"jsonrpc":"2.0","id":1,"method":"eth_sendRawTransaction","params":["0x{}"]}},{{"jsonrpc":"2.0","id":2,"method":"eth_blockNumber","params":[]}}]"#,hex(&tx));
    let req=|b:&str|{let (t,_r)=std::sync::mpsc::channel();Request::local(b.as_bytes().to_vec(),t,Instant::now()+Duration::from_secs(30),false)};
    let held=pools.hold_heavy(HEAVY_RESP_POOL-1024).unwrap();
    let busy=answer(&mut n,&req(&body));let v:serde_json::Value=serde_json::from_str(&busy.body).unwrap();
    assert!(v.as_array().map_or(false,|a|a.len()==2&&a.iter().all(|i|i["error"]["code"]==-32021)),"every member is busy: {v}");
    assert_eq!(n.pool.txs.len(),0,"nothing ran: the transaction was not pooled");drop(busy);drop(held);assert_eq!(pools.in_use().0,0);
    let ok=answer(&mut n,&req(&body));let v:serde_json::Value=serde_json::from_str(&ok.body).unwrap();
    assert!(v[0]["result"].is_string(),"with the pool free the batch runs: {v}");assert_eq!(n.pool.txs.len(),1);
    assert_eq!(pools.in_use().0,ok.body.capacity(),"the reservation shrank to the answer");drop(ok);assert_eq!(pools.in_use().0,0,"released with the reply");
}
fn hex(b:&[u8])->String{b.iter().map(|x|format!("{x:02x}")).collect()}
/// Light/heavy scheduling, deterministically (review F2, F9): a turn runs at most LIGHT_TURN
/// waiting light requests, then the node wakes itself for the rest, so any other event waits for
/// at most LIGHT_TURN light executions however many are queued. Light requests do overtake heavy
/// ones that arrived earlier (and run between a batch's members): HTTP requests sent concurrently
/// have no order, and a client that needs one waits for each answer.
#[test] fn a_light_turn_runs_at_most_light_turn_requests_then_wakes_the_node(){let _g=serial();
    use pocold::{node::{Event,LIGHT_TURN},rpc::Request};
    let dir=std::env::temp_dir().join(format!("pocold-net-turn-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let cfg=Config{dir,name:"turn".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits::default()};
    let mut n=Node::open(cfg,spec()).unwrap();let lane=n.light_lane();
    let (t,r)=std::sync::mpsc::channel();let queued=20;
    for _ in 0..queued{lane.tx.try_send(Event::Rpc(Request::local(LIGHT.as_bytes().to_vec(),t.clone(),Instant::now()+Duration::from_secs(30),true))).unwrap();}
    let w0=counter(&pocold::rpc::COUNTERS.wakes);
    n.drain_light();let first=r.try_iter().count();
    assert_eq!(first,LIGHT_TURN,"one turn runs LIGHT_TURN light requests");
    assert!(lane.wake.load(Ordering::SeqCst)&&counter(&pocold::rpc::COUNTERS.wakes)==w0+1,"a full turn wakes the node for the rest");
    n.drain_light();assert_eq!(r.try_iter().count(),LIGHT_TURN);n.drain_light();assert_eq!(r.try_iter().count(),queued-2*LIGHT_TURN,"the last turn takes what is left");
}
/// Review F2: under a light flood main events still progress: a new peer's handshake is answered
/// and a heavy request completes, each within a bound that does not grow with the flood (before
/// every main event at most LIGHT_TURN light requests run).
#[test] fn a_light_flood_coalesces_wakes_and_main_events_progress(){let _g=serial();
    let c=&pocold::rpc::COUNTERS;let d=heavy_duration();let r=start("flood",small());
    let (w0,p0)=(counter(&c.wakes),counter(&c.light_pushed));
    let stop=Arc::new(AtomicBool::new(false));
    let flood:Vec<_>=(0..16).map(|_|{let (p,stop)=(r.rpc,stop.clone());std::thread::spawn(move||{let mut n=0;while !stop.load(Ordering::SeqCst){if rpc(p,LIGHT,false).0==200{n+=1}}n})}).collect();
    std::thread::sleep(Duration::from_millis(500));
    // A peer's handshake is a main event: Connected makes the node send its Hello.
    let t=Instant::now();let mut s=connect(&r);s.write_all(&hello(&r,"flood-peer")).unwrap();
    let mut len=[0u8;4];s.read_exact(&mut len).unwrap();let mut f=vec![0u8;u32::from_be_bytes(len) as usize];s.read_exact(&mut f).unwrap();
    let hello_at=t.elapsed();assert!(matches!(Msg::decode(&f),Ok(Msg::Hello{..})),"the node answers the handshake");
    let (hc,ht)=timed(r.rpc,HEAVY);
    stop.store(true,Ordering::SeqCst);let served:usize=flood.into_iter().map(|h|h.join().unwrap()).sum();
    let (wakes,pushed)=(counter(&c.wakes)-w0,counter(&c.light_pushed)-p0);
    println!("FLOOD {}",serde_json::json!({"lightServed":served,"lightPushed":pushed,"wakes":wakes,"helloMs":hello_at.as_millis() as u64,"heavyMs":ht.as_millis() as u64,"heavyAloneMs":d.as_millis() as u64}));
    assert!(served>=100,"the flood ran: {served}");
    // How many coalesce depends on how fast the node drains (build speed): only the bound is asserted
    // here; coalescing itself is checked deterministically by light_lane_wakes_coalesce_until_taken.
    assert!(wakes>=1&&wakes<=pushed,"{wakes} Wakes for {pushed} light requests");
    assert!(hello_at<Duration::from_secs(2),"the handshake progressed under the flood: {hello_at:?}");
    assert_eq!(hc,200);assert!(ht<d*4+Duration::from_secs(1),"the heavy request progressed under the flood: {ht:?} (alone {d:?})");
}
/// Review F7: the miner is joined before it is replaced, so a mining node never has two miner
/// threads. The node mines alone (fallback templates) until height 2; every new template replaces
/// the miner (at least two start). Exact: the handle high-water mark is 1. Cross-check: /proc is
/// sampled for threads named "miner". The node stops at that height, so it does not load the tests
/// after this one; on a slow host it only takes longer.
#[test] fn miner_replacement_keeps_at_most_one_miner_thread(){let _g=serial();
    // /proc sampling is a Linux cross-check; the handle high-water mark is exact on every platform.
    let miners=||->Vec<String>{if !cfg!(target_os="linux"){return vec![]}std::fs::read_dir("/proc/self/task").unwrap().filter_map(|e|{let e=e.ok()?;let n=std::fs::read_to_string(e.path().join("comm")).ok()?;(n.trim()=="miner").then(||e.file_name().to_string_lossy().to_string())}).collect()};
    if !cfg!(target_os="linux"){eprintln!("THREADS-NOT-MEASURED (miner sampling)");}
    let before=miners().len();assert_eq!(before,0,"no miner runs before the test");
    let dir=std::env::temp_dir().join(format!("pocold-net-miner-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let cfg=Config{dir,name:"miner".into(),miner:Some(devnet::miner_identity("a").key),listen:None,peers:vec![],rpc:None,stop_at:Some(2),log:false,limits:small()};
    let s0=pocold::miner::STARTS.load(Ordering::SeqCst);
    let mut node=Node::open(cfg,spec()).unwrap();node.start_network().unwrap();let run=std::thread::spawn(move||node.run());
    let (mut peak,mut seen)=(0usize,std::collections::HashSet::new());let t=Instant::now();
    while t.elapsed()<Duration::from_secs(600)&&!run.is_finished(){let m=miners();peak=peak.max(m.len());seen.extend(m);}
    assert!(run.is_finished(),"the mining node reached height 2");run.join().unwrap();
    let starts=pocold::miner::STARTS.load(Ordering::SeqCst)-s0;
    assert!(wait_until(2000,||miners().is_empty()),"the miner is joined when the node stops");
    let handles=pocold::miner::PEAK.load(Ordering::SeqCst);
    println!("MINER {}",serde_json::json!({"minerStarts":starts,"minerThreadsSeen":seen.len(),"peakConcurrentSampled":peak,"peakLiveHandles":handles}));
    assert!(starts>=2,"the miner was replaced at least once: {starts} started");
    // Exact: the handle count bounds the threads at every instant; sampling /proc is a cross-check.
    assert_eq!(handles,1,"a miner was started before the previous one was joined");
    assert!(peak<=1,"at most one miner thread at a time: {peak}");
}
/// Bounded adversarial load: 4 connections send 30M-gas eth_calls back to back, and 2 send
/// 20-member heavy batches. Each request holds the consensus thread for at most one heavy member
/// beyond the work deadline, and the queue is FIFO, so a legitimate request waits behind at most
/// one request per adversarial connection. Every legitimate request is answered with its result.
#[test] fn legitimate_requests_progress_under_bounded_heavy_load(){let _g=serial();
    let mut l=small();l.rpc.work_deadline=Duration::from_millis(2500);let r=start("progress",l);
    let (code,heavy)=timed(r.rpc,HEAVY);assert_eq!(code,200);
    let stop=std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let batch=format!("[{}]",vec![HEAVY;20].join(","));
    let adv:Vec<_>=(0..6).map(|i|{let (p,stop,body)=(r.rpc,stop.clone(),if i<4{HEAVY.to_string()}else{batch.clone()});
        std::thread::spawn(move||{let mut n=0;while !stop.load(Ordering::SeqCst){rpc(p,&body,false);n+=1}n})}).collect();
    std::thread::sleep(Duration::from_millis(300));
    let t0=Instant::now();let mut legit=Vec::new();
    while t0.elapsed()<Duration::from_secs(12){legit.push(timed(r.rpc,LIGHT));std::thread::sleep(Duration::from_millis(100));}
    stop.store(true,Ordering::SeqCst);let rounds:usize=adv.into_iter().map(|h|h.join().unwrap()).sum();
    let worst=legit.iter().map(|x|x.1).max().unwrap();
    // FIFO: at most one request per adversarial connection ahead, each <= deadline + one heavy member.
    let bound=Duration::from_millis(2500)+heavy*2;
    let ok=legit.iter().filter(|x|x.0==200).count();
    println!("PROGRESS {}",serde_json::json!({"heavyMs":heavy.as_millis() as u64,"legitRequests":legit.len(),"legitOk":ok,"worstMs":worst.as_millis() as u64,"adversaryRounds":rounds}));
    assert!(legit.len()>=4&&rounds>=6,"the load ran: {} legit, {rounds} adversarial",legit.len());
    assert_eq!(ok,legit.len(),"every legitimate request is answered: {legit:?}");
    assert!(worst<=bound,"worst legitimate latency {worst:?} > {bound:?}");
}
/// Review F2, deterministically: while a Wake is pending, further light requests send none; once
/// the node takes it (the run loop clears the flag), the next request sends one again.
#[test] fn light_lane_wakes_coalesce_until_taken(){let _g=serial();
    let dir=std::env::temp_dir().join(format!("pocold-net-wake-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let cfg=Config{dir,name:"wake".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits::default()};
    let n=Node::open(cfg,spec()).unwrap();let lane=n.light_lane();let w0=counter(&pocold::rpc::COUNTERS.wakes);
    for _ in 0..5{lane.wake(&n.tx);}
    assert_eq!(counter(&pocold::rpc::COUNTERS.wakes)-w0,1,"five light requests, one pending Wake");
    lane.wake.store(false,Ordering::SeqCst);lane.wake(&n.tx);
    assert_eq!(counter(&pocold::rpc::COUNTERS.wakes)-w0,2,"after the node takes it, the next request wakes it again");
}
/// Response pools, end to end: an answer is reserved until its connection has written it. A heavy
/// batch whose answer (about 16 MiB) is larger than the socket buffers is sent to a client that
/// does not read: while it waits, the heavy pool holds the answer; once the client reads it all,
/// the reservation is gone. (A regression that releases the reservation before the write fails.)
#[test] fn an_answer_is_reserved_until_its_connection_has_written_it(){let _g=serial();
    let mut l=small();l.rpc.write_deadline=Duration::from_secs(30);l.rpc.work_deadline=Duration::from_secs(60);let r=start("resv",l);
    // Each member reverts with 1 MiB of memory (PUSH3 0x100000 PUSH1 0 REVERT), answered as
    // "execution reverted:<hex>": 2 MiB per member; the batch stops at BATCH_RESP_MAX (about 14 MiB).
    // An unread socket takes the send buffer (tcp_wmem max) plus the receiver's unread window.
    let member=r#"{"jsonrpc":"2.0","id":1,"method":"eth_call","params":[{"data":"0x621000006000fd","gas":"0x1c9c380"},"latest"]}"#;
    let body=format!("[{}]",vec![member;10].join(","));
    let mut s=unread_client(r.rpc);
    write!(s,"POST / HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",body.len()).unwrap();
    // Wait until the answer is built and its write is blocked on the unread socket.
    let want=(pocold::rpc::RESP_MAX as u64)+(12<<20);
    let built=wait_until(60_000,||stats(r.rpc)["heavyReserved"].as_u64().unwrap_or(0)>=want);
    // The first sample can land just after the answer is built; the reservation must still be there
    // a second later, while the write waits for the client.
    std::thread::sleep(Duration::from_secs(1));let during=stats(r.rpc)["heavyReserved"].as_u64().unwrap();
    assert!(built&&during>=want,"while the client does not read, the pool holds its answer: built={built}, {during} B reserved");
    let mut out=Vec::new();s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();let _=s.read_to_end(&mut out);
    assert!(out.len()>(12<<20),"the client reads the whole answer: {} B",out.len());
    assert!(wait_until(10_000,||stats(r.rpc)["heavyReserved"].as_u64()==Some(pocold::rpc::RESP_MAX as u64)),"after the write the reservation is released (only the stats request's own remains)");
    println!("RESERVED {}",serde_json::json!({"reservedWhileUnread":during,"answerBytes":out.len()}));
}

// ---------------- round 2 (rpcguard track): the RpcGuard mechanisms, on real loopback sockets ----------------
/// Reads the whole JSON-RPC body of one request and parses it.
fn rpc_json(port:u16,body:&str)->serde_json::Value{let (_,b)=rpc(port,body,false);serde_json::from_str::<serde_json::Value>(&b).unwrap_or(serde_json::Value::Null)}
/// An eth_call (or eth_estimateGas) to the BLAKE2F precompile (0x09) with `rounds` rounds: one
/// precompile run that no stop hook can cut short, of a length set by its gas (1 gas per round).
fn blake(rounds:u32,method:&str)->String{
    let input=format!("{rounds:08x}{}01","00".repeat(64+128+16));
    format!(r#"{{"jsonrpc":"2.0","id":5,"method":"{method}","params":[{{"to":"0x0000000000000000000000000000000000000009","data":"0x{input}","gas":"0x1c9c380"}},"latest"]}}"#)
}
/// The longest single uninterruptible heavy request here: eth_call BLAKE2F with 29.9M rounds (30M gas).
fn blake_max()->String{blake(29_900_000,"eth_call")}
/// How long `body` holds the consensus thread on this host and build: the shorter of two warm runs on
/// a node with wide limits. Timings under test are fractions of it.
fn duration_of(tag:&str,body:&str)->Duration{
    let mut wide=small();wide.rpc.work_deadline=Duration::from_secs(60);wide.rpc.compute_heavy=Duration::from_secs(60);let w=start(tag,wide);
    (0..2).map(|_|{let t=Instant::now();let v=rpc_json(w.rpc,body);assert!(v["result"].is_string(),"{v}");t.elapsed()}).min().unwrap()
}
/// Starts `body` on a thread and returns once the consensus thread is running it (the heavyActive gauge).
fn hold_with(port:u16,body:&str)->std::thread::JoinHandle<serde_json::Value>{
    let (b0,body)=(counter(&pocold::rpc::COUNTERS.heavy_active),body.to_string());let h=std::thread::spawn(move||rpc_json(port,&body));
    assert!(wait_until(10_000,||counter(&pocold::rpc::COUNTERS.heavy_active)>b0),"the holder runs");h
}
fn busy_of(v:&serde_json::Value)->bool{v["error"]["code"]==-32021&&v["error"]["data"]["reason"]=="busy"&&(250..=2000).contains(&v["error"]["data"]["retryAfterMs"].as_u64().unwrap_or(0))}
/// ~14 MiB heavy batch (each member reverts with 1 MiB), larger than the socket buffers, so a
/// non-reading client's write blocks and A stays far below the response size.
fn pacer_batch()->String{
    let member=r#"{"jsonrpc":"2.0","id":1,"method":"eth_call","params":[{"data":"0x621000006000fd","gas":"0x1c9c380"},"latest"]}"#;
    format!("[{}]",vec![member;10].join(","))
}
/// SendPacer (network.md:237-256): a client that does not read is closed after GRACE (A below
/// MIN_SEND_RATE·sendTime), within ENFORCE_SLACK of the violation instant (CloseOK, recorded in
/// pacerLateMaxMs and lastPacerClose); an honest reader gets the whole answer, byte for byte, with no
/// pacer close, and fast. MIN_SEND_RATE is raised and GRACE shortened so the bound is reached quickly.
#[test] fn send_pacer_closes_a_slow_reader_after_grace(){let _g=serial();
    // R = 1 MiB/s: with SO_SNDBUF set (SEND_SOCK_BUF) a non-reading client takes only the kernel send
    // buffer plus its receive window (well under R·GRACE = 512 KiB), so the pacer closes it at GRACE;
    // an honest reader is far above R (the pacer waits for writability, then writes the next chunk).
    let mut l=small();l.rpc.min_send_rate=1<<20;l.rpc.grace=Duration::from_millis(500);l.rpc.enforce_slack=Duration::from_millis(200);
    l.rpc.work_deadline=Duration::from_secs(60);l.rpc.write_deadline=Duration::from_secs(60);
    let r=start("pacer",l);let c=&pocold::rpc::COUNTERS;
    let body=pacer_batch();
    // Review 2026-10-27: the time to the answer's first byte, which the pacer's own figures do not cover. An
    // honest client reads the same batch first: its first byte comes after the batch is executed, which this
    // host's load sets. The slow reader's first byte must come about as soon (below).
    let honest_first={let mut h=TcpStream::connect(("127.0.0.1",r.rpc)).unwrap();h.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
        let t=Instant::now();write!(h,"POST / HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",body.len()).unwrap();
        let mut b=[0u8;1];assert_eq!(h.read(&mut b).unwrap(),1,"an honest reader gets the answer");let first=t.elapsed();let mut rest=Vec::new();let _=h.read_to_end(&mut rest);first};
    let (p0,w0)=(counter(&c.pacer_closed),counter(&c.pacer_waits));c.pacer_late_max_ms.store(0,Ordering::SeqCst);
    let mut s=unread_client(r.rpc);
    write!(s,"POST / HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",body.len()).unwrap();
    // Do not read. Poll pacer_closed and record when the pacer fires. The time from the request includes
    // executing the batch before its answer is sent (0.6-0.9 s on Linux): Windows run 40522024488
    // (palcertcom/pocol) saw no close within the 5 s this wait had. So it runs to 20 s, and the bounds below
    // are on the send time, from the first byte sent: the violation instant is s_v = max(GRACE, A/R), with A
    // what the kernel accepted for this client. The client also peeks (reads nothing, so its receive window
    // stays closed) to see when the first byte reached it, for the bounds the client sees for itself.
    let t=Instant::now();let mut first=None;s.set_nonblocking(true).unwrap();
    while counter(&c.pacer_closed)==p0&&t.elapsed()<Duration::from_secs(20){
        if first.is_none(){let mut b=[0u8;1];if matches!(s.peek(&mut b),Ok(n) if n>0){first=Some(t.elapsed());}}
        std::thread::sleep(Duration::from_millis(5));
    }
    let closed_at=t.elapsed();s.set_nonblocking(false).unwrap();let (late,waits)=(counter(&c.pacer_late_max_ms),counter(&c.pacer_waits)-w0);
    let st=rpc_json(r.rpc,r#"{"jsonrpc":"2.0","id":2,"method":"pocol_getRpcStats","params":[]}"#)["result"].clone();
    assert!(counter(&c.pacer_closed)>p0,"the pacer closed the non-reading client (after {closed_at:?}; stats {st})");
    let (send_time,accepted)=(st["lastPacerClose"]["sendTime"].as_f64().unwrap(),st["lastPacerClose"]["accepted"].as_u64().unwrap());
    let s_v=(accepted as f64/(1u64<<20) as f64).max(0.5);
    // Written to stdout directly, as ABSORB is, so A and s_v reach the log of a passing run too.
    let first=first.expect("the answer's first byte reached the slow reader before the close");
    let window=closed_at.saturating_sub(first).as_secs_f64();
    let line=format!("PACER {}\n",serde_json::json!({"os":std::env::consts::OS,"closedMs":closed_at.as_millis() as u64,"sendTimeS":send_time,
        "acceptedBytes":accepted,"violationS":s_v,"pacerWaits":waits,"pacerLateMaxMs":late,"firstByteMs":first.as_millis() as u64,
        "honestFirstByteMs":honest_first.as_millis() as u64,"clientWindowS":window}));
    let _=std::io::Write::write_all(&mut std::io::stdout(),line.as_bytes());
    // Review 2 (RG2-M2): a blocked write waits for writability, waking only at the backstop and at the
    // violation instant: about one wait per PACER_BACKSTOP of send time, not one per millisecond.
    let backstop=pocold::rpc::PACER_BACKSTOP.as_millis() as usize;
    let bound=2*((send_time*1000.0) as usize/backstop)+10;
    assert!(waits<=bound,"the pacer waited {waits} times for a client that does not read (bound {bound})");
    // Review 3 (RG3V-m2): and it does wake at the backstop (network.md:246), about every 50 ms before
    // GRACE, not only at the timer: at least half of GRACE/PACER_BACKSTOP waits.
    let floor=500/backstop/2;
    assert!(waits>=floor,"the pacer waited {waits} times before GRACE: the backstop check is missing (floor {floor})");
    // Seen from the client, independently of the pacer's own figures: the close came GRACE or more after the
    // first byte, and within ENFORCE_SLACK of s_v (0.3 s more for the 5 ms polls and the scheduling of this
    // thread). And the first byte came about as soon as an honest reader's of the same batch: a delay before
    // the pacer starts its clock would not show in sendTime (review 2026-10-27, a 4 s sleep before t0).
    assert!(window>=0.45,"not before GRACE, seen from the client: {window} s from the first byte to the close");
    assert!(window<=s_v+0.2+0.3,"within ENFORCE_SLACK of s_v, seen from the client: {window} s from the first byte, s_v = {s_v} s");
    assert!(first<=honest_first*2+Duration::from_secs(1),"the slow reader's first byte came {first:?} after its request, an honest reader's {honest_first:?}");
    assert!(send_time<s_v+1.0,"after the violation instant, promptly: send time {send_time} s, s_v = {s_v} s");
    // ENFORCE_SLACK: the close came within 200 ms of the violation instant s_v (TIMER_EPS + scheduling).
    assert!(late<=200,"pacer lateness {late} ms > ENFORCE_SLACK");
    assert!(send_time>0.5&&(accepted as f64)<(1u64<<20) as f64*send_time,"CloseOK: sendTime > GRACE and A < R·sendTime: {send_time} s, {accepted} B");
    s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();assert!(closed(&mut s),"the slow reader's connection is closed");
    // Honest reader: a client that reads promptly gets the whole ~14 MiB answer (sendDone, no close),
    // and it is the complete answer: ten members, each a 1 MiB revert payload.
    let (p1,sd0)=(counter(&c.pacer_closed),counter(&c.send_done));c.batch_send_max_ms.store(0,Ordering::SeqCst);
    let mut s2=TcpStream::connect(("127.0.0.1",r.rpc)).unwrap();s2.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
    write!(s2,"POST / HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",body.len()).unwrap();
    let mut out=Vec::new();let _=s2.read_to_end(&mut out);
    let at=out.windows(4).position(|w|w==b"\r\n\r\n").expect("an HTTP answer")+4;
    let len:usize=String::from_utf8_lossy(&out[..at]).lines().find_map(|h|h.strip_prefix("Content-Length: ").map(|x|x.trim().parse().unwrap())).unwrap();
    assert_eq!(out.len()-at,len,"every byte of the answer arrived");
    let v:serde_json::Value=serde_json::from_slice(&out[at..]).expect("the answer is complete JSON");let a=v.as_array().unwrap();
    assert_eq!(a.len(),10);let first=a[0]["error"]["message"].as_str().unwrap().to_string();assert_eq!(first.len(),"execution reverted:0x".len()+2*(1<<20));
    assert!(a.iter().filter(|m|m["error"]["message"].as_str()==Some(first.as_str())).count()>=6,"the members are the identical revert payload (the tail may be batchBytes)");
    assert_eq!(counter(&c.pacer_closed),p1,"the honest reader is not closed by the pacer");
    assert!(counter(&c.send_done)>sd0,"sendDone recorded for the honest reader");
    // Review 3 (RG3V-M2): and quickly. The pacer's send time for the ~14 MiB answer (batchSendMaxS, from
    // the first byte to sendDone) is within 2 s, at least 7 MiB/s: a loopback reader takes it at over
    // 1 GiB/s here, while writes larger than SEND_SOCK_BUF (no write cap) or a pacer that waits out its
    // backstop on every block run at about 2 MiB/s (RPCGUARD.md §4a).
    let sent=counter(&c.batch_send_max_ms);
    println!("HONEST {}",serde_json::json!({"answerBytes":out.len()-at,"sendMs":sent}));
    assert!(sent<=2000,"the honest reader's ~14 MiB answer took {sent} ms to send");
}
/// Review R2-4: bytes after the request (a trailing CRLF that some clients add, or a pipelined request,
/// which is never served: network.md:187) are drained before the close, so the close does not reset the
/// connection. Before, the client of a large answer saw `Connection reset by peer` after the body on
/// every try. The ~14 MiB batch answer is read in full, without an error, after 0 and 300 ms.
#[test] fn bytes_after_the_request_do_not_reset_the_answer(){let _g=serial();
    let mut l=small();l.rpc.work_deadline=Duration::from_secs(60);l.rpc.write_deadline=Duration::from_secs(120);let r=start("trailing",l);
    let body=pacer_batch();
    for (tail,wait) in [(&b"\r\n"[..],0u64),(&b"\r\n"[..],300),(&b"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 2\r\n\r\n{}"[..],300)]{
        let mut s=TcpStream::connect(("127.0.0.1",r.rpc)).unwrap();s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
        s.write_all(format!("POST / HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",body.len()).as_bytes()).unwrap();
        std::thread::sleep(Duration::from_millis(30));s.write_all(tail).unwrap();std::thread::sleep(Duration::from_millis(wait));
        let mut out=Vec::new();let res=s.read_to_end(&mut out);
        let at=out.windows(4).position(|w|w==b"\r\n\r\n").expect("an HTTP answer")+4;
        let len:usize=String::from_utf8_lossy(&out[..at]).lines().find_map(|h|h.strip_prefix("Content-Length: ").map(|x|x.trim().parse().unwrap())).unwrap();
        assert!(res.is_ok(),"tail {:?}, wait {wait} ms: {:?} after {} of {len} body bytes",String::from_utf8_lossy(tail),res.err(),out.len()-at);
        assert_eq!(out.len()-at,len,"every byte of the answer arrived");assert!(len>10<<20,"the large answer: {len} B");
    }
}
/// Review 3 (RG3V-M1): the pacer's timer is at s_v + TIMER_EPS with s_v = max(GRACE, A/R) and is
/// re-armed on every change of A (network.md:243-246). A reader above R keeps A/R ahead of sendTime
/// past GRACE: its blocked writes wait for writability or the backstop. A timer at GRACE alone would
/// make every block past GRACE a zero wait (a busy loop). R = 256 KiB/s and GRACE = 200 ms; the client
/// reads 64 KiB every 100 ms (640 KiB/s) for 1.5 s past GRACE, without a pacer close and within a
/// bound on the waits, then stops reading: the pacer closes it once sendTime passes A/R (CloseOK,
/// lateness within ENFORCE_SLACK), long after GRACE.
#[test] fn send_pacer_timer_follows_a_over_r_past_grace(){let _g=serial();
    let (rate,grace)=(256u64<<10,Duration::from_millis(200));
    let mut l=small();l.rpc.min_send_rate=rate;l.rpc.grace=grace;l.rpc.enforce_slack=Duration::from_millis(200);
    l.rpc.work_deadline=Duration::from_secs(60);l.rpc.write_deadline=Duration::from_secs(120);
    let r=start("pacer-ar",l);let c=&pocold::rpc::COUNTERS;let p0=counter(&c.pacer_closed);c.pacer_late_max_ms.store(0,Ordering::SeqCst);
    let body=pacer_batch();let mut s=unread_client(r.rpc);s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
    write!(s,"POST / HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",body.len()).unwrap();
    let mut buf=vec![0u8;64<<10];
    let mut chunk=|s:&mut TcpStream|{let mut k=0;while k<buf.len(){match s.read(&mut buf[k..]){Ok(0)=>break,Ok(n)=>k+=n,Err(e)=>panic!("read: {e}")}}k};
    // sendTime starts with the first byte: the first chunk marks it (within a few ms).
    let mut got=chunk(&mut s);let t0=Instant::now();let (from,to)=(grace+Duration::from_millis(100),grace+Duration::from_millis(1600));
    let mut win=None;
    while t0.elapsed()<to{
        std::thread::sleep(Duration::from_millis(100));
        if win.is_none()&&t0.elapsed()>=from{win=Some((t0.elapsed(),counter(&c.pacer_waits),got));}
        got+=chunk(&mut s);
    }
    let (w_from,w0,g0)=win.unwrap();let (secs,waits,read)=((t0.elapsed()-w_from).as_secs_f64(),counter(&c.pacer_waits)-w0,got-g0);
    assert_eq!(counter(&c.pacer_closed),p0,"a reader above R is not closed by the pacer");
    // Each wait ends at the backstop (<= 1 per 50 ms) or with the socket writable, after which the pacer
    // writes what the socket takes (allowed here: one wait per KiB read). A zero wait loops far above it.
    let bound=2*(secs/pocold::rpc::PACER_BACKSTOP.as_secs_f64()) as usize+read/1024+10;
    println!("PACER-AR {}",serde_json::json!({"windowS":secs,"pacerWaits":waits,"bytesRead":read,"bound":bound}));
    assert!(waits<=bound,"past GRACE with A/R ahead of sendTime the pacer waited {waits} times in {secs:.2} s (bound {bound})");
    // Stop reading: A stops growing, and the pacer closes once sendTime passes A/R.
    let stop=Instant::now();while counter(&c.pacer_closed)==p0&&stop.elapsed()<Duration::from_secs(90){std::thread::sleep(Duration::from_millis(10));}
    assert!(counter(&c.pacer_closed)>p0,"the pacer closed the reader once it stopped");
    let st=rpc_json(r.rpc,r#"{"jsonrpc":"2.0","id":2,"method":"pocol_getRpcStats","params":[]}"#)["result"].clone();
    let (send_time,accepted)=(st["lastPacerClose"]["sendTime"].as_f64().unwrap(),st["lastPacerClose"]["accepted"].as_u64().unwrap() as f64);
    let late=counter(&c.pacer_late_max_ms);
    println!("PACER-AR-CLOSE {}",serde_json::json!({"lastPacerClose":st["lastPacerClose"],"pacerLateMaxMs":late}));
    assert!(accepted<rate as f64*send_time,"CloseOK: A < R·sendTime: {send_time} s, {accepted} B");
    assert!(send_time>(to-Duration::from_millis(100)).as_secs_f64()&&accepted>=got as f64,"closed at A/R, long past GRACE: {send_time} s, A {accepted} B, read {got} B");
    assert!(send_time-accepted/rate as f64<=0.2&&late<=200,"within ENFORCE_SLACK of s_v = A/R: {send_time} s, A/R {} s, lateness {late} ms",accepted/rate as f64);
}
/// The per-connection token bucket (network.md:215-218): a connection carries one request, which
/// takes k tokens from a full bucket of bucket_cap; more than that is -32021 {rate, retryAfterMs}
/// for every member without running. With bucket_cap 5 (the spec's is 20 = BATCH_MAX, which no
/// batch within BATCH_MAX exceeds): 6 members are refused, 5 are served; a batch over BATCH_MAX is
/// -32600, not rate.
#[test] fn the_per_connection_bucket_charges_k_tokens_per_request(){let _g=serial();
    let mut l=small();l.rpc.rate=20;l.rpc.bucket_cap=5;let r=start("rate",l);let c=&pocold::rpc::COUNTERS;let r0=counter(&c.rate);
    let batch=|k:usize|format!("[{}]",(0..k).map(|i|format!(r#"{{"jsonrpc":"2.0","id":{i},"method":"eth_chainId","params":[]}}"#)).collect::<Vec<_>>().join(","));
    let v=rpc_json(r.rpc,&batch(6));let a=v.as_array().unwrap();assert_eq!(a.len(),6);
    for (i,x) in a.iter().enumerate(){assert_eq!(x["error"]["code"],-32021,"{x}");assert_eq!(x["error"]["data"]["reason"],"rate");assert_eq!(x["error"]["data"]["retryAfterMs"],50);assert_eq!(x["id"],i as u64);}
    assert_eq!(counter(&c.rate)-r0,1,"one rate-limited request");
    let v=rpc_json(r.rpc,&batch(5));assert!(v.as_array().unwrap().iter().all(|x|x["result"].is_string()),"5 tokens fit: {v}");
    assert!(rpc_json(r.rpc,LIGHT)["result"].is_string(),"a singleton takes one token");
    assert_eq!(rpc_json(r.rpc,&batch(21))["error"]["code"],-32600,"a batch over BATCH_MAX is -32600, never rate");
}
/// COMPUTE_DEADLINE, light lane (network.md:235): a light request admitted past its (here zero)
/// light deadline is -32020 {deadline} without running its method. Deterministic (no timing margin).
#[test] fn light_compute_deadline_fires(){let _g=serial();
    let mut l=small();l.rpc.compute_light=Duration::ZERO;let r=start("lightdl",l);
    let v=rpc_json(r.rpc,LIGHT);// eth_blockNumber is a light method
    assert_eq!(v["error"]["code"],-32020,"the light deadline fires: {v}");assert_eq!(v["error"]["data"]["reason"],"deadline");
    // A heavy method on the same node (COMPUTE_HEAVY is the default 5 s) still answers.
    assert!(rpc_json(r.rpc,r#"{"jsonrpc":"2.0","id":1,"method":"pocol_status","params":[]}"#)["result"].is_object(),"heavy methods are unaffected");
}
/// COMPUTE_DEADLINE, heavy (network.md:235, :286): an eth_call that runs past COMPUTE_HEAVY is ended
/// by the simulation stop hook and answered -32020 {deadline}. The infinite-loop call runs for ~d;
/// the deadline is a fraction of it, so it always fires with a wide margin. Round 3: a late answer of
/// any kind is also -32020 (`member`), so the test checks that the stop hook ended the run: the answer
/// comes long before the run would have ended.
#[test] fn heavy_compute_deadline_cancels_eth_call(){let _g=serial();
    let d=heavy_duration();let dl=(d/8).max(Duration::from_millis(3));let mut l=small();l.rpc.compute_heavy=dl;l.rpc.work_deadline=Duration::from_secs(30);
    let r=start("heavydl",l);
    let t=Instant::now();let v=rpc_json(r.rpc,HEAVY);let took=t.elapsed();// 30M-gas JUMP loop
    assert_eq!(v["error"]["code"],-32020,"eth_call past COMPUTE_HEAVY is -32020: {v}");assert_eq!(v["error"]["data"]["reason"],"deadline");
    // d is the shorter of two whole runs (with their HTTP round trip); a run that is not stopped takes it.
    assert!(took<d*3/4,"the stop hook ended the run at its deadline ({dl:?}), not at its end ({d:?}): {took:?}");
    println!("HEAVYDL {}",serde_json::json!({"heavyMs":d.as_millis() as u64,"computeHeavyMs":dl.as_millis() as u64,"answeredMs":took.as_millis() as u64}));
}
/// Review B3: a precompile run is not interruptible, so the stop hook cannot end it at the deadline;
/// the answer is still -32020 {deadline}, never a late success (the deadline is checked again once
/// the answer is encoded). cancelMaxMs records how late the -32020 came: the uninterruptible span
/// pocold does not bound (CANCEL_MAX is reported absent).
#[test] fn a_precompile_call_past_its_deadline_is_32020_not_a_late_success(){let _g=serial();
    let mut l=small();l.rpc.compute_heavy=Duration::from_millis(20);l.rpc.work_deadline=Duration::from_secs(60);let r=start("blakedl",l);
    let c=&pocold::rpc::COUNTERS;c.cancel_late_max_ms.store(0,Ordering::SeqCst);
    let t=Instant::now();let v=rpc_json(r.rpc,&blake_max());let took=t.elapsed();
    assert_eq!(v["error"]["code"],-32020,"a late precompile result is -32020: {v}");assert_eq!(v["error"]["data"]["reason"],"deadline");assert!(v.get("result").is_none());
    let late=counter(&c.cancel_late_max_ms);
    assert_eq!(rpc_json(r.rpc,r#"{"jsonrpc":"2.0","id":2,"method":"pocol_getRpcStats","params":[]}"#)["result"]["cancelMaxMs"],late as u64);
    println!("BLAKEDL {}",serde_json::json!({"answeredMs":took.as_millis() as u64,"computeHeavyMs":20,"cancelLateMs":late}));
}
/// Review B1: COMPUTE_DEADLINE runs from admission, not from arrival (network.md:235), and QUEUE_WAIT
/// bounds the wait before admission (network.md:234). In process, deterministically: a light and a
/// heavy request that waited longer than their compute deadline but less than QUEUE_WAIT are
/// answered with their results; one that waited past QUEUE_WAIT is -32021 busy without running (a
/// transaction is not pooled).
#[test] fn compute_deadline_runs_from_admission_and_queue_wait_bounds_admission(){let _g=serial();
    use pocold::rpc::{answer,Request};
    let dir=std::env::temp_dir().join(format!("pocold-net-admit-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let mut l=Limits::default();l.rpc.compute_light=Duration::from_millis(50);l.rpc.compute_heavy=Duration::from_millis(50);l.rpc.queue_wait=Duration::from_millis(1500);
    let cfg=Config{dir,name:"admit".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:l};
    let mut n=Node::open(cfg,spec()).unwrap();
    let req=|b:&str,light:bool|{let (t,_r)=std::sync::mpsc::channel();Request::local(b.as_bytes().to_vec(),t,Instant::now()+Duration::from_secs(30),light)};
    let (lq,hq)=(req(LIGHT,true),req(r#"{"jsonrpc":"2.0","id":3,"method":"pocol_status","params":[]}"#,false));
    std::thread::sleep(Duration::from_millis(250));// 5x the compute deadlines, a sixth of QUEUE_WAIT
    let v:serde_json::Value=serde_json::from_str(&answer(&mut n,&lq).body).unwrap();assert!(v["result"].is_string(),"a light request queued past its compute deadline runs: {v}");
    let v:serde_json::Value=serde_json::from_str(&answer(&mut n,&hq).body).unwrap();assert!(v["result"].is_object(),"so does a heavy one: {v}");
    let tx=Tx{chain_id:777940,nonce:0,max_priority:U256::from(1u64),max_fee:U256::from(10_000_000_000u64),gas:21000,to:Some(revm::primitives::Address::ZERO),value:U256::ZERO,data:Default::default()}.sign_1559(&devnet::account_key("alice"));
    let late=req(&format!(r#"{{"jsonrpc":"2.0","id":4,"method":"eth_sendRawTransaction","params":["0x{}"]}}"#,hex(&tx)),true);
    let w0=counter(&pocold::rpc::COUNTERS.queue_wait);std::thread::sleep(Duration::from_millis(1600));
    let v:serde_json::Value=serde_json::from_str(&answer(&mut n,&late).body).unwrap();
    assert!(busy_of(&v)&&v["id"]==4,"not admitted within QUEUE_WAIT: -32021 busy: {v}");assert_eq!(n.pool.txs.len(),0,"and it never ran");
    assert_eq!(counter(&pocold::rpc::COUNTERS.queue_wait)-w0,1);
}
/// COMPUTE_DEADLINE at the handler entry (network.md:235): a request whose deadline has passed when
/// its handler starts is -32020 {deadline} and its method never runs. A transaction (a method whose
/// effect would make its late answer be kept) is not pooled; with time left it is. In process.
#[test] fn a_request_reaching_its_handler_past_the_deadline_never_runs(){let _g=serial();
    use pocold::rpc::{answer,Request};
    let dir=std::env::temp_dir().join(format!("pocold-net-entry-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let mut l=Limits::default();l.rpc.compute_light=Duration::ZERO;
    let cfg=Config{dir,name:"entry".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:l};
    let mut n=Node::open(cfg,spec()).unwrap();
    let tx=Tx{chain_id:777940,nonce:0,max_priority:U256::from(1u64),max_fee:U256::from(10_000_000_000u64),gas:21000,to:Some(revm::primitives::Address::ZERO),value:U256::ZERO,data:Default::default()}.sign_1559(&devnet::account_key("alice"));
    let body=format!(r#"{{"jsonrpc":"2.0","id":6,"method":"eth_sendRawTransaction","params":["0x{}"]}}"#,hex(&tx));
    let req=||{let (t,_r)=std::sync::mpsc::channel();Request::local(body.as_bytes().to_vec(),t,Instant::now()+Duration::from_secs(30),true)};
    let v:serde_json::Value=serde_json::from_str(&answer(&mut n,&req()).body).unwrap();
    assert_eq!(v["error"]["code"],-32020,"{v}");assert_eq!(v["error"]["data"]["reason"],"deadline");assert_eq!(n.pool.txs.len(),0,"the method never ran");
    n.cfg.limits.rpc.compute_light=Duration::from_secs(5);
    let v:serde_json::Value=serde_json::from_str(&answer(&mut n,&req()).body).unwrap();assert!(v["result"].is_string(),"{v}");assert_eq!(n.pool.txs.len(),1);
}
/// Review R2-2: the pool admission of eth_sendRawTransaction runs under the request's COMPUTE_DEADLINE
/// (network.md RpcGuard (D) step 4, (F)). An interpreted execution is stopped at its next poll; a
/// precompile run, which no poll can cut short, is checked again before the commit. Either way the
/// transaction is refused with -32020 {deadline} and nothing is pooled. In process, deterministically.
#[test] fn a_transaction_admission_past_its_compute_deadline_is_refused_and_not_pooled(){let _g=serial();
    use pocold::rpc::{answer,Request};
    let dir=std::env::temp_dir().join(format!("pocold-net-admitdl-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let cfg=Config{dir,name:"admitdl".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits::default()};
    let mut n=Node::open(cfg,spec()).unwrap();let alice=devnet::account_key("alice");
    let tx=|nonce:u64,to:Option<revm::primitives::Address>,data:Vec<u8>|Tx{chain_id:777940,nonce,max_priority:U256::from(1u64),max_fee:U256::from(10_000_000_000u64),gas:7_900_000,to,value:U256::ZERO,data:data.into()}.sign_1559(&alice);
    // A create whose init code loops until it runs out of gas (JUMPDEST PUSH1 0 JUMP): interpreted.
    let spin=tx(0,None,vec![0x5b,0x60,0x00,0x56]);
    let t=Instant::now();let e=n.submit_tx_by(spin.clone(),false,Some(Instant::now())).unwrap_err();let stopped=t.elapsed();
    assert_eq!(e,pocold::node::SUBMIT_DEADLINE);assert_eq!(n.pool.txs.len(),0,"refused, not pooled");
    let t=Instant::now();n.submit_tx_by(spin,false,None).unwrap();let full=t.elapsed();assert_eq!(n.pool.txs.len(),1,"positive control: with no deadline it is pooled");
    assert!(stopped*4<full,"the stop hook ended the execution early ({stopped:?} against a whole run of {full:?})");
    // BLAKE2F with 7.8M rounds as the transaction itself: no poll can stop it, the check before commit does.
    let input=pocold::types::unhex(&format!("{:08x}{}01",7_800_000u32,"00".repeat(64+128+16))).unwrap();
    let blake=tx(1,Some(revm::primitives::Address::with_last_byte(9)),input);
    let e=n.submit_tx_by(blake.clone(),false,Some(Instant::now())).unwrap_err();assert_eq!(e,pocold::node::SUBMIT_DEADLINE);assert_eq!(n.pool.txs.len(),1);
    // Through the RPC: a 1 ms light deadline. The answer is -32020 {deadline} (kept, not replaced) and nothing is pooled.
    n.cfg.limits.rpc.compute_light=Duration::from_millis(1);
    let body=format!(r#"{{"jsonrpc":"2.0","id":7,"method":"eth_sendRawTransaction","params":["0x{}"]}}"#,hex(&blake));
    let req=||{let (t,_r)=std::sync::mpsc::channel();Request::local(body.as_bytes().to_vec(),t,Instant::now()+Duration::from_secs(30),true)};
    let v:serde_json::Value=serde_json::from_str(&answer(&mut n,&req()).body).unwrap();
    assert_eq!((v["error"]["code"].clone(),v["error"]["data"]["reason"].clone()),(serde_json::json!(-32020),serde_json::json!("deadline")),"{v}");assert_eq!(n.pool.txs.len(),1);
    n.cfg.limits.rpc.compute_light=Duration::from_secs(5);
    let v:serde_json::Value=serde_json::from_str(&answer(&mut n,&req()).body).unwrap();assert!(v["result"].is_string(),"positive control: with time left it is pooled: {v}");assert_eq!(n.pool.txs.len(),2);
}
/// Review B1, end to end at the default limits: a light request that waits behind more heavy work
/// than its 500 ms compute deadline is answered with its result (or busy past QUEUE_WAIT), never
/// -32020. The holder is an eth_estimateGas over BLAKE2F (about a dozen uninterruptible runs).
#[test] fn light_requests_behind_long_heavy_work_are_answered_never_deadline(){let _g=serial();
    let holder=blake(5_400_000,"eth_estimateGas");let hd=duration_of("behind-d",&holder);
    let mut l=small();l.rpc.compute_light=(hd/4).min(Duration::from_millis(500));let r=start("behind",l);
    let h=hold_with(r.rpc,&holder);let t=Instant::now();let v=rpc_json(r.rpc,LIGHT);let waited=t.elapsed();
    assert!(v["result"].is_string()||busy_of(&v),"answered (or busy past QUEUE_WAIT), never -32020: {v}");
    assert!(h.join().unwrap()["result"].is_string(),"the holder is answered too");
    println!("BEHIND {}",serde_json::json!({"holderMs":hd.as_millis() as u64,"lightWaitedMs":waited.as_millis() as u64,"answer":v}));
}
/// Review B2: QUEUE_WAIT bounds the wait of a request that holds a queue place (network.md:234). While
/// an uninterruptible heavy request holds the consensus thread, a queued heavy and a queued light
/// request are each answered -32021 {busy, retryAfterMs in [250,2000]} at QUEUE_WAIT, long before the
/// holder ends, and never run; their bodies are released when the node reaches them.
#[test] fn queued_requests_are_busy_at_queue_wait(){let _g=serial();
    let b=duration_of("qwait-d",&blake_max());let qw=(b/6).max(Duration::from_millis(20));
    let mut l=small();l.rpc.queue_wait=qw;l.rpc.work_deadline=Duration::from_secs(60);let r=start("qwait",l);let c=&pocold::rpc::COUNTERS;
    let (w0,q0)=(counter(&c.queue_wait),counter(&c.queued));
    let h=hold_with(r.rpc,&blake_max());
    let reqs:Vec<_>=[HEAVY,LIGHT].iter().map(|body|{let (p,body)=(r.rpc,body.to_string());std::thread::spawn(move||{let t=Instant::now();let v=rpc_json(p,&body);(v,t.elapsed())})}).collect();
    let res:Vec<(serde_json::Value,Duration)>=reqs.into_iter().map(|x|x.join().unwrap()).collect();
    for (v,t) in &res{assert!(busy_of(v),"busy at QUEUE_WAIT: {v}");assert!(*t>=qw&&*t<b*3/4,"answered at QUEUE_WAIT ({qw:?}), before the holder ends ({b:?}): {t:?}");}
    assert!(counter(&c.queue_wait)-w0>=2,"both counted");assert!(h.join().unwrap()["result"].is_string(),"the holder is answered");
    assert!(wait_until(10_000,||counter(&c.queued)==q0),"the queued heavy body is released when the node reaches it");
    assert!(rpc_json(r.rpc,LIGHT)["result"].is_string(),"the node serves again");
    println!("QWAIT {}",serde_json::json!({"holderMs":b.as_millis() as u64,"queueWaitMs":qw.as_millis() as u64,"answeredMs":res.iter().map(|x|x.1.as_millis() as u64).collect::<Vec<_>>()}));
}
/// LIGHT_QUEUE (network.md:214): with the consensus thread held, light requests beyond the lane's
/// places are answered -32021 {busy} at once (HTTP 200, never 503); the queued ones are answered
/// when the node reaches them.
#[test] fn a_full_light_lane_answers_busy(){let _g=serial();
    let mut l=small();l.rpc.light_queue=2;l.rpc.queue_wait=Duration::from_secs(60);l.rpc.work_deadline=Duration::from_secs(60);let r=start("lane",l);
    let c=&pocold::rpc::COUNTERS;let f0=counter(&c.light_full);
    let h=hold_with(r.rpc,&blake_max());
    let ls:Vec<_>=(0..6).map(|i|{let p=r.rpc;std::thread::spawn(move||rpc(p,&format!(r#"{{"jsonrpc":"2.0","id":{i},"method":"eth_blockNumber","params":[]}}"#),false))}).collect();
    let res:Vec<(u16,String)>=ls.into_iter().map(|x|x.join().unwrap()).collect();h.join().unwrap();
    assert!(res.iter().all(|x|x.0==200),"every answer is HTTP 200: {res:?}");
    let vs:Vec<serde_json::Value>=res.iter().map(|x|serde_json::from_str(&x.1).unwrap()).collect();
    let busy=vs.iter().filter(|v|busy_of(v)).count();let ok=vs.iter().filter(|v|v["result"].is_string()).count();
    assert!(busy>=1&&ok>=1&&busy+ok==6,"busy beyond the lane, results for the queued: {vs:?}");assert!(counter(&c.light_full)-f0>=1);
    println!("LANE {}",serde_json::json!({"busy":busy,"ok":ok}));
}
/// Heavy admission queue overflow (network.md:214, :234): with the consensus thread stalled on one
/// 30M-gas call, a small HEAVY_QUEUE filled beyond its depth answers -32021 {busy, retryAfterMs}.
#[test] fn heavy_queue_overflow_answers_busy_with_retry_after(){let _g=serial();
    let d=heavy_duration();let mut l=small();l.rpc.heavy_queue=2;l.rpc.queue_wait=Duration::from_millis(50);l.rpc.work_deadline=Duration::from_secs(60);
    let r=start("busyq",l);let c=&pocold::rpc::COUNTERS;let b0=counter(&c.busy);
    // Hold the consensus thread with one heavy call for ~d; it occupies no queue place once running.
    let holder={let p=r.rpc;std::thread::spawn(move||rpc(p,HEAVY,false).0)};std::thread::sleep(d/8);
    // While it runs, fire several more heavy calls; with a queue of 2 and a 50 ms wait, some are busy.
    let hs:Vec<_>=(0..6).map(|_|{let p=r.rpc;std::thread::spawn(move||rpc_json(p,HEAVY))}).collect();
    let res:Vec<serde_json::Value>=hs.into_iter().map(|h|h.join().unwrap()).collect();holder.join().unwrap();
    let busy:Vec<&serde_json::Value>=res.iter().filter(|v|v["error"]["code"]==-32021).collect();
    assert!(!busy.is_empty(),"a full heavy queue answers -32021 busy: {res:?}");
    for v in &busy{assert!(busy_of(v),"{v}");}
    assert!(counter(&c.busy)>=b0+busy.len(),"busy answers are counted");
    assert!(wait_until(10_000,||rpc(r.rpc,LIGHT,false).0==200),"the node serves again once the stall clears");
    println!("BUSYQ {}",serde_json::json!({"busy":busy.len(),"of":res.len()}));
}
/// BATCH_WALL (network.md:282), from read-end to sendDone: reaching it closes the connection without
/// a complete answer and counts batchWallHits, whether it is reached while members run (A) or while
/// the answer is sent to a client that does not read (B; the pacer is set not to fire first).
#[test] fn batch_wall_closes_the_batch(){let _g=serial();
    let c=&pocold::rpc::COUNTERS;let d=heavy_duration();
    let mut l=small();l.rpc.batch_wall=(d/2).max(Duration::from_millis(2));l.rpc.work_deadline=Duration::from_secs(60);let r=start("wall",l);
    let w0=counter(&c.batch_wall_hits);let (code,b)=rpc(r.rpc,&format!("[{HEAVY},{HEAVY},{LIGHT}]"),false);
    assert!(code==0&&b.is_empty(),"closed without an answer: {code} {b}");assert_eq!(counter(&c.batch_wall_hits)-w0,1);
    let mut l=small();l.rpc.batch_wall=Duration::from_secs(2);l.rpc.min_send_rate=1;l.rpc.grace=Duration::from_secs(600);l.rpc.work_deadline=Duration::from_secs(60);let r=start("wall2",l);
    let member=r#"{"jsonrpc":"2.0","id":1,"method":"eth_call","params":[{"data":"0x621000006000fd","gas":"0x1c9c380"},"latest"]}"#;let body=format!("[{}]",vec![member;10].join(","));
    let (w1,p0,s0)=(counter(&c.batch_wall_hits),counter(&c.pacer_closed),counter(&c.send_done));
    let mut s=unread_client(r.rpc);let t=Instant::now();
    write!(s,"POST / HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",body.len()).unwrap();
    assert!(wait_until(20_000,||counter(&c.batch_wall_hits)>w1),"the wall closes the send");let at=t.elapsed();
    assert!(at>=Duration::from_millis(1900)&&at<Duration::from_secs(15),"at BATCH_WALL from read-end: {at:?}");
    assert_eq!((counter(&c.pacer_closed),counter(&c.send_done)),(p0,s0),"not the pacer, and no sendDone");
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();let mut out=Vec::new();let _=s.read_to_end(&mut out);
    // The bytes the socket had accepted may still arrive; the answer itself is cut short.
    let hd=out.windows(4).position(|w|w==b"\r\n\r\n").map_or(out.len(),|i|i+4);
    let len:usize=String::from_utf8_lossy(&out[..hd]).lines().find_map(|h|h.strip_prefix("Content-Length: ").map(|x|x.trim().parse().unwrap())).unwrap_or(usize::MAX);
    assert!(out.len()-hd<len,"the answer is incomplete: {} of {len} B",out.len()-hd);
    println!("WALL {}",serde_json::json!({"computeWallMs":(d/2).as_millis() as u64,"sendWallClosedMs":at.as_millis() as u64,"bytesBeforeClose":out.len()}));
}
/// Review V7: eth_estimateGas stops its search at the first simulation the deadline stops (one
/// -32020, no further simulations). The initcode loops 250k times (6.5M gas) and succeeds, so the
/// search runs about a dozen simulations; the deadline is three of them.
#[test] fn estimate_gas_stops_its_search_at_the_deadline(){let _g=serial();
    const LOOP:&str=r#"{"jsonrpc":"2.0","id":8,"method":"%M","params":[{"data":"0x6203d0905b600190038060045700","gas":"0x7a1200"},"latest"]}"#;
    let (call,est)=(LOOP.replace("%M","eth_call"),LOOP.replace("%M","eth_estimateGas"));
    let mut wide=small();wide.rpc.work_deadline=Duration::from_secs(60);wide.rpc.compute_heavy=Duration::from_secs(60);let w=start("estimate-wide",wide);
    let t1=(0..3).map(|_|{let t=Instant::now();let v=rpc_json(w.rpc,&call);assert!(v["result"].is_string(),"{v}");t.elapsed()}).min().unwrap();
    let te=(0..2).map(|_|{let t=Instant::now();assert!(rpc_json(w.rpc,&est)["result"].is_string(),"with wide limits the estimate is answered");t.elapsed()}).min().unwrap();
    // The deadline falls inside the search: after the first simulation, before the estimate ends.
    let dl=(t1*3).min(te/3).max(Duration::from_millis(3));
    let mut l=small();l.rpc.compute_heavy=dl;l.rpc.work_deadline=Duration::from_secs(60);let r=start("estimate",l);
    let c=&pocold::rpc::COUNTERS;let h0=counter(&c.deadline_hits);
    let v=rpc_json(r.rpc,&est);
    assert_eq!(v["error"]["code"],-32020,"{v}");assert_eq!(v["error"]["data"]["reason"],"deadline");
    assert_eq!(counter(&c.deadline_hits)-h0,1,"one stopped simulation, no further ones");
    println!("ESTIMATE {}",serde_json::json!({"callMs":t1.as_millis() as u64,"estimateMs":te.as_millis() as u64,"computeHeavyMs":dl.as_millis() as u64}));
}
/// pocol_getParams (network.md:336) and pocol_getRpcStats (network.md:347) are served as light
/// methods: rpcLimits carries the running RpcGuard limits, with null for the limits pocold does not
/// enforce (never nominal), and the stat record carries every field of the spec record.
#[test] fn get_params_and_rpc_stats_report_the_limits(){let _g=serial();
    let r=start("params",small());
    let p=rpc_json(r.rpc,r#"{"jsonrpc":"2.0","id":1,"method":"pocol_getParams","params":[]}"#);
    let lim=&p["result"]["rpcLimits"];
    assert_eq!(lim["connMax"],32);assert_eq!(lim["heavyQueue"],16);assert_eq!(lim["lightQueue"],32);assert_eq!(lim["reqBuf"],1<<20);assert_eq!(lim["connBuf"],pocold::rpc::SMALL_REPLY_MAX);
    assert_eq!(lim["respMax"],8<<20);assert_eq!(lim["batchRespMax"],16<<20);assert_eq!(lim["logsScratch"],16<<20);assert_eq!(lim["rate"],20);assert_eq!(lim["bucketCap"],20);
    assert_eq!(lim["minSendRate"],65536);assert_eq!(lim["graceMs"],2000);assert_eq!(lim["enforceSlackMs"],200);assert_eq!(lim["timerEpsMs"],1);assert_eq!(lim["batchWallMs"],420000);
    assert_eq!(lim["computeHeavyMs"],5000);assert_eq!(lim["computeLightMs"],500);assert_eq!(lim["queueWaitMs"],2000);assert_eq!(lim["heavyRespMem"],48<<20);
    for k in ["scratchHeavy","scratchLight","cancelMaxMs","memTotal"]{assert!(lim[k].is_null(),"{k} is not enforced: null, never nominal");}
    assert_eq!(lim["sendSockBuf"],pocold::rpc::SEND_SOCK_BUF);
    let res=&p["result"];assert!(res["genesisHash"].is_string()&&res["chainId"].as_u64().is_some());
    assert_eq!(res["netKey"],pocold::types::hex(&pocold::rpc::net_key(r.chain_id,&r.genesis)));assert_eq!(res["forkSchedule"],serde_json::json!([[1,0]]));
    let st=rpc_json(r.rpc,r#"{"jsonrpc":"2.0","id":2,"method":"pocol_getRpcStats","params":[]}"#);let st=st["result"].as_object().unwrap().clone();
    for k in ["conns","heavyActive","heavyQueued","lightActive","lightQueued","heavyResv","lightResv","scratchHeavyMax","maxResPerConn","cancelled","cancelMaxMs","busyCount",
        "rateCount","limitCount","originDenied","pacerClosed","pacerLateMaxMs","sendDoneCount","batchWallHits"]{assert!(st[k].as_u64().is_some(),"pocol_getRpcStats has {k}: {st:?}");}
    for k in ["batchSendMaxS","batchWallMaxS"]{assert!(st[k].as_f64().is_some(),"{k}");}
    for k in ["scratchLightMax","snapshotsOpen","anchorRejects","traceDropped","lastPacerClose","sendOverrunMaxMs"]{assert!(st.contains_key(k),"pocol_getRpcStats has {k}");}
    assert_eq!(st["lightActive"],1,"the stats request itself is the light request running");
    // SO_SNDBUF was set on the stats request's own connection: Linux reads back twice the value it was
    // given, Windows the value itself; a connection left at the default reads back something else
    // (Linux 16 KiB before autotuning, Windows 64 KiB).
    let want=if cfg!(target_os="linux"){2*pocold::rpc::SEND_SOCK_BUF}else{pocold::rpc::SEND_SOCK_BUF};
    assert_eq!(st["sendSockBuf"],want,"the RPC connection's kernel send buffer is SEND_SOCK_BUF");
}
/// Measurement only (no assertion on the platform): whether the kernel takes a whole unread ~14 MiB
/// answer for a client with default socket options, one second after the answer is built. Printed
/// as ABSORB for the evidence; the answer is then read in full, and the pool must return to baseline.
#[test] fn unread_default_client_absorption(){let _g=serial();
    let mut l=small();l.rpc.write_deadline=Duration::from_secs(30);l.rpc.work_deadline=Duration::from_secs(60);let r=start("absorb",l);
    let member=r#"{"jsonrpc":"2.0","id":1,"method":"eth_call","params":[{"data":"0x621000006000fd","gas":"0x1c9c380"},"latest"]}"#;
    let body=format!("[{}]",vec![member;10].join(","));let base=pocold::rpc::RESP_MAX as u64;
    let mut s=TcpStream::connect(("127.0.0.1",r.rpc)).unwrap();let _=s.set_nodelay(true);
    write!(s,"POST / HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",body.len()).unwrap();
    let mut peak=0u64;let t=Instant::now();
    while t.elapsed()<Duration::from_secs(3){peak=peak.max(stats(r.rpc)["heavyReserved"].as_u64().unwrap_or(0));if peak>base+(12<<20){break}std::thread::sleep(Duration::from_millis(5));}
    std::thread::sleep(Duration::from_secs(1));let after=stats(r.rpc)["heavyReserved"].as_u64().unwrap_or(0);
    let mut out=Vec::new();s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();let _=s.read_to_end(&mut out);
    assert!(wait_until(10_000,||stats(r.rpc)["heavyReserved"].as_u64()==Some(base)),"the reservation is released after the answer");
    // Written to stdout directly, not through println!, so the line reaches the log of a passing run too.
    let line=format!("ABSORB {}\n",serde_json::json!({"os":std::env::consts::OS,"answerBytes":out.len(),"peakReserved":peak.saturating_sub(base),
        "reservedOneSecondLater":after.saturating_sub(base),"kernelTookWholeAnswer":peak>base+(12<<20)&&after<=base}));
    let _=std::io::Write::write_all(&mut std::io::stdout(),line.as_bytes());
}

// ---------------- round 3 (rpcguard track): the second review's findings (RG2-*) ----------------
/// Initcode that copies a BLAKE2F input of `rounds` rounds into memory, STATICCALLs 0x09 with all its
/// gas, then runs `tail`: one uninterruptible precompile run (1 gas per round) followed by an end that
/// has no hooked instruction (REVERT(0,0) is 60006000fd, INVALID is fe).
fn blake_then(rounds:u32,tail:&str)->String{
    let len=20+tail.len()/2;
    // CODECOPY(0, len, 213); STATICCALL(gas, 0x09, 0, 213, 0, 64); POP; tail.
    let code=format!("60d560{len:02x}6000396040600060d5600060095afa50{tail}");assert_eq!(code.len()/2,len);
    format!("0x{code}{rounds:08x}{}00","00".repeat(64+128+16))
}
/// Review 2 (RG2-M1, witness W1): a read-only eth_call whose precompile run outlasts COMPUTE_DEADLINE
/// and then reverts, or halts, is answered -32020 {deadline} (network.md:235), not with its late error
/// (code 3 "execution reverted", -32000 "halt"). The deadline is 20 ms; the run takes hundreds.
#[test] fn a_late_revert_or_halt_past_its_deadline_is_32020(){let _g=serial();
    let mut l=small();l.rpc.compute_heavy=Duration::from_millis(20);l.rpc.work_deadline=Duration::from_secs(60);let r=start("lateerr",l);
    let c=&pocold::rpc::COUNTERS;let mut seen=vec![];
    for (case,tail) in [("revert","60006000fd"),("halt","fe")]{
        let body=format!(r#"{{"jsonrpc":"2.0","id":7,"method":"eth_call","params":[{{"data":"{}","gas":"0x1c9c380"}},"latest"]}}"#,blake_then(28_000_000,tail));
        let h0=counter(&c.deadline_hits);let t=Instant::now();let v=rpc_json(r.rpc,&body);let took=t.elapsed();
        assert_eq!(v["error"]["code"],-32020,"{case} past COMPUTE_DEADLINE is -32020: {v}");assert_eq!(v["error"]["data"]["reason"],"deadline");assert_eq!(v["id"],7);
        assert!(counter(&c.deadline_hits)>h0,"counted as a deadline answer");seen.push(serde_json::json!({"case":case,"ms":took.as_millis() as u64}));
    }
    // Control: with a wide deadline the same calls answer their own errors.
    let mut l=small();l.rpc.compute_heavy=Duration::from_secs(60);l.rpc.work_deadline=Duration::from_secs(60);let w=start("lateerr-wide",l);
    for (tail,code) in [("60006000fd",3),("fe",-32000)]{
        let v=rpc_json(w.rpc,&format!(r#"{{"jsonrpc":"2.0","id":7,"method":"eth_call","params":[{{"data":"{}","gas":"0x1c9c380"}},"latest"]}}"#,blake_then(28_000_000,tail)));
        assert_eq!(v["error"]["code"],code,"{v}");
    }
    println!("LATEERR {}",serde_json::Value::Array(seen));
}
/// Review 2 (RG2-M3, N1, witness W4): QUEUE_WAIT bounds the wait for a HEAVY_QUEUE place too
/// (network.md:234). With one place, held by a queued request behind a long holder, a second heavy
/// request is -32021 busy at QUEUE_WAIT, long before the place frees (when the holder ends).
#[test] fn queue_wait_bounds_the_wait_for_a_heavy_queue_place(){let _g=serial();
    let b=duration_of("place-d",&blake_max());let qw=(b/6).max(Duration::from_millis(20));
    let mut l=small();l.rpc.heavy_queue=1;l.rpc.queue_wait=qw;l.rpc.work_deadline=Duration::from_secs(60);let r=start("place",l);
    let holder=format!("[{},{},{}]",blake_max(),blake_max(),blake_max());let h=hold_with(r.rpc,&holder);
    let req=|p:u16|std::thread::spawn(move||{let t=Instant::now();let v=rpc_json(p,r#"{"jsonrpc":"2.0","id":3,"method":"pocol_status","params":[]}"#);(v,t.elapsed())});
    let a=req(r.rpc);assert!(wait_until(5_000,||counter(&pocold::rpc::COUNTERS.queued)>=1),"the first request takes the one place");
    let (vb,tb)=req(r.rpc).join().unwrap();let (va,ta)=a.join().unwrap();
    assert!(busy_of(&va)&&busy_of(&vb),"both busy: {va} {vb}");
    assert!(tb>=qw&&tb<b,"the place waiter is busy at QUEUE_WAIT ({qw:?}), not when the place frees (about {:?}): {tb:?}",b*3);
    assert!(h.join().unwrap().is_array(),"the holder is answered");
    println!("PLACE {}",serde_json::json!({"blakeMs":b.as_millis() as u64,"queueWaitMs":qw.as_millis() as u64,"queuedMs":ta.as_millis() as u64,"placeWaiterMs":tb.as_millis() as u64}));
}
/// Review 2 (RG2-M3, N2): QUEUE_WAIT bounds the wait on a full main event queue (node::EVENT_QUEUE)
/// too. Behind a long holder, 150 heavy requests (more than the queue holds) are each -32021 busy at
/// QUEUE_WAIT, those that found the queue full included, long before the queue drains. The connection
/// cap and the memory budget are raised so 150 connections are served.
#[test] fn queue_wait_bounds_the_wait_on_a_full_main_queue(){let _g=serial();
    let b=duration_of("mainq-d",&blake_max());let qw=(b/4).max(Duration::from_millis(20));let n=150usize;assert!(n>pocold::node::EVENT_QUEUE);
    let mut l=small();l.rpc.conn_max=n+8;l.rpc.heavy_queue=4*n;l.rpc.queue_wait=qw;l.rpc.work_deadline=Duration::from_secs(60);l.mem_budget=64<<30;let r=start("mainq",l);
    let c=&pocold::rpc::COUNTERS;let f0=counter(&c.queue_full);
    let holder=format!("[{},{},{}]",blake_max(),blake_max(),blake_max());let h=hold_with(r.rpc,&holder);
    // One client per millisecond: a burst of 150 connects can overflow the listen backlog (128), and a
    // dropped SYN is retried only after a second, which would time the retry, not QUEUE_WAIT.
    let hs:Vec<_>=(0..n).map(|i|{let p=r.rpc;std::thread::sleep(Duration::from_millis(1));
        std::thread::spawn(move||{let t=Instant::now();let v=rpc_json(p,&format!(r#"{{"jsonrpc":"2.0","id":{i},"method":"pocol_status","params":[]}}"#));(v,t.elapsed())})}).collect();
    let res:Vec<(serde_json::Value,Duration)>=hs.into_iter().map(|x|x.join().unwrap()).collect();let full=counter(&c.queue_full)-f0;
    let worst=res.iter().map(|x|x.1).max().unwrap();
    assert!(res.iter().all(|(v,_)|busy_of(v)),"every request is busy at QUEUE_WAIT: {:?}",res.iter().find(|(v,_)|!busy_of(v)));
    assert!(full>=1,"some requests found the main queue full ({full})");
    assert!(worst<b*2,"answered at QUEUE_WAIT ({qw:?}), not when the queue drains (about {:?}): worst {worst:?}",b*3);
    assert!(h.join().unwrap().is_array(),"the holder is answered");
    println!("MAINQ {}",serde_json::json!({"blakeMs":b.as_millis() as u64,"queueWaitMs":qw.as_millis() as u64,"requests":n,"foundQueueFull":full,"worstMs":worst.as_millis() as u64}));
}
/// Review 2 (RG2-M3, N6, witness W10): atomic admission, in process. A request its connection already
/// marked gone is never run by the node (a transaction is not pooled), deterministically; and racing
/// the connection's QUEUED->GONE against the node's admission never runs a request the connection
/// gave up, never answers busy one the node admitted, and never loses one.
#[test] fn the_node_never_runs_a_request_its_connection_gave_up(){let _g=serial();
    use pocold::rpc::{answer,Request,QUEUED,ADMITTED,GONE};use std::sync::atomic::AtomicU8;
    let dir=std::env::temp_dir().join(format!("pocold-net-cas-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let cfg=Config{dir,name:"cas".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits::default()};
    let mut n=Node::open(cfg,spec()).unwrap();
    let tx=Tx{chain_id:777940,nonce:0,max_priority:U256::from(1u64),max_fee:U256::from(10_000_000_000u64),gas:21000,to:Some(revm::primitives::Address::ZERO),value:U256::ZERO,data:Default::default()}.sign_1559(&devnet::account_key("alice"));
    let req=|b:&str|{let (t,_r)=std::sync::mpsc::channel();Request::local(b.as_bytes().to_vec(),t,Instant::now()+Duration::from_secs(30),true)};
    let send=format!(r#"{{"jsonrpc":"2.0","id":4,"method":"eth_sendRawTransaction","params":["0x{}"]}}"#,hex(&tx));
    let q=req(&send);q.state.store(GONE,Ordering::SeqCst);let a=answer(&mut n,&q);
    assert!(a.body.is_empty(),"its connection answered: nothing more: {}",a.body);assert_eq!(n.pool.txs.len(),0,"and it never ran");
    let q=req(&send);let a=answer(&mut n,&q);assert!(a.body.contains("\"result\""),"{}",a.body);assert_eq!(q.state.load(Ordering::SeqCst),ADMITTED);assert_eq!(n.pool.txs.len(),1);
    // The race: a 3 ms QUEUE_WAIT, so the node's own QUEUED->GONE (busy) is exercised too.
    n.cfg.limits.rpc.queue_wait=Duration::from_micros(3000);let body=r#"{"jsonrpc":"2.0","id":3,"method":"eth_chainId","params":[]}"#;
    let mut seed=0x9e37_79b9_7f4a_7c15u64;let mut rnd=||{seed^=seed<<13;seed^=seed>>7;seed^=seed<<17;seed};
    let (mut kinds,mut bad)=(std::collections::BTreeMap::<&str,usize>::new(),vec![]);
    for _ in 0..1000{
        let q=req(body);let st:Arc<AtomicU8>=q.state.clone();let (dc,dn)=(rnd()%5000,rnd()%5000);
        let h=std::thread::spawn(move||{let t=Instant::now();while t.elapsed()<Duration::from_micros(dc){std::hint::spin_loop()}st.compare_exchange(QUEUED,GONE,Ordering::SeqCst,Ordering::SeqCst)});
        let t=Instant::now();while t.elapsed()<Duration::from_micros(dn){std::hint::spin_loop()}
        let a=answer(&mut n,&q);let conn=h.join().unwrap();
        let kind=match (a.body.is_empty(),a.body.contains("\"result\""),a.body.contains("-32021"),conn){
            (false,true,_,Err(ADMITTED))=>"admitted, answered",(true,..,Ok(_))=>"gone at the connection, skipped",(false,_,true,Err(GONE))=>"gone at the node, busy",_=>{bad.push(format!("{} {conn:?}",a.body));"violation"}};
        *kinds.entry(kind).or_default()+=1;
    }
    println!("CAS {}",serde_json::json!(kinds));
    assert!(bad.is_empty(),"{:?}",&bad[..bad.len().min(5)]);
    assert!(kinds.get("admitted, answered").is_some()&&kinds.get("gone at the connection, skipped").is_some(),"both orders were exercised: {kinds:?}");
}
/// Review 2 (RG2-M3, N7, witness W12): BATCH_WALL runs from the read-end, so the time a batch waits in
/// the queue counts (network.md:282): a batch that waited past BATCH_WALL behind a long holder is closed
/// without running when the node reaches it.
#[test] fn batch_wall_counts_the_time_queued(){let _g=serial();
    let b=duration_of("wallq-d",&blake_max());let c=&pocold::rpc::COUNTERS;
    let mut l=small();l.rpc.batch_wall=(b/3).max(Duration::from_millis(2));l.rpc.queue_wait=Duration::from_secs(30);l.rpc.work_deadline=Duration::from_secs(60);let r=start("wallq",l);
    let (w0,e0)=(counter(&c.batch_wall_hits),counter(&c.expired_members));let h=hold_with(r.rpc,&blake_max());
    let (code,body)=rpc(r.rpc,&format!("[{LIGHT},{LIGHT}]"),false);
    assert!(code==0&&body.is_empty(),"a batch queued past BATCH_WALL is closed: {code} {body}");assert_eq!(counter(&c.batch_wall_hits)-w0,1);
    assert_eq!(counter(&c.expired_members),e0,"closed by the wall, not by the work deadline");
    assert!(h.join().unwrap()["result"].is_string(),"the holder is answered");
}
/// Review 2 (RG2-M3, N8): a log query answered after its COMPUTE_DEADLINE without reaching a block check
/// (its range starts past the head) is -32020 {deadline} with the resume data of no progress:
/// fromBlock, lastCompleteBlock null (fromBlock−1, as at a limit on fromBlock) and nextFromBlock =
/// fromBlock (network.md:300), for eth_getLogs and pocol_getLogs. In process, deterministic: the heavy
/// deadline is zero.
#[test] fn a_late_get_logs_answer_carries_resume_data(){let _g=serial();
    use pocold::rpc::{answer,Request};
    let dir=std::env::temp_dir().join(format!("pocold-net-latelogs-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let mut l=Limits::default();l.rpc.compute_heavy=Duration::ZERO;
    let cfg=Config{dir,name:"latelogs".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:l};
    let mut n=Node::open(cfg,spec()).unwrap();let from=n.chain.head().height+5;
    for m in ["eth_getLogs","pocol_getLogs"]{
        let body=format!(r#"{{"jsonrpc":"2.0","id":9,"method":"{m}","params":[{{"fromBlock":"0x{from:x}","toBlock":"0x{from:x}"}}]}}"#);
        let (t,_r)=std::sync::mpsc::channel();let a=answer(&mut n,&Request::local(body.into_bytes(),t,Instant::now()+Duration::from_secs(30),false));
        let v:serde_json::Value=serde_json::from_str(&a.body).unwrap();let d=&v["error"]["data"];
        assert_eq!(v["error"]["code"],-32020,"{m}: {v}");assert_eq!(d["reason"],"deadline");
        assert_eq!((d["fromBlock"].as_str(),d["nextFromBlock"].as_str()),(Some(format!("0x{from:x}").as_str()),Some(format!("0x{from:x}").as_str())),"{m}: resume data {d}");
        assert!(d.get("lastCompleteBlock").is_some_and(|x|x.is_null()),"{m}: {d}");
    }
}
/// Review 2 (RG2-M3, N9, witness W13): each batch member gets its own COMPUTE_DEADLINE from when it
/// starts. Three uninterruptible members of about d each, under a heavy deadline of 2d: all three
/// answer their results (one deadline for the whole batch would end the third at 3d).
#[test] fn batch_members_each_get_their_own_compute_deadline(){let _g=serial();
    let one=blake(12_000_000,"eth_call");let d=duration_of("members-d",&one);
    let mut l=small();l.rpc.compute_heavy=d*2;l.rpc.work_deadline=Duration::from_secs(60);let r=start("members",l);
    let t=Instant::now();let v=rpc_json(r.rpc,&format!("[{one},{one},{one}]"));let took=t.elapsed();
    let a=v.as_array().unwrap_or_else(||panic!("{v}"));assert_eq!(a.len(),3);
    for (k,x) in a.iter().enumerate(){assert!(x["result"].is_string(),"member {k} within its own deadline ({:?}): {x}",d*2);}
    println!("MEMBERS {}",serde_json::json!({"memberMs":d.as_millis() as u64,"computeHeavyMs":(d*2).as_millis() as u64,"batchMs":took.as_millis() as u64}));
}
/// Review 2 (RG2-M3 I2, RG2-m6): a connection whose SO_SNDBUF cannot be set to SEND_SOCK_BUF is closed
/// unserved and counted as sockbufRefused, not as an RPC_CONN_MAX refusal: the setting fails, or it
/// reads back outside [SEND_SOCK_BUF, 2·SEND_SOCK_BUF] (`rpc::SOCKBUF_FAULT` injects both). Read-backs
/// inside that range are served.
#[test] fn a_connection_whose_send_buffer_cannot_be_set_is_refused(){let _g=serial();
    use pocold::rpc::{SOCKBUF_FAULT,SEND_SOCK_BUF};
    struct Reset;impl Drop for Reset{fn drop(&mut self){SOCKBUF_FAULT.store(0,Ordering::SeqCst);}}
    let r=start("sockbuf",small());let c=&pocold::rpc::COUNTERS;let _reset=Reset;
    for (fault,served) in [(usize::MAX,false),(3*SEND_SOCK_BUF,false),(SEND_SOCK_BUF-1,false),(SEND_SOCK_BUF,true),(2*SEND_SOCK_BUF,true),(0,true)]{
        SOCKBUF_FAULT.store(fault,Ordering::SeqCst);let (s0,f0)=(counter(&c.sockbuf_refused),counter(&c.refused));
        let (code,b)=rpc(r.rpc,LIGHT,false);
        if served{assert_eq!(code,200,"read-back {fault}: served: {b}");assert_eq!(counter(&c.sockbuf_refused),s0);}
        else{assert!(code==0&&b.is_empty(),"read-back {fault}: closed unserved: {code} {b}");assert_eq!(counter(&c.sockbuf_refused)-s0,1,"read-back {fault}");}
        assert_eq!(counter(&c.refused),f0,"not an RPC_CONN_MAX refusal");
    }
    assert_eq!(stats(r.rpc)["sockbufRefused"].as_u64(),Some(counter(&c.sockbuf_refused) as u64));
}
/// Review 2 (RG2-M4): an answer over its class's limit while encoding is -32020 {resultBytes} with a
/// clean release (network.md:210, RG12): an eth_blockNumber answer inflated past LIGHT_RESP_MAX (by a
/// 600 KiB id, as RG12's lightInflate) on the light lane, and one past RESP_MAX on the heavy lane. The
/// reservation shrinks to the error and is released with the reply (lightResv = 0). In process.
#[test] fn an_answer_over_its_limit_is_32020_result_bytes_with_a_clean_release(){let _g=serial();
    use pocold::rpc::{answer,Request,LIGHT_RESP_MAX,RESP_MAX};
    let dir=std::env::temp_dir().join(format!("pocold-net-inflate-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let cfg=Config{dir,name:"inflate".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits::default()};
    let mut n=Node::open(cfg,spec()).unwrap();let pools=n.resp_pools();
    let body=|id:usize|format!(r#"{{"jsonrpc":"2.0","id":"{}","method":"eth_blockNumber","params":[]}}"#,"i".repeat(id));
    let req=|b:String,light:bool|{let (t,_r)=std::sync::mpsc::channel();Request::local(b.into_bytes(),t,Instant::now()+Duration::from_secs(30),light)};
    for (light,id) in [(true,600<<10),(false,RESP_MAX)]{
        let a=answer(&mut n,&req(body(id),light));let v:serde_json::Value=serde_json::from_str(&a.body).unwrap();
        assert_eq!((v["error"]["code"].as_i64(),v["error"]["data"]["reason"].as_str()),(Some(-32020),Some("resultBytes")),"light={light}: {v}");
        assert!(v["id"].is_null()&&a.body.len()<256,"the error does not echo the inflating id");
        let (h,l)=pools.in_use();assert_eq!(if light{(h,l)}else{(l,h)},(0,a.body.capacity()),"the reservation shrank to the error");
        drop(a);assert_eq!(pools.in_use(),(0,0),"released with the reply (light={light})");
    }
    let a=answer(&mut n,&req(body(LIGHT_RESP_MAX/2),true));assert!(a.body.contains("\"result\""),"within the limit it is answered");
}
/// Review 2 (RG2-m5): the COMPUTE_DEADLINE follows the method's class (network.md (B), :235), as for
/// batch members: a light method whose body is over LIGHT_BODY_MAX (so it runs on the heavy lane) has
/// the light deadline (here zero: -32020 {deadline}), and a heavy method with the same body the heavy one.
#[test] fn a_light_method_over_light_body_max_has_the_light_deadline(){let _g=serial();
    let mut l=small();l.rpc.compute_light=Duration::ZERO;let r=start("lightbig",l);let pad="p".repeat(pocold::rpc::LIGHT_BODY_MAX);
    let big=format!(r#"{{"jsonrpc":"2.0","id":2,"method":"eth_blockNumber","params":[],"pad":"{pad}"}}"#);assert!(!pocold::rpc::is_light(big.as_bytes()));
    let v=rpc_json(r.rpc,&big);assert_eq!(v["error"]["code"],-32020,"{v}");assert_eq!(v["error"]["data"]["reason"],"deadline");
    let v=rpc_json(r.rpc,&format!(r#"{{"jsonrpc":"2.0","id":3,"method":"pocol_status","params":[],"pad":"{pad}"}}"#));assert!(v["result"].is_object(),"{v}");
}
/// Review 3 (RG3V-M3): the class limit follows the method (network.md (B), :210), whatever the lane. An
/// eth_blockNumber answer inflated past LIGHT_RESP_MAX by a 600 KiB id (RG12's lightInflate) arrives
/// on the heavy lane (its body is over LIGHT_BODY_MAX) and is still -32020 {resultBytes}, the id elided,
/// alone or as a batch member; within LIGHT_RESP_MAX it is answered, and a heavy method with the same id
/// has RESP_MAX. Over the socket; no reservation is left afterwards.
#[test] fn a_light_method_answer_over_light_resp_max_is_32020_on_the_heavy_lane(){let _g=serial();
    let mut l=small();l.rpc.work_deadline=Duration::from_secs(60);let r=start("lightinflate",l);
    let one=|m:&str,id:usize|format!(r#"{{"jsonrpc":"2.0","id":"{}","method":"{m}","params":[]}}"#,"i".repeat(id));
    let big=one("eth_blockNumber",600<<10);assert!(!pocold::rpc::is_light(big.as_bytes()));
    let rb=|v:&serde_json::Value|v["error"]["code"]==-32020&&v["error"]["data"]["reason"]=="resultBytes"&&v["id"].is_null();
    let v=rpc_json(r.rpc,&big);assert!(rb(&v),"a light answer over LIGHT_RESP_MAX on the heavy lane: {}",&v.to_string()[..v.to_string().len().min(300)]);
    let v=rpc_json(r.rpc,&format!("[{big}]"));assert!(v.as_array().is_some_and(|a|a.len()==1&&rb(&a[0])),"as a batch member: {}",&v.to_string()[..v.to_string().len().min(300)]);
    let v=rpc_json(r.rpc,&one("eth_blockNumber",400<<10));assert!(v["result"].is_string(),"within LIGHT_RESP_MAX it is answered");
    let v=rpc_json(r.rpc,&one("pocol_status",600<<10));assert!(v["result"].is_object(),"a heavy method's limit is RESP_MAX");
    assert!(wait_until(5000,||{let s=stats(r.rpc);s["heavyReserved"].as_u64()==Some(pocold::rpc::RESP_MAX as u64)&&s["lightReserved"]==0}),"released (only the stats request's own remains): {}",stats(r.rpc));
}
/// Review 3 (RG3V-m1): the conditional light entries of network.md (B) have the light class's
/// COMPUTE_DEADLINE (:235; here zero, so -32020 {deadline}): eth_getBlockBy* with hashes only (the full
/// flag false or absent) and pocol_getHeaders with count <= 64, alone or as batch members. Their heavy
/// forms (full blocks; count > 64) keep the heavy deadline. A listed light method over LIGHT_BODY_MAX
/// has the light deadline whatever the shape of its body (here a long id, not a padding field).
#[test] fn the_conditional_light_entries_have_the_light_deadline(){let _g=serial();
    let mut l=small();l.rpc.compute_light=Duration::ZERO;l.rpc.compute_heavy=Duration::from_secs(60);let r=start("lightcond",l);
    let gh=format!("\"0x{}\"",hex(&r.genesis));
    let req=|m:&str,p:&str|format!(r#"{{"jsonrpc":"2.0","id":7,"method":"{m}","params":{p}}}"#);
    let dl=|v:&serde_json::Value|v["error"]["code"]==-32020&&v["error"]["data"]["reason"]=="deadline";
    for (m,p,light) in [("eth_getBlockByNumber",r#"["latest",false]"#.to_string(),true),("eth_getBlockByNumber",r#"["latest"]"#.into(),true),
        ("eth_getBlockByNumber",r#"["latest",true]"#.into(),false),(("eth_getBlockByHash"),format!("[{gh},false]"),true),("eth_getBlockByHash",format!("[{gh},true]"),false),
        ("pocol_getHeaders","[0,64]".into(),true),("pocol_getHeaders",r#"[0,"0x40"]"#.into(),true),("pocol_getHeaders","[0,65]".into(),false)]{
        let v=rpc_json(r.rpc,&req(m,&p));
        if light{assert!(dl(&v),"{m} {p}: light deadline: {v}")}else{assert!(v.get("result").is_some(),"{m} {p}: heavy deadline: {v}")}
    }
    let v=rpc_json(r.rpc,&format!("[{},{}]",req("eth_getBlockByNumber",r#"["latest",false]"#),req("eth_getBlockByNumber",r#"["latest",true]"#)));
    let a=v.as_array().unwrap_or_else(||panic!("{v}"));assert!(dl(&a[0])&&a[1]["result"].is_object(),"batch members by class: {v}");
    let big=format!(r#"{{"jsonrpc":"2.0","id":"{}","method":"eth_blockNumber","params":[]}}"#,"i".repeat(pocold::rpc::LIGHT_BODY_MAX));
    assert!(!pocold::rpc::is_light(big.as_bytes()));let v=rpc_json(r.rpc,&big);assert!(dl(&v),"{}",&v.to_string()[..200.min(v.to_string().len())]);
}
/// Review 3 (RG3V-m3): COMPUTE_DEADLINE runs from admission "including encoding" (network.md:235). A
/// handler that ends within its deadline but whose encoding ends after it (the ENCODE_DELAY_MS test
/// hook holds the encoded answer of the id ENCODE_DELAY_ID) is -32020 {deadline}: a heavy method, a
/// light method (alone and as a batch member) and a conditional light entry. The same requests
/// with another id are answered.
#[test] fn the_compute_deadline_includes_encoding(){let _g=serial();
    use pocold::rpc::{ENCODE_DELAY_MS,ENCODE_DELAY_ID};
    struct Reset;impl Drop for Reset{fn drop(&mut self){ENCODE_DELAY_MS.store(0,Ordering::SeqCst);}}
    let mut l=small();l.rpc.compute_heavy=Duration::from_millis(300);l.rpc.compute_light=Duration::from_millis(300);l.rpc.work_deadline=Duration::from_secs(60);
    let r=start("encode-deadline",l);let _reset=Reset;ENCODE_DELAY_MS.store(700,Ordering::SeqCst);
    let req=|m:&str,p:&str,id:&str|format!(r#"{{"jsonrpc":"2.0","id":"{id}","method":"{m}","params":{p}}}"#);
    let dl=|v:&serde_json::Value|v["error"]["code"]==-32020&&v["error"]["data"]["reason"]=="deadline"&&v["id"]==ENCODE_DELAY_ID;
    for (m,p) in [("eth_getBlockByNumber",r#"["latest",true]"#),("eth_blockNumber","[]"),("pocol_getHeaders","[0,1]")]{
        let v=rpc_json(r.rpc,&req(m,p,"other"));assert!(v.get("result").is_some(),"{m}: answered without the delay: {v}");
        let t=Instant::now();let v=rpc_json(r.rpc,&req(m,p,ENCODE_DELAY_ID));
        assert!(dl(&v)&&t.elapsed()>=Duration::from_millis(700),"{m}: a slow encoding past the deadline is -32020 {{deadline}}: {v}");
    }
    let v=rpc_json(r.rpc,&format!("[{},{}]",req("eth_blockNumber","[]","other"),req("eth_blockNumber","[]",ENCODE_DELAY_ID)));
    let a=v.as_array().unwrap_or_else(||panic!("{v}"));assert!(a[0]["result"].is_string()&&dl(&a[1]),"batch member: {v}");
}

/// PR #27 review R2-1 (network.md (I): RPC does not hold up validation, mining or sync): one batch of
/// heavy eth_call members used to run on the consensus thread with only the light lane drained between
/// members, so a P2P Hello waited for the whole batch. Queued P2P, miner and timer events now run between
/// members: a peer that connects during the batch gets the node's Hello long before the batch ends.
#[test] fn a_batch_does_not_hold_p2p_events_for_its_whole_length(){let _g=serial();
    let r=start("batch-p2p",Limits::default());
    let batch=format!("[{}]",(0..8).map(|_|blake_max()).collect::<Vec<_>>().join(","));
    let port=r.rpc;let ha=&pocold::rpc::COUNTERS.heavy_active;let ha0=ha.load(Ordering::SeqCst);
    let h=std::thread::spawn(move||{let t=Instant::now();
        let mut s=TcpStream::connect(("127.0.0.1",port)).unwrap();s.set_read_timeout(Some(Duration::from_secs(120))).unwrap();let _=s.set_nodelay(true);
        s.write_all(format!("POST / HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{batch}",batch.len()).as_bytes()).unwrap();
        let mut out=String::new();let _=s.read_to_string(&mut out);(out.get(9..12).map(str::to_string),t.elapsed())});
    let t=Instant::now();while ha.load(Ordering::SeqCst)==ha0&&t.elapsed()<Duration::from_secs(10){std::thread::sleep(Duration::from_millis(5));}
    std::thread::sleep(Duration::from_millis(200));
    // Time from connecting until the node's Hello arrives: the node sends it when the consensus thread
    // handles the Connected event.
    let mut s=TcpStream::connect(("127.0.0.1",r.p2p)).unwrap();s.set_read_timeout(Some(Duration::from_secs(120))).unwrap();
    // A heavy request sent during the batch is answered (it is set aside between members, never run inside the batch).
    let port2=r.rpc;let other=std::thread::spawn(move||{let mut s=TcpStream::connect(("127.0.0.1",port2)).unwrap();s.set_read_timeout(Some(Duration::from_secs(120))).unwrap();
        let body=blake(1,"eth_call");s.write_all(format!("POST / HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",body.len()).as_bytes()).unwrap();
        let mut out=String::new();let _=s.read_to_string(&mut out);out});
    let t=Instant::now();s.write_all(&hello(&r,"during-batch")).unwrap();let mut b=[0u8;4];s.read_exact(&mut b).unwrap();let hello_after=t.elapsed();
    let (code,batch_took)=h.join().unwrap();
    assert_eq!(code.as_deref(),Some("200"));
    let o=other.join().unwrap();
    // It waited behind the batch longer than QUEUE_WAIT, so busy is the RpcGuard answer (as before); what
    // matters is that it is answered, never lost or left hanging by being set aside.
    assert!(o.starts_with("HTTP/1.1 200")&&(o.contains("\"result\"")||o.contains("-32021")),"the request queued during the batch is answered: {o}");
    assert!(hello_after*3<batch_took,"the Hello came {hello_after:?} into a {batch_took:?} batch: it waited for the batch");
    println!("BATCH-P2P {}",serde_json::json!({"helloMs":hello_after.as_millis() as u64,"batchMs":batch_took.as_millis() as u64}));
}
