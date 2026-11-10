//! Experimental wire v2 peer transport: TCP on the explicitly configured listener, frames
//! [u32 BE length][RLP([type, ...fields])], length <= MAX_FRAME. Malformed frames
//! or a wrong network identity close the connection; they never touch block state.
//!
//! Resource bounds (A05, local and not consensus; `Limits`):
//! - at most `peers_max` (default 32) established inbound connections (their Hello accepted, `Net::establish`) and, counted
//!   apart, at most `handshaking_max` still waiting for their Hello, so at most `inbound_max` (their sum) inbound
//!   connections exist at once; excess connections are closed on accept, before any thread is spawned. Outbound
//!   connections are separate (one per configured peer, `dial`; each carries the address it was dialed at, its
//!   `Origin`), so inbound connections can never take the slots the node needs to reach its own peers;
//! - at most `per_group_max` (default 4) inbound connections per public address group (`group_of`: IPv4 /16, IPv6
//!   /32, a tunnelled IPv4 address as that address), plus one spare place (`GROUP_SPARE`): a newcomer that takes it
//!   must evict the youngest unprotected member of its own group at its first eligible Hello (`victim_in`), or it is
//!   closed, so one idle host cannot lock the rest of its /16 out for good. Past the spare place a connection is
//!   refused even while places are free: nodes behind one NAT or one provider range share these places. Groups are counted per address, so one host
//!   that reaches the node from several groups (an IPv4 /16 and an IPv6 /32, say) holds a full set of places in
//!   each. Addresses that are not globally reachable (loopback, private, link-local, ...) and the node's configured
//!   peers, by their exact address (`Net::protect`), are not grouped;
//! - inbound eviction (local policy, as Bitcoin Core's): while `peers_max` connections are established, the first
//!   Hello for this network and build (`Net::eligible`) from a newcomer evicts one established inbound peer, or the
//!   newcomer is closed. Protected are a keyed netgroup lottery, the latest deliverers of new blocks, templates,
//!   transactions and shares (each kind its own quota), the longest connected, peers within their grace period,
//!   and as many connections from a configured peer's address as it has configured peers; the victim is the
//!   youngest of the rest in the eviction key (`EvictKey`) with the most established connections (`victim`), so
//!   one host, one /16 or one flood cannot hold the places. Evictions are rate-limited per newcomer key and in
//!   all (`EvictLimits`). A victim keeps its places until its reader releases them;
//! - the first frame must be a Hello and must arrive within `hello_deadline`; a frame
//!   whose length has arrived must complete within `frame_deadline`; an established peer
//!   silent for `idle_deadline` is closed;
//! - every peer has a writer thread and an outbound queue of at most `queue_bytes_max`;
//!   the consensus thread only enqueues. Gossip to a full queue drops that peer (a peer
//!   that stops reading never blocks consensus); range responses stop at half the queue;
//! - inbound frames wait in a bounded event queue (backpressure stops the reader), so
//!   frame memory is at most (inbound_max + NET_EVENT_QUEUE) · MAX_FRAME.
use std::collections::{HashMap,VecDeque};
use std::cmp::Reverse;
use std::hash::BuildHasher;
use std::io::{Read,Write};
use std::net::{IpAddr,TcpListener,TcpStream,SocketAddr};
use std::sync::{Arc,Condvar,Mutex,MutexGuard,OnceLock,atomic::{AtomicBool,AtomicU64,AtomicUsize,Ordering}};
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
    /// Established inbound connections (their Hello accepted, `Net::establish`); past it a newcomer evicts or is closed.
    /// Default 32; `pocold run --peers-max N`, or POCOL_PEERS_MAX (1..=PEERS_MAX_LIMIT).
    pub peers_max:usize,
    /// Inbound connections still waiting for their Hello, counted apart from `peers_max` (`inbound_max`).
    pub handshaking_max:usize,
    /// Inbound connections per public address group (`group_of`, `Net::admit`), plus `GROUP_SPARE` that must evict
    /// within the group; 0 disables grouping. `pocold run --per-group-max N`, or POCOL_PER_GROUP_MAX.
    pub per_group_max:usize,
    pub hello_deadline:Duration,pub frame_deadline:Duration,pub idle_deadline:Duration,
    pub queue_bytes_max:usize,pub write_deadline:Duration,
    /// Inbound eviction's grace and budget (`Net::establish`).
    pub evict:EvictLimits,
}
impl Default for Limits{
    // PR3: 32 established inbound, 8 waiting for a Hello (A13 3.42 MB per place); a public /16 or /32 holds at most 4.
    // (D-A13-BUDGET had cut peers_max 32 -> 8 while every place, a handshake's included, counted in it; network.md:
    // <= 50 peers by default: 32 + 8 inbound and at most 7 outbound.)
    fn default()->Self{Self{peers_max:32,handshaking_max:8,per_group_max:4,hello_deadline:Duration::from_secs(5),frame_deadline:Duration::from_secs(30),
        idle_deadline:Duration::from_secs(300),queue_bytes_max:2*1024*1024,write_deadline:Duration::from_secs(10),evict:EvictLimits::default()}}
}
impl Limits{
    /// The physical inbound bound: the established places and the places waiting for a Hello (`Net::admit`, `listen`).
    /// A13 counts every one of them.
    pub fn inbound_max(&self)->usize{self.peers_max.saturating_add(self.handshaking_max)}
}
/// The largest `peers_max` an operator may set (`--peers-max`, POCOL_PEERS_MAX): A13 counts every place at about 3.73
/// MiB of budget, and the thread inventory (`resources::tasks_needed`: 2·inbound_max + 4·dials + 40 tasks with 32 RPC
/// connections) is 340 at 128 with 7 peers, within the 512 tasks a seed's unit allows (`Node::start_network` refuses to
/// boot past the service's pids.max, `resources::tasks_fit`).
pub const PEERS_MAX_LIMIT:usize=128;
/// `--peers-max N` / POCOL_PEERS_MAX: a decimal integer in 1..=PEERS_MAX_LIMIT, surrounding white space ignored. 0 is
/// refused: a node that listens with no established place would evict at every Hello or refuse every peer.
pub fn parse_peers_max(s:&str)->Result<usize,String>{
    let n:usize=s.trim().parse().map_err(|e|format!("{e}"))?;
    if !(1..=PEERS_MAX_LIMIT).contains(&n){return Err(format!("must be 1..={PEERS_MAX_LIMIT}"))}Ok(n)
}
/// Inbound eviction's grace and budget (local policy, `Net::establish`, VA-SPEC §1b):
/// - `grace`: a peer established less than this ago is never a victim, so a flood of newcomers cannot evict the
///   honest peers that joined just before it;
/// - `key_interval`: one newcomer eviction key (`EvictKey`) evicts at most once per interval;
/// - `burst`, `refill`: all newcomers together evict at most `burst` peers at once, then one per `refill` (GCRA).
///
/// A configured (exempt) newcomer is not charged. A newcomer the budget refuses is closed, as it was before eviction
/// existed, so a spent budget fails safe.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct EvictLimits{pub grace:Duration,pub key_interval:Duration,pub burst:u32,pub refill:Duration}
impl Default for EvictLimits{fn default()->Self{Self{grace:Duration::from_secs(30),key_interval:Duration::from_secs(30),burst:8,refill:Duration::from_secs(2)}}}
/// An address group: a public peer's IPv4 /16 or IPv6 /32 (`group_of`), for the inbound cap and the address book's
/// keys (`addrbook::Key`, ordered for a deterministic iteration).
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash,PartialOrd,Ord)]
pub enum Group{V4([u8;2]),V6([u8;4])}
/// The address group of a public peer: its IPv4 /16 or IPv6 /32, Bitcoin Core's netgroup granularity
/// (Bitcoin Core uses netgroups to choose whom to evict and to spread its outbound connections; this node caps a
/// group's inbound places, `Net::admit`, and evicts from the eviction key with the most established places, `EvictKey`,
/// `victim`).
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
    /// group was full (also counted in `refused`): at accept past the spare place, or at the Hello of the one that took
    /// it when no member of its group could be evicted (`Net::establish`).
    pub groups:Mutex<HashMap<Group,usize>>,pub refused_group:AtomicUsize,
    /// Inbound eviction (`Net::establish`): peers evicted for a newcomer (`evicted_in_group`: within the newcomer's own
    /// full group), and newcomers closed at their Hello because no peer could be evicted (`refused_full`) or the
    /// eviction budget was spent (`refused_rate`), both also counted in `refused`.
    pub evicted:AtomicUsize,pub evicted_in_group:AtomicUsize,pub refused_full:AtomicUsize,pub refused_rate:AtomicUsize}
/// Places a full address group may take beyond `per_group_max` (`Net::admit`). A connection in one is `Admit::over`:
/// at its first eligible Hello it evicts within its own group or is closed (`Net::establish`), so the group's
/// established members stay within the cap. One, so a group holds at most per_group_max+1 places.
pub const GROUP_SPARE:usize=1;
/// An admitted inbound connection (`Net::admit`). `over`: it took its group's spare place (`GROUP_SPARE`).
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
struct Admit{over:bool}
/// What an eligible Hello is checked against (`Net::expect_hello`, `Net::eligible`): this node's consensus identity
/// (chain, genesis and `Consensus.id`, VA-SPEC §2.1), which node.rs checks every Hello against through the same
/// `identity::accepts`, and its own self-connection nonce.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct HelloId{pub consensus:crate::identity::Consensus,pub nonce:u64}
/// The key inbound eviction groups connections by (`victim`), from the canonical address:
/// - `Net`: its address group (`group_of`, also with per_group_max 0);
/// - `Ula`: all of fc00::/7, where cjdns and ULA overlays mint addresses cheaply;
/// - `V6Local`: another IPv6 address that is not grouped (loopback, link-local, ...), by its /32;
/// - `Host`: an IPv4 address that is not grouped (LAN, loopback), by itself; :: when the socket reported no address.
///
/// Only eviction uses it; the per-group cap stays on `Group`.
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash,PartialOrd,Ord)]
pub enum EvictKey{Net(Group),Host(IpAddr),Ula,V6Local([u8;4])}
/// `EvictKey` of a peer at `ip` under `grouper` (`group_of` in the node).
pub fn evict_key(grouper:fn(IpAddr)->Option<Group>,ip:Option<IpAddr>)->EvictKey{
    let Some(ip)=ip.map(|ip|ip.to_canonical()) else{return EvictKey::Host(IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED))};
    if let Some(g)=grouper(ip){return EvictKey::Net(g)}
    match ip{IpAddr::V6(a)=>{let o=a.octets();if (o[0]&0xfe)==0xfc{EvictKey::Ula}else{EvictKey::V6Local([o[0],o[1],o[2],o[3]])}}IpAddr::V4(_)=>EvictKey::Host(ip)}
}
/// A13 ("queues/inbound eviction"): the bytes counted per registry slot (`Slot`), per selection candidate (`Cand`),
/// per connection's activity record behind its Arc (`Activity`, with the 16-byte Arc header), and the eviction ring's
/// entries and bytes per entry. Fixed, so the pinned total does not follow rustc's layout; the types are asserted to
/// fit. EVICT_RING exceeds the evictions one key_interval allows at the defaults (burst 8, then one per 2 s for 30 s: 23),
/// so the ring still holds every key that evicted within the interval.
pub const SLOT_BYTES:usize=96;pub const CAND_BYTES:usize=96;pub const ACTIVITY_BYTES:usize=64;pub const EVICT_RING:usize=64;pub const EVICT_ENTRY_BYTES:usize=32;
const _:()=assert!(std::mem::size_of::<Slot>()<=SLOT_BYTES&&std::mem::size_of::<Cand>()<=CAND_BYTES&&std::mem::size_of::<Activity>()+16<=ACTIVITY_BYTES
    &&std::mem::size_of::<(EvictKey,u64)>()<=EVICT_ENTRY_BYTES,"an eviction type exceeds its A13 bytes");
