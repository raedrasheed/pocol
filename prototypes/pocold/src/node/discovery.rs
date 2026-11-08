//! Peer discovery, the node side (VA-SPEC §1a; local policy: it never changes validity, chain state or admission). The
//! node tells its connections apart by `net::Origin`: one it dialed is known by the address it dialed (the outbound set
//! of `net::dial`, H2), never by the address its socket reports.
//! - Advertising (VA-SPEC §1a.1, §2): a listening node with discovery on sends its listener's port in Hello.listenPort,
//!   and every listening node sends its per-process nonce, whatever the setting. A connection's first Hello carrying
//!   this node's own nonce is a self-connection: it is closed before any Status is kept and the address dialed for it
//!   is not dialed again (H1: a seed among its own configured peers held a connection to itself).
//! - Learning (VA-SPEC §1a.2), from a connection's first compatible Hello only: an inbound connection's source IP with
//!   its listenPort (a self-advertisement), and the address this node dialed (verified with listenPort != 0, forgotten
//!   with 0); then the one Addr that answers this node's GetAddr on a connection it dialed, within
//!   `getaddr_timeout_ms`. Nothing else is learned, and learned addresses never enter `Net::protect`.
//! - Answering (VA-SPEC §1a.5): the first GetAddr of an inbound connection, through `Peer::offer`.
//!
//! Every other discovery message is ignored and counted; nothing here closes or penalizes a peer (a malformed frame
//! still closes its connection in the reader, VA-SPEC §1).
use std::collections::HashMap;
use std::net::SocketAddr;
use crate::addrbook::{self,Book,Kind,Outcome,Source};
use crate::net::{self,Msg,Origin,PeerId};
use super::{Node,now_ms};

/// Peer discovery's local settings (`node::Limits::discovery`, VA-SPEC §1a). `enabled`: learn, answer and advertise
/// the listener's port (off: `--no-discovery`, the escape hatch; the nonce check and the H2 fix still apply);
/// `allow_local`: local mode (`--discover-local`), which stores addresses without an address group when they came
/// from a source without one. `rng_seed` and `times` are for tests only (plan D13: no flag or variable sets them).
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Discovery{pub enabled:bool,pub allow_local:bool,pub rng_seed:Option<u64>,pub times:DiscoveryTimes}
impl Default for Discovery{fn default()->Self{Self{enabled:true,allow_local:false,rng_seed:None,times:DiscoveryTimes::default()}}}
/// Discovery's timings, in milliseconds: the period of its tick; the book's (`addrbook::Times`: the relay window, the
/// horizon after which an entry is terrible, the failure window, the release hold and its cap, the answer list's
/// age); how long an outstanding GetAddr waits for its Addr; the first save after the network's start and the period
/// of the later ones; and how often live connections refresh their entries.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct DiscoveryTimes{pub tick_ms:u64,pub relay_window_ms:u64,pub horizon_ms:u64,pub fail_recent_ms:u64,pub hold_base_ms:u64,pub hold_max_ms:u64,
    pub cache_ms:u64,pub getaddr_timeout_ms:u64,pub save_first_ms:u64,pub save_every_ms:u64,pub refresh_ms:u64}
impl Default for DiscoveryTimes{
    fn default()->Self{let b=addrbook::Times::default();
        Self{tick_ms:500,relay_window_ms:b.relay_window_ms,horizon_ms:b.horizon_ms,fail_recent_ms:b.fail_recent_ms,hold_base_ms:b.hold_base_ms,hold_max_ms:b.hold_max_ms,
            cache_ms:b.cache_ms,getaddr_timeout_ms:120_000,save_first_ms:60_000,save_every_ms:900_000,refresh_ms:3_600_000}}
}
impl DiscoveryTimes{
    fn book(&self)->addrbook::Times{addrbook::Times{relay_window_ms:self.relay_window_ms,horizon_ms:self.horizon_ms,fail_recent_ms:self.fail_recent_ms,hold_base_ms:self.hold_base_ms,hold_max_ms:self.hold_max_ms,cache_ms:self.cache_ms}}
}

/// One connection's discovery state, from its Connected to its Closed (discovery on only). `advertised`: the endpoint
/// an inbound connection advertised in its first Hello; `compatible`: its first Hello was compatible; `consent`: a
/// connection this node dialed announced a listener (listenPort != 0), so its dialed address is verified; `asked_ms`:
/// the GetAddr sent on a connection this node dialed and not yet answered or expired; `answered`: an inbound
/// connection's GetAddr was taken (at most one per connection).
struct Link{origin:Origin,remote:Option<SocketAddr>,advertised:Option<SocketAddr>,compatible:bool,consent:bool,asked_ms:Option<u64>,answered:bool}
/// Shown in `pocol_status` as `discovery.counters`. GetAddr messages: sent, answered, ignored (on a connection this
/// node dialed, a second one, or discovery off), expired unanswered, and answers not queued because the requester's
/// queue was past half. Addr messages: taken as the answer to this node's GetAddr, and ignored. The records of the
/// answers taken: new entries (added, or in place of another), entries refreshed, and records dropped (not learnable,
/// no room under the quotas, or a repeat within the answer). Self-advertisements received, and connections closed
/// as self-connections (each end of one counts).
#[derive(Default)]
struct Counters{get_addr_sent:u64,get_addr_answered:u64,get_addr_ignored:u64,get_addr_timeouts:u64,answer_skipped_queue:u64,
    addr_received:u64,addr_unsolicited:u64,addr_accepted:u64,addr_refreshed:u64,addr_dropped:u64,self_ads:u64,self_connections:u64}
