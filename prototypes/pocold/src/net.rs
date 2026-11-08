//! Experimental wire v2 peer transport: TCP on the explicitly configured listener, frames
//! [u32 BE length][RLP([type, ...fields])], length <= MAX_FRAME. Malformed frames
//! or a wrong network identity close the connection; they never touch block state.
//!
//! Resource bounds (A05, local and not consensus; `Limits`):
//! - at most `peers_max` inbound connections, of which at most `handshaking_max` have not
//!   yet delivered their Hello; excess connections are closed on accept, before any thread is
//!   spawned. Outbound connections are separate (one per configured peer, `dial`), so inbound
//!   connections can never take the slots the node needs to reach its own peers;
//! - at most `per_group_max` (default 4, half of `peers_max`) inbound connections per public address
//!   group (`group_of`: IPv4 /16, IPv6 /32, a tunnelled IPv4 address as that address), so one public
//!   /16 or /32 never holds more than half of the inbound slots. A connection past it is refused even
//!   while slots are free: nodes behind one NAT or one provider range share these places until peer
//!   discovery and eviction exist. Groups are counted per address, so one host that reaches the node
//!   from several groups (an IPv4 /16 and an IPv6 /32, say) holds a full set of places in each. Addresses
//!   that are not globally reachable (loopback, private, link-local, ...) and the node's configured
//!   peers, by their exact address (`Net::protect`), are not grouped;
//! - the first frame must be a Hello and must arrive within `hello_deadline`; a frame
//!   whose length has arrived must complete within `frame_deadline`; an established peer
//!   silent for `idle_deadline` is closed;
//! - every peer has a writer thread and an outbound queue of at most `queue_bytes_max`;
//!   the consensus thread only enqueues. Gossip to a full queue drops that peer (a peer
//!   that stops reading never blocks consensus); range responses stop at half the queue;
//! - inbound frames wait in a bounded event queue (backpressure stops the reader), so
//!   frame memory is at most (peers_max + NET_EVENT_QUEUE) · MAX_FRAME.
use std::collections::{HashMap,HashSet,VecDeque};
use std::io::{Read,Write};
use std::net::{IpAddr,TcpListener,TcpStream,SocketAddr};
use std::sync::{Arc,Condvar,Mutex,atomic::{AtomicBool,AtomicUsize,Ordering}};
use std::sync::mpsc::SyncSender;
use std::time::{Duration,Instant};
use crate::rlp;

/// Largest frame. A Block message (header <= 3067 B plus a body <= BODY_MAX) fits for
/// BODY_MAX <= 196608; R15 checks the profile's message sizes against this at admission.
pub const MAX_FRAME:usize=256*1024;
/// Inbound messages waiting for the consensus thread, per listener or dialer.
/// D-A13-BUDGET: 64 -> 32 per forwarder; a full queue blocks the peer readers behind it.
pub const NET_EVENT_QUEUE:usize=32;
/// A pre-handshake peer cannot allocate a full block frame or inject control characters in logs.
pub const HELLO_MAX:usize=512;
pub const NODE_NAME_MAX:usize=64;

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Limits{
    pub peers_max:usize,pub handshaking_max:usize,
    /// Inbound connections per public address group (`group_of`, `Net::admit`); 0 disables grouping.
    /// `pocold run --per-group-max N`, or POCOL_PER_GROUP_MAX.
    pub per_group_max:usize,
    pub hello_deadline:Duration,pub frame_deadline:Duration,pub idle_deadline:Duration,
    pub queue_bytes_max:usize,pub write_deadline:Duration,
}
impl Default for Limits{
    // D-A13-BUDGET: peers_max 32 -> 8 inbound (network.md: <= 50 peers); a devnet node has two.
    // per_group_max 4: a public /16 (IPv4) or /32 (IPv6) holds at most half of the inbound slots (4 of 8).
    fn default()->Self{Self{peers_max:8,handshaking_max:8,per_group_max:4,hello_deadline:Duration::from_secs(5),frame_deadline:Duration::from_secs(30),
        idle_deadline:Duration::from_secs(300),queue_bytes_max:2*1024*1024,write_deadline:Duration::from_secs(10)}}
}
/// An inbound address group: a public peer's IPv4 /16 or IPv6 /32 (`group_of`).
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub enum Group{V4([u8;2]),V6([u8;4])}
/// The address group of a public peer: its IPv4 /16 or IPv6 /32, Bitcoin Core's netgroup granularity
/// (Bitcoin Core uses netgroups to choose whom to evict and to spread its outbound connections; this node
/// has no eviction yet and caps the inbound places of a group instead, `Net::admit`).
/// - An IPv6 address that carries an IPv4 address is grouped as that IPv4 address, so a tunnel is no
///   second way into the same /16: IPv4-mapped ::ffff:0:0/96 (a dual-stack socket's IPv4 peer), SIIT
///   ::ffff:0:0:0/96, NAT64 64:ff9b::/96, 6to4 2002::/16 and Teredo 2001::/32 (its client address,
///   inverted). A tunnel form whose IPv4 address is not public keeps its IPv6 /32.
/// - All of 200::/7 (key-derived Yggdrasil addresses: one key can mint any /32) is one group. cjdns uses
///   fc00::/8 inside the unique-local range, which LANs and container networks also use; as in Bitcoin Core
///   without its cjdns setting, it stays ungrouped.
/// - None for an address that is not globally reachable in the IANA special-purpose registries, as
///   Rust's `is_global` reads them: IPv4 this-network 0/8, private, shared 100.64/10, loopback,
///   link-local, IETF 192.0.0/24, documentation, benchmarking 198.18/15, reserved 240/4 and broadcast,
///   and multicast; IPv6 unspecified, loopback, local-use translation 64:ff9b:1::/48, discard 100::/64,
///   the IETF block 2001::/23 except its globally reachable parts, documentation 2001:db8::/32 and
///   3fff::/20, SRv6 5f00::/16, unique-local, link-local, site-local and multicast. Devnets, LAN testnets
///   and local tests are therefore never limited by group.
///
/// A local service policy, not a validity rule.
pub fn group_of(ip:IpAddr)->Option<Group>{
    fn v4(a:std::net::Ipv4Addr)->Option<Group>{
        let o=a.octets();
        let local=o[0]==0||o[0]==10||o[0]==127||o[0]>=224                         // this network, private, loopback, multicast, 240/4, broadcast
            ||(o[0]==100&&(o[1]&0xc0)==64)||(o[0]==169&&o[1]==254)                  // shared (CGNAT), link-local
            ||(o[0]==172&&(o[1]&0xf0)==16)||(o[0]==192&&o[1]==168)                  // private
            ||(o[0]==192&&o[1]==0&&o[2]==0&&o[3]!=9&&o[3]!=10)                      // IETF protocol assignments (.9, .10 are anycast)
            ||(o[0]==192&&o[1]==0&&o[2]==2)||(o[0]==198&&o[1]==51&&o[2]==100)||(o[0]==203&&o[1]==0&&o[2]==113) // documentation
            ||(o[0]==198&&(o[1]&0xfe)==18);                                         // benchmarking
        if local{None}else{Some(Group::V4([o[0],o[1]]))}
    }
    let a=match ip{IpAddr::V4(a)=>return v4(a),IpAddr::V6(a)=>a};
    let (s,o)=(a.segments(),a.octets());
    let own=Group::V6([o[0],o[1],o[2],o[3]]);
    let tunnel=|b:[u8;4]|Some(v4(b.into()).unwrap_or(own));
    match s{
        [0,0,0,0,0,0xffff,_,_]=>v4([o[12],o[13],o[14],o[15]].into()),
        [0,0,0,0,0xffff,0,_,_]|[0x64,0xff9b,0,0,0,0,_,_]=>tunnel([o[12],o[13],o[14],o[15]]),
        [0x2002,..]=>tunnel([o[2],o[3],o[4],o[5]]),
        [0x2001,0,..]=>tunnel([!o[12],!o[13],!o[14],!o[15]]),
        _ if (s[0]&0xfe00)==0x0200=>Some(Group::V6([0x02,0,0,0])),
        _=>{
            let ietf=s[0]==0x2001&&s[1]<0x200&&!(s[1]==1||s[1]==3||s[1]==4||(0x20..=0x3f).contains(&s[1])); // 2001::/23 minus anycast, AMT, AS112, ORCHIDv2, DET
            let local=a.is_unspecified()||a.is_loopback()||s[0]>=0xff00                // multicast
                ||matches!(s,[0x64,0xff9b,1,..])||matches!(s,[0x100,0,0,0,..])||ietf
                ||(s[0]==0x2001&&s[1]==0x0db8)||(s[0]==0x3fff&&s[1]<0x1000)||s[0]==0x5f00 // documentation, SRv6
                ||(s[0]&0xfe00)==0xfc00||(s[0]&0xffc0)==0xfe80||(s[0]&0xffc0)==0xfec0;   // unique-local, link-local, site-local
            if local{None}else{Some(own)}
        }
    }
}
/// Shared connection accounting of one node.
#[derive(Debug,Default)]
pub struct Counters{pub live:AtomicUsize,pub handshaking:AtomicUsize,pub refused:AtomicUsize,pub dropped_slow:AtomicUsize,
    /// Outbound connections (not counted in `live`/`handshaking`).
    pub outbound:AtomicUsize,
    /// Live inbound connections per public address group, and the connections refused because their
    /// group was full (also counted in `refused`).
    pub groups:Mutex<HashMap<Group,usize>>,pub refused_group:AtomicUsize}
