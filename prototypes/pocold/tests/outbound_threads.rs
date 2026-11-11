//! PR2b (plan §5.3, B8): automatic outbound's threads stay within the A13 thread inventory (THREAD-INVENTORY: `out-{t}`
//! per slot, one `out-fwd`, and a slot connection's reader and writer, the reader joined before the slot's next dial)
//! while its slots churn through addresses that accept and close at once. One node in its own test binary, so that the
//! process-wide thread counts are this node's alone. Linux only: the counts are read from /proc/self/task (on other
//! systems the binary is empty, as the audit's THREADS-NOT-MEASURED notes for Windows).
#![cfg(target_os="linux")]
use std::io::{Read,Write};
use std::net::{TcpListener,TcpStream};
use std::sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}};
use std::time::{Duration,Instant};
use pocold::{chain::Spec,net::Msg,node::{Config,Discovery,DiscoveryTimes,Limits,Node,Outbound}};
use pocol_lp4_execution::devnet::{self,DevnetConfig};
#[path="support/hello.rs"] mod support;

fn spec()->Spec{
    let n=devnet::build(&DevnetConfig::new(777942,pocold::node::now_s(),&["a","b","c"])).unwrap();
    let t=pocol_lp4_execution::admission::measure(&n.spec_bytes,&n.alloc,1).unwrap();
    Spec::new(pocol_lp4_execution::admission::admit(&n.request(&pocold::single_host_local(),Some(&t))).unwrap_or_else(|e|panic!("{e}")))
}
fn frame(m:&Msg)->Vec<u8>{let b=m.encode();let mut f=(b.len() as u32).to_be_bytes().to_vec();f.extend(b);f}
fn read(s:&mut TcpStream,wait:Duration)->Option<Msg>{
    s.set_read_timeout(Some(wait)).unwrap();let mut len=[0u8;4];s.read_exact(&mut len).ok()?;
    let mut b=vec![0u8;u32::from_be_bytes(len) as usize];s.read_exact(&mut b).ok()?;Msg::decode(&b).ok()
}
/// The names of this process's threads (Linux `comm`, at most 15 bytes).
fn threads()->Vec<String>{
    std::fs::read_dir("/proc/self/task").unwrap().filter_map(|e|std::fs::read_to_string(e.ok()?.path().join("comm")).ok()).map(|s|s.trim().to_string()).collect()
}
fn count(names:&[String],f:impl Fn(&str)->bool)->usize{names.iter().filter(|n|f(n)).count()}

/// K = 2 adds exactly its three slot threads (`out-0` … `out-2`, the spare's included) and one forwarder. While the
/// slots dial three learned listeners that accept and close at once, the peer threads never exceed a reader and a
/// writer per configured peer and slot (plus one exiting), and each unverified address is dialed at most once per
/// retry spacing (2 s here, so at most 6 times in 10 s).
#[test] fn slot_threads_stay_within_the_inventory_under_churn(){
    let base=threads();assert_eq!(count(&base,|n|n.starts_with("out-")),0,"no slot thread before the node: {base:?}");
    let stop=Arc::new(AtomicBool::new(false));
    // Three listeners that accept and close at once, each counting its connections.
    let mut hits=Vec::new();let mut ls=Vec::new();
    for _ in 0..3{let l=TcpListener::bind("127.0.0.1:0").unwrap();l.set_nonblocking(true).unwrap();ls.push(l.local_addr().unwrap());
        let n=Arc::new(Mutex::new(0usize));hits.push(n.clone());let s=stop.clone();
        std::thread::spawn(move||while !s.load(Ordering::SeqCst){match l.accept(){Ok((c,_))=>{*n.lock().unwrap()+=1;drop(c)}Err(_)=>std::thread::sleep(Duration::from_millis(5))}});}
    let r=TcpListener::bind("127.0.0.1:0").unwrap();let ra=r.local_addr().unwrap();
    let dir=std::env::temp_dir().join(format!("pocold-outbound-threads-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let times=DiscoveryTimes{tick_ms:100,retry_ms:2_000,retry_verified_ms:1_000,backoff_base_ms:200,backoff_max_ms:1_000,..Default::default()};
    let d=Discovery{allow_local:true,rng_seed:Some(7),outbound:Outbound::Count(2),times,..Default::default()};
    let cfg=Config{dir:dir.clone(),name:"threads".into(),miner:None,listen:None,peers:vec![ra],rpc:None,stop_at:None,log:false,limits:Limits{discovery:d,..Default::default()}};
    let spec=spec();let (chain_id,genesis,id)=(spec.chain_id(),spec.genesis_hash(),support::hello_identity(&spec));
    let mut n=Node::open(cfg,spec).unwrap();n.start_network().unwrap();
    let s=stop.clone();let run=std::thread::spawn(move||n.run_controlled(|_|s.load(Ordering::SeqCst)));
    // A new thread names itself once it runs (pthread_setname_np from inside it), so the names settle within moments.
    let slots=|a:&[String]|(count(a,|n|n.starts_with("out-")&&n!="out-fwd"),count(a,|n|n=="out-fwd"));
    let t=Instant::now();while slots(&threads())!=(3,1)&&t.elapsed()<Duration::from_secs(5){std::thread::sleep(Duration::from_millis(10));}
    let after=threads();assert_eq!(slots(&after),(3,1),"K = 2: three slots and one forwarder: {after:?}");
    // R, the configured peer, answers the node's GetAddr with the three listeners.
    let mut c=loop{if let Ok((c,_))=r.accept(){break c}};
    assert!(matches!(read(&mut c,Duration::from_secs(5)),Some(Msg::Hello{..})));
    c.write_all(&frame(&Msg::Hello{chain_id,genesis,head:genesis,height:0,work:[0;32],node:"r".into(),protocol:pocold::identity::PROTOCOL_VERSION,compatibility:id,listen_port:ra.port(),nonce:0})).unwrap();
    assert_eq!(read(&mut c,Duration::from_secs(5)),Some(Msg::GetAddr));
    c.write_all(&frame(&Msg::Addr{addrs:ls.clone()})).unwrap();
    let (t,mut peak)=(Instant::now(),0);
    while t.elapsed()<Duration::from_secs(10){peak=peak.max(count(&threads(),|n|n.starts_with("peer-")));std::thread::sleep(Duration::from_millis(20));}
    let dialed:Vec<usize>=hits.iter().map(|h|*h.lock().unwrap()).collect();
    assert!(dialed.iter().sum::<usize>()>=3,"the slots dial the learned listeners: {dialed:?}");
    assert!(dialed.iter().all(|d|*d<=6),"at most once per retry spacing: {dialed:?}");
    assert!(peak<=2*(1+3)+1,"peer threads {peak}: a reader and a writer per configured peer and slot, plus one exiting");
    stop.store(true,Ordering::SeqCst);run.join().unwrap();drop(c);let _=std::fs::remove_dir_all(&dir);
}