/// The node's discovery state. `listening`: it has a listener (`cfg.listen`), so it sends `nonce` (nonzero, drawn once
/// per process); `listen_port`: the bound listener's port, read when the network starts with discovery on; `rng`:
/// the book's shuffles (seeded from the OS, or `Discovery::rng_seed`); `links`: each connection's state.
pub(super) struct Disc{on:bool,allow_local:bool,listening:bool,nonce:u64,listen_port:u16,book:Book,rng:addrbook::Rng,links:HashMap<PeerId,Link>,
    times:DiscoveryTimes,next_tick_ms:u64,next_refresh_ms:u64,counters:Counters,
    /// Tests: the clock the hooks read (`Node::disc_now`).
    #[cfg(test)] test_now:Option<u64>}
impl Disc{
    /// At `Node::open`: the nonce and the generator's seed from the OS (`Err` without OS randomness), and an empty book,
    /// which reserves its BOOK_MAX entries only with discovery on.
    pub(super) fn open(s:&Discovery,listening:bool)->Result<Self,String>{
        let mut r=[0u8;16];getrandom::getrandom(&mut r).map_err(|_|"OS randomness unavailable")?;
        let [nonce,seed]=[0,8].map(|i|u64::from_be_bytes(r[i..i+8].try_into().unwrap()));
        let mut book=if s.enabled{Book::new(s.allow_local)}else{Book::disabled()};book.set_times(s.times.book());
        Ok(Self{on:s.enabled,allow_local:s.allow_local,listening,nonce:nonce.max(1),listen_port:0,book,rng:addrbook::Rng::new(s.rng_seed.unwrap_or(seed)),
            links:HashMap::new(),times:s.times,next_tick_ms:0,next_refresh_ms:0,counters:Counters::default(),#[cfg(test)]test_now:None})
    }
    /// The network started with this listener port (`start_network`): advertised only with discovery on.
    pub(super) fn bound(&mut self,port:u16){if self.on{self.listen_port=port}}
    /// Hello.listenPort (VA-SPEC §1a.1): the listener's port with discovery on, else 0.
    pub(super) fn listen_port(&self)->u16{if self.on{self.listen_port}else{0}}
    /// Hello.nonce (VA-SPEC §2): this process's nonce on a listening node, else 0 (a node that does not listen cannot
    /// be dialed by itself, and sends nothing that would link it across addresses).
    pub(super) fn wire_nonce(&self)->u64{if self.listening{self.nonce}else{0}}
}

impl Node{
    /// The clock of the discovery hooks, ms since the epoch (tests set their own).
    pub(super) fn disc_now(&self)->u64{
        #[cfg(test)]{if let Some(t)=self.disc.test_now{return t}}
        now_ms() as u64
    }
    pub(super) fn disc_connected(&mut self,id:PeerId,p:&net::Peer){
        if self.disc.on{self.disc.links.insert(id,Link{origin:p.origin,remote:p.remote,advertised:None,compatible:false,consent:false,asked_ms:None,answered:false});}
    }
    /// A connection ended. One this node dialed that consented refreshes its verification (`Book::touch`).
    pub(super) fn disc_closed(&mut self,id:PeerId,now:u64){
        let Some(l)=self.disc.links.remove(&id) else{return};
        if let Some(a)=l.origin.dialed().filter(|_|l.compatible&&l.consent){self.disc.book.touch(a,false,now);}
    }
    /// VA-SPEC §2: a first Hello carrying this node's own nonce. The connection is closed and removed before any Status
    /// is kept, so `compatiblePeers` never counts it, and an address this node dialed for it is marked as its own
    /// (`Book::mark_self`). A configured address stays in the outbound set: its peer is gone from `peers` before its
    /// Closed arrives, so the Closed handler does not clear the set (`allow_redial`), and the dialer, which sees the
    /// address as live, stops dialing it (H1). An inbound end (the other end of the same connection, or a peer that
    /// replays the nonce) marks nothing and is closed once this node's Hello, queued at Connected, is written
    /// (`Peer::finish`): closed at once, it could cut that Hello off, and the dialing end, which alone knows the
    /// address, would see the connection end before any Hello and dial again (1 in 5 loopback runs needed 2 or 3 more
    /// connections before the dialer stopped).
    pub(super) fn disc_self_connection(&mut self,from:PeerId){
        let Some(p)=self.peers.remove(&from) else{return};
        match p.origin.dialed(){Some(a)=>{p.close();self.disc.book.mark_self(a);}None=>p.finish()}
        self.disc.counters.self_connections+=1;
        self.log(&format!("peer {}: a connection to this node itself (its nonce); closing",p.addr));
    }
    /// VA-SPEC §1a.4: a connection this node dialed whose Hello names another chain or genesis (`network`) forgets the
    /// dialed address; another release (protocol version or compatibility digest) holds it back for a while, since
    /// after an upgrade the network is split until its nodes have upgraded.
    pub(super) fn disc_incompatible(&mut self,from:PeerId,network:bool,now:u64){
        let Some(a)=self.peers.get(&from).and_then(|p|p.origin.dialed()) else{return};
        if network{self.disc.book.remove(a)}else{self.disc.book.held(a,now)}
    }
    /// VA-SPEC §1a.2: a connection's first compatible Hello. Inbound with listenPort != 0: its source IP with that port
    /// is a self-advertisement, never another IP. Dialed by this node: the dialed address (never the port the Hello
    /// claims) is verified when listenPort != 0 and forgotten when it is 0 (consent withdrawn); then the peer is asked
    /// for addresses, once (VA-SPEC §1a.5).
    pub(super) fn disc_first_hello(&mut self,from:PeerId,port:u16,now:u64){
        if !self.disc.on{return}
        let d=&mut self.disc;let Some(l)=d.links.get_mut(&from) else{return};
        l.compatible=true;
        match l.origin.dialed(){
            None=>{
                let Some(r)=l.remote.filter(|_|port!=0) else{return};
                let a=SocketAddr::new(r.ip().to_canonical(),port);l.advertised=Some(a);d.counters.self_ads+=1;
                d.book.insert_unverified(a,Source::Inbound,Kind::Direct,net::group_of(a.ip()).is_none(),now);
            }
            Some(a)=>{
                if port!=0{l.consent=true;d.book.good(a,matches!(l.origin,Origin::Configured(_)),net::group_of(a.ip()).is_none(),now);}else{d.book.remove(a);}
                l.asked_ms=Some(now);d.counters.get_addr_sent+=1;
                if let Some(p)=self.peers.get(&from){p.send(&Msg::GetAddr);}
            }
        }
    }
    /// VA-SPEC §1a.5: GetAddr is answered once per inbound connection (its first GetAddr), never on a connection this
    /// node dialed, with an Addr of at most `net::ADDR_MAX` entries, possibly none: the book's answer without the
    /// requester's own advertised endpoint, and without local entries unless the requester is local itself. It goes
    /// through `Peer::offer`, so it is queued only while the requester's queue is below half, and the peer is kept
    /// either way.
    pub(super) fn disc_getaddr(&mut self,from:PeerId,now:u64){
        let d=&mut self.disc;
        let Some(l)=d.links.get_mut(&from).filter(|l|l.origin==Origin::Inbound&&!l.answered) else{d.counters.get_addr_ignored+=1;return};
        l.answered=true;
        let local=l.remote.is_some_and(|r|net::group_of(r.ip()).is_none());
        let m=Msg::Addr{addrs:d.book.answer(local,l.advertised,&mut d.rng,now)};
        if self.peers.get(&from).is_some_and(|p|p.offer(&m)){d.counters.get_addr_answered+=1}else{d.counters.answer_skipped_queue+=1}
    }
    /// VA-SPEC §1a.2c: an Addr is taken only as the answer to this node's GetAddr on a connection it dialed, within
    /// `getaddr_timeout_ms`, once; its records are heard from that peer (a configured peer by its address, a slot's by
    /// its key), local ones only when the peer is local itself. Anything else is ignored and counted.
    pub(super) fn disc_addr(&mut self,from:PeerId,addrs:Vec<SocketAddr>,now:u64){
        let d=&mut self.disc;
        let Some((a,t,l))=d.links.get_mut(&from).and_then(|l|Some((l.origin.dialed()?,l.asked_ms?,l))) else{d.counters.addr_unsolicited+=1;return};
        if now.saturating_sub(t)>d.times.getaddr_timeout_ms{d.counters.addr_unsolicited+=1;return}
        l.asked_ms=None;d.counters.addr_received+=1;
        let src=match l.origin{Origin::Slot(..)=>Source::Peer(match net::group_of(a.ip()){Some(g)=>addrbook::Key::Net(g),None=>addrbook::Key::Local(SocketAddr::new(a.ip().to_canonical(),a.port()))}),_=>Source::Cfg(a)};
        let from_local=net::group_of(a.ip()).is_none();
        for (i,x) in addrs.iter().enumerate(){
            if addrs[..i].contains(x){d.counters.addr_dropped+=1;continue}
            match d.book.insert_unverified(*x,src,Kind::Heard,from_local,now){Outcome::Added|Outcome::Replaced=>d.counters.addr_accepted+=1,
                Outcome::Refreshed=>d.counters.addr_refreshed+=1,Outcome::Dropped(_)=>d.counters.addr_dropped+=1}
        }
    }
    /// From `tick`, at most every `tick_ms`: an outstanding GetAddr expires after `getaddr_timeout_ms` (its Addr would
    /// be ignored from then on), and every `refresh_ms` the live connections refresh their entries (`Book::touch`: the
    /// verification of a consenting connection this node dialed, the advertisement of an inbound one), so that a
    /// long-lived connection stays within the relay window.
    pub(super) fn disc_tick(&mut self,now:u64){
        let d=&mut self.disc;
        if !d.on||now<d.next_tick_ms{return}
        d.next_tick_ms=now.saturating_add(d.times.tick_ms);
        for l in d.links.values_mut(){if l.asked_ms.is_some_and(|t|now.saturating_sub(t)>d.times.getaddr_timeout_ms){l.asked_ms=None;d.counters.get_addr_timeouts+=1;}}
        if now>=d.next_refresh_ms{d.next_refresh_ms=now.saturating_add(d.times.refresh_ms);
            for l in d.links.values(){match (l.origin.dialed(),l.advertised){(Some(a),_) if l.compatible&&l.consent=>d.book.touch(a,false,now),(None,Some(a))=>d.book.touch(a,true,now),_=>{}}}}
    }
    /// `pocol_status.discovery` (plan §8): the settings, the book's size (never its contents), this node's own
    /// addresses (at most addrbook::SELF_MAX) and the counters.
    pub(super) fn disc_status(&self)->serde_json::Value{
        let (d,now)=(&self.disc,self.disc_now());let e=d.book.entries();
        let mut keys:Vec<addrbook::Key>=e.iter().map(|e|e.key).collect();keys.sort_unstable();keys.dedup();
        let c=&d.counters;
        serde_json::json!({"enabled":d.on,"allowLocal":d.allow_local,"listenPort":d.listen_port(),"selfDetection":d.wire_nonce()!=0,
            "book":{"entries":e.len(),"verified":e.iter().filter(|x|x.verified_ms>0).count(),"relayable":e.iter().filter(|x|d.book.relayable(x,now)).count(),"keys":keys.len(),
                "max":if d.on{addrbook::BOOK_MAX}else{0}},
            "self":d.book.selfs().iter().map(|a|a.to_string()).collect::<Vec<_>>(),
            "counters":{"getAddrSent":c.get_addr_sent,"getAddrAnswered":c.get_addr_answered,"getAddrIgnored":c.get_addr_ignored,"getAddrTimeouts":c.get_addr_timeouts,
                "answerSkippedQueue":c.answer_skipped_queue,"addrReceived":c.addr_received,"addrUnsolicited":c.addr_unsolicited,"addrAccepted":c.addr_accepted,
                "addrRefreshed":c.addr_refreshed,"addrDropped":c.addr_dropped,"selfAds":c.self_ads,"selfConnections":c.self_connections}})
    }
}

#[cfg(test)]
mod tests{
    use crate::node::*;
    use super::{Discovery,Source};
    use std::io::Read;
    use std::net::TcpStream;
    use crate::net::Origin;
    use pocol_lp4_execution::devnet::{self,DevnetConfig};
    const T0:u64=1_800_000_000_000;
    /// A devnet node without a network (the relay_tests pattern): events are handed to `handle` directly.
    fn open(tag:&str)->(Node,std::path::PathBuf){open_with(tag,false,Discovery::default())}
    /// `open`, listening (so it sends its nonce; nothing is bound without `start_network`) or not, with these settings.
    fn open_with(tag:&str,listen:bool,d:Discovery)->(Node,std::path::PathBuf){
        let c=DevnetConfig::new(777919,1_700_000_000,&["a","b","c"]);
        let spec=Spec::new(devnet::build(&c).unwrap().admit(&crate::single_host_local()).unwrap().0);
        let dir=std::env::temp_dir().join(format!("pocold-disc-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
        let cfg=Config{dir:dir.clone(),name:tag.into(),miner:None,listen:listen.then(||"127.0.0.1:30300".parse().unwrap()),peers:vec![],rpc:None,stop_at:None,log:false,
            limits:Limits{discovery:d,..Limits::default()}};
        let mut n=Node::open(cfg,spec).unwrap();n.disc.test_now=Some(T0);(n,dir)
    }
    fn sa(s:&str)->SocketAddr{s.parse().unwrap()}
    /// A peer's Hello for this node's network and head (so it starts no sync session), announcing `listen_port` and
    /// `nonce`.
    fn hello(n:&Node,listen_port:u16,nonce:u64)->Msg{let mut h=n.hello();if let Msg::Hello{listen_port:p,nonce:x,..}=&mut h{(*p,*x)=(listen_port,nonce);}h}
    /// A connection of this `origin` whose socket reported `remote`, handed to the node; for a dialed one the address is
    /// in the outbound set first, as `net::dial` puts it there. The other end of the socket pair is returned.
    fn connect(n:&mut Node,id:PeerId,origin:Origin,remote:Option<SocketAddr>)->TcpStream{
        if let Some(a)=origin.dialed(){n.outbound.lock().unwrap().insert(a.to_string());}
        let (p,other)=net::test_peer_as(id,4096,origin,remote);n.handle(Event::Net(NetEvent::Connected(id,p)));other
    }
    fn msg(n:&mut Node,id:PeerId,m:Msg){n.handle(Event::Net(NetEvent::Message(id,m)))}
    /// Peer `id`'s Hello (`hello`) handed to the node.
    fn greet(n:&mut Node,id:PeerId,listen_port:u16,nonce:u64){let h=hello(n,listen_port,nonce);msg(n,id,h)}
    /// The node closed the connection whose other end this is (EOF; the test peer writes nothing).
    fn closed(s:&mut TcpStream)->bool{s.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();let mut b=[0u8;1];
        match s.read(&mut b){Ok(0)=>true,Err(e)=>!matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut),Ok(_)=>false}}
    fn dialing(n:&Node,a:SocketAddr)->bool{n.outbound.lock().unwrap().contains(&a.to_string())}
    /// The Addr answers waiting in a peer's queue.
    fn answers(n:&Node,id:PeerId)->Vec<Vec<SocketAddr>>{n.peers[&id].queued().into_iter().filter_map(|m|match m{Msg::Addr{addrs}=>Some(addrs),_=>None}).collect()}

    /// H2: `net::dial` puts the address it dials into the outbound set before the handshake and dials again once
    /// the address has left it, so the node clears the set by that address (`Origin::Configured`). A socket whose
    /// `peer_addr` failed (a reset right after connect) reports none, and its `addr` is empty: clearing by `addr`
    /// would stall that dialer for good. Both ways a configured connection ends clear the set: its Closed, and an
    /// incompatible Hello, which removes the peer before its Closed arrives. An inbound connection never does.
    #[test] fn the_outbound_set_is_cleared_by_the_dialed_address(){
        let (mut n,dir)=open("h2");
        let a:SocketAddr="81.2.69.9:30300".parse().unwrap();
        let dialing=|n:&Node|n.outbound.lock().unwrap().contains(&a.to_string());
        // As net::dial does: the address is in the set before the connection is handed to the node.
        let connect=|n:&mut Node,id:PeerId,origin:net::Origin,remote:Option<SocketAddr>|{n.outbound.lock().unwrap().insert(a.to_string());
            let (p,other)=net::test_peer_as(id,4096,origin,remote);n.handle(Event::Net(NetEvent::Connected(id,p)));other};
        let _c1=connect(&mut n,1,net::Origin::Configured(a),None);assert_eq!(n.peers[&1].addr,"","the socket reported no address");
        n.handle(Event::Net(NetEvent::Closed(1)));
        assert!(!dialing(&n),"after Closed the dialer dials again");
        let _c2=connect(&mut n,2,net::Origin::Configured(a),None);
        let mut wrong=n.hello();if let Msg::Hello{chain_id,..}=&mut wrong{*chain_id+=1;}
        n.handle(Event::Net(NetEvent::Message(2,wrong)));
        assert!(n.peer_count()==0&&!dialing(&n),"an incompatible Hello closes the connection, and the dialer dials again");
        let _c3=connect(&mut n,3,net::Origin::Inbound,Some(a));n.handle(Event::Net(NetEvent::Closed(3)));
        assert!(dialing(&n),"an inbound connection from that very address leaves the set alone");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// VA-SPEC §1a.2a: an inbound connection's first compatible Hello with listenPort != 0 advertises the connection's
    /// source IP with that port (never the source port, never another IP); later Hellos are Status refreshes whose
    /// listenPort is not read; listenPort 0 advertises nothing. A connection this node dialed advertises nothing: its
    /// Hello verifies the dialed address, whatever port it claims, and with listenPort 0 forgets it (§1a.2b).
    #[test] fn an_inbound_first_hello_advertises_its_source_ip_and_port_once(){
        let (mut n,dir)=open("selfad");
        let entry=|n:&Node,s:&str|n.disc.book.get(sa(s)).cloned();
        let _i=connect(&mut n,1,Origin::Inbound,Some(sa("81.2.69.7:51000")));greet(&mut n,1,30300,0);
        assert!(entry(&n,"81.2.69.7:30300").is_some_and(|e|(e.source,e.direct_ms,e.heard_ms,e.verified_ms)==(Source::Inbound,T0,0,0)),"the source IP with the advertised port, self-advertised");
        assert_eq!((n.disc.book.entries().len(),n.disc.counters.self_ads,n.disc.links[&1].advertised),(1,1,Some(sa("81.2.69.7:30300"))),"and nothing else: not the source port");
        n.disc.test_now=Some(T0+5);greet(&mut n,1,40000,0);
        assert!(n.disc.book.entries().len()==1&&entry(&n,"81.2.69.7:30300").unwrap().direct_ms==T0,"a later Hello's port is not read, nor does it refresh the advertisement");
        let _j=connect(&mut n,2,Origin::Inbound,Some(sa("81.2.70.8:51001")));greet(&mut n,2,0,0);
        assert_eq!((n.disc.book.entries().len(),n.disc.counters.self_ads),(1,1),"listenPort 0 advertises nothing");
        let d=sa("81.3.0.9:30300");let _o=connect(&mut n,3,Origin::Configured(d),Some(d));greet(&mut n,3,40000,0);
        assert!(entry(&n,"81.3.0.9:30300").is_some_and(|e|e.verified_ms==T0+5),"the dialed address is verified");
        assert!(entry(&n,"81.3.0.9:40000").is_none()&&n.disc.book.entries().len()==2&&n.disc.counters.self_ads==1,"the port a dialed peer claims is not stored");
        n.handle(Event::Net(NetEvent::Closed(3)));let _o=connect(&mut n,4,Origin::Configured(d),Some(d));greet(&mut n,4,0,0);
        assert!(entry(&n,"81.3.0.9:30300").is_none(),"a dialed peer that announces no listener withdrew its consent: forgotten");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// VA-SPEC §1a.5: an inbound connection's first GetAddr is answered, with an Addr of what the book relays (none from
    /// an empty book), never the requester's own advertised endpoint; its second GetAddr, and a GetAddr on a connection
    /// this node dialed, are ignored. The answer goes through `Peer::offer`: past half the requester's queue it is not
    /// queued, although gossip would still fit, and the peer is kept.
    #[test] fn getaddr_is_answered_once_per_inbound_connection_through_offer(){
        let (mut n,dir)=open("answer");
        let _i=connect(&mut n,1,Origin::Inbound,Some(sa("81.2.69.7:51000")));greet(&mut n,1,0,0);
        msg(&mut n,1,Msg::GetAddr);assert_eq!(answers(&n,1),[vec![]],"an empty book answers with no record");
        msg(&mut n,1,Msg::GetAddr);assert_eq!(answers(&n,1).len(),1,"a second GetAddr is not answered");
        assert_eq!((n.disc.counters.get_addr_answered,n.disc.counters.get_addr_ignored),(1,1));
        // Once the answer list is rebuilt (it is at most cache_ms old), a self-advertised entry is answered to others.
        let _j=connect(&mut n,2,Origin::Inbound,Some(sa("81.5.0.1:51000")));greet(&mut n,2,30300,0);
        n.disc.test_now=Some(T0+60_000);
        let _k=connect(&mut n,3,Origin::Inbound,Some(sa("81.6.0.1:51000")));greet(&mut n,3,30300,0);
        msg(&mut n,3,Msg::GetAddr);assert_eq!(answers(&n,3),[vec![sa("81.5.0.1:30300")]],"the other node's endpoint, not the requester's own");
        msg(&mut n,2,Msg::GetAddr);assert_eq!(answers(&n,2),[vec![sa("81.6.0.1:30300")]],"and the other way round: never the requester's own endpoint");
        let c=sa("81.7.0.1:30300");let _o=connect(&mut n,4,Origin::Configured(c),Some(c));greet(&mut n,4,30300,0);
        msg(&mut n,4,Msg::GetAddr);assert!(answers(&n,4).is_empty()&&n.disc.counters.get_addr_ignored==2,"never on a connection this node dialed");
        // Past half the queue (4096 B), below the whole of it.
        let _q=connect(&mut n,5,Origin::Inbound,Some(sa("81.8.0.1:51000")));greet(&mut n,5,0,0);
        let p=n.peers[&5].clone();let filler=Msg::Tx{raw:vec![7u8;400]};while p.queued_bytes()<=2048{assert!(p.send(&filler));}
        let q=p.queued_bytes();msg(&mut n,5,Msg::GetAddr);
        assert!(p.queued_bytes()==q&&answers(&n,5).is_empty(),"not queued past half the queue");
        assert_eq!((n.disc.counters.answer_skipped_queue,n.disc.counters.get_addr_answered),(1,3));
        assert!(n.peers.contains_key(&5)&&p.send(&Msg::Tx{raw:vec![1u8;8]}),"the peer is kept");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// VA-SPEC §1a.2c: a connection this node dialed is asked for addresses right after its first compatible Hello, and
    /// its one Addr within getaddr_timeout_ms (120 s) is heard from it: new records are added, known ones refreshed, a
    /// repeat within the answer dropped. A second Addr, an Addr on an inbound connection and one past the timeout are
    /// ignored; the tick expires an unanswered GetAddr.
    #[test] fn addr_is_accepted_only_as_the_answer_to_our_getaddr(){
        let (mut n,dir)=open("addr");let c=sa("81.2.69.9:30300");
        let _k=connect(&mut n,9,Origin::Inbound,Some(sa("81.4.0.1:51000")));greet(&mut n,9,30300,0);
        let _c=connect(&mut n,1,Origin::Configured(c),Some(c));greet(&mut n,1,30300,0);
        assert!(matches!(n.peers[&1].queued().as_slice(),[Msg::Hello{..},Msg::GetAddr]),"asked right after its first Hello");
        let three=[sa("81.5.0.1:30300"),sa("81.6.0.1:30300"),sa("[2606:4700::1]:30300")];
        msg(&mut n,1,Msg::Addr{addrs:vec![three[0],three[1],three[0],sa("81.4.0.1:30300"),three[2]]});
        let k=&n.disc.counters;assert_eq!((k.addr_received,k.addr_accepted,k.addr_refreshed,k.addr_dropped),(1,3,1,1));
        assert!(three.iter().all(|a|n.disc.book.get(*a).is_some_and(|e|(e.source,e.heard_ms,e.verified_ms)==(Source::Cfg(c),T0,0))),"heard from the configured peer");
        assert_eq!(n.disc.book.get(sa("81.4.0.1:30300")).map(|e|(e.source,e.heard_ms,e.direct_ms)),Some((Source::Inbound,T0,T0)),"a known entry is refreshed, its source kept");
        let ignored=|n:&mut Node,id:PeerId,a:&str|{msg(n,id,Msg::Addr{addrs:vec![sa(a)]});n.disc.book.get(sa(a)).is_none()};
        assert!(ignored(&mut n,1,"81.7.0.1:30300"),"a second Addr on the connection");
        assert!(ignored(&mut n,9,"81.8.0.1:30300"),"an Addr on an inbound connection");
        let (e,f)=(sa("81.10.0.1:30300"),sa("81.12.0.1:30300"));
        let _e=connect(&mut n,2,Origin::Configured(e),Some(e));greet(&mut n,2,30300,0);
        let _f=connect(&mut n,3,Origin::Configured(f),Some(f));greet(&mut n,3,30300,0);
        n.disc.test_now=Some(T0+120_000);assert!(!ignored(&mut n,2,"81.11.0.1:30300"),"an answer 120 s after the GetAddr is taken");
        n.disc.test_now=Some(T0+120_001);assert!(ignored(&mut n,3,"81.13.0.1:30300"),"one 120 s and 1 ms after it is not");
        assert_eq!((n.disc.counters.addr_unsolicited,n.disc.counters.addr_received,n.disc.counters.get_addr_timeouts),(3,2,0));
        n.disc_tick(T0+120_000);assert_eq!(n.disc.counters.get_addr_timeouts,0,"not expired at 120 s");
        n.disc_tick(T0+120_501);assert_eq!(n.disc.counters.get_addr_timeouts,1,"the unanswered GetAddr expires once");
        n.disc_tick(T0+121_001);assert_eq!(n.disc.counters.get_addr_timeouts,1);
        // A peer ahead of this node: its GetAddr goes before the sync session's GetFrom, so that the answer is not
        // queued behind blocks (an answer past half the queue is not sent).
        let g=sa("81.14.0.1:30300");let _g=connect(&mut n,4,Origin::Configured(g),Some(g));
        let mut ahead=hello(&n,30300,0);if let Msg::Hello{head,height,work,..}=&mut ahead{(*head,*height,*work)=([9;32],5,[0xff;32]);}msg(&mut n,4,ahead);
        assert!(matches!(n.peers[&4].queued().as_slice(),[Msg::Hello{..},Msg::GetAddr,Msg::GetFrom{..}]),"{:?}",n.peers[&4].queued());
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// VA-SPEC §2, H1: a listening node sends its nonce, and a first Hello carrying it is a self-connection. The
    /// configured connection is closed and removed before any Status is kept, its address is this node's own and
    /// stays in the outbound set, also after its Closed, so the dialer stops. The inbound end marks nothing and is
    /// finished, not closed at once, so that this node's Hello still reaches the dialing end. Another nonzero nonce is
    /// another node; a node that does not listen sends 0 and never matches.
    #[test] fn a_hello_with_our_nonce_is_a_self_connection_and_stops_its_dialer(){
        let (mut n,dir)=open_with("self",true,Discovery::default());
        let ours=n.disc.wire_nonce();assert!(ours!=0&&matches!(n.hello(),Msg::Hello{nonce,..} if nonce==ours),"a listening node sends its nonce");
        let x=sa("81.2.69.9:30300");let mut cx=connect(&mut n,1,Origin::Configured(x),Some(x));let px=n.peers[&1].clone();greet(&mut n,1,30300,ours);
        assert!(!n.peers.contains_key(&1)&&px.ended()==(true,false)&&closed(&mut cx),"closed at once and removed");
        assert!(n.disc.book.selfs()==[x]&&n.disc.book.get(x).is_none(),"its address is this node's own, never an entry");
        n.handle(Event::Net(NetEvent::Closed(1)));assert!(dialing(&n,x),"the address stays in the outbound set: its dialer stops");
        let st=n.status_json();
        assert_eq!((st["compatiblePeers"].as_u64(),st["discovery"]["self"].clone(),st["discovery"]["counters"]["selfConnections"].as_u64(),st["discovery"]["selfDetection"].as_bool()),
            (Some(0),serde_json::json!(["81.2.69.9:30300"]),Some(1),Some(true)));
        let _ci=connect(&mut n,2,Origin::Inbound,Some(sa("81.2.69.9:51000")));let pi=n.peers[&2].clone();greet(&mut n,2,30300,ours);
        assert!(!n.peers.contains_key(&2)&&pi.ended()==(false,true),"the inbound end is removed, and finished: its writer closes it once our Hello is out");
        assert!(n.disc.book.selfs()==[x]&&n.disc.book.entries().is_empty()&&n.disc.counters.self_connections==2,"and marks nothing, nor advertises");
        let y=sa("81.3.0.1:30300");let _y=connect(&mut n,3,Origin::Configured(y),Some(y));greet(&mut n,3,30300,ours^1);
        assert!(n.peer_status.contains_key(&3)&&n.disc.book.get(y).is_some_and(|e|e.verified_ms==T0),"another nonce is another node");
        let (mut q,qdir)=open("self-quiet");
        assert!(q.disc.wire_nonce()==0&&q.disc.nonce!=0&&matches!(q.hello(),Msg::Hello{nonce:0,..}),"a node that does not listen sends 0");
        for (id,nonce) in [(1,0),(2,q.disc.nonce)]{let z=SocketAddr::new([81,3,0,id as u8].into(),30300);let _z=connect(&mut q,id,Origin::Configured(z),Some(z));
            greet(&mut q,id,30300,nonce);assert!(q.peer_status.contains_key(&id),"nonce {nonce:#x} is another node's");}
        assert_eq!(q.disc.counters.self_connections,0);
        drop((n,q));let _=std::fs::remove_dir_all(&dir);let _=std::fs::remove_dir_all(&qdir);
    }
    /// Judge 1's adversarial case: a peer this node dialed that replays this node's nonce (it saw it in a Hello) closes
    /// its own link only: only the address dialed for it is marked, and the honest connection stays, with its Status.
    #[test] fn a_replayed_nonce_closes_only_the_replayers_own_link(){
        let (mut n,dir)=open_with("replay",true,Discovery::default());let ours=n.disc.wire_nonce();
        let (h,p)=(sa("81.2.69.9:30300"),sa("81.3.0.1:30300"));
        let _h=connect(&mut n,1,Origin::Configured(h),Some(h));greet(&mut n,1,30300,0x1111);
        let mut cp=connect(&mut n,2,Origin::Configured(p),Some(p));greet(&mut n,2,30300,ours);
        let _i=connect(&mut n,3,Origin::Inbound,Some(sa("81.4.0.1:51000")));let pi=n.peers[&3].clone();greet(&mut n,3,30300,ours);
        assert_eq!(n.disc.book.selfs(),[p],"only the replayer's own dialed address");
        assert!(closed(&mut cp)&&!n.peers.contains_key(&2)&&!n.peers.contains_key(&3)&&pi.ended()==(false,true),"each replayer's own link ends");
        assert!(n.peers.contains_key(&1)&&n.peer_status.contains_key(&1)&&n.disc.book.get(h).is_some_and(|e|e.verified_ms==T0)&&dialing(&n,h),"the honest link stays");
        greet(&mut n,1,30300,ours);
        assert!(n.peer_status.contains_key(&1)&&n.disc.book.selfs()==[p],"a later Hello is a Status refresh: its nonce is not read");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// VA-SPEC §1a.4: a connection this node dialed that turns out to run another release (compatibility digest or
    /// protocol version) holds its entry back, min(10 min · 2^(n-1), 6 h); one on another chain or genesis removes it.
    /// An inbound connection's mismatch changes nothing.
    #[test] fn incompatible_and_wrong_network_hellos_update_the_book(){
        let (mut n,dir)=open("mismatch");let a=sa("81.2.69.9:30300");let mut id=0;
        let mut dial=|n:&mut Node,f:&dyn Fn(&mut Msg)|{id+=1;let _c=connect(n,id,Origin::Configured(a),Some(a));let mut h=hello(n,30300,0);f(&mut h);msg(n,id,h);
            n.handle(Event::Net(NetEvent::Closed(id)));};
        let entry=|n:&Node|n.disc.book.get(a).map(|e|(e.verified_ms,e.holds,e.hold_until_ms));
        dial(&mut n,&|_|{});assert_eq!(entry(&n),Some((T0,0,0)),"verified");
        dial(&mut n,&|h|if let Msg::Hello{compatibility,..}=h{compatibility[0]^=1});assert_eq!(entry(&n),Some((T0,1,T0+600_000)),"another compatibility digest: held");
        dial(&mut n,&|h|if let Msg::Hello{protocol,..}=h{*protocol+=1});assert_eq!(entry(&n),Some((T0,2,T0+1_200_000)),"another protocol version: held twice as long");
        let _i=connect(&mut n,50,Origin::Inbound,Some(a));let mut g=hello(&n,30300,0);if let Msg::Hello{genesis,..}=&mut g{genesis[0]^=1}msg(&mut n,50,g);
        assert_eq!(entry(&n),Some((T0,2,T0+1_200_000)),"an inbound mismatch changes nothing, even from that very address");
        dial(&mut n,&|h|if let Msg::Hello{genesis,..}=h{genesis[0]^=1});assert_eq!(entry(&n),None,"another genesis: forgotten");
        dial(&mut n,&|_|{});assert_eq!(entry(&n),Some((T0,0,0)),"verified again, with no hold");
        dial(&mut n,&|h|if let Msg::Hello{chain_id,..}=h{*chain_id+=1});assert_eq!(entry(&n),None,"another chain: forgotten");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// Learned addresses never enter `Net::protect`, which exempts only the configured peers from the inbound group cap
    /// (net::Limits::per_group_max): an address learned from a self-advertisement or from hearsay is grouped as before.
    #[test] fn learned_addresses_are_never_protected_from_the_group_cap(){
        let (mut n,dir)=open("protect");let ip:std::net::IpAddr="81.2.69.7".parse().unwrap();
        let _i=connect(&mut n,1,Origin::Inbound,Some(sa("81.2.69.7:51000")));greet(&mut n,1,30300,0);
        let c=sa("81.3.0.9:30300");let _c=connect(&mut n,2,Origin::Configured(c),Some(c));greet(&mut n,2,30300,0);
        msg(&mut n,2,Msg::Addr{addrs:vec![sa("81.2.69.7:30301")]});
        assert!(n.disc.book.get(sa("81.2.69.7:30300")).is_some()&&n.disc.book.get(sa("81.2.69.7:30301")).is_some(),"learned both ways");
        assert_eq!(n.net.inbound_group(ip),Some(net::Group::V4([81,2])),"still counted in its group");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// Local mode (`--discover-local`, VA-SPEC §1a.3): an address without an address group is learned only from a source
    /// without one (an inbound connection from it, a dialed local peer, a local peer's hearsay) and answered only to a
    /// requester without one; a public peer's hearsay of a local address is dropped.
    #[test] fn local_mode_learns_and_answers_local_addresses_only_among_local_peers(){
        let (mut n,dir)=open_with("local",false,Discovery{allow_local:true,..Discovery::default()});
        let _i=connect(&mut n,1,Origin::Inbound,Some(sa("127.0.0.1:51000")));greet(&mut n,1,40001,0);
        let r=sa("127.0.0.1:40002");let _r=connect(&mut n,2,Origin::Configured(r),Some(r));greet(&mut n,2,40002,0);
        msg(&mut n,2,Msg::Addr{addrs:vec![sa("127.0.0.1:40003"),sa("10.0.0.1:30300")]});
        let p=sa("81.2.69.9:30300");let _p=connect(&mut n,3,Origin::Configured(p),Some(p));greet(&mut n,3,30300,0);
        msg(&mut n,3,Msg::Addr{addrs:vec![sa("127.0.0.1:40004"),sa("81.5.0.1:30300")]});
        let mut held:Vec<SocketAddr>=n.disc.book.entries().iter().map(|e|e.addr).collect();held.sort();
        let mut want=["127.0.0.1:40001","127.0.0.1:40002","127.0.0.1:40003","10.0.0.1:30300","81.2.69.9:30300","81.5.0.1:30300"].map(sa);want.sort();
        assert_eq!(held,want,"every local address from a local source; the public peer's local hearsay (:40004) dropped");
        let ask=|n:&mut Node,id:PeerId,remote:&str|{let _q=connect(n,id,Origin::Inbound,Some(sa(remote)));greet(n,id,0,0);msg(n,id,Msg::GetAddr);let mut a=answers(n,id).concat();a.sort();a};
        let mut local=["127.0.0.1:40001","127.0.0.1:40002","81.2.69.9:30300"].map(sa);local.sort();
        assert_eq!(ask(&mut n,4,"127.0.0.1:51001"),local,"a local requester: the advertised and verified entries, local ones included");
        assert_eq!(ask(&mut n,5,"81.6.0.1:51000"),[p],"a public requester: no local entry");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// Live connections keep their entries within the relay window (VA-SPEC §1a.5: 7 days from a verification or an
    /// advertisement): every refresh_ms (1 h) a consenting connection this node dialed refreshes its verification and
    /// an inbound one its advertisement, and a dialed one also when it ends. Links end with their connections.
    #[test] fn live_connections_refresh_their_entries_every_hour_and_when_they_end(){
        const H:u64=3_600_000;
        let (mut n,dir)=open("refresh");let (c,i)=(sa("81.2.69.9:30300"),sa("81.5.0.1:30300"));
        let _c=connect(&mut n,1,Origin::Configured(c),Some(c));greet(&mut n,1,30300,0);
        let _i=connect(&mut n,2,Origin::Inbound,Some(sa("81.5.0.1:51000")));greet(&mut n,2,30300,0);
        let times=|n:&Node|(n.disc.book.get(c).unwrap().verified_ms,n.disc.book.get(i).unwrap().direct_ms);
        n.disc_tick(T0);n.disc_tick(T0+H-500);assert_eq!(times(&n),(T0,T0),"not within the hour");
        n.disc_tick(T0+H);assert_eq!(times(&n),(T0+H,T0+H),"refreshed once the hour is up");
        n.disc.test_now=Some(T0+H+5);n.handle(Event::Net(NetEvent::Closed(1)));n.handle(Event::Net(NetEvent::Closed(2)));
        assert_eq!(times(&n),(T0+H+5,T0+H),"the end of a dialed connection refreshes its verification");
        assert!(n.disc.links.is_empty(),"no link outlives its connection");
        n.disc_tick(T0+2*H);assert_eq!(times(&n),(T0+H+5,T0+H),"an ended connection refreshes nothing");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// `--no-discovery` (VA-SPEC §1a, the escape hatch): Hello announces no port, no GetAddr is sent, a GetAddr is not
    /// answered, an Addr not taken, a self-advertisement not stored and no link state is kept; the nonce check still
    /// stops a dialer that reached this node itself.
    #[test] fn discovery_off_sends_and_stores_nothing(){
        let (mut n,dir)=open_with("off",true,Discovery{enabled:false,..Discovery::default()});
        n.disc.listen_port=30300;
        let ours=n.disc.wire_nonce();assert!(ours!=0&&matches!(n.hello(),Msg::Hello{listen_port:0,nonce,..} if nonce==ours),"listenPort 0; the nonce is still sent");
        let c=sa("81.2.69.9:30300");let _c=connect(&mut n,1,Origin::Configured(c),Some(c));greet(&mut n,1,30300,0);
        assert!(matches!(n.peers[&1].queued().as_slice(),[Msg::Hello{..}])&&n.peer_status.contains_key(&1),"no GetAddr");
        msg(&mut n,1,Msg::Addr{addrs:vec![sa("81.5.0.1:30300")]});
        let _i=connect(&mut n,2,Origin::Inbound,Some(sa("81.3.0.1:51000")));greet(&mut n,2,30300,0);msg(&mut n,2,Msg::GetAddr);
        assert!(answers(&n,2).is_empty()&&n.disc.book.entries().is_empty()&&n.disc.links.is_empty(),"nothing answered, stored or kept");
        let k=&n.disc.counters;assert_eq!((k.get_addr_sent,k.get_addr_ignored,k.addr_unsolicited,k.self_ads),(0,1,1,0));
        let x=sa("81.4.0.1:30300");let mut cx=connect(&mut n,3,Origin::Configured(x),Some(x));greet(&mut n,3,30300,ours);
        n.handle(Event::Net(NetEvent::Closed(3)));
        assert!(closed(&mut cx)&&!n.peers.contains_key(&3)&&dialing(&n,x)&&n.disc.counters.self_connections==1,"a self-connection still stops its dialer");
        let st=n.status_json();assert_eq!((st["discovery"]["enabled"].as_bool(),st["discovery"]["listenPort"].as_u64(),st["discovery"]["book"]["max"].as_u64()),(Some(false),Some(0),Some(0)));
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
}