/// What a connection delivered first (`Peer::credit`): a new block, a template that locked, a transaction that
/// entered the pool, a valid new share. Each kind has its own protection quota (`victim`).
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Useful{Block,Template,Tx,Share}
/// Per connection, the time (`mono_ms`) of its last credited delivery of each `Useful` kind; 0 = never.
#[derive(Debug,Default)]
pub struct Activity{last:[AtomicU64;4]}
impl Activity{fn get(&self)->[u64;4]{std::array::from_fn(|k|self.last[k].load(Ordering::SeqCst))}}
/// Milliseconds since the first call in this process, plus 1, so that 0 can mean "never". Monotonic.
pub fn mono_ms()->u64{static EPOCH:OnceLock<Instant>=OnceLock::new();u64::try_from(EPOCH.get_or_init(Instant::now).elapsed().as_millis()).unwrap_or(u64::MAX).saturating_add(1)}
fn ms(d:Duration)->u64{u64::try_from(d.as_millis()).unwrap_or(u64::MAX)}
/// One admitted inbound connection (`Net::enlist`, from its Connected until its release, `Net::forget`). `hello`: its
/// first Hello was eligible and established (`Net::establish`); `exempt`: it is one of the connections a configured
/// peer's address may hold (`Net::protect`); `at`: when it was established (`mono_ms`); `group`: the address group it
/// holds a place in (`Net::admit`); `over`: that place is its group's spare one, until it is established.
struct Slot{id:PeerId,key:EvictKey,ip:Option<IpAddr>,group:Option<Group>,hello:bool,exempt:bool,over:bool,at:u64,act:Arc<Activity>,out:Arc<Outbox>,sock:Arc<TcpStream>}
/// An eviction candidate (`victim`): an open, established, non-exempt slot other than the newcomer's. `rank`: its key's
/// keyed hash (the lottery); `last`: its `Activity`; `free`: past its grace period; `prot`: protected by a step so far.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
struct Cand{id:PeerId,key:EvictKey,rank:u64,last:[u64;4],free:bool,prot:bool}
/// The inbound registry: the admitted inbound connections, the selection scratch (both reserved for `inbound_max`
/// entries in `Net::new` and never grown: slots.len() <= live <= inbound_max), and the eviction budget's state, a ring
/// of the latest charges (key, time; time 0 = empty) and the GCRA's theoretical arrival time `tat`.
struct Registry{slots:Vec<Slot>,scratch:Vec<Cand>,ring:[(EvictKey,u64);EVICT_RING],ring_next:usize,tat:u64}
impl std::fmt::Debug for Registry{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{write!(f,"Registry({} slots)",self.slots.len())}}
impl Registry{
    fn new(cap:usize)->Self{Self{slots:Vec::with_capacity(cap),scratch:Vec::with_capacity(cap),ring:[(EvictKey::Ula,0);EVICT_RING],ring_next:0,tat:0}}
    /// Whether a newcomer of `key` may evict at `now`: its key did not evict within `key_interval`, and the GCRA allows
    /// it (`tat <= now + (burst-1)·refill`). A time after `now` counts as within the interval (saturating).
    fn allows(&self,key:EvictKey,now:u64,e:&EvictLimits)->bool{
        if self.ring.iter().any(|(k,t)|*t!=0&&*k==key&&now.saturating_sub(*t)<ms(e.key_interval)){return false}
        self.tat<=now.saturating_add(u64::from(e.burst.saturating_sub(1)).saturating_mul(ms(e.refill)))
    }
    /// Records an eviction by a newcomer of `key` at `now`.
    fn charge(&mut self,key:EvictKey,now:u64,e:&EvictLimits){self.ring[self.ring_next]=(key,now);self.ring_next=(self.ring_next+1)%EVICT_RING;self.tat=self.tat.max(now).saturating_add(ms(e.refill));}
}
fn open(s:&Slot)->bool{s.hello&&!s.out.closed.load(Ordering::SeqCst)}
/// `victim`'s protection quotas: the lottery's distinct keys, and per `Useful` kind the latest deliverers.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
struct Policy{protect_groups:usize,protect_recent:usize}
impl Policy{fn of(l:&Limits)->Self{Self{protect_groups:4,protect_recent:(l.peers_max/16).max(4)}}}
/// The peer to evict for a newcomer, or None (pure; Bitcoin Core's SelectNodeToEvict without its latency and
/// relay-type steps). `c` is every open, established, non-exempt connection but the newcomer's; it is sorted in place
/// and its `prot` marks set, so nothing is allocated. A candidate is a `free && !prot` entry. Every sort key ends in
/// the unique id, so the result does not depend on the input order.
/// 1. The lottery: over the candidates by (Reverse(rank), key, id), the first (oldest) of each of the first
///    `protect_groups` distinct keys. An attacker cannot know which keys this process favours.
/// 2. Per `Useful` kind, the `protect_recent` latest deliverers (last > 0): each kind has its own quota, so cheap
///    shares cannot crowd out a template relayer.
/// 3. The older half of the candidates (by id, rounded down).
/// 4. With no candidate left, None.
/// 5. The key with the most entries of `c`, protected and graced ones included (credits cannot shrink a group's
///    apparent size), among the keys that still have a candidate, ties to the youngest candidate; its youngest
///    candidate.
fn victim(c:&mut [Cand],p:&Policy)->Option<PeerId>{lottery(c,p.protect_groups);recent(c,p.protect_recent);older_half(c);pick(c)}
fn candidate(x:&Cand)->bool{x.free&&!x.prot}
/// `victim` step 1.
fn lottery(c:&mut [Cand],groups:usize){
    c.sort_unstable_by_key(|x|(Reverse(x.rank),x.key,x.id));let (mut keys,mut prev)=(0,None);
    for x in c.iter_mut(){if keys==groups{break}if !candidate(x)||prev==Some(x.key){continue}prev=Some(x.key);keys+=1;x.prot=true;}
}
/// `victim` step 2.
fn recent(c:&mut [Cand],quota:usize){
    for k in 0..4{
        c.sort_unstable_by_key(|x|(Reverse(x.last[k]),x.id));let mut n=0;
        for x in c.iter_mut(){if n==quota||x.last[k]==0{break}if candidate(x){x.prot=true;n+=1;}}
    }
}
/// `victim` step 3.
fn older_half(c:&mut [Cand]){
    c.sort_unstable_by_key(|x|x.id);let half=c.iter().filter(|x|candidate(x)).count()/2;
    for x in c.iter_mut().filter(|x|candidate(x)).take(half){x.prot=true;}
}
/// `victim` steps 4 and 5.
fn pick(c:&mut [Cand])->Option<PeerId>{
    c.sort_unstable_by_key(|x|(x.key,x.id));let mut best:Option<(usize,PeerId)>=None;
    for run in c.chunk_by(|a,b|a.key==b.key){
        if let Some(y)=run.iter().filter(|x|candidate(x)).map(|x|x.id).max(){if best.is_none_or(|b|(run.len(),y)>b){best=Some((run.len(),y));}}
    }
    best.map(|b|b.1)
}
/// The peer to evict for a newcomer that took the spare place of its full address group (pure; `Net::establish`): among
/// the entries of `c` in `key` that are past their grace period, the oldest, the latest deliverer of a new block and the
/// latest deliverer of a locked template are kept (a deliverer only with a credit, last > 0), and the youngest of the
/// rest goes; None if no entry is left. Every choice ends in the unique id, so the input order does not matter. A
/// newcomer of a group can thus evict only within that group, never its oldest member or its latest block or template
/// source, and other groups are untouched (A4).
fn victim_in(c:&[Cand],key:EvictKey)->Option<PeerId>{
    let g=||c.iter().filter(move|x|x.key==key&&x.free);
    let oldest=g().map(|x|x.id).min();
    let latest=|k:Useful|g().filter(|x|x.last[k as usize]>0).max_by_key(|x|(x.last[k as usize],x.id)).map(|x|x.id);
    let kept=[oldest,latest(Useful::Block),latest(Useful::Template)];
    g().map(|x|x.id).filter(|id|!kept.contains(&Some(*id))).max()
}
#[derive(Clone,Debug)]
pub struct Net{pub limits:Limits,pub counters:Arc<Counters>,
    /// The node's configured peers' canonical addresses, each with the number of configured peers at it (`protect`).
    protected:Arc<std::sync::RwLock<HashMap<IpAddr,u8>>>,
    /// The address grouping, `group_of` (unit tests group loopback sources to drive `listen` end to end).
    grouper:fn(IpAddr)->Option<Group>,
    /// The admitted inbound connections and the eviction budget (`enlist`, `establish`, `forget`).
    inbound:Arc<Mutex<Registry>>,
    /// The lottery's keyed hash (Bitcoin Core's nKeyedNetGroup), random per node.
    rank:std::hash::RandomState,
    /// What an eligible Hello carries (`expect_hello`); unset, every Hello is eligible.
    hello_id:Arc<OnceLock<HelloId>>}
