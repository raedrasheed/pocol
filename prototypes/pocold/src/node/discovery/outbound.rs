//! Automatic outbound (VA-SPEC §1a.7-§1a.9; local policy, never validity): the K automatic slots, the spare slot
//! (a feeler, or an extra full slot while the node is stalled or quiet), usefulness, health and the stall rules.
//! Each slot is one `net::slot` thread holding at most one connection; the node assigns it an address only while the
//! slot is idle (after the previous connection's Closed or DialFailed), so a slot's state here is the truth about its
//! thread. A grandchild of `node` (D21): it reads the private state of `Disc` and `Node` without a boundary line.
//! Every function returns early while no slot is installed (`slots` empty: K = 0, discovery off, or a node whose
//! network never started), so in-crate tests that never start the network see the PR2a behaviour.
use std::collections::{HashSet,VecDeque};
use std::net::SocketAddr;
use std::sync::mpsc::SyncSender;
use crate::addrbook::{self,Class,Key,Source};
use crate::net::{self,DialError,Msg,Origin,PeerId};
use super::{Discovery,Link};
use crate::node::{Node,now_ms};

/// A slot's role: one of the K automatic slots, or the spare (exactly one). Roles change only in a swap (§2.6).
#[derive(Clone,Copy,Debug,PartialEq,Eq)] pub(super) enum Role{Auto,Spare}
/// What a slot's connection is for: an automatic peer (or a pending anchor, dialed first after a restart), a feeler
/// (no Hello sent, never a peer) or the extra full peer of a stall.
#[derive(Clone,Copy,Debug,PartialEq,Eq)] pub(super) enum SlotKind{Auto,Anchor,Feeler,Extra}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(super) enum SlotState{Idle{next_ms:u64},Dialing{addr:SocketAddr,kind:SlotKind,since_ms:u64},Up{peer:PeerId,addr:SocketAddr,kind:SlotKind,since_ms:u64}}
/// One slot: its role and state, its consecutive failures (the backoff) and whether its last fill found no candidate.
pub(super) struct Slot{pub(super) role:Role,pub(super) state:SlotState,pub(super) fails:u32,pub(super) starved:bool}
impl Slot{
    fn addr(&self)->Option<SocketAddr>{match self.state{SlotState::Idle{..}=>None,SlotState::Dialing{addr,..}|SlotState::Up{addr,..}=>Some(addr)}}
    fn kind(&self)->Option<SlotKind>{match self.state{SlotState::Idle{..}=>None,SlotState::Dialing{kind,..}|SlotState::Up{kind,..}=>Some(kind)}}
    fn peer(&self)->Option<PeerId>{match self.state{SlotState::Up{peer,..}=>Some(peer),_=>None}}
}
/// A seed (VA-SPEC §1a.8): its address (canonical) as its dialer knows it (`dial`, the outbound set's text) and its
/// key, its live connection, when it was last wanted (None: suppressed, not dialed), when its connection became
/// compatible, whether it is this node itself, and its holds (`fails`, `hold_until_ms`).
pub(super) struct Seed{pub(super) addr:SocketAddr,pub(super) dial:SocketAddr,pub(super) key:Key,pub(super) peer:Option<PeerId>,pub(super) wanted_ms:Option<u64>,
    pub(super) compatible_ms:Option<u64>,pub(super) is_self:bool,pub(super) fails:u32,pub(super) hold_until_ms:u64}
/// The feeler's connection, kept here and never in `Node::peers`; `hello`: its Hello was read.
pub(super) struct Feeler{id:PeerId,peer:net::Peer,hello:bool}
/// Shown in `pocol_status.discovery.counters` (plan §6).
#[derive(Default)]
pub(super) struct OutCounters{pub(super) dials:u64,pub(super) dial_failed:u64,pub(super) dial_no_hello:u64,pub(super) dial_busy:u64,pub(super) dial_incompatible:u64,
    pub(super) dial_no_route:u64,pub(super) slots_stale:u64,pub(super) feelers:u64,pub(super) feelers_ok:u64,pub(super) feelers_incompatible:u64,pub(super) anchors_dialed:u64,
    pub(super) extras:u64,pub(super) extra_swaps:u64,pub(super) extra_closed:u64,pub(super) extra_starved:u64,pub(super) seed_wants:u64,pub(super) seed_releases:u64,
    pub(super) seed_rotations:u64,pub(super) seed_holds:u64,pub(super) stalls:u64,pub(super) quiets:u64,pub(super) bootstrap_loaded:u64}
