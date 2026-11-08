//! Peer discovery's address book (VA-SPEC §1a.3-§1a.6): local policy, never validity. The module is pure: the clock
//! (`now`, milliseconds since the epoch) and the random generator (`Rng`) are passed in, and every scan runs over the
//! entries in order (at most BOOK_MAX, reserved once; a linear scan is cheap at that size), so a fixed seed gives a
//! fixed result. The node keeps one book (`node::discovery`); `parse` is also the fuzz entry point.
//! - An entry's key is its diversity group (`Key`): its address group (`net::group_of`), or in local mode its own
//!   endpoint. `learnable` decides what may be stored at all.
//! - Quotas: GROUP_MAX entries per key, IP_PORTS_MAX ports per IP, SOURCE_MAX unverified entries per answering peer
//!   and SELFADS_MAX self-advertisements. A newcomer takes the place of a terrible entry only, except that an
//!   answering peer at its quota replaces its own oldest entry and a verified newcomer an unverified one; a verified
//!   entry is never replaced. No volume of hearsay or self-advertisements, from any number of sources, therefore
//!   displaces another source's entry or one this node verified (Heilman et al., USENIX Security 2015,
//!   countermeasure 1: overwrite only terrible entries), and no test-before-evict is needed.
//! - Answers carry verified and self-advertised entries only, never hearsay, so an address nobody verified travels
//!   one hop at most; no timestamp leaves the node.
use std::net::{IpAddr,SocketAddr};
use crate::net::{self,Group};

/// Entries in one book (VA-SPEC §1a.4), reserved at once and never grown.
pub const BOOK_MAX:usize=1024;
/// Entries per key.
pub const GROUP_MAX:usize=8;
/// Ports per IP address, for keys that are address groups (in local mode every node may share 127.0.0.1).
pub const IP_PORTS_MAX:usize=2;
/// Unverified entries per answering peer (`Source::Peer`, `Source::Cfg`), and self-advertised ones together
/// (`Source::Inbound`). Verified entries count toward neither.
pub const SOURCE_MAX:usize=64;pub const SELFADS_MAX:usize=256;
/// Entries per key in the answer list, and its length (VA-SPEC §1a.5; one answer is at most `net::ADDR_MAX`).
pub const ANSWER_PER_KEY:usize=2;pub const CACHE_MAX:usize=128;
/// This node's own addresses, as self-connections find them (VA-SPEC §2): in memory only, the oldest dropped first.
pub const SELF_MAX:usize=16;
/// Anchors in the file (VA-SPEC §1a.7: automatic outbound writes them).
pub const ANCHORS_MAX:usize=2;
/// The lowest port learned (VA-SPEC §1a.3); configured addresses are exempt.
pub const LEARN_PORT_MIN:u16=1024;
/// The longest line `encode` writes, its newline included.
pub const FILE_LINE_MAX:usize=256;
const DAY_MS:u64=86_400_000;

/// An entry's diversity group (VA-SPEC §1a.4): its address group, or in local mode its own endpoint, so that loopback
/// and LAN nodes sharing one IP are still told apart. Ordered, for a deterministic iteration over keys.
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash,PartialOrd,Ord)]
pub enum Key{Net(Group),Local(SocketAddr)}
/// Who told this node about an entry: the answering peer, by its key (an automatic outbound peer) or by its exact
/// address (a configured peer: each seed is its own source although the seeds share one address group); `Inbound`
/// for every self-advertisement together, so that many inbound connections are still one source. An entry this node
/// verified before anyone named it is its own source (`Peer` of its key).
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash,PartialOrd,Ord)]
pub enum Source{Peer(Key),Cfg(SocketAddr),Inbound}
/// How an unverified entry was learned: heard in a peer's Addr, or advertised by the address itself (the first Hello
/// of an inbound connection).
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Kind{Heard,Direct}
/// What an insertion did. A newcomer is dropped when it is not learnable (`Filter`) or when its key (`Group`), its
/// IP's ports (`Ports`), its source's quota (`Source`) or the book (`Full`) is full and holds nothing it may replace.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Outcome{Added,Refreshed,Replaced,Dropped(Why)}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Why{Filter,Group,Ports,Source,Full}
/// One address. Times are milliseconds since the epoch, 0 for never: `heard_ms` when a peer's Addr last named it,
/// `direct_ms` when it last advertised itself, `verified_ms` the last handshake this node dialed to it with its
/// consent (listenPort != 0); `last_try_ms` the last dial, `last_count_ms` the last counted failure, `hold_until_ms`
/// the end of a hold for another release. `attempts` counts failures (a verification clears it), `holds` the holds.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Entry{pub addr:SocketAddr,pub key:Key,pub source:Source,pub heard_ms:u64,pub direct_ms:u64,pub verified_ms:u64,
    pub last_try_ms:u64,pub last_count_ms:u64,pub hold_until_ms:u64,pub attempts:u8,pub holds:u8}
/// The book's timings (VA-SPEC §1a.4, §1a.5). The node takes them from its `DiscoveryTimes`, which only tests change.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Times{pub relay_window_ms:u64,pub horizon_ms:u64,pub fail_recent_ms:u64,pub hold_base_ms:u64,pub hold_max_ms:u64,pub cache_ms:u64}
impl Default for Times{
    fn default()->Self{Self{relay_window_ms:7*DAY_MS,horizon_ms:30*DAY_MS,fail_recent_ms:7*DAY_MS,hold_base_ms:600_000,hold_max_ms:6*3_600_000,cache_ms:60_000}}
}
/// xorshift64* (Vigna, "An experimental exploration of Marsaglia's xorshift generators, scrambled", 2016): the book's
/// shuffles. The node seeds it once per process from the OS; tests fix the seed. xorshift never leaves 0, so seed 0
/// maps to 0x9E3779B97F4A7C15.
#[derive(Clone,Debug)]
pub struct Rng(u64);
impl Rng{
    pub fn new(seed:u64)->Self{Self(if seed==0{0x9E37_79B9_7F4A_7C15}else{seed})}
    pub fn next_u64(&mut self)->u64{let mut x=self.0;x^=x>>12;x^=x<<25;x^=x>>27;self.0=x;x.wrapping_mul(0x2545_F491_4F6C_DD1D)}
    /// Uniform in 0..n (0 for n = 0), by rejection: the 2^64 mod n lowest outputs are drawn again, so that every
    /// residue has as many outputs as the others.
    pub fn below(&mut self,n:u64)->u64{if n==0{return 0}let low=n.wrapping_neg()%n;loop{let x=self.next_u64();if x>=low{return x%n}}}
    /// A uniform permutation (Fisher-Yates).
    pub fn shuffle<T>(&mut self,v:&mut [T]){for i in (1..v.len()).rev(){let j=self.below(i as u64+1) as usize;v.swap(i,j);}}
}

fn canonical(a:SocketAddr)->SocketAddr{SocketAddr::new(a.ip().to_canonical(),a.port())}
fn canonical_source(s:Source)->Source{match s{Source::Cfg(a)=>Source::Cfg(canonical(a)),Source::Peer(Key::Local(a))=>Source::Peer(Key::Local(canonical(a))),s=>s}}
/// The latest time anything was heard from, advertised by or verified at an entry.
fn freshness(e:&Entry)->u64{e.heard_ms.max(e.direct_ms).max(e.verified_ms)}
/// The addresses local mode may store: loopback, private (10/8, 172.16/12, 192.168/16), shared 100.64/10 and
/// unique-local fc00::/7. Link-local, unspecified, multicast, broadcast and reserved ranges are never stored.
fn local_ip(ip:IpAddr)->bool{
    match ip{IpAddr::V4(a)=>{let o=a.octets();a.is_loopback()||a.is_private()||(o[0]==100&&(o[1]&0xc0)==64)}IpAddr::V6(a)=>a.is_loopback()||(a.segments()[0]&0xfe00)==0xfc00}
}