#[derive(Clone,Debug)]
pub struct Net{pub limits:Limits,pub counters:Arc<Counters>,
    /// Addresses exempt from the per-group limit, canonical: the node's configured peers (`protect`).
    protected:Arc<std::sync::RwLock<HashSet<IpAddr>>>,
    /// The address grouping, `group_of` (unit tests group loopback sources to drive `listen` end to end).
    grouper:fn(IpAddr)->Option<Group>}
impl Net{
    pub fn new(limits:Limits)->Self{Self{limits,counters:Arc::new(Counters::default()),protected:Arc::default(),grouper:group_of}}
    #[cfg(test)]
    fn with_grouper(mut self,grouper:fn(IpAddr)->Option<Group>)->Self{self.grouper=grouper;self}
    /// Exempts these exact addresses from the per-group inbound limit. A node's configured peers (for
    /// example the other seeds, which dial it) are never refused for sharing an address group. Stored
    /// canonical: a dual-stack listener reports an IPv4 peer as ::ffff:a.b.c.d.
    pub fn protect(&self,ips:impl IntoIterator<Item=IpAddr>){if let Ok(mut p)=self.protected.write(){p.extend(ips.into_iter().map(|ip|ip.to_canonical()));}}
    /// The group an inbound connection from `ip` is counted in, or None when it is not limited by group.
    pub fn inbound_group(&self,ip:IpAddr)->Option<Group>{
        let ip=ip.to_canonical();
        if self.limits.per_group_max==0||self.protected.read().map_or(false,|p|p.contains(&ip)){return None}
        (self.grouper)(ip)
    }
    /// Reserves an inbound connection slot, a handshake slot and a place in `group` (at most
    /// `per_group_max` per group, also while other slots are free), or refuses.
    fn admit(&self,group:Option<Group>)->bool{
        let c=&self.counters;
        if c.live.fetch_add(1,Ordering::SeqCst)>=self.limits.peers_max{c.live.fetch_sub(1,Ordering::SeqCst);c.refused.fetch_add(1,Ordering::SeqCst);return false}
        if c.handshaking.fetch_add(1,Ordering::SeqCst)>=self.limits.handshaking_max{c.handshaking.fetch_sub(1,Ordering::SeqCst);c.live.fetch_sub(1,Ordering::SeqCst);c.refused.fetch_add(1,Ordering::SeqCst);return false}
        if let Some(g)=group{
            let placed=match c.groups.lock(){Ok(mut m)=>{let n=m.entry(g).or_insert(0);if *n<self.limits.per_group_max{*n+=1;true}else{false}}Err(_)=>false};
            if !placed{c.handshaking.fetch_sub(1,Ordering::SeqCst);c.live.fetch_sub(1,Ordering::SeqCst);c.refused.fetch_add(1,Ordering::SeqCst);c.refused_group.fetch_add(1,Ordering::SeqCst);return false}
        }
        true
    }
    /// Gives back the place an admitted inbound connection held in its group.
    fn release_group(&self,group:Option<Group>){
        let Some(g)=group else{return};
        if let Ok(mut m)=self.counters.groups.lock(){if let Some(n)=m.get_mut(&g){*n=n.saturating_sub(1);if *n==0{m.remove(&g);}}}
    }
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub enum Msg{
    /// Status (network.md:7): chainId, genesisHash, headHash, height, totalWork (32 B big-endian), name.
    Hello{chain_id:u64,genesis:[u8;32],head:[u8;32],height:u64,work:[u8;32],node:String,protocol:u64,compatibility:[u8;32]},
    Block{header:Vec<u8>,body:Vec<u8>},
    Template{shell:Vec<u8>,body:Vec<u8>},
    Share{tid:[u8;32],nonce:u64},
    GetBlock{hash:[u8;32]},
    GetRange{start:u64,count:u64},
    Tx{raw:Vec<u8>},
    /// GetHeaders-like range by locator (network.md:8: locator <= 32, count <= 512): the answer is the
    /// responder's canonical blocks after the highest locator hash on its canonical chain.
    GetFrom{locator:Vec<[u8;32]>,count:u64},
}
/// Locator hashes per GetFrom (network.md:8).
pub const LOCATOR_MAX:usize=32;
fn u64b(v:u64,o:&mut Vec<u8>){let b=v.to_be_bytes();let f=b.iter().position(|x|*x!=0).unwrap_or(8);rlp::encode_bytes(&b[f..],o)}
impl Msg{
    pub fn encode(&self)->Vec<u8>{
        let mut p=Vec::new();
        match self{
            Msg::Hello{chain_id,genesis,head,height,work,node,protocol,compatibility}=>{u64b(0,&mut p);u64b(*chain_id,&mut p);rlp::encode_bytes(genesis,&mut p);rlp::encode_bytes(head,&mut p);u64b(*height,&mut p);rlp::encode_bytes(work,&mut p);rlp::encode_bytes(node.as_bytes(),&mut p);u64b(*protocol,&mut p);rlp::encode_bytes(compatibility,&mut p);}
            Msg::Block{header,body}=>{u64b(1,&mut p);rlp::encode_bytes(header,&mut p);rlp::encode_bytes(body,&mut p);}
            Msg::Template{shell,body}=>{u64b(2,&mut p);rlp::encode_bytes(shell,&mut p);rlp::encode_bytes(body,&mut p);}
            Msg::Share{tid,nonce}=>{u64b(3,&mut p);rlp::encode_bytes(tid,&mut p);rlp::encode_bytes(&nonce.to_be_bytes(),&mut p);}
            Msg::GetBlock{hash}=>{u64b(4,&mut p);rlp::encode_bytes(hash,&mut p);}
            Msg::GetRange{start,count}=>{u64b(5,&mut p);u64b(*start,&mut p);u64b(*count,&mut p);}
            Msg::Tx{raw}=>{u64b(6,&mut p);rlp::encode_bytes(raw,&mut p);}
            Msg::GetFrom{locator,count}=>{u64b(7,&mut p);rlp::encode_bytes(&locator.concat(),&mut p);u64b(*count,&mut p);}
        }
        let mut o=Vec::new();rlp::encode_list_payload(&p,&mut o);o
    }
    pub fn decode(b:&[u8])->Result<Self,String>{
        let it=rlp::decode(b).map_err(|e|e.code().to_string())?;let f=it.list().ok_or("msg")?;
        let bytes=|i:usize|->Result<Vec<u8>,String>{f.get(i).and_then(|x|x.bytes()).map(|x|x.to_vec()).ok_or_else(||"field".to_string())};
        let num=|i:usize|->Result<u64,String>{let v=bytes(i)?;if v.len()>8||v.first()==Some(&0){return Err("int".into())}Ok(v.iter().fold(0u64,|a,x|(a<<8)|*x as u64))};
        let h32=|i:usize|->Result<[u8;32],String>{bytes(i)?.try_into().map_err(|_|"hash".to_string())};
        let n=|k:usize|->Result<(),String>{if f.len()==k{Ok(())}else{Err("arity".into())}};
        let m=match num(0)?{
            0=>{n(9)?;let name=bytes(6)?;if name.is_empty()||name.len()>NODE_NAME_MAX||!name.iter().all(|b|b.is_ascii_graphic()){return Err("node name".into())}
                Msg::Hello{chain_id:num(1)?,genesis:h32(2)?,head:h32(3)?,height:num(4)?,work:h32(5)?,node:String::from_utf8(name).map_err(|_|"utf8")?,protocol:num(7)?,compatibility:h32(8)?}}
            1=>{n(3)?;Msg::Block{header:bytes(1)?,body:bytes(2)?}}
            2=>{n(3)?;Msg::Template{shell:bytes(1)?,body:bytes(2)?}}
            3=>{n(3)?;let nb=bytes(2)?;if nb.len()!=8{return Err("nonce".into())}Msg::Share{tid:h32(1)?,nonce:u64::from_be_bytes(nb.try_into().unwrap())}}
            4=>{n(2)?;Msg::GetBlock{hash:h32(1)?}}
            5=>{n(3)?;Msg::GetRange{start:num(1)?,count:num(2)?}}
            6=>{n(2)?;Msg::Tx{raw:bytes(1)?}}
            7=>{n(3)?;let l=bytes(1)?;if l.is_empty()||l.len()%32!=0||l.len()>32*LOCATOR_MAX{return Err("locator".into())}
                Msg::GetFrom{locator:l.chunks(32).map(|c|c.try_into().unwrap()).collect(),count:num(2)?}}
            _=>return Err("type".into())
        };
        if m.encode()!=b{return Err("noncanonical".into())}
        Ok(m)
    }
}

pub type PeerId=u64;
#[derive(Debug)]
pub enum NetEvent{Connected(PeerId,Peer),Message(PeerId,Msg),Closed(PeerId)}

/// Outbound frames of one peer: bytes queued, closed flag; the writer thread drains it.
struct Outbox{q:Mutex<(VecDeque<Vec<u8>>,usize)>,cv:Condvar,closed:AtomicBool,max:usize}
#[derive(Clone)]
pub struct Peer{pub id:PeerId,pub addr:String,pub outbound:bool,out:Arc<Outbox>,sock:Arc<TcpStream>,counters:Arc<Counters>}
impl std::fmt::Debug for Peer{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{write!(f,"Peer({} {})",self.id,self.addr)}}
impl Peer{
    fn enqueue(&self,m:&Msg,limit:usize)->Result<(),bool>{
        if self.out.closed.load(Ordering::SeqCst){return Err(false)}
        let b=m.encode();if b.len()>MAX_FRAME{return Err(false)}
        let mut q=match self.out.q.lock(){Ok(q)=>q,Err(_)=>return Err(false)};
        if q.1+b.len()+4>limit{return Err(true)}
        q.1+=b.len()+4;let mut f=(b.len() as u32).to_be_bytes().to_vec();f.extend(b);q.0.push_back(f);drop(q);self.out.cv.notify_one();Ok(())
    }
    /// Gossip: never blocks. A peer whose queue is full stops reading; it is dropped.
    pub fn send(&self,m:&Msg)->bool{
        match self.enqueue(m,self.out.max){Ok(())=>true,Err(full)=>{if full{self.counters.dropped_slow.fetch_add(1,Ordering::SeqCst);self.close();}false}}
    }
    /// A response that may be cut short (range sync): false once half the queue is in use;
    /// the peer is kept and asks again.
    pub fn offer(&self,m:&Msg)->bool{self.enqueue(m,self.out.max/2).is_ok()}
    pub fn queued_bytes(&self)->usize{self.out.q.lock().map(|q|q.1).unwrap_or(0)}
    pub fn close(&self){close_outbox(&self.out);let _=self.sock.shutdown(std::net::Shutdown::Both);}
}

/// The flag is set under the queue lock: the writer tests it under that lock before waiting, so
/// a close between its test and its wait cannot lose the wakeup (the reader joins the writer).
fn close_outbox(out:&Outbox){{let _q=out.q.lock();out.closed.store(true,Ordering::SeqCst);}out.cv.notify_all();}
/// The writer's wait for its next frame; None once the peer is closed. `between` runs after the
/// closed test and before the wait, with the lock held (a test hook; a no-op in the node).
fn next_frame(out:&Outbox,between:&dyn Fn())->Option<Vec<u8>>{
    let Ok(mut q)=out.q.lock() else{return None};
    loop{
        if out.closed.load(Ordering::SeqCst){return None}
        if let Some(f)=q.0.pop_front(){q.1-=f.len();return Some(f)}
        between();
        q=match out.cv.wait(q){Ok(q)=>q,Err(_)=>return None};
    }
}
static NEXT:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(1);
/// The longest single wait for data. A closed peer's reader notices `closed` within it: shutdown
/// wakes a blocked read on Linux but not on Windows, where the reader would otherwise hold its slot
/// until the idle deadline.
const READ_POLL:Duration=Duration::from_millis(100);
/// Reads exactly `buf` before `deadline`; false on EOF, error, deadline or once `closed` is set. Each
/// read waits for data first (poll/WSAPoll, `rpc::sockwait`, at most READ_POLL), so it never blocks
/// and no receive ever times out on the socket: SO_RCVTIMEO is not used (on Windows a timed-out
/// receive leaves the socket in an indeterminate state). The socket stays blocking: the peer's
/// writer thread shares it.
fn read_by(r:&mut TcpStream,buf:&mut [u8],deadline:Instant,closed:&AtomicBool)->bool{
    let mut got=0;
    while got<buf.len(){
        if closed.load(Ordering::SeqCst){return false}
        let left=deadline.saturating_duration_since(Instant::now());if left.is_zero(){return false}
        match crate::rpc::sockwait::readable(r,left.min(READ_POLL)){Ok(true)=>{},Ok(false)=>continue,Err(_)=>return false}
        match r.read(&mut buf[got..]){Ok(0)=>return false,Ok(n)=>got+=n,Err(e) if e.kind()==std::io::ErrorKind::Interrupted=>{},Err(_)=>return false}
    }
    true
}
/// Starts reader and writer threads for an admitted connection (inbound: slot already reserved).
/// Returns whether the connection was handed to the node (every later end sends Closed), and
/// the reader's handle.
/// A13 thread inventory: the reader joins the writer, then delivers Closed, and only then
/// releases the slot, so a released slot never has a thread behind it that can still block (a
/// full event queue keeps the slot taken instead). The listener and the dialer join the reader
/// before its place is used again, so exiting readers are counted too.
/// `group`: the address group an inbound connection holds a place in (`Net::admit`), given back with
/// its slot, by the same single release.
fn spawn_peer(stream:TcpStream,outbound:bool,events:SyncSender<NetEvent>,net:Net,group:Option<Group>)->(bool,Option<std::thread::JoinHandle<()>>){
    let release=move|net:&Net,handshaking:bool|{
        if outbound{net.counters.outbound.fetch_sub(1,Ordering::SeqCst);}
        else{net.counters.live.fetch_sub(1,Ordering::SeqCst);if handshaking{net.counters.handshaking.fetch_sub(1,Ordering::SeqCst);}net.release_group(group);}
    };
    if outbound{net.counters.outbound.fetch_add(1,Ordering::SeqCst);}
    let _=stream.set_nodelay(true);let addr=stream.peer_addr().map(|a|a.to_string()).unwrap_or_default();
    let id=NEXT.fetch_add(1,Ordering::SeqCst);
    let (Ok(reader),Ok(writer))=(stream.try_clone(),stream.try_clone()) else{release(&net,true);return (false,None)};
    let out=Arc::new(Outbox{q:Mutex::new((VecDeque::new(),0)),cv:Condvar::new(),closed:AtomicBool::new(false),max:net.limits.queue_bytes_max});
    let peer=Peer{id,addr,outbound,out:out.clone(),sock:Arc::new(stream),counters:net.counters.clone()};
    let write_deadline=net.limits.write_deadline;
    let wpeer=peer.clone();
    let writer=crate::threads::spawn(format!("peer-{id}-w"),crate::threads::IO_STACK,move||{
        let mut w=writer;let _=w.set_write_timeout(Some(write_deadline));
        loop{
            let Some(frame)=next_frame(&out,&||{}) else{return};
            if let Err(e)=w.write_all(&frame){
                // A write that cannot finish within write_deadline: the peer stopped reading.
                if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut){wpeer.counters.dropped_slow.fetch_add(1,Ordering::SeqCst);}
                wpeer.close();return
            }
        }
    });
    let Ok(writer)=writer else{release(&net,true);return (false,None)};
    if events.send(NetEvent::Connected(id,peer.clone())).is_err(){peer.close();let _=writer.join();release(&net,true);return (false,None)}
    let writer=Arc::new(Mutex::new(Some(writer)));let rwriter=writer.clone();
    let join=|w:&Mutex<Option<std::thread::JoinHandle<()>>>|{if let Some(h)=w.lock().ok().and_then(|mut g|g.take()){let _=h.join();}};
    let rpeer=peer.clone();let rnet=net.clone();
    let tev=events.clone();
    let spawned=crate::threads::spawn(format!("peer-{id}"),crate::threads::IO_STACK,move||{let net=rnet;let events=tev;
        let mut r=reader;let l=net.limits.clone();let mut hello=false;let mut last=Instant::now();let started=Instant::now();
        loop{
            let wait=if hello{last+l.idle_deadline}else{started+l.hello_deadline};
            let mut len=[0u8;4];if !read_by(&mut r,&mut len,wait,&rpeer.out.closed){break}
            let n=u32::from_be_bytes(len) as usize;if n==0||n>MAX_FRAME{break}
            if !hello&&n>HELLO_MAX{break}
            // Before the Hello the whole handshake frame must also meet the Hello deadline.
            let frame_by=Instant::now()+l.frame_deadline;let frame_by=if hello{frame_by}else{frame_by.min(wait)};
            let mut buf=vec![0u8;n];if !read_by(&mut r,&mut buf,frame_by,&rpeer.out.closed){break}
            let Ok(m)=Msg::decode(&buf) else{break};
            if !hello{
                // Nothing but a Hello is accepted before the handshake.
                if !matches!(m,Msg::Hello{..}){break}
                hello=true;if !outbound{net.counters.handshaking.fetch_sub(1,Ordering::SeqCst);}
            }
            last=Instant::now();
            if events.send(NetEvent::Message(id,m)).is_err(){break}
        }
        rpeer.close();join(&rwriter);let _=events.send(NetEvent::Closed(id));release(&net,!hello);
    });
    // The node already knows this peer: it must also learn that it is gone.
    match spawned{Ok(h)=>(true,Some(h)),Err(_)=>{peer.close();join(&writer);let _=events.send(NetEvent::Closed(id));release(&net,true);(true,None)}}
}
/// Accepts inbound peers on the operator-selected listener, within the connection limits.
/// A loopback bind remains local; a private/public bind accepts remote peers. RPC separately
/// remains loopback-only. Every peer must pass the node's network/compatibility handshake.
pub fn listen(listener:TcpListener,events:SyncSender<NetEvent>,net:Net){
    crate::threads::spawn("p2p-listen",crate::threads::IO_STACK,move||{
        let mut readers=Vec::new();
        for s in listener.incoming(){
            let Ok(s)=s else{continue};
            // A reader that released its slot but is not yet joined still counts.
            crate::threads::reap(&mut readers);
            let room=readers.len()<net.limits.peers_max;if !room{net.counters.refused.fetch_add(1,Ordering::SeqCst);}
            let group=s.peer_addr().ok().and_then(|a|net.inbound_group(a.ip()));
            if room&&net.admit(group){if let (_,Some(h))=spawn_peer(s,false,events.clone(),net.clone(),group){readers.push(h);}}else{let _=s.shutdown(std::net::Shutdown::Both);}
        }
    }).expect("spawn listener");
}
/// Keeps an outbound connection to `addr`, reconnecting after loss. The node
/// maintains `connected` from Connected/Closed events (outbound peer addresses).
pub fn dial(addr:SocketAddr,events:SyncSender<NetEvent>,connected:Arc<Mutex<std::collections::HashSet<String>>>,net:Net){
    crate::threads::spawn(format!("dial-{addr}"),crate::threads::IO_STACK,move||{
        let mut reader:Option<std::thread::JoinHandle<()>>=None;
        loop{
            let live=connected.lock().map(|s|s.contains(&addr.to_string())).unwrap_or(false);
            if !live{
                // The node has seen Closed, which the previous reader sends just before it ends.
                if let Some(h)=reader.take(){let _=h.join();}
                if let Ok(s)=TcpStream::connect_timeout(&addr,Duration::from_millis(500)){
                    if let Ok(mut set)=connected.lock(){set.insert(addr.to_string());}
                    // Not handed to the node (no Closed will follow): dial again later.
                    let (handed,h)=spawn_peer(s,true,events.clone(),net.clone(),None);reader=h;
                    if !handed{if let Ok(mut set)=connected.lock(){set.remove(&addr.to_string());}}
                }
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }).expect("spawn dialer");
}

/// Unit tests only: a peer on a loopback pair whose outbox has no writer thread, so its queue only fills.
/// The other end of the pair is returned to keep the connection open.
#[cfg(test)]
pub(crate) fn test_peer(id:PeerId,max:usize)->(Peer,TcpStream){
    let l=TcpListener::bind("127.0.0.1:0").unwrap();let s=TcpStream::connect(l.local_addr().unwrap()).unwrap();let (r,_)=l.accept().unwrap();
    let out=Arc::new(Outbox{q:Mutex::new((VecDeque::new(),0)),cv:Condvar::new(),closed:AtomicBool::new(false),max});
    (Peer{id,addr:"test".into(),outbound:false,out,sock:Arc::new(s),counters:Arc::new(Counters::default())},r)
}
#[cfg(test)]
mod tests{
    use super::*;
    use std::sync::mpsc;
    /// D-REORG-REDB wire: Status carries the 32-byte totalWork, GetFrom a locator of 1..=32 hashes;
    /// both round-trip, and a locator that is empty, ragged or longer than 32 hashes is refused.
    #[test] fn status_work_and_locator_requests_round_trip_within_bounds(){
        let h=Msg::Hello{chain_id:7,genesis:[1;32],head:[2;32],height:9,work:[3;32],node:"n".into(),protocol:crate::identity::PROTOCOL_VERSION,compatibility:crate::identity::compatibility()};assert_eq!(Msg::decode(&h.encode()).unwrap(),h);
        let g=Msg::GetFrom{locator:vec![[4;32],[5;32]],count:512};assert_eq!(Msg::decode(&g.encode()).unwrap(),g);
        let full=Msg::GetFrom{locator:vec![[6;32];LOCATOR_MAX],count:1};assert_eq!(Msg::decode(&full.encode()).unwrap(),full);
        let raw=|loc:Vec<u8>|{let mut p=Vec::new();u64b(7,&mut p);rlp::encode_bytes(&loc,&mut p);u64b(1,&mut p);let mut o=Vec::new();rlp::encode_list_payload(&p,&mut o);o};
        for bad in [vec![],vec![0u8;31],vec![0u8;33],vec![0u8;32*(LOCATOR_MAX+1)]]{assert!(Msg::decode(&raw(bad)).is_err());}
        // The previous Hello (no work field) is refused by arity, never read with a default work.
        let mut p=Vec::new();u64b(0,&mut p);u64b(7,&mut p);rlp::encode_bytes(&[1;32],&mut p);rlp::encode_bytes(&[2;32],&mut p);u64b(9,&mut p);rlp::encode_bytes(b"n",&mut p);
        let mut o=Vec::new();rlp::encode_list_payload(&p,&mut o);assert!(Msg::decode(&o).is_err());
    }
    /// D-7 re-insertion relays with `offer`: once half the queue is in use an offer is refused and the peer is
    /// kept, while gossip (`send`) past the whole queue drops the peer as slow. No writer thread: the queue
    /// only fills.
    #[test] fn an_offer_past_half_the_queue_is_refused_and_keeps_the_peer(){
        let l=TcpListener::bind("127.0.0.1:0").unwrap();let s=TcpStream::connect(l.local_addr().unwrap()).unwrap();let _r=l.accept().unwrap();
        let out=Arc::new(Outbox{q:Mutex::new((VecDeque::new(),0)),cv:Condvar::new(),closed:AtomicBool::new(false),max:4096});
        let p=Peer{id:1,addr:"t".into(),outbound:false,out:out.clone(),sock:Arc::new(s),counters:Arc::new(Counters::default())};
        let m=Msg::Tx{raw:vec![7u8;400]};let f=m.encode().len()+4;
        let mut offered=0;while p.offer(&m){offered+=1;}
        assert_eq!(offered,2048/f,"offers fill half the queue");assert!(!out.closed.load(Ordering::SeqCst),"a refused offer keeps the peer");
        assert_eq!(p.counters.dropped_slow.load(Ordering::SeqCst),0);
        let mut sent=0;while p.send(&m){sent+=1;}
        assert_eq!(offered+sent,4096/f,"gossip fills the whole queue");
        assert!(out.closed.load(Ordering::SeqCst)&&p.counters.dropped_slow.load(Ordering::SeqCst)==1,"gossip past the queue drops the peer");
    }
    /// Windows run 2: shutdown does not wake a blocked read there, so a dropped peer's reader must
    /// see its closed flag. Here only the flag is set (no shutdown, the peer stays open and silent):
    /// the read returns within READ_POLL plus margin, not at its 10 s deadline.
    /// Round 3: the read waits through `rpc::sockwait` (no SO_RCVTIMEO); a read that blocked would never
    /// return here, so it runs on its own thread and the test fails after 2 s instead of hanging.
    #[test] fn a_reader_sees_its_closed_flag_without_a_shutdown(){
        let l=TcpListener::bind("127.0.0.1:0").unwrap();let _c=TcpStream::connect(l.local_addr().unwrap()).unwrap();
        let (mut r,_)=l.accept().unwrap();let closed=Arc::new(AtomicBool::new(false));
        let c=closed.clone();std::thread::spawn(move||{std::thread::sleep(Duration::from_millis(50));c.store(true,Ordering::SeqCst);});
        let (tx,rx)=mpsc::channel();let t=Instant::now();
        std::thread::spawn(move||{let mut b=[0u8;4];let _=tx.send(read_by(&mut r,&mut b,Instant::now()+Duration::from_secs(10),&closed));});
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)),Ok(false),"the reader waited {:?} after its peer was closed",t.elapsed());
    }
    /// Review F1: a close that lands after the writer found the outbox open and before it waits
    /// must still wake it. The hook holds the writer exactly there (with the queue lock) while the
    /// close is issued; the writer must then return within the timeout. Deterministic: the close
    /// is issued only once the writer is at the hook, and the hook is released only after the close
    /// has either completed (a close that ignores the lock) or blocked on the lock for 200 ms.
    #[test] fn a_close_between_the_writers_test_and_its_wait_wakes_it(){
        let out=Arc::new(Outbox{q:Mutex::new((VecDeque::new(),0)),cv:Condvar::new(),closed:AtomicBool::new(false),max:1<<20});
        let (at_tx,at_rx)=mpsc::channel::<()>();let (go_tx,go_rx)=mpsc::channel::<()>();let (done_tx,done_rx)=mpsc::channel();
        let o=out.clone();let go=Mutex::new(go_rx);let at=Mutex::new(at_tx);
        std::thread::spawn(move||{let hook=||{let _=at.lock().unwrap().send(());let _=go.lock().unwrap().recv();};let _=done_tx.send(next_frame(&o,&hook));});
        at_rx.recv_timeout(Duration::from_secs(5)).expect("the writer reaches the point between its test and its wait");
        let (closed_tx,closed_rx)=mpsc::channel();let c=out.clone();
        std::thread::spawn(move||{close_outbox(&c);let _=closed_tx.send(());});
        let closed_first=closed_rx.recv_timeout(Duration::from_millis(200)).is_ok();
        drop(go_tx);
        let r=done_rx.recv_timeout(Duration::from_secs(5));
        assert!(matches!(r,Ok(None)),"the writer was not woken by the close (close completed before the wait: {closed_first}): {r:?}");
        assert!(!closed_first,"the close must wait for the writer to release the queue lock");
        closed_rx.recv_timeout(Duration::from_secs(5)).expect("the close completes once the writer waits");
    }
    /// Inbound address groups: a public IPv4 peer is grouped by /16 and a public IPv6 peer by /32; an IPv6
    /// address that carries an IPv4 address is grouped as that address; addresses that are not globally
    /// reachable are not grouped, so devnets, LAN testnets and loopback tests keep their behaviour.
    #[test] fn public_addresses_are_grouped_by_netgroup_and_local_ones_are_not(){
        let ip=|s:&str|->IpAddr{s.parse().unwrap()};
        let v6=|s:&str|{let o=match ip(s){IpAddr::V6(a)=>a.octets(),_=>unreachable!()};Some(Group::V6([o[0],o[1],o[2],o[3]]))};
        assert_eq!(group_of(ip("8.8.8.8")),Some(Group::V4([8,8])));
        assert_eq!(group_of(ip("8.8.200.1")),group_of(ip("8.8.8.8")),"same /16");
        assert_ne!(group_of(ip("8.9.8.8")),group_of(ip("8.8.8.8")),"another /16");
        assert_eq!(group_of(ip("2606:4700:10::1")),Some(Group::V6([0x26,0x06,0x47,0x00])));
        assert_eq!(group_of(ip("2606:4700:ffff::9")),group_of(ip("2606:4700:10::1")),"same /32");
        assert_ne!(group_of(ip("2606:4701::1")),group_of(ip("2606:4700:10::1")),"another /32");
        // An IPv4 address carried in IPv6 is grouped as that address: a tunnel is no second way into a /16.
        for carried in ["::ffff:8.8.4.4","::ffff:0:8.8.4.4","64:ff9b::808:404","2002:808:404::1","2001:0:4136:e378:8000:63bf:f7f7:fbfb"]{
            assert_eq!(group_of(ip(carried)),Some(Group::V4([8,8])),"{carried} carries 8.8.x.x");
        }
        assert_eq!(group_of(ip("2002:c0a8:101::1")),v6("2002:c0a8:101::1"),"a tunnel form of a private IPv4 address keeps its own /32");
        assert_eq!(group_of(ip("200:1234:5678::1")),group_of(ip("3ff:9::1")),"200::/7 (key-derived, Yggdrasil) is one group");
        // The globally reachable parts of 2001::/23 are grouped like any public /32.
        for public in ["2001:1::1","2001:3::1","2001:4:112::1","2001:20::1","2001:3f::1","192.0.0.9","192.0.0.10"]{
            assert!(group_of(ip(public)).is_some(),"{public} is globally reachable");
        }
        for local in ["127.0.0.1","127.3.4.5","10.1.2.3","172.16.0.9","172.31.255.1","192.168.1.1","169.254.0.1","100.64.0.1","100.127.255.1",
            "0.0.0.0","0.1.2.3","255.255.255.255","224.0.0.1","192.0.2.1","198.51.100.7","203.0.113.9",
            "192.0.0.1","192.0.0.255","198.18.0.1","198.19.255.255","240.0.0.1","250.1.2.3",
            "::1","::","fe80::1","fc00::1","fc00:f853:ccd:e793::2","fd00::1","fd12:3456::1","fdff::1","fec0::1","ff02::1","2001:db8::1","3fff::1","3fff:fff::1",
            "64:ff9b:1::1","100::1","2001:2::1","2001:10::1","2001:1ff::1","5f00::1","::ffff:127.0.0.1","::ffff:10.0.0.1"]{
            assert_eq!(group_of(ip(local)),None,"{local} is not grouped");
        }
        assert!(group_of(ip("100.128.0.1")).is_some()&&group_of(ip("100.63.255.1")).is_some(),"just outside the shared space 100.64/10");
        assert!(group_of(ip("172.32.0.1")).is_some()&&group_of(ip("198.20.0.1")).is_some()&&group_of(ip("223.255.255.1")).is_some(),"just outside 172.16/12, 198.18/15, 224/4");
        assert!(group_of(ip("2001:200::1")).is_some()&&group_of(ip("3fff:1000::1")).is_some(),"just outside 2001::/23 and 3fff::/20");
    }
    /// Per-group admission: a group holds at most per_group_max inbound places (by default half of the
    /// inbound slots), also while other slots are free; a refusal by group takes no slot; a released place can
    /// be taken again; other groups and ungrouped peers are unaffected; a configured peer is exempt by its
    /// canonical address (a dual-stack listener reports IPv4 as ::ffff:a.b.c.d); per_group_max 0 disables it.
    #[test] fn a_public_group_holds_at_most_per_group_max_inbound_places(){
        let d=Limits::default();assert_eq!(d.per_group_max,d.peers_max/2,"by default one group holds at most half of the inbound slots");
        let net=Net::new(Limits{peers_max:8,handshaking_max:8,per_group_max:3,..Limits::default()});
        let (ga,gb)=(group_of("81.2.69.1".parse().unwrap()),group_of("81.3.1.1".parse().unwrap()));
        assert!(ga.is_some()&&gb.is_some()&&ga!=gb);
        let c=&net.counters;let load=|a:&AtomicUsize|a.load(Ordering::SeqCst);
        let places=|g:Option<Group>|c.groups.lock().unwrap().get(&g.unwrap()).copied().unwrap_or(0);
        // What spawn_peer's single release does when a connection ends.
        let close=|g:Option<Group>|{c.live.fetch_sub(1,Ordering::SeqCst);c.handshaking.fetch_sub(1,Ordering::SeqCst);net.release_group(g);};
        assert!(net.admit(ga)&&net.admit(ga)&&net.admit(ga),"per_group_max places in one group");
        assert!(!net.admit(ga),"one more from that group is refused although 5 slots are free");
        assert_eq!((load(&c.live),load(&c.handshaking),load(&c.refused),load(&c.refused_group)),(3,3,1,1),"a refusal by group takes no slot");
        assert!(net.admit(gb)&&net.admit(None)&&net.admit(None),"another group and ungrouped peers are admitted");
        close(ga);assert_eq!(places(ga),2);assert!(net.admit(ga),"a released place can be taken again");
        assert_eq!((places(ga),places(gb)),(3,1));
        close(gb);assert_eq!(c.groups.lock().unwrap().get(&gb.unwrap()),None,"an empty group is removed");
        // Configured peers are exempt by their exact canonical address, also as seen on a dual-stack listener.
        let mapped:IpAddr="::ffff:81.2.69.8".parse().unwrap();
        assert_eq!(net.inbound_group(mapped),ga,"a mapped stranger is grouped as its IPv4 /16");
        net.protect(["81.2.69.8".parse::<IpAddr>().unwrap()]);
        assert_eq!(net.inbound_group(mapped),None,"a configured IPv4 peer on a dual-stack listener is not grouped");
        assert_eq!(net.inbound_group("81.2.69.8".parse().unwrap()),None,"nor on an IPv4 listener");
        net.protect(["::ffff:81.2.69.9".parse::<IpAddr>().unwrap()]);
        assert_eq!(net.inbound_group("81.2.69.9".parse().unwrap()),None,"a mapped configured address protects its IPv4 form");
        assert_eq!(net.inbound_group("81.2.69.10".parse().unwrap()),ga,"its neighbours are still grouped");
        assert_eq!(net.inbound_group("127.0.0.1".parse().unwrap()),None,"loopback is not grouped");
        let off=Net::new(Limits{per_group_max:0,..Limits::default()});
        assert_eq!(off.inbound_group("81.2.69.1".parse().unwrap()),None,"per_group_max 0 disables the limit");
    }
    /// End to end through `listen` and `spawn_peer` (every loopback source counted in one public group): an
    /// accepted connection takes its group's place at accept, a refused one takes none, and each connection
    /// gives its place back exactly once, whether it closes after its Hello or never sends a Hello. The
    /// connections that must stay send a Hello, so a slow runner cannot end them early at the Hello deadline.
    #[test] fn listen_takes_and_gives_back_one_group_place_per_inbound_connection(){
        const G:Group=Group::V4([81,2]);
        fn one_group(ip:IpAddr)->Option<Group>{if ip.is_loopback(){Some(G)}else{group_of(ip)}}
        let net=Net::new(Limits{peers_max:4,handshaking_max:4,per_group_max:2,hello_deadline:Duration::from_millis(800),..Limits::default()}).with_grouper(one_group);
        let l=TcpListener::bind("127.0.0.1:0").unwrap();let addr=l.local_addr().unwrap();
        let (tx,rx)=mpsc::sync_channel(NET_EVENT_QUEUE);listen(l,tx,net.clone());
        std::thread::spawn(move||{for _ in rx{}});
        let c=net.counters.clone();
        let places=||c.groups.lock().unwrap().get(&G).copied().unwrap_or(0);
        let shaking=||c.handshaking.load(Ordering::SeqCst);
        let until=|f:&dyn Fn()->bool|{let t=Instant::now();while !f()&&t.elapsed()<Duration::from_secs(10){std::thread::sleep(Duration::from_millis(5));}f()};
        let closed=|s:&mut TcpStream|{s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();let mut b=[0u8;1];matches!(s.read(&mut b),Ok(0)|Err(_))};
        let hello=||{let m=Msg::Hello{chain_id:7,genesis:[1;32],head:[1;32],height:0,work:[0;32],node:"g".into(),protocol:1,compatibility:[0;32]}.encode();
            let mut f=(m.len() as u32).to_be_bytes().to_vec();f.extend(m);f};
        let mut a=TcpStream::connect(addr).unwrap();a.write_all(&hello()).unwrap();
        let mut b=TcpStream::connect(addr).unwrap();b.write_all(&hello()).unwrap();
        assert!(until(&||places()==2&&shaking()==0),"two connections take the group's places; their Hellos end their handshakes");
        let mut x=TcpStream::connect(addr).unwrap();
        assert!(closed(&mut x),"a third connection from the group is closed at accept although slots are free");
        assert_eq!((c.refused_group.load(Ordering::SeqCst),places()),(1,2),"the refused connection took no place");
        drop(a);assert!(until(&||places()==1),"a closed connection gives back its place");
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(places(),1,"exactly once: b still holds its place");
        let mut d=TcpStream::connect(addr).unwrap();assert!(until(&||places()==2),"the freed place is taken again");
        assert!(closed(&mut d),"a connection that never sends a Hello is closed at its deadline");
        assert!(until(&||places()==1),"and gives back its place");
        drop(b);
        assert!(until(&||places()==0&&c.live.load(Ordering::SeqCst)==0&&shaking()==0),"all places and slots are back");
        assert!(c.groups.lock().unwrap().is_empty(),"no empty group is left behind");
    }
}