/// Automatic outbound's state (A13 `queues/discovery: outbound state`: the struct and the vectors reserved at open,
/// OUT_STATE_BYTES in all). `k`: the resolved automatic slots; `slots`/`tx`: the K + 1 slots and their address
/// channels (empty until the network starts); `family_down`: per address family (0 IPv4, 1 IPv6) the address whose
/// route probe failed and when it was last probed; `last_peer_head_ms`: the last time a peer's block moved the head;
/// `suppressed`: the seeds not wanted (their dialers see them as live and wait); `cursor`: the next seed to want;
/// `started_ms`: the slots' start (the seeds' boot window); `healthy_since`: since when healthy >= 2;
/// `last_rotation_ms`: the last stall rotation; `stalled`/`quiet`: last tick's values (their rising edges are counted); `trouble_since`: since when either holds;
/// `next_extra_ms`: the next extra assignment while it does; `anchors_left`: anchors not dialed yet in this process,
/// `loaded_anchors`: the anchors read at the network's start with their save times, `anchors_bad`: those dialed whose
/// connection failed, was incompatible or closed short, `saved_anchors`: the set last written.
pub(super) struct Out{pub(super) k:usize,pub(super) slots:Vec<Slot>,pub(super) tx:Vec<SyncSender<SocketAddr>>,pub(super) feeler:Option<Feeler>,pub(super) next_feeler_ms:u64,
    pub(super) seeds:Vec<Seed>,pub(super) suppressed:HashSet<SocketAddr>,pub(super) seed_node:bool,pub(super) cursor:usize,pub(super) bootstrap:Vec<SocketAddr>,pub(super) cursor_b:usize,
    pub(super) anchors_left:VecDeque<SocketAddr>,pub(super) loaded_anchors:Vec<(SocketAddr,u64)>,pub(super) anchors_bad:Vec<SocketAddr>,pub(super) saved_anchors:Vec<(SocketAddr,u64)>,
    pub(super) family_down:[Option<(SocketAddr,u64)>;2],pub(super) last_peer_head_ms:u64,pub(super) started_ms:u64,pub(super) healthy_since:Option<u64>,pub(super) last_rotation_ms:u64,
    pub(super) stalled:bool,pub(super) quiet:bool,pub(super) trouble_since:Option<u64>,pub(super) next_extra_ms:u64,pub(super) counters:OutCounters,
    /// Tests: the result of every route probe (`net::route_ok`).
    #[cfg(test)] pub(super) route_ok:Option<bool>}
/// A13 (`resources::derive`, `queues/discovery: outbound state`): the bytes counted for `Out` with automatic outbound.
pub const OUT_STATE_BYTES:usize=4096;
// The struct, the slots and channels (at most OUTBOUND_MAX + 1), the seeds, bootstrap addresses, the suppressed set
// (a HashSet of SEEDS_MAX reserves about twice as many buckets, each with a control byte) and the four anchor lists of
// at most ANCHORS_MAX fit OUT_STATE_BYTES (760 + 9·80 + 7·226 + 512 + 320 = 3,894 B on x86_64).
const _:()={use std::mem::size_of;let s=super::OUTBOUND_MAX as usize+1;
    assert!(size_of::<Out>()+s*(size_of::<Slot>()+size_of::<SyncSender<SocketAddr>>())+super::SEEDS_MAX*(size_of::<Seed>()+2*(size_of::<SocketAddr>()+1))
        +super::BOOTSTRAP_MAX*size_of::<SocketAddr>()+4*addrbook::ANCHORS_MAX*size_of::<(SocketAddr,u64)>()<=OUT_STATE_BYTES)};
impl Out{
    /// At `Node::open`, from the resolved settings: K, the seeds (keyed as the book keys them), whether this host is a
    /// seed, the bootstrap addresses. Nothing runs until `Node::disc_start_outbound` installs the slots.
    pub(super) fn new(s:&Discovery)->Self{
        let mut seeds=Vec::with_capacity(super::SEEDS_MAX);seeds.extend(s.seeds.iter().map(|a|Seed{addr:canon(*a),dial:*a,key:key_of(*a),peer:None,wanted_ms:None,compatible_ms:None,is_self:false,fails:0,hold_until_ms:0}));
        let mut bootstrap=Vec::with_capacity(super::BOOTSTRAP_MAX);bootstrap.extend(s.bootstrap.iter().map(|a|canon(*a)));
        Self{k:s.automatic(),slots:Vec::new(),tx:Vec::new(),feeler:None,next_feeler_ms:0,seeds,suppressed:HashSet::with_capacity(super::SEEDS_MAX),seed_node:s.is_seed,cursor:0,bootstrap,cursor_b:0,
            anchors_left:VecDeque::with_capacity(addrbook::ANCHORS_MAX),loaded_anchors:Vec::with_capacity(addrbook::ANCHORS_MAX),anchors_bad:Vec::with_capacity(addrbook::ANCHORS_MAX),saved_anchors:Vec::with_capacity(addrbook::ANCHORS_MAX),
            family_down:[None;2],last_peer_head_ms:0,started_ms:0,healthy_since:None,last_rotation_ms:0,stalled:false,quiet:false,trouble_since:None,next_extra_ms:0,counters:OutCounters::default(),
            #[cfg(test)]route_ok:None}
    }
    fn spare(&self)->Option<usize>{self.slots.iter().position(|s|s.role==Role::Spare)}
    /// VA-SPEC §1a.7: the anchors the file held (`addrbook::parse`), those saved within `keep_ms` only; each is dialed
    /// first, once in this process.
    pub(super) fn load_anchors(&mut self,anchors:&[(SocketAddr,u64)],now:u64,keep_ms:u64){
        self.loaded_anchors.clear();self.anchors_left.clear();
        for (a,ms) in anchors.iter().filter(|(_,ms)|now.saturating_sub(*ms)<=keep_ms).take(addrbook::ANCHORS_MAX){self.loaded_anchors.push((canon(*a),*ms));self.anchors_left.push_back(canon(*a));}
        self.saved_anchors.clone_from(&self.loaded_anchors);
    }
    /// Whether `id` is the feeler's connection.
    pub(super) fn is_feeler(&self,id:PeerId)->bool{self.feeler.as_ref().is_some_and(|f|f.id==id)}
    /// Whether `a` is a seed the policy does not want now (VA-SPEC §1a.8): its address stays in the outbound set.
    pub(super) fn suppressed(&self,a:SocketAddr)->bool{self.suppressed.contains(&canon(a))}
    /// The feeler's slot and address, when `id` is the feeler's connection.
    pub(super) fn feeler_slot(&self,id:PeerId)->Option<(usize,SocketAddr)>{
        if !self.is_feeler(id){return None}
        self.slots.iter().enumerate().find_map(|(t,s)|match s.state{SlotState::Up{peer,addr,..} if peer==id=>Some((t,addr)),_=>None})
    }
}
/// How the node takes a new connection (`Node::disc_connected`): as a peer (it sends its Hello and keeps it), as the
/// feeler (it sends nothing and keeps it out of `peers`), or closed at once.
pub(in crate::node) enum Arrival{Peer,Feeler,Close}
fn canon(a:SocketAddr)->SocketAddr{SocketAddr::new(a.ip().to_canonical(),a.port())}
/// An address's diversity key, as `addrbook::Book::learnable` computes it (local mode: its own endpoint).
pub(super) fn key_of(a:SocketAddr)->Key{let a=canon(a);match net::group_of(a.ip()){Some(g)=>Key::Net(g),None=>Key::Local(a)}}
/// The address family's index in `Out::family_down`: 0 IPv4 (mapped addresses included), 1 IPv6.
fn family(a:SocketAddr)->usize{usize::from(!canon(a).is_ipv4())}
/// The number of distinct keys, without allocating: each key counts at its first position.
/// D26 (the Block arm of `Node::on_msg`): whether a block's import makes its sender useful: imported (`ok`) at a
/// height within 2 of the head after it, or a Duplicate (`duplicate`) solicited by the sender's own sync session. An
/// echo of a block this node broadcast is an unsolicited Duplicate and earns nothing.
pub(in crate::node) fn first_delivery(ok:bool,duplicate:bool,height:u64,head:u64,solicited:bool)->bool{(ok&&height.saturating_add(2)>=head)||(solicited&&duplicate)}
fn distinct(keys:impl Iterator<Item=Key>+Clone)->usize{keys.clone().enumerate().filter(|(i,k)|!keys.clone().take(*i).any(|x|x==*k)).count()}

impl Node{
    /// W (VA-SPEC §1a.7): max(stall_min_ms, stall_blocks·T_blk).
    pub(super) fn window_ms(&self)->u64{let t=&self.disc.times;t.stall_blocks.saturating_mul(u64::from(self.chain.spec.cp().T_blk)).saturating_mul(1000).max(t.stall_min_ms)}
    /// D25: the head's own timestamp is W old or older. Lagged or withheld delivery cannot hide it; this node's own
    /// blocks keep it current.
    pub(super) fn stalled(&self,now:u64)->bool{now.saturating_sub(self.chain.head().ts.saturating_mul(1000))>=self.window_ms()}
    /// D25: not stalled, and no block a peer delivered moved the head for quiet_factor·W (a lone miner).
    pub(super) fn quiet(&self,now:u64)->bool{!self.stalled(now)&&now.saturating_sub(self.disc.out.last_peer_head_ms)>=self.disc.times.quiet_factor.saturating_mul(self.window_ms())}
    /// A configured connection to `a` came up (`id`) or ended (None): a seed's live connection (VA-SPEC §1a.8). An end
    /// clears its compatibility, and a wanted seed has seed_connect_ms again from then to become compatible.
    pub(super) fn seed_connected(&mut self,a:SocketAddr,id:Option<PeerId>,now:u64){
        let a=canon(a);let Some(s)=self.disc.out.seeds.iter_mut().find(|s|s.addr==a) else{return};
        s.peer=id;if id.is_none(){s.compatible_ms=None;if s.wanted_ms.is_some(){s.wanted_ms=Some(now)}}
    }
    /// VA-SPEC §1a.8: the first compatible Hello of a seed's connection `from` (its failures cleared), or one carrying
    /// this node's own nonce (`is_self`: this node is a seed, and keeps all others).
    pub(super) fn seed_hello(&mut self,from:PeerId,is_self:bool,now:u64){
        let o=&mut self.disc.out;let Some(s)=o.seeds.iter_mut().find(|s|s.peer==Some(from)) else{return};
        if is_self{s.is_self=true;o.seed_node=true}else{s.compatible_ms=Some(now);s.fails=0}
    }
    /// D18': a connection of an address family (one this node dialed, or an inbound one's first Hello) shows that it
    /// has a route: the family is up again (and the seeds' holds are cleared, `seeds_online`).
    pub(super) fn family_up(&mut self,a:Option<SocketAddr>){if let Some(a)=a{if self.disc.out.family_down[family(a)].take().is_some(){self.seeds_online()}}}
    /// This host is back online (a family down came back, or a first compatible peer after none): every seed's holds
    /// and failures are cleared, so that the holds a local outage built up (up to 6 h) do not keep a node that needs
    /// its seeds (an empty or stale book, no bootstrap list) cut off once its network returns.
    pub(super) fn seeds_online(&mut self){for s in &mut self.disc.out.seeds{s.fails=0;s.hold_until_ms=0;}}
    /// Whether this host has a route to `a` (`net::route_ok`, which sends nothing; tests set `route_ok`).
    fn route(&self,a:SocketAddr)->bool{
        #[cfg(test)] if let Some(ok)=self.disc.out.route_ok{return ok}
        net::route_ok(a)
    }
    /// D18' for the seeds: a wanted seed that did not become compatible failed locally when this node has no
    /// compatible peer and the seed's family is down or has no route; such a failure is held only seed_hold_base_ms
    /// and does not count.
    fn local_failure(&self,a:SocketAddr)->bool{self.peer_status.is_empty()&&(self.disc.out.family_down[family(a)].is_some()||!self.route(a))}
    /// A seed's address (VA-SPEC §1a.8).
    fn is_seed_addr(&self,a:SocketAddr)->bool{let a=canon(a);self.disc.out.seeds.iter().any(|s|s.addr==a)}
    /// The key a link counts under: the address dialed, or an inbound connection's source.
    fn link_key(l:&Link)->Option<Key>{l.origin.dialed().or(l.remote).map(key_of)}
    /// D26 (VA-SPEC §1a.8): the distinct keys, none a seed's, of compatible peers in either direction that were useful
    /// within W; 0 unless one of them is an automatic slot or a configured peer that is not a seed, so that inbound
    /// peers alone (which anyone can open) never make a node healthy.
    pub(super) fn healthy(&self,now:u64)->usize{
        let (d,w)=(&self.disc,self.window_ms());
        let useful=|l:&&Link|l.compatible&&l.useful_ms>0&&l.useful_ms.saturating_add(w)>=now&&!l.origin.dialed().is_some_and(|a|self.is_seed_addr(a))
            &&Self::link_key(l).is_some_and(|k|!d.out.seeds.iter().any(|s|s.key==k));
        let outbound=|l:&Link|match l.origin{Origin::Slot(t,_)=>d.out.slots.get(usize::from(t)).is_some_and(|s|s.role==Role::Auto),Origin::Configured(_)=>true,Origin::Inbound=>false};
        if !d.links.values().filter(useful).any(outbound){return 0}
        distinct(d.links.values().filter(useful).filter_map(Self::link_key))
    }
    /// The distinct keys of the compatible peers, in either direction.
    fn compatible_keys(&self)->usize{distinct(self.disc.links.values().filter(|l|l.compatible).filter_map(Self::link_key))}
    /// VA-SPEC §1a.8: an automatic slot found no candidate at its last fill and the node has fewer than K compatible
    /// peers (in distinct keys).
    pub(super) fn starved(&self)->bool{self.disc.out.slots.iter().any(|s|s.role==Role::Auto&&s.starved)&&self.compatible_keys()<self.disc.out.k}
    /// §2.1: an address no slot or feeler dials: one a live connection or another slot dialed, an inbound connection's
    /// advertised endpoint, a configured peer, this node's own, or one of an address family that is down.
    fn excluded_addr(&self,a:SocketAddr)->bool{
        let (a,d)=(canon(a),&self.disc);
        d.links.values().any(|l|l.origin.dialed().map(canon)==Some(a)||l.advertised==Some(a))||d.out.slots.iter().any(|s|s.addr()==Some(a))
            ||self.cfg.peers.iter().any(|p|canon(*p)==a)||d.book.selfs().contains(&a)||d.out.family_down[family(a)].is_some()
    }
    /// §2.1: one connection per key among the automatic slots and the extra: the keys the others (not slot `t`) are
    /// dialing or connected to.
    fn excluded_key(&self,k:Key,t:usize)->bool{
        self.disc.out.slots.iter().enumerate().any(|(i,s)|i!=t&&(s.role==Role::Auto||s.kind()==Some(SlotKind::Extra))&&s.addr().is_some_and(|a|key_of(a)==k))
    }
    /// D25: the extra goes to a new source: not the source of an entry an automatic slot is connected to.
    fn extra_source_ok(&self,e:&addrbook::Entry)->bool{
        !self.disc.out.slots.iter().any(|s|s.role==Role::Auto&&matches!(s.state,SlotState::Up{addr,..} if self.disc.book.get(addr).is_some_and(|x|x.source==e.source)))
    }
    /// `Book::select` with the node's generator, relaxed while the node has no compatible peer (B4).
    fn pick(&mut self,class:Class,ok:impl Fn(&Node,&addrbook::Entry)->bool,now:u64)->Option<SocketAddr>{
        let mut rng=self.disc.rng.clone();let relax=self.peer_status.is_empty();
        let r=self.disc.book.select(class,|e|ok(self,e),&mut rng,now,relax);self.disc.rng=rng;r
    }
    /// The wait after k consecutive failures: min(backoff_base_ms·2^(k-1), backoff_max_ms).
    fn backoff(&self,fails:u32)->u64{let t=&self.disc.times;t.backoff_base_ms.saturating_mul(1u64.checked_shl(fails.saturating_sub(1)).unwrap_or(u64::MAX)).min(t.backoff_max_ms)}
    /// A slot's attempt failed: one more failure, idle for the backoff.
    fn slot_fail(&mut self,t:usize,now:u64){let s=&mut self.disc.out.slots[t];s.fails=s.fails.saturating_add(1);let f=s.fails;self.disc.out.slots[t].state=SlotState::Idle{next_ms:now.saturating_add(self.backoff(f))};}
    /// §2.1: slot `t` dials `a`: the entry's last dial is now, and the address goes to the slot's thread. A full or
    /// closed channel (the thread is gone) leaves the slot idle for the backoff's base.
    pub(super) fn assign(&mut self,t:usize,a:SocketAddr,kind:SlotKind,now:u64){
        self.disc.book.tried(a,now);let base=self.disc.times.backoff_base_ms;let o=&mut self.disc.out;
        o.counters.dials+=1;match kind{SlotKind::Anchor=>o.counters.anchors_dialed+=1,SlotKind::Feeler=>o.counters.feelers+=1,SlotKind::Extra=>o.counters.extras+=1,SlotKind::Auto=>{}}
        let sent=o.tx.get(t).is_some_and(|tx|tx.try_send(a).is_ok());
        let s=&mut o.slots[t];s.starved=false;s.state=if sent{SlotState::Dialing{addr:a,kind,since_ms:now}}else{SlotState::Idle{next_ms:now.saturating_add(base)}};
    }
    /// §2.1: an idle automatic slot takes a pending anchor (once per process, whatever its retry spacing), else the
    /// book's choice of any class, never an excluded address or key. Nothing: starved, and it looks again after the
    /// backoff's base.
    fn fill(&mut self,t:usize,now:u64){
        let anchor=self.disc.out.anchors_left.iter().position(|a|!self.excluded_addr(*a)&&!self.excluded_key(key_of(*a),t));
        if let Some(i)=anchor{if let Some(a)=self.disc.out.anchors_left.remove(i){return self.assign(t,a,SlotKind::Anchor,now)}}
        match self.pick(Class::Any,|n,e|!n.excluded_addr(e.addr)&&!n.excluded_key(e.key,t),now){
            Some(a)=>self.assign(t,a,SlotKind::Auto,now),
            None=>{let next=now.saturating_add(self.disc.times.backoff_base_ms);let s=&mut self.disc.out.slots[t];s.starved=true;s.state=SlotState::Idle{next_ms:next};}
        }
    }
    /// D18': a family that is down is probed again every probe_ms (`net::route_ok`, a UDP connect that sends nothing);
    /// a probe that succeeds brings it back and clears the seeds' holds.
    fn probe_families(&mut self,now:u64){
        let every=self.disc.times.probe_ms;
        for f in 0..2{
            let Some((a,at))=self.disc.out.family_down[f] else{continue};
            if now.saturating_sub(at)<every{continue}
            if self.route(a){self.disc.out.family_down[f]=None;self.seeds_online()}else{self.disc.out.family_down[f]=Some((a,now))}
        }
    }
    /// From `disc_tick`, every tick_ms: the family probes, the stall and quiet edges, the seed policy, then each idle
    /// automatic slot whose backoff ended is filled, and the spare is used (§2.4, §2.6).
    pub(super) fn out_tick(&mut self,now:u64){
        if self.disc.out.slots.is_empty(){return}
        self.probe_families(now);
        let (stalled,quiet)=(self.stalled(now),self.quiet(now));let o=&mut self.disc.out;
        if stalled&&!o.stalled{o.counters.stalls+=1}if quiet&&!o.quiet{o.counters.quiets+=1}
        (o.stalled,o.quiet)=(stalled,quiet);
        if stalled||quiet{if o.trouble_since.is_none(){o.trouble_since=Some(now);o.next_extra_ms=now;}}else{o.trouble_since=None}
        let healthy=self.healthy(now);let o=&mut self.disc.out;
        if healthy>=2{o.healthy_since.get_or_insert(now);}else{o.healthy_since=None}
        self.seed_policy(now,stalled,healthy);
        for t in 0..self.disc.out.slots.len(){
            let s=&self.disc.out.slots[t];
            if s.role==Role::Auto&&matches!(s.state,SlotState::Idle{next_ms} if next_ms<=now){self.fill(t,now)}
        }
        self.spare(now,stalled||quiet);
    }
    /// The spare slot (§2.4, §2.6). Up as the extra: closed once the trouble ended or W after it came up without
    /// moving the head. Idle while stalled or quiet: the extra, at the trouble's rising edge and every W while it
    /// lasts. Idle otherwise: a feeler when the automatic slots are settled (each up or starved, none dialing) and its
    /// interval passed, to an unverified entry (`NewOnly`), not self-advertised once VERIFIED_INBOUND_MAX such entries
    /// are verified.
    fn spare(&mut self,now:u64,trouble:bool){
        let Some(t)=self.disc.out.spare() else{return};let w=self.window_ms();
        match self.disc.out.slots[t].state{
            SlotState::Up{peer,kind:SlotKind::Extra,since_ms,..}=>{
                if (!trouble||now.saturating_sub(since_ms)>=w)&&self.disc.links.get(&peer).is_some_and(|l|!l.closed_by_us){
                    self.disc_closed_by_us(peer);if let Some(p)=self.peers.get(&peer){p.close();}self.disc.out.counters.extra_closed+=1;
                }
            }
            SlotState::Idle{next_ms} if next_ms<=now=>{
                if trouble{
                    if now<self.disc.out.next_extra_ms{return}
                    self.disc.out.next_extra_ms=now.saturating_add(w);
                    match self.extra_addr(t,now){Some(a)=>self.assign(t,a,SlotKind::Extra,now),None=>self.disc.out.counters.extra_starved+=1}
                }else if self.feeler_due(now){
                    let cap=self.disc.book.verified_inbound()>=addrbook::VERIFIED_INBOUND_MAX;
                    if let Some(a)=self.pick(Class::NewOnly,|n,e|!n.excluded_addr(e.addr)&&!(cap&&e.source==Source::Inbound),now){
                        let ti=&self.disc.times;let base=if self.disc.listening{ti.feeler_listen_ms}else{ti.feeler_quiet_ms};let jitter=self.disc.rng.below(ti.feeler_jitter_ms);
                        self.disc.out.next_feeler_ms=now.saturating_add(base).saturating_add(jitter);self.assign(t,a,SlotKind::Feeler,now);
                    }
                }
            }
            _=>{}
        }
    }
    /// §2.4: the automatic slots are settled (none dialing; each up or starved) and the feeler's interval passed.
    fn feeler_due(&self,now:u64)->bool{
        let o=&self.disc.out;
        now>=o.next_feeler_ms&&o.slots.iter().filter(|s|s.role==Role::Auto).all(|s|matches!(s.state,SlotState::Up{..})||(s.starved&&matches!(s.state,SlotState::Idle{..})))
    }
    /// D25, D17: the extra's address. While no seed can be wanted (every one held or this node, or none), the next
    /// bootstrap address in turn that is not excluded and may be dialed (or was last tried retry_verified_ms ago);
    /// else the book's choice, not excluded, from a source no automatic slot is connected to, failing that from any.
    fn extra_addr(&mut self,t:usize,now:u64)->Option<SocketAddr>{
        let o=&self.disc.out;let n=o.bootstrap.len();
        if n>0&&!o.seeds.iter().any(|s|!s.is_self&&s.hold_until_ms<=now){
            let relax=self.peer_status.is_empty();let b=&self.disc.book;
            for i in 0..n{
                let a=o.bootstrap[(o.cursor_b+i)%n];
                if self.excluded_addr(a)||self.excluded_key(key_of(a),t){continue}
                if b.get(a).map_or(true,|e|b.dialable(e,now,relax)||now.saturating_sub(e.last_try_ms)>=self.disc.times.retry_verified_ms){self.disc.out.cursor_b=(o.cursor_b+i+1)%n;return Some(a)}
            }
        }
        self.pick(Class::Any,|n,e|!n.excluded_addr(e.addr)&&!n.excluded_key(e.key,t)&&n.extra_source_ok(e),now)
            .or_else(||self.pick(Class::Any,|n,e|!n.excluded_addr(e.addr)&&!n.excluded_key(e.key,t),now))
    }
    /// The source slot `t`'s verified address takes should it have left the book while dialed (`Book::good_dialed`):
    /// a bootstrap address its own `Cfg`, an anchor its own key (the node saved it), anything else none, so that an
    /// entry a slot or feeler took from the book never becomes a source of its own.
    pub(super) fn slot_source(&self,t:usize,a:SocketAddr)->Option<Source>{
        let (a,o)=(canon(a),&self.disc.out);
        if o.bootstrap.contains(&a){Some(Source::Cfg(a))}else if o.slots.get(t).and_then(Slot::kind)==Some(SlotKind::Anchor){Some(Source::Peer(key_of(a)))}else{None}
    }
    /// A connection slot `t` dialed to `a` (`Origin::Slot`): kept only while the slot is dialing that address (else it
    /// is closed, `slotsStale`). Its family is up again. A feeler is kept here, out of `peers`; any other is a peer.
    pub(super) fn slot_connected(&mut self,id:PeerId,p:&net::Peer,t:usize,a:SocketAddr,now:u64)->Arrival{
        let o=&mut self.disc.out;
        let Some(SlotState::Dialing{addr,kind,..})=o.slots.get(t).map(|s|s.state) else{o.counters.slots_stale+=1;return Arrival::Close};
        if addr!=a{o.counters.slots_stale+=1;return Arrival::Close}
        o.slots[t].state=SlotState::Up{peer:id,addr:a,kind,since_ms:now};self.family_up(Some(a));let o=&mut self.disc.out;
        if kind==SlotKind::Feeler{o.feeler=Some(Feeler{id,peer:p.clone(),hello:false});return Arrival::Feeler}
        self.disc.links.insert(id,Link::new(p));Arrival::Peer
    }
    /// §2.3 (D27): slot `t`'s dial of `a` failed, taken only while it dials `a`. A remote failure (refused, timed out,
    /// unreachable from the network) is a book failure and a slot failure; no local route marks the family down and
    /// waits the backoff's base, nothing else; a connection not handed over is the slot's failure only. A feeler goes
    /// idle without backoff; an anchor that failed is not saved again.
    pub(in crate::node) fn disc_dial_failed(&mut self,t:u8,a:SocketAddr,e:DialError,now:u64){
        let t=usize::from(t);
        let Some(SlotState::Dialing{addr,kind,..})=self.disc.out.slots.get(t).map(|s|s.state) else{return};
        if addr!=a{return}
        if kind==SlotKind::Anchor{self.anchor_bad(a)}
        match e{
            DialError::Refused|DialError::Timeout|DialError::Unreachable=>{self.disc.book.failed(a,now);self.disc.out.counters.dial_failed+=1}
            DialError::NoRoute=>{let o=&mut self.disc.out;o.family_down[family(a)]=Some((a,now));o.counters.dial_no_route+=1;
                o.slots[t].state=SlotState::Idle{next_ms:now.saturating_add(self.disc.times.backoff_base_ms)};return}
            DialError::NotHanded=>self.disc.out.counters.dial_failed+=1,
        }
        if kind==SlotKind::Feeler{let s=&mut self.disc.out.slots[t];s.fails=0;s.state=SlotState::Idle{next_ms:now}}else{self.slot_fail(t,now)}
    }
    /// §2.3: slot `t`'s connection `id` to `a` ended (`l`: its link; None for the feeler). A self-connection: idle at
    /// once (its address is marked as this node's). Incompatible: a failure (the Hello already held or removed the
    /// entry). No Hello at all (closed, a non-Hello first frame, the Hello deadline): an unverified entry's failure, a
    /// verified one only busy (a full listener). Compatible and alive stable_ms, not closed by this node: its
    /// verification refreshed, the slot's failures cleared and refilled at once. Compatible but short, or closed by
    /// this node: busy only, never touched (a touch would date the verification after the dial and make the address
    /// dialable at once), and a failure. The feeler: a failure for the entry without a Hello; idle, no backoff.
    pub(super) fn slot_closed(&mut self,id:PeerId,t:usize,a:SocketAddr,l:Option<&Link>,now:u64){
        let Some(SlotState::Up{peer,kind,since_ms,..})=self.disc.out.slots.get(t).map(|s|s.state) else{return};
        if peer!=id{return}
        if kind==SlotKind::Feeler{
            if !self.disc.out.feeler.take().is_some_and(|f|f.hello){self.disc.book.failed(a,now)}
            let s=&mut self.disc.out.slots[t];s.fails=0;s.state=SlotState::Idle{next_ms:now};return
        }
        let Some(l)=l else{return};
        let lived=now.saturating_sub(since_ms)>=self.disc.times.stable_ms;
        if kind==SlotKind::Anchor&&(l.self_conn||l.mismatch.is_some()||!l.compatible||!lived){self.anchor_bad(a)}
        let c=&mut self.disc.out.counters;
        if l.self_conn{self.disc.out.slots[t].state=SlotState::Idle{next_ms:now};return}
        if l.mismatch.is_some(){c.dial_incompatible+=1}
        else if !l.compatible{c.dial_no_hello+=1;if self.disc.book.get(a).is_some_and(|e|e.verified_ms>0){self.disc.book.busy(a,now)}else{self.disc.book.failed(a,now)}}
        else if lived&&!l.closed_by_us{self.disc.book.touch(a,false,now);let s=&mut self.disc.out.slots[t];s.fails=0;s.state=SlotState::Idle{next_ms:now};return}
        else{c.dial_busy+=1;self.disc.book.busy(a,now)}
        self.slot_fail(t,now);
    }
    /// D12' (VA-SPEC §1a.7): the feeler's first frame, its peer's Hello. Our nonce: the address is this node's own.
    /// Another network: forgotten; another release: held. Compatible with a listener: verified (`feelersOk`), and with
    /// listenPort 0 forgotten. Then the feeler closes: it sent nothing, so the peer never established it and evicted
    /// nobody for it (`net::Net::establish` runs only on a Hello it receives).
    pub(in crate::node) fn disc_feeler_hello(&mut self,from:PeerId,m:Msg,now:u64){
        let Some(f)=self.disc.out.feeler.as_mut().filter(|f|f.id==from&&!f.hello) else{return};
        f.hello=true;let peer=f.peer.clone();
        let Some((t,a))=self.disc.out.slots.iter().enumerate().find(|(_,s)|s.peer()==Some(from)).and_then(|(t,s)|Some((t,s.addr()?))) else{peer.close();return};
        if let Msg::Hello{chain_id,genesis,protocol,compatibility,listen_port,nonce,..}=m{
            let (ours,src)=(self.disc.wire_nonce(),self.slot_source(t,a));let (b,c)=(&mut self.disc.book,&mut self.disc.out.counters);
            if nonce!=0&&nonce==ours{b.mark_self(a)}
            else{match crate::identity::accepts(&self.consensus,chain_id,&genesis,protocol,&compatibility){
                Err(crate::identity::Mismatch::Network)=>{b.remove(a);c.feelers_incompatible+=1}
                Err(_)=>{b.held(a,now);c.feelers_incompatible+=1}
                Ok(_) if listen_port!=0=>{b.good_dialed(a,src,net::group_of(a.ip()).is_none(),now);c.feelers_ok+=1}
                Ok(_)=>b.remove(a)}}
        }
        peer.close();
    }
    /// D26: peer `from` delivered something first (a block within 2 of the head, or a block of its sync session; or a
    /// template that locked).
    pub(in crate::node) fn disc_useful(&mut self,from:PeerId,block:bool,now:u64){
        if let Some(l)=self.disc.links.get_mut(&from){l.useful_ms=now;if block{l.blocks=l.blocks.saturating_add(1)}}
    }
    /// A block from peer `from` moved the head. While stalled or quiet and `from` is the extra (D25), it takes the
    /// place of an automatic slot: one without a connection, else the one useful longest ago (ties: the lowest
    /// index), never the sync session's peer, which is closed by this node (`busy`, no failure for its entry); the two
    /// slots exchange roles and the extra's connection becomes an automatic one (`extraSwaps`).
    pub(in crate::node) fn disc_head(&mut self,from:PeerId,now:u64){
        self.disc.out.last_peer_head_ms=now;
        let o=&self.disc.out;if o.trouble_since.is_none(){return}
        let Some(x)=o.slots.iter().position(|s|s.role==Role::Spare&&matches!(s.state,SlotState::Up{peer,kind:SlotKind::Extra,..} if peer==from)) else{return};
        let session=self.session.as_ref().map(|s|s.peer);
        let victim=o.slots.iter().enumerate().filter(|(_,s)|s.role==Role::Auto).filter_map(|(i,s)|match s.state{
            SlotState::Up{peer,..}=>(Some(peer)!=session).then(||(1u8,self.disc.links.get(&peer).map_or(0,|l|l.useful_ms),i,Some(peer))),
            SlotState::Idle{..}=>Some((0,0,i,None)),SlotState::Dialing{..}=>None}).min();
        let Some((_,_,v,peer))=victim else{return};
        if let Some(p)=peer{self.disc_closed_by_us(p);if let Some(c)=self.peers.get(&p){c.close();}}
        let o=&mut self.disc.out;o.slots[v].role=Role::Spare;o.slots[x].role=Role::Auto;
        if let SlotState::Up{kind,..}=&mut o.slots[x].state{*kind=SlotKind::Auto}
        o.counters.extra_swaps+=1;
    }
    /// This node closes `from` on purpose (a swap, the extra's end, an invalid template): its end is no failure of
    /// the address, and it is not refreshed either (`slot_closed`).
    pub(in crate::node) fn disc_closed_by_us(&mut self,from:PeerId){if let Some(l)=self.disc.links.get_mut(&from){l.closed_by_us=true}}
    /// `pocol_status.discovery.outbound` (plan §6): settings, health and the stall state, and per slot its role,
    /// kind, state, address, key, uptime and failures; arrays bounded (slots <= OUTBOUND_MAX + 1, seeds <= SEEDS_MAX,
    /// anchors <= ANCHORS_MAX), never the book's contents.
    pub(super) fn out_status(&self,now:u64)->serde_json::Value{
        let o=&self.disc.out;
        let name=|k:SlotKind|match k{SlotKind::Auto=>"auto",SlotKind::Anchor=>"anchor",SlotKind::Feeler=>"feeler",SlotKind::Extra=>"extra"};
        let key=|a:SocketAddr|match key_of(a){Key::Net(net::Group::V4([x,y]))=>format!("{x}.{y}/16"),Key::Net(net::Group::V6(g))=>format!("{:x}:{:x}/32",u16::from_be_bytes([g[0],g[1]]),u16::from_be_bytes([g[2],g[3]])),Key::Local(_)=>"local".into()};
        let slots:Vec<serde_json::Value>=o.slots.iter().enumerate().map(|(i,s)|{
            let (state,since)=match s.state{SlotState::Idle{..}=>("idle",None),SlotState::Dialing{..}=>("dialing",None),SlotState::Up{since_ms,..}=>("up",Some(since_ms))};
            serde_json::json!({"index":i,"role":if s.role==Role::Auto{"auto"}else{"spare"},"kind":s.kind().map(name),"state":state,"addr":s.addr().map(|a|a.to_string()),
                "key":s.addr().map(key),"upSeconds":since.map(|t|now.saturating_sub(t)/1000),"fails":s.fails})}).collect();
        let seeds:Vec<serde_json::Value>=o.seeds.iter().map(|s|serde_json::json!({"addr":s.addr.to_string(),"state":self.seed_state(s,now),"fails":s.fails})).collect();
        let head_age=now.saturating_sub(self.chain.head().ts.saturating_mul(1000))/1000;
        serde_json::json!({"automatic":o.k,"seedTarget":o.seeds.iter().filter(|s|s.wanted_ms.is_some()).count(),"healthy":self.healthy(now),"starved":self.starved(),"stalled":self.stalled(now),"quiet":self.quiet(now),
            "headAgeSeconds":head_age,"peerHeadAgeSeconds":now.saturating_sub(o.last_peer_head_ms)/1000,"seedNode":o.seed_node,"seedsRetired":self.cfg.limits.discovery.seeds_retired,
            "familyDown":{"v4":o.family_down[0].is_some(),"v6":o.family_down[1].is_some()},"windowSeconds":self.window_ms()/1000,
            "slots":slots,"seeds":seeds,"anchors":o.saved_anchors.iter().map(|a|a.0.to_string()).collect::<Vec<_>>(),"bootstrap":o.bootstrap.len()})
    }
    /// A seed's state in the status: this node itself, held, connected, wanted and dialing, or suppressed (not
    /// dialed). Without automatic slots the seeds are configured peers like the others: connected or dialing.
    fn seed_state(&self,s:&Seed,now:u64)->&'static str{
        if s.is_self{"self"}else if s.hold_until_ms>now{"held"}else if s.peer.is_some(){"connected"}
        else if s.wanted_ms.is_some()||self.disc.out.slots.is_empty(){"dialing"}else{"suppressed"}
    }
    /// The counters of `pocol_status.discovery.counters` that automatic outbound keeps (plan §6).
    pub(super) fn out_counters(&self)->serde_json::Value{
        let c=&self.disc.out.counters;
        serde_json::json!({"dials":c.dials,"dialFailed":c.dial_failed,"dialNoHello":c.dial_no_hello,"dialBusy":c.dial_busy,"dialIncompatible":c.dial_incompatible,"dialNoRoute":c.dial_no_route,
            "slotsStale":c.slots_stale,"feelers":c.feelers,"feelersOk":c.feelers_ok,"feelersIncompatible":c.feelers_incompatible,"anchorsDialed":c.anchors_dialed,"extras":c.extras,
            "extraSwaps":c.extra_swaps,"extraClosed":c.extra_closed,"extraStarved":c.extra_starved,"seedWants":c.seed_wants,"seedReleases":c.seed_releases,"seedRotations":c.seed_rotations,
            "seedHolds":c.seed_holds,"stalls":c.stalls,"quiets":c.quiets,"bootstrapLoaded":c.bootstrap_loaded})
    }
    /// D26 (VA-SPEC §1a.7): the anchors to save. While stalled, the set last saved stands (less any whose dial failed in
    /// this process). Otherwise the automatic
    /// slots' connections that are compatible, up anchor_uptime_ms, delivered a block and were useful within W, one per
    /// key, longest-lived first, saved now; then, while fewer than ANCHORS_MAX, the loaded anchors of other keys within
    /// anchor_keep_ms of their save, but none dialed in this process whose connection failed or ended short.
    pub(super) fn disc_anchors(&self,now:u64)->Vec<(SocketAddr,u64)>{
        if self.stalled(now)&&!self.disc.out.slots.is_empty(){let o=&self.disc.out;return o.saved_anchors.iter().filter(|a|!o.anchors_bad.contains(&a.0)).copied().collect()}
        let (o,w,t)=(&self.disc.out,self.window_ms(),&self.disc.times);
        let mut up:Vec<(u64,SocketAddr)>=o.slots.iter().filter(|s|s.role==Role::Auto).filter_map(|s|match s.state{
            SlotState::Up{peer,addr,since_ms,..}=>self.disc.links.get(&peer).filter(|l|l.compatible&&l.blocks>=1&&l.useful_ms.saturating_add(w)>=now&&now.saturating_sub(since_ms)>=t.anchor_uptime_ms).map(|_|(since_ms,addr)),
            _=>None}).collect();
        up.sort_unstable();
        let mut out:Vec<(SocketAddr,u64)>=Vec::with_capacity(addrbook::ANCHORS_MAX);
        for (_,a) in up{if out.len()<addrbook::ANCHORS_MAX&&!out.iter().any(|x|key_of(x.0)==key_of(a)){out.push((a,now))}}
        for (a,ms) in &o.loaded_anchors{
            if out.len()<addrbook::ANCHORS_MAX&&now.saturating_sub(*ms)<=t.anchor_keep_ms&&!o.anchors_bad.contains(a)&&!out.iter().any(|x|key_of(x.0)==key_of(*a)){out.push((*a,*ms))}
        }
        out
    }
    /// A loaded anchor dialed in this process failed, was incompatible or ended short: never saved again. Each anchor is
    /// dialed once, so the list holds at most ANCHORS_MAX.
    fn anchor_bad(&mut self,a:SocketAddr){let b=&mut self.disc.out.anchors_bad;if !b.contains(&a)&&b.len()<addrbook::ANCHORS_MAX{b.push(a)}}
    /// The anchors to save are other addresses than the set last saved (`disc_tick` then saves at once).
    pub(super) fn anchors_moved(&self,now:u64)->bool{let a=self.disc_anchors(now);a.len()!=self.disc.out.saved_anchors.len()||a.iter().zip(&self.disc.out.saved_anchors).any(|(x,y)|x.0!=y.0)}
    /// VA-SPEC §1a.7, at the network's start after the listener and before the configured dialers: the K + 1 slot
    /// threads (`net::slot`, the spare last) and the one forwarder that hands their events to the node. Nothing with
    /// K = 0 or discovery off.
    pub(in crate::node) fn disc_start_outbound(&mut self,now:u64)->Result<(),String>{
        let k=self.disc.out.k;if k==0||!self.disc.on{return Ok(())}
        let tx=self.tx.clone();let (otx,orx)=std::sync::mpsc::sync_channel(net::NET_EVENT_QUEUE);
        crate::threads::spawn("out-fwd",crate::threads::IO_STACK,move||{for e in orx{if tx.send(crate::node::Event::Net(e)).is_err(){break}}}).map_err(|e|format!("spawn: {e}"))?;
        for t in 0..=k{let (stx,srx)=std::sync::mpsc::sync_channel(1);net::slot(t as u8,srx,otx.clone(),self.net.clone()).map_err(|e|format!("spawn: {e}"))?;
            self.disc.out.tx.push(stx);self.disc.out.slots.push(Slot{role:if t==k{Role::Spare}else{Role::Auto},state:SlotState::Idle{next_ms:now},fails:0,starved:false});}
        self.seeds_start(now);Ok(())
    }
    /// VA-SPEC §1a.8, when the slots start (before any configured dialer): every seed is suppressed, its address put
    /// into the outbound set so that its dialer sees it as live and waits until the policy wants it; the first seed
    /// to want is drawn at random.
    fn seeds_start(&mut self,now:u64){
        let o=&mut self.disc.out;o.started_ms=now;o.last_peer_head_ms=now;
        for s in &o.seeds{o.suppressed.insert(s.addr);if let Ok(mut set)=self.outbound.lock(){set.insert(s.dial.to_string());}}
        o.cursor=self.disc.rng.below(o.seeds.len() as u64) as usize;
    }
    /// Seed `i` is wanted: its dialer may dial it (it leaves the suppressed and the outbound set).
    fn want(&mut self,i:usize,now:u64){
        let o=&mut self.disc.out;let s=&mut o.seeds[i];s.wanted_ms=Some(now);o.suppressed.remove(&s.addr);o.counters.seed_wants+=1;
        if let Ok(mut set)=self.outbound.lock(){set.remove(&s.dial.to_string());}
    }
    /// Seed `i` is no longer wanted: back into the suppressed and the outbound set, its live connection closed.
    fn suppress(&mut self,i:usize){
        let o=&mut self.disc.out;let s=&mut o.seeds[i];s.wanted_ms=None;o.suppressed.insert(s.addr);
        if let Ok(mut set)=self.outbound.lock(){set.insert(s.dial.to_string());}
        if let Some(p)=s.peer.and_then(|id|self.peers.get(&id)){p.close();}
    }
    /// VA-SPEC §1a.8, D27: the slots that can be filled without a seed: the distinct keys of automatic slots dialing or
    /// up (B3: an assigned entry is no longer dialable), of verified entries outside the exclusions and those keys that
    /// may be dialed by their retry spacing (not relaxed: one that just failed does not count), and the pending anchors.
    pub(super) fn fillable(&self,now:u64)->usize{
        let o=&self.disc.out;
        let slots=distinct(o.slots.iter().filter(|s|s.role==Role::Auto).filter_map(Slot::addr).map(key_of));
        slots+self.disc.book.verified_keys(|e|!self.excluded_addr(e.addr)&&!self.excluded_key(e.key,usize::MAX),now,false)+o.anchors_left.len()
    }
    /// D7'' (VA-SPEC §1a.8), each tick with automatic outbound: how many seeds to want, and the one change a tick
    /// toward it. A seed node wants every other seed. During the boot window: none while the slots can be filled
    /// without one (one without a listener, D7'a, so that it does not release and want again at the window's end), else
    /// up to SEED_LINKS_MAX. Then one while stalled, healthy < 2 (or for less than
    /// seed_release_ms), starved or not listening (D7'a), else none. A wanted seed not compatible seed_connect_ms after
    /// it was wanted is held (1 min doubling to 6 h) and the next is wanted. One beyond the target is released: one not
    /// connected first, else one compatible seed_hold_ms whose GetAddr is answered or expired and that is not the sync
    /// peer, the most recently wanted first. While stalled, a wanted seed compatible for W without session progress is
    /// replaced by the next, at most once per W, never while the session progresses.
    fn seed_policy(&mut self,now:u64,stalled:bool,healthy:usize){
        if self.disc.out.seeds.is_empty(){return}
        let t=self.disc.times;
        // Holds first: a wanted seed that did not become compatible in time (a local failure only for the base).
        for i in 0..self.disc.out.seeds.len(){
            let s=&self.disc.out.seeds[i];
            if s.compatible_ms.is_none()&&s.wanted_ms.is_some_and(|w|now.saturating_sub(w)>=t.seed_connect_ms){
                let local=self.local_failure(s.addr);let s=&mut self.disc.out.seeds[i];
                let hold=if local{t.seed_hold_base_ms}else{s.fails=s.fails.saturating_add(1);t.seed_hold_base_ms.saturating_mul(1u64.checked_shl(s.fails-1).unwrap_or(u64::MAX))};
                s.hold_until_ms=now.saturating_add(hold.min(t.seed_hold_max_ms));self.disc.out.counters.seed_holds+=1;self.suppress(i);
            }
        }
        let o=&self.disc.out;let eligible=|s:&Seed|!s.is_self&&s.hold_until_ms<=now;
        let n_eligible=o.seeds.iter().filter(|s|eligible(s)).count();
        if o.seed_node{for i in 0..o.seeds.len(){let s=&self.disc.out.seeds[i];if eligible(s)&&s.wanted_ms.is_none(){self.want(i,now)}}return}
        let target=if now<o.started_ms.saturating_add(t.seed_boot_ms){if self.fillable(now)>=o.k{usize::from(!self.disc.listening)}else{super::SEED_LINKS_MAX}}
            else{usize::from(stalled||healthy<2||self.starved()||!self.disc.listening||o.healthy_since.is_none_or(|h|now.saturating_sub(h)<t.seed_release_ms))};
        let target=target.min(super::SEED_LINKS_MAX).min(n_eligible);
        let wanted=o.seeds.iter().filter(|s|s.wanted_ms.is_some()).count();
        let session=self.session.as_ref().map(|s|s.peer);
        if wanted>target{
            let releasable=|s:&Seed|s.peer.is_none_or(|p|s.compatible_ms.is_some_and(|c|now.saturating_sub(c)>=t.seed_hold_ms)&&Some(p)!=session
                &&self.disc.links.get(&p).is_none_or(|l|l.asked_ms.is_none()));
            let pick=o.seeds.iter().enumerate().filter(|(_,s)|s.wanted_ms.is_some()&&releasable(s)).max_by_key(|(_,s)|(s.peer.is_none(),s.wanted_ms)).map(|(i,_)|i);
            if let Some(i)=pick{self.suppress(i);self.disc.out.counters.seed_releases+=1;}
        }else{for _ in wanted..target{self.want_next(now);}}
        // Stall rotation (B5): never the session's peer, never while the session progresses.
        let o=&self.disc.out;let w=self.window_ms();
        let progressing=self.session.as_ref().is_some_and(|s|now_ms().saturating_sub(s.last_ms)<crate::node::SESSION_STALL_MS);
        if stalled&&n_eligible>=2&&!progressing&&now.saturating_sub(o.last_rotation_ms)>=w{
            if let Some(i)=o.seeds.iter().position(|s|s.wanted_ms.is_some()&&s.compatible_ms.is_some_and(|c|now.saturating_sub(c)>=w)&&s.peer.is_some()&&s.peer!=session){
                self.suppress(i);self.disc.out.last_rotation_ms=now;self.disc.out.counters.seed_rotations+=1;self.want_next(now);
            }
        }
    }
    /// Wants the next eligible seed not wanted, in turn from `cursor`.
    fn want_next(&mut self,now:u64){
        let o=&self.disc.out;let n=o.seeds.len();
        let next=(0..n).map(|j|(o.cursor+j)%n).find(|&i|{let s=&o.seeds[i];!s.is_self&&s.hold_until_ms<=now&&s.wanted_ms.is_none()});
        if let Some(i)=next{self.disc.out.cursor=(i+1)%n;self.want(i,now)}
    }
    /// Tests: installs the K + 1 slots without threads (the spare last) and returns the receiving end of each slot's
    /// channel, where the assigned addresses arrive (64 of them may wait, so a test need not take each one).
    #[cfg(test)]
    pub(crate) fn disc_test_slots(&mut self,now:u64)->Vec<std::sync::mpsc::Receiver<SocketAddr>>{
        let k=self.disc.out.k;let mut rx=Vec::new();
        for t in 0..=k{let (stx,srx)=std::sync::mpsc::sync_channel(64);self.disc.out.tx.push(stx);rx.push(srx);
            self.disc.out.slots.push(Slot{role:if t==k{Role::Spare}else{Role::Auto},state:SlotState::Idle{next_ms:now},fails:0,starved:false});}
        self.seeds_start(now);rx
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use super::super::tests::{open_peers,sa,hello,connect,msg,greet,closed};
    use crate::node::{Event,NetEvent,Outbound,Discovery,DiscoveryTimes,now_ms};
    use crate::addrbook::Kind;
    use std::sync::mpsc::Receiver;
    /// A clock one second after the devnet genesis's timestamp: the head is young, so the node is not stalled.
    const G:u64=1_700_000_001_000;
    const S:u64=1_000;
    /// A node with K automatic slots (`peers` configured, `seeds` among them), its clock at `now`, the slots installed
    /// without threads (the spare last) and their channels.
    fn node(tag:&str,k:u8,listen:bool,peers:Vec<SocketAddr>,seeds:Vec<SocketAddr>,now:u64)->(Node,Vec<Receiver<SocketAddr>>,std::path::PathBuf){
        let (mut n,dir)=opened(tag,Discovery{outbound:Outbound::Count(k),seeds,rng_seed:Some(7),..Discovery::default()},listen,peers,now);let rx=n.disc_test_slots(now);(n,rx,dir)
    }
    /// The node of `node` with these settings, before its slots are installed.
    fn opened(tag:&str,d:Discovery,listen:bool,peers:Vec<SocketAddr>,now:u64)->(Node,std::path::PathBuf){
        let (n,dir)=open_peers(tag,listen,d,peers);let mut n=n.unwrap();n.disc.test_now=Some(now);(n,dir)
    }
    fn tick(n:&mut Node,now:u64){n.disc.test_now=Some(now);n.out_tick(now)}
    fn state(n:&Node,t:usize)->SlotState{n.disc.out.slots[t].state}
    fn next(n:&Node,t:usize)->Option<u64>{match state(n,t){SlotState::Idle{next_ms}=>Some(next_ms),_=>None}}
    /// A verified entry (a handshake this node dialed, at `now`).
    fn verified(n:&mut Node,a:SocketAddr,now:u64){n.disc.book.good(a,false,false,now);}
    /// An unverified entry heard from the configured peer `from`.
    fn heard(n:&mut Node,a:SocketAddr,from:SocketAddr,now:u64){n.disc.book.insert_unverified(a,Source::Cfg(from),Kind::Heard,false,now);}
    /// Slot `t` dials `a` and its connection `id` comes up; with `hello` the peer's Hello (a listener) arrives too.
    fn up(n:&mut Node,t:usize,a:SocketAddr,id:PeerId,hello:bool,now:u64)->std::net::TcpStream{
        n.disc.test_now=Some(now);n.assign(t,a,SlotKind::Auto,now);let c=connect(n,id,Origin::Slot(t as u8,a),Some(a));if hello{greet(n,id,30300,0)}c
    }
    fn close(n:&mut Node,id:PeerId,now:u64){n.disc.test_now=Some(now);n.handle(Event::Net(NetEvent::Closed(id)))}
    fn attempts(n:&Node,a:SocketAddr)->u8{n.disc.book.get(a).unwrap().attempts}

    /// VA-SPEC §1a.7, §2.1: each automatic slot takes one address per key, never a configured peer (a seed among
    /// them), an inbound connection's advertised endpoint, nor a key another slot is dialing. Eight slots over ten
    /// allowed keys, one of them twice: eight distinct keys, none excluded.
    #[test] fn automatic_slots_take_one_address_per_key_and_skip_excluded_ones(){
        let (c,seed)=(sa("81.20.0.1:30300"),sa("81.23.0.1:30300"));
        let (mut n,rx,dir)=node("o-excl",8,false,vec![c,seed],vec![seed],G);
        for i in 10..20{verified(&mut n,SocketAddr::new([81,i,0,1].into(),30300),G)}
        verified(&mut n,sa("81.10.0.2:30300"),G);verified(&mut n,c,G);verified(&mut n,seed,G);
        let _i=connect(&mut n,100,Origin::Inbound,Some(sa("81.22.0.1:51000")));greet(&mut n,100,30300,0);let adv=sa("81.22.0.1:30300");verified(&mut n,adv,G);
        tick(&mut n,G);
        let got:Vec<SocketAddr>=rx[..8].iter().map(|r|r.try_recv().expect("every automatic slot dials")).collect();
        let mut keys:Vec<Key>=got.iter().map(|a|key_of(*a)).collect();keys.sort();keys.dedup();
        assert_eq!(keys.len(),8,"one address per key: {got:?}");
        assert!(got.iter().all(|a|![c,seed,adv].contains(a)),"no configured peer, seed or advertised endpoint: {got:?}");
        assert!(rx[8].try_recv().is_err(),"the spare runs no feeler while slots are dialing");
        assert_eq!(n.disc.out.counters.dials,8);
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// §2.3 (D27, D18', VA-SPEC §1a.7): refusals back off 1, 2, 4 … 60 s. A connection without a Hello is a failure of
    /// an unverified entry (counted once between two verifications) and only busy for a verified one; a connection
    /// not handed over marks nothing; no local route skips the family until a probe succeeds, marking nothing; a
    /// remote unreachable is a failure. A compatible connection closed at 59 s is busy (verified spacing applies: no
    /// touch after the dial), one closed at 61 s clears the failures and is dialable at once; another release is held.
    #[test] fn slot_outcomes_set_backoff_and_book_state(){
        let (u,v,w)=(sa("81.30.0.1:30300"),sa("81.31.0.1:30300"),sa("81.32.0.1:30300"));
        let (mut n,_rx,dir)=node("o-out",1,false,vec![],vec![],G);
        heard(&mut n,u,sa("81.1.0.1:30300"),G);verified(&mut n,v,G);
        let mut now=G;let mut waits=vec![];
        for _ in 0..7{n.assign(0,u,SlotKind::Auto,now);n.disc_dial_failed(0,u,DialError::Refused,now);waits.push((next(&n,0).unwrap()-now)/S);now+=S;}
        assert_eq!(waits,[1,2,4,8,16,32,60],"the slot's backoff");
        assert_eq!(attempts(&n,u),1,"failures count once between two verifications");
        n.disc.out.slots[0].fails=0;
        let _c=up(&mut n,0,u,1,false,now);close(&mut n,1,now);verified(&mut n,w,now);
        let _c=up(&mut n,0,u,2,false,now+1);close(&mut n,2,now+1);
        assert_eq!((attempts(&n,u),n.disc.out.counters.dial_no_hello),(2,2),"no Hello: an unverified entry's failure, counted after a verification elsewhere");
        let _c=up(&mut n,0,v,3,false,now);close(&mut n,3,now);
        assert!(attempts(&n,v)==0&&n.disc.book.get(v).unwrap().last_try_ms==now,"no Hello at a verified entry: busy only");
        let before=n.disc.book.get(w).cloned();n.assign(0,w,SlotKind::Auto,now);let tried=n.disc.book.get(w).cloned();
        n.disc_dial_failed(0,w,DialError::NotHanded,now);
        assert!(n.disc.book.get(w).cloned()==tried&&tried.unwrap().attempts==before.unwrap().attempts,"not handed over: no book change");
        let fails=n.disc.out.slots[0].fails;n.disc.out.route_ok=Some(false);n.assign(0,w,SlotKind::Auto,now);n.disc_dial_failed(0,w,DialError::NoRoute,now);
        assert!(n.disc.out.family_down[0].is_some()&&attempts(&n,w)==0&&n.disc.out.slots[0].fails==fails,"no route: IPv4 down, no failure for anyone");
        tick(&mut n,now+S);assert!(n.disc.out.slots[0].starved&&next(&n,0).is_some(),"while IPv4 is down nothing is dialed");
        n.disc.out.route_ok=Some(true);let probe=n.disc.times.probe_ms;tick(&mut n,now+probe);assert!(n.disc.out.family_down[0].is_none(),"a probe that succeeds brings the family back");
        let x=sa("81.33.0.1:30300");heard(&mut n,x,sa("81.1.0.1:30300"),now);verified(&mut n,w,now+2);
        n.disc.out.slots[0].state=SlotState::Idle{next_ms:0};n.assign(0,x,SlotKind::Auto,now+3);n.disc_dial_failed(0,x,DialError::Unreachable,now+3);
        assert_eq!(attempts(&n,x),1,"unreachable from the network is a failure");
        // A compatible peer elsewhere: no relax, the spacing holds.
        let _i=connect(&mut n,50,Origin::Inbound,Some(sa("81.50.0.1:51000")));greet(&mut n,50,0,0);
        n.disc.out.slots[0].state=SlotState::Idle{next_ms:0};let t0=now+10*S;let fails=n.disc.out.slots[0].fails;
        let _c=up(&mut n,0,v,4,true,t0);let t1=t0+59*S;close(&mut n,4,t1);
        let e=n.disc.book.get(v).unwrap().clone();
        assert!(e.verified_ms==t0&&!n.disc.book.dialable(&e,t1+59*S,false)&&n.disc.book.dialable(&e,t1+60*S,false),"short: busy, the verified spacing applies");
        assert_eq!((n.disc.out.slots[0].fails,n.disc.out.counters.dial_busy),(fails+1,1),"and a slot failure");
        n.disc.out.slots[0].state=SlotState::Idle{next_ms:0};let t2=t1+60*S;
        let _c=up(&mut n,0,v,5,true,t2);let t3=t2+61*S;close(&mut n,5,t3);
        let e=n.disc.book.get(v).unwrap().clone();
        assert!(e.verified_ms==t3&&n.disc.book.dialable(&e,t3,false)&&n.disc.out.slots[0].fails==0&&next(&n,0)==Some(t3),"lived: touched, cleared, refilled at once");
        let r=sa("81.34.0.1:30300");verified(&mut n,r,t3);let _c=up(&mut n,0,r,6,false,t3);
        let mut h=hello(&n,30300,0);if let Msg::Hello{protocol,..}=&mut h{*protocol=0;}msg(&mut n,6,h);close(&mut n,6,t3);
        assert_eq!((n.disc.book.get(r).unwrap().holds,n.disc.out.slots[0].fails,n.disc.out.counters.dial_incompatible),(1,1,1),"another release: held once, one failure");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// §2.3: a Connected for a slot that is not dialing that address (a stale one) is closed at once and never a peer.
    #[test] fn a_connected_slot_is_kept_only_while_its_slot_waits_for_it(){
        let (a,b)=(sa("81.30.0.1:30300"),sa("81.31.0.1:30300"));
        let (mut n,_rx,dir)=node("o-stale",2,false,vec![],vec![],G);
        n.assign(1,b,SlotKind::Auto,G);let mut c=connect(&mut n,1,Origin::Slot(1,a),Some(a));
        assert!(closed(&mut c)&&!n.peers.contains_key(&1)&&n.disc.out.counters.slots_stale==1&&n.disc.links.is_empty());
        assert!(matches!(state(&n,1),SlotState::Dialing{addr,..} if addr==b),"the slot still waits for its own dial");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// D12' (VA-SPEC §1a.7): the spare's feeler runs only once the automatic slots are settled, to an unverified
    /// entry; it sends nothing (so the remote never establishes it and evicts nobody), is never a peer, sends no
    /// GetAddr, and reads the remote's Hello: a listener is verified, listenPort 0 forgotten, our nonce marks the
    /// address as ours, another network forgets it, another release holds it. Its interval is 120-150 s with a
    /// listener. With VERIFIED_INBOUND_MAX verified self-advertisements it no longer picks self-advertised entries.
    #[test] fn a_feeler_runs_only_when_the_slots_are_settled_sends_nothing_and_never_becomes_a_peer(){
        let p=sa("81.40.0.1:30300");
        let (mut n,rx,dir)=node("o-feel",1,true,vec![],vec![],G);
        verified(&mut n,p,G);let fs:Vec<SocketAddr>=(0..6).map(|i|SocketAddr::new([81,41+i,0,1].into(),30300)).collect();
        for f in &fs{heard(&mut n,*f,sa("81.1.0.1:30300"),G)}
        n.assign(0,p,SlotKind::Auto,G);tick(&mut n,G);assert!(rx[1].try_recv().is_err(),"no feeler while a slot dials");
        let _p=connect(&mut n,1,Origin::Slot(0,p),Some(p));greet(&mut n,1,30300,0);
        let mut now=G;let mut id=10;
        // Each visit: the feeler connects, the remote's Hello (changed by `edit`) arrives, it closes.
        let mut visit=|n:&mut Node,edit:&dyn Fn(&mut Msg)|->SocketAddr{
            now+=S;id+=1;n.disc.out.next_feeler_ms=0;tick(n,now);let f=rx[1].try_recv().expect("a feeler");
            assert!(fs.contains(&f)&&matches!(state(n,1),SlotState::Dialing{kind:SlotKind::Feeler,..}));
            let mut c=connect(n,id,Origin::Slot(1,f),Some(f));
            let q=n.disc.out.feeler.as_ref().unwrap().peer.queued();assert!(q.is_empty()&&!n.peers.contains_key(&id),"it sends nothing and is no peer");
            let mut h=hello(n,30300,0);edit(&mut h);msg(n,id,h);
            assert!(closed(&mut c)&&!n.peer_status.contains_key(&id)&&n.disc.out.feeler.as_ref().unwrap().peer.queued().is_empty(),"closed after the Hello, no GetAddr");
            n.handle(Event::Net(NetEvent::Closed(id)));assert!(n.disc.out.feeler.is_none()&&next(n,1).is_some());f
        };
        let f=visit(&mut n,&|_|{});assert!(n.disc.book.get(f).is_some_and(|e|e.verified_ms>0)&&n.disc.out.counters.feelers_ok==1,"a listener is verified");
        let f=visit(&mut n,&|h|if let Msg::Hello{listen_port,..}=h{*listen_port=0});assert!(n.disc.book.get(f).is_none(),"listenPort 0 is forgotten");
        let ours=n.disc.wire_nonce();let f=visit(&mut n,&|h|if let Msg::Hello{nonce,..}=h{*nonce=ours});assert!(n.disc.book.selfs().contains(&f),"our nonce: our own address");
        let f=visit(&mut n,&|h|if let Msg::Hello{chain_id,..}=h{*chain_id+=1});assert!(n.disc.book.get(f).is_none(),"another network is forgotten");
        let f=visit(&mut n,&|h|if let Msg::Hello{protocol,..}=h{*protocol=0});assert!(n.disc.book.get(f).is_some_and(|e|e.holds==1),"another release is held");
        assert_eq!(n.disc.out.counters.feelers_incompatible,2);
        // The interval, with a listener: 120 s plus up to 30 s.
        let start=G+10*S;n.disc.out.next_feeler_ms=0;tick(&mut n,start);let _=rx[1].try_recv().unwrap();
        let gap=n.disc.out.next_feeler_ms-start;assert!((120_000..150_000).contains(&gap),"{gap}");
        drop(n);let _=std::fs::remove_dir_all(&dir);
        // The cap: 128 verified self-advertisements, and one unverified entry of each kind.
        let (mut m,rx,dir)=node("o-cap",1,true,vec![],vec![],G);
        let up_peer=sa("81.40.0.1:30300");verified(&mut m,up_peer,G);m.assign(0,up_peer,SlotKind::Auto,G);let _p=connect(&mut m,1,Origin::Slot(0,up_peer),Some(up_peer));greet(&mut m,1,30300,0);
        let selfad=|m:&mut Node,a:SocketAddr|{m.disc.book.insert_unverified(a,Source::Inbound,Kind::Direct,false,G);};
        for i in 0..128u32{let a=SocketAddr::new([82,(i/8) as u8,(i%8) as u8,1].into(),30300);selfad(&mut m,a);verified(&mut m,a,G);}
        assert_eq!(m.disc.book.verified_inbound(),128);
        let (si,hc)=(sa("83.1.0.1:30300"),sa("83.2.0.1:30300"));selfad(&mut m,si);heard(&mut m,hc,sa("81.1.0.1:30300"),G);
        for i in 0..20{m.disc.out.next_feeler_ms=0;tick(&mut m,G+i*S);if let Ok(f)=rx[1].try_recv(){assert_eq!(f,hc,"only another source");
            m.disc.out.slots[1].state=SlotState::Idle{next_ms:0};m.disc.book.get(hc).unwrap();}}
        drop(m);let _=std::fs::remove_dir_all(&dir);
    }
    /// Residual (a) of PR2a (D27): a squatted self-advertisement that accepts and stays silent until the Hello
    /// deadline (or closes, or refuses) collects counted failures from the feeler (each after a verification
    /// elsewhere) and is terrible after 3. A listener a feeler verified is shielded from a co-tenant's withdrawal.
    #[test] fn squatters_that_refuse_close_or_stay_silent_become_terrible(){
        let (p,q)=(sa("81.40.0.1:30300"),sa("81.60.0.1:30300"));
        let (mut n,rx,dir)=node("o-squat",1,true,vec![],vec![],G);n.disc.times.stall_min_ms=u64::MAX/4;
        verified(&mut n,p,G);n.assign(0,p,SlotKind::Auto,G);let _p=connect(&mut n,1,Origin::Slot(0,p),Some(p));greet(&mut n,1,30300,0);
        let sq=sa("[2a00:1::9]:30300");n.disc.book.insert_unverified(sq,Source::Inbound,Kind::Direct,false,G);
        let mut now=G;
        for (i,how) in ["refuse","close","silent"].iter().enumerate(){
            now+=S;verified(&mut n,q,now);n.disc.out.next_feeler_ms=0;n.disc.book.get(sq).unwrap();
            let e=n.disc.book.get(sq).unwrap().clone();n.disc.out.slots[1].state=SlotState::Idle{next_ms:0};
            now=now.max(e.last_try_ms+n.disc.times.retry_ms);tick(&mut n,now);
            assert_eq!(rx[1].try_recv().unwrap(),sq,"visit {i}");
            match *how{"refuse"=>n.disc_dial_failed(1,sq,DialError::Refused,now),_=>{let _c=connect(&mut n,20+i as u64,Origin::Slot(1,sq),Some(sq));close(&mut n,20+i as u64,now)}}
            assert_eq!(attempts(&n,sq),i as u8+1,"{how}: a counted failure");
        }
        assert!(n.disc.book.terrible(n.disc.book.get(sq).unwrap(),now),"terrible after 3");
        // The shield: a listener at X verified by a feeler stays when a co-tenant at X withdraws.
        let (l,l2)=(sa("81.70.0.1:30300"),sa("81.70.0.1:30301"));for a in [l,l2]{n.disc.book.insert_unverified(a,Source::Inbound,Kind::Direct,false,now);}
        n.disc.book.remove(sq);n.disc.out.slots[1].state=SlotState::Idle{next_ms:0};n.disc.out.next_feeler_ms=0;
        let mut f=None;for _ in 0..10{tick(&mut n,now);if let Ok(a)=rx[1].try_recv(){if a==l{f=Some(a);break}n.disc_dial_failed(1,a,DialError::NotHanded,now);n.disc.out.next_feeler_ms=0;}}
        assert_eq!(f,Some(l));let _c=connect(&mut n,30,Origin::Slot(1,l),Some(l));greet(&mut n,30,30300,0);close(&mut n,30,now);
        assert!(n.disc.book.get(l).is_some_and(|e|e.verified_ms>0),"the feeler verified it");
        let _w=connect(&mut n,31,Origin::Inbound,Some(sa("81.70.0.1:52000")));greet(&mut n,31,0,0x77);
        assert!(n.disc.book.get(l).is_some()&&n.disc.book.get(l2).is_none(),"the verified listener stays, the unverified self-advertisement goes");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// D26: first delivery. An echo of a block this node already holds (a Duplicate outside the sender's sync
    /// session, node.rs broadcasts to every peer but the sender) earns nothing; the same Duplicate from the session's
    /// peer, solicited, does; a new block counts within 2 of the head only. A template that locks makes its sender
    /// useful (`try_lock_signed`), without a block. (The genesis header is not a wire header, so this in-crate test
    /// checks the Block arm's rule, `first_delivery`, and the template hook through `disc_useful`.)
    #[test] fn usefulness_is_first_delivery(){
        assert!(!first_delivery(false,true,5,5,false),"an echo earns nothing");
        assert!(first_delivery(false,true,5,5,true),"a Duplicate of the peer's own session does");
        assert!(first_delivery(true,false,8,10,false)&&!first_delivery(true,false,7,10,false),"a new block within 2 of the head");
        assert!(!first_delivery(false,false,10,10,true),"a rejected block never does");
        let (mut n,_rx,dir)=node("o-useful",1,false,vec![],vec![],G);
        let _a=up(&mut n,0,sa("81.30.0.1:30300"),1,true,G);n.disc_useful(1,false,G+S);
        assert_eq!((n.disc.links[&1].useful_ms,n.disc.links[&1].blocks),(G+S,0),"a locked template: useful, no block");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// D26 (VA-SPEC §1a.8): healthy counts distinct keys of useful peers in either direction, but only with at least
    /// one useful automatic slot or configured peer among them; a seed's key never counts; usefulness lasts W.
    #[test] fn healthy_counts_useful_peers_in_either_direction_with_one_outbound(){
        let seed=sa("81.43.0.1:30300");
        let (mut n,_rx,dir)=node("o-healthy",2,false,vec![seed],vec![seed],G);
        let useful=|n:&mut Node,id:PeerId|n.disc_useful(id,true,G);
        let _i=connect(&mut n,1,Origin::Inbound,Some(sa("81.40.0.1:51000")));greet(&mut n,1,0,0);useful(&mut n,1);
        let _j=connect(&mut n,2,Origin::Inbound,Some(sa("81.41.0.1:51000")));greet(&mut n,2,0,0);useful(&mut n,2);
        assert_eq!(n.healthy(G),0,"inbound peers alone");
        let _s=connect(&mut n,3,Origin::Configured(seed),Some(seed));greet(&mut n,3,30300,0);useful(&mut n,3);
        assert_eq!(n.healthy(G),0,"a seed is no outbound peer for health");
        let _k=up(&mut n,0,sa("81.42.0.1:30300"),4,true,G);useful(&mut n,4);
        assert_eq!(n.healthy(G),3,"with one useful automatic slot, both directions count; the seed does not");
        let _x=connect(&mut n,5,Origin::Inbound,Some(sa("81.43.0.9:51000")));greet(&mut n,5,0,0);useful(&mut n,5);
        assert_eq!(n.healthy(G),3,"nor does a peer in a seed's key");
        let w=n.window_ms();assert_eq!((n.healthy(G+w),n.healthy(G+w+1)),(3,0),"usefulness lasts W");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// D25 (VA-SPEC §1a.7): stalled is the head's own age (W), whatever the slots deliver. The spare becomes the extra
    /// within a tick, to a source no automatic slot is connected to; its head-moving block swaps it in for the
    /// automatic slot useful longest ago, never the sync session's peer, whose end is no failure. An extra that
    /// moves nothing is closed after W.
    #[test] fn stall_is_the_heads_age_and_the_extra_slot_recovers(){
        let (x,y)=(sa("81.1.0.1:30300"),sa("81.2.0.1:30300"));
        let (a1,a2,b1,b2)=(sa("81.30.0.1:30300"),sa("81.31.0.1:30300"),sa("81.32.0.1:30300"),sa("81.33.0.1:30300"));
        let w=120_000;let now=G-S+w;
        let (mut n,rx,dir)=node("o-stall",2,false,vec![],vec![],now);
        for a in [a1,a2,b1]{heard(&mut n,a,x,G)}heard(&mut n,b2,y,G);
        let _1=up(&mut n,0,a1,1,true,now);let _2=up(&mut n,1,a2,2,true,now);
        n.disc_useful(1,true,now-2);n.disc_useful(2,true,now-1);
        assert!(!n.stalled(now-1)&&n.stalled(now)&&n.window_ms()==w,"stalled once the head is W old, whatever the slots deliver");
        tick(&mut n,now);assert_eq!((rx[2].try_recv().ok(),n.disc.out.counters.stalls),(Some(b2),1),"the extra goes to a new source at once");
        let mut e=connect(&mut n,3,Origin::Slot(2,b2),Some(b2));greet(&mut n,3,30300,0);
        n.start_session(1,now_ms());let mut c2=_2;
        n.disc_head(3,now+S);
        assert!(closed(&mut c2)&&n.disc.out.counters.extra_swaps==1,"the slot useful longest ago that is not the session peer is closed");
        assert!(n.disc.out.slots[2].role==Role::Auto&&n.disc.out.slots[1].role==Role::Spare&&matches!(state(&n,2),SlotState::Up{kind:SlotKind::Auto,..}),"roles swapped");
        close(&mut n,2,now+S);assert_eq!(attempts(&n,a2),0,"no failure for the slot this node closed");
        // The new spare, still stalled: another extra W later, closed after W without a head.
        tick(&mut n,now+w);let b=rx[1].try_recv().expect("the next extra");assert!([a2,b1].contains(&b),"no new source is left: any other address");
        let mut f=connect(&mut n,4,Origin::Slot(1,b),Some(b));greet(&mut n,4,30300,0);
        tick(&mut n,now+2*w-1);assert!(n.peers.contains_key(&4));
        tick(&mut n,now+2*w+S);assert!(closed(&mut f)&&n.disc.out.counters.extra_closed==1,"an extra that moves nothing is closed after W");
        let _=&mut e;drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// D25 (B6): a node whose own blocks keep its head young is not stalled; with no peer's block moving its head for
    /// 4W it is quiet, and the spare probes one extra per W.
    #[test] fn a_lone_miner_is_not_stalled_but_probes_when_quiet(){
        let (mut n,rx,dir)=node("o-quiet",1,false,vec![],vec![],G);
        let p=sa("81.40.0.1:30300");let _p=up(&mut n,0,p,1,true,G);
        for i in 0..3{heard(&mut n,SocketAddr::new([81,50+i,0,1].into(),30300),sa("81.1.0.1:30300"),G)}
        let w=n.window_ms();n.disc.out.last_peer_head_ms=G-4*w+S;
        assert!(!n.stalled(G)&&!n.quiet(G));
        tick(&mut n,G+S);assert!(n.quiet(G+S)&&n.disc.out.counters.quiets==1);
        let first=rx[1].try_recv().expect("one extra");n.disc_dial_failed(1,first,DialError::Refused,G+S);
        n.disc.out.slots[1].state=SlotState::Idle{next_ms:0};tick(&mut n,G+2*S);assert!(rx[1].try_recv().is_err(),"one per W");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// B4: with no compatible peer (an offline boot), a verified entry is redialed after the slot's backoff, not
    /// after its retry spacing.
    #[test] fn an_offline_boot_reconnects_within_backoff(){
        let v=sa("81.31.0.1:30300");
        let (mut n,rx,dir)=node("o-offline",1,false,vec![],vec![],G);
        n.disc.times.retry_verified_ms=600_000;verified(&mut n,v,G-1);
        let mut now=G;
        for _ in 0..5{tick(&mut n,now);assert_eq!(rx[0].try_recv().ok(),Some(v),"redialed at {now}");n.disc_dial_failed(0,v,DialError::Timeout,now);now=next(&n,0).unwrap();}
        assert!(now-G<=n.disc.times.backoff_max_ms,"within the backoff");
        let _i=connect(&mut n,9,Origin::Inbound,Some(sa("81.50.0.1:51000")));greet(&mut n,9,0,0);
        tick(&mut n,now);assert!(rx[0].try_recv().is_err(),"with a compatible peer the spacing holds");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// D26, VA-SPEC §1a.7: the anchors are the automatic peers connected anchor_uptime_ms (10 min) that delivered a
    /// block and were useful within W, one per key; a change is saved at once though the book is clean, but not while
    /// the node is stalled. After a restart the anchor is the first assignment (`anchorsDialed`); one saved more than
    /// anchor_keep_ms (24 h) ago is not dialed; a loaded anchor whose dial failed is not saved again.
    #[test] fn anchors_come_from_currently_useful_long_lived_peers_and_are_dialed_first(){
        const M:u64=60_000;
        let (mut n,rx,dir)=node("o-anchor",4,false,vec![],vec![],G);let _=rx;
        n.disc.times.stall_min_ms=700_000;n.disc.times.save_first_ms=u64::MAX/4;n.disc.times.save_every_ms=u64::MAX/4;n.disc_load(G);
        let w=n.window_ms();assert_eq!(w,700_000);
        let a:Vec<SocketAddr>=(0..4).map(|i|SocketAddr::new([81,30+i,0,1].into(),30300)).collect();
        let now=G+11*M;
        let _0=up(&mut n,0,a[0],1,true,G+2*M);let _1=up(&mut n,1,a[1],2,true,G);let _2=up(&mut n,2,a[2],3,true,G);let mut c3=up(&mut n,3,a[3],4,true,G);
        n.disc_useful(1,true,now);n.disc_useful(3,true,now-w-S);n.disc_useful(4,true,now);n.disc_useful(2,false,now);
        n.disc.test_now=Some(G+5*M);n.disc_save();assert_eq!(n.disc.counters.book_saves,1);assert!(!n.disc.book.dirty());
        n.disc.test_now=Some(now);n.disc_tick(now);
        assert_eq!((n.disc.counters.book_saves,n.disc.out.saved_anchors.iter().map(|x|x.0).collect::<Vec<_>>()),(2,vec![a[3]]),"only the long-lived, currently useful peer with a block; saved though the book was clean");
        let file=std::fs::read(dir.join(super::super::FILE)).unwrap();
        let (_,anchors)=crate::addrbook::parse(&file,n.chain.spec.chain_id(),&n.chain.spec.genesis_hash(),false,now).unwrap();assert_eq!(anchors,[(a[3],now)]);
        let late=G+w+M;n.disc.test_now=Some(late);n.handle(Event::Net(NetEvent::Closed(4)));n.disc_tick(late);
        assert!(n.stalled(late)&&n.disc.counters.book_saves==2&&n.disc.out.saved_anchors.len()==1,"no anchor change is saved while stalled");
        let _=closed(&mut c3);
        // A restart: the anchor first, whatever its retry spacing.
        let (mut m,mdir)=opened("o-anchor2",Discovery{outbound:Outbound::Count(2),rng_seed:Some(7),..Discovery::default()},false,vec![],now+M);
        std::fs::write(mdir.join(super::super::FILE),&file).unwrap();m.disc_load(now+M);m.disc.book.tried(a[3],now);let rx=m.disc_test_slots(now+M);
        for i in 0..3{verified(&mut m,a[i],now)}
        tick(&mut m,now+M);assert!(matches!(state(&m,0),SlotState::Dialing{addr,kind:SlotKind::Anchor,..} if addr==a[3])&&rx[0].try_recv().ok()==Some(a[3])&&m.disc.out.counters.anchors_dialed==1,"the anchor is dialed first");
        m.disc_dial_failed(0,a[3],DialError::Refused,now+M);assert!(m.disc_anchors(now+M).is_empty(),"a loaded anchor whose dial failed is not saved again");
        let (mut o,odir)=opened("o-anchor3",Discovery{outbound:Outbound::Count(2),rng_seed:Some(7),..Discovery::default()},false,vec![],now+25*60*M);
        std::fs::write(odir.join(super::super::FILE),&file).unwrap();o.disc_load(now+25*60*M);
        assert!(o.disc.out.anchors_left.is_empty()&&o.disc.out.loaded_anchors.is_empty(),"an anchor saved 25 h ago is not dialed");
        drop((n,m,o));for d in [dir,mdir,odir]{let _=std::fs::remove_dir_all(&d);}
    }
    /// D17 (VA-SPEC §1a.9): at the network's start each bootstrap address is an unverified entry of its own source
    /// (one a slot peer named first is re-sourced), never answered before it is verified; an otherwise empty book's
    /// first assignments are the bootstrap addresses.
    #[test] fn bootstrap_addresses_are_unverified_entries_of_their_own_source(){
        let (b1,b2,x)=(sa("81.30.0.1:30300"),sa("81.31.0.1:30300"),sa("81.9.0.1:30300"));
        let (mut n,dir)=opened("o-boot",Discovery{outbound:Outbound::Count(2),bootstrap:vec![b1,b2],rng_seed:Some(7),..Discovery::default()},false,vec![],G);
        n.disc.book.insert_unverified(b1,Source::Peer(key_of(x)),Kind::Heard,false,G);
        n.disc_load(G);
        for b in [b1,b2]{let e=n.disc.book.get(b).unwrap();assert_eq!((e.source,e.verified_ms),(Source::Cfg(b),0),"{b}");}
        assert_eq!(n.disc.out.counters.bootstrap_loaded,2);
        let _i=connect(&mut n,1,Origin::Inbound,Some(sa("81.50.0.1:51000")));greet(&mut n,1,0,0);msg(&mut n,1,Msg::GetAddr);
        assert!(n.peers[&1].queued().iter().all(|m|!matches!(m,Msg::Addr{addrs} if addrs.contains(&b1)||addrs.contains(&b2))),"never answered before verified");
        let rx=n.disc_test_slots(G);tick(&mut n,G);
        let mut got=[rx[0].try_recv().unwrap(),rx[1].try_recv().unwrap()];got.sort();assert_eq!(got,[b1,b2]);
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    fn wanted(n:&Node)->Vec<usize>{n.disc.out.seeds.iter().enumerate().filter(|(_,s)|s.wanted_ms.is_some()).map(|(i,_)|i).collect()}
    fn in_set(n:&Node,a:SocketAddr)->bool{n.outbound.lock().unwrap().contains(&a.to_string())}
    fn seeds3()->Vec<SocketAddr>{(0..3).map(|i|SocketAddr::new([108,165,128,89+i].into(),30300)).collect()}
    /// D7'' (B3, VA-SPEC §1a.8): every seed starts suppressed (in the outbound set, so its dialer waits). During the
    /// boot window an empty book wants SEED_LINKS_MAX of them; a book that can fill the K slots wants none, also once
    /// the slots are dialing its entries; when those dials fail, the seeds are wanted.
    #[test] fn the_boot_wants_seeds_only_when_the_book_cannot_fill_the_slots(){
        let seeds=seeds3();
        let (mut n,_rx,dir)=node("o-boot-empty",2,true,seeds.clone(),seeds.clone(),G);
        assert!(seeds.iter().all(|s|in_set(&n,*s))&&wanted(&n).is_empty(),"every seed suppressed before the first tick");
        tick(&mut n,G);let w=wanted(&n);assert_eq!(w.len(),2,"an empty book wants two of three");
        assert!(w.iter().all(|i|!in_set(&n,seeds[*i]))&&seeds.iter().filter(|s|in_set(&n,**s)).count()==1,"a wanted seed's dialer may dial");
        drop(n);let _=std::fs::remove_dir_all(&dir);
        let (mut n,rx,dir)=node("o-boot-full",2,true,seeds.clone(),seeds.clone(),G);
        let v=[sa("81.30.0.1:30300"),sa("81.31.0.1:30300")];for a in v{verified(&mut n,a,G-S)}
        tick(&mut n,G);assert!(wanted(&n).is_empty(),"the book can fill the slots");
        assert!(matches!(state(&n,0),SlotState::Dialing{..})&&matches!(state(&n,1),SlotState::Dialing{..}));
        tick(&mut n,G+S);assert!(wanted(&n).is_empty(),"also while the slots dial its entries");
        for t in 0..2{let a=rx[t].try_recv().unwrap();n.disc_dial_failed(t as u8,a,DialError::Refused,G+S);}
        tick(&mut n,G+2*S);assert_eq!(wanted(&n).len(),2,"once they failed, the seeds are wanted");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// D7'' (A5, B1): a node without a listener wants one seed (D7'a); a wanted seed not compatible 30 s after it was
    /// wanted is held, 1 min doubling to 6 h, and the next is wanted, so 3 dead seeds see few want cycles in 2 h; with
    /// all held none is wanted and every one stays in the outbound set. A seed that connects clears its failures.
    #[test] fn dead_seeds_are_held_with_backoff(){
        let seeds=seeds3();
        let (mut n,_rx,dir)=node("o-dead",2,false,seeds.clone(),seeds.clone(),G);n.disc.times.stall_min_ms=u64::MAX/4;n.disc.out.route_ok=Some(true);
        let mut now=G;let end=G+2*3_600_000;let mut all_held=false;
        let boot=G+n.disc.times.seed_boot_ms;let mut holds:Vec<Vec<u64>>=vec![vec![];3];
        while now<end{let before:Vec<u64>=n.disc.out.seeds.iter().map(|s|s.hold_until_ms).collect();tick(&mut n,now);assert!(wanted(&n).len()<=if now<boot{2}else{1});
            for (i,s) in n.disc.out.seeds.iter().enumerate(){if s.hold_until_ms!=before[i]{holds[i].push((s.hold_until_ms-now)/60_000)}}
            if n.disc.out.seeds.iter().all(|s|s.hold_until_ms>now){all_held=true;assert!(wanted(&n).is_empty()&&seeds.iter().all(|s|in_set(&n,*s)),"all held: none wanted, every dialer waits");}
            now+=S;}
        let c=&n.disc.out.counters;assert!(c.seed_wants<=24&&c.seed_holds>=3,"want cycles {} holds {}",c.seed_wants,c.seed_holds);
        assert!(all_held,"every seed held at some point");
        let fails:Vec<u32>=n.disc.out.seeds.iter().map(|s|s.fails).collect();assert!(fails.iter().all(|f|*f>=4),"{fails:?}");
        for h in &holds{assert!(h.len()>=3&&h.iter().enumerate().all(|(k,m)|*m==1<<k),"the holds double from 1 min: {h:?}");}
        while wanted(&n).is_empty(){now+=S;tick(&mut n,now);}
        let i=wanted(&n)[0];let s=seeds[i];let _c=connect(&mut n,1,Origin::Configured(s),Some(s));greet(&mut n,1,30300,0);
        assert_eq!(n.disc.out.seeds[i].fails,0,"a seed that connects clears its failures");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// D18', D7'' (VA-SPEC §1a.8): a seed that fails while this host has no route (and no compatible peer) is held
    /// only the base and counts no failure, so 2 h offline leave no long hold and a seed is wanted within a minute of
    /// the route's return. Holds that dead seeds built up behind a router without upstream are cleared by the first
    /// compatible peer after none, and by a family that comes back.
    #[test] fn an_outage_builds_no_long_seed_holds(){
        let seeds=seeds3();let end=G+2*3_600_000;
        let run=|n:&mut Node,from:u64,to:u64|{let mut now=from;while now<to{tick(n,now);now+=S;}};
        let (mut n,_rx,dir)=node("o-outage",2,false,seeds.clone(),seeds.clone(),G);n.disc.times.stall_min_ms=u64::MAX/4;n.disc.out.route_ok=Some(false);
        run(&mut n,G,end);
        assert!(n.disc.out.seeds.iter().all(|s|s.fails==0&&s.hold_until_ms<=end+n.disc.times.seed_hold_base_ms),"no route: base holds only, no failures");
        n.disc.out.route_ok=Some(true);let wants=n.disc.out.counters.seed_wants;run(&mut n,end,end+61*S);
        assert!(n.disc.out.counters.seed_wants>wants,"a seed is wanted within a minute of the route's return");
        drop(n);let _=std::fs::remove_dir_all(&dir);
        let (mut n,_rx,dir)=node("o-outage2",2,false,seeds.clone(),seeds.clone(),G);n.disc.times.stall_min_ms=u64::MAX/4;n.disc.out.route_ok=Some(true);
        run(&mut n,G,end);
        assert!(n.disc.out.seeds.iter().all(|s|s.fails>=4),"a route but no peer: the dead seeds' holds grow");
        let _i=connect(&mut n,9,Origin::Inbound,Some(sa("81.50.0.1:51000")));greet(&mut n,9,0,0);
        assert!(n.disc.out.seeds.iter().all(|s|s.fails==0&&s.hold_until_ms==0),"the first compatible peer after none clears them");
        tick(&mut n,end);assert_eq!(wanted(&n).len(),1,"and a seed is wanted at once");
        n.disc.out.seeds[0].fails=5;n.disc.out.seeds[0].hold_until_ms=u64::MAX;n.disc.out.family_down[0]=Some((seeds[0],end));
        let probe=n.disc.times.probe_ms;tick(&mut n,end+probe);assert!(n.disc.out.family_down[0].is_none()&&n.disc.out.seeds[0].fails==0&&n.disc.out.seeds[0].hold_until_ms==0,"a family that comes back clears them");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// D7'' (VA-SPEC §1a.8): a listening node wants a seed until 2 useful peers have held for seed_release_ms, then
    /// releases it, but not while its GetAddr is outstanding nor while it is the sync peer. A suppressed seed's address
    /// stays in the outbound set when its connection ends; a Connected from a suppressed seed is closed at once.
    #[test] fn the_seed_policy_holds_rotates_and_releases(){
        let seeds=seeds3();
        let (mut n,_rx,dir)=node("o-release",2,true,seeds.clone(),seeds.clone(),G);n.disc.times.stall_min_ms=u64::MAX/4;
        // A release hold of 10 s, so that the release falls within the GetAddr's 2 minutes.
        n.disc.times.seed_release_ms=10_000;
        let boot=G+n.disc.times.seed_boot_ms;
        tick(&mut n,G);let i=wanted(&n)[0];let s=seeds[i];
        let _s=connect(&mut n,1,Origin::Configured(s),Some(s));greet(&mut n,1,30300,0);
        let _a=up(&mut n,0,sa("81.30.0.1:30300"),2,true,G);let _b=up(&mut n,1,sa("81.31.0.1:30300"),3,true,G);
        let mut now=boot;tick(&mut n,now);assert_eq!(wanted(&n),[i],"after the boot one seed while healthy < 2");
        let useful=|n:&mut Node,now:u64|{n.disc_useful(2,true,now);n.disc_useful(3,true,now)};
        useful(&mut n,now);tick(&mut n,now);let since=n.disc.out.healthy_since.unwrap();assert_eq!(since,now);
        now=since+n.disc.times.seed_release_ms-S;useful(&mut n,now);tick(&mut n,now);assert_eq!(wanted(&n),[i],"healthy for less than 2 min");
        now+=S;useful(&mut n,now);tick(&mut n,now);assert_eq!(wanted(&n),[i],"its GetAddr is outstanding");
        msg(&mut n,1,Msg::Addr{addrs:vec![]});n.start_session(1,now_ms());tick(&mut n,now);assert_eq!(wanted(&n),[i],"it is the sync peer");
        n.end_session(true);let mut c=connect(&mut n,9,Origin::Inbound,None);let _=&mut c;
        tick(&mut n,now);assert!(wanted(&n).is_empty()&&n.disc.out.counters.seed_releases==1,"released");
        assert!(in_set(&n,s),"suppressed: in the outbound set");
        n.handle(Event::Net(NetEvent::Closed(1)));assert!(in_set(&n,s),"its Closed leaves it there");
        let mut again=connect(&mut n,4,Origin::Configured(s),Some(s));assert!(closed(&mut again)&&!n.peers.contains_key(&4),"a Connected from a suppressed seed is closed");
        n.handle(Event::Net(NetEvent::Closed(4)));assert!(in_set(&n,s));
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// D7'a: a node without a listener keeps exactly one seed while it answers, however healthy two attacker
    /// listeners make it look.
    #[test] fn two_attacker_listeners_never_release_a_non_listening_nodes_seed(){
        let seeds=seeds3();
        let (mut n,_rx,dir)=node("o-quietnode",2,false,seeds.clone(),seeds.clone(),G);n.disc.times.stall_min_ms=u64::MAX/4;
        tick(&mut n,G);let i=wanted(&n)[0];let s=seeds[i];let _s=connect(&mut n,1,Origin::Configured(s),Some(s));greet(&mut n,1,30300,0);msg(&mut n,1,Msg::Addr{addrs:vec![]});
        let _a=up(&mut n,0,sa("81.30.0.1:30300"),2,true,G);let _b=up(&mut n,1,sa("81.31.0.1:30300"),3,true,G);
        let mut now=G;while now<G+3_600_000{n.disc_useful(2,true,now);n.disc_useful(3,true,now);tick(&mut n,now);assert_eq!(wanted(&n),[i],"at {now}");now+=10*S;}
        assert!(n.healthy(now-10*S)>=2);
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// D27 (B12): a seed whose connection carries this node's own nonce is this node: it keeps every other seed wanted
    /// whatever its health; `is_seed` at open does the same, also with a book that could fill the slots.
    #[test] fn a_seed_that_is_this_node_keeps_the_other_seeds(){
        let seeds=seeds3();
        let (mut n,_rx,dir)=node("o-selfseed",2,true,seeds.clone(),seeds.clone(),G);n.disc.times.stall_min_ms=u64::MAX/4;
        n.disc_load(G);tick(&mut n,G);for i in wanted(&n){let s=seeds[i];let ours=n.disc.wire_nonce();let _c=connect(&mut n,10+i as u64,Origin::Configured(s),Some(s));greet(&mut n,10+i as u64,30300,if i==1{ours}else{0});}
        if !n.disc.out.seed_node{let s=seeds[1];n.want(1,G);let ours=n.disc.wire_nonce();let _c=connect(&mut n,20,Origin::Configured(s),Some(s));greet(&mut n,20,30300,ours);}
        assert!(n.disc.out.seed_node&&n.disc.out.seeds[1].is_self);
        let late=G+n.disc.times.seed_boot_ms+n.disc.times.seed_release_ms*3;tick(&mut n,late);
        assert_eq!(wanted(&n),[0,2],"the other seeds stay wanted");
        assert_eq!(n.status_json()["discovery"]["outbound"]["seedNode"],true);
        drop(n);let _=std::fs::remove_dir_all(&dir);
        let (mut m,dir)=opened("o-isseed",Discovery{outbound:Outbound::Count(2),seeds:seeds.clone(),is_seed:true,rng_seed:Some(7),..Discovery::default()},true,seeds.clone(),G);
        for i in 0..4{verified(&mut m,SocketAddr::new([81,30+i,0,1].into(),30300),G-S)}let _rx=m.disc_test_slots(G);tick(&mut m,G);
        assert_eq!(wanted(&m),[0,1,2],"is_seed: every seed wanted, though the book could fill the slots");
        drop(m);let _=std::fs::remove_dir_all(&dir);
    }
    /// D7'' (B5): while stalled, a wanted seed compatible for W is replaced by the next, at most once per W, but never
    /// while a sync session progresses and never the session's peer.
    #[test] fn a_stall_rotation_spares_a_progressing_session(){
        let seeds=seeds3();let w=120_000;let start=G-S+w;
        let (mut n,_rx,dir)=node("o-rotate",2,true,seeds.clone(),seeds.clone(),start);
        tick(&mut n,start);let i=wanted(&n)[0];let s=seeds[i];let _s=connect(&mut n,1,Origin::Configured(s),Some(s));greet(&mut n,1,30300,0);
        let _x=connect(&mut n,2,Origin::Inbound,Some(sa("81.40.0.1:51000")));greet(&mut n,2,0,0);
        let now=start+2*w;assert!(n.stalled(now));
        n.start_session(2,now_ms());tick(&mut n,now);assert_eq!((wanted(&n).contains(&i),n.disc.out.counters.seed_rotations),(true,0),"another peer's session progresses");
        n.session.as_mut().unwrap().last_ms=now_ms()-SESSION_STALL;tick(&mut n,now+S);
        assert!(!wanted(&n).contains(&i)&&n.disc.out.counters.seed_rotations==1&&wanted(&n).len()==1,"rotated once the session stopped progressing");
        let j=wanted(&n)[0];let t=seeds[j];let _t=connect(&mut n,3,Origin::Configured(t),Some(t));greet(&mut n,3,30300,0);
        n.end_session(false);n.start_session(3,now_ms());n.session.as_mut().unwrap().last_ms=now_ms()-SESSION_STALL;
        tick(&mut n,now+3*w);assert_eq!(n.disc.out.counters.seed_rotations,1,"never the session's peer");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// D17, D25 (VA-SPEC §1a.7, §1a.9): with every seed held, a stalled node's extra goes to the bootstrap addresses in
    /// turn, one per W, before any of the 300 entries 30 other sources put into its book.
    #[test] fn with_every_seed_held_a_stalled_node_dials_a_bootstrap_address(){
        let (seeds,b)=(seeds3(),[sa("81.30.0.1:30300"),sa("81.31.0.1:30300")]);let w=120_000;let now=G-S+w;
        let (mut n,dir)=opened("o-bootx",Discovery{outbound:Outbound::Count(1),seeds:seeds.clone(),bootstrap:b.to_vec(),rng_seed:Some(7),..Discovery::default()},false,seeds,now);
        n.disc_load(now);
        for i in 0..300u32{let a=SocketAddr::new([91+(i/250) as u8,(i%250) as u8,0,1].into(),30300);
            n.disc.book.insert_unverified(a,Source::Peer(key_of(SocketAddr::new([70,(i%30) as u8,0,1].into(),30300))),Kind::Heard,false,now);}
        assert!(n.disc.book.entries().len()>=302,"the book is poisoned");
        // The seeds held after P, the first compatible peer (which clears their holds, `seeds_online`).
        let rx=n.disc_test_slots(now);let p=sa("81.40.0.1:30300");verified(&mut n,p,now);let _p=up(&mut n,0,p,1,true,now);let _=rx[0].try_recv();
        for s in &mut n.disc.out.seeds{s.fails=1;s.hold_until_ms=u64::MAX/2;}
        tick(&mut n,now);assert!(n.stalled(now));assert_eq!(rx[1].try_recv().ok(),Some(b[0]),"the stall's extra: the first bootstrap address");
        n.disc_dial_failed(1,b[0],DialError::Refused,now);
        tick(&mut n,now+S);assert!(rx[1].try_recv().is_err(),"one extra per W");
        tick(&mut n,now+w);assert_eq!(rx[1].try_recv().ok(),Some(b[1]),"W later, the next bootstrap address");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// D27 (VA-SPEC §1a.4, §1a.7): an address a slot peer's Addr named that accepts connections but never sends a
    /// Hello (an echo service: the feeler sends nothing, so it echoes nothing, and the Hello deadline closes it) takes
    /// a counted failure at each feeler visit; after 3, spaced and each after a verification elsewhere, it is terrible
    /// and the next save purges it from the book and the file.
    #[test] fn a_hearsay_echo_service_is_forgotten(){
        let (p,q,echo)=(sa("81.40.0.1:30300"),sa("81.60.0.1:30300"),sa("81.77.0.1:30307"));
        let (mut n,rx,dir)=node("o-echo",1,true,vec![],vec![],G);n.disc.times.stall_min_ms=u64::MAX/4;n.disc_load(G);
        verified(&mut n,p,G);let _p=up(&mut n,0,p,1,true,G);msg(&mut n,1,Msg::Addr{addrs:vec![echo]});
        assert_eq!(n.disc.book.get(echo).map(|e|(e.source,e.verified_ms)),Some((Source::Peer(key_of(p)),0)),"hearsay from the slot peer");
        let mut now=G;
        for i in 0..3u64{
            now+=S;verified(&mut n,q,now);let e=n.disc.book.get(echo).unwrap().clone();now=now.max(e.last_try_ms+n.disc.times.retry_ms);
            n.disc.out.next_feeler_ms=0;tick(&mut n,now);assert_eq!(rx[1].try_recv().ok(),Some(echo),"visit {i}");
            let _c=connect(&mut n,10+i,Origin::Slot(1,echo),Some(echo));assert!(n.disc.out.feeler.is_some());close(&mut n,10+i,now);
            assert_eq!(attempts(&n,echo),i as u8+1,"visit {i}: a counted failure");
        }
        assert!(n.disc.book.terrible(n.disc.book.get(echo).unwrap(),now),"terrible after 3");
        n.disc.next_save_ms=0;n.disc.test_now=Some(now);n.disc_tick(now);
        let file=std::fs::read_to_string(dir.join(super::super::FILE)).unwrap();
        assert!(n.disc.book.get(echo).is_none()&&!file.contains("81.77.0.1"),"purged at the save");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// D24 (VA-SPEC §1a.7): a self-advertisement withdrawn by a second connection from its IP while a slot or the
    /// feeler dials it is not placed again as its own verified source when the Hello comes (it would be a source of
    /// its own in the tried class, uncounted by the feeler's cap). Without the withdrawal it is verified in place and
    /// stays Inbound. An anchor or a bootstrap address no longer in the book is placed as its own or as `Cfg`.
    #[test] fn a_self_advertisement_withdrawn_mid_dial_never_becomes_its_own_verified_source(){
        let (p,b,x)=(sa("81.40.0.1:30300"),sa("81.91.0.1:30300"),sa("81.90.0.1:30300"));
        let (mut n,dir)=opened("o-launder",Discovery{outbound:Outbound::Count(1),bootstrap:vec![b],rng_seed:Some(7),times:DiscoveryTimes{stall_min_ms:u64::MAX/4,..DiscoveryTimes::default()},..Discovery::default()},true,vec![],G);
        let _rx=n.disc_test_slots(G);verified(&mut n,p,G);let _p=up(&mut n,0,p,1,true,G);
        let mut id=10;
        // The self-ad at x (a closed inbound connection), slot `t` dials it as `kind`, then (with `withdraw`) a second
        // inbound connection from x's IP withdraws it before x's Hello arrives.
        let mut run=|n:&mut Node,t:usize,kind:SlotKind,withdraw:bool|{
            id+=3;let _i=connect(n,id,Origin::Inbound,Some(sa("81.90.0.1:51000")));greet(n,id,30300,0);close(n,id,G);
            assert_eq!(n.disc.book.get(x).map(|e|(e.source,e.verified_ms)),Some((Source::Inbound,0)));
            n.disc.out.slots[t].state=SlotState::Idle{next_ms:0};n.assign(t,x,kind,G);let _c=connect(n,id+1,Origin::Slot(t as u8,x),Some(x));
            if withdraw{let _w=connect(n,id+2,Origin::Inbound,Some(sa("81.90.0.1:51001")));greet(n,id+2,0,0x77);assert!(n.disc.book.get(x).is_none(),"withdrawn mid-dial");}
            greet(n,id+1,30300,0);let got=n.disc.book.get(x).map(|e|(e.source,e.verified_ms>0));n.disc.book.remove(x);close(n,id+1,G);got
        };
        for (t,kind) in [(0,SlotKind::Auto),(1,SlotKind::Feeler)]{
            assert_eq!(run(&mut n,t,kind,true),None,"{kind:?}: not placed as its own source");
            assert_eq!(run(&mut n,t,kind,false),Some((Source::Inbound,true)),"{kind:?}: verified in place, still Inbound");
        }
        assert_eq!(n.slot_source(0,b),Some(Source::Cfg(b)),"a bootstrap address keeps its own source");
        n.disc.out.slots[0].state=SlotState::Up{peer:1,addr:x,kind:SlotKind::Anchor,since_ms:G};
        assert_eq!(n.slot_source(0,x),Some(Source::Peer(key_of(x))),"an anchor is its own");
        n.disc.out.slots[0].state=SlotState::Up{peer:1,addr:x,kind:SlotKind::Auto,since_ms:G};assert_eq!(n.slot_source(0,x),None);
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    const SESSION_STALL:u128=crate::node::SESSION_STALL_MS;
}
