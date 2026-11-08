//! Peer discovery between real sockets (VA-SPEC §1a): a node over loopback with raw peers, in local mode
//! (`Discovery::allow_local`), the only mode in which loopback addresses are learned.
use std::io::{Read,Write};
use std::net::{SocketAddr,TcpListener,TcpStream};
use std::sync::{Arc,atomic::{AtomicBool,Ordering}};
use std::time::{Duration,Instant};
use pocold::{chain::Spec,net::Msg,node::{Config,Discovery,DiscoveryTimes,Limits,Node}};
use pocol_lp4_execution::devnet::{self,DevnetConfig};

/// One devnet per test binary: its genesis is now and its timing is measured once (as in tests/net.rs).
fn spec()->Spec{
    static GENESIS:std::sync::OnceLock<u64>=std::sync::OnceLock::new();
    static TIMING:std::sync::OnceLock<pocol_lp4_execution::admission::Timing>=std::sync::OnceLock::new();
    let n=devnet::build(&DevnetConfig::new(777941,*GENESIS.get_or_init(pocold::node::now_s),&["a","b","c"])).unwrap();
    let t=TIMING.get_or_init(||pocol_lp4_execution::admission::measure(&n.spec_bytes,&n.alloc,1).unwrap());
    Spec::new(pocol_lp4_execution::admission::admit(&n.request(&pocold::single_host_local(),Some(t))).unwrap_or_else(|e|panic!("{e}")))
}
fn frame(m:&Msg)->Vec<u8>{let b=m.encode();let mut f=(b.len() as u32).to_be_bytes().to_vec();f.extend(b);f}
/// The next frame from the node within `wait` (None on timeout or close).
fn read(s:&mut TcpStream,wait:Duration)->Option<Msg>{
    s.set_read_timeout(Some(wait)).unwrap();let mut len=[0u8;4];s.read_exact(&mut len).ok()?;
    let mut b=vec![0u8;u32::from_be_bytes(len) as usize];s.read_exact(&mut b).ok()?;Msg::decode(&b).ok()
}
fn wait_until(ms:u64,f:impl Fn()->bool)->bool{let t=Instant::now();while t.elapsed()<Duration::from_millis(ms){if f(){return true}std::thread::sleep(Duration::from_millis(20));}f()}
/// The node's pocol_status over its RPC port.
fn status(rpc:SocketAddr)->serde_json::Value{
    let body=r#"{"jsonrpc":"2.0","id":1,"method":"pocol_status","params":[]}"#;
    let mut s=TcpStream::connect(rpc).unwrap();s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.write_all(format!("POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",body.len()).as_bytes()).unwrap();
    let mut out=String::new();let _=s.read_to_string(&mut out);
    serde_json::from_str::<serde_json::Value>(out.split("\r\n\r\n").nth(1).unwrap_or("null")).unwrap()["result"].clone()
}

