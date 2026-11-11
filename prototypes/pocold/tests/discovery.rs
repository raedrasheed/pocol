//! Peer discovery between real sockets (VA-SPEC §1a): a node over loopback with raw peers. Loopback addresses are
//! learned only in local mode (`Discovery::allow_local`); in public mode the same exchange stores nothing.
use std::io::{Read,Write};
use std::net::{SocketAddr,TcpListener,TcpStream};
use std::sync::{Arc,atomic::{AtomicBool,Ordering}};
use std::time::{Duration,Instant};
use pocold::{chain::Spec,net::Msg,node::{Config,Discovery,DiscoveryTimes,Limits,Node,Outbound}};
use pocol_lp4_execution::devnet::{self,DevnetConfig};
#[path="support/hello.rs"] mod support;

/// One devnet per test binary: its genesis is now and its timing is measured once (as in tests/net.rs).
fn spec()->Spec{
    static GENESIS:std::sync::OnceLock<u64>=std::sync::OnceLock::new();
    static TIMING:std::sync::OnceLock<pocol_lp4_execution::admission::Timing>=std::sync::OnceLock::new();
    let mut cfg=DevnetConfig::new(777941,*GENESIS.get_or_init(pocold::node::now_s),&["a","b","c"]);
    // Discovery's chain-progress assertions need miners to keep searching after S stops.
    // Bounded ranges can all exhaust without a solution and wait entire 25 s attempts,
    // so a healthy connected network can miss the test's deadline. Use spec-1 full nonce.
    cfg.nonce_mode=1;
    let n=devnet::build(&cfg).unwrap();
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
/// `f`'s value once it has stayed the same for `quiet_ms` (polled every 20 ms), or None if it still moved after `ms`.
fn settled<T:PartialEq>(ms:u64,quiet_ms:u64,f:impl Fn()->T)->Option<T>{
    let t=Instant::now();let (mut v,mut since)=(f(),Instant::now());
    while t.elapsed()<Duration::from_millis(ms){
        if since.elapsed()>=Duration::from_millis(quiet_ms){return Some(v)}
        std::thread::sleep(Duration::from_millis(20));let w=f();if w!=v{(v,since)=(w,Instant::now());}
    }
    None
}
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
        compatibility:support::hello_identity(&s),listen_port,nonce:0})}