impl Net{
    pub fn new(limits:Limits)->Self{let inbound=Arc::new(Mutex::new(Registry::new(limits.inbound_max())));
        Self{limits,counters:Arc::new(Counters::default()),protected:Arc::default(),grouper:group_of,inbound,rank:Default::default(),hello_id:Arc::default()}}
    #[cfg(test)]
    fn with_grouper(mut self,grouper:fn(IpAddr)->Option<Group>)->Self{self.grouper=grouper;self}
    /// Exempts these exact addresses from the per-group inbound limit, and counts each one's configured peers: a node's
    /// configured peers (for example the other seeds, which dial it) are never refused for sharing an address group,
    /// and as many connections from an address as it has configured peers are never evicted (`establish`). Pass every
    /// configured peer's address, duplicates included. Stored canonical: a dual-stack listener reports an IPv4 peer as
    /// ::ffff:a.b.c.d.
    pub fn protect(&self,ips:impl IntoIterator<Item=IpAddr>){if let Ok(mut p)=self.protected.write(){for ip in ips{let n=p.entry(ip.to_canonical()).or_insert(0);*n=n.saturating_add(1);}}}
    /// The group an inbound connection from `ip` is counted in, or None when it is not limited by group.
    pub fn inbound_group(&self,ip:IpAddr)->Option<Group>{
        let ip=ip.to_canonical();
        if self.limits.per_group_max==0||self.protected.read().map_or(false,|p|p.contains_key(&ip)){return None}
        (self.grouper)(ip)
    }
    /// Reserves an inbound connection place (at most `inbound_max`), a handshake place (at most `handshaking_max`) and
    /// a place in `group` (at most `per_group_max` per group, also while other places are free, then `GROUP_SPARE`
    /// more, `Admit::over`), or refuses.
    fn admit(&self,group:Option<Group>)->Option<Admit>{
        let (c,l)=(&self.counters,&self.limits);
        if c.live.fetch_add(1,Ordering::SeqCst)>=l.inbound_max(){c.live.fetch_sub(1,Ordering::SeqCst);c.refused.fetch_add(1,Ordering::SeqCst);return None}
        if c.handshaking.fetch_add(1,Ordering::SeqCst)>=l.handshaking_max{c.handshaking.fetch_sub(1,Ordering::SeqCst);c.live.fetch_sub(1,Ordering::SeqCst);c.refused.fetch_add(1,Ordering::SeqCst);return None}
        let Some(g)=group else{return Some(Admit{over:false})};
        let placed=match c.groups.lock(){Ok(mut m)=>{let n=m.entry(g).or_insert(0);if *n<l.per_group_max.saturating_add(GROUP_SPARE){*n+=1;Some(Admit{over:*n>l.per_group_max})}else{None}}Err(_)=>None};
        if placed.is_none(){c.handshaking.fetch_sub(1,Ordering::SeqCst);c.live.fetch_sub(1,Ordering::SeqCst);c.refused.fetch_add(1,Ordering::SeqCst);c.refused_group.fetch_add(1,Ordering::SeqCst);}
        placed
    }
    /// Gives back the place an admitted inbound connection held in its group.
    fn release_group(&self,group:Option<Group>){
        let Some(g)=group else{return};
        if let Ok(mut m)=self.counters.groups.lock(){if let Some(n)=m.get_mut(&g){*n=n.saturating_sub(1);if *n==0{m.remove(&g);}}}
    }
    /// A poisoned lock is recovered: the code under it is pure and leaves the registry consistent.
    fn registry(&self)->MutexGuard<'_,Registry>{self.inbound.lock().unwrap_or_else(|e|e.into_inner())}
    /// Sets what an eligible Hello carries. The node calls it once its nonce is final and before it listens.
    pub fn expect_hello(&self,id:HelloId){let _=self.hello_id.set(id);}
    /// Whether `m` is a Hello the node would keep (node.rs: `identity::accepts`, the one predicate, and not the node's
    /// own nonce): only such a Hello is established, so a Hello for another network, wire range or consensus identity,
    /// or a self-connection, never evicts or counts as held. True while `expect_hello` was not called (unit tests, a
    /// node without a network).
    pub fn eligible(&self,m:&Msg)->bool{
        let Some(id)=self.hello_id.get() else{return true};
        let Msg::Hello{chain_id,genesis,protocol,compatibility,nonce,..}=m else{return false};
        !(*nonce!=0&&*nonce==id.nonce)&&crate::identity::accepts(&id.consensus,*chain_id,genesis,*protocol,compatibility).is_ok()
    }
    /// Registers an admitted inbound connection (not yet established) with the group place it holds and whether that
    /// place is the spare one (`admit`), right before the node learns of it.
    fn enlist(&self,p:&Peer,group:Option<Group>,over:bool){
        let ip=p.remote.map(|a|a.ip().to_canonical());let key=evict_key(self.grouper,ip);let mut r=self.registry();
        // Never grown: at most `live` <= inbound_max connections are enlisted (released ones are forgotten first).
        if r.slots.len()<self.limits.inbound_max(){r.slots.push(Slot{id:p.id,key,ip,group,hello:false,exempt:false,over,at:0,act:p.act.clone(),out:p.out.clone(),sock:p.sock.clone()});}
    }
    /// Removes a released inbound connection from the registry.
    fn forget(&self,id:PeerId){let mut r=self.registry();if let Some(i)=r.slots.iter().position(|s|s.id==id){r.slots.swap_remove(i);}}
    /// The inbound connections established and still open.
    pub fn established(&self)->usize{self.registry().slots.iter().filter(|s|open(s)).count()}
    /// An inbound connection's first eligible Hello: true once it is established, false if it must be closed. While
    /// `peers_max` are held (established and open; a victim already closed no longer counts), a newcomer evicts the
    /// `victim` among the others (open, established, not exempt), within the eviction budget; one that cannot is
    /// refused. A configured peer's connection is exempt (at most `protect`'s count per address): it is never a victim,
    /// is not charged, and may exceed the cap by at most min(configured peers, handshaking_max/2) when no victim exists,
    /// so at least half of the handshake places remain. A newcomer in its group's spare place (`Admit::over`) whose
    /// group still holds more than per_group_max places evicts within that group (`victim_in`, the same budget and
    /// grace, among the connections that hold a place in it), which also frees a place in the set, or is refused by group; one whose group has shrunk meanwhile is an
    /// ordinary newcomer. The victim keeps its live and group places until its own release, so physical connections
    /// stay within inbound_max. Lock order: the registry, then a victim's outbox or the group counts; no path takes the
    /// registry while it holds either.
    pub fn establish(&self,id:PeerId)->bool{
        let (l,c)=(&self.limits,&self.counters);
        // `now` is read once the lock is held, so the charges recorded under it never go back in time.
        let mut g=self.registry();let now=mono_ms();let r=&mut *g;
        let Some(me)=r.slots.iter().position(|s|s.id==id) else{return true};
        let held=r.slots.iter().filter(|s|open(s)).count();
        let (ip,key)=(r.slots[me].ip,r.slots[me].key);
        let (allowed,configured)=self.protected.read().map(|p|(ip.and_then(|ip|p.get(&ip)).map_or(0,|n|usize::from(*n)),p.values().map(|n|usize::from(*n)).sum::<usize>())).unwrap_or((0,0));
        let exempt=allowed>0&&r.slots.iter().filter(|s|open(s)&&s.exempt&&s.ip==ip).count()<allowed;
        r.slots[me].exempt=exempt;
        // The spare place's newcomer evicts within its group only while the group is still over its cap.
        let group=r.slots[me].group.filter(|g|r.slots[me].over&&c.groups.lock().map_or(true,|m|m.get(g).copied().unwrap_or(0)>l.per_group_max));
        if group.is_some()||held>=l.peers_max{
            if !exempt&&!r.allows(key,now,&l.evict){c.refused_rate.fetch_add(1,Ordering::SeqCst);c.refused.fetch_add(1,Ordering::SeqCst);return false}
            let grace=ms(l.evict.grace);r.scratch.clear();
            // In the group path only the slots that hold a place in that group are candidates: a configured address's
            // further connections share its key but hold no place (`inbound_group`), so evicting one would leave the
            // group past its cap with the spare place taken, refusing every later newcomer of it at accept.
            r.scratch.extend(r.slots.iter().filter(|s|s.id!=id&&open(s)&&!s.exempt&&(group.is_none()||s.group==group)).map(|s|Cand{id:s.id,key:s.key,rank:self.rank.hash_one(s.key),last:s.act.get(),free:now.saturating_sub(s.at)>=grace,prot:false}));
            let v=match group{Some(g)=>victim_in(&r.scratch,EvictKey::Net(g)),None=>victim(&mut r.scratch,&Policy::of(l))};
            match v{
                Some(v)=>{if let Some(s)=r.slots.iter().find(|s|s.id==v){shut(&s.out,&s.sock);}c.evicted.fetch_add(1,Ordering::SeqCst);
                    if group.is_some(){c.evicted_in_group.fetch_add(1,Ordering::SeqCst);}if !exempt{r.charge(key,now,&l.evict);}}
                None if exempt&&group.is_none()&&held<l.peers_max.saturating_add(configured.min(l.handshaking_max/2))=>{}
                None=>{if group.is_some(){c.refused_group.fetch_add(1,Ordering::SeqCst);}else{c.refused_full.fetch_add(1,Ordering::SeqCst);}c.refused.fetch_add(1,Ordering::SeqCst);return false}
            }
        }
        let s=&mut r.slots[me];s.hello=true;s.at=now;s.over=false;true
    }
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub enum Msg{
    /// Status (network.md:7): chainId, genesisHash, headHash, height, totalWork (32 B big-endian), name, the
    /// sender's maximum wire version and its consensus identity (`compatibility`, VA-SPEC §2.1); then peer discovery's two fields (VA-SPEC §1a.1, §2): the
    /// port the sender's listener accepts peers on and the sender's per-process self-connection nonce (8 bytes),
    /// each 0 for none.
    Hello{chain_id:u64,genesis:[u8;32],head:[u8;32],height:u64,work:[u8;32],node:String,protocol:u64,compatibility:[u8;32],listen_port:u16,nonce:u64},
    Block{header:Vec<u8>,body:Vec<u8>},
    Template{shell:Vec<u8>,body:Vec<u8>},
    Share{tid:[u8;32],nonce:u64},
    GetBlock{hash:[u8;32]},
    GetRange{start:u64,count:u64},
    Tx{raw:Vec<u8>},
    /// GetHeaders-like range by locator (network.md:8: locator <= 32, count <= 512): the answer is the
    /// responder's canonical blocks after the highest locator hash on its canonical chain.
    GetFrom{locator:Vec<[u8;32]>,count:u64},
    /// Peer discovery (VA-SPEC §1a.5): asks a peer for addresses. No fields.
    GetAddr,
    /// The answer to GetAddr: at most ADDR_MAX addresses, sent as one byte string of ADDR_RECORD-byte records
    /// (a 16-byte IPv6 address, IPv4 as ::ffff:a.b.c.d, then a nonzero big-endian port). Decoding loses
    /// nothing: a mapped address is read as IPv4, any other as IPv6 with flowinfo and scope 0. Duplicates and
    /// addresses that are not globally reachable are no decode error; the receiver filters them.
    Addr{addrs:Vec<SocketAddr>},
}
/// Locator hashes per GetFrom (network.md:8).
pub const LOCATOR_MAX:usize=32;
/// Addr (VA-SPEC §1a.5): the bytes of one record (a 16-byte IPv6 address and a u16 port) and the records of one
/// answer. The largest Addr frame is 1,159 B.
pub const ADDR_RECORD:usize=18;pub const ADDR_MAX:usize=64;
fn u64b(v:u64,o:&mut Vec<u8>){let b=v.to_be_bytes();let f=b.iter().position(|x|*x!=0).unwrap_or(8);rlp::encode_bytes(&b[f..],o)}
impl Msg{
    pub fn encode(&self)->Vec<u8>{
        let mut p=Vec::new();
        match self{
            Msg::Hello{chain_id,genesis,head,height,work,node,protocol,compatibility,listen_port,nonce}=>{u64b(0,&mut p);u64b(*chain_id,&mut p);rlp::encode_bytes(genesis,&mut p);rlp::encode_bytes(head,&mut p);u64b(*height,&mut p);rlp::encode_bytes(work,&mut p);rlp::encode_bytes(node.as_bytes(),&mut p);u64b(*protocol,&mut p);rlp::encode_bytes(compatibility,&mut p);
                u64b(u64::from(*listen_port),&mut p);rlp::encode_bytes(&nonce.to_be_bytes(),&mut p);}
            Msg::Block{header,body}=>{u64b(1,&mut p);rlp::encode_bytes(header,&mut p);rlp::encode_bytes(body,&mut p);}
            Msg::Template{shell,body}=>{u64b(2,&mut p);rlp::encode_bytes(shell,&mut p);rlp::encode_bytes(body,&mut p);}
            Msg::Share{tid,nonce}=>{u64b(3,&mut p);rlp::encode_bytes(tid,&mut p);rlp::encode_bytes(&nonce.to_be_bytes(),&mut p);}
            Msg::GetBlock{hash}=>{u64b(4,&mut p);rlp::encode_bytes(hash,&mut p);}
            Msg::GetRange{start,count}=>{u64b(5,&mut p);u64b(*start,&mut p);u64b(*count,&mut p);}
            Msg::Tx{raw}=>{u64b(6,&mut p);rlp::encode_bytes(raw,&mut p);}
            Msg::GetFrom{locator,count}=>{u64b(7,&mut p);rlp::encode_bytes(&locator.concat(),&mut p);u64b(*count,&mut p);}
            Msg::GetAddr=>u64b(8,&mut p),
            Msg::Addr{addrs}=>{u64b(9,&mut p);let mut r=Vec::with_capacity(addrs.len()*ADDR_RECORD);
                for a in addrs{let ip=match a.ip(){IpAddr::V4(v4)=>v4.to_ipv6_mapped(),IpAddr::V6(v6)=>v6};r.extend(ip.octets());r.extend(a.port().to_be_bytes());}
                rlp::encode_bytes(&r,&mut p);}
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
            0=>{n(11)?;let name=bytes(6)?;if name.is_empty()||name.len()>NODE_NAME_MAX||!name.iter().all(|b|b.is_ascii_graphic()){return Err("node name".into())}
                let nb=bytes(10)?;if nb.len()!=8{return Err("nonce".into())}
                Msg::Hello{chain_id:num(1)?,genesis:h32(2)?,head:h32(3)?,height:num(4)?,work:h32(5)?,node:String::from_utf8(name).map_err(|_|"utf8")?,protocol:num(7)?,compatibility:h32(8)?,
                    listen_port:u16::try_from(num(9)?).map_err(|_|"port")?,nonce:u64::from_be_bytes(nb.try_into().unwrap())}}
            1=>{n(3)?;Msg::Block{header:bytes(1)?,body:bytes(2)?}}
            2=>{n(3)?;Msg::Template{shell:bytes(1)?,body:bytes(2)?}}
            3=>{n(3)?;let nb=bytes(2)?;if nb.len()!=8{return Err("nonce".into())}Msg::Share{tid:h32(1)?,nonce:u64::from_be_bytes(nb.try_into().unwrap())}}
            4=>{n(2)?;Msg::GetBlock{hash:h32(1)?}}
            5=>{n(3)?;Msg::GetRange{start:num(1)?,count:num(2)?}}
            6=>{n(2)?;Msg::Tx{raw:bytes(1)?}}
            7=>{n(3)?;let l=bytes(1)?;if l.is_empty()||l.len()%32!=0||l.len()>32*LOCATOR_MAX{return Err("locator".into())}
                Msg::GetFrom{locator:l.chunks(32).map(|c|c.try_into().unwrap()).collect(),count:num(2)?}}
            8=>{n(1)?;Msg::GetAddr}
            9=>{n(2)?;let r=bytes(1)?;if r.len()%ADDR_RECORD!=0||r.len()>ADDR_RECORD*ADDR_MAX{return Err("addr".into())}
                let mut addrs=Vec::with_capacity(r.len()/ADDR_RECORD);
                for c in r.chunks_exact(ADDR_RECORD){let port=u16::from_be_bytes([c[16],c[17]]);if port==0{return Err("addr".into())}
                    let ip=std::net::Ipv6Addr::from(<[u8;16]>::try_from(&c[..16]).unwrap());addrs.push(SocketAddr::new(ip.to_ipv4_mapped().map_or(IpAddr::V6(ip),IpAddr::V4),port));}
                Msg::Addr{addrs}}
            _=>return Err("type".into())
        };
        if m.encode()!=b{return Err("noncanonical".into())}
        Ok(m)
    }
}

pub type PeerId=u64;
#[derive(Debug)]
/// `Legacy`: the connection's first frame was PoCol 0.4.0's 9-item Hello (`legacy_hello`); its Closed follows.
pub enum NetEvent{Connected(PeerId,Peer),Message(PeerId,Msg),Closed(PeerId),Legacy(PeerId)}
/// Whether `b` is a list whose first item is type 0 (Hello) with the nine items of the PoCol 0.4.0 Hello (no listenPort
/// and nonce, VA-SPEC §1a.1). It is only counted and logged: such a peer is refused by arity as before.
pub fn legacy_hello(b:&[u8])->bool{
    let Ok(it)=rlp::decode(b) else{return false};let Some(f)=it.list() else{return false};
    f.len()==9&&f.first().and_then(|x|x.bytes()).is_some_and(|t|t.is_empty())
}
/// How a connection came about: accepted by the listener, or dialed by this node for a configured peer (`dial`)
/// or by an automatic outbound slot (VA-SPEC §1a.7). A dialed connection keeps the address it was dialed at, and
/// the node's outbound bookkeeping is keyed by that address, never by the one the socket reports (H2).
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Origin{Inbound,Configured(SocketAddr),Slot(u8,SocketAddr)}
impl Origin{
    /// The address this node dialed; None for an inbound connection.
    pub fn dialed(self)->Option<SocketAddr>{match self{Origin::Inbound=>None,Origin::Configured(a)|Origin::Slot(_,a)=>Some(a)}}
}

/// Outbound frames of one peer: bytes queued, closed flag; the writer thread drains it. `finish`: the writer closes
/// the peer once it has drained the queue (`Peer::finish`).
struct Outbox{q:Mutex<(VecDeque<Vec<u8>>,usize)>,cv:Condvar,closed:AtomicBool,finish:AtomicBool,max:usize}
/// A connection handed to the node. `remote` is the address its socket reports (None if `peer_addr` failed, a
/// reset right after connect, say) and `addr` is that address as text ("" then); `outbound` is
/// `origin != Origin::Inbound`. `act`: what it delivered first, and when (`credit`; inbound eviction reads it).
#[derive(Clone)]
pub struct Peer{pub id:PeerId,pub addr:String,pub outbound:bool,pub origin:Origin,pub remote:Option<SocketAddr>,out:Arc<Outbox>,sock:Arc<TcpStream>,counters:Arc<Counters>,act:Arc<Activity>}
impl std::fmt::Debug for Peer{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{write!(f,"Peer({} {})",self.id,self.addr)}}
impl Peer{
    fn enqueue(&self,m:&Msg,limit:usize)->Result<(),bool>{
        if self.out.closed.load(Ordering::SeqCst){return Err(false)}
        let b=m.encode();if b.len()>MAX_FRAME{return Err(false)}
        let mut q=match self.out.q.lock(){Ok(q)=>q,Err(_)=>return Err(false)};
        if q.1+b.len()+4>limit{return Err(true)}
        q.1+=b.len()+4;let mut f=(b.len() as u32).to_be_bytes().to_vec();f.extend(b);q.0.push_back(f);drop(q);self.out.cv.notify_one();Ok(())
    }
    /// The address of this end of the connection (None if `local_addr` fails).
    pub fn local(&self)->Option<SocketAddr>{self.sock.local_addr().ok()}
    /// Gossip: never blocks. A peer whose queue is full stops reading; it is dropped.
    pub fn send(&self,m:&Msg)->bool{
        match self.enqueue(m,self.out.max){Ok(())=>true,Err(full)=>{if full{self.counters.dropped_slow.fetch_add(1,Ordering::SeqCst);self.close();}false}}
    }
    /// A response that may be cut short (range sync): false once half the queue is in use;
    /// the peer is kept and asks again.
    pub fn offer(&self,m:&Msg)->bool{self.enqueue(m,self.out.max/2).is_ok()}
    pub fn queued_bytes(&self)->usize{self.out.q.lock().map(|q|q.1).unwrap_or(0)}
    pub fn close(&self){shut(&self.out,&self.sock)}
    /// Records that this connection delivered something of kind `u` first, now (inbound eviction's protection).
    pub fn credit(&self,u:Useful){self.act.last[u as usize].store(mono_ms(),Ordering::SeqCst);}
    /// The time (`mono_ms`) of its last credited delivery of each `Useful` kind, in their order; 0 = never.
    pub fn activity(&self)->[u64;4]{self.act.get()}
    /// Closes the connection once the frames queued so far are written (the writer closes it), where `close` drops
    /// them: a self-connection's inbound end (VA-SPEC §2) must still deliver this node's Hello, whose nonce tells the
    /// dialing end, the only one that knows the address to stop dialing. Set under the queue lock, as `closed` is.
    pub fn finish(&self){{let _q=self.out.q.lock();self.out.finish.store(true,Ordering::SeqCst);}self.out.cv.notify_all();}
}

/// The flag is set under the queue lock: the writer tests it under that lock before waiting, so
/// a close between its test and its wait cannot lose the wakeup (the reader joins the writer).
fn close_outbox(out:&Outbox){{let _q=out.q.lock();out.closed.store(true,Ordering::SeqCst);}out.cv.notify_all();}
/// Closes a connection: its outbox (queued frames are dropped, the writer ends) and its socket, which wakes its reader
/// (`Peer::close`, and an eviction's victim, `Net::establish`).
fn shut(out:&Outbox,sock:&TcpStream){close_outbox(out);let _=sock.shutdown(std::net::Shutdown::Both);}
/// The writer's wait for its next frame; None once the peer is closed, or finished and drained. `between` runs after
/// the closed test and before the wait, with the lock held (a test hook; a no-op in the node).
fn next_frame(out:&Outbox,between:&dyn Fn())->Option<Vec<u8>>{
    let Ok(mut q)=out.q.lock() else{return None};
    loop{
        if out.closed.load(Ordering::SeqCst){return None}
        if let Some(f)=q.0.pop_front(){q.1-=f.len();return Some(f)}
        if out.finish.load(Ordering::SeqCst){return None}
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
/// The places an admitted connection holds, given back once by `Drop`: an outbound connection's count, or an inbound
/// one's registry slot (`Net::forget`, first), live place, handshake place while `handshaking` and group place. A Drop
/// guard, so a reader that panics still gives them back.
struct Release{net:Net,id:PeerId,group:Option<Group>,outbound:bool,handshaking:bool}
impl Drop for Release{
    fn drop(&mut self){let c=&self.net.counters;
        if self.outbound{c.outbound.fetch_sub(1,Ordering::SeqCst);}
        else{self.net.forget(self.id);c.live.fetch_sub(1,Ordering::SeqCst);if self.handshaking{c.handshaking.fetch_sub(1,Ordering::SeqCst);}self.net.release_group(self.group);}
    }
}
/// Starts reader and writer threads for an admitted connection (inbound: slot already reserved).
/// Returns whether the connection was handed to the node (every later end sends Closed), and
/// the reader's handle.
/// A13 thread inventory: the reader joins the writer, then delivers Closed, and only then
/// releases the slot, so a released slot never has a thread behind it that can still block (a
/// full event queue keeps the slot taken instead). The listener and the dialer join the reader
/// before its place is used again, so exiting readers are counted too.
/// `origin`: how the connection came about; a dialed one is counted in `Counters::outbound`, not in the inbound
/// slots. `group`: the address group an inbound connection holds a place in (`Net::admit`), given back with
/// its slot, by the same single release (`Release`); `over`: that place is the group's spare one (`Admit::over`). An inbound connection is enlisted in the registry right before
/// the node learns of it, and established at its first eligible Hello (`Net::establish`), which may evict another or
/// refuse it: a refused connection ends before its Hello reaches the node.
fn spawn_peer(stream:TcpStream,origin:Origin,events:SyncSender<NetEvent>,net:Net,group:Option<Group>,over:bool)->(bool,Option<std::thread::JoinHandle<()>>){
    let outbound=origin!=Origin::Inbound;
    if outbound{net.counters.outbound.fetch_add(1,Ordering::SeqCst);}
    let id=NEXT.fetch_add(1,Ordering::SeqCst);
    let release=Release{net:net.clone(),id,group,outbound,handshaking:true};
    let _=stream.set_nodelay(true);let remote=stream.peer_addr().ok();let addr=remote.map(|a|a.to_string()).unwrap_or_default();
    let (Ok(reader),Ok(writer))=(stream.try_clone(),stream.try_clone()) else{drop(release);return (false,None)};
    let out=Arc::new(Outbox{q:Mutex::new((VecDeque::new(),0)),cv:Condvar::new(),closed:AtomicBool::new(false),finish:AtomicBool::new(false),max:net.limits.queue_bytes_max});
    let peer=Peer{id,addr,outbound,origin,remote,out:out.clone(),sock:Arc::new(stream),counters:net.counters.clone(),act:Arc::default()};
    let write_deadline=net.limits.write_deadline;
    let wpeer=peer.clone();
    let writer=crate::threads::spawn(format!("peer-{id}-w"),crate::threads::IO_STACK,move||{
        let mut w=writer;let _=w.set_write_timeout(Some(write_deadline));
        loop{
            // None: closed already, or finished and drained (closed here).
            let Some(frame)=next_frame(&out,&||{}) else{wpeer.close();return};
            if let Err(e)=w.write_all(&frame){
                // A write that cannot finish within write_deadline: the peer stopped reading.
                if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut){wpeer.counters.dropped_slow.fetch_add(1,Ordering::SeqCst);}
                wpeer.close();return
            }
        }
    });
    let Ok(writer)=writer else{drop(release);return (false,None)};
    if !outbound{net.enlist(&peer,group,over);}
    if events.send(NetEvent::Connected(id,peer.clone())).is_err(){peer.close();let _=writer.join();drop(release);return (false,None)}
    let writer=Arc::new(Mutex::new(Some(writer)));let rwriter=writer.clone();
    let join=|w:&Mutex<Option<std::thread::JoinHandle<()>>>|{if let Some(h)=w.lock().ok().and_then(|mut g|g.take()){let _=h.join();}};
    let rpeer=peer.clone();let rnet=net.clone();
    let tev=events.clone();
    // The reader takes the guard when it starts; a spawn that fails leaves it here, given back after Closed.
    let guard=Arc::new(Mutex::new(Some(release)));let rguard=guard.clone();
    let spawned=crate::threads::spawn(format!("peer-{id}"),crate::threads::IO_STACK,move||{let net=rnet;let events=tev;
        let Some(mut release)=rguard.lock().unwrap_or_else(|e|e.into_inner()).take() else{return};drop(rguard);
        let mut r=reader;let l=net.limits.clone();let mut hello=false;let mut last=Instant::now();let started=Instant::now();
        loop{
            let wait=if hello{last+l.idle_deadline}else{started+l.hello_deadline};
            let mut len=[0u8;4];if !read_by(&mut r,&mut len,wait,&rpeer.out.closed){break}
            let n=u32::from_be_bytes(len) as usize;if n==0||n>MAX_FRAME{break}
            if !hello&&n>HELLO_MAX{break}
            // Before the Hello the whole handshake frame must also meet the Hello deadline.
            let frame_by=Instant::now()+l.frame_deadline;let frame_by=if hello{frame_by}else{frame_by.min(wait)};
            let mut buf=vec![0u8;n];if !read_by(&mut r,&mut buf,frame_by,&rpeer.out.closed){break}
            // A first frame that is PoCol 0.4.0's 9-item Hello is named, so a node still meeting 0.4.0 peers can tell.
            let Ok(m)=Msg::decode(&buf) else{if !hello&&legacy_hello(&buf){let _=events.send(NetEvent::Legacy(id));}break};
            if !hello{
                // Nothing but a Hello is accepted before the handshake.
                if !matches!(m,Msg::Hello{..}){break}
                hello=true;release.handshaking=false;
                // An ineligible Hello reaches the node unestablished (never held, never a candidate), which closes it.
                if !outbound{net.counters.handshaking.fetch_sub(1,Ordering::SeqCst);if net.eligible(&m)&&!net.establish(id){break}}
            }
            last=Instant::now();
            if events.send(NetEvent::Message(id,m)).is_err(){break}
        }
        rpeer.close();join(&rwriter);let _=events.send(NetEvent::Closed(id));drop(release);
    });
    // The node already knows this peer: it must also learn that it is gone.
    match spawned{Ok(h)=>(true,Some(h)),Err(_)=>{peer.close();join(&writer);let _=events.send(NetEvent::Closed(id));drop(guard);(true,None)}}
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
            let room=readers.len()<net.limits.inbound_max();if !room{net.counters.refused.fetch_add(1,Ordering::SeqCst);}
            let group=s.peer_addr().ok().and_then(|a|net.inbound_group(a.ip()));
            match if room{net.admit(group)}else{None}{
                Some(a)=>{if let (_,Some(h))=spawn_peer(s,Origin::Inbound,events.clone(),net.clone(),group,a.over){readers.push(h);}}
                None=>{let _=s.shutdown(std::net::Shutdown::Both);}
            }
        }
    }).expect("spawn listener");
}
/// The dialer's poll, and its redial delay after a connection that lasted (`redial_delay`).
pub const DIAL_RETRY:Duration=Duration::from_millis(300);
/// A connection that lasted at least this long resets the redial delay to DIAL_RETRY.
pub const DIAL_STABLE:Duration=Duration::from_secs(60);
/// The longest redial delay.
pub const DIAL_BACKOFF_MAX:Duration=Duration::from_secs(30);
/// The wait before a configured peer is dialed again, from the previous wait and how long the last connection lasted:
/// - `None` (the connect failed, or the connection was not handed to the node): the previous wait;
/// - at least DIAL_STABLE: DIAL_RETRY;
/// - shorter: twice the previous wait, at most DIAL_BACKOFF_MAX.
///
/// So a peer that closes at once (a full node that evicts or refuses this one, inbound eviction) is redialed after 0.6 s,
/// 1.2 s, ... 30 s, not every 300 ms, and the evictions a redial would cause there settle down; a stable peer that is
/// lost is redialed at once.
pub fn redial_delay(prev:Duration,lasted:Option<Duration>)->Duration{
    match lasted{None=>prev,Some(l) if l>=DIAL_STABLE=>DIAL_RETRY,Some(_)=>prev.saturating_mul(2).min(DIAL_BACKOFF_MAX)}
}
/// Keeps an outbound connection to `addr`, reconnecting after loss with `redial_delay`'s backoff. The node
/// maintains `connected` from Connected/Closed events (outbound peer addresses).
pub fn dial(addr:SocketAddr,events:SyncSender<NetEvent>,connected:Arc<Mutex<std::collections::HashSet<String>>>,net:Net){
    crate::threads::spawn(format!("dial-{addr}"),crate::threads::IO_STACK,move||{
        let mut reader:Option<std::thread::JoinHandle<()>>=None;
        // `since`: when the live connection was handed to the node; `next`: the earliest time to dial again.
        let (mut since,mut delay,mut next):(Option<Instant>,Duration,Instant)=(None,DIAL_RETRY,Instant::now());
        loop{
            let live=connected.lock().map(|s|s.contains(&addr.to_string())).unwrap_or(false);
            if !live{
                // The node has seen Closed, which the previous reader sends just before it ends.
                if let Some(h)=reader.take(){let _=h.join();}
                if let Some(t)=since.take(){delay=redial_delay(delay,Some(t.elapsed()));next=Instant::now()+delay;}
                if Instant::now()>=next{
                    let handed=match TcpStream::connect_timeout(&addr,Duration::from_millis(500)){
                        Ok(s)=>{
                            if let Ok(mut set)=connected.lock(){set.insert(addr.to_string());}
                            // Not handed to the node (no Closed will follow): dial again later.
                            let (handed,h)=spawn_peer(s,Origin::Configured(addr),events.clone(),net.clone(),None,false);reader=h;
                            if !handed{if let Ok(mut set)=connected.lock(){set.remove(&addr.to_string());}}
                            handed}
                        Err(_)=>false};
                    if handed{since=Some(Instant::now());}else{delay=redial_delay(delay,None);next=Instant::now()+delay;}
                }
            }
            std::thread::sleep(DIAL_RETRY);
        }
    }).expect("spawn dialer");
}