/// The address book. `last_good_ms`: the last handshake this node dialed that verified an address (it starts at 1, so
/// that an entry's first failure counts); `selfs`: this node's own addresses; `cache`: the answer list, built at
/// `cache_ms`; `dirty`: changed since it was loaded or saved.
#[derive(Debug)]
pub struct Book{on:bool,allow_local:bool,times:Times,entries:Vec<Entry>,last_good_ms:u64,selfs:Vec<SocketAddr>,cache:Vec<SocketAddr>,cache_ms:Option<u64>,dirty:bool}
impl Book{
    /// An empty book that reserves its BOOK_MAX entries now; `allow_local`: local mode (`--discover-local`).
    pub fn new(allow_local:bool)->Self{
        Self{on:true,allow_local,times:Times::default(),entries:Vec::with_capacity(BOOK_MAX),last_good_ms:1,selfs:Vec::new(),cache:Vec::new(),cache_ms:None,dirty:false}
    }
    /// Discovery off: the book reserves nothing, stores nothing and answers nothing.
    pub fn disabled()->Self{
        Self{on:false,allow_local:false,times:Times::default(),entries:Vec::new(),last_good_ms:1,selfs:Vec::new(),cache:Vec::new(),cache_ms:None,dirty:false}
    }
    pub fn set_times(&mut self,t:Times){self.times=t}
    pub fn entries(&self)->&[Entry]{&self.entries}
    pub fn get(&self,a:SocketAddr)->Option<&Entry>{let a=canonical(a);self.entries.iter().find(|e|e.addr==a)}
    pub fn selfs(&self)->&[SocketAddr]{&self.selfs}
    pub fn last_good_ms(&self)->u64{self.last_good_ms}
    pub fn dirty(&self)->bool{self.dirty}
    /// The book was written (`encode`) and the write succeeded.
    pub fn saved(&mut self){self.dirty=false}

