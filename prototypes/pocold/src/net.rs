//! Peer transport (VA-spec v1, development/VA-SPEC.md): TCP on loopback, frames
//! [u32 BE length][RLP([type, ...fields])], length <= MAX_FRAME. Malformed frames
//! or a wrong network identity close the connection; they never touch block state.
//!
//! Resource bounds (A05, local and not consensus; `Limits`):
//! - at most `peers_max` inbound connections, of which at most `handshaking_max` have not
//!   yet delivered their Hello; excess connections are closed on accept, before any thread is
//!   spawned. Outbound connections are separate (one per configured peer, `dial`), so inbound
//!   connections can never take the slots the node needs to reach its own peers;
//! - the first frame must be a Hello and must arrive within `hello_deadline`; a frame
//!   whose length has arrived must complete within `frame_deadline`; an established peer
//!   silent for `idle_deadline` is closed;
//! - every peer has a writer thread and an outbound queue of at most `queue_bytes_max`;
//!   the consensus thread only enqueues. Gossip to a full queue drops that peer (a peer
//!   that stops reading never blocks consensus); range responses stop at half the queue;
//! - inbound frames wait in a bounded event queue (backpressure stops the reader), so
//!   frame memory is at most (peers_max + NET_EVENT_QUEUE) · MAX_FRAME.
use std::collections::VecDeque;
use std::io::{Read,Write};
use std::net::{TcpListener,TcpStream,SocketAddr};
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

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Limits{
    pub peers_max:usize,pub handshaking_max:usize,
    pub hello_deadline:Duration,pub frame_deadline:Duration,pub idle_deadline:Duration,
    pub queue_bytes_max:usize,pub write_deadline:Duration,
}
impl Default for Limits{
    // D-A13-BUDGET: peers_max 32 -> 8 inbound (network.md: <= 50 peers); a devnet node has two.
    fn default()->Self{Self{peers_max:8,handshaking_max:8,hello_deadline:Duration::from_secs(5),frame_deadline:Duration::from_secs(30),
        idle_deadline:Duration::from_secs(300),queue_bytes_max:2*1024*1024,write_deadline:Duration::from_secs(10)}}
}
/// Shared connection accounting of one node.
#[derive(Debug,Default)]
pub struct Counters{pub live:AtomicUsize,pub handshaking:AtomicUsize,pub refused:AtomicUsize,pub dropped_slow:AtomicUsize,
    /// Outbound connections (not counted in `live`/`handshaking`).
    pub outbound:AtomicUsize}
#[derive(Clone,Debug)]
pub struct Net{pub limits:Limits,pub counters:Arc<Counters>}
impl Net{
    pub fn new(limits:Limits)->Self{Self{limits,counters:Arc::new(Counters::default())}}
    /// Reserves an inbound connection slot (and a handshake slot), or refuses.
    fn admit(&self)->bool{
        let c=&self.counters;
        if c.live.fetch_add(1,Ordering::SeqCst)>=self.limits.peers_max{c.live.fetch_sub(1,Ordering::SeqCst);c.refused.fetch_add(1,Ordering::SeqCst);return false}
        if c.handshaking.fetch_add(1,Ordering::SeqCst)>=self.limits.handshaking_max{c.handshaking.fetch_sub(1,Ordering::SeqCst);c.live.fetch_sub(1,Ordering::SeqCst);c.refused.fetch_add(1,Ordering::SeqCst);return false}
        true
    }
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub enum Msg{
    /// Status (network.md:7): chainId, genesisHash, headHash, height, totalWork (32 B big-endian), name.
    Hello{chain_id:u64,genesis:[u8;32],head:[u8;32],height:u64,work:[u8;32],node:String},
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
            Msg::Hello{chain_id,genesis,head,height,work,node}=>{u64b(0,&mut p);u64b(*chain_id,&mut p);rlp::encode_bytes(genesis,&mut p);rlp::encode_bytes(head,&mut p);u64b(*height,&mut p);rlp::encode_bytes(work,&mut p);rlp::encode_bytes(node.as_bytes(),&mut p);}
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
            0=>{n(7)?;Msg::Hello{chain_id:num(1)?,genesis:h32(2)?,head:h32(3)?,height:num(4)?,work:h32(5)?,node:String::from_utf8(bytes(6)?).map_err(|_|"utf8")?}}
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
fn spawn_peer(stream:TcpStream,outbound:bool,events:SyncSender<NetEvent>,net:Net)->(bool,Option<std::thread::JoinHandle<()>>){
    let release=move|net:&Net,handshaking:bool|{
        if outbound{net.counters.outbound.fetch_sub(1,Ordering::SeqCst);}
        else{net.counters.live.fetch_sub(1,Ordering::SeqCst);if handshaking{net.counters.handshaking.fetch_sub(1,Ordering::SeqCst);}}
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
/// Accepts inbound peers on `listener` (loopback only), within the connection limits.
pub fn listen(listener:TcpListener,events:SyncSender<NetEvent>,net:Net){
    crate::threads::spawn("p2p-listen",crate::threads::IO_STACK,move||{
        let mut readers=Vec::new();
        for s in listener.incoming(){
            let Ok(s)=s else{continue};
            if !s.peer_addr().map(|a|a.ip().is_loopback()).unwrap_or(false){continue}
            // A reader that released its slot but is not yet joined still counts.
            crate::threads::reap(&mut readers);
            let room=readers.len()<net.limits.peers_max;if !room{net.counters.refused.fetch_add(1,Ordering::SeqCst);}
            if room&&net.admit(){if let (_,Some(h))=spawn_peer(s,false,events.clone(),net.clone()){readers.push(h);}}else{let _=s.shutdown(std::net::Shutdown::Both);}
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
                    let (handed,h)=spawn_peer(s,true,events.clone(),net.clone());reader=h;
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
        let h=Msg::Hello{chain_id:7,genesis:[1;32],head:[2;32],height:9,work:[3;32],node:"n".into()};assert_eq!(Msg::decode(&h.encode()).unwrap(),h);
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
}
