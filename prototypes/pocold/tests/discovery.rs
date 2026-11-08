//! Peer discovery between real sockets (VA-SPEC §1a): a node over loopback with raw peers. Loopback addresses are
//! learned only in local mode (`Discovery::allow_local`); in public mode the same exchange stores nothing.
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

/// A raw peer's Hello for the test network at genesis (so it starts no sync session), announcing `listen_port`.
fn raw_hello(listen_port:u16)->Vec<u8>{let s=spec();let genesis=s.genesis_hash();
    frame(&Msg::Hello{chain_id:s.chain_id(),genesis,head:genesis,height:0,work:[0;32],node:"raw".into(),protocol:pocold::identity::PROTOCOL_VERSION,
        compatibility:pocold::identity::compatibility(),listen_port,nonce:0})}
/// A node listening on loopback and running on its own thread (`run_controlled`) until dropped, in local mode or not,
/// with the configured peers `peers` gives for its listener's address. Its network counters stay readable.
struct Running{p2p:SocketAddr,rpc:SocketAddr,net:pocold::net::Net,stop:Arc<AtomicBool>,run:Option<std::thread::JoinHandle<()>>}
impl Running{
    fn start(tag:&str,allow_local:bool,peers:impl FnOnce(SocketAddr)->Vec<SocketAddr>)->Self{
        let dir=std::env::temp_dir().join(format!("pocold-disc-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
        // The node's ports stay held until it binds them (tests/relay.rs).
        let held=[TcpListener::bind("127.0.0.1:0").unwrap(),TcpListener::bind("127.0.0.1:0").unwrap()];let [p2p,rpc]=held.each_ref().map(|l|l.local_addr().unwrap());
        let limits=Limits{discovery:Discovery{allow_local,rng_seed:Some(7),..Default::default()},..Default::default()};
        let cfg=Config{dir,name:tag.into(),miner:None,listen:Some(p2p),peers:peers(p2p),rpc:Some(rpc),stop_at:None,log:false,limits};
        let mut n=Node::open(cfg,spec()).unwrap();let net=n.net.clone();drop(held);n.start_network().unwrap();
        let stop=Arc::new(AtomicBool::new(false));let s=stop.clone();
        Self{p2p,rpc,net,stop,run:Some(std::thread::spawn(move||n.run_controlled(|_|s.load(Ordering::SeqCst))))}
    }
    fn status(&self)->serde_json::Value{status(self.rpc)}
    fn counter(&self,k:&str)->u64{self.status()["discovery"]["counters"][k].as_u64().unwrap()}
}
impl Drop for Running{fn drop(&mut self){self.stop.store(true,Ordering::SeqCst);if let Some(r)=self.run.take(){let _=r.join();}}}
/// The node's connection to the raw listener `r`, which it dials as a configured peer.
fn accept(r:&TcpListener)->TcpStream{
    r.set_nonblocking(true).unwrap();let t=Instant::now();
    let c=loop{if let Ok((c,_))=r.accept(){break c}assert!(t.elapsed()<Duration::from_secs(5),"the node dials its configured peer");std::thread::sleep(Duration::from_millis(20));};
    c.set_nonblocking(false).unwrap();c
}
/// Nothing is waiting on `s` (WouldBlock at once): read only after the node has shown, through its status, that it
/// took the message that could have been answered.
fn nothing_waiting(s:&mut TcpStream)->bool{s.set_nonblocking(true).unwrap();let r=matches!(s.read(&mut [0u8;1]),Err(e) if e.kind()==std::io::ErrorKind::WouldBlock);s.set_nonblocking(false).unwrap();r}
const A:&str="127.0.0.1:40001";const B:&str="127.0.0.1:40002";const I_PORT:u16=40003;
/// The exchange both modes share: the node announces its listener and nonce to R, its configured peer; after R's
/// Hello its next frame is GetAddr, which R answers with A and B. `learned` is the book's size once R's Addr is taken
/// (`addrReceived`), checked before I connects. Then I, an inbound peer advertising I_PORT, asks: its Addr is
/// returned, and the node's counters once a second GetAddr was taken and ignored.
fn ask_and_answer(n:&Running,r:&TcpListener)->(serde_json::Value,Vec<SocketAddr>){
    let ra=r.local_addr().unwrap();let mut c=accept(r);
    assert!(matches!(read(&mut c,Duration::from_secs(5)),Some(Msg::Hello{listen_port,nonce,..}) if listen_port==n.p2p.port()&&nonce!=0),"the node announces its listener and its nonce");
    c.write_all(&raw_hello(ra.port())).unwrap();
    assert_eq!(read(&mut c,Duration::from_secs(5)),Some(Msg::GetAddr),"the node's next frame after R's Hello asks R for addresses");
    c.write_all(&frame(&Msg::Addr{addrs:vec![A.parse().unwrap(),B.parse().unwrap()]})).unwrap();
    assert!(wait_until(10_000,||n.counter("addrReceived")==1),"R's answer is taken");
    let learned=n.status()["discovery"]["book"].clone();
    let mut i=TcpStream::connect(n.p2p).unwrap();i.write_all(&raw_hello(I_PORT)).unwrap();
    assert!(matches!(read(&mut i,Duration::from_secs(5)),Some(Msg::Hello{..})));
    i.write_all(&frame(&Msg::GetAddr)).unwrap();
    let Some(Msg::Addr{addrs})=read(&mut i,Duration::from_secs(5)) else{panic!("I's GetAddr is answered")};
    // A second GetAddr on the same connection: taken (ignored) before anything could be queued for it.
    i.write_all(&frame(&Msg::GetAddr)).unwrap();
    assert!(wait_until(10_000,||n.counter("getAddrIgnored")==1),"the second GetAddr is taken");
    assert!(nothing_waiting(&mut i),"and not answered: one answer per inbound connection");
    let st=n.status();assert_eq!(st["discovery"]["counters"]["getAddrAnswered"],1);
    (serde_json::json!({"learned":learned,"after":st["discovery"]}),addrs)
}

/// VA-SPEC §1a.2 and §1a.5, in local mode: a node asks its configured peer R for addresses once R's Hello shows it
/// compatible, and learns R (verified: R announced its listener) and the two addresses R heard of. An inbound peer I
/// that asks is answered once, with what the node verified or was advertised: R, never the hearsay A and B, and never
/// I's own advertised address.
#[test] fn a_node_asks_its_configured_peer_and_answers_inbound_peers(){
    let r=TcpListener::bind("127.0.0.1:0").unwrap();let ra=r.local_addr().unwrap();
    let n=Running::start("ask",true,|_|vec![ra]);
    let (st,answer)=ask_and_answer(&n,&r);
    assert_eq!((st["learned"]["entries"].as_u64(),st["learned"]["verified"].as_u64()),(Some(3),Some(1)),"R verified, A and B heard from it: {st}");
    assert_eq!(answer,vec![ra],"I is answered with R only: {st}");
    assert_eq!((st["after"]["book"]["entries"].as_u64(),st["after"]["counters"]["selfAds"].as_u64()),(Some(4),Some(1)),"I's advertisement is stored: {st}");
}

/// The same exchange in public mode (the default): loopback addresses have no address group, so none is learned (R,
/// its answer, I's advertisement), and I's GetAddr is answered with an empty Addr.
#[test] fn public_mode_never_learns_loopback_addresses(){
    let r=TcpListener::bind("127.0.0.1:0").unwrap();let ra=r.local_addr().unwrap();
    let n=Running::start("public",false,|_|vec![ra]);
    let (st,answer)=ask_and_answer(&n,&r);
    assert_eq!(st["learned"]["entries"].as_u64(),Some(0),"{st}");
    assert_eq!(answer,Vec::<SocketAddr>::new(),"{st}");
    let c=&st["after"]["counters"];
    assert_eq!((st["after"]["book"]["entries"].as_u64(),c["addrDropped"].as_u64(),c["addrAccepted"].as_u64(),c["selfAds"].as_u64()),(Some(0),Some(2),Some(0),Some(1)),"{st}");
}