    /// VA-SPEC §1a.3: the canonical address (IPv4 for a mapped one) and its key, or None when it may not be stored: a
    /// port below LEARN_PORT_MIN unless `configured` (a configured peer's own address) or port 0 (an Addr record cannot
    /// carry it); IPv6 outside 2000::/3 (NAT64, SIIT and 200::/7 carry an address group but are not learned); an
    /// address without an address group unless the book is in local mode, `from_local` (it came from a source that is
    /// not globally reachable either: for a self-advertisement the address itself, for hearsay the answering peer's
    /// dialed address) and it is a local one (`local_ip`); one of this node's own addresses.
    pub fn learnable(&self,a:SocketAddr,from_local:bool,configured:bool)->Option<(SocketAddr,Key)>{
        let a=canonical(a);
        if a.port()==0||(a.port()<LEARN_PORT_MIN&&!configured)||self.selfs.contains(&a){return None}
        let key=match net::group_of(a.ip()){
            Some(g)=>{if let IpAddr::V6(v6)=a.ip(){if (v6.segments()[0]&0xe000)!=0x2000{return None}}Key::Net(g)}
            None if self.allow_local&&from_local&&local_ip(a.ip())=>Key::Local(a),
            None=>return None,
        };
        Some((a,key))
    }
    /// An unverified newcomer (VA-SPEC §1a.2): heard in the Addr that answered this node's GetAddr (`Kind::Heard`, from
    /// that peer) or advertised in an inbound connection's first Hello (`Kind::Direct`, `Source::Inbound`). An entry
    /// already in the book is only refreshed; its source does not change.
    pub fn insert_unverified(&mut self,a:SocketAddr,src:Source,kind:Kind,from_local:bool,now:u64)->Outcome{
        if !self.on{return Outcome::Dropped(Why::Full)}
        let Some((a,key))=self.learnable(a,from_local,false) else{return Outcome::Dropped(Why::Filter)};
        if let Some(e)=self.entries.iter_mut().find(|e|e.addr==a){match kind{Kind::Heard=>e.heard_ms=now,Kind::Direct=>e.direct_ms=now}self.dirty=true;return Outcome::Refreshed}
        let (heard_ms,direct_ms)=match kind{Kind::Heard=>(now,0),Kind::Direct=>(0,now)};
        self.place(Entry{addr:a,key,source:canonical_source(src),heard_ms,direct_ms,verified_ms:0,last_try_ms:0,last_count_ms:0,hold_until_ms:0,attempts:0,holds:0},now)
    }
    /// A handshake this node dialed, with consent (VA-SPEC §1a.2b: listenPort != 0). The address is verified, its
    /// failures and hold are cleared, and `last_good_ms` moves (also when the address may not be stored: the handshake
    /// still shows this node is online), so that failures count again. An entry already in the book keeps its source;
    /// a new one is its own source (`Source::Peer` of its key). Verified entries count toward no source quota.
    pub fn good(&mut self,a:SocketAddr,configured:bool,from_local:bool,now:u64)->Outcome{
        if !self.on{return Outcome::Dropped(Why::Full)}
        self.last_good_ms=now;self.dirty=true;
        let Some((a,key))=self.learnable(a,from_local,configured) else{return Outcome::Dropped(Why::Filter)};
        if let Some(e)=self.entries.iter_mut().find(|e|e.addr==a){e.verified_ms=now;e.last_try_ms=e.last_try_ms.min(now);e.attempts=0;e.hold_until_ms=0;return Outcome::Refreshed}
        self.place(Entry{addr:a,key,source:Source::Peer(key),heard_ms:0,direct_ms:0,verified_ms:now,last_try_ms:0,last_count_ms:0,hold_until_ms:0,attempts:0,holds:0},now)
    }
    /// Places a newcomer that is not in the book (VA-SPEC §1a.4). The quotas it may fill: its key (GROUP_MAX), its IP's
    /// ports (IP_PORTS_MAX, address-group keys only), its source (unverified newcomers only: SOURCE_MAX, or SELFADS_MAX
    /// for `Inbound`) and the book (BOOK_MAX). With none full it is added. Otherwise it may only take the place of an
    /// entry inside every full quota (its key, its IP, its source's unverified entries): a terrible one, or for a
    /// verified newcomer an unverified one, terrible first, then the oldest freshness, then the lowest address. An
    /// unverified newcomer whose IP is full is always dropped; one whose source alone is full (`Peer`, `Cfg`) may
    /// replace that source's unverified entry heard longest ago, so a source rotates only its own share. A verified
    /// entry is never replaced.
    fn place(&mut self,n:Entry,now:u64)->Outcome{
        let verified=n.verified_ms>0;
        let (mut keys,mut ips,mut srcs)=(0,0,0);
        for e in &self.entries{keys+=usize::from(e.key==n.key);ips+=usize::from(e.addr.ip()==n.addr.ip());srcs+=usize::from(e.verified_ms==0&&e.source==n.source);}
        let key_full=keys>=GROUP_MAX;let ip_full=matches!(n.key,Key::Net(_))&&ips>=IP_PORTS_MAX;
        let src_full=!verified&&srcs>=if n.source==Source::Inbound{SELFADS_MAX}else{SOURCE_MAX};
        if !(key_full||ip_full||src_full||self.entries.len()>=BOOK_MAX){self.entries.push(n);self.dirty=true;return Outcome::Added}
        let why=if ip_full{Why::Ports}else if key_full{Why::Group}else if src_full{Why::Source}else{Why::Full};
        if ip_full&&!verified{return Outcome::Dropped(why)}
        let inside=|e:&Entry|(!key_full||e.key==n.key)&&(!ip_full||e.addr.ip()==n.addr.ip())&&(!src_full||(e.verified_ms==0&&e.source==n.source));
        let mut victim=self.entries.iter().enumerate().filter(|(_,e)|inside(e))
            .filter_map(|(i,e)|{let t=self.terrible(e,now);(t||(verified&&e.verified_ms==0)).then_some((!t,freshness(e),e.addr,i))}).min().map(|v|v.3);
        if victim.is_none()&&src_full&&!key_full&&n.source!=Source::Inbound{
            victim=self.entries.iter().enumerate().filter(|(_,e)|inside(e)).min_by_key(|(_,e)|(e.heard_ms,e.addr)).map(|(i,_)|i);
        }
        match victim{Some(i)=>{self.entries[i]=n;self.dirty=true;Outcome::Replaced}None=>Outcome::Dropped(why)}
    }
    fn update(&mut self,a:SocketAddr,f:impl FnOnce(&mut Entry)){let a=canonical(a);if let Some(e)=self.entries.iter_mut().find(|e|e.addr==a){f(e);self.dirty=true;}}
    /// A dial starts (automatic outbound).
    pub fn tried(&mut self,a:SocketAddr,now:u64){self.update(a,|e|e.last_try_ms=now)}
    /// A dial failed (refused or timed out). It counts as a failure only if a handshake this node dialed verified an
    /// address since this entry's last counted failure: while this node is offline each entry counts one failure, so
    /// an outage does not make the whole book terrible.
    pub fn failed(&mut self,a:SocketAddr,now:u64){
        let good=self.last_good_ms;self.update(a,|e|{e.last_try_ms=now;if e.last_count_ms<good{e.attempts=e.attempts.saturating_add(1);e.last_count_ms=now;}})
    }
    /// A connection that closed before any Hello, or soon after (a full listener, say): no failure.
    pub fn busy(&mut self,a:SocketAddr,now:u64){self.update(a,|e|e.last_try_ms=now)}
    /// Another release (protocol or compatibility mismatch): the n-th hold lasts min(hold_base · 2^(n-1), hold_max).
    /// No failure: after an upgrade the network is split for a while.
    pub fn held(&mut self,a:SocketAddr,now:u64){
        let t=self.times;
        self.update(a,|e|{e.holds=e.holds.saturating_add(1);let f=1u64.checked_shl(u32::from(e.holds-1)).unwrap_or(u64::MAX);
            e.hold_until_ms=now.saturating_add(t.hold_base_ms.saturating_mul(f).min(t.hold_max_ms));e.last_try_ms=now;})
    }
    /// Another chain or genesis, or a peer this node dialed that withdrew its consent (listenPort 0).
    pub fn remove(&mut self,a:SocketAddr){let a=canonical(a);if let Some(i)=self.entries.iter().position(|e|e.addr==a){self.entries.remove(i);self.dirty=true;}}
    /// This node's own address (a self-connection, VA-SPEC §2): removed, and refused from now on by this process.
    pub fn mark_self(&mut self,a:SocketAddr){
        if !self.on{return}let a=canonical(a);self.remove(a);
        if !self.selfs.contains(&a){if self.selfs.len()==SELF_MAX{self.selfs.remove(0);}self.selfs.push(a);}
    }
    /// A live connection, or one that lived long, refreshes the time it verified the entry (`direct`: the time the
    /// entry advertised itself). Only a time already set moves: an entry becomes verified through `good` alone.
    pub fn touch(&mut self,a:SocketAddr,direct:bool,now:u64){self.update(a,|e|{let t=if direct{&mut e.direct_ms}else{&mut e.verified_ms};if *t!=0{*t=now}})}
    /// VA-SPEC §1a.4: never verified with 3 counted failures; 10 failures and no verification within `fail_recent_ms`;
    /// or nothing heard, advertised or verified within `horizon_ms`. A terrible entry is replaced first and never
    /// answered; `parse` drops it. All time arithmetic saturates.
    pub fn terrible(&self,e:&Entry,now:u64)->bool{
        (e.verified_ms==0&&e.attempts>=3)||(e.attempts>=10&&now.saturating_sub(e.verified_ms)>self.times.fail_recent_ms)||now.saturating_sub(freshness(e))>self.times.horizon_ms
    }
    /// VA-SPEC §1a.5: neither terrible nor held, and verified or self-advertised within `relay_window_ms`.
    pub fn relayable(&self,e:&Entry,now:u64)->bool{!self.terrible(e,now)&&e.hold_until_ms<=now&&(self.recent(e.verified_ms,now)||self.recent(e.direct_ms,now))}
    fn recent(&self,ms:u64,now:u64)->bool{ms!=0&&now.saturating_sub(ms)<=self.times.relay_window_ms}
    /// The answer to an inbound GetAddr (VA-SPEC §1a.5): at most `net::ADDR_MAX` addresses of the answer list, without
    /// the requester's own advertised address (`requester`, compared by endpoint: on loopback every node is 127.0.0.1)
    /// and, unless the requester is itself local, without local entries. The list is rebuilt once it is `cache_ms` old
    /// (or the clock went back), so asking again within that time reveals nothing new.
    pub fn answer(&mut self,requester_local:bool,requester:Option<SocketAddr>,rng:&mut Rng,now:u64)->Vec<SocketAddr>{
        if !self.on{return Vec::new()}
        if self.cache_ms.map_or(true,|t|now<t||now-t>=self.times.cache_ms){self.rebuild(rng,now)}
        let requester=requester.map(canonical);
        self.cache.iter().copied().filter(|a|Some(*a)!=requester&&(requester_local||net::group_of(a.ip()).is_some())).take(net::ADDR_MAX).collect()
    }
    /// The answer list: the relayable entries, those verified within the relay window first and then the others (whose
    /// self-advertisement is recent), each part shuffled, at most ANSWER_PER_KEY per key and CACHE_MAX in all.
    fn rebuild(&mut self,rng:&mut Rng,now:u64){
        let (mut first,mut then)=(Vec::new(),Vec::new());
        for (i,e) in self.entries.iter().enumerate(){if self.relayable(e,now){if self.recent(e.verified_ms,now){first.push(i)}else{then.push(i)}}}
        rng.shuffle(&mut first);rng.shuffle(&mut then);
        self.cache.clear();let mut keys:Vec<Key>=Vec::with_capacity(CACHE_MAX);
        for i in first.into_iter().chain(then){
            if self.cache.len()==CACHE_MAX{break}
            let e=&self.entries[i];if keys.iter().filter(|k|**k==e.key).count()<ANSWER_PER_KEY{keys.push(e.key);self.cache.push(e.addr);}
        }
        self.cache_ms=Some(now);
    }
    /// The file (VA-SPEC §1a.6), plain text:
    /// ```text
    /// pocol-addrbook 1 <chainId> <genesis hash, 64 hex digits>
    /// g <last_good_ms>
    /// e <addr> <source> <heard_ms> <direct_ms> <verified_ms> <last_try_ms> <last_count_ms> <hold_until_ms> <attempts> <holds>
    /// a <addr> <saved_ms>                   (at most ANCHORS_MAX)
    /// end <number of lines before this one> <SHA-256 of every byte before this line, hex>
    /// ```
    /// A source is `i` (Inbound), `c:<addr>` (a configured peer), `p4:<a>.<b>` or `p6:<8 hex digits>` (an answering
    /// peer's address group) or `pl:<addr>` (a local answering peer); keys are not written, `parse` recomputes them. No
    /// line is longer than FILE_LINE_MAX.
    pub fn encode(&self,chain_id:u64,genesis:&[u8;32],anchors:&[(SocketAddr,u64)])->Vec<u8>{
        use std::fmt::Write;
        let anchors=&anchors[..anchors.len().min(ANCHORS_MAX)];
        let mut s=String::with_capacity(FILE_LINE_MAX*(self.entries.len()+anchors.len()+3));
        let _=writeln!(s,"pocol-addrbook 1 {chain_id} {}\ng {}",crate::hex::encode(genesis),self.last_good_ms);
        for e in &self.entries{let _=writeln!(s,"e {} {} {} {} {} {} {} {} {} {}",e.addr,e.source,e.heard_ms,e.direct_ms,e.verified_ms,e.last_try_ms,e.last_count_ms,e.hold_until_ms,e.attempts,e.holds);}
        for (a,ms) in anchors{let _=writeln!(s,"a {} {ms}",canonical(*a));}
        let _=writeln!(s,"end {} {}",2+self.entries.len()+anchors.len(),crate::hex::encode(&crate::hashes::sha256(s.as_bytes())));
        s.into_bytes()
    }
    /// One entry read from the file: learnable again, with the locality its source implies (a configured peer below
    /// LEARN_PORT_MIN is learned again at its next handshake), then placed under the quotas with its stored times and
    /// counts. A terrible entry, and a second line for one address, are dropped.
    fn load(&mut self,a:SocketAddr,source:Source,t:[u64;6],attempts:u8,holds:u8,now:u64){
        let from_local=match source{Source::Inbound=>net::group_of(a.ip()).is_none(),Source::Peer(k)=>matches!(k,Key::Local(_)),Source::Cfg(c)=>net::group_of(c.ip()).is_none()};
        let Some((addr,key))=self.learnable(a,from_local,false) else{return};
        let [heard_ms,direct_ms,verified_ms,last_try_ms,last_count_ms,hold_until_ms]=t;
        let e=Entry{addr,key,source,heard_ms,direct_ms,verified_ms,last_try_ms,last_count_ms,hold_until_ms,attempts,holds};
        if !self.terrible(&e,now)&&!self.entries.iter().any(|x|x.addr==addr){self.place(e,now);}
    }
}
impl std::fmt::Display for Source{
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{
        match self{Source::Inbound=>write!(f,"i"),Source::Cfg(a)=>write!(f,"c:{a}"),Source::Peer(Key::Local(a))=>write!(f,"pl:{a}"),
            Source::Peer(Key::Net(Group::V4([a,b])))=>write!(f,"p4:{a}.{b}"),Source::Peer(Key::Net(Group::V6(g)))=>write!(f,"p6:{:08x}",u32::from_be_bytes(*g))}
    }
}
/// A decimal number of digits only (no sign).
fn num<T:std::str::FromStr>(s:&str)->Option<T>{if !s.is_empty()&&s.bytes().all(|c|c.is_ascii_digit()){s.parse().ok()}else{None}}
/// The N fields of a line, separated by single spaces.
fn fields<const N:usize>(line:&str)->Option<[&str;N]>{let mut f=["";N];let mut it=line.split(' ');for x in &mut f{*x=it.next()?;}if it.next().is_some(){None}else{Some(f)}}
fn source_of(s:&str)->Option<Source>{
    if s=="i"{return Some(Source::Inbound)}
    let (tag,v)=s.split_once(':')?;let addr=||v.parse::<SocketAddr>().ok().map(canonical);
    Some(match tag{"c"=>Source::Cfg(addr()?),"pl"=>Source::Peer(Key::Local(addr()?)),
        "p4"=>{let (a,b)=v.split_once('.')?;Source::Peer(Key::Net(Group::V4([num(a)?,num(b)?])))}
        "p6" if v.len()==8&&v.bytes().all(|c|c.is_ascii_hexdigit())=>Source::Peer(Key::Net(Group::V6(u32::from_str_radix(v,16).ok()?.to_be_bytes()))),
        _=>return None})
}
/// Reads a file `encode` wrote (VA-SPEC §1a.6): the book and its anchors. A pure function (the node reads the file
/// with `launcher::read_bounded`) and the fuzz entry point. The whole file is refused when it exceeds that read's bound
/// (`launcher::MAX_INVITATION`, 1 MiB), when its end line is missing, counts another number of lines or carries
/// another checksum, when its header names another format, chain or genesis, and when any line is malformed. Every
/// entry read then passes `learnable` and the quotas again (`load`), so a stale or edited file exceeds no bound, and
/// at most ANCHORS_MAX learnable anchors are kept. The error is the reason, for the node's status.
pub fn parse(bytes:&[u8],chain_id:u64,genesis:&[u8;32],allow_local:bool,now:u64)->Result<(Book,Vec<(SocketAddr,u64)>),String>{
    if bytes.len()>crate::launcher::MAX_INVITATION{return Err("file exceeds 1 MiB".into())}
    let text=std::str::from_utf8(bytes).map_err(|_|"not text")?;
    let t=text.strip_suffix('\n').ok_or("no end line")?;let cut=t.rfind('\n').ok_or("no end line")?+1;
    let (body,end)=(&text[..cut],&t[cut..]);
    let Some(["end",n,sum])=fields::<3>(end) else{return Err("no end line".into())};
    if num::<usize>(n)!=Some(body.bytes().filter(|c|*c==b'\n').count()){return Err("line count".into())}
    if sum!=crate::hex::encode(&crate::hashes::sha256(body.as_bytes())){return Err("checksum".into())}
    let mut lines=body.split_terminator('\n');
    let Some(["pocol-addrbook","1",c,g])=lines.next().and_then(fields::<4>) else{return Err("header".into())};
    if num::<u64>(c)!=Some(chain_id){return Err("another chain".into())}
    if g!=crate::hex::encode(genesis){return Err("another genesis".into())}
    let mut book=Book::new(allow_local);let bad=|i:usize|format!("line {i}");
    match lines.next().and_then(fields::<2>){Some(["g",ms])=>book.last_good_ms=num::<u64>(ms).ok_or_else(||bad(2))?.max(1),_=>return Err(bad(2))}
    let mut anchors:Vec<(SocketAddr,u64)>=Vec::with_capacity(ANCHORS_MAX);
    for (i,line) in (3..).zip(lines){
        if line.starts_with("e "){
            let f=fields::<11>(line).ok_or_else(||bad(i))?;
            let a:SocketAddr=f[1].parse().map_err(|_|bad(i))?;let source=source_of(f[2]).ok_or_else(||bad(i))?;
            let mut t=[0u64;6];for (x,s) in t.iter_mut().zip(&f[3..9]){*x=num(s).ok_or_else(||bad(i))?;}
            book.load(a,source,t,num(f[9]).ok_or_else(||bad(i))?,num(f[10]).ok_or_else(||bad(i))?,now);
        }else if line.starts_with("a "){
            let f=fields::<3>(line).ok_or_else(||bad(i))?;let a:SocketAddr=f[1].parse().map_err(|_|bad(i))?;let ms=num(f[2]).ok_or_else(||bad(i))?;
            let a=canonical(a);
            if anchors.len()<ANCHORS_MAX&&!anchors.iter().any(|x|x.0==a)&&book.learnable(a,net::group_of(a.ip()).is_none(),false).is_some(){anchors.push((a,ms));}
        }else{return Err(bad(i))}
    }
    book.dirty=false;
    Ok((book,anchors))
}