/// Unit tests only: a peer on a loopback pair whose outbox has no writer thread, so its queue only fills.
/// The other end of the pair is returned to keep the connection open.
#[cfg(test)]
pub(crate) fn test_peer(id:PeerId,max:usize)->(Peer,TcpStream){
    let l=TcpListener::bind("127.0.0.1:0").unwrap();let s=TcpStream::connect(l.local_addr().unwrap()).unwrap();let (r,_)=l.accept().unwrap();
    let out=Arc::new(Outbox{q:Mutex::new((VecDeque::new(),0)),cv:Condvar::new(),closed:AtomicBool::new(false),finish:AtomicBool::new(false),max});
    (Peer{id,addr:"test".into(),outbound:false,origin:Origin::Inbound,remote:None,out,sock:Arc::new(s),counters:Arc::new(Counters::default()),act:Arc::default()},r)
}
/// Unit tests only: `test_peer` as a connection of this `origin` whose socket reported `remote`; `addr` and
/// `outbound` follow from them as in `spawn_peer` (`addr` is "" when `remote` is None).
#[cfg(test)]
pub(crate) fn test_peer_as(id:PeerId,max:usize,origin:Origin,remote:Option<SocketAddr>)->(Peer,TcpStream){
    let (mut p,r)=test_peer(id,max);p.origin=origin;p.remote=remote;p.outbound=origin!=Origin::Inbound;p.addr=remote.map(|a|a.to_string()).unwrap_or_default();(p,r)
}
/// Unit tests only: the messages waiting in a `test_peer`'s queue, oldest first (it has no writer thread), and whether
/// it was closed or finished.
#[cfg(test)]
impl Peer{
    pub(crate) fn queued(&self)->Vec<Msg>{self.out.q.lock().unwrap().0.iter().map(|f|Msg::decode(&f[4..]).unwrap()).collect()}
    pub(crate) fn ended(&self)->(bool,bool){(self.out.closed.load(Ordering::SeqCst),self.out.finish.load(Ordering::SeqCst))}
}
#[cfg(test)]
mod tests{
    use super::*;
    use std::sync::mpsc;
    /// D-REORG-REDB wire: Status carries the 32-byte totalWork, GetFrom a locator of 1..=32 hashes;
    /// both round-trip, and a locator that is empty, ragged or longer than 32 hashes is refused.
    #[test] fn status_work_and_locator_requests_round_trip_within_bounds(){
        let h=Msg::Hello{chain_id:7,genesis:[1;32],head:[2;32],height:9,work:[3;32],node:"n".into(),protocol:crate::identity::PROTOCOL_VERSION,compatibility:[4;32],listen_port:0,nonce:0};assert_eq!(Msg::decode(&h.encode()).unwrap(),h);
        let g=Msg::GetFrom{locator:vec![[4;32],[5;32]],count:512};assert_eq!(Msg::decode(&g.encode()).unwrap(),g);
        let full=Msg::GetFrom{locator:vec![[6;32];LOCATOR_MAX],count:1};assert_eq!(Msg::decode(&full.encode()).unwrap(),full);
        let raw=|loc:Vec<u8>|{let mut p=Vec::new();u64b(7,&mut p);rlp::encode_bytes(&loc,&mut p);u64b(1,&mut p);let mut o=Vec::new();rlp::encode_list_payload(&p,&mut o);o};
        for bad in [vec![],vec![0u8;31],vec![0u8;33],vec![0u8;32*(LOCATOR_MAX+1)]]{assert!(Msg::decode(&raw(bad)).is_err());}
        // The previous Hello (no work field) is refused by arity, never read with a default work.
        let mut p=Vec::new();u64b(0,&mut p);u64b(7,&mut p);rlp::encode_bytes(&[1;32],&mut p);rlp::encode_bytes(&[2;32],&mut p);u64b(9,&mut p);rlp::encode_bytes(b"n",&mut p);
        let mut o=Vec::new();rlp::encode_list_payload(&p,&mut o);assert!(Msg::decode(&o).is_err());
    }
    /// Peer discovery (VA-SPEC §1a.1, §2): Hello ends with listenPort, a minimal integer of at most 65535, and an
    /// 8-byte nonce, each 0 for none; both round-trip. A port above 65535 or not minimal, a nonce of another length
    /// and the previous 9-item Hello are refused, each for its own reason (the re-encoding check alone would refuse
    /// a truncated port too, as noncanonical). The largest Hello fits the pre-handshake bound HELLO_MAX.
    #[test] fn hello_carries_listen_port_and_nonce_and_the_previous_hello_is_refused(){
        let hello=|listen_port,nonce|Msg::Hello{chain_id:7,genesis:[1;32],head:[2;32],height:9,work:[3;32],node:"n".into(),protocol:2,compatibility:[4;32],listen_port,nonce};
        for (port,nonce) in [(30300,0x0102030405060708),(0,0),(u16::MAX,u64::MAX)]{let h=hello(port,nonce);assert_eq!(Msg::decode(&h.encode()),Ok(h));}
        // The nine items of the previous Hello, then the items a case appends.
        let int=|v:u64|{let mut o=Vec::new();u64b(v,&mut o);o};let bin=|b:&[u8]|{let mut o=Vec::new();rlp::encode_bytes(b,&mut o);o};
        let raw=|tail:&[Vec<u8>]|{let head=[int(0),int(7),bin(&[1;32]),bin(&[2;32]),int(9),bin(&[3;32]),bin(b"n"),int(2),bin(&[4;32])];
            let mut o=Vec::new();rlp::encode_list_payload(&[&head[..],tail].concat().concat(),&mut o);o};
        assert_eq!(Msg::decode(&raw(&[int(30300),bin(&[5;8])])),Ok(hello(30300,0x0505050505050505)),"the eleven items");
        assert_eq!(Msg::decode(&raw(&[int(65536),bin(&[5;8])])),Err("port".into()),"a port above 65535");
        assert_eq!(Msg::decode(&raw(&[bin(&[0,1]),bin(&[5;8])])),Err("int".into()),"a port with a leading zero");
        for len in [0,7,9]{assert_eq!(Msg::decode(&raw(&[int(1),bin(&vec![5;len])])),Err("nonce".into()),"a {len}-byte nonce");}
        assert_eq!(Msg::decode(&raw(&[])),Err("arity".into()),"the previous 9-item Hello");
        assert!(legacy_hello(&raw(&[]))&&!legacy_hello(&raw(&[int(1),bin(&[5;8])])),"named only for the 9 items (NetEvent::Legacy)");
        assert_eq!(Msg::decode(&raw(&[int(1),bin(&[5;8]),int(1)])),Err("arity".into()),"a 12-item Hello");
        let largest=Msg::Hello{chain_id:u64::MAX,genesis:[0xff;32],head:[0xff;32],height:u64::MAX,work:[0xff;32],node:"n".repeat(NODE_NAME_MAX),protocol:u64::MAX,compatibility:[0xff;32],listen_port:u16::MAX,nonce:u64::MAX};
        assert!(largest.encode().len()<=HELLO_MAX,"the largest Hello is {} B",largest.encode().len());assert_eq!(Msg::decode(&largest.encode()),Ok(largest));
    }
    /// Peer discovery (VA-SPEC §1a.5): GetAddr has no fields; Addr carries 0..=ADDR_MAX records of ADDR_RECORD bytes in
    /// one byte string. Mixed IPv4 and IPv6 records round-trip, IPv4 travels mapped and a mapped record is read back as
    /// IPv4. More than ADDR_MAX records, a string that is not whole records, a record with port 0 and a list in place
    /// of the string are refused, each for its own reason (a ragged string read by whole records alone would only fail
    /// the re-encoding check).
    #[test] fn getaddr_and_addr_round_trip_within_bounds(){
        let int=|v:u64|{let mut o=Vec::new();u64b(v,&mut o);o};let bin=|b:&[u8]|{let mut o=Vec::new();rlp::encode_bytes(b,&mut o);o};
        let raw=|items:&[Vec<u8>]|{let mut o=Vec::new();rlp::encode_list_payload(&items.concat(),&mut o);o};
        assert_eq!(Msg::GetAddr.encode(),raw(&[int(8)]));assert_eq!(Msg::decode(&raw(&[int(8)])),Ok(Msg::GetAddr));
        assert_eq!(Msg::decode(&raw(&[int(8),int(1)])),Err("arity".into()),"GetAddr carries no field");
        let addr=|i:usize|if i%2==0{SocketAddr::new(IpAddr::from([81,2,69,i as u8]),30300)}else{SocketAddr::new(IpAddr::from([0x2606,0x4700,0,0,0,0,0,i as u16]),1024+i as u16)};
        for n in [0,1,2,ADDR_MAX]{let m=Msg::Addr{addrs:(0..n).map(addr).collect()};assert_eq!(Msg::decode(&m.encode()),Ok(m),"{n} records");}
        assert_eq!(Msg::Addr{addrs:(0..ADDR_MAX).map(addr).collect()}.encode().len(),1159,"the largest Addr frame");
        let record=|ip:&str,port:u16|{let mut r=ip.parse::<std::net::Ipv6Addr>().unwrap().octets().to_vec();r.extend(port.to_be_bytes());r};
        let mapped=raw(&[int(9),bin(&record("::ffff:81.2.69.7",30300))]);let v4:SocketAddr="81.2.69.7:30300".parse().unwrap();
        assert_eq!(Msg::Addr{addrs:vec![v4]}.encode(),mapped,"IPv4 travels mapped");
        assert_eq!(Msg::decode(&mapped),Ok(Msg::Addr{addrs:vec![v4]}),"a mapped record is read as IPv4");
        let records=|n:usize|(0..n).flat_map(|i|record("2606:4700::1",1024+i as u16)).collect::<Vec<u8>>();
        assert!(matches!(Msg::decode(&raw(&[int(9),bin(&records(ADDR_MAX))])),Ok(Msg::Addr{addrs}) if addrs.len()==ADDR_MAX));
        assert_eq!(Msg::decode(&raw(&[int(9),bin(&records(ADDR_MAX+1))])),Err("addr".into()),"more than ADDR_MAX records");
        for len in [17,19,35]{assert_eq!(Msg::decode(&raw(&[int(9),bin(&vec![1;len])])),Err("addr".into()),"a {len}-byte string");}
        assert_eq!(Msg::decode(&raw(&[int(9),bin(&[records(1),record("2606:4700::2",0)].concat())])),Err("addr".into()),"a record with port 0");
        let list={let mut l=Vec::new();rlp::encode_list_payload(&bin(&records(1)),&mut l);l};
        assert_eq!(Msg::decode(&raw(&[int(9),list])),Err("field".into()),"a list in place of the string");
    }
    /// D-7 re-insertion relays with `offer`: once half the queue is in use an offer is refused and the peer is
    /// kept, while gossip (`send`) past the whole queue drops the peer as slow. No writer thread: the queue
    /// only fills.
    #[test] fn an_offer_past_half_the_queue_is_refused_and_keeps_the_peer(){
        let l=TcpListener::bind("127.0.0.1:0").unwrap();let s=TcpStream::connect(l.local_addr().unwrap()).unwrap();let _r=l.accept().unwrap();
        let out=Arc::new(Outbox{q:Mutex::new((VecDeque::new(),0)),cv:Condvar::new(),closed:AtomicBool::new(false),finish:AtomicBool::new(false),max:4096});
        let p=Peer{id:1,addr:"t".into(),outbound:false,origin:Origin::Inbound,remote:None,out:out.clone(),sock:Arc::new(s),counters:Arc::new(Counters::default()),act:Arc::default()};
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
        let out=Arc::new(Outbox{q:Mutex::new((VecDeque::new(),0)),cv:Condvar::new(),closed:AtomicBool::new(false),finish:AtomicBool::new(false),max:1<<20});
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
    /// `Peer::finish` (a self-connection's inbound end, VA-SPEC §2): every frame queued before it is written, then the
    /// writer closes the connection and its reader ends; `close` would have dropped them. A writer waiting for frames
    /// stays open until the finish wakes it. (The reader's Hello deadline is far away: only the writer ends these.)
    #[test] fn a_finished_peer_writes_what_was_queued_then_closes(){
        let pair=||{let l=TcpListener::bind("127.0.0.1:0").unwrap();let a=l.local_addr().unwrap();let c=TcpStream::connect(a).unwrap();let (s,_)=l.accept().unwrap();
            let net=Net::new(Limits{hello_deadline:Duration::from_secs(120),..Limits::default()});
            let (tx,rx)=mpsc::sync_channel(NET_EVENT_QUEUE);assert!(spawn_peer(s,Origin::Configured(a),tx,net,None,false).0);
            let Ok(NetEvent::Connected(_,p))=rx.recv_timeout(Duration::from_secs(5)) else{panic!("handed to the node")};(p,c,rx)};
        let m=Msg::GetBlock{hash:[7;32]};let f={let b=m.encode();let mut f=(b.len() as u32).to_be_bytes().to_vec();f.extend(b);f};
        let (p,mut c,rx)=pair();for _ in 0..3{assert!(p.send(&m));}p.finish();
        c.set_read_timeout(Some(Duration::from_secs(5))).unwrap();let mut got=Vec::new();c.read_to_end(&mut got).unwrap();
        assert_eq!(got,f.repeat(3),"the frames queued before the finish, then the end of the stream");
        assert!(matches!(rx.recv_timeout(Duration::from_secs(5)),Ok(NetEvent::Closed(_))),"its reader ends");
        let (p,mut c,_rx)=pair();assert!(p.send(&m));c.set_read_timeout(Some(Duration::from_millis(300))).unwrap();
        let mut one=vec![0u8;f.len()];c.read_exact(&mut one).unwrap();assert_eq!(one,f);
        let mut b=[0u8;1];assert!(matches!(c.read(&mut b),Err(e) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)),"open while its writer waits");
        p.finish();c.set_read_timeout(Some(Duration::from_secs(5))).unwrap();assert_eq!(c.read(&mut b).unwrap(),0,"the finish wakes the writer, which closes it");
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
    /// Per-group admission: a group holds at most per_group_max inbound places and its spare one (by default 4 and 1),
    /// also while other slots are free; a refusal by group takes no slot; a released place can be taken again; other
    /// groups and ungrouped peers are unaffected; a configured peer is exempt by its canonical address (a dual-stack
    /// listener reports IPv4 as ::ffff:a.b.c.d); per_group_max 0 disables it.
    #[test] fn a_public_group_holds_at_most_per_group_max_inbound_places(){
        let d=Limits::default();assert_eq!(d.per_group_max,4,"by default one group holds at most 4 inbound places");
        assert_eq!((d.peers_max,d.handshaking_max,d.per_group_max),(32,8,4),"32 established, 8 waiting for a Hello, 4 per group");
        assert_eq!(d.inbound_max(),d.peers_max+d.handshaking_max,"the physical bound counts the places waiting for a Hello apart");
        let net=Net::new(Limits{peers_max:8,handshaking_max:8,per_group_max:3,..Limits::default()});
        let (ga,gb)=(group_of("81.2.69.1".parse().unwrap()),group_of("81.3.1.1".parse().unwrap()));
        assert!(ga.is_some()&&gb.is_some()&&ga!=gb);
        let c=&net.counters;let load=|a:&AtomicUsize|a.load(Ordering::SeqCst);
        let places=|g:Option<Group>|c.groups.lock().unwrap().get(&g.unwrap()).copied().unwrap_or(0);
        // What spawn_peer's single release does when a connection ends.
        let close=|g:Option<Group>|{c.live.fetch_sub(1,Ordering::SeqCst);c.handshaking.fetch_sub(1,Ordering::SeqCst);net.release_group(g);};
        let (cap,spare)=(Some(Admit{over:false}),Some(Admit{over:true}));
        assert_eq!([net.admit(ga),net.admit(ga),net.admit(ga),net.admit(ga)],[cap,cap,cap,spare],"per_group_max places in one group, then its spare one");
        assert_eq!(net.admit(ga),None,"one more from that group is refused although 4 slots are free");
        assert_eq!((load(&c.live),load(&c.handshaking),load(&c.refused),load(&c.refused_group)),(4,4,1,1),"a refusal by group takes no slot");
        assert!(net.admit(gb)==cap&&net.admit(None)==cap&&net.admit(None)==cap,"another group and ungrouped peers are admitted");
        close(ga);assert_eq!(places(ga),3);assert_eq!(net.admit(ga),spare,"a released place can be taken again");
        assert_eq!((places(ga),places(gb)),(4,1));
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
    /// gives its place back exactly once, whether it closes after its Hello, never sends a Hello or is evicted. A
    /// connection to a full group takes the spare place: without a Hello it is closed at its deadline, with one it
    /// evicts the group's youngest member and stays; while the spare place is taken, the next is closed at accept.
    /// The connections that must stay send a Hello, so a slow runner cannot end them early at the Hello deadline.
    #[test] fn listen_takes_and_gives_back_one_group_place_per_inbound_connection(){
        const G:Group=Group::V4([81,2]);
        fn one_group(ip:IpAddr)->Option<Group>{if ip.is_loopback(){Some(G)}else{group_of(ip)}}
        let net=Net::new(Limits{peers_max:4,handshaking_max:4,per_group_max:2,hello_deadline:Duration::from_millis(800),
            evict:EvictLimits{grace:Duration::ZERO,..EvictLimits::default()},..Limits::default()}).with_grouper(one_group);
        let l=TcpListener::bind("127.0.0.1:0").unwrap();let addr=l.local_addr().unwrap();
        let (tx,rx)=mpsc::sync_channel(NET_EVENT_QUEUE);listen(l,tx,net.clone());
        std::thread::spawn(move||{for _ in rx{}});
        let c=net.counters.clone();
        let places=||c.groups.lock().unwrap().get(&G).copied().unwrap_or(0);
        let shaking=||c.handshaking.load(Ordering::SeqCst);
        let until=|f:&dyn Fn()->bool|{let t=Instant::now();while !f()&&t.elapsed()<Duration::from_secs(10){std::thread::sleep(Duration::from_millis(5));}f()};
        let closed=|s:&mut TcpStream|{s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();let mut b=[0u8;1];matches!(s.read(&mut b),Ok(0)|Err(_))};
        let hello=||{let m=Msg::Hello{chain_id:7,genesis:[1;32],head:[1;32],height:0,work:[0;32],node:"g".into(),protocol:crate::identity::PROTOCOL_VERSION,compatibility:[0;32],listen_port:0,nonce:0}.encode();
            let mut f=(m.len() as u32).to_be_bytes().to_vec();f.extend(m);f};
        let mut a=TcpStream::connect(addr).unwrap();a.write_all(&hello()).unwrap();
        let mut b=TcpStream::connect(addr).unwrap();b.write_all(&hello()).unwrap();
        assert!(until(&||places()==2&&shaking()==0),"two connections take the group's places; their Hellos end their handshakes");
        let load=|a:&AtomicUsize|a.load(Ordering::SeqCst);
        let mut x=TcpStream::connect(addr).unwrap();assert!(until(&||places()==3),"a third connection from the full group takes the spare place");
        assert!(closed(&mut x),"without a Hello it is closed at its deadline");
        assert!(until(&||places()==2),"and gives back the spare place");
        let mut y=TcpStream::connect(addr).unwrap();assert!(until(&||places()==3),"the spare place is taken again");
        let mut z=TcpStream::connect(addr).unwrap();
        assert!(closed(&mut z),"a fourth connection while the spare place is taken is closed at accept");
        assert_eq!((load(&c.refused_group),places()),(1,3),"the refused connection took no place");
        y.write_all(&hello()).unwrap();
        assert!(closed(&mut b),"with a Hello the spare place's newcomer evicts the group's youngest member");
        assert!(until(&||places()==2&&shaking()==0),"whose place is given back");
        assert_eq!((load(&c.evicted),load(&c.evicted_in_group),load(&c.refused_group)),(1,1,1));
        a.set_read_timeout(Some(Duration::from_millis(300))).unwrap();let mut one=[0u8;1];
        assert!(matches!(a.read(&mut one),Err(e) if matches!(e.kind(),std::io::ErrorKind::WouldBlock|std::io::ErrorKind::TimedOut)),"the oldest member stays");
        drop(a);assert!(until(&||places()==1),"a closed connection gives back its place");
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(places(),1,"exactly once: y still holds its place");
        let mut d=TcpStream::connect(addr).unwrap();assert!(until(&||places()==2),"the freed place is taken again");
        assert!(closed(&mut d),"a connection that never sends a Hello is closed at its deadline");
        assert!(until(&||places()==1),"and gives back its place");
        drop(y);
        assert!(until(&||places()==0&&c.live.load(Ordering::SeqCst)==0&&shaking()==0),"all places and slots are back");
        assert!(c.groups.lock().unwrap().is_empty(),"no empty group is left behind");
    }
    // ---------------- PR3: inbound eviction ----------------
    /// A full address group admits one spare place (`GROUP_SPARE`, `Admit::over`) and then refuses; every place, the
    /// spare one included, is given back exactly once; per_group_max 0 has no groups and so no spare place.
    #[test] fn a_full_group_admits_one_spare_place(){
        const G:Group=Group::V4([81,2]);
        let net=Net::new(Limits{peers_max:8,handshaking_max:8,per_group_max:2,..Limits::default()}).with_grouper(|_|Some(G));let c=net.counters.clone();
        let load=|a:&AtomicUsize|a.load(Ordering::SeqCst);let places=||c.groups.lock().unwrap().get(&G).copied();
        let got:Vec<Option<Admit>>=(0..4).map(|_|net.admit(Some(G))).collect();
        assert_eq!(got,[Some(Admit{over:false}),Some(Admit{over:false}),Some(Admit{over:true}),None],"the cap, then the spare place, then a refusal");
        assert_eq!((places(),load(&c.live),load(&c.refused_group)),(Some(3),3,1));
        // Each connection's single release (`Release`).
        let mut held=Vec::new();for id in 0..3{held.push(Release{net:net.clone(),id,group:Some(G),outbound:false,handshaking:true});}
        drop(held.pop());assert_eq!(places(),Some(2));assert_eq!(net.admit(Some(G)),Some(Admit{over:true}),"a released spare place can be taken again");
        held.push(Release{net:net.clone(),id:9,group:Some(G),outbound:false,handshaking:true});drop(held);
        assert_eq!((places(),load(&c.live),load(&c.handshaking)),(None,0,0),"every place given back exactly once");
        let off=Net::new(Limits{per_group_max:0,..Limits::default()});assert_eq!(off.inbound_group("81.2.69.1".parse().unwrap()),None);
    }
    /// A newcomer in its full group's spare place, through `Net::establish` on `test_peer` connections (no threads): it
    /// evicts the youngest free member of its own group (`evicted_in_group`), even while the set has room, and never the
    /// group's oldest member, its latest block or template source, a member in grace, or a member of another group; with
    /// nothing to evict in its group it is refused by group. One whose group shrank back to the cap before its Hello is
    /// an ordinary newcomer and evicts nobody. A configured address in the group's range shares its key but holds no
    /// place in it, so its further connections are never the group's victim and the group returns to its cap.
    #[test] fn a_full_group_evicts_within_itself(){
        const G:Group=Group::V4([81,2]);const H:Group=Group::V4([81,3]);
        fn two(ip:IpAddr)->Option<Group>{match ip{IpAddr::V4(a) if a.octets()[1]==2=>Some(G),_=>Some(H)}}
        let net=|per_group_max,grace|Net::new(Limits{peers_max:16,handshaking_max:8,per_group_max,
            evict:EvictLimits{grace,key_interval:Duration::ZERO,burst:1000,refill:Duration::ZERO},..Limits::default()}).with_grouper(two);
        let load=|a:&AtomicUsize|a.load(Ordering::SeqCst);let mut keep=Vec::new();
        // Admits and enlists connection `id` from group `g`, as `listen` and `spawn_peer` do; true in the spare place.
        let mut join=|n:&Net,id:PeerId,g:Group|{let a=n.admit(Some(g)).unwrap();let ip=if g==G{[81,2,0,1]}else{[81,3,0,1]};
            let (p,s)=test_peer_as(id,1<<16,Origin::Inbound,Some(SocketAddr::from((ip,id as u16))));n.enlist(&p,Some(g),a.over);keep.push(s);(p,a.over)};
        let release=|n:&Net,id|drop(Release{net:n.clone(),id,group:Some(G),outbound:false,handshaking:false});
        let n1=net(4,Duration::ZERO);
        let g:Vec<Peer>=(1..=4).map(|id|join(&n1,id,G).0).collect();let (h,_)=join(&n1,5,H);
        assert!((1..=5).all(|id|n1.establish(id)),"within the cap");
        g[1].credit(Useful::Block);g[2].credit(Useful::Template);
        let (x,over)=join(&n1,6,G);assert!(over,"the fifth of the group takes the spare place");
        assert!(n1.establish(6),"and evicts although the set has room");
        assert_eq!(g.iter().map(|p|p.ended().0).collect::<Vec<_>>(),[false,false,false,true],"the youngest free member goes; 1 oldest, 2 block, 3 template stay");
        assert_eq!((h.ended().0,x.ended().0,load(&n1.counters.evicted),load(&n1.counters.evicted_in_group)),(false,false,1,1),"another group is untouched");
        // The victim's release gives its place back; a spare newcomer whose group shrinks before its Hello evicts nobody.
        release(&n1,4);let (y,over)=join(&n1,7,G);assert!(over);release(&n1,3);
        assert!(n1.establish(7),"room in its group again");assert_eq!((load(&n1.counters.evicted),y.ended().0),(1,false));
        // Nothing to evict: the oldest member and the latest block source are kept.
        let n2=net(2,Duration::ZERO);let (o,_)=join(&n2,10,G);let (b,_)=join(&n2,11,G);assert!(n2.establish(10)&&n2.establish(11));
        b.credit(Useful::Block);let (_z,over)=join(&n2,12,G);assert!(over);
        assert!(!n2.establish(12),"nothing left to evict in the group");
        assert_eq!((o.ended().0,b.ended().0,load(&n2.counters.refused_group),load(&n2.counters.refused_full),load(&n2.counters.refused)),(false,false,1,0,1),"refused by group");
        // Grace: a member established within the grace period is never chosen.
        let n3=net(1,Duration::from_secs(60));let (m,_)=join(&n3,20,G);assert!(n3.establish(20));
        let (_w,over)=join(&n3,21,G);assert!(over);assert!(!n3.establish(21),"a member in grace is never chosen");assert!(!m.ended().0);
        // A configured address in the group's /16 shares its key but holds no place in it: its connections past its
        // configured count are never chosen in the group, so the victim frees a place and the spare one works again.
        let n4=net(4,Duration::ZERO);let cfg=SocketAddr::from(([81,2,0,200],1));n4.protect([cfg.ip()]);
        assert_eq!(n4.inbound_group(cfg.ip()),None);
        let m:Vec<Peer>=(30..=33).map(|id|join(&n4,id,G).0).collect();
        let mut socks=Vec::new();let c:Vec<Peer>=(34..=36).map(|id|{assert!(n4.admit(None).is_some());let (p,s)=test_peer_as(id,1<<16,Origin::Inbound,Some(cfg));
            n4.enlist(&p,None,false);socks.push(s);p}).collect();
        assert!((30..=36).all(|id|n4.establish(id)),"within the cap; 34 exempt, 35 and 36 ordinary");
        let (_v,over)=join(&n4,37,G);assert!(over,"the fifth place holder takes the spare place");
        assert!(n4.establish(37));
        assert_eq!((m.iter().map(|p|p.ended().0).collect::<Vec<_>>(),c.iter().map(|p|p.ended().0).collect::<Vec<_>>()),
            (vec![false,false,false,true],vec![false,false,false]),"the group's youngest place holder goes, not the configured address's youngest connection");
        let places=||n4.counters.groups.lock().unwrap().get(&G).copied();
        release(&n4,33);assert_eq!(places(),Some(4),"its release brings the group back to its cap");
        // The node ends each handshake at its Hello (`test_peer` connections hold theirs).
        n4.counters.handshaking.store(0,Ordering::SeqCst);assert_eq!(n4.admit(Some(G)),Some(Admit{over:true}),"and the spare place can be taken again");
    }
    fn key(i:u16)->EvictKey{EvictKey::Net(Group::V4(i.to_be_bytes()))}
    /// A free candidate with no credit; `rank` is its key's lottery hash.
    fn cand(id:PeerId,key:EvictKey,rank:u64)->Cand{Cand{id,key,rank,last:[0;4],free:true,prot:false}}
    fn prot(c:&[Cand])->Vec<PeerId>{let mut v:Vec<PeerId>=c.iter().filter(|x|x.prot).map(|x|x.id).collect();v.sort();v}
    /// No lottery and the minimum per-kind quota, so step 5 shows on its own beside the older half.
    const NO_LOTTERY:Policy=Policy{protect_groups:0,protect_recent:4};
    /// Step 5: the key with the most established members, protected and graced ones included, gives the victim, its
    /// youngest candidate; a tie goes to the key whose youngest candidate is younger, in both orders. A group of 4 with 2
    /// credited (protected) members still loses to an honest group of 3, and credited singletons do not make an idle
    /// honest group of 3 the largest: usefulness buys protection, never a smaller apparent group. No candidate, no victim.
    #[test] fn the_victim_is_the_youngest_of_the_largest_group_by_full_membership(){
        let (a,b,c)=(key(1),key(2),key(3));
        // A {1,3,5}, B {2,4,6}, C {7}: the older half (1, 2, 3) is protected; B's 6 beats A's 5, and C is smaller.
        let mut v=vec![cand(1,a,0),cand(3,a,0),cand(5,a,0),cand(2,b,0),cand(4,b,0),cand(6,b,0),cand(7,c,0)];
        assert_eq!(victim(&mut v,&NO_LOTTERY),Some(6),"the tie goes to B, whose youngest candidate is younger; C is smaller although younger");
        let mut v=vec![cand(2,a,0),cand(4,a,0),cand(6,a,0),cand(1,b,0),cand(3,b,0),cand(5,b,0),cand(7,c,0)];
        assert_eq!(victim(&mut v,&NO_LOTTERY),Some(6),"the other tie order goes to A");
        // Attacker X {1,3,5,7}, 5 and 7 credited; honest H {2,4,6}: X keeps its full size of 4.
        let credited=|mut x:Cand,k:usize,t:u64|{x.last[k]=t;x};
        let (x,h)=(key(10),key(11));
        let mut v=vec![cand(1,x,0),cand(3,x,0),credited(cand(5,x,0),0,50),credited(cand(7,x,0),0,70),cand(2,h,0),cand(4,h,0),cand(6,h,0)];
        assert_eq!(victim(&mut v,&NO_LOTTERY),Some(3),"the attacker group of 4 loses its youngest unprotected member");
        assert_eq!(prot(&v),vec![1,2,5,7],"its credited members and the older half are protected");
        // X {2,4,6,8}, 8 credited; idle honest H {1,3,5}; credited singletons 9, 10, 11 (each its own key).
        let mut v=vec![cand(2,x,0),cand(4,x,0),cand(6,x,0),credited(cand(8,x,0),0,80),cand(1,h,0),cand(3,h,0),cand(5,h,0),
            credited(cand(9,key(20),0),2,90),credited(cand(10,key(21),0),2,91),credited(cand(11,key(22),0),2,92)];
        assert_eq!(victim(&mut v,&NO_LOTTERY),Some(6),"the idle honest group of 3 is not the largest");
        assert_eq!(victim(&mut [],&Policy{protect_groups:4,protect_recent:4}),None,"no connection, no victim");
        let mut all=vec![credited(cand(1,a,0),1,5),credited(cand(2,a,0),1,6)];
        assert_eq!(victim(&mut all,&NO_LOTTERY),None,"every connection protected, no victim");
        let mut one=vec![cand(1,a,0)];assert_eq!(victim(&mut one,&Policy{protect_groups:4,protect_recent:4}),None,"a lone connection wins the lottery");
    }
    /// Steps 1-3 on 24 candidates in 6 keys of 4: the lottery protects the oldest member of each of the protect_groups keys
    /// with the highest rank; each kind protects its protect_recent latest deliverers, Share credits never take Template
    /// places, and a kind never credited protects nobody; then the older half of what is left, rounded down. The quota
    /// is max(4, peers_max/16).
    #[test] fn lottery_recent_per_kind_and_older_half_are_protected(){
        // Key j (0..6) holds ids j+1, j+7, j+13, j+19 and ranks j·10: the lottery favours keys 5, 4, 3, 2.
        let set=||->Vec<Cand>{(0..24u64).map(|i|cand(i+1,key((i%6) as u16),(i%6)*10)).collect()};
        let mut c=set();lottery(&mut c,4);assert_eq!(prot(&c),vec![3,4,5,6],"the oldest of each of the four highest-ranked keys");
        let mut c=set();lottery(&mut c,0);assert!(prot(&c).is_empty());
        // Templates from 1..=6 (the latest 3, 4, 5, 6), shares from 7..=20, blocks from 21 and 22 only, no transactions.
        let credited=||{let mut c=set();for x in c.iter_mut(){
            if x.id<=6{x.last[Useful::Template as usize]=x.id;}else if x.id<=20{x.last[Useful::Share as usize]=100+x.id;}else if x.id<=22{x.last[Useful::Block as usize]=x.id;}}c};
        let mut c=credited();recent(&mut c,4);
        assert_eq!(prot(&c),vec![3,4,5,6,17,18,19,20,21,22],"4 latest templates, 4 latest shares, the 2 blocks, no transaction");
        // A connection already protected does not use up a kind's quota: with templates' 3..=6 protected first, the next
        // latest template deliverers take the quota.
        let mut c=credited();for x in c.iter_mut(){if (3..=6).contains(&x.id){x.prot=true;}}recent(&mut c,4);
        assert_eq!(prot(&c),vec![1,2,3,4,5,6,17,18,19,20,21,22]);
        // The older half: 24 - 12 = 12 candidates left, the oldest 6 of them protected.
        older_half(&mut c);assert_eq!(prot(&c),vec![1,2,3,4,5,6,7,8,9,10,11,12,17,18,19,20,21,22],"6 more: 7..=12");
        let mut odd:Vec<Cand>=(1..=5).map(|i|cand(i,key(1),0)).collect();older_half(&mut odd);assert_eq!(prot(&odd),vec![1,2],"5 candidates: 2 protected");
        // All three steps together, as `victim` runs them, before it picks: with the default quotas the lottery, the
        // kinds and the older half all apply.
        // Left: 13..=16, 23, 24, one in each key of 4; the tie goes to the youngest.
        let mut c=credited();assert_eq!(victim(&mut c,&Policy{protect_groups:4,protect_recent:4}),Some(24),"the youngest left of the largest key");
        assert_eq!(prot(&c),vec![1,2,3,4,5,6,7,8,9,10,11,12,17,18,19,20,21,22],"lottery 3..=6, templates 1, 2, shares and blocks, then half of 12: 7..=12");
        let quota=|p:usize|Policy::of(&Limits{peers_max:p,..Limits::default()}).protect_recent;
        assert_eq!((quota(8),quota(32),quota(64),quota(128)),(4,4,4,8),"max(4, peers_max/16)");
        assert_eq!(Policy::of(&Limits::default()).protect_groups,4);
    }
    /// A connection in its grace period (`free` false) counts in its key's size but is never returned; with only graced
    /// connections left there is no victim. (Exempt connections are not candidates at all: `Net::establish` leaves them
    /// out, see `establish_takes_a_victims_place_or_refuses`.)
    #[test] fn graced_peers_are_counted_but_never_chosen(){
        let (x,h)=(key(1),key(2));let graced=|mut c:Cand|{c.free=false;c};
        let mut v=vec![cand(4,x,0),cand(5,x,0),graced(cand(6,x,0)),graced(cand(7,x,0)),cand(1,h,0),cand(2,h,0),cand(3,h,0)];
        assert_eq!(victim(&mut v,&NO_LOTTERY),Some(5),"X counts its 2 graced members (4 > 3) and loses its youngest free one");
        let mut v=vec![graced(cand(1,x,0)),graced(cand(2,x,0)),graced(cand(3,h,0))];
        assert_eq!(victim(&mut v,&Policy{protect_groups:4,protect_recent:4}),None,"only graced connections left");
    }
    /// Heap's algorithm: every permutation of `v`, each handed to `f`.
    fn permutations(v:&mut Vec<Cand>,k:usize,f:&mut dyn FnMut(&[Cand])){
        if k<=1{f(v);return}
        for i in 0..k{permutations(v,k-1,f);if k%2==0{v.swap(i,k-1);}else{v.swap(0,k-1);}}
    }
    /// Every sort key ends in the unique id, so the victim does not depend on the order of the candidates: all 5,040
    /// orders of 7 mixed candidates (three keys, credits, a graced one) give the same victim and protect the same set.
    #[test] fn the_victim_does_not_depend_on_candidate_order(){
        let mut v=vec![cand(1,key(1),9),cand(2,key(2),5),cand(3,key(1),9),Cand{last:[0,0,7,0],..cand(4,key(3),1)},cand(5,key(2),5),
            Cand{free:false,..cand(6,key(1),9)},cand(7,key(2),5)];
        let p=Policy{protect_groups:1,protect_recent:4};
        let mut first=v.clone();let want=(victim(&mut first,&p),prot(&first));assert!(want.0.is_some());
        let (mut n,mut same)=(0,true);
        permutations(&mut v,7,&mut|c|{let mut c=c.to_vec();let got=(victim(&mut c,&p),prot(&c));n+=1;same&=got==want;});
        assert_eq!((n,same),(5040,true),"the victim {:?} for every order",want.0);
        // `victim_in` on 7 entries of key 1 (and one of key 2): 2 and 3 delivered a block at the same time, so the tie
        // breaks by id; the victim is the youngest left whatever the order.
        let mut w=vec![cand(1,key(1),9),Cand{last:[4,0,0,0],..cand(2,key(1),9)},Cand{last:[4,0,0,0],..cand(3,key(1),9)},Cand{last:[0,6,0,0],..cand(4,key(1),9)},
            cand(5,key(1),9),Cand{free:false,..cand(6,key(1),9)},cand(7,key(2),5)];
        let want=victim_in(&w,key(1));assert_eq!(want,Some(5),"1 oldest, 3 the latest block (by id on a tie), 4 the latest template; 6 graced, 7 another key");
        let (mut n,mut same)=(0,true);
        permutations(&mut w,7,&mut|c|{n+=1;same&=victim_in(c,key(1))==want;});
        assert_eq!((n,same),(5040,true),"victim_in {want:?} for every order");
    }
    /// `Net::establish` on a registry of `test_peer` connections (no threads), established 2 at most, 4 waiting places:
    /// - a newcomer past the cap evicts (the victim's outbox is closed, `evicted` counts it); a connection without its
    ///   Hello, a closed one, an exempt one and the newcomer itself are never chosen, and a closed one is not held;
    /// - with no victim the newcomer is refused (`refused_full`), and with the key's budget spent too (`refused_rate`);
    /// - a configured address's connections are exempt only up to its count (`protect`), the rest are ordinary; an exempt
    ///   newcomer is not charged and exceeds the cap only within peers_max + min(configured peers, handshaking_max/2).
    #[test] fn establish_takes_a_victims_place_or_refuses(){
        let free=EvictLimits{grace:Duration::ZERO,key_interval:Duration::ZERO,burst:1000,refill:Duration::ZERO};
        let net=|peers_max,evict|Net::new(Limits{peers_max,handshaking_max:4,evict,..Limits::default()});
        let load=|a:&AtomicUsize|a.load(Ordering::SeqCst);
        let mut keep=Vec::new();
        let mut enlist=|n:&Net,id:PeerId,remote:Option<SocketAddr>|{let (p,s)=test_peer_as(id,1<<16,Origin::Inbound,remote);n.enlist(&p,None,false);keep.push(s);p};
        // Eviction: a, b held; x never sends its Hello; newcomer n evicts b (a wins the lottery of the one key).
        let n1=net(2,free.clone());
        let (a,b,x,n)=(enlist(&n1,1,None),enlist(&n1,2,None),enlist(&n1,3,None),enlist(&n1,4,None));
        assert!(n1.establish(1)&&n1.establish(2),"within the cap");assert_eq!(n1.established(),2);
        assert!(n1.establish(4),"the newcomer takes the victim's place");
        assert_eq!((a.ended().0,b.ended().0,x.ended().0,n.ended().0),(false,true,false,false),"b, the youngest held, is evicted; never x, n or the lottery's a");
        assert_eq!((load(&n1.counters.evicted),n1.established()),(1,2));
        // A closed connection that is not yet released is not held: the next newcomer finds a place without evicting.
        a.close();let m=enlist(&n1,5,None);assert!(n1.establish(5));
        assert_eq!((load(&n1.counters.evicted),n1.established(),m.ended().0),(1,2,false),"closed a is not held");
        n1.forget(2);n1.forget(1);assert_eq!(n1.registry().slots.len(),3,"released connections leave the registry");
        // No victim: a lone other connection wins the lottery.
        let n2=net(1,free.clone());let (o,_)=(enlist(&n2,10,None),enlist(&n2,11,None));
        assert!(n2.establish(10));assert!(!n2.establish(11),"no victim, the newcomer is refused");
        assert_eq!((load(&n2.counters.refused_full),load(&n2.counters.refused),load(&n2.counters.evicted),o.ended().0),(1,1,0,false));
        // The budget: one key evicts once per key_interval.
        let n3=net(2,EvictLimits{key_interval:Duration::from_secs(60),..free.clone()});for id in 20..24{enlist(&n3,id,None);}
        assert!(n3.establish(20)&&n3.establish(21)&&n3.establish(22),"the first eviction");
        assert!(!n3.establish(23),"the second within key_interval is refused");
        assert_eq!((load(&n3.counters.evicted),load(&n3.counters.refused_rate),load(&n3.counters.refused)),(1,1,1));
        // Exemption: a configured address with 4 configured peers, 4 waiting places: at most 2 over the cap of 2.
        let ip:IpAddr="81.2.69.8".parse().unwrap();let at=|port|Some(SocketAddr::new(ip,port));
        let n4=net(2,EvictLimits{key_interval:Duration::from_secs(60),..free.clone()});n4.protect([ip;4]);
        let (o1,o2)=(enlist(&n4,30,None),enlist(&n4,31,None));assert!(n4.establish(30)&&n4.establish(31));
        let e:Vec<Peer>=(32..37).map(|id|enlist(&n4,id,at(id as u16))).collect();
        assert!(n4.establish(32),"an exempt newcomer evicts");assert_eq!((o1.ended().0,o2.ended().0),(false,true));
        assert_eq!(n4.registry().tat,0,"and is not charged");
        assert!(n4.establish(33)&&n4.establish(34),"exempt newcomers with no victim exceed the cap by min(4, 4/2)");assert_eq!(n4.established(),4);
        assert!(!n4.establish(35),"the fourth exempt newcomer finds no room within the over-cap bound");
        assert_eq!(load(&n4.counters.refused_full),1);
        // A configured address with 2 configured peers: its first 2 connections are exempt, the rest ordinary candidates.
        let n5=net(4,free.clone());n5.protect([ip;2]);
        let f:Vec<Peer>=(40..44).map(|id|enlist(&n5,id,at(id as u16))).collect();
        assert!((40..44).all(|id|n5.establish(id)),"within the cap");
        assert_eq!(n5.registry().slots.iter().map(|s|s.exempt).collect::<Vec<_>>(),[true,true,false,false],"exactly protect's count is exempt");
        enlist(&n5,45,None);assert!(n5.establish(45));
        assert_eq!(f.iter().map(|p|p.ended().0).collect::<Vec<_>>(),[false,false,false,true],"the youngest ordinary connection from the configured address goes");
        let n6=net(2,free);n6.protect([ip;2]);for id in 50..53{enlist(&n6,id,at(id as u16));}
        assert!(n6.establish(50)&&n6.establish(51));assert!(!n6.establish(52),"exempt connections are never victims");
        drop(e);
    }
    /// Only a Hello the node would keep is established (and so may evict): with `expect_hello` unset, every Hello; then
    /// exactly the Hellos `identity::accepts` takes (the node's own close decision), less a nonzero nonce that is the
    /// node's own (a self-connection). Nonce 0 (a node that does not listen) is eligible. T15: a table of every field
    /// that can differ, then seeded random Hellos drawn mostly from near-misses.
    #[test] fn eligible_matches_accepts(){
        use crate::identity::{accepts,Consensus,PROTOCOL_VERSION as V,PROTOCOL_VERSION_MIN as MIN};
        let ours=Consensus::new(7,[1;32],&[(1,0),(2,100)],1,0,None);let other=Consensus::new(7,[1;32],&[(1,0)],1,0,None);
        let hello=|chain_id,genesis,protocol,compatibility,nonce|Msg::Hello{chain_id,genesis,head:[0;32],height:0,work:[0;32],node:"n".into(),protocol,compatibility,listen_port:0,nonce};
        let n=Net::new(Limits::default());assert!(n.eligible(&hello(8,[9;32],0,[0;32],5)),"unset: every Hello");
        n.expect_hello(HelloId{consensus:ours.clone(),nonce:42});
        let rule=|m:&Msg|{let Msg::Hello{chain_id,genesis,protocol,compatibility,nonce,..}=m else{return false};
            !(*nonce!=0&&*nonce==42)&&accepts(&ours,*chain_id,genesis,*protocol,compatibility).is_ok()};
        let table=[(hello(7,[1;32],V,ours.id,5),true,"this network and identity"),(hello(7,[1;32],V,ours.id,0),true,"nonce 0"),
            (hello(7,[1;32],V+1,ours.id,5),true,"a newer maximum wire version"),(hello(7,[1;32],MIN,ours.id,5),true,"the minimum wire version"),
            (hello(8,[1;32],V,ours.id,5),false,"chain"),(hello(7,[2;32],V,ours.id,5),false,"genesis"),(hello(7,[1;32],MIN-1,ours.id,5),false,"wire below MIN"),
            (hello(7,[1;32],V,other.id,5),false,"another schedule"),(hello(7,[1;32],V,[0;32],5),false,"another identity"),(hello(7,[1;32],V,ours.id,42),false,"own nonce")];
        for (m,want,why) in &table{assert_eq!(n.eligible(m),*want,"{why}");assert_eq!(n.eligible(m),rule(m),"{why}");}
        assert!(!n.eligible(&Msg::GetAddr),"not a Hello");
        let mut x=0x9e37_79b9_7f4a_7c15u64;let mut next=||{x^=x<<13;x^=x>>7;x^=x<<17;x};
        for _ in 0..2000{
            let r=next();let b=|bit:u64|r>>bit&1==0;
            let id=if b(2){ours.id}else if b(3){other.id}else{[r as u8;32]};let nonce=if b(4){r>>16}else if b(5){42}else{0};
            let m=hello(if b(0){7}else{8},if b(1){[1;32]}else{[2;32]},[V,MIN,MIN-1,V+1,0,u64::MAX][(r>>8) as usize%6],id,nonce);
            assert_eq!(n.eligible(&m),rule(&m),"{m:?}");
        }
    }
    /// The eviction budget: the GCRA allows `burst` evictions at once and then one per `refill`; one key evicts at most
    /// once per `key_interval`; the ring keeps the latest EVICT_RING charges and wraps; a charge stamped after `now`
    /// counts as within the interval.
    #[test] fn the_eviction_budget_limits_per_key_and_globally(){
        let e=EvictLimits{grace:Duration::ZERO,key_interval:Duration::from_secs(30),burst:3,refill:Duration::from_secs(2)};
        let mut r=Registry::new(4);
        for i in 0..3{assert!(r.allows(key(i),1000,&e),"burst {i}");r.charge(key(i),1000,&e);}
        assert!(!r.allows(key(3),1000,&e),"the burst is spent");assert!(!r.allows(key(3),2999,&e));
        assert!(r.allows(key(3),3000,&e),"one more after refill");r.charge(key(3),3000,&e);
        assert!(!r.allows(key(4),3000,&e)&&!r.allows(key(4),4999,&e)&&r.allows(key(4),5000,&e),"then one per refill");
        assert!(!r.allows(key(0),30_999,&e),"key 0 evicted at 1000: not again within key_interval");assert!(r.allows(key(0),31_000,&e));
        assert!(!r.allows(key(0),500,&e),"a charge after now saturates to within the interval");
        let free=EvictLimits{key_interval:Duration::from_secs(3600),burst:1000,refill:Duration::ZERO,..e};let mut r=Registry::new(4);
        for i in 0..EVICT_RING as u16{r.charge(key(i),10,&free);}assert_eq!(r.ring_next,0,"the ring wraps");
        assert!(!r.allows(key(0),20,&free));r.charge(key(999),20,&free);
        assert!(r.allows(key(0),20,&free),"the oldest charge is overwritten");assert!(!r.allows(key(1),20,&free)&&!r.allows(key(999),20,&free));
        assert_eq!(r.ring_next,1);
    }
    /// Eviction keys: fc00::/7 (cjdns, ULA overlays) is one key, other ungrouped IPv6 is keyed by its /32, a public
    /// address by its group, and ungrouped IPv4 by its exact address (a mapped one as IPv4); no address is ::.
    #[test] fn evict_keys_lump_overlay_ipv6(){
        let k=|s:&str|evict_key(group_of,Some(s.parse().unwrap()));
        assert_eq!((k("fc00::1"),k("fc12:3456::9"),k("fd00::1"),k("fdff:1::2")),(EvictKey::Ula,EvictKey::Ula,EvictKey::Ula,EvictKey::Ula),"all of fc00::/7 is one key");
        assert_eq!((k("::1"),k("fe80::1"),k("fe80::2")),(EvictKey::V6Local([0;4]),EvictKey::V6Local([0xfe,0x80,0,0]),EvictKey::V6Local([0xfe,0x80,0,0])),"other ungrouped IPv6 by /32");
        assert_eq!(k("8.8.4.4"),EvictKey::Net(Group::V4([8,8])),"a public address by its /16");
        assert_eq!(k("2606:4700::1"),EvictKey::Net(Group::V6([0x26,0x06,0x47,0x00])));
        assert_ne!(k("10.0.0.1"),k("10.0.0.2"),"private IPv4 hosts are separate keys");
        assert_eq!(k("::ffff:10.0.0.1"),EvictKey::Host("10.0.0.1".parse().unwrap()),"a mapped address as IPv4");
        assert_eq!(evict_key(group_of,None),EvictKey::Host("::".parse().unwrap()));
    }
    /// A reader that panics still gives back what its connection held: dropping its `Release` frees the registry slot, the
    /// live place, the handshake place and the group place, once.
    #[test] fn a_panicking_reader_still_releases_its_places(){
        const G:Group=Group::V4([81,2]);
        let net=Net::new(Limits::default()).with_grouper(|_|Some(G));let c=net.counters.clone();
        assert!(net.admit(Some(G)).is_some());let (p,_s)=test_peer(77,1<<16);net.enlist(&p,Some(G),false);
        let release=Release{net:net.clone(),id:77,group:Some(G),outbound:false,handshaking:true};
        assert!(std::thread::spawn(move||{let _held=release;panic!("a reader bug")}).join().is_err());
        let load=|a:&AtomicUsize|a.load(Ordering::SeqCst);
        assert_eq!((load(&c.live),load(&c.handshaking),c.groups.lock().unwrap().get(&G).copied(),net.registry().slots.len()),(0,0,None,0));
    }
    /// U10: a configured peer whose connections end quickly is redialed after 0.6 s, 1.2 s, ... at most 30 s; a connection
    /// that lasted DIAL_STABLE resets the wait to 300 ms; a failed connect keeps it.
    #[test] fn redial_delay_doubles_after_short_connections_and_resets_after_a_stable_one(){
        let ms=Duration::from_millis;let short=Some(Duration::ZERO);
        let mut d=DIAL_RETRY;let mut seen=vec![];for _ in 0..10{d=redial_delay(d,short);seen.push(d.as_millis());}
        assert_eq!(seen,[600,1200,2400,4800,9600,19200,30000,30000,30000,30000],"doubling from 300 ms, capped at 30 s");
        assert_eq!(redial_delay(ms(1200),Some(DIAL_STABLE-ms(1))),ms(2400),"just short of DIAL_STABLE is short");
        assert_eq!(redial_delay(DIAL_BACKOFF_MAX,Some(DIAL_STABLE)),DIAL_RETRY,"a stable connection resets the wait");
        assert_eq!((redial_delay(ms(4800),None),redial_delay(DIAL_RETRY,None)),(ms(4800),DIAL_RETRY),"a failed connect keeps the wait");
        assert_eq!(redial_delay(Duration::MAX,short),DIAL_BACKOFF_MAX,"no overflow");
        assert_eq!((DIAL_RETRY,DIAL_STABLE,DIAL_BACKOFF_MAX),(ms(300),Duration::from_secs(60),Duration::from_secs(30)));
    }
    /// Wire golden vectors (plan §1.6, VA-SPEC §2.2): now that a net.rs change no longer partitions the network, the
    /// frames same-rules nodes exchange are pinned. Each canonical message encodes to its pinned frame and decodes
    /// back to itself, `v2/limits` pins MAX_FRAME, the Hello arity and the decoders' bounds, and the file holds exactly
    /// these keys for every accepted version. The consensus guard keeps a pinned key frozen until PROTOCOL_VERSION_MIN
    /// passes it, and lets a new key in only at a new PROTOCOL_VERSION.
    mod wire_golden{
        use super::*;
        use crate::identity::{PROTOCOL_VERSION,PROTOCOL_VERSION_MIN};
        const FILE:&str=include_str!("../tests/data/wire-golden.json");
        /// One canonical message per type 0–9, then the limits; `v` is the wire version that introduced the frame.
        fn canonical()->Vec<(&'static str,u64,Vec<u8>,Option<Msg>)>{
            let msgs=[
                ("v2/hello",Msg::Hello{chain_id:1599846715,genesis:[0x11;32],head:[0x22;32],height:15000,work:{let mut w=[0;32];w[30]=0x01;w[31]=0x2a;w},node:"pocol-golden".into(),
                    protocol:2,compatibility:[0x44;32],listen_port:30333,nonce:0x0102030405060708}),
                ("v2/block",Msg::Block{header:vec![0xc1,0x80],body:vec![0xc0]}),
                ("v2/template",Msg::Template{shell:vec![1,2,3],body:vec![]}),
                ("v2/share",Msg::Share{tid:[0x55;32],nonce:0xfedcba9876543210}),
                ("v2/get-block",Msg::GetBlock{hash:[0x66;32]}),
                ("v2/get-range",Msg::GetRange{start:15000,count:512}),
                ("v2/tx",Msg::Tx{raw:vec![0x02,0xf8,0x01]}),
                ("v2/get-from",Msg::GetFrom{locator:vec![[0x77;32],[0x88;32]],count:512}),
                ("v2/get-addr",Msg::GetAddr),
                ("v2/addr",Msg::Addr{addrs:vec!["203.0.113.7:30333".parse().unwrap(),"[2001:db8::1]:30334".parse().unwrap()]}),
            ];
            let arity=rlp::decode(&msgs[0].1.encode()).unwrap().list().unwrap().len() as u64;
            let mut p=Vec::new();for v in [MAX_FRAME as u64,arity,HELLO_MAX as u64,NODE_NAME_MAX as u64,LOCATOR_MAX as u64,ADDR_RECORD as u64,ADDR_MAX as u64]{u64b(v,&mut p)}
            let mut limits=Vec::new();rlp::encode_list_payload(&p,&mut limits);
            let mut out:Vec<_>=msgs.into_iter().map(|(k,m)|(k,2,m.encode(),Some(m))).collect();
            out.push(("v2/limits",2,limits,None));out
        }
        fn hex(b:&[u8])->String{b.iter().map(|x|format!("{x:02x}")).collect()}
        #[test] fn every_message_type_encodes_to_its_pinned_frame_and_decodes_back_to_itself(){
            let c=canonical();
            let want=format!("[\n{}\n]\n",c.iter().map(|(k,v,b,_)|format!(" {{\"key\":\"{k}\",\"v\":{v},\"hex\":\"{}\"}}",hex(b))).collect::<Vec<_>>().join(",\n"));
            let file:Vec<serde_json::Value>=serde_json::from_str(FILE).expect("wire-golden.json");
            let accepted:Vec<&serde_json::Value>=file.iter().filter(|e|e["v"].as_u64().is_some_and(|v|v>=PROTOCOL_VERSION_MIN)).collect();
            let keys:Vec<&str>=accepted.iter().map(|e|e["key"].as_str().unwrap()).collect();
            assert_eq!(keys,c.iter().map(|x|x.0).collect::<Vec<_>>(),"the file holds exactly the canonical keys of every accepted version; the encoding of this tree is:\n{want}");
            for ((k,v,bytes,msg),e) in c.iter().zip(accepted){
                assert!(*v>=PROTOCOL_VERSION_MIN&&*v<=PROTOCOL_VERSION&&e["v"]==*v,"{k}: v");
                assert_eq!(e["hex"].as_str().unwrap(),hex(bytes),"{k} no longer encodes to its pinned frame (old decoders are strict; see the consensus guard's wire rules). This tree encodes:\n{want}");
                if let Some(m)=msg{assert_eq!(Msg::decode(bytes).as_ref(),Ok(m),"{k} decodes back to itself")}
            }
            let types:Vec<u64>=c.iter().filter(|x|x.3.is_some()).map(|x|{let it=rlp::decode(&x.2).unwrap();let t=it.list().unwrap()[0].bytes().unwrap().iter().fold(0u64,|a,b|(a<<8)|*b as u64);t}).collect();
            assert_eq!(types,(0..=9).collect::<Vec<_>>(),"one canonical message per Msg type");
        }
    }
}