/// A node listening on loopback and running on its own thread (`run_controlled`) until dropped, in local mode or not,
/// with the configured peers `peers` gives for its listener's address. Its network counters stay readable.
struct Running{p2p:SocketAddr,rpc:SocketAddr,net:pocold::net::Net,stop:Arc<AtomicBool>,run:Option<std::thread::JoinHandle<()>>}
impl Running{
    fn start(tag:&str,allow_local:bool,peers:impl FnOnce(SocketAddr)->Vec<SocketAddr>)->Self{
        let dir=std::env::temp_dir().join(format!("pocold-disc-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
        // The node's ports stay held until it binds them (tests/relay.rs).
        let held=[TcpListener::bind("127.0.0.1:0").unwrap(),TcpListener::bind("127.0.0.1:0").unwrap()];let [p2p,rpc]=held.each_ref().map(|l|l.local_addr().unwrap());
        // PR2a's behaviour alone: no automatic outbound (PR2b's tests set their own).
        let limits=Limits{discovery:Discovery{allow_local,rng_seed:Some(7),outbound:Outbound::Count(0),..Default::default()},..Default::default()};
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
    assert!(wait_until(10_000,||ended()&&n.status()["peers"].as_array().is_some_and(|p|p.is_empty())),"both ends of the connection closed, and left the node");
    // The readers end before their last events reach the node loop (a forwarder carries them, and pocol_status is not
    // ordered with them): the count is read once the node has taken both ends (each leaves `peers` by its Hello or its
    // Closed, which follow its Connected and Message) and it has stopped moving.
    let once=settled(10_000,500,||n.counter("selfConnections")).expect("the count settles");
    assert!((1..=2).contains(&once),"one connection, counted at each end that read the other's Hello: {once}");
    // The dialer retries a lost connection within 300 ms (net::dial): ten times that, nothing.
    std::thread::sleep(Duration::from_secs(3));
    let st=n.status();
    assert_eq!((st["discovery"]["counters"]["selfConnections"].as_u64(),st["compatiblePeers"].as_u64(),&st["discovery"]["self"]),(Some(once),Some(0),&own),"{}",st["discovery"]);
    assert!(ended(),"no connection since");
}

/// VA-SPEC §1a.6: a clean stop (`run_controlled` stopped by its host) saves the book, and the next start of the network
/// loads it: the same entries, verified, relayable and keys. A file with one changed digit fails its checksum: it is
/// ignored, the book starts empty, the reason is shown, and the node runs: it answers and learns as before.
#[test] fn the_address_book_survives_a_clean_restart_and_a_damaged_file_is_ignored(){
    let dir=std::env::temp_dir().join(format!("pocold-disc-book-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);let file=dir.join("addrbook");
    // Local mode; the first save an hour away, so that the file can come only from the stop.
    let limits=||Limits{discovery:Discovery{allow_local:true,rng_seed:Some(7),outbound:Outbound::Count(0),times:DiscoveryTimes{save_first_ms:3_600_000,..Default::default()},..Default::default()},..Default::default()};
    let config=|listen:Option<SocketAddr>,peers:Vec<SocketAddr>,rpc:Option<SocketAddr>|Config{dir:dir.clone(),name:"book".into(),miner:None,listen,peers,rpc,stop_at:None,log:false,limits:limits()};
    let r=TcpListener::bind("127.0.0.1:0").unwrap();let ra=r.local_addr().unwrap();
    // The node's ports stay held until it binds them (tests/relay.rs).
    let hold=||{let held=[TcpListener::bind("127.0.0.1:0").unwrap(),TcpListener::bind("127.0.0.1:0").unwrap()];let a=held.each_ref().map(|l|l.local_addr().unwrap());(held,a)};
    let (held,[p2p,rpc])=hold();
    let first=spec();let (chain_id,genesis,identity)=(first.chain_id(),first.genesis_hash(),support::hello_identity(&first));
    let hello=|listen_port:u16|frame(&Msg::Hello{chain_id,genesis,head:genesis,height:0,work:[0;32],node:"raw".into(),protocol:pocold::identity::PROTOCOL_VERSION,
        compatibility:identity,listen_port,nonce:0});
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
    // It runs, and its discovery works: reopened with a listener, it answers an inbound peer's Hello and GetAddr (an
    // empty Addr: the book is empty but for the peer's own advertisement, which is never answered to it) and learns
    // that advertisement.
    let (held,[p2p,rpc])=hold();
    let mut n=Node::open(config(Some(p2p),vec![],Some(rpc)),spec()).unwrap();drop(held);n.start_network().unwrap();let st=book(n.status_json());
    assert_eq!((st["entries"].as_u64(),st["file"].as_str()),(Some(0),Some("ignored: checksum")),"{st}");
    let stop=Arc::new(AtomicBool::new(false));let s=stop.clone();let run=std::thread::spawn(move||n.run_controlled(|_|s.load(Ordering::SeqCst)));
    let mut i=TcpStream::connect(p2p).unwrap();
    assert!(matches!(read(&mut i,Duration::from_secs(5)),Some(Msg::Hello{..})),"it greets an inbound peer");
    i.write_all(&hello(40005)).unwrap();i.write_all(&frame(&Msg::GetAddr)).unwrap();
    assert_eq!(read(&mut i,Duration::from_secs(5)),Some(Msg::Addr{addrs:vec![]}),"and answers its GetAddr");
    assert!(wait_until(10_000,||book(status(rpc))["entries"]==1),"and learns its advertisement: {}",book(status(rpc)));
    stop.store(true,Ordering::SeqCst);run.join().unwrap();
    let _=std::fs::remove_dir_all(&dir);
}

// ---------------- PR2b: automatic outbound, seeds and bootstrap between live nodes (plan §5.3) ----------------
/// The short timings of PR2b's live tests: a 100 ms tick, retry spacings of 2 s (unverified) and 1 s (verified), slot
/// backoff 0.2-1 s, a connection stable after 1 s, the seeds' boot window 1 s, a wanted seed 3 s to become compatible,
/// released 0.5 s after it is and 1 s after health holds, held 2 s doubling; anchors after 1.5 s; saves after 0.5 s and
/// every second; feelers every second; the stall window 20 s; answers rebuilt every 100 ms.
fn short()->DiscoveryTimes{DiscoveryTimes{tick_ms:100,retry_ms:2_000,retry_verified_ms:1_000,backoff_base_ms:200,backoff_max_ms:1_000,stable_ms:1_000,
    seed_boot_ms:1_000,seed_connect_ms:3_000,seed_hold_ms:500,seed_release_ms:1_000,seed_hold_base_ms:2_000,anchor_uptime_ms:1_500,save_first_ms:500,save_every_ms:1_000,
    feeler_listen_ms:1_000,feeler_quiet_ms:1_000,feeler_jitter_ms:200,stall_min_ms:20_000,stall_blocks:1,cache_ms:100,..Default::default()}}
/// `short` never stalled nor quiet: for the tests without a miner, whose head stays at the binary-wide genesis (`spec`),
/// which is 20 s old once the test starts 20 s after the binary's first (`--test-threads=1`, a loaded runner); stalled,
/// the spare runs the extra in place of the feeler.
fn no_stall()->DiscoveryTimes{DiscoveryTimes{stall_min_ms:u64::MAX/4,..short()}}
/// What a PR2b live node is: its name, whether it listens, which genesis miner it mines as, its configured peers
/// (its seeds among them), seeds, bootstrap addresses and automatic outbound.
struct Live<'a>{name:&'a str,listen:bool,miner:Option<&'a str>,peers:Vec<SocketAddr>,seeds:Vec<SocketAddr>,bootstrap:Vec<SocketAddr>,outbound:Outbound}
impl Running{
    /// A PR2b live node in local mode with the short timings, its data in `dir` (kept, so that a restart on the same
    /// directory reads its book and anchors), its two ports held until it binds them.
    fn live(dir:&std::path::Path,l:Live)->Self{Self::live_with(dir,l,Default::default(),short())}
    /// `live` with these network limits and discovery timings.
    fn live_with(dir:&std::path::Path,l:Live,net:pocold::net::Limits,times:DiscoveryTimes)->Self{
        let held=[TcpListener::bind("127.0.0.1:0").unwrap(),TcpListener::bind("127.0.0.1:0").unwrap()];let [p2p,rpc]=held.each_ref().map(|x|x.local_addr().unwrap());
        let d=Discovery{allow_local:true,rng_seed:Some(7),outbound:l.outbound,seeds:l.seeds,bootstrap:l.bootstrap,times,..Default::default()};
        let cfg=Config{dir:dir.into(),name:l.name.into(),miner:l.miner.map(|m|devnet::miner_identity(m).key),listen:l.listen.then_some(p2p),peers:l.peers,rpc:Some(rpc),stop_at:None,log:false,
            limits:Limits{discovery:d,net,..Default::default()}};
        let mut n=Node::open(cfg,spec()).unwrap();let net=n.net.clone();drop(held);n.start_network().unwrap();
        let stop=Arc::new(AtomicBool::new(false));let s=stop.clone();
        Self{p2p,rpc,net,stop,run:Some(std::thread::spawn(move||n.run_controlled(|_|s.load(Ordering::SeqCst))))}
    }
    /// The addresses of its automatic slots that are up.
    fn slots_up(&self)->std::collections::BTreeSet<String>{
        self.status()["discovery"]["outbound"]["slots"].as_array().into_iter().flatten().filter(|s|s["state"]=="up"&&s["role"]=="auto").filter_map(|s|s["addr"].as_str().map(String::from)).collect()
    }
    fn head(&self)->(u64,String){let h=&self.status()["head"];(h["height"].as_u64().unwrap_or(0),h["hash"].as_str().unwrap_or("").into())}
}
fn set(a:&[SocketAddr])->std::collections::BTreeSet<String>{a.iter().map(|x|x.to_string()).collect()}
fn live_root(tag:&str)->std::path::PathBuf{let d=std::env::temp_dir().join(format!("pocold-disc-pr2b-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&d);d}
/// The nodes' heads are equal and at least `h`.
fn agree(nodes:&[&Running],h:u64)->bool{let heads:Vec<(u64,String)>=nodes.iter().map(|n|n.head()).collect();heads[0].0>=h&&heads.iter().all(|x|*x==heads[0])}

/// G-OUT, G-SEED, G-NOSEED (VA-SPEC §1a.7-§1a.8): J, configured only with the seed S, fills its two automatic slots
/// with the listeners A and B it learned from S's answer, and releases S once two useful peers hold. With S stopped
/// the three keep one chain. A clean stop saves J's anchors; restarted on its directory with S still down, J dials an
/// anchor first and follows the chain again.
#[test] fn a_node_finds_and_keeps_listeners_it_never_configured(){
    let root=live_root("find");
    let s=Running::live(&root.join("s"),Live{name:"s",listen:true,miner:Some("c"),peers:vec![],seeds:vec![],bootstrap:vec![],outbound:Outbound::Count(0)});
    let seed=vec![s.p2p];
    let a=Running::live(&root.join("a"),Live{name:"a",listen:true,miner:Some("a"),peers:seed.clone(),seeds:seed.clone(),bootstrap:vec![],outbound:Outbound::Count(0)});
    let b=Running::live(&root.join("b"),Live{name:"b",listen:true,miner:Some("b"),peers:seed.clone(),seeds:seed.clone(),bootstrap:vec![],outbound:Outbound::Count(0)});
    assert!(wait_until(30_000,||s.counter("selfAds")>=2),"S holds A's and B's self-advertisements");
    let j=Running::live(&root.join("j"),Live{name:"j",listen:true,miner:None,peers:seed.clone(),seeds:seed.clone(),bootstrap:vec![],outbound:Outbound::Count(2)});
    let want=set(&[a.p2p,b.p2p]);
    assert!(wait_until(60_000,||j.slots_up()==want),"J's automatic slots are A and B, which it never configured: {}",j.status()["discovery"]["outbound"]);
    assert!(wait_until(60_000,||j.counter("seedReleases")>=1),"J releases S once two useful peers hold: {}",j.status()["discovery"]["outbound"]);
    drop(s);
    let h=a.head().0+2;
    assert!(wait_until(120_000,||agree(&[&a,&b,&j],h)),"with S stopped A, B and J keep one chain: {:?} {:?} {:?}",a.head(),b.head(),j.head());
    assert!(wait_until(30_000,||!j.status()["discovery"]["outbound"]["anchors"].as_array().unwrap().is_empty()),"J has an anchor: {}",j.status()["discovery"]["outbound"]);
    drop(j);
    let book=std::fs::read_to_string(root.join("j").join("addrbook")).unwrap();
    assert!(book.lines().any(|l|l.starts_with("a ")),"the clean stop saved the anchors");
    let j2=Running::live(&root.join("j"),Live{name:"j",listen:true,miner:None,peers:seed.clone(),seeds:seed.clone(),bootstrap:vec![],outbound:Outbound::Count(2)});
    assert!(wait_until(30_000,||j2.counter("anchorsDialed")>=1&&!j2.slots_up().is_empty()),"restarted with S down, J dials its anchor first: {}",j2.status()["discovery"]);
    let h=a.head().0+1;
    assert!(wait_until(120_000,||agree(&[&a,&j2],h)),"and follows the chain: {:?} {:?}",a.head(),j2.head());
    drop((a,b,j2));let _=std::fs::remove_dir_all(&root);
}
/// G-NOSEED, D17 (VA-SPEC §1a.9): a brand-new node whose only seed is dead reaches the network through its bootstrap
/// address A, learns B from A's answer and fills both slots; the dead seed is held.
#[test] fn a_new_node_bootstraps_from_its_bootstrap_list_with_the_seeds_down(){
    let root=live_root("boot");
    let dead={let l=TcpListener::bind("127.0.0.1:0").unwrap();l.local_addr().unwrap()};
    let a=Running::live(&root.join("a"),Live{name:"a",listen:true,miner:None,peers:vec![],seeds:vec![],bootstrap:vec![],outbound:Outbound::Count(0)});
    let b=Running::live(&root.join("b"),Live{name:"b",listen:true,miner:None,peers:vec![a.p2p],seeds:vec![],bootstrap:vec![],outbound:Outbound::Count(0)});
    assert!(wait_until(30_000,||a.counter("selfAds")>=1),"A holds B's self-advertisement");
    let k=Running::live_with(&root.join("k"),Live{name:"k",listen:true,miner:None,peers:vec![dead],seeds:vec![dead],bootstrap:vec![a.p2p],outbound:Outbound::Count(2)},Default::default(),no_stall());
    let held=std::cell::Cell::new(false);
    assert!(wait_until(60_000,||{held.set(held.get()||k.status()["discovery"]["outbound"]["seeds"][0]["state"]=="held");k.slots_up()==set(&[a.p2p,b.p2p])}),"K fills its slots from A and what A knows: {}",k.status()["discovery"]);
    assert!(held.get()||wait_until(30_000,||k.counter("seedHolds")>=1),"the dead seed is held: {}",k.status()["discovery"]);
    assert_eq!(k.counter("bootstrapLoaded"),1);
    drop((a,b,k));let _=std::fs::remove_dir_all(&root);
}
/// D22, D7'' (A5, B1): a seed that accepts and closes at once (it never answers a Hello) is dialed a few times, then
/// held with growing holds: at most 12 connections in the first minute, at most 3 in its second half (at bc7e683 a
/// dead configured address is dialed about 3 times a second). A node without a listener wants it all along (D7'a).
#[test] fn a_dead_seed_is_dialed_at_most_a_few_times_a_minute(){
    let root=live_root("dead");
    let r=TcpListener::bind("127.0.0.1:0").unwrap();let ra=r.local_addr().unwrap();r.set_nonblocking(true).unwrap();
    let accepts=Arc::new(std::sync::Mutex::new(Vec::<Instant>::new()));let (acc,done)=(accepts.clone(),Arc::new(AtomicBool::new(false)));let d=done.clone();
    let t=std::thread::spawn(move||while !d.load(Ordering::SeqCst){match r.accept(){Ok((c,_))=>{acc.lock().unwrap().push(Instant::now());drop(c)}Err(_)=>std::thread::sleep(Duration::from_millis(10))}});
    let start=Instant::now();
    let n=Running::live(&root.join("n"),Live{name:"n",listen:false,miner:None,peers:vec![ra],seeds:vec![ra],bootstrap:vec![],outbound:Outbound::Count(2)});
    let mut held=false;
    while start.elapsed()<Duration::from_secs(60){held|=n.status()["discovery"]["outbound"]["seeds"][0]["state"]=="held";std::thread::sleep(Duration::from_millis(200));}
    done.store(true,Ordering::SeqCst);t.join().unwrap();
    let at:Vec<u64>=accepts.lock().unwrap().iter().map(|x|x.duration_since(start).as_millis() as u64).collect();
    assert!(at.len()<=12&&at.iter().filter(|x|**x>=30_000).count()<=3,"the dead seed's connections: {at:?}");
    assert!(held&&n.counter("seedHolds")>=2,"held: {}",n.status()["discovery"]);
    drop(n);let _=std::fs::remove_dir_all(&root);
}
/// R, a raw configured or bootstrap peer of the node behind `c`: it reads the node's Hello, answers it at genesis
/// announcing its listener `port`, and returns once the node has asked it for addresses (GetAddr), unanswered.
fn greet_and_wait_for_getaddr(c:&mut TcpStream,port:u16){
    assert!(matches!(read(c,Duration::from_secs(5)),Some(Msg::Hello{..})),"the node greets R");
    c.write_all(&raw_hello(port)).unwrap();
    assert!((0..16).any(|_|read(c,Duration::from_secs(5))==Some(Msg::GetAddr)),"and asks R for addresses");
}
/// D12' (VA-SPEC §1a.7, §1b): a feeler sends nothing, so a full listener never establishes it and evicts nobody for
/// it. X holds its one automatic slot to the listener P; only then does its configured raw peer R name F, a node whose
/// three inbound places raw peers hold and whose eviction is free (no grace, no budget; with three peers of one key
/// the lottery and the older half protect two, so a newcomer's Hello would evict the third). X's feeler verifies F
/// (`feelersOk`); F evicts nobody and keeps its peers; X's automatic slot never names F.
#[test] fn a_feeler_verifies_without_evicting_a_full_listener(){
    let root=live_root("feeler");
    let free=pocold::net::EvictLimits{grace:Duration::ZERO,key_interval:Duration::ZERO,burst:1000,refill:Duration::ZERO};
    let f=Running::live_with(&root.join("f"),Live{name:"f",listen:true,miner:None,peers:vec![],seeds:vec![],bootstrap:vec![],outbound:Outbound::Count(0)},
        pocold::net::Limits{peers_max:3,handshaking_max:1,evict:free,..Default::default()},no_stall());
    let mut fp:Vec<TcpStream>=(0..3).map(|i|{let mut c=TcpStream::connect(f.p2p).unwrap();assert!(matches!(read(&mut c,Duration::from_secs(5)),Some(Msg::Hello{..})));
        c.write_all(&raw_hello(0)).unwrap();assert!(wait_until(10_000,||f.net.established()==i+1),"F's place {i} is held");c}).collect();
    let p=Running::live(&root.join("p"),Live{name:"p",listen:true,miner:None,peers:vec![],seeds:vec![],bootstrap:vec![],outbound:Outbound::Count(0)});
    let r=TcpListener::bind("127.0.0.1:0").unwrap();let ra=r.local_addr().unwrap();
    let x=Running::live_with(&root.join("x"),Live{name:"x",listen:true,miner:None,peers:vec![ra],seeds:vec![],bootstrap:vec![p.p2p],outbound:Outbound::Count(1)},Default::default(),no_stall());
    let mut c=accept(&r);greet_and_wait_for_getaddr(&mut c,ra.port());
    assert!(wait_until(30_000,||x.slots_up()==set(&[p.p2p])),"X's automatic slot holds P: {}",x.status()["discovery"]["outbound"]);
    let st=x.status();assert!(st["discovery"]["outbound"]["stalled"]==false&&st["discovery"]["outbound"]["quiet"]==false,"X is neither stalled nor quiet, so its spare runs feelers: {}",st["discovery"]["outbound"]);
    c.write_all(&frame(&Msg::Addr{addrs:vec![f.p2p]})).unwrap();
    let named=std::cell::Cell::new(false);let fa=f.p2p.to_string();
    let auto_names_f=|st:&serde_json::Value|st["discovery"]["outbound"]["slots"].as_array().into_iter().flatten().any(|s|s["role"]=="auto"&&s["addr"]==fa.as_str());
    assert!(wait_until(30_000,||{let st=x.status();named.set(named.get()||auto_names_f(&st));st["discovery"]["counters"]["feelersOk"].as_u64()>=Some(1)}),
        "X's feeler verifies F: {}",x.status()["discovery"]);
    assert!(!named.get()&&!auto_names_f(&x.status())&&x.slots_up()==set(&[p.p2p]),"X's automatic slot never names F");
    assert_eq!((f.status()["stats"]["inboundEvicted"].as_u64(),f.net.established()),(Some(0),3),"F evicted nobody for the feeler");
    for c in &mut fp{c.set_read_timeout(Some(Duration::from_millis(300))).unwrap();
        assert!(matches!(c.read(&mut [0u8;1]),Err(e) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)),"and F's peers are still connected");}
    drop((x,p,f));let _=std::fs::remove_dir_all(&root);
}
/// D25 (VA-SPEC §1a.7): J's one automatic slot holds R, an attacker-like raw peer that answers J's Hello at genesis and
/// never sends a block, while the miners M, N and O build the chain; R's answer names M. Once J's head is W old J is
/// stalled; its extra dials M (from a source no automatic slot is connected to), M's blocks move J's head, M takes R's
/// place (`extraSwaps`), R's connection is closed, and J follows the chain.
///
/// J starts only once M, N and O agree on a block (the ordering barrier). Without it, on a loaded runner whose miners
/// had made no block yet when J stalled, J's first extra reached M with nothing to give: it was closed after W, the
/// next extra went to N or O (named by M's answer on that extra), and that one swapped in once the first block came, so
/// M never held the slot (CI, PR #9). With a block on M, M's first Hello starts J's sync session and the session's
/// first block swaps M in.
#[test] fn a_stalled_node_recovers_through_an_extra_slot(){
    let root=live_root("stall");
    let m=Running::live(&root.join("m"),Live{name:"m",listen:true,miner:Some("a"),peers:vec![],seeds:vec![],bootstrap:vec![],outbound:Outbound::Count(0)});
    let mesh=vec![m.p2p];
    let n=Running::live(&root.join("n"),Live{name:"n",listen:true,miner:Some("b"),peers:mesh.clone(),seeds:vec![],bootstrap:vec![],outbound:Outbound::Count(0)});
    let o=Running::live(&root.join("o"),Live{name:"o",listen:true,miner:Some("c"),peers:mesh.clone(),seeds:vec![],bootstrap:vec![],outbound:Outbound::Count(0)});
    assert!(wait_until(120_000,||agree(&[&m,&n,&o],1)),"the miners build the chain before J starts: {:?} {:?} {:?}",m.head(),n.head(),o.head());
    let r=TcpListener::bind("127.0.0.1:0").unwrap();let ra=r.local_addr().unwrap();
    let j=Running::live(&root.join("j"),Live{name:"j",listen:true,miner:None,peers:vec![],seeds:vec![],bootstrap:vec![ra],outbound:Outbound::Count(1)});
    let mut c=accept(&r);greet_and_wait_for_getaddr(&mut c,ra.port());c.write_all(&frame(&Msg::Addr{addrs:mesh.clone()})).unwrap();
    assert!(wait_until(30_000,||j.slots_up()==set(&[ra])),"J's automatic slot holds R: {}",j.status()["discovery"]["outbound"]);
    // R keeps its connection and reads whatever J sends, until J closes it.
    c.set_read_timeout(None).unwrap();let (etx,erx)=std::sync::mpsc::channel();
    std::thread::spawn(move||{let mut b=[0u8;4096];while matches!(c.read(&mut b),Ok(k) if k>0){}let _=etx.send(());});
    assert!(wait_until(120_000,||j.counter("extraSwaps")>=1),"J's extra replaces R: {}",j.status()["discovery"]);
    assert!(j.counter("stalls")>=1&&j.counter("extras")>=1,"{}",j.status()["discovery"]);
    assert!(erx.recv_timeout(Duration::from_secs(30)).is_ok(),"J closed R's connection");
    assert!(wait_until(30_000,||j.slots_up()==set(&[m.p2p])),"M ({}; N {}, O {}) holds J's automatic slot: {}",m.p2p,n.p2p,o.p2p,j.status()["discovery"]["outbound"]);
    let h=m.head().0.max(1);
    assert!(wait_until(120_000,||agree(&[&m,&j],h)),"and J follows the chain: {:?} {:?}",m.head(),j.head());
    drop((j,m,n,o));let _=std::fs::remove_dir_all(&root);
}