/// VA-SPEC §1a.6: a clean stop (`run_controlled` stopped by its host) saves the book, and the next start of the network
/// loads it: the same entries, verified, relayable and keys. A file with one changed digit fails its checksum: it is
/// ignored, the book starts empty, the reason is shown, and the node runs.
#[test] fn the_address_book_survives_a_clean_restart_and_a_damaged_file_is_ignored(){
    let dir=std::env::temp_dir().join(format!("pocold-disc-book-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);let file=dir.join("addrbook");
    // Local mode; the first save an hour away, so that the file can come only from the stop.
    let limits=||Limits{discovery:Discovery{allow_local:true,rng_seed:Some(7),times:DiscoveryTimes{save_first_ms:3_600_000,..Default::default()},..Default::default()},..Default::default()};
    let config=|listen:Option<SocketAddr>,peers:Vec<SocketAddr>,rpc:Option<SocketAddr>|Config{dir:dir.clone(),name:"book".into(),miner:None,listen,peers,rpc,stop_at:None,log:false,limits:limits()};
    let r=TcpListener::bind("127.0.0.1:0").unwrap();let ra=r.local_addr().unwrap();
    // The node's ports stay held until it binds them (tests/relay.rs).
    let held=[TcpListener::bind("127.0.0.1:0").unwrap(),TcpListener::bind("127.0.0.1:0").unwrap()];let [p2p,rpc]=held.each_ref().map(|l|l.local_addr().unwrap());
    let first=spec();let (chain_id,genesis)=(first.chain_id(),first.genesis_hash());
    let hello=|listen_port:u16|frame(&Msg::Hello{chain_id,genesis,head:genesis,height:0,work:[0;32],node:"raw".into(),protocol:pocold::identity::PROTOCOL_VERSION,
        compatibility:pocold::identity::compatibility(),listen_port,nonce:0});
    let mut n=Node::open(config(Some(p2p),vec![ra],Some(rpc)),first).unwrap();drop(held);n.start_network().unwrap();
    let stop=Arc::new(AtomicBool::new(false));let s=stop.clone();
    let run=std::thread::spawn(move||{n.run_controlled(|_|s.load(Ordering::SeqCst));n});
    // R, the configured peer: it announces its listener and answers the node's GetAddr with two addresses.
    r.set_nonblocking(true).unwrap();let t=Instant::now();
    let mut c=loop{if let Ok((c,_))=r.accept(){break c}assert!(t.elapsed()<Duration::from_secs(5),"the node dials R");std::thread::sleep(Duration::from_millis(20));};
    c.set_nonblocking(false).unwrap();
    assert!(matches!(read(&mut c,Duration::from_secs(5)),Some(Msg::Hello{listen_port,nonce,..}) if listen_port==p2p.port()&&nonce!=0),"the node announces its listener and its nonce");
    c.write_all(&hello(ra.port())).unwrap();
    assert_eq!(read(&mut c,Duration::from_secs(5)),Some(Msg::GetAddr),"and asks R for addresses");
    c.write_all(&frame(&Msg::Addr{addrs:vec!["127.0.0.1:40001".parse().unwrap(),"127.0.0.1:40002".parse().unwrap()]})).unwrap();
    // I, an inbound peer, advertises a listener.
    let mut i=TcpStream::connect(p2p).unwrap();i.write_all(&hello(40003)).unwrap();assert!(matches!(read(&mut i,Duration::from_secs(5)),Some(Msg::Hello{..})));
    let book=|v:serde_json::Value|v["discovery"]["book"].clone();
    assert!(wait_until(10_000,||book(status(rpc))["entries"]==4),"R verified, the two addresses heard from it and I's: {}",book(status(rpc)));
    assert!(!file.exists(),"not saved before the stop");
    stop.store(true,Ordering::SeqCst);let n=run.join().unwrap();
    assert!(file.exists(),"a clean stop saves the book");
    let before=book(n.status_json());
    assert_eq!((before["verified"].as_u64(),before["relayable"].as_u64(),before["keys"].as_u64(),before["file"].as_str()),(Some(1),Some(2),Some(4),Some("absent")),"{before}");
    drop(n);
    // Reopened without a listener or peers, the node's network start loads the file.
    let reopen=||{let mut n=Node::open(config(None,vec![],None),spec()).unwrap();n.start_network().unwrap();n};
    let after=book(reopen().status_json());
    assert_eq!(after["file"],"loaded");for k in ["entries","verified","relayable","keys"]{assert_eq!(after[k],before[k],"{k}");}
    // One digit of the `g` line changed: still a well-formed file, so only the checksum can refuse it.
    let mut b=std::fs::read(&file).unwrap();let g=b.windows(3).position(|w|w==b"\ng ").unwrap()+1;let end=g+b[g..].iter().position(|x|*x==b'\n').unwrap();b[end-1]^=1;
    std::fs::write(&file,&b).unwrap();
    let mut n=reopen();let st=book(n.status_json());
    assert_eq!((st["entries"].as_u64(),st["file"].as_str()),(Some(0),Some("ignored: checksum")),"{st}");
    // It runs: its loop takes events (three, from its tick thread) until its host stops it.
    let mut checks=0;n.run_controlled(|_|{checks+=1;checks>3});assert_eq!(checks,4,"the node ran");
    let _=std::fs::remove_dir_all(&dir);
}