#[cfg(test)]
mod tests{
    use super::*;
    const T0:u64=1_800_000_000_000;
    const DAY:u64=DAY_MS;
    fn sa(s:&str)->SocketAddr{s.parse().unwrap()}
    /// A public IPv4 address in the k-th /16 from 11.0/16 on (first octets 11 to 99 are all globally reachable).
    fn v4(k:usize,host:u8)->SocketAddr{assert!(k<89*256);SocketAddr::new(IpAddr::from([11+(k/256) as u8,(k%256) as u8,0,host]),30300)}
    /// A public IPv6 address in the k-th /32 of 2a00::/12.
    fn v6(k:usize)->SocketAddr{assert!(k<1<<20);SocketAddr::new(IpAddr::from([0x2a00|(k>>16) as u16,k as u16,0,0,0,0,0,1]),30300)}
    /// The source of an answering peer in 9.k/16.
    fn peer(k:usize)->Source{Source::Peer(Key::Net(Group::V4([9,k as u8])))}
    fn count(b:&Book,f:impl Fn(&Entry)->bool)->usize{b.entries().iter().filter(|e|f(e)).count()}

    /// VA-SPEC §1a.3: what may be stored, and under which key.
    #[test] fn learnable_addresses_and_keys(){
        let (public,local)=(Book::new(false),Book::new(true));
        let key=|b:&Book,s:&str,from_local:bool|b.learnable(sa(s),from_local,false).map(|(_,k)|k);
        assert_eq!(public.learnable(sa("8.8.8.8:30300"),false,false),Some((sa("8.8.8.8:30300"),Key::Net(Group::V4([8,8])))));
        assert_eq!(public.learnable(sa("[2606:4700::1]:30300"),false,false),Some((sa("[2606:4700::1]:30300"),Key::Net(Group::V6([0x26,0x06,0x47,0x00])))));
        assert_eq!(public.learnable(sa("[::ffff:8.8.8.8]:30300"),false,false),Some((sa("8.8.8.8:30300"),Key::Net(Group::V4([8,8])))),"a mapped address is stored as IPv4");
        // The port floor, from which a configured peer's own address is exempt; port 0 never travels in an Addr record.
        assert_eq!(key(&public,"8.8.8.8:1023",false),None);assert!(key(&public,"8.8.8.8:1024",false).is_some());
        assert!(public.learnable(sa("8.8.8.8:1023"),false,true).is_some(),"a configured address below 1024");
        assert_eq!(public.learnable(sa("8.8.8.8:0"),false,true),None);
        // Only 2000::/3 of IPv6: NAT64, SIIT and 200::/7 have an address group but are refused.
        for s in ["[64:ff9b::808:808]:30300","[64:ff9b::1]:30300","[::ffff:0:808:808]:30300","[200::1]:30300","[3ff:9::1]:30300","[4000::1]:30300"]{
            assert_eq!(key(&public,s,false),None,"{s}");assert_eq!(key(&local,s,true),None,"{s} in local mode");
        }
        assert_eq!(key(&public,"[2002:808:404::1]:30300",false),Some(Key::Net(Group::V4([8,8]))),"6to4 shares the /16 of the IPv4 address it carries");
        // Local addresses: only in local mode and only from a source that is local too; each endpoint its own key.
        for s in ["127.0.0.1:40001","[::ffff:127.0.0.1]:40001","10.1.2.3:30300","172.16.0.9:30300","192.168.1.1:30300","100.64.0.1:30300","[::1]:30300","[fc00::1]:30300","[fd12:3456::1]:30300"]{
            assert_eq!(key(&local,s,true),Some(Key::Local(canonical(sa(s)))),"{s} in local mode from a local source");
            assert_eq!(key(&local,s,false),None,"{s} from a public source");assert_eq!(key(&public,s,true),None,"{s} without local mode");
        }
        assert_ne!(key(&local,"127.0.0.1:40001",true),key(&local,"127.0.0.1:40002",true),"two loopback endpoints are two keys");
        // Never stored, not even in local mode from a local source.
        for s in ["169.254.0.1:30300","[fe80::1]:30300","0.0.0.0:30300","[::]:30300","0.1.2.3:30300","224.0.0.1:30300","[ff02::1]:30300","255.255.255.255:30300","240.0.0.1:30300","192.0.2.1:30300"]{
            assert_eq!(key(&local,s,true),None,"{s}");
        }
        // This node's own addresses, in any form; another port of the same IP is someone else.
        let mut b=Book::new(true);b.mark_self(sa("8.8.8.8:30300"));b.mark_self(sa("127.0.0.1:40001"));
        assert_eq!(b.learnable(sa("8.8.8.8:30300"),false,true),None,"own address, even when configured");
        assert_eq!(b.learnable(sa("[::ffff:8.8.8.8]:30300"),false,false),None,"own address, mapped");
        assert_eq!(key(&b,"127.0.0.1:40001",true),None);assert!(key(&b,"8.8.8.8:30301",false).is_some()&&key(&b,"127.0.0.1:40002",true).is_some());
        for p in 0..SELF_MAX as u16-1{b.mark_self(SocketAddr::new([8,8,8,8].into(),40000+p));}
        assert!(b.selfs().len()==SELF_MAX&&key(&b,"8.8.8.8:30300",false).is_some()&&key(&b,"127.0.0.1:40001",true).is_none(),"16 own addresses, the oldest forgotten first");
    }
    /// VA-SPEC §1a.4: every quota holds at its bound, exactly.
    #[test] fn quotas_hold_at_their_edges(){
        // 8 per key (from 8 sources): the 9th is dropped.
        let mut b=Book::new(false);
        for h in 1..=8{assert_eq!(b.insert_unverified(SocketAddr::new([81,2,69,h].into(),30300),peer(h.into()),Kind::Heard,false,T0),Outcome::Added,"entry {h} of the key");}
        assert_eq!(b.insert_unverified(sa("81.2.70.1:30300"),peer(9),Kind::Heard,false,T0),Outcome::Dropped(Why::Group));
        assert_eq!(b.insert_unverified(sa("81.2.69.1:30300"),Source::Inbound,Kind::Direct,false,T0+5),Outcome::Refreshed,"a known address is refreshed in a full key");
        assert!(b.get(sa("81.2.69.1:30300")).is_some_and(|e|(e.heard_ms,e.direct_ms,e.source)==(T0,T0+5,peer(1))));
        // 2 ports per IP: the 3rd is dropped. Local entries have no port quota: in local mode every node may be 127.0.0.1.
        let mut b=Book::new(true);
        for p in [30300,30301]{assert_eq!(b.insert_unverified(SocketAddr::new([81,2,69,1].into(),p),peer(p.into()),Kind::Heard,false,T0),Outcome::Added);}
        assert_eq!(b.insert_unverified(sa("81.2.69.1:30302"),peer(3),Kind::Heard,false,T0),Outcome::Dropped(Why::Ports));
        for p in 40001..=40003{assert_eq!(b.insert_unverified(SocketAddr::new([127,0,0,1].into(),p),Source::Inbound,Kind::Direct,true,T0),Outcome::Added);}
        // A terrible port of a full IP: an unverified newcomer on that IP is still dropped; a verified one takes its place.
        let mut now=T0;for _ in 0..3{now+=2;b.good(sa("81.9.0.1:30300"),false,false,now);b.failed(sa("81.2.69.1:30300"),now+1);}
        assert!(b.terrible(b.get(sa("81.2.69.1:30300")).unwrap(),now+2));
        assert_eq!(b.insert_unverified(sa("81.2.69.1:30302"),peer(3),Kind::Heard,false,now+2),Outcome::Dropped(Why::Ports));
        assert_eq!(b.good(sa("81.2.69.1:30302"),false,false,now+2),Outcome::Replaced);assert!(b.get(sa("81.2.69.1:30300")).is_none());
        // 64 unverified per answering peer, a slot's or a configured one's: the 65th replaces that source's entry heard
        // longest ago (here the one with the highest address), never another source's older one.
        for src in [peer(1),Source::Cfg(sa("81.2.69.9:30300"))]{
            let mut b=Book::new(false);
            assert_eq!(b.insert_unverified(sa("81.3.0.1:30300"),peer(2),Kind::Heard,false,T0-DAY),Outcome::Added);
            for k in 0..SOURCE_MAX{assert_eq!(b.insert_unverified(v4(k,1),src,Kind::Heard,false,T0+(SOURCE_MAX-k) as u64),Outcome::Added,"{src} entry {k}");}
            assert_eq!(b.insert_unverified(v4(SOURCE_MAX,1),src,Kind::Heard,false,T0+DAY),Outcome::Replaced);
            assert!(b.get(v4(SOURCE_MAX-1,1)).is_none()&&b.get(v4(0,1)).is_some()&&b.get(sa("81.3.0.1:30300")).is_some(),"{src} rotated its own oldest entry");
            assert_eq!(count(&b,|e|e.source==src),SOURCE_MAX);
        }
        // 256 self-advertisements in all, which never rotate: the 257th is dropped.
        let mut b=Book::new(false);
        for k in 0..SELFADS_MAX{assert_eq!(b.insert_unverified(v4(k,1),Source::Inbound,Kind::Direct,false,T0+k as u64),Outcome::Added,"self-advertisement {k}");}
        assert_eq!(b.insert_unverified(v4(SELFADS_MAX,1),Source::Inbound,Kind::Direct,false,T0+DAY),Outcome::Dropped(Why::Source));
        // 1024 in the book (16 sources of 64, one key each): the 1025th is dropped while no entry is terrible, and takes
        // the place of the one that is.
        let mut b=Book::new(false);
        for k in 0..BOOK_MAX{assert_eq!(b.insert_unverified(v4(k,1),peer(k/SOURCE_MAX),Kind::Heard,false,if k==0{T0}else{T0+DAY}),Outcome::Added,"entry {k}");}
        assert_eq!(b.insert_unverified(v4(BOOK_MAX,1),peer(16),Kind::Heard,false,T0+30*DAY),Outcome::Dropped(Why::Full),"nothing is terrible yet");
        assert_eq!(b.insert_unverified(v4(BOOK_MAX,1),peer(16),Kind::Heard,false,T0+30*DAY+1),Outcome::Replaced,"30 days without news make an entry terrible");
        assert!(b.get(v4(0,1)).is_none()&&b.get(v4(BOOK_MAX,1)).is_some());assert_eq!(b.entries().len(),BOOK_MAX);
        assert_eq!(b.entries.capacity(),BOOK_MAX,"never grown");
        // Discovery off: nothing is reserved or stored.
        let mut d=Book::disabled();
        assert_eq!((d.insert_unverified(v4(0,1),peer(1),Kind::Heard,false,T0),d.good(v4(1,1),false,false,T0)),(Outcome::Dropped(Why::Full),Outcome::Dropped(Why::Full)));
        d.mark_self(v4(2,1));assert!(d.entries.capacity()==0&&d.selfs.is_empty()&&!d.dirty(),"a disabled book reserves and holds nothing");
    }
    /// Heilman et al. (2015) countermeasure 1 at the book's scale. 200 honest entries from 20 sources; then floods of
    /// hearsay from one source and from 20 more, and of self-advertisements from one /32. Every honest entry stays, no
    /// attacker source holds more than SOURCE_MAX entries, the /32 holds GROUP_MAX (the first flood source filled it;
    /// nothing displaces those) and the book BOOK_MAX.
    #[test] fn floods_never_displace_other_sources_entries(){
        let mut b=Book::new(false);let mut now=T0;
        let mut add=|b:&mut Book,a:SocketAddr,src:Source,kind:Kind|{now+=1;b.insert_unverified(a,src,kind,false,now)};
        let honest:Vec<SocketAddr>=(0..200).map(|k|v4(k,1)).collect();
        for (k,a) in honest.iter().enumerate(){assert_eq!(add(&mut b,*a,peer(k/10),Kind::Heard),Outcome::Added);}
        let attacker=|s:usize|Source::Peer(Key::Net(Group::V4([66,s as u8])));
        let slash32=|i:usize|SocketAddr::new(IpAddr::from([0x2606,0x4700,0,0,0,0,(i>>16) as u16,i as u16]),30300);let k32=Key::Net(Group::V6([0x26,0x06,0x47,0x00]));
        for i in 0..20_000{add(&mut b,v4(200+i,1),attacker(0),Kind::Heard);}
        assert_eq!(count(&b,|e|e.source==attacker(0)),SOURCE_MAX,"one source keeps its quota and rotates within it");
        // 20 more sources of 1,000 each, the last 50 of each in the /32: the book fills up.
        for s in 1..=20{for j in 0..1000{add(&mut b,if j<950{v6(s*1000+j)}else{slash32(s*1000+j)},attacker(s),Kind::Heard);}}
        assert_eq!(b.entries().len(),BOOK_MAX);
        for i in 0..20_000{add(&mut b,slash32(100_000+i),Source::Inbound,Kind::Direct);}
        assert!(honest.iter().all(|a|b.get(*a).is_some()),"every honest entry is still there");
        for s in 0..=20{assert!(count(&b,|e|e.source==attacker(s))<=SOURCE_MAX,"attacker source {s}");}
        assert_eq!(count(&b,|e|e.key==k32),GROUP_MAX,"the /32");assert_eq!(count(&b,|e|e.source==Source::Inbound),0,"its self-advertisements found no place");
        assert_eq!((b.entries().len(),b.entries.capacity()),(BOOK_MAX,BOOK_MAX));
    }
    /// VA-SPEC §1a.4: a verified newcomer may replace an unverified entry, however fresh, but never a verified one: not
    /// in a full key, not on a full IP, not in a full book.
    #[test] fn a_verified_entry_is_never_displaced_by_another(){
        let k=|h:u8|SocketAddr::new([81,2,69,h].into(),30300);
        let mut b=Book::new(false);
        for h in 1..=8{assert_eq!(b.good(k(h),false,false,T0),Outcome::Added);}
        assert_eq!(b.good(k(9),false,false,T0+DAY),Outcome::Dropped(Why::Group),"a verified newcomer");
        assert_eq!(b.insert_unverified(k(10),peer(1),Kind::Heard,false,T0+DAY),Outcome::Dropped(Why::Group),"an unverified one");
        let mut b=Book::new(false);
        for h in 1..=7{b.good(k(h),false,false,T0);}assert_eq!(b.insert_unverified(k(8),peer(1),Kind::Heard,false,T0+DAY),Outcome::Added);
        assert_eq!(b.good(k(9),false,false,T0+DAY),Outcome::Replaced);
        assert!(b.get(k(8)).is_none()&&[1,2,3,4,5,6,7,9].iter().all(|h|b.get(k(*h)).is_some_and(|e|e.verified_ms>0)),"the unverified entry made room, the verified ones stay");
        // Two verified ports of an IP; then one verified and one unverified.
        let mut b=Book::new(false);b.good(sa("81.2.69.1:30300"),false,false,T0);b.good(sa("81.2.69.1:30301"),false,false,T0);
        assert_eq!(b.good(sa("81.2.69.1:30302"),false,false,T0+DAY),Outcome::Dropped(Why::Ports));
        let mut b=Book::new(false);b.good(sa("81.2.69.1:30300"),false,false,T0);b.insert_unverified(sa("81.2.69.1:30301"),peer(1),Kind::Heard,false,T0+DAY);
        assert_eq!(b.good(sa("81.2.69.1:30302"),false,false,T0+DAY),Outcome::Replaced);assert!(b.get(sa("81.2.69.1:30301")).is_none());
        // A full book: of unverified entries, the one with the oldest news makes room; of verified ones, none does.
        let mut b=Book::new(false);
        for i in 0..BOOK_MAX{b.insert_unverified(v4(i,1),peer(i/SOURCE_MAX),Kind::Heard,false,T0+(BOOK_MAX-i) as u64);}
        assert_eq!(b.good(v4(BOOK_MAX,1),false,false,T0+DAY),Outcome::Replaced);assert!(b.get(v4(BOOK_MAX-1,1)).is_none());
        let mut b=Book::new(false);for i in 0..BOOK_MAX{b.good(v4(i,1),false,false,T0);}
        assert_eq!(b.good(v4(BOOK_MAX,1),false,false,T0+DAY),Outcome::Dropped(Why::Full));
        assert_eq!(b.insert_unverified(v4(BOOK_MAX,1),peer(1),Kind::Heard,false,T0+DAY),Outcome::Dropped(Why::Full));
        assert_eq!(count(&b,|e|e.verified_ms==T0),BOOK_MAX);
    }
    /// VA-SPEC §1a.4: terrible entries, failures counted at most once between two verifications, and release holds.
    #[test] fn terrible_entries_and_counted_failures(){
        let mut b=Book::new(false);let (x,other)=(sa("81.2.69.1:30300"),sa("81.3.0.1:30300"));
        b.insert_unverified(x,peer(1),Kind::Heard,false,T0);
        let attempts=|b:&Book|b.get(x).unwrap().attempts;let terrible=|b:&Book,now|b.terrible(b.get(x).unwrap(),now);
        b.failed(x,T0+1);assert_eq!(attempts(&b),1,"the first failure counts (last_good starts at 1)");
        b.failed(x,T0+2);assert_eq!(attempts(&b),1,"no verification since the last counted failure: an outage counts once");
        b.good(other,false,false,T0+3);b.busy(x,T0+4);b.tried(x,T0+4);assert_eq!(attempts(&b),1,"a connection closed before any Hello, and a dial starting, are no failures");
        b.failed(x,T0+5);assert_eq!(attempts(&b),2,"a verification since: it counts");
        b.good(other,false,false,T0+5);b.failed(x,T0+6);assert_eq!(attempts(&b),2,"a verification in the millisecond of the counted failure is not since it");
        assert!(!terrible(&b,T0+7),"2 counted failures");
        b.good(other,false,false,T0+8);b.failed(x,T0+9);assert!(attempts(&b)==3&&terrible(&b,T0+10),"3 counted failures of an entry never verified");
        assert_eq!(b.get(x).unwrap().last_try_ms,T0+9);
        // A verified entry with 10 counted failures is terrible once its verification is older than 7 days.
        let y=sa("81.4.0.1:30300");b.good(y,false,false,T0);
        for i in 0..10{b.good(other,false,false,T0+20+2*i);b.failed(y,T0+21+2*i);}
        let e=b.get(y).unwrap().clone();assert_eq!(e.attempts,10);
        assert!(!b.terrible(&e,T0+6*DAY)&&!b.terrible(&e,T0+7*DAY),"verified 7 days ago");assert!(b.terrible(&e,T0+7*DAY+1)&&b.terrible(&e,T0+8*DAY));
        assert!(!b.terrible(&Entry{attempts:9,..e.clone()},T0+8*DAY),"9 failures");
        assert_eq!(b.good(y,false,false,T0+8*DAY),Outcome::Refreshed);assert_eq!(b.get(y).unwrap().attempts,0,"a verification clears the failures");
        // Nothing heard, advertised or verified for 30 days; the latest of the three counts.
        let quiet=Entry{verified_ms:0,attempts:0,heard_ms:T0,..e};
        assert!(!b.terrible(&quiet,T0+30*DAY)&&b.terrible(&quiet,T0+30*DAY+1));
        for fresh in [Entry{direct_ms:T0+1,..quiet.clone()},Entry{verified_ms:T0+1,..quiet.clone()}]{assert!(!b.terrible(&fresh,T0+30*DAY+1),"{fresh:?}");}
        // Holds of 600 s, 1200 s, ... capped at 6 h, also past 64 doublings and once `holds` saturates; no failure.
        let mut b=Book::new(false);b.good(x,false,false,T0);
        for n in 1..=300u32{
            b.held(x,T0);let want=1u64.checked_shl(n.min(255)-1).map_or(u64::MAX,|f|600_000u64.saturating_mul(f)).min(6*3_600_000);
            assert_eq!(b.get(x).unwrap().hold_until_ms,T0+want,"hold {n}");
        }
        assert!(b.get(x).is_some_and(|e|e.attempts==0&&e.holds==u8::MAX&&!b.relayable(e,T0+6*3_600_000-1)&&b.relayable(e,T0+6*3_600_000)),"a held entry is answered again once its hold ends");
        b.good(x,false,false,T0+1);assert_eq!(b.get(x).unwrap().hold_until_ms,0,"a verification ends the hold");
    }
    /// VA-SPEC §1a.5: 100 verified entries in 60 keys, 100 self-advertised and 100 heard ones, plus entries that are
    /// stale, held, terrible and local. The answer list holds the verified ones first, 2 per key, then self-advertised
    /// ones, 128 in all, never hearsay; an answer is at most 64 of it in order, without local entries for a public
    /// requester and never the requester itself; the list is the same for 60 s and then drawn again.
    #[test] fn answers_are_verified_first_two_per_key_and_bounded(){
        assert_ne!(Rng::new(0).next_u64(),0,"a zero seed still draws");
        let mut b=Book::new(true);let mut rng=Rng::new(7);let now=T0+1000;
        let mut verified=Vec::new();
        for k in 0..60{for h in 1..=(if k<20{3}else{1}){let a=v4(k,h);assert_eq!(b.good(a,false,false,T0),Outcome::Added);verified.push(a);}}
        let direct:Vec<SocketAddr>=(60..160).map(|k|v4(k,1)).collect();for a in &direct{b.insert_unverified(*a,Source::Inbound,Kind::Direct,false,T0);}
        let heard:Vec<SocketAddr>=(160..260).map(|k|v4(k,1)).collect();for (i,a) in heard.iter().enumerate(){b.insert_unverified(*a,peer(i%2),Kind::Heard,false,T0);}
        let (stale,held,terrible)=([v4(260,1),v4(261,1)],v4(262,1),v4(263,1));
        b.good(stale[0],false,false,now-7*DAY-1);b.insert_unverified(stale[1],Source::Inbound,Kind::Direct,false,now-7*DAY-1);
        b.good(held,false,false,T0);b.held(held,T0);
        b.insert_unverified(terrible,Source::Inbound,Kind::Direct,false,T0);for i in 0..3{b.good(verified[0],false,false,T0+2*i);b.failed(terrible,T0+2*i+1);}
        let local:Vec<SocketAddr>=(1..=4).map(|p|SocketAddr::new([127,0,0,1].into(),40000+p)).collect();for a in &local{b.good(*a,false,true,T0);}
        assert!(b.relayable(&Entry{verified_ms:now-7*DAY,..b.get(stale[0]).unwrap().clone()},now),"verified exactly 7 days ago is still relayable");
        // A live connection's refresh moves only a time already set: hearsay never becomes verified or self-advertised.
        for direct in [false,true]{b.touch(heard[0],direct,now);}b.touch(verified[1],false,now);
        assert!(b.get(heard[0]).is_some_and(|e|e.verified_ms==0&&e.direct_ms==0)&&b.get(verified[1]).is_some_and(|e|e.verified_ms==now));
        let first=b.answer(false,None,&mut rng,now);let list=b.cache.clone();
        assert_eq!(list.len(),CACHE_MAX);
        assert!(list.iter().enumerate().all(|(i,a)|!list[..i].contains(a)),"no address twice");
        let at=list.iter().position(|a|!verified.contains(a)&&!local.contains(a)).unwrap();
        assert_eq!(at,80+4,"2 of each 3-entry key and 1 of each other verified key, and the 4 local entries, come first");
        assert!(list[at..].iter().all(|a|direct.contains(a)),"then self-advertised entries");
        for a in &list{let k=b.get(*a).unwrap().key;assert!(count(&b,|e|e.key==k&&list.contains(&e.addr))<=ANSWER_PER_KEY,"{a}");}
        for a in heard.iter().chain(&stale).chain([&held,&terrible]){assert!(!list.contains(a),"{a} is never answered");}
        assert_eq!(first,list.iter().copied().filter(|a|!local.contains(a)).take(net::ADDR_MAX).collect::<Vec<_>>());
        assert!(first.len()==net::ADDR_MAX&&first.iter().all(|a|verified.contains(a)),"64, all verified");
        // The requester's own advertised address is left out; the list stays as it is for 60 s.
        let requester=first[0];let again=b.answer(false,Some(requester),&mut rng,now+1);
        assert!(!again.contains(&requester)&&again.len()==net::ADDR_MAX&&again[..net::ADDR_MAX-1]==first[1..],"the same answer without the requester");
        let gone=list[1];b.remove(gone);
        b.answer(false,None,&mut rng,now+59_999);assert_eq!(b.cache,list,"the same list within 60 s, although an entry left");
        b.answer(false,None,&mut rng,now+60_000);
        assert!(b.cache.len()==CACHE_MAX&&!b.cache.contains(&gone),"rebuilt after 60 s");
        let kept:Vec<SocketAddr>=b.cache.iter().copied().filter(|a|list.contains(a)).collect();
        assert_ne!(kept,list.iter().copied().filter(|a|kept.contains(a)).collect::<Vec<_>>(),"drawn in a new order");
        // A list with room for every relayable entry: local entries go to a local requester only, and never its own
        // endpoint (another node on 127.0.0.1 is someone else); hearsay, a held entry and a terrible one never go.
        let mut s=Book::new(true);let (p,l1,l2)=(sa("81.2.69.1:30300"),local[0],local[1]);
        for (a,l) in [(p,false),(l1,true),(l2,true)]{s.good(a,false,l,T0);}
        let (h,hold,bad)=(sa("81.3.0.1:30300"),sa("81.4.0.1:30300"),sa("81.5.0.1:30300"));
        s.insert_unverified(h,peer(1),Kind::Heard,false,T0);s.good(hold,false,false,T0);s.held(hold,T0);
        s.insert_unverified(bad,Source::Inbound,Kind::Direct,false,T0);for i in 0..3{s.good(p,false,false,T0+2*i);s.failed(bad,T0+2*i+1);}
        assert_eq!(s.answer(false,None,&mut rng,now),vec![p],"a public requester");
        let mut to_local=s.answer(true,Some(l1),&mut rng,now);to_local.sort();assert_eq!(to_local,vec![p,l2],"a local requester");
        assert!(Book::disabled().answer(true,None,&mut rng,now).is_empty());
    }
    /// VA-SPEC §1a.6: encode then parse gives the same entries, anchors and last_good_ms; a file that is foreign, damaged,
    /// cut, miscounted or over 1 MiB is refused whole; what a well-formed file holds passes the filter and the quotas
    /// again; every line fits FILE_LINE_MAX.
    #[test] fn the_file_round_trips_and_fails_closed(){
        const CHAIN:u64=777900;let genesis=[7u8;32];let now=T0+10;
        let mut b=Book::new(true);
        b.good(sa("81.2.69.1:30300"),false,false,T0);
        b.insert_unverified(sa("[2606:4700::1]:30300"),Source::Inbound,Kind::Direct,false,T0+1);
        b.insert_unverified(sa("81.3.0.1:30300"),Source::Peer(Key::Net(Group::V6([0x26,0x06,0x47,0x00]))),Kind::Heard,false,T0+2);
        b.insert_unverified(sa("81.4.0.1:30300"),Source::Cfg(sa("81.2.69.1:30300")),Kind::Heard,false,T0+3);
        b.insert_unverified(sa("81.6.0.1:30300"),peer(3),Kind::Heard,false,T0+3);
        b.insert_unverified(sa("127.0.0.1:40001"),Source::Peer(Key::Local(sa("127.0.0.1:40002"))),Kind::Heard,true,T0+4);
        b.good(sa("127.0.0.1:40002"),false,true,T0+5);
        b.held(sa("81.2.69.1:30300"),T0+6);b.failed(sa("81.3.0.1:30300"),T0+7);b.mark_self(sa("81.8.0.1:30300"));
        // The longest line: IPv6 address and source at full length, every number at its largest.
        let long=sa("[2606:4700:ffff:ffff:ffff:ffff:ffff:ffff]:65535");
        b.insert_unverified(long,Source::Cfg(sa("[2606:4701:ffff:ffff:ffff:ffff:ffff:ffff]:65535")),Kind::Heard,false,T0);
        let e=b.entries.iter_mut().find(|e|e.addr==long).unwrap();
        (e.heard_ms,e.direct_ms,e.verified_ms,e.last_try_ms,e.last_count_ms,e.hold_until_ms,e.attempts,e.holds)=(u64::MAX,u64::MAX,u64::MAX,u64::MAX,u64::MAX,u64::MAX,u8::MAX,u8::MAX);
        assert!(b.dirty());b.saved();assert!(!b.dirty());b.touch(sa("81.2.69.1:30300"),false,T0+9);assert!(b.dirty(),"a refresh is a change to save");
        let anchors=[(sa("81.7.0.1:30300"),T0+8),(sa("[2606:4700::2]:30300"),T0+9)];
        let file=b.encode(CHAIN,&genesis,&anchors);let text=std::str::from_utf8(&file).unwrap();
        for l in text.split_terminator('\n'){assert!(l.len()<FILE_LINE_MAX,"{} B: {l}",l.len()+1);}
        let (back,got)=parse(&file,CHAIN,&genesis,true,now).unwrap();
        assert_eq!((back.entries(),&got[..],back.last_good_ms()),(b.entries(),&anchors[..],b.last_good_ms()));
        assert!(back.entries().len()==8&&!back.dirty()&&back.selfs().is_empty(),"own addresses are not kept");
        assert_eq!(b.encode(CHAIN,&genesis,&[anchors[0],anchors[1],(sa("81.9.0.1:30300"),T0)]),file,"at most 2 anchors are written");
        // Refused whole.
        let refused=|f:&[u8]|parse(f,CHAIN,&genesis,true,now).err();
        assert_eq!(parse(&file,CHAIN+1,&genesis,true,now).err().as_deref(),Some("another chain"));
        assert_eq!(parse(&file,CHAIN,&[8;32],true,now).err().as_deref(),Some("another genesis"));
        for i in 0..file.len(){let mut f=file.clone();f[i]^=1;assert!(refused(&f).is_some(),"a flipped byte at {i}");}
        let cut=text[..text.len()-1].rfind('\n').unwrap()+1;let body=&text[..cut];
        assert_eq!(refused(&file[..cut]).as_deref(),Some("no end line"));
        let with_end=|body:&str,n:usize|format!("{body}end {n} {}\n",crate::hex::encode(&crate::hashes::sha256(body.as_bytes()))).into_bytes();
        let lines=body.matches('\n').count();assert!(refused(&with_end(body,lines)).is_none(),"the same file, end line rebuilt");
        for n in [lines-1,lines+1]{assert_eq!(refused(&with_end(body,n)).as_deref(),Some("line count"));}
        // Files written here: the header and g lines, then these lines.
        let file_of=|lines:&[String]|with_end(&format!("pocol-addrbook 1 {CHAIN} {}\ng 1\n{}",crate::hex::encode(&genesis),lines.iter().map(|l|format!("{l}\n")).collect::<String>()),lines.len()+2);
        let line=|a:SocketAddr|format!("e {a} i 0 {T0} {T0} 0 0 0 0 0");
        let one_key=|n:usize|(0..n).map(|i|line(SocketAddr::new([81,2,(i/250) as u8,(i%250) as u8+1].into(),30300))).collect::<Vec<_>>();
        let (big,fits)=(file_of(&one_key(18_000)),file_of(&one_key(16_000)));assert!(big.len()>crate::launcher::MAX_INVITATION&&fits.len()<=crate::launcher::MAX_INVITATION);
        assert_eq!(refused(&big).as_deref(),Some("file exceeds 1 MiB"));
        assert_eq!(parse(&fits,CHAIN,&genesis,false,now).unwrap().0.entries().len(),GROUP_MAX,"a well-formed file under the bound loads within the key quota");
        assert_eq!(parse(&file_of(&one_key(5000)),CHAIN,&genesis,false,now).unwrap().0.entries().len(),GROUP_MAX,"5,000 lines in one key");
        let (spread,_)=parse(&file_of(&(0..5000).map(|k|line(v4(k,1))).collect::<Vec<_>>()),CHAIN,&genesis,false,now).unwrap();
        assert_eq!((spread.entries().len(),spread.entries.capacity()),(BOOK_MAX,BOOK_MAX),"5,000 lines in distinct keys");
        for bad in ["e 81.2.69.1:30300 i 0 0 0 0 0 0 0","e 81.2.69.1:30300 x 1 0 0 0 0 0 0 0","e 81.2.69.1:30300 i +1 0 0 0 0 0 0 0","e 81.2.69.1 i 1 0 0 0 0 0 0 0",
            "e 81.2.69.1:30300 i 1 0 0 0 0 0 256 0","e 81.2.69.1:30300 p4:9 1 0 0 0 0 0 0 0","e 81.2.69.1:30300 p6:2606470 1 0 0 0 0 0 0 0","a 81.2.69.1:30300","x 1","",
            "e 81.2.69.1:30300  i 1 0 0 0 0 0 0 0"]{
            assert_eq!(refused(&file_of(&[line(sa("81.3.0.1:30300")),bad.into()])).as_deref(),Some("line 4"),"{bad:?}");
        }
        assert_eq!(refused(&with_end("pocol-addrbook 2 777900 00\ng 1\n",2)).as_deref(),Some("header"));
        // Each line passes the filter and the quotas again: local entries and anchors only in local mode, terrible
        // entries never, low ports never (not even a configured peer's: it is learned again at its next handshake).
        let (public,got)=parse(&b.encode(CHAIN,&genesis,&[(sa("127.0.0.1:40003"),T0),anchors[0]]),CHAIN,&genesis,false,now).unwrap();
        assert!(public.entries().len()==6&&public.entries().iter().all(|e|matches!(e.key,Key::Net(_)))&&got==[anchors[0]],"local entries and anchors dropped");
        let (_,two)=parse(&file_of(&["a 81.2.69.1:30300 1".into(),"a 81.2.69.1:30300 2".into(),"a 81.2.69.2:30300 3".into(),"a 81.2.69.3:30300 4".into()]),CHAIN,&genesis,true,now).unwrap();
        assert_eq!(two,[(sa("81.2.69.1:30300"),1),(sa("81.2.69.2:30300"),3)],"the first 2 distinct anchors");
        let kept=|l:&str|parse(&file_of(&l.split('\n').map(String::from).collect::<Vec<_>>()),CHAIN,&genesis,true,now).unwrap().0.entries().len();
        assert_eq!(kept(&format!("e 81.2.69.1:30300 p4:9.1 {T0} 0 0 0 0 0 2 0")),1,"2 counted failures");
        assert_eq!(kept(&format!("e 81.2.69.1:30300 p4:9.1 {T0} 0 0 0 0 0 3 0")),0,"3 counted failures of an entry never verified are terrible");
        assert_eq!(kept(&format!("e 81.2.69.1:30300 p4:9.1 {} 0 0 0 0 0 0 0",now-30*DAY-1)),0,"nothing heard for 30 days");
        assert_eq!(kept(&format!("e 81.2.69.1:1023 c:81.2.69.1:1023 0 0 {T0} 0 0 0 0 0")),0,"a port below 1024");
        assert_eq!(kept(&format!("e 127.0.0.1:40001 p4:9.1 {T0} 0 0 0 0 0 0 0")),0,"a local entry heard from a public source");
        assert_eq!(kept(&format!("e 81.2.69.1:30300 i 0 {T0} 0 0 0 0 0 0\ne 81.2.69.1:30300 i 0 {} 0 0 0 0 0 0",T0+1)),1,"a second line for an address");
    }
}