/// VA-SPEC §2 (H1): a node configured with its own listener's address connects to itself once. Its nonce marks the
/// connection, both ends close, the address is shown as the node's own, no compatible peer is counted, and the
/// dialer, which still sees the address as live, does not dial it again.
#[test] fn a_node_configured_with_its_own_address_detects_itself_once(){
    let n=Running::start("self",false,|p2p|vec![p2p]);
    let own=serde_json::json!([n.p2p.to_string()]);
    assert!(wait_until(10_000,||n.status()["discovery"]["self"]==own),"{}",n.status()["discovery"]);
    let ended=||n.net.counters.live.load(Ordering::SeqCst)==0&&n.net.counters.outbound.load(Ordering::SeqCst)==0;
    assert!(wait_until(10_000,ended),"both ends of the connection closed");
    let once=n.counter("selfConnections");
    assert!((1..=2).contains(&once),"one connection, counted at each end that read the other's Hello: {once}");
    // The dialer retries a lost connection within 300 ms (net::dial): ten times that, nothing.
    std::thread::sleep(Duration::from_secs(3));
    let st=n.status();
    assert_eq!((st["discovery"]["counters"]["selfConnections"].as_u64(),st["compatiblePeers"].as_u64(),&st["discovery"]["self"]),(Some(once),Some(0),&own),"{}",st["discovery"]);
    assert!(ended(),"no connection since");
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
