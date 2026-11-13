//! Node orchestration: one consensus thread owns the chain, journal, pool and
//! production state machine (consensus.md §4) and processes events from peers,
//! the range miner, RPC and a 50 ms tick.
use std::collections::{BTreeSet,HashMap,HashSet,VecDeque};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc,Mutex,mpsc::{self,Receiver,Sender,SyncSender}};
use std::time::{Duration,Instant,SystemTime,UNIX_EPOCH};
use crate::{hashes,header::Header};
use pocol_lp4_execution::{apply::ParentResult,txsign::Key,wire,native::{self,NativeBlockContext},user_phase::{self,PoolMeter}};
use revm::{InMemoryDB,primitives::{Address,B256,U256}};
use crate::{chain::{self,Chain,ChildCtx,Entry,Reject,Spec},miner::{self,MinerEvent,MinerHandle},net::{self,Msg,NetEvent,Peer,PeerId,Useful},store::{self,Journal},types::*};
mod discovery;
mod imports;
pub mod latency;
pub use discovery::{Discovery,DiscoveryTimes,LINK_BYTES,OUT_STATE_BYTES,Outbound,parse_outbound,endpoints,is_own_seed,SEEDS_MAX,SEED_LINKS_MAX,BOOTSTRAP_MAX};
pub mod canon;

/// D-REORG-REDB: the journal is the off-heap block store. `record_at` reads a block record back by
/// its frame offset for on-demand body fetch (fork choice and retention stay on resident metadata).
impl chain::BlockBytes for Journal{
    fn record_at(&self,off:u64)->Result<(Vec<u8>,Vec<u8>,Vec<u8>),Reject>{
        self.read_at(off).map_err(|e|Reject::Local(format!("journal read_at {off}: {e:?}")))
    }
}
/// The node's block source: journal records, (C-1 S4) the nonce ranges in history.redb and (C-1 S5) the canonical
/// entries below the window in the canonical index.
struct Src<'a>{journal:&'a Journal,history:&'a crate::history::History,canon:&'a canon::Index}
impl chain::BlockBytes for Src<'_>{
    fn record_at(&self,off:u64)->Result<(Vec<u8>,Vec<u8>,Vec<u8>),Reject>{chain::BlockBytes::record_at(self.journal,off)}
    /// A history.redb read that fails is a local failure (review F3), not "ranges unavailable".
    fn ranges_of(&self,hash:&[u8;32])->Result<Option<Vec<(Address,u128,u128)>>,Reject>{self.history.ranges(hash).map_err(|e|Reject::Local(format!("history: {}",e.0)))}
    /// One index read and one journal read (header, body and ParentResult), and the nonce ranges when the entry is
    /// wanted whole (`checkpoint::entry_of`, which checks that the record hashes to the row's block). A row that
    /// does not check marks the index damaged; every failure is the node's own storage (exit 70; the next open rebuilds).
    fn canonical_entry(&self,h:u64,slim:bool)->Result<Option<(Entry,Body)>,Reject>{
        let r=self.canon.at(h).map_err(Reject::Local)?.ok_or_else(||Reject::Local(self.canon.mark_damaged(&format!("no row at height {h} below the window"))))?;
        let off=r.body_off.ok_or_else(||Reject::Local(self.canon.mark_damaged(&format!("the row at height {h} names no journal record"))))?;
        let payload=self.journal.payload_at(off).map_err(|e|Reject::Local(format!("journal read at {off}: {e:?}")))?;
        let ranges=if slim{None}else{Some(match self.history.ranges(&r.hash){Ok(Some(x))=>x,Ok(None)=>return Err(Reject::Local(self.history.mark_damaged("nonce ranges missing for a journaled block").0)),
            Err(e)=>return Err(Reject::Local(format!("history: {}",e.0)))})};
        let why=|m:&str|format!("the row at height {h}: {m}");
        let (e,body)=crate::checkpoint::entry_of(&r,&payload,|_|Ok(()),||Ok(Some(Vec::new())),||Ok(ranges.clone()),store::MAX_RECORD,slim,&why).map_err(|m|Reject::Local(self.canon.mark_damaged(&m)))?;
        if e.height!=h{return Err(Reject::Local(self.canon.mark_damaged(&format!("the row at height {h} names a block at height {}",e.height))))}
        Ok(Some((e,body)))
    }
    fn canonical_hash(&self,h:u64)->Result<Option<[u8;32]>,Reject>{Ok(self.canon.at(h).map_err(Reject::Local)?.map(|r|r.hash))}
}
/// C-1 S5 (PR7 §7): one replay event of open's journal pass, two per record: the record's block (`at` = 2·record)
/// and its parent (`at` = 2·record + 1). One sorted vector of them, 36 bytes each, replaced a map of a queue per hash
/// (its growth measured 189 bytes a record, tests/open_transient.rs): the next event of a hash after record `i` is
/// found by halving (`next_event`).
#[derive(Clone,Copy,PartialEq,Eq,PartialOrd,Ord)]
pub(crate) struct Ev{hash:[u8;32],at:u32}
/// The records `Ev::at` can name: open refuses a longer journal (it holds 2^31 records, years of blocks).
pub const EV_RECORDS_MAX:usize=1<<31;
/// Whether the next replay event of `hash` at or after record `rec` is a child record (Some(true)), its own block's
/// record (Some(false)), or there is none (None).
fn next_event(evs:&[Ev],hash:&[u8;32],rec:usize)->Option<bool>{
    let at=rec as u64*2;let i=evs.partition_point(|e|(&e.hash,u64::from(e.at))<(hash,at));evs.get(i).filter(|e|e.hash==*hash).map(|e|e.at&1==1)
}
/// C-1 S5 (PR7 §7): open's first pass keeps the rolling hash of the journal prefix every ROLL_SAMPLE records (and at
/// its end), not after every record: a prefix in between is rolled again from the sample below it, at most
/// ROLL_SAMPLE − 1 payloads read back (a checkpoint's binding), and replay compares its own running hash at each
/// sample and at the end.
pub const ROLL_SAMPLE:usize=256;
/// C-1 S5: writes the entries the window evicted (`Chain::evicted_out`) to the canonical index, one commit.
fn flush_evicted(chain:&mut Chain,canon:&canon::Index)->Result<(),String>{
    if chain.evicted_out.is_empty(){return Ok(())}
    let rows:Vec<(u64,crate::checkpoint::Rec)>=chain.evicted_out.drain(..).map(|e|(e.height,crate::checkpoint::rec_of_meta(&e))).collect();
    canon.put(&rows).map_err(|e|format!("canonical index: {e}"))?;crate::crash_point("canon");Ok(())
}
/// NeedsRebuild retries of one block: its parent, and (C-1 S4) each slimmed evidence ancestor restored in
/// turn (only after the head fell back more than R_keep). Beyond them the block is refused, never accepted.
const REBUILD_TRIES:usize=17;
/// PR4 (I-5): deep TrieRebuilds (import_with's NeedsRebuild path for a missing parent state; a C-1 S4 slim
/// restore, header and ranges only, is not charged) an unsolicited flood may drive. Token bucket:
/// capacity REBUILD_BURST, one token back every REBUILD_REFILL_MS. The seal cache (chain::COSTLY_MAX) only
/// reduces cost: COSTLY_MAX+1 PoW-valid seals cycled evict each other (PR4 plan §7 finding B). A real heavier
/// branch dropped here is re-fetched by the sync session its peer's Status starts (network.md D38), so no
/// block honest peers accept is lost.
pub const REBUILD_BURST:u32=8;
pub const REBUILD_REFILL_MS:u128=1000;
/// Deep rebuilds (not slim restores) one sync session may drive before it is ended as not-reached. An honest adoption within
/// R_keep needs one (the fork point); the rest extend resident state. Only blocks on the session's branch
/// draw on it (`on_session_branch`): a side block the session's peer relays is unsolicited for the budget, so
/// relayed side traffic cannot end an honest heal session. This bounds a peer whose lying Hello makes its blocks
/// solicited (PR4 plan §7 finding C) to SESSION_REBUILD_MAX per session, and sessions are gated by
/// SESSION_GAP_MS and the round-robin among claimants.
pub const SESSION_REBUILD_MAX:u32=4;

pub struct Config{
    pub dir:PathBuf,
    pub name:String,
    pub miner:Option<Key>,
    pub listen:Option<SocketAddr>,
    pub peers:Vec<SocketAddr>,
    pub rpc:Option<SocketAddr>,
    /// Optional: stop after the canonical head reaches this height.
    pub stop_at:Option<u64>,
    pub log:bool,
    /// A05 local resource bounds (P2P and RPC separately).
    pub limits:Limits,
}
#[derive(Clone,Debug)]
pub struct Limits{pub net:net::Limits,pub rpc:crate::rpc::Limits,
    /// Node memory budget for the A13 resident bound (resources::check at open).
    pub mem_budget:u64,
    /// Observed journal records for the continuous resource projection with eviction off, never a consensus limit.
    pub continuous_history:Option<u64>,
    /// Set by open from the admitted fork schedule. The timing index grows on
    /// disk with journal records even when the execution history is bounded.
    pub first_imports:bool,
    /// C-1 S5 (PR7 §6.2): the observed sizes of history.redb, state.redb and canon.redb, which the continuous chain's
    /// bound counts with eviction on (None: not observed, and that bound is None). Node::open observes them before
    /// its checks and import_with every 256 records; a library caller passes them, as `continuous_history`.
    pub disk:Option<crate::resources::DiskBytes>,
    /// Local retention (consensus.md:29, governance.md:15; never validity): R_keep and the cap on
    /// unprotected non-canonical metadata (chain::R_KEEP, chain::SIDE_BLOCKS_MAX). Live import and
    /// replay must use the same values; tests lower them to reach the boundaries quickly.
    pub r_keep:u64,pub side_cap:usize,
    /// Decision D-7 (local pool policy, never validity): the longest one slice of pool revalidation runs
    /// on the consensus thread before the next event is taken (POOL_SLICE), and the bound on the pooled
    /// transactions' summed declared gas limits (None: POOL_GAS_BLOCKS × GAS_LIMIT, approved 2026-10-25;
    /// a set value must be at least GAS_LIMIT, and A13 counts the pool at it: `pool_gas_bound`).
    pub pool_slice:Duration,pub pool_gas_max:Option<u64>,
    /// PR6 (local pool policy, never validity): the most declared gas one sender's pooled transactions may sum to
    /// (None: POOL_SENDER_GAS_BLOCKS × GAS_LIMIT; a set value must be at least GAS_LIMIT and at most the pool's
    /// gas bound: `pool_sender_gas_bound`). Transactions returned from an abandoned block are not counted.
    pub pool_sender_gas_max:Option<u64>,
    /// C-1 S1: how many canonical blocks below the head stay resident (None: K_hist). Older canonical
    /// history is served from the journal and history.redb. A test reference sets u64::MAX (none offloaded).
    pub history_window:Option<u64>,
    /// C-1 S3a off: every retained state is kept full (the behaviour before S3a). A test reference only.
    pub full_states:bool,
    /// C-1 S4: canonical journaled entries deeper than this are slimmed (None: 2·R_keep + E_win + 257;
    /// u64::MAX: never, a test reference).
    pub slim_depth:Option<u64>,
    /// C-1 E4: states are kept as trie versions in `state.redb` (true) or whole in memory (false, the
    /// behaviour before E4 and a test reference).
    pub state_store:bool,
    /// C-1 E4.4 (D-6): journal records between two local checkpoints of a store-mode node (0: none).
    pub checkpoint_every:u64,
    /// C-1 S5 (PR7): canonical entries more than this many blocks below the head leave memory for the canonical
    /// index (`Chain::window`). None: the default, `resources::evict_depth_of`; Some(0): off (every entry stays
    /// resident, the behaviour before PR7); Some(u64::MAX): never evicts (a test reference). A value below the
    /// smallest safe depth is refused at open. Node::open sets the value it uses here.
    pub evict_depth:Option<u64>,
    /// Peer discovery (VA-SPEC §1a; local policy, never validity): on by default, local mode off.
    pub discovery:Discovery}
impl Default for Limits{fn default()->Self{Self{net:Default::default(),rpc:Default::default(),mem_budget:MEM_BUDGET,continuous_history:None,first_imports:false,disk:None,r_keep:chain::R_KEEP,side_cap:chain::SIDE_BLOCKS_MAX,pool_slice:POOL_SLICE,pool_gas_max:None,pool_sender_gas_max:None,history_window:None,full_states:false,slim_depth:None,state_store:false,checkpoint_every:crate::checkpoint::EVERY,evict_depth:None,discovery:Discovery::default()}}}
/// C-1 S5 (PR7): what a view of a canonical block below the window reads besides its index row and journal
/// record: its receipts (history.redb) only when asked.
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub struct Want{pub receipts:bool}
/// What a view that cannot read its block's history answers (the store is logged, marked and rebuilt at the next open).
pub const HISTORY_UNAVAILABLE:&str="block history unavailable (logged; the store is rebuilt at the next open)";
/// C-1 S1: replay refills history.redb with non-durable commits and makes them durable every this many blocks.
const HISTORY_SYNC:u64=256;
/// D-7: one slice of pool revalidation. The consensus thread is held for at most this, plus the
/// execution of the one transaction in progress when it ends (one transaction is at most GAS_LIMIT,
/// which the R14/R19 gate bounds by t_exec).
pub const POOL_SLICE:Duration=Duration::from_millis(50);
/// D-6 (2026-10-27): the `release` rows one release step works off (`StateStore::release_step`). At the
/// network's state caps a step, fsynced, held the consensus thread 32-45 ms on average (180 ms at most, two
/// runs), against 8-11 s for the whole release inside a checkpoint's write.
pub const RELEASE_SLICE:usize=256;
/// Default memory budget of one single-host devnet node.
pub const MEM_BUDGET:u64=6<<30;
/// Blocks waiting for their timestamp, buffered templates, equivocation observations,
/// status rejects and evidence awaiting inclusion (A13 accounting).
pub const FUTURE_MAX:usize=64;pub const TEMPLATES_MAX:usize=64;pub const SEEN_MAX:usize=4096;pub const REJECTS_MAX:usize=256;pub const EVIDENCE_POOL_MAX:usize=64;
/// V3-04 (v3 profiles only, A13 `history/first-import template first-seen times`): per signed-template slot (the
/// seen_templates key: parent and signer, height), the TemplateID this node first LOCKED there and when, kept for
/// classifying its solved block. At most TEMPLATE_SEEN_MAX slots; each is TEMPLATE_SEEN_ENTRY_BYTES of a HashMap
/// bucket at most (resources.rs asserts that the type fits).
pub const TEMPLATE_SEEN_MAX:usize=256;pub const TEMPLATE_SEEN_ENTRY_BYTES:usize=96;
/// The TemplateID a template_seen slot holds once its signer equivocated there: no template of the slot counts
/// (a keccak256 output, which a TemplateID is, is never all zero in practice).
const NO_TEMPLATE:[u8;32]=[0;32];
/// Gossip: TemplateIDs already relayed (each is relayed at most once), and the shares relayed per locked
/// template, as a multiple of S_max (a block carries at most S_max shares; more only cost bandwidth).
pub const RELAYED_MAX:usize=256;pub const SHARE_RELAY_PER_S_MAX:usize=4;
/// V3-02 (node-local, v3 program set only): the shares seen for the templates a carrier can still credit, at most
/// SHARE_RELAY_PER_S_MAX·S_max nonces each (`LateCache`); a proposer carries the uncredited ones as kind-3 evidence.
/// `height` is the template's block height.
#[derive(Clone,Debug)]
pub struct LateTemplate{pub tid:[u8;32],pub height:u64,pub nonce_max:u128,pub t_share:Fixed,pub shares:Vec<u64>}
/// V3-02: the templates of the production height (not yet canonical) the late-share cache keeps, newest first: the
/// template being left and the one before it (an attempt change, or a signed template and its equivocation fallback).
pub const LATE_PENDING_MAX:usize=2;
/// V3-02: the late-share cache, oldest first (A13: resources.rs "queues/late-share cache"). After each `prune` it holds
/// the canonical template of each height in [max(1, H+1-D), H] (H the head's height: the window of the next carrier)
/// and at most LATE_PENDING_MAX templates of height H+1; one more between a `stash` and its `prune`.
#[derive(Clone,Debug,Default)]
pub struct LateCache{pub templates:VecDeque<LateTemplate>}
impl LateCache{
    /// Adds a template (or merges its shares into the cached copy); each keeps at most `cap` nonces, in ascending order.
    pub fn stash(&mut self,t:LateTemplate,cap:usize){
        if let Some(x)=self.templates.iter_mut().find(|x|x.tid==t.tid){for n in t.shares{x.add(n,cap);}return}
        let mut fresh=LateTemplate{tid:t.tid,height:t.height,nonce_max:t.nonce_max,t_share:t.t_share,shares:Vec::new()};for n in t.shares{fresh.add(n,cap);}
        self.templates.push_back(fresh);
    }
    /// Keeps only what a carrier above a head at height `head` can use: a template of height j in [max(1, head+1-d),
    /// head] only when it is the branch's template at j (`canonical(j)`: a dead attempt, an equivocation's other
    /// template or a sibling's never pushes a canonical one out), and the newest LATE_PENDING_MAX of height head+1.
    pub fn prune(&mut self,head:u64,d:u64,canonical:impl Fn(u64)->Option<[u8;32]>){
        let lo=(head+1).saturating_sub(d).max(1);
        self.templates.retain(|t|t.height>=lo&&(t.height>head||canonical(t.height)==Some(t.tid))&&t.height<=head+1);
        let pending=self.templates.iter().filter(|t|t.height==head+1).count();
        let mut drop=pending.saturating_sub(LATE_PENDING_MAX);
        self.templates.retain(|t|{if drop>0&&t.height==head+1{drop-=1;false}else{true}});
    }
    pub fn get(&self,tid:&[u8;32])->Option<&LateTemplate>{self.templates.iter().find(|x|x.tid==*tid)}
    /// Takes a cached template out (it is locked again, and mined: its shares go back to the production's).
    pub fn remove(&mut self,tid:&[u8;32])->Option<LateTemplate>{let i=self.templates.iter().position(|x|x.tid==*tid)?;self.templates.remove(i)}
    /// A share received for a cached template, kept when it is below its nMax, meets its share target and fits the cap.
    /// None when the template is not cached, else whether the share was new and kept (to be relayed).
    pub fn take(&mut self,tid:&[u8;32],nonce:u64,cap:usize)->Option<bool>{
        let x=self.templates.iter_mut().find(|x|x.tid==*tid)?;
        Some((nonce as u128)<x.nonce_max&&le_target(&pow_hash(tid,nonce),&x.t_share)&&x.add(nonce,cap))
    }
    pub fn len(&self)->usize{self.templates.len()}
    pub fn is_empty(&self)->bool{self.templates.is_empty()}
}
impl LateTemplate{
    /// Adds a nonce within `cap`; true when it was not there and was added.
    fn add(&mut self,n:u64,cap:usize)->bool{if self.shares.len()>=cap{return false}match self.shares.binary_search(&n){Err(i)=>{self.shares.insert(i,n);true}Ok(_)=>false}}
}
/// F7: hard cap on the persisted attempt-0 signing record (a backstop beyond the reorg-window prune).
pub const SIGNED_A0_MAX:usize=4096;
/// Events waiting for the consensus thread (all sources); senders block when it is full.
/// D-A13-BUDGET: 256 -> 128 (network.md: queue <= 256); a full queue blocks its senders.
pub const EVENT_QUEUE:usize=128;
/// A13 (resources, "queues/node peer and status maps"): the bytes counted for one entry of `peers` (a
/// `(PeerId,Peer)` bucket), one of `peer_status` (a `(PeerId,PeerStatus)` bucket) and one peer's address text
/// (`Peer::addr`, at most 58 characters, `[v6%scope]:port`). Fixed, so the pinned total does not follow rustc's
/// layout; resources.rs asserts that the types fit.
pub const PEER_ENTRY_BYTES:usize=192;pub const STATUS_ENTRY_BYTES:usize=96;pub const ADDR_TEXT_BYTES:usize=64;
/// Orphans (blocks whose parent is unknown): parent keys, blocks per parent and bytes, all peers together.
pub const ORPHAN_PARENTS_MAX:usize=64;pub const ORPHANS_PER_PARENT:usize=16;pub const ORPHAN_BYTES_MAX:usize=16*1024*1024;
/// Receipt times kept for production (only recent parents matter).
pub const RECEIVED_MAX:usize=1024;
/// Sync sessions (network.md D38: Status, locator <= 32, count <= 512): blocks asked per round, the
/// quiet time that ends a round (the next round is asked), the silence that ends a session (the peer's
/// claim is then set aside until its next Hello), and the gap between session starts.
pub const SYNC_COUNT:u64=512;pub const ROUND_IDLE_MS:u128=250;pub const SESSION_STALL_MS:u128=3000;pub const SESSION_GAP_MS:u128=1000;

/// A peer's announced Status (Hello): head, height and cumulative work. Unverified until its blocks
/// arrive; `stale` once a session with it ended without reaching that head (cleared by its next Hello).
#[derive(Clone,Debug)]
pub struct PeerStatus{pub work:U256,pub head:[u8;32],pub height:u64,pub stale:bool}
/// One sync session with one peer (D-REORG-REDB re-fetch): the blocks it delivers are solicited
/// (accepted even if retention removed them before) and protected from the side cap; `tip` is the
/// last block it delivered, the locator head of the next round.
#[derive(Clone,Debug)]
pub struct Session{pub peer:PeerId,pub tip:Option<[u8;32]>,pub asked_ms:u128,pub last_ms:u128,pub got:u64,pub got_round:u64,pub rounds:u64,
    /// Highest height the session has delivered a stored block at: a copy of a block already stored
    /// counts as progress only above it and inside the R_keep window, so replayed or junk blocks cannot
    /// keep a session alive (`last_ms` is the time of the last progress).
    pub best_h:u64,
    /// PR4 (I-5): deep rebuilds its branch's blocks drove (at most SESSION_REBUILD_MAX), and the head height the
    /// current round's locator was built at (`on_session_branch`).
    pub rebuilds:u32,pub loc_head:u64}

/// Rpc: a request for the consensus thread (rpc::Request).
/// Wake: light RPC requests are waiting in the light lane (at most one Wake is pending).
pub enum Event{Net(NetEvent),Miner(MinerEvent),Rpc(crate::rpc::Request),Tick,Wake}
/// Light requests run before each event the consensus thread takes (and between batch
/// members): at most LIGHT_TURN, so a light flood delays any other event by at most LIGHT_TURN
/// light executions; a lane left non-empty wakes the node again.
pub const LIGHT_TURN:usize=8;
/// Queued non-RPC events run between two members of an RPC batch (`drain_between_members`).
pub const EVENT_TURN:usize=8;
/// `submit_tx_by`'s refusal when the request's COMPUTE_DEADLINE passed before the transaction was pooled.
pub const SUBMIT_DEADLINE:&str="-32020 compute deadline (not pooled)";
/// `submit_tx_by`'s refusal when the pool's revalidation backlog (D-7) did not finish by the request's deadline.
pub const SUBMIT_BUSY:&str="-32021 pool revalidation in progress (not pooled)";
/// `admit_tx`'s refusal of a transaction whose declared gas limit would take the pooled sum past the bound (D-7).
pub const POOL_FULL_GAS:&str="-32016 pool full (gas)";
/// `admit_tx`'s refusal of a transaction that would take its sender's pooled declared gas past the sender quota
/// (PR6 P1, `pool_sender_gas_bound`).
pub const POOL_FULL_SENDER:&str="-32016 pool full (sender gas)";
/// `admit_tx`'s answer for a valid transaction refused only by the pool's gas bound and held (parked) to be
/// admitted at the next head, ahead of later arrivals (PR6 P2). It is not executed; it is relayed now only within
/// `park`'s bound (PR6 second review), else when admitted.
pub const POOL_PARKED:&str="-32016 pool full (gas): held for the next block";
pub fn now_ms()->u128{SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()}
pub fn now_s()->u64{(now_ms()/1000) as u64}

struct Locked{header:Header,body:Body,tid:[u8;32],ctx:ChildCtx,fallback:bool}
/// Propose: Draw(h,0) waits until its template timestamp max(L(0),C) is not in the
/// future for receivers (C+Φ >= L(0)); with Φ=0 a template built earlier is rejected.
enum Mode{Wait{deadline_ms:u128},Locked(Box<Locked>),Idle,Propose{at_s:u64}}
struct Production{parent:[u8;32],height:u64,attempt:u32,mode:Mode,shares:BTreeSet<u64>,miner:Option<MinerHandle>,solved:bool,received_ms:u128}

/// Pending pool: transactions are kept only if they execute in order on top of
/// the head state (pending state), so several nonces per sender are allowed. The pending
/// state is normalized after every transaction and follows the block caps (`PoolMeter`), so
/// it is bounded like a snapshot (A13).
/// The code total of a state that passes the profile's code limits, or None (then every submission runs
/// the full checks, as before).
fn checked_code_total(state:&InMemoryDB,spec:&Spec)->Option<u64>{
    let cp=spec.cp();pocol_lp4_execution::code_caps::checked_total(state,pocol_lp4_execution::code_caps::CodeLimits{runtime:cp.B_code_max,logical:cp.CODE_CAP}).ok()
}
/// C-1 E2/E3: a full entry's code total once its state passes the code limits, from its summary (a
/// whole-state check without one).
fn summary_code_total(e:&Entry,spec:&Spec)->Option<u64>{
    let cp=spec.cp();let lim=pocol_lp4_execution::code_caps::CodeLimits{runtime:cp.B_code_max,logical:cp.CODE_CAP};
    match &e.summary{Some(s)=>s.code_check(lim).ok().map(|_|s.inventory()),
        // C-1 E4: a stored state without its summary is summarized in place.
        None if e.stored.is_some()=>{let s=pocol_lp4_execution::state_summary::Summary::scan_view(&chain::StateHandle::of(e),cp.B_code_max);s.code_check(lim).ok().map(|_|s.inventory())}
        None=>checked_code_total(&e.state,spec)}
}
/// The pool's meter and code total for a new pending state over `head`: from its summary, or counted.
fn pool_start(head:&Entry,spec:&Spec)->(PoolMeter,Option<u64>){
    let meter=match &head.summary{Some(s)=>PoolMeter::from_counts(s.user_slots,s.user_accounts,s.sys_slots,spec.profile()),
        None if head.stored.is_some()=>{let s=pocol_lp4_execution::state_summary::Summary::scan_view(&chain::StateHandle::of(head),spec.cp().B_code_max);PoolMeter::from_counts(s.user_slots,s.user_accounts,s.sys_slots,spec.profile())}
        None=>PoolMeter::of(&head.state,spec.profile())};
    (meter,summary_code_total(head,spec))
}
/// `code_total`: the pending state's logical code total once it is known to pass the code limits (the head
/// state is checked once per new head; decision package C-2), so a submission does not rescan the state.
/// C-1 E3: the pending state is a layer of the pool's writes over the head state it was started on
/// (`pending_base`, shared with the head entry), not a copy of the head.
pub struct Pool{pub txs:Vec<(B256,Vec<u8>)>,pending:pocol_lp4_execution::view::Layer,pending_base:chain::StateHandle,pub bytes:usize,meter:PoolMeter,code_total:Option<u64>,
    /// D-7: what a new head left to revalidate, in pool order. An abandoned block (its journal
    /// offset) expands to its transactions when reached; a transaction is re-executed on the pending
    /// state. `backlog_bytes` counts the transactions held here against POOL_MAX_BYTES.
    backlog:VecDeque<PoolItem>,backlog_bytes:usize,
    /// D-7: summed declared gas limits of `txs`, and their bound (`pool_gas_bound`, at most POOL_MAX_TXS x GAS_LIMIT,
    /// the pending-layer term A13 counts). N4: an entry is charged the most gas it or a replacement of it declared
    /// since the last head (`Meta::gas`), and `gas` adds `ghost_gas`, the gas of the evicted transactions (and of a
    /// returned transaction's replacement declaring less) whose writes stay in the pending layer until the next head.
    /// `gas` never falls between two heads, so it still bounds what the layer holds (A13), and a new head recounts it.
    pub gas:u64,pub gas_max:u64,pub ghost_gas:u64,
    /// PR6 P4: per pooled transaction (aligned with `txs`), its sender, declared gas limit and whether it is
    /// exempt from the sender quota (returned from an abandoned block); and the quota (`pool_sender_gas_bound`).
    /// N4: also its replacements since the last head (`Meta`); its nonce and fee caps are read from its bytes
    /// (`TxFields`), so the entry keeps its PR6 size.
    meta:Vec<Meta>,pub sender_gas_max:u64,
    /// N4 (V3-20, node-local): replacements relayed since the last head (at most POOL_REPLACE_RELAY_MAX).
    replace_relays:usize,
    /// PR6 P2: transactions refused only by the gas bound and held for the next head (at most `parked_max`
    /// <= POOL_PARKED_MAX; per sender a run of its next nonces, in arrival order); their bytes share POOL_MAX_BYTES
    /// with `txs` and the backlog; and the arrival counter that orders them. PR6 review: they are template
    /// candidates and are relayed when parked (PR6 second review: within `parked_relays`).
    parked:Vec<Parked>,parked_bytes:usize,park_seq:u64,parked_max:usize,
    /// PR6 second review: the transactions relayed when parked since the last head (at most `parked_max`).
    parked_relays:usize}
/// A parked transaction (PR6 P2): its declared gas, nonce, arrival, its sender's run (the declared gas and arrival
/// of the sender's first parked one, which orders the run at the next head), sender, hash, its cost at its fee cap
/// (gas_limit × fee cap + value), how it is relayed once admitted, and its bytes. PR6 third review: `ahead`, the
/// number of pooled transactions when it was parked. The pool only grows between two heads, so the pooled
/// transactions at an index below it arrived before it and the others after it (the big-first slot, B6).
/// N4: its replacements since the last head; a replacement keeps the entry's arrival, run and `ahead`. Its fee caps
/// are read from its bytes (`TxFields`).
struct Parked{gas:u64,nonce:u64,seq:u64,run:(u64,u64),ahead:u64,sender:Address,hash:B256,cost:U256,relay:Relay,raw:Vec<u8>,repl:u8}
/// N4 (V3-20, node-local): a pooled transaction's entry beside its raw bytes (`Pool::meta`): sender, charged gas,
/// whether it is exempt from the sender quota (PR6 P4) and its replacements since the last head. N4 second review:
/// `gas` is what the entry is charged in `Pool::gas` and the sender quota: its declared gas, or after replacements
/// since the last head the most that it (the executed one) or any of them declared, so lowering and raising again
/// charges nothing twice. The current declaration is read from the bytes (`TxFields::gas`).
#[derive(Clone,Copy,Debug)]
struct Meta{gas:u64,sender:Address,exempt:bool,repl:u8}
/// N4 (V3-20, node-local): what the fee market reads of a pooled or parked transaction: its nonce, declared gas
/// limit, fee caps in wei per gas (maxFeePerGas and maxPriorityFeePerGas; a legacy or EIP-2930 transaction's gas
/// price for both, as revm prices it) and value. Read from its raw bytes (`tx_fields`, no signature recovery) rather
/// than stored beside it, so the pool's entries keep their PR6 sizes (A13).
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub struct TxFields{pub nonce:u64,pub gas:u64,pub cap:U256,pub tip:U256,pub value:U256}
impl TxFields{
    pub fn of(tx:&revm::primitives::TxEnv)->TxFields{TxFields{nonce:tx.nonce.unwrap_or(0),gas:tx.gas_limit,cap:tx.gas_price,tip:tx.gas_priority_fee.unwrap_or(tx.gas_price),value:tx.value}}
    /// The effective tip per gas at `base`, the fee order's key: min(tip cap, fee cap - base fee), or 0 when the fee
    /// cap is below the base fee (such a transaction is not includable now; it is tried last and fails then).
    /// Saturated at u64::MAX wei per gas (18.4 ether per gas): tips past it compare equal and the candidate order
    /// decides between them.
    pub fn effective_tip(&self,base:U256)->u64{if self.cap<base{0}else{u64::try_from(self.tip.min(self.cap-base)).unwrap_or(u64::MAX)}}
    /// gas_limit x fee cap + value, what the sender must hold for it (None past U256).
    pub fn cost(&self)->Option<U256>{U256::from(self.gas).checked_mul(self.cap).and_then(|c|c.checked_add(self.value))}
    /// Whether `self` replaces `old` (replace-by-fee): both caps strictly higher and each at least POOL_PRICE_BUMP
    /// percent above the old one (EIP-1559 clients' rule).
    pub fn bumps(&self,old:&TxFields)->bool{
        let need=|o:U256|o.saturating_mul(U256::from(100+POOL_PRICE_BUMP));let have=|n:U256|n.saturating_mul(U256::from(100));
        self.cap>old.cap&&self.tip>old.tip&&have(self.cap)>=need(old.cap)&&have(self.tip)>=need(old.tip)
    }
    /// The least caps that replace `old`: each strictly above it and at least POOL_PRICE_BUMP percent above it.
    pub fn min_replacement(old:&TxFields)->(U256,U256){
        let f=|o:U256|o.saturating_mul(U256::from(100+POOL_PRICE_BUMP)).div_ceil(U256::from(100)).max(o.saturating_add(U256::from(1)));
        (f(old.cap),f(old.tip))
    }
}
/// One RLP item at the start of `b`: whether it is a list, its payload and the bytes after it.
fn rlp_item(b:&[u8])->Option<(bool,&[u8],&[u8])>{
    let (&h,rest)=b.split_first()?;
    let long=|n:usize,r:&[u8]|->Option<(usize,usize)>{if n>8||r.len()<n{return None}Some((r[..n].iter().fold(0usize,|a,x|(a<<8)|*x as usize),n))};
    let (list,len,skip)=match h{0x00..=0x7f=>return Some((false,&b[..1],rest)),0x80..=0xb7=>(false,(h-0x80) as usize,0),
        0xb8..=0xbf=>{let (l,n)=long((h-0xb7) as usize,rest)?;(false,l,n)},0xc0..=0xf7=>(true,(h-0xc0) as usize,0),_=>{let (l,n)=long((h-0xf7) as usize,rest)?;(true,l,n)}};
    let rest=&rest[skip..];if rest.len()<len{return None}Some((list,&rest[..len],&rest[len..]))
}
/// N4: a decoded transaction's `TxFields` read from its raw bytes (wire.rs's layouts: legacy [nonce, gasPrice, gas,
/// to, value, ..], EIP-2930 [chainId, nonce, gasPrice, gas, to, value, ..], EIP-1559 [chainId, nonce,
/// maxPriorityFeePerGas, maxFeePerGas, gas, to, value, ..]; `gas` is the gas limit). Only the leading fields are walked; no allocation and
/// no signature recovery. The pool holds only transactions `wire::decode_for_profile` accepted, so this is None only
/// for bytes it would refuse (`pool_audit` checks the two agree).
pub fn tx_fields(raw:&[u8])->Option<TxFields>{
    let (kind,enc)=match *raw.first()?{k@(0x01|0x02)=>(k,&raw[1..]),0xc0..=0xff=>(0,raw),_=>return None};
    let (list,mut p,_)=rlp_item(enc)?;if !list{return None}
    let mut f=[&[][..];7];let n=match kind{0=>5,1=>6,_=>7};
    for x in f.iter_mut().take(n){let (l,v,r)=rlp_item(p)?;if l{return None}*x=v;p=r;}
    let u=|b:&[u8]|(b.len()<=32).then(||U256::from_be_slice(b));let u64_=|b:&[u8]|(b.len()<=8).then(||b.iter().fold(0u64,|a,x|(a<<8)|*x as u64));
    Some(match kind{0=>TxFields{nonce:u64_(f[0])?,gas:u64_(f[2])?,cap:u(f[1])?,tip:u(f[1])?,value:u(f[4])?},1=>TxFields{nonce:u64_(f[1])?,gas:u64_(f[3])?,cap:u(f[2])?,tip:u(f[2])?,value:u(f[5])?},
        _=>TxFields{nonce:u64_(f[1])?,gas:u64_(f[4])?,tip:u(f[2])?,cap:u(f[3])?,value:u(f[6])?}})
}
/// N4 (V3-20, node-local): replace-by-fee. A transaction with a pooled or parked one's sender and nonce replaces it
/// only when both its fee caps rise by at least this many percent (`TxFields::bumps`).
pub const POOL_PRICE_BUMP:u64=10;
/// N4: the most replacements of one pooled or parked entry, and of one sender's entries, between two heads; one more
/// is refused with POOL_REPLACE_LIMIT until the next head. There is no pool-wide limit on accepting them (one sender
/// cannot use up another's), so the entries and senders bound them: at most 4 x (POOL_MAX_TXS + POOL_PARKED_MAX).
pub const POOL_REPLACE_SLOT_MAX:u8=4;pub const POOL_REPLACE_SENDER_MAX:u32=8;
/// N4: the most replacements relayed between two heads. Each is offered to the peers once while fewer were relayed,
/// so replacements cost at most this many relays per head, whatever the bump; a later one is pooled here and not
/// relayed (`Stats::replace_unrelayed`).
pub const POOL_REPLACE_RELAY_MAX:usize=256;
/// N4: the refusal of a replacement whose caps do not both rise by POOL_PRICE_BUMP percent (the answer goes on with
/// the least caps that would replace it). -32000, the code and words EIP-1559 clients use.
pub const POOL_REPLACE_UNDERPRICED:&str="-32000 replacement transaction underpriced";
/// N4: the refusal of a replacement past POOL_REPLACE_SLOT_MAX or POOL_REPLACE_SENDER_MAX since the last head.
pub const POOL_REPLACE_LIMIT:&str="-32016 replacement limit: retry after the next block";
/// N4: the refusal of a new submission (RPC or a peer's relay) whose sender's head-state balance does not cover it
/// with the sender's pooled and parked transactions, each at gas_limit x maxFeePerGas + value (`Node::committed`).
pub const POOL_OVERDRAFT:&str="-32000 insufficient funds for gas * price + value with the sender's pooled and parked transactions";
/// N4: the refusal of a transaction at the pool's count or byte bound that does not pay a higher effective tip than
/// the lowest one it could evict (the answer goes on with that tip), or finds none to evict.
pub const POOL_EVICT_UNDERPRICED:&str="-32016 pool full: tip too low to evict";
/// PR6 P2: the most parked transactions. Each is decoded and validated again at the next head, so this bounds
/// that work (about POOL_PARKED_MAX signature recoveries per head).
pub const POOL_PARKED_MAX:usize=1024;
/// A13 (resources, PR6): the bytes counted for one pool meta entry, one parked entry (its raw bytes are inside
/// POOL_MAX_BYTES) and one append-round candidate (its raw bytes are counted apart). Fixed, so the pinned total
/// does not follow rustc's layout; checked below.
/// N4: the entries keep these sizes (the replacement count fits their padding). FEE_ORDER_ENTRY_BYTES: one candidate
/// of the fee order's scratch (`fee_order_and_big`: its index, which the output reuses, and its priority), of an
/// eviction's scan (`eviction_victim`: a pooled index, or a parked index by sender) or of the parked set's fee
/// eviction (`evict_parked_by_fee`: a parked index and its priority); each allocated at its full length once, never
/// grown. A13 counts it in the place of the selection's former first-seen sender set (`resources::selection_bytes`).
pub const META_ENTRY_BYTES:usize=32;pub const PARKED_ENTRY_BYTES:usize=160;pub const CAND_ENTRY_BYTES:usize=64;pub const FEE_ORDER_ENTRY_BYTES:usize=12;
const _:()=assert!(std::mem::size_of::<u32>()+std::mem::size_of::<u64>()<=FEE_ORDER_ENTRY_BYTES);
const _:()=assert!(std::mem::size_of::<Meta>()<=META_ENTRY_BYTES&&std::mem::size_of::<Parked>()<=PARKED_ENTRY_BYTES&&std::mem::size_of::<Cand>()<=CAND_ENTRY_BYTES);
/// One backlog entry: an abandoned block's journal offset, or a transaction and how to relay it once
/// admitted. N4 review: `Fresh` is a P2P transaction that arrived during the backlog (`relay_tx`), a new submission
/// (it may replace or evict, `admit_tx`), relayed once admitted as it would have been; a `Tx` is revalidated.
enum PoolItem{Block(u64),Tx(Vec<u8>,Relay),Fresh(Vec<u8>)}
/// N4: the entry a replacement takes: a pooled transaction's index, or a parked one's.
#[derive(Clone,Copy,Debug)]
enum Slot{Pooled(usize),Parked(usize)}
/// How an admitted transaction is relayed (a local policy). `Gossip` sends it to every peer, and a peer whose
/// queue is full is dropped. `Offer` is for a transaction returned from an abandoned block (D-7): this node
/// may be the only one holding it, and a node that is not drawn never builds a block, so it is offered to
/// each peer, and a peer with half its queue in use does not get it and is kept (one reorganization returns
/// up to a whole pool at once).
/// `Returned` is such a transaction of the previous pool at a later revalidation: not relayed again, and still
/// exempt from the sender quota (PR6 P1).
#[derive(Clone,Copy,PartialEq,Eq,Debug)]
enum Relay{No,Gossip,Offer,Returned}
/// D-A13-BUDGET: 64 -> 32 MiB (network.md: pool <= 64 MiB); 256 transactions of TX_MAX.
pub const POOL_MAX_BYTES:usize=32*1024*1024;
/// Local limit on pooled transactions (about ten blocks of plain transfers): every new head
/// re-executes the pool, so its length bounds that work.
pub const POOL_MAX_TXS:usize=4096;
/// PR6 B3: the most pool and backlog candidates one template selection decodes (each decode recovers a
/// signature). A backlog flooded during revalidation can hold POOL_MAX_BYTES/MIN_TX_BYTES transactions.
pub const BUILD_VISIT_MAX:usize=POOL_MAX_TXS;
/// The smallest gas a transaction can use and the smallest raw transaction (resources::TX_GAS_MIN,
/// resources::MIN_TX_BYTES): the selection stops once less than either is left.
const TX_GAS_MIN:u64=crate::resources::TX_GAS_MIN as u64;const MIN_TX_BYTES:u64=crate::resources::MIN_TX_BYTES as u64;
/// Encoded body item length (RLP string header included); the three list headers take <= 12 bytes.
fn enc(n:usize)->usize{n+1+if n<56{0}else if n<256{1}else if n<65536{2}else{3}}
/// A pool transaction the selection left out only because it did not fit by gas (PR6 B2), or a later one of
/// its sender; `build_signed_template`'s append round (B5) may add it within the room the real build left.
#[derive(Clone,Debug)]
pub struct Cand{pub sender:Address,pub gas_limit:u64,pub raw:Vec<u8>}
/// A template selection (`Node::template_selection`): the transactions in block order, the candidates kept
/// for the append round (`rest`, in pool order, at most BODY_MAX bytes and the TX_CAP room), the gas counted
/// for `txs` (gas used, or declared gas where `fallback`), and whether declared gas was counted (no checked
/// parent state), and the gas the simulation executed (`executed`: before refunds, committed or left out; at most
/// `selection_exec_budget(GAS_LIMIT)`).
#[derive(Clone,Debug,Default)]
pub struct Selection{pub txs:Vec<Vec<u8>>,pub rest:Vec<Cand>,pub counted:u64,pub fallback:bool,pub executed:u64}
/// PR6 B5: the most `build_signed` calls one signed template takes (the retry loop's bound before PR6).
pub const TEMPLATE_BUILDS_MAX:usize=8;
/// Whether a `build_signed` refusal is the USER phase's gas check (`apply User("Gas")`: a transaction's
/// declared gas exceeded what the block had left). It carries no transaction index.
pub fn is_user_gas(why:&str)->bool{why.ends_with("User(\"Gas\")")}
/// PR6 review: the log cap a transaction's own logs pass in a child of a block whose chain charged `cum` log bytes,
/// if any: the block meter's log checks (`user_phase.rs` BlockMeter::check) with nothing before it in the block,
/// LOG_COUNT_MAX, LOG_BLOCK_MAX and, when H_END != 0, LOG_CAP. Such a transaction halts in every block built on
/// that parent, wherever it is placed.
pub fn own_logs_halt(cp:&pocol_lp4_execution::profile::ConsensusParams,cum:u64,logs:&[revm::primitives::Log])->Option<user_phase::Halt>{
    own_logs_halt_for(cp,cum,logs,false)
}
pub fn own_logs_halt_for(cp:&pocol_lp4_execution::profile::ConsensusParams,cum:u64,logs:&[revm::primitives::Log],v3:bool)->Option<user_phase::Halt>{
    use user_phase::{caps,Halt};let (n,c)=(logs.len() as u64,user_phase::log_charge(logs));let limits=user_phase::log_caps(v3);
    if n>limits.count{Some(Halt::LogCount)}else if c>limits.bytes{Some(Halt::LogBytes)}
    else if cp.H_END!=0&&cum.saturating_add(c)>caps::LOG_CAP{Some(Halt::LogCap)}else{None}
}
/// PR6 review: an exceptional halt on the selection's overlay, as the block applies it (`user_phase.rs`
/// apply_halt): the sender's nonce + 1, the sender pays gas_limit·price, the escrow receives gas_limit·tip; nothing
/// else changes. None on an arithmetic failure (unreachable after the engine's balance check).
fn sim_halt<S:pocol_lp4_execution::view::WorkState+?Sized>(st:&mut S,caller:Address,gas_limit:u64,price:U256,tip:U256)->Option<()>{
    let fee=price.checked_mul(U256::from(gas_limit))?;let reward=tip.checked_mul(U256::from(gas_limit))?;
    let mut s=st.account(&caller).and_then(|a|a.info()).unwrap_or_default();
    s.nonce=s.nonce.checked_add(1)?;s.balance=s.balance.checked_sub(fee)?;st.insert_info(caller,s);
    if reward!=U256::ZERO{let e=pocol_lp4_execution::pay::escrow_address();let mut c=st.account(&e).and_then(|a|a.info()).unwrap_or_default();
        c.balance=c.balance.checked_add(reward)?;st.insert_info(e,c);}
    Some(())
}
/// PR6 second review: the most gas one template selection executes, committed or not: GAS_LIMIT for candidates it
/// executes and then leaves out (B4), plus 5/4 GAS_LIMIT, the most its committed transactions spend (EIP-3529), so
/// the bound never binds before GAS_LIMIT of execution was wasted.
pub fn selection_exec_budget(gas_limit:u64)->u64{gas_limit.saturating_add(gas_limit.saturating_mul(5)/4)}
/// The gas an execution spent, before the refund (EIP-3529): what the engine ran.
fn executed_gas(r:&revm::primitives::ExecutionResult)->u64{match r{revm::primitives::ExecutionResult::Success{gas_used,gas_refunded,..}=>gas_used.saturating_add(*gas_refunded),r=>r.gas_used()}}
/// PR6 second review: the engine's checks before execution, with no state read (revm 19.7 validate_env and
/// validate_initial_tx_gas under Cancun, as `native::build_engine` runs them, and EIP-2681's nonce): the block's
/// gas limit, chain id, fee cap order, initcode size and intrinsic gas (calldata, access list, CREATE, initcode
/// words). Parking executes nothing, so it refuses what these refuse: such a transaction can never be included or
/// paid. The answer is the engine's (`-32000 {e:?}`). The fee cap against the next base fee stays `park`'s own check
/// (POOL_FULL_GAS: the base fee moves), so the base fee is left at zero here.
fn pre_execution(tx:&revm::primitives::TxEnv,ctx:&NativeBlockContext,chain_id:u64)->Result<(),String>{
    use revm::{handler::mainnet::{validate_env,validate_initial_tx_gas},primitives::{BlobExcessGasAndPrice,CancunSpec,EVMError,Env,InvalidTransaction}};
    if tx.nonce==Some(u64::MAX){return Err(format!("-32000 {:?}",EVMError::<core::convert::Infallible>::Transaction(InvalidTransaction::NonceOverflowInTransaction)))}
    let mut env=Env::default();env.cfg.chain_id=chain_id;env.tx=tx.clone();
    let b=&mut env.block;b.number=ctx.height;b.timestamp=ctx.timestamp;b.gas_limit=ctx.gas_limit;b.prevrandao=Some(ctx.prevrandao);
    b.coinbase=pocol_lp4_execution::pay::escrow_address();b.blob_excess_gas_and_price=Some(BlobExcessGasAndPrice::new(0,false));
    validate_env::<CancunSpec,revm::db::EmptyDB>(&env).and_then(|_|validate_initial_tx_gas::<CancunSpec,revm::db::EmptyDB>(&env).map(|_|())).map_err(|e|format!("-32000 {e:?}"))
}
/// PR6 B5, the append round: indices into `rest` to add after a built block, within `gas` of room counting
/// declared gas, the TX_CAP/TXBYTES room and the BODY room. The head of each sender's queue (pool order) is a
/// candidate; the smallest declared gas goes first (ties in pool order); a head that does not fit stops its
/// sender's queue.
fn append_round(rest:&[Cand],mut gas:u64,mut txs:u64,mut bytes:u64,mut body:usize)->Vec<usize>{
    use std::{cmp::Reverse,collections::BinaryHeap};
    let mut queues:Vec<VecDeque<usize>>=Vec::new();let mut of:HashMap<Address,usize>=HashMap::new();
    for (i,c) in rest.iter().enumerate(){let q=*of.entry(c.sender).or_insert_with(||{queues.push(VecDeque::new());queues.len()-1});queues[q].push_back(i);}
    let mut heap:BinaryHeap<Reverse<(u64,usize,usize)>>=queues.iter().enumerate().filter_map(|(q,v)|v.front().map(|&i|Reverse((rest[i].gas_limit,i,q)))).collect();
    let mut out=Vec::new();
    while let Some(Reverse((g,i,q)))=heap.pop(){
        let n=rest[i].raw.len();
        if g>gas||txs==0||n as u64>bytes||enc(n)>body{continue}
        gas-=g;txs-=1;bytes-=n as u64;body-=enc(n);out.push(i);queues[q].pop_front();
        if let Some(&j)=queues[q].front(){heap.push(Reverse((rest[j].gas_limit,j,q)));}
    }
    out
}
/// Decision D-7 (approved 2026-10-25, local pool policy): by default the pooled transactions' declared gas
/// limits sum to at most this many GAS_LIMITs. It bounds the gas a new head re-executes and what the
/// pending layer can hold; it bounds no execution time (gas is not time), which the slices do.
pub const POOL_GAS_BLOCKS:u64=4;
/// The pool's gas bound: `Limits::pool_gas_max`, or POOL_GAS_BLOCKS × GAS_LIMIT. A value below GAS_LIMIT is
/// refused: a transaction a block can hold must fit an empty pool.
pub fn pool_gas_bound(cp:&pocol_lp4_execution::profile::ConsensusParams,limits:&Limits)->Result<u64,String>{
    let g=match limits.pool_gas_max{Some(g)=>g,None=>cp.GAS_LIMIT.checked_mul(POOL_GAS_BLOCKS).ok_or("pool gas bound: overflow")?};
    if g<cp.GAS_LIMIT{return Err(format!("pool_gas_max {g} is below GAS_LIMIT {}: a transaction a block can hold could never be pooled",cp.GAS_LIMIT))}
    Ok(g)
}
/// N4 second review: the gas bound the pool applies (`Pool::gas_max`): `pool_gas_bound`, at most POOL_MAX_TXS x
/// GAS_LIMIT, the pending layer's term A13 counts (`resources::pool_gas`). Without evictions the count bound binds
/// first above it, so this changes nothing; an eviction leaves its victim's writes in the layer (`ghost_gas`), so
/// without it a large `pool_gas_max` let evictions take the layer past that term.
pub fn pool_gas_in_effect(cp:&pocol_lp4_execution::profile::ConsensusParams,limits:&Limits)->Result<u64,String>{
    Ok(pool_gas_bound(cp,limits)?.min((POOL_MAX_TXS as u64).saturating_mul(cp.GAS_LIMIT)))
}
/// PR6 P1: by default one sender's pooled transactions declare at most this many GAS_LIMITs (a sender pays only
/// for the gas it uses, so declared gas it holds in the pool must be bounded per sender).
pub const POOL_SENDER_GAS_BLOCKS:u64=1;
/// The sender quota: `Limits::pool_sender_gas_max`, or POOL_SENDER_GAS_BLOCKS × GAS_LIMIT. Refused below GAS_LIMIT
/// (a transaction a block can hold must fit an empty quota) and above the pool's gas bound.
pub fn pool_sender_gas_bound(cp:&pocol_lp4_execution::profile::ConsensusParams,limits:&Limits)->Result<u64,String>{
    let q=match limits.pool_sender_gas_max{Some(q)=>q,None=>cp.GAS_LIMIT.checked_mul(POOL_SENDER_GAS_BLOCKS).ok_or("pool sender gas bound: overflow")?};
    if q<cp.GAS_LIMIT{return Err(format!("pool_sender_gas_max {q} is below GAS_LIMIT {}: a transaction a block can hold could never be pooled",cp.GAS_LIMIT))}
    let g=pool_gas_bound(cp,limits)?;
    if q>g{return Err(format!("pool_sender_gas_max {q} is above the pool's gas bound {g}"))}
    Ok(q)
}

pub struct Node{
    pub cfg:Config,
    pub chain:Chain,
    journal:Journal,
    /// C-1 S1: receipts and the transaction index of every validated block (derived; rebuilt by replay).
    pub history:crate::history::History,
    /// C-1 S5 (PR7): the canonical entries below the in-memory window (derived; rebuilt by replay).
    canon:canon::Index,
    peers:HashMap<PeerId,Peer>,
    /// Sync: each peer's announced Status, the live session (at most one), when a session last
    /// started and the round-robin turn among peers announcing more work.
    pub peer_status:HashMap<PeerId,PeerStatus>,pub session:Option<Session>,sync_ask_ms:u128,sync_turn:usize,
    outbound:Arc<Mutex<HashSet<String>>>,
    pub tx:SyncSender<Event>,
    rx:Receiver<Event>,
    /// RPC light lane (network.md classification table): drained before every main event and
    /// between heavy batch members, so a light request never waits behind a queue of heavy work.
    light:crate::rpc::LightLane,light_rx:Receiver<Event>,draining:bool,resp:crate::rpc::RespPools,
    /// RPC events taken off the queue between batch members, run next in arrival order (at most the
    /// heavy queue's places: every queued RPC request holds one). `between`: such a drain is running.
    deferred:VecDeque<Event>,between:bool,
    /// At most one timer Tick waits in the event queue: a tick that takes longer than the timer
    /// period must not fill the queue ahead of blocks, peers and RPC (round 2).
    tick_pending:Arc<std::sync::atomic::AtomicBool>,
    miner_tx:Sender<MinerEvent>,
    prod:Option<Production>,
    orphans:HashMap<[u8;32],Vec<Block>>,
    /// Orphan parent keys, oldest first, and the bytes their blocks hold.
    orphan_order:VecDeque<[u8;32]>,orphan_bytes:usize,
    pub net:net::Net,
    future:Vec<Block>,
    /// Own sealed blocks waiting in `future` for their timestamp (Φ=2φ may be 0 on one host).
    own_future:HashSet<[u8;32]>,
    templates:HashMap<[u8;32],(Header,Body,u128,PeerId)>,
    /// V3-04: seen_key slot -> (TemplateID, first lock ms) of the signed templates this node validated as minable and
    /// locked (`note_template_seen`), NO_TEMPLATE for a slot whose signer equivocated (`template_equivocated`); empty
    /// on profiles that cannot activate v3. Not persisted: a block first imported after a restart whose template was
    /// locked before it is judged on its own arrival.
    template_seen:HashMap<([u8;32],u64),([u8;32],u128)>,
    /// V3-05 telemetry (node-local, every profile): per-proposer template arrival and lock latency in the attempt-0
    /// slots this node waits in (`latency::Tracker`, bounded by latency::BOUND_BYTES; pocol_templateLatency).
    latency:latency::Tracker,
    /// Gossip: TemplateIDs this node sent or relayed (at most RELAYED_MAX), so each is relayed once.
    relayed:HashSet<[u8;32]>,
    received:HashMap<[u8;32],u128>,
    pub pool:Pool,
    pub rejects:VecDeque<(String,String)>,
    pub stats:Stats,
    pub stopped:bool,
    /// C-1 S5 (PR7 §4.4): the LOCAL CAPACITY warning was logged (the bound reached 15/16 of the budget); logged again
    /// only after it fell below.
    capacity_warned:bool,
    /// Equivocation observations and evidence awaiting inclusion (incentives.md Evidence). seen_templates holds at
    /// most SEEN_MAX + 1 keys (`evict_seen_templates`).
    pub seen_templates:HashMap<([u8;32],u64),(Vec<u8>,Vec<u8>,Vec<u8>)>,
    seen_wins:HashMap<([u8;32],u64),([u8;32],[u8;65])>,
    pub evidence_pool:Vec<Vec<u8>>,
    /// V3-02 (v3 program set only): shares seen for the templates a carrier can still credit (`LateCache`).
    pub late_cache:LateCache,
    /// F7 double-sign guard. `signed_a0` records, per (parent_hash, height), the template id this node
    /// has already signed at attempt 0; it is persisted to `signed-a0` in the data dir and fsynced
    /// BEFORE the template is broadcast. A second, different signed template for the same pair is
    /// self-equivocation (type 1) that burns the bond, so once a pair is recorded this node never signs
    /// a different one — on a restart it declines to sign; on a same-process mining-key toggle it
    /// re-broadcasts the cached template from `signed_a0_mem` (not persisted).
    signed_a0:HashMap<([u8;32],u64),[u8;32]>,
    signed_a0_mem:HashMap<([u8;32],u64),(Vec<u8>,Vec<u8>)>,
    /// C-1 E4.4: the node store (store mode) and the checkpoint cadence: records between checkpoints (0:
    /// none), the records the last one covers, and the journal's rolling hash (`checkpoint::roll`).
    state_store:Option<Arc<crate::state_store::StateStore>>,ckpt:Ckpt,
    /// D-6: with both pending, the next background step is a release step (they alternate with pool slices).
    release_turn:bool,
    /// PR4 (I-5): the unsolicited deep-rebuild bucket: tokens left (<= REBUILD_BURST), when tokens were last
    /// given back (0: not yet), and the clock it reads (None: the wall clock, `now_ms`; tests set it). Three
    /// scalars, no A13 term.
    pub rebuild_tokens:u32,pub rebuild_refill_ms:u128,pub rebuild_clock:Option<u128>,
    /// Deterministic first-import clock for in-process simulations; None uses the
    /// wall clock. This never changes the legacy validity/Future clock.
    #[doc(hidden)] pub import_clock:Option<u128>,
    /// Genesis has no journal frame. Its original receipt is restored from
    /// timing-index coverage; other timestamps are read from the journal.
    genesis_received_ms:Option<u128>,
    /// Peer discovery (VA-SPEC §1a, `node::discovery`): the self-connection nonce, the address book and each
    /// connection's discovery state.
    disc:discovery::Disc,
    /// VA-SPEC §2: this node's consensus identity, set once from `chain.spec` (it is what every Hello carries and is
    /// checked against).
    pub consensus:crate::identity::Consensus,
}
#[derive(Default)]
struct Ckpt{every:u64,last:u64,roll:[u8;32]}
/// C-1 E4.4: writes a checkpoint of `chain` after the journal prefix `b`. A failure is logged, never fatal:
/// a checkpoint only shortens a later open.
fn write_checkpoint(chain:&Chain,st:&crate::state_store::StateStore,b:&crate::checkpoint::Binding)->Result<u64,String>{
    let (bytes,roots)=crate::checkpoint::encode(chain,b)?;
    crate::crash_point("checkpoint-before");
    let q=st.write_checkpoint(&bytes,&roots,crate::checkpoint::KEPT).map_err(|e|e.0)?;
    crate::crash_point("checkpoint-after");Ok(q)
}
#[derive(Default,Clone,Debug)]
pub struct Stats{pub imported:u64,pub mined:u64,pub signed_templates:u64,pub fallback_templates:u64,pub shares_seen:u64,pub hashes:u64,pub reorgs:u64,
    /// Sync sessions started and rounds asked; solicited imports of blocks retention had removed
    /// (re-fetch); canonical blocks made whole after adoptions (restore); journal records replay skipped.
    pub sync_sessions:u64,pub sync_rounds:u64,pub refetched:u64,pub restored:u64,pub replay_skipped:u64,
    /// C-1 S1: blocks whose receipts and transaction index replay wrote to history.redb (missing ones).
    pub history_refilled:u64,
    /// C-1 S4: slimmed entries restored for a validation (a parent or an evidence ancestor).
    pub unslims:u64,
    /// Peers whose template from Draw(h,0) for this node's parent failed its checks (consensus.md:101).
    pub template_penalties:u64,
    /// Gossip (network.md:13,15): signed templates of other proposers this node relayed after locking on
    /// them, valid shares it relayed, and valid new shares it received from peers.
    pub relayed_templates:u64,pub relayed_shares:u64,pub peer_shares:u64,
    /// V3-02: kind-3 late-share items this node put in its templates.
    pub late_items:u64,
    /// Inbound eviction's protection (`Node::credit`, net::Useful order): a peer's first delivery of a new block, a
    /// template that locked, a transaction that entered the pool at once and a valid new share.
    pub credits:[u64;4],
    /// D-7: pool revalidation slices run and the longest one (ms); offers to peers of transactions
    /// returned from abandoned blocks, and those a peer's queue had no room for.
    pub pool_slices:u64,pub pool_slice_max_ms:u64,pub reinserted_offered:u64,pub reinserted_unoffered:u64,
    /// C-1 E4.4: the journal records the last open resumed after (None: replayed from genesis), the
    /// checkpoints it refused (each released), the pins it released (versions no checkpoint holds), and
    /// the checkpoints written since.
    pub resumed_from:Option<u64>,pub checkpoints_refused:u64,pub pins_released:u64,pub checkpoints_written:u64,
    /// C-1 E4.4: how the checkpoint open resumed from read its stored versions (`checkpoint::Read`).
    pub versions_read:crate::checkpoint::Read,
    /// D-6: release steps run, the longest (ms), the `release` rows they worked off, and the rows open
    /// worked off before reading a checkpoint.
    pub release_steps:u64,pub release_step_max_ms:u64,pub released_rows:u64,pub released_at_open:u64,
    /// Where the last open spent its time (`OpenTiming`).
    pub open:OpenTiming,
    /// PR6: template selections that counted declared gas (no checked parent state; never in production), gas
    /// failures recovered with the declared-safe prefix, transactions the append round added to a signed
    /// template, and `build_signed` calls for signed templates.
    pub template_gas_fallbacks:u64,pub template_gas_recoveries:u64,pub template_appended:u64,pub template_builds:u64,
    /// PR6 P1: submissions refused by the sender quota; P2: parked transactions evicted by one declaring less.
    pub sender_quota_refused:u64,pub parked_evicted:u64,
    /// N4 (V3-20): replacements accepted, refused (underpriced or past a limit), offered to the peers and accepted
    /// past the relay bound (not relayed); pooled transactions evicted by a higher tip and submissions refused for a
    /// tip too low to evict; parked ones evicted by a higher tip; new submissions refused by the overdraft rule.
    pub pool_replaced:u64,pub replace_refused:u64,pub replace_relayed:u64,pub replace_unrelayed:u64,pub pool_evicted:u64,pub evict_refused:u64,pub parked_fee_evicted:u64,pub overdraft_refused:u64,
    /// PR6 review: parked transactions relayed when parked.
    pub parked_relayed:u64,
    /// VA-SPEC §2.5: Hellos refused for another network, a wire version below MIN and another consensus identity
    /// (`identity::Mismatch` order), the last refusal's reason (at most INCOMPATIBLE_REASON_MAX characters) and the
    /// connections whose first frame was PoCol 0.4.0's 9-item Hello (`net::legacy_hello`).
    pub incompatible:[u64;3],pub last_incompatible:String,pub legacy_hellos:u64}
/// The characters of `lastIncompatibleReason` kept (a bounded status field: the peer's name is at most NODE_NAME_MAX).
pub const INCOMPATIBLE_REASON_MAX:usize=160;
/// 2026-10-28: the time (ms) each step of `Node::open` took, so an open's cost can be attributed: reading the
/// journal (every record's payload, its replay events and rolling hashes), opening history.redb and state.redb
/// (redb's full integrity check of every page included), the release at open, reading and checking the
/// checkpoints and resetting the pins, restoring a checkpoint (every stored version read and checked), and the
/// replay of the records after it, split into validation (each block re-executed and its state committed),
/// the history.redb writes, retention and restore after an adoption, and the checkpoints replay writes.
#[derive(Default,Clone,Debug,serde::Serialize)]
#[serde(rename_all="camelCase")]
pub struct OpenTiming{pub journal_ms:u64,pub history_open_ms:u64,pub store_open_ms:u64,pub release_ms:u64,pub checkpoints_ms:u64,pub reset_pins_ms:u64,pub restore_ms:u64,
    pub replay_ms:u64,pub replay_records:u64,pub replay_validate_ms:u64,pub replay_history_ms:u64,pub replay_retain_ms:u64,pub replay_checkpoint_ms:u64,pub total_ms:u64}
fn ms(t:Instant)->u64{t.elapsed().as_millis() as u64}
/// The process's task limit: pids.max of its cgroup v2 (the `0::<path>` line of /proc/self/cgroup under
/// /sys/fs/cgroup, which systemd sets from TasksMax). None when either file cannot be read (another OS, cgroup v1, the
/// root cgroup, which has no pids.max).
fn pids_max()->Option<String>{
    if !cfg!(target_os="linux"){return None}
    let cg=std::fs::read_to_string("/proc/self/cgroup").ok()?;
    let path=cg.lines().find_map(|l|l.strip_prefix("0::"))?.trim().trim_end_matches('/');
    std::fs::read_to_string(format!("/sys/fs/cgroup{path}/pids.max")).ok()
}

fn orphan_size(b:&Block)->usize{b.header.encoded.len()+b.body.txs.iter().chain(b.body.evidence.iter()).map(|x|x.len()).sum::<usize>()}
fn result_bytes(r:&Option<ParentResult>)->Vec<u8>{
    let mut p=Vec::new();
    if let Some(r)=r{crate::rlp::encode_bytes(r.winner.as_slice(),&mut p);let mut l=Vec::new();for (n,o) in &r.shares{let mut q=Vec::new();crate::rlp::encode_bytes(&n.to_be_bytes(),&mut q);crate::rlp::encode_bytes(o.as_slice(),&mut q);crate::rlp::encode_list_payload(&q,&mut l);}
        crate::rlp::encode_list_payload(&l,&mut p);}
    let mut o=Vec::new();crate::rlp::encode_list_payload(&p,&mut o);o
}

/// The largest R_keep this data directory has used (`retention.rkeep`), raised to `configured` and
/// written durably (temporary file, fsync, rename, directory fsync) before replay starts. Windows cannot
/// open a directory as a file (Access is denied), so there the directory fsync is skipped, as in
/// `store::sync_dir`.
fn retention_rkeep(dir:&std::path::Path,configured:u64)->std::io::Result<u64>{
    use std::io::Write;
    let path=dir.join("retention.rkeep");
    let stored=std::fs::read_to_string(&path).ok().and_then(|s|s.trim().parse::<u64>().ok());
    let rk=stored.map_or(configured,|x|x.max(configured));
    if stored!=Some(rk){
        let tmp=dir.join("retention.rkeep.tmp");
        {let mut f=std::fs::File::create(&tmp)?;f.write_all(rk.to_string().as_bytes())?;f.sync_all()?;}
        std::fs::rename(&tmp,&path)?;if cfg!(unix){std::fs::File::open(dir)?.sync_all()?;}
    }
    Ok(rk)
}

/// C-1 S5 (PR7 §7, review 2026-11): the record count the journal's commit file gives (its valid slot with the larger
/// seq, as `store::open` reads it), read before the journal pass so that the replay tables are checked before the pass
/// builds them. None when it cannot be read (no journal or commit file, a format-1 journal, no valid slot): the pass
/// then decides, and the exact check after it runs either way. The slot layout (store.rs's module docs) is read here,
/// not exposed by store.rs, which is a consensus-boundary file.
fn committed_records(dir:&std::path::Path,binding:&[u8;32])->Option<u64>{
    use std::io::Read;use sha2::{Digest,Sha256};
    let mut magic=[0u8;8];std::fs::File::open(dir.join("blocks.journal")).ok()?.read_exact(&mut magic).ok()?;if &magic!=b"POCOLJ2\n"{return None}
    let c=std::fs::read(dir.join("blocks.commit")).ok()?;let second=match c.len(){4160=>4096,128=>64,_=>return None};
    let slot=|s:&[u8]|{let u=|i:usize|u64::from_be_bytes(s[i..i+8].try_into().unwrap());let (seq,count,end)=(u(0),u(8),u(16));
        let mut h=Sha256::new();h.update(b"POCOL-COMMIT");h.update(binding);h.update(seq.to_be_bytes());h.update(count.to_be_bytes());h.update(end.to_be_bytes());
        (s[24..56]==h.finalize()[..]&&s[56..64].iter().all(|b|*b==0)).then_some((seq,count))};
    [slot(&c[..64]),slot(&c[second..second+64])].into_iter().flatten().max().map(|(_,n)|n)
}

/// F7: load the attempt-0 signing record from `<dir>/signed-a0` (one line per pair:
/// `hex(parent) height hex(tid)`). Malformed or missing lines are skipped — an empty map just means
/// nothing has been signed yet. Only template ids persist (not bodies), so the file stays tiny.
fn load_signed_a0(dir:&std::path::Path)->HashMap<([u8;32],u64),[u8;32]>{
    let dec=|s:&str|->Option<[u8;32]>{crate::types::unhex(s).ok().and_then(|v|<[u8;32]>::try_from(v).ok())};
    let mut m=HashMap::new();
    if let Ok(text)=std::fs::read_to_string(dir.join("signed-a0")){
        for line in text.lines(){
            let mut it=line.split_whitespace();
            if let (Some(p),Some(h),Some(t))=(it.next(),it.next(),it.next()){
                if let (Some(ph),Ok(hh),Some(th))=(dec(p),h.parse::<u64>(),dec(t)){m.insert((ph,hh),th);}
            }
        }
    }
    m
}

/// C-1 E5: a closing node leaves its entries' versions pinned (`StateStore::close`); the next open releases them.
/// C-1 S5: and the canonical index's last puts are made durable, if they can be (replay writes them again if not).
impl Drop for Node{fn drop(&mut self){if let Some(s)=&self.state_store{s.close();}let _=self.canon.sync();}}
impl Node{
    /// Opens the journal, replays and re-executes every committed block.
    pub fn open(mut cfg:Config,spec:Spec)->Result<Self,String>{
        if cfg.name.is_empty()||cfg.name.len()>net::NODE_NAME_MAX||!cfg.name.bytes().all(|b|b.is_ascii_graphic()){return Err("node name must be 1..64 printable ASCII bytes without spaces".into())}
        // A13: the resident bound of this profile with these limits must fit the budget (the pool counted at
        // its gas bound, which must itself be valid).
        let t_open=Instant::now();let mut timing=OpenTiming::default();
        // The quota applies within the gas bound in effect (the operator's value is validated against the configured bound).
        let pool_gas_max=pool_gas_in_effect(spec.cp(),&cfg.limits)?;let pool_sender_max=pool_sender_gas_bound(spec.cp(),&cfg.limits)?.min(pool_gas_max);
        // Replay must not use a smaller R_keep than live used for any record (it would drop a parent live
        // kept); a larger one only keeps blocks that were below the head when imported. The largest value
        // this data directory has used is kept durably beside the journal and replay uses it.
        std::fs::create_dir_all(&cfg.dir).map_err(|e|format!("data directory: {e}"))?;
        let rk_replay=retention_rkeep(&cfg.dir,cfg.limits.r_keep).map_err(|e|format!("retention.rkeep: {e}"))?;
        // C-1 S5: the eviction depth, from that R_keep, set once in the limits so that A13 and the chain use one value.
        let evict=crate::resources::evict_depth_of(spec.cp(),spec.local().k_hist,&cfg.limits,rk_replay)?;cfg.limits.evict_depth=Some(evict.unwrap_or(0));
        cfg.limits.first_imports=imports::enabled(&spec);
        // C3-2: in the same v3 domain one receipt can exceed LIGHT_RESP_MAX, so it is served heavy.
        cfg.limits.rpc.heavy_receipts=cfg.limits.first_imports;
        if spec.cp().H_END==0||cfg.limits.first_imports{cfg.limits.continuous_history=Some(0);}
        // D3', D14' (VA-SPEC §1a.7, network.md:157): `auto` becomes a count within the 18 outbound places, an explicit
        // count past them is refused, and every seed must be a configured peer (it is dialed as one, VA-SPEC §1a.8).
        cfg.limits.discovery.resolve(cfg.listen.is_some(),cfg.peers.len())?;
        if let Some(s)=cfg.limits.discovery.seeds.iter().find(|s|!cfg.peers.contains(s)){return Err(format!("seed {s}: every seed must also be a configured peer"))}
        // PR7 §6.2 (A1-6): the files are observed before the first check (a missing one counts 0).
        cfg.limits.disk=Some(crate::resources::DiskBytes::observe(&cfg.dir));
        crate::resources::check(spec.cp(),spec.local().k_hist,spec.local().side_max,&cfg.limits,cfg.peers.len())?;
        let binding=spec.genesis_hash();
        // PR7 §7 (G5, review 2026-11): the replay tables are checked before the pass builds them, at the committed count.
        // A journal they do not fit stops here with the LOCAL CAPACITY message, not out of memory during the pass (the
        // check after the pass came after its tables). The pass adds at most one uncommitted record, which the exact
        // check after it counts; at the committed count this refuses only a journal that check refuses (both grow with
        // the count, and the pass changes none of the observed files).
        if let Some(n)=committed_records(&cfg.dir,&binding){let mut l=cfg.limits.clone();if spec.cp().H_END==0||l.first_imports{l.continuous_history=Some(n);}
            crate::resources::check_open(spec.cp(),spec.local().k_hist,spec.local().side_max,&l,cfg.peers.len(),n)?;}
        // One pass over the journal (2026-10-28: its payloads are not held, condition `open-replay-transient`):
        // each verified record's replay events (its block and its parent) and the rolling hash of the journal prefix
        // (C-1 E4.4 binding), sampled every ROLL_SAMPLE records (C-1 S5, PR7 §7: the frame offsets are the journal's
        // own, `Recovered::offsets`; what open holds per record is counted, `resources::open_transient`).
        let mut evs:Vec<Ev>=Vec::new();
        let (mut samples,mut roll_end,mut bad):(Vec<[u8;32]>,[u8;32],Option<String>)=(vec![[0u8;32]],[0u8;32],None);
        let t=Instant::now();
        let rec=store::open_with(&cfg.dir,&binding,&mut |i,_,payload|{
            if bad.is_none(){match (||{
                let record=store::decode_record_ref(payload)?;let header=decode_header(record.header)?;
                if spec.v3_active(header.h)&&record.first_import.is_none(){return Err("old v3-preview journal has no first-import timing; its historical flags cannot be reconstructed".to_string())}
                Ok(header)})(){
                Ok(header)=>{
                    if header.protocol_version!=spec.active_version(header.h){bad=Some(format!("record {i}: stored block {} conflicts with the configured consensus fork schedule",header.h));}
                    else if i>=EV_RECORDS_MAX{bad=Some(format!("record {i}: a journal of more than {EV_RECORDS_MAX} records cannot be replayed"));}
                    else{let at=(i as u32)*2;evs.push(Ev{hash:block_hash(&header),at});evs.push(Ev{hash:header.parent_hash,at:at+1});}
                }
                Err(e)=>bad=Some(format!("record {i}: {e}"))}}
            roll_end=crate::checkpoint::roll(&roll_end,payload);if (i+1)%ROLL_SAMPLE==0{samples.push(roll_end);}
        }).map_err(|e|format!("store: {e:?}"))?;timing.journal_ms=ms(t);
        if let Some(e)=bad{return Err(e)}
        evs.sort_unstable();
        cfg.limits.disk=Some(crate::resources::DiskBytes::observe(&cfg.dir));
        if spec.cp().H_END==0||cfg.limits.first_imports{cfg.limits.continuous_history=Some(rec.journal.records);crate::resources::check(spec.cp(),spec.local().k_hist,spec.local().side_max,&cfg.limits,cfg.peers.len())?;}
        // PR7 §7: the replay's own tables must fit beside the bound (they grow with the journal, not the window).
        crate::resources::check_open(spec.cp(),spec.local().k_hist,spec.local().side_max,&cfg.limits,cfg.peers.len(),rec.journal.records)?;
        let mut chain=Chain::new(spec);chain.r_keep=rk_replay;chain.side_cap=cfg.limits.side_cap;chain.replaying=true;
        // C-1 S1: canonical history below the resident window lives off the heap (journal + history.redb).
        // C-1 E4.4: a history.redb created now holds nothing a checkpoint's entries need (replay refills it).
        let history_kept=cfg.dir.join(crate::history::FILE).exists();
        let t=Instant::now();let history=crate::history::History::open(&cfg.dir).map_err(|e|format!("history: {}",e.0))?;timing.history_open_ms=ms(t);
        // C-1 S5: an index created now holds nothing a snapshot based above height 1 needs (replay refills it).
        let canon_kept=cfg.dir.join(canon::FILE).exists();let mut canon=canon::Index::open(&cfg.dir,&chain.spec.genesis_hash())?;
        // C-1 S5 (PR7 §2.4): in store mode a rebuild anchor every R_keep heights below the K_hist cut.
        let rk=rk_replay.max(cfg.limits.r_keep);
        chain.evict_depth=evict;chain.evict_batch=(cfg.limits.side_cap as u64).saturating_add(2*rk).saturating_add(2);chain.anchor_step=(cfg.limits.state_store&&evict.is_some()).then_some(rk);chain.history_window=Some(cfg.limits.history_window.unwrap_or(chain.k_hist));chain.keep_full_states=cfg.limits.full_states;
        // C-1 S4: deeper than any fork (R_keep), a head that fell back by R_keep, the evidence window and
        // the 256-ancestor window: no validation reads a slimmed entry unless the head fell back further.
        chain.slim_depth=Some(cfg.limits.slim_depth.unwrap_or(2*rk_replay.max(cfg.limits.r_keep)+u64::from(chain.spec.cp().E_win)+257));
        let n_rec=rec.offsets.len();
        // The journal prefix of `k` records: where it ends, and its rolling hash (from the sample below it).
        let end_of=|k:usize|->u64{if k<n_rec{rec.offsets[k]}else{rec.journal.end_offset()}};
        let roll_at=|k:usize|->Result<[u8;32],String>{if k>=n_rec{return Ok(roll_end)}let j=k/ROLL_SAMPLE;let mut r=samples[j];
            for i in j*ROLL_SAMPLE..k{r=crate::checkpoint::roll(&r,&rec.payload(i).map_err(|e|format!("record {i}: {e:?}"))?);}Ok(r)};
        let mut genesis_received_ms=imports::open(&chain,&canon,&rec,end_of,roll_at,roll_end,now_ms())?;
        // C-1 E4: states in state.redb. C-1 E4.4 (D-6): open resumes from the newest checkpoint that verifies
        // (`checkpoint::restore`) and replays only the records after it; one that does not is released and
        // the one before it is tried. With none, or a store that is new, damaged or inconsistent, or a
        // history.redb that was rebuilt, the store is started empty and replay re-executes every block
        // from genesis and refills it (derived data, I1). Checkpoints are kept only under the R_keep replay
        // uses (a lowered configured R_keep would make live keep less than replay).
        let ckpt_every=if cfg.limits.state_store&&cfg.limits.r_keep==rk_replay{cfg.limits.checkpoint_every}else{0};
        let (mut start,mut refused,mut released,mut state_store,mut versions_read)=(0usize,0u64,0u64,None,crate::checkpoint::Read::default());let mut opened_release=0u64;
        if cfg.limits.state_store{
            let t=Instant::now();let mut st=Arc::new(crate::state_store::StateStore::open(&cfg.dir).map_err(|e|format!("state store: {}",e.0))?);timing.store_open_ms=ms(t);
            let mut dyn_st:Arc<dyn pocol_lp4_execution::node_trie::TrieStore>=st.clone();
            let mut resumed=None;
            // D-6: what a checkpoint's write left owed is worked off before anything is read (a store owes at
            // most one dropped checkpoint). A count that contradicts what is owed marks the store damaged
            // (`release_all`), so its checkpoints are not used and it is rebuilt. A storage error (an I/O error)
            // stops this open: redb refuses the store until it is opened again, and the store, its checkpoints and
            // what it owes are kept for the next open (2026-10-28; before, open went on reading the refused store
            // and panicked).
            let t=Instant::now();let mut released_at_open=if st.rebuilt||ckpt_every==0{0}else{match st.release_all(){Ok(n)=>n,Err(_) if st.is_damaged()=>0,
                Err(e)=>return Err(format!("state store: the release at open: {}",e.0))}};timing.release_ms=ms(t);
            // The pins are reset to the checkpoints' before genesis's version is pinned for its entry.
            let t=Instant::now();let mut reset_ms=0u64;
            // Each checkpoint is decoded and checked here, then dropped, and decoded again only when it is tried below, so
            // one decoded snapshot is alive at a time beside the KEPT checkpoints' bytes (`resources::migration_fits`
            // counts that for a format-1 one, which holds every entry of the chain). Before, all KEPT were decoded at once
            // and alive through the migration's restore, uncounted (review 2026-11, finding 6).
            let snaps:Result<Vec<(u64,Vec<u8>)>,String>=if ckpt_every>0&&!st.rebuilt&&!history.rebuilt&&history_kept{
                st.checkpoints().map_err(|e|e.0).and_then(|c|c.into_iter().map(|(q,b,roots)|{
                    let s=crate::checkpoint::decode(&b).map_err(|e|format!("checkpoint {q}: {e}"))?;
                    if roots!=crate::checkpoint::roots(&s){return Err(format!("checkpoint {q}: its roots record is not its snapshot's"))}Ok((q,b))}).collect())
                    .and_then(|s:Vec<(u64,Vec<u8>)>|if s.is_empty(){Ok(s)}else{let t=Instant::now();let r=st.reset_pins().map(|n|{released=n;s}).map_err(|e|e.0);reset_ms=ms(t);r})}else{Ok(vec![])};
            timing.checkpoints_ms=ms(t);timing.reset_pins_ms=reset_ms;
            // A store found damaged so far (the release at open met a row naming a node that is not stored,
            // say) is set aside now and started empty, as `StateStore::open` sets aside one marked damaged:
            // nothing more is read from it, and replay refills the new one (I1). Before, open went on to pin
            // genesis's version in the damaged store and panicked (Windows CI run 40589342452, 2026-10-27).
            let snaps=if st.is_damaged(){
                drop(dyn_st);drop(st);
                st=Arc::new(crate::state_store::StateStore::open(&cfg.dir).map_err(|e|format!("state store: {}",e.0))?);dyn_st=st.clone();
                (released_at_open,released)=(0,0);Err("the store was found damaged at open".to_string())
            }else{snaps};
            chain.enable_store(dyn_st.clone()).map_err(|e|format!("state store: {e:?}"))?;
            let t=Instant::now();
            {
                match snaps{
                    Ok(mut snaps)=>{
                        snaps.sort_by_key(|x|std::cmp::Reverse(x.0));
                        for (q,raw) in snaps{
                            // It decoded above, from the same bytes.
                            let s=crate::checkpoint::decode(&raw).map_err(|e|format!("checkpoint {q}: {e}"))?;drop(raw);
                            let b=s.binding;let k=b.records as usize;
                            // Replay's protect set after record k: the blocks whose next journal event is a child record.
                            let protect:std::collections::HashSet<[u8;32]>=s.entries.iter().filter(|e|next_event(&evs,&e.hash,k)==Some(true)).map(|e|e.hash).collect();
                            // The binding's records rolled again from the sample below it are read back from the journal: a
                            // read that fails is the node's own storage failing, as in the restore below (it stops the open,
                            // nothing is dropped). Before, it returned the bare read error (review 2026-11, io-fault CI).
                            let r=if k>n_rec||end_of(k)!=b.end||roll_at(k).map_err(|e|format!("state store: checkpoint {q} could not be read: journal {e}"))?!=b.hash{Err("it is not bound to this journal's records".to_string())}
                                // C-1 S5: a window based above height 1 needs the index rows below it; a format-1 snapshot needs
                                // none, but restores every entry of the chain at once (the migration), which must fit.
                                else if !s.v1&&s.canon_base>1&&(canon.rebuilt||!canon_kept){Err("the canonical index does not reach the snapshot's window (it was rebuilt or is new)".to_string())}
                                else if let Some(e)=s.v1.then(||crate::resources::migration_fits(chain.spec.cp(),chain.spec.local().k_hist,chain.spec.local().side_max,&cfg.limits,cfg.peers.len(),s.entries.len() as u64,n_rec as u64).err()).flatten(){Err(e)}
                                else if s.protect.as_ref().map_or(false,|p|p.len()!=protect.len()||p.iter().any(|h|!protect.contains(h))){Err("its protect set is not the one replay had after its last record".to_string())}
                                else{crate::checkpoint::restore(&mut chain,&s,&crate::checkpoint::JournalSource{offsets:&rec.offsets,journal:&rec.journal,records:k,history:&history,canon:&canon},&dyn_st)};
                            // 2026-10-28 (independent review F1, F2): a restore that failed because the journal or
                            // state.redb could not be read is the node's own storage failing, not the checkpoint: open
                            // stops, and nothing is dropped, set aside or rebuilt for it.
                            match r{Ok(r)=>{resumed=Some(k);versions_read=r;chain.protect=protect;break}
                                Err(e) if e.starts_with(crate::checkpoint::LOCAL)||st.io_failed()=>return Err(format!("state store: checkpoint {q} could not be read: {e}")),
                                Err(e)=>{refused+=1;if cfg.log{eprintln!("[{}] checkpoint {q} (record {k}) refused: {e}",cfg.name);}
                                    let dropped=st.drop_checkpoint(q);
                                    if st.io_failed(){return Err(format!("state store: dropping checkpoint {q}: {}",dropped.err().map_or_else(String::new,|e|e.0)))}
                                    if dropped.is_err()||st.is_damaged(){break}}}
                        }
                    }
                    Err(e) if st.io_failed()=>return Err(format!("state store: the checkpoints could not be read: {e}")),
                    Err(e)=>{if cfg.log{eprintln!("[{}] checkpoints unusable, the state store is rebuilt: {e}",cfg.name);}}
                }
            }
            timing.restore_ms=ms(t);
            // C-1 S5: an index a refused snapshot found damaged is set aside now, so replay can refill it (I1).
            if canon.is_damaged(){
                drop(canon);canon=canon::Index::open(&cfg.dir,&chain.spec.genesis_hash())?;
                // Replacing the derived file also discarded its first-import
                // table. Restore it from the journal before replay/live use.
                genesis_received_ms=imports::open(&chain,&canon,&rec,end_of,roll_at,roll_end,now_ms())?;
            }
            match resumed{
                Some(k)=>{start=k;state_store=Some(st);opened_release=released_at_open;}
                None=>{
                    // Started empty: every handle on the old file goes before it is removed (Windows cannot
                    // remove an open file).
                    if let Some(g)=chain.blocks.get_mut(&chain.head){g.stored=None;}chain.trie_store=None;drop(dyn_st);drop(st);
                    for f in [crate::state_store::FILE,crate::state_store::DAMAGED]{let _=std::fs::remove_file(cfg.dir.join(f));}
                    let st=Arc::new(crate::state_store::StateStore::open(&cfg.dir).map_err(|e|format!("state store: {}",e.0))?);
                    chain.enable_store(st.clone()).map_err(|e|format!("state store: {e:?}"))?;state_store=Some(st);
                }
            }
        }
        // D-REORG-REDB replay. Every record is replayed in journal order with retain() after each, as
        // live import did. Live journals a block only after validating it on a present parent, and
        // only when it is absent, so: (1) a block whose NEXT journal event is a child record is
        // protected from the side cap until then (live had it present throughout, so the protected set
        // never exceeds live's resident side set, and replay bounds all side blocks by side_cap +
        // protect_max), which guarantees every record's parent is present;
        // (2) a record of a block already present is a re-import after live removed it, so it takes a
        // new importSeq (consensus.md:205). With the same R_keep boundary, every replayed head equals
        // the live head after the same record (D-REORG-REDB-DESIGN-FREEZE.md Revision 2).
        // C-1 E4.4: resumed after record `start` (the protect set the events up to it leave, the blocks whose next event
        // is a child record, was set when the checkpoint was restored). `run`: the rolling hash of the records replayed.
        let mut rec_off=end_of(start); // first record frame starts after MAGIC(8)+binding(32)
        let mut run=roll_at(start)?;
        let (mut skipped,mut restored,mut refilled)=(0u64,0usize,0u64);
        let (mut ckpt_last,mut written)=(start as u64,0u64);
        let t_replay=Instant::now();timing.replay_records=(n_rec-start.min(n_rec)) as u64;
        // A version an entry stops holding is released inside the next block's commit (StateStore::set_deferred).
        if let Some(st)=&state_store{st.set_deferred(true).map_err(|e|format!("state store: {}",e.0))?;}
        for i in start..n_rec{
            let payload=rec.payload(i).map_err(|e|format!("record {i}: {e:?}"))?;let payload=&payload;
            // Read back from the file: bound again to the rolling hash the first pass computed (review F7), at each
            // sample and at the end (PR7 §7), so a record rewritten with its digest between the two passes stops the open
            // within ROLL_SAMPLE records. What was replayed before was validated in full and wrote derived data only
            // (state, history and the canonical index, which the next open rebuilds or checks against the journal).
            run=crate::checkpoint::roll(&run,payload);
            if ((i+1)%ROLL_SAMPLE==0&&run!=samples[(i+1)/ROLL_SAMPLE])||(i+1==n_rec&&run!=roll_end){return Err(format!("record {i}: the journal changed while it was opened"))}
            let this_off=rec_off; rec_off+=(payload.len()+store::OVERHEAD) as u64;
            let record=store::decode_record_ref(payload).map_err(|e|format!("record {i}: {e}"))?;
            let first_import=record.first_import;
            let header=decode_header(record.header).map_err(|e|format!("record {i}: {e}"))?;
            let body=Body::decode(record.body,chain.spec.cp().BODY_MAX as usize).map_err(|e|format!("record {i}: {e}"))?;
            let block=Block{header,body};let hash=block.hash();let parent=block.header.parent_hash;let old_head=chain.head;
            // C-1 S5 (§3.3): a record of a journal written before live refused blocks at or below head - R_keep (import_with)
            // can name a canonical block below the window (i), or build on a canonical block (ii). Replay without eviction
            // re-imports the first (a new importSeq, its record's copy) and inserts the second, which the same retain
            // removes. Both are reproduced here without the entry: the row takes the new importSeq and offset, and the
            // removal is counted as that retain's. (ii) is taken before validation whether its parent is below the window
            // or in it (review 2026-11): a parent at most 255 heights above the window's base has ancestors the window no
            // longer holds, so validating its child read a short BLOCKHASH window (`child_ctx` now refuses that) and failed
            // the open; and a rebuild to validate a block the retain drops at once walked from below the window.
            let old_record=|chain:&mut Chain|->Result<bool,String>{
                let base=chain.canonical.base();if base<2{return Ok(false)}
                let deep=|x:&[u8;32]|->Result<Option<u64>,String>{Ok(canon.height_of(x).map_err(|e|format!("record {i}: canonical index: {e}"))?.filter(|h|*h<base))};
                if let Some(h)=deep(&hash)?{
                    let mut r=canon.at(h).map_err(|e|format!("record {i}: canonical index: {e}"))?.ok_or_else(||format!("record {i}: canonical index: no row at height {h}"))?;
                    if chain.spec.v3_active(h)&&(r.flags&crate::checkpoint::flags::LATE==0)!=first_import.map_or(true,|m|m.timely){return Err(format!("record {i}: canonical first import changed"))}
                    r.import_seq=chain.note_reimported();r.body_off=Some(this_off);canon.put(&[(h,r)]).map_err(|e|format!("record {i}: canonical index: {e}"))?;return Ok(true)
                }
                let held=block.header.h>0&&chain.canonical.get(block.header.h as usize-1)==Some(&parent);
                if held||deep(&parent)?.is_some(){chain.note_removed(hash);return Ok(true)}Ok(false)};
            // Only a record the retain after it would remove (live refuses one now), of a block replay does not hold.
            let deep_record=!chain.blocks.contains_key(&hash)&&block.header.h.saturating_add(chain.r_keep)<=chain.head().height;
            // Same rules as live import: a journaled fork child of a block below the K_hist window
            // needs the TrieRebuild that import performed before journaling it.
            let t=Instant::now();let taken=deep_record&&old_record(&mut chain)?;let validated=(!taken).then(||chain.validate(&block,None,0));timing.replay_validate_ms+=ms(t);
            match validated{
                None=>{}
                Some(Err(Reject::Duplicate))=>{
                    if chain.spec.v3_active(block.header.h)&&chain.get(&hash).unwrap().timely!=first_import.map_or(true,|m|m.timely){return Err(format!("record {i}: first import changed"))}
                    chain.reimport(&hash,block.header.clone(),Some(this_off));
                }
                // Unreachable by (1) for a block whose parent replay holds; counted (Stats::replay_skipped) rather than
                // refusing to open. An old record below the window was taken above; this is a last resort.
                Some(Err(Reject::UnknownParent))=>{if !old_record(&mut chain)?{skipped+=1;}}
                Some(r)=>{
                    let (mut entry,_)=match r{
                        Err(Reject::NeedsRebuild(p))=>{let t=Instant::now();let src=Src{journal:&rec.journal,history:&history,canon:&canon};let mut q=p;let mut tries=0;
                            loop{chain.ensure_state_with(&q,Some(&src)).map_err(|e|format!("replay {i}: rebuild {e:?}"))?;tries+=1;
                                match chain.validate(&block,None,0){Err(Reject::NeedsRebuild(n)) if tries<REBUILD_TRIES=>q=n,r=>{timing.replay_validate_ms+=ms(t);break r.map_err(|e|format!("replay {i}: {e:?}"))?}}}}
                        r=>r.map_err(|e|format!("replay {i}: {e:?}"))?,
                    };
                    if result_bytes(&entry.result).as_slice()!=record.result{return Err(format!("parentResultMismatch at record {i}"))}
                    entry.body_off=Some(this_off);entry.timely=first_import.map_or(true,|m|m.timely);
                    // Replay refills a store that was lost or damaged (I1): a stored block is left as it is.
                    // Writes are made durable every HISTORY_SYNC blocks (redb frees pages only then) and at the end.
                    let t=Instant::now();
                    if history.put_with(&entry.hash,&entry.receipts,&entry.tx_hashes,&entry.ranges,false).map_err(|e|format!("replay {i}: history {}",e.0))?{refilled+=1;
                        if refilled%HISTORY_SYNC==0{history.sync().map_err(|e|format!("replay {i}: history {}",e.0))?;}}
                    timing.replay_history_ms+=ms(t);
                    chain.insert(entry);
                }
            }
            for b in [hash,parent]{if next_event(&evs,&b,i+1)==Some(true){chain.protect.insert(b);}else{chain.protect.remove(&b);}}
            // As live import: the fork height is taken before retain(), and an adopted branch whose
            // blocks were offloaded as side blocks is made whole right after it (review R3-3: restoring
            // once at the end re-executed every canonical block from the lowest offloaded one up).
            let fork_h=if chain.head!=old_head{chain.lca_height(&old_head,&chain.head).unwrap_or(0)}else{0};
            let adopted=chain.head!=old_head;let t=Instant::now();
            chain.retain_unsettled();
            // C-1 S5: the window, as live import moves it after the same record.
            chain.window(&Src{journal:&rec.journal,history:&history,canon:&canon}).map_err(|e|format!("replay {i}: the window: {e:?}"))?;flush_evicted(&mut chain,&canon).map_err(|e|format!("replay {i}: {e}"))?;
            if adopted{restored+=chain.restore_canonical(fork_h,&Src{journal:&rec.journal,history:&history,canon:&canon}).map_err(|e|format!("replay {i}: restore canonical {e:?}"))?;}
            chain.settle();timing.replay_retain_ms+=ms(t);
            // C-1 E4.4: replay checkpoints too, after making its history.redb writes durable.
            if let Some(st)=&state_store{if ckpt_every>0&&(i as u64+1)-ckpt_last>=ckpt_every{let t=Instant::now();
                history.sync().map_err(|e|format!("replay {i}: history {}",e.0))?;
                // C-1 S5 (I5): the index is durable below the window the checkpoint describes.
                canon.sync().map_err(|e|format!("replay {i}: canonical index: {e}"))?;
                match write_checkpoint(&chain,st,&crate::checkpoint::Binding{records:i as u64+1,end:end_of(i+1),hash:run}){
                    Ok(_)=>{written+=1;ckpt_last=i as u64+1;}
                    Err(e) if st.io_failed()=>return Err(format!("replay {i}: state store: the checkpoint's write: {e}")),
                    Err(e)=>if cfg.log{eprintln!("[{}] checkpoint at record {}: {e}",cfg.name,i+1)}}timing.replay_checkpoint_ms+=ms(t);}}
        }
        timing.replay_ms=ms(t_replay);
        if refilled>0{history.sync().map_err(|e|format!("replay: history {}",e.0))?;}
        drop(evs);drop(samples);chain.protect.clear();chain.replaying=false;chain.r_keep=cfg.limits.r_keep;chain.retain_unsettled();
        // C-1 S5: at open the window reaches its floor whatever was restored (a format-1 snapshot holds every entry):
        // batch after batch, each made durable, then the tables give back what the evicted entries held.
        {let src=Src{journal:&rec.journal,history:&history,canon:&canon};
            loop{chain.window(&src).map_err(|e|format!("open: the window: {e:?}"))?;let more=!chain.evicted_out.is_empty();flush_evicted(&mut chain,&canon).map_err(|e|format!("open: {e}"))?;
                if !more||chain.window_done(){break}canon.sync().map_err(|e|format!("open: canonical index: {e}"))?;}}
        canon.sync().map_err(|e|format!("open: canonical index: {e}"))?;chain.shrink();
        // Every adoption was restored above, so this finds nothing unless replay skipped a record.
        restored+=chain.restore_canonical(0,&Src{journal:&rec.journal,history:&history,canon:&canon}).map_err(|e|format!("replay: restore canonical {e:?}"))?;chain.settle();
        if let Some(st)=&state_store{st.set_deferred(false).map_err(|e|format!("state store: the late releases: {}",e.0))?;}
        let (tx,rx)=mpsc::sync_channel(EVENT_QUEUE);let (miner_tx,miner_rx)=mpsc::channel::<MinerEvent>();
        let (light,light_rx)=crate::rpc::LightLane::new(cfg.limits.rpc.light_queue);let resp=crate::rpc::RespPools::new(&cfg.limits.rpc);
        let fwd=tx.clone();crate::threads::spawn("miner-fwd",crate::threads::IO_STACK,move||{for e in miner_rx{if fwd.send(Event::Miner(e)).is_err(){break}}}).map_err(|e|format!("spawn: {e}"))?;
        let head_state=chain::StateHandle::of(chain.head());let (meter,pool_code)=pool_start(chain.head(),&chain.spec);
        let net=net::Net::new(cfg.limits.net.clone());
        let signed_a0=load_signed_a0(&cfg.dir); // F7: resume the attempt-0 signing record across restarts
        let disc=discovery::Disc::open(&cfg.limits.discovery,cfg.listen)?;
        let consensus=crate::identity::Consensus::of(&chain.spec);
        let mut node=Self{cfg,chain,net,disc,consensus,orphan_order:VecDeque::new(),orphan_bytes:0,journal:rec.journal,history,canon,peers:HashMap::new(),peer_status:HashMap::new(),session:None,sync_ask_ms:0,sync_turn:0,outbound:Arc::new(Mutex::new(HashSet::new())),tx,rx,light,light_rx,draining:false,deferred:VecDeque::new(),between:false,resp,tick_pending:Arc::new(std::sync::atomic::AtomicBool::new(false)),miner_tx,prod:None,
            orphans:HashMap::new(),future:Vec::new(),own_future:HashSet::new(),templates:HashMap::new(),template_seen:HashMap::new(),latency:Default::default(),relayed:HashSet::new(),received:HashMap::new(),
            pool:Pool{code_total:pool_code,txs:vec![],pending:Default::default(),pending_base:head_state,bytes:0,meter,backlog:VecDeque::new(),backlog_bytes:0,gas:0,gas_max:pool_gas_max,ghost_gas:0,meta:vec![],sender_gas_max:pool_sender_max,replace_relays:0,parked:vec![],parked_bytes:0,park_seq:0,parked_max:POOL_PARKED_MAX,parked_relays:0},rejects:VecDeque::new(),stats:Stats::default(),stopped:false,capacity_warned:false,seen_templates:HashMap::new(),seen_wins:HashMap::new(),evidence_pool:Vec::new(),late_cache:LateCache::default(),signed_a0,signed_a0_mem:HashMap::new(),
            state_store,ckpt:Ckpt{every:ckpt_every,last:ckpt_last,roll:roll_end},release_turn:false,rebuild_tokens:REBUILD_BURST,rebuild_refill_ms:0,rebuild_clock:None,import_clock:None,genesis_received_ms};
        node.stats.imported=n_rec as u64;node.stats.replay_skipped=skipped;node.stats.restored=restored as u64;node.stats.history_refilled=refilled;
        node.stats.resumed_from=(start>0).then_some(start as u64);node.stats.checkpoints_refused=refused;node.stats.pins_released=released;node.stats.checkpoints_written=written;node.stats.versions_read=versions_read;node.stats.released_at_open=opened_release;node.stats.open=OpenTiming{total_ms:ms(t_open),..timing};
        if rec.torn_tail{node.log("recovered torn journal tail into a new journal");}
        if rec.migrated_v1{node.log("migrated a fully verified v1 journal to format v2 (original kept as blocks.journal.v1-n)");}
        Ok(node)
    }
    /// Local first-import metadata, verified against the original journal
    /// frame. This is not a lifetime RAM map, and is absent on legacy profiles.
    pub fn first_import(&self,hash:&[u8;32])->Result<Option<store::FirstImport>,String>{
        if !imports::enabled(&self.chain.spec){return Ok(None)}
        if *hash==self.chain.spec.genesis_hash(){return Ok(self.genesis_received_ms.map(|received_ms|store::FirstImport{received_ms,parent_received_ms:received_ms,timely:true}))}
        imports::first(&self.canon,&self.journal,hash)
    }
    /// C-1 E4.4: a checkpoint every `checkpoint_every` journal records (store mode). The block's state,
    /// history and journal record are durable already (I3), so the checkpoint describes durable data only.
    fn checkpoint_if_due(&mut self){
        let Some(st)=self.state_store.clone() else{return};
        if self.ckpt.every==0||self.journal.records-self.ckpt.last<self.ckpt.every{return}
        // C-1 S5 (I5): the index is durable below the window the checkpoint describes.
        if let Err(e)=self.canon.sync(){self.fatal(&format!("canonical index: {e}"))}crate::crash_point("canon-sync");
        let b=crate::checkpoint::Binding{records:self.journal.records,end:self.journal.end_offset(),hash:self.ckpt.roll};
        match write_checkpoint(&self.chain,&st,&b){Ok(_)=>{self.stats.checkpoints_written+=1;self.ckpt.last=b.records;}
            // 2026-10-28 (review F4): redb refuses the store after an I/O failure; a read before the next commit would
            // panic, so the node stops here, as a failed block commit stops it.
            Err(e) if st.io_failed()=>self.fatal(&format!("checkpoint at record {}: {e}",b.records)),
            Err(e)=>self.log(&format!("checkpoint at record {}: {e}",b.records))}
    }
    /// C-1 E4.4: writes a checkpoint now (store mode; tests and a clean stop).
    pub fn checkpoint_now(&mut self)->Result<u64,String>{
        let st=self.state_store.clone().ok_or("no state store")?;
        self.canon.sync().map_err(|e|format!("canonical index: {e}"))?;crate::crash_point("canon-sync");
        let b=crate::checkpoint::Binding{records:self.journal.records,end:self.journal.end_offset(),hash:self.ckpt.roll};
        let q=write_checkpoint(&self.chain,&st,&b)?;self.stats.checkpoints_written+=1;self.ckpt.last=b.records;Ok(q)
    }
    /// The node store (store mode).
    pub fn state_store(&self)->Option<&Arc<crate::state_store::StateStore>>{self.state_store.as_ref()}
    pub fn log(&self,m:&str){if self.cfg.log{eprintln!("[{} {}] {m}",self.cfg.name,now_ms()%100_000_000);}}
    /// The node's own storage failed (`Reject::Local`): it stops, as when a history.redb or journal write
    /// fails (exit 70), and the next open recovers. Logged whether or not `log` is on.
    fn fatal(&self,why:&str)->!{eprintln!("[{}] FATAL local storage: {why}",self.cfg.name);std::process::exit(70)}

    pub fn start_network(&mut self)->Result<(),String>{
        // R2 (PR3): the service must hold the whole A13 thread inventory, or a thread spawn would fail while the node
        // runs (a miner's is not fatal, a peer's closes it); a task limit that cannot be read is not checked.
        if let Some(m)=pids_max(){
            let need=crate::resources::tasks_needed(&self.cfg.limits.net,self.cfg.peers.len(),self.cfg.limits.discovery.slots(),self.cfg.limits.rpc.conn_max).unwrap_or(u128::MAX);
            crate::resources::tasks_fit(need,&m)?;
        }
        // The configured peers (for example the other seeds, which also dial this node) are never refused
        // for sharing a public address group with each other (net::Limits::per_group_max), and as many inbound
        // connections from an address as it has configured peers are never evicted: every configured address is
        // passed, duplicates included (net::Net::protect counts them).
        self.net.protect(self.cfg.peers.iter().map(|a|a.ip()));
        // VA-SPEC §1a.6: the address book is read before any connection is accepted or dialed.
        self.disc_load(self.disc_now());
        if let Some(a)=self.cfg.listen{let l=std::net::TcpListener::bind(a).map_err(|e|format!("listen {a}: {e}"))?;
            // Hello.listenPort (VA-SPEC §1a.1): the address actually bound (an ephemeral port for port 0).
            self.disc.bound(l.local_addr().map_err(|e|format!("listen {a}: {e}"))?);
            // Inbound eviction counts only a Hello this node would keep (on_msg's Hello checks), so the nonce is final
            // here and the listener does not run yet.
            self.net.expect_hello(net::HelloId{consensus:self.consensus.clone(),nonce:self.disc.wire_nonce()});
            let tx=self.tx.clone();let (ntx,nrx)=mpsc::sync_channel(net::NET_EVENT_QUEUE);crate::threads::spawn("listen-fwd",crate::threads::IO_STACK,move||{for e in nrx{if tx.send(Event::Net(e)).is_err(){break}}}).map_err(|e|format!("spawn: {e}"))?;
            net::listen(l,ntx,self.net.clone());}
        // VA-SPEC §1a.7: the automatic outbound slots, before the configured dialers.
        self.disc_start_outbound(self.disc_now())?;
        for a in self.cfg.peers.clone(){let tx=self.tx.clone();let (ntx,nrx)=mpsc::sync_channel(net::NET_EVENT_QUEUE);crate::threads::spawn(format!("dial-fwd-{a}"),crate::threads::IO_STACK,move||{for e in nrx{if tx.send(Event::Net(e)).is_err(){break}}}).map_err(|e|format!("spawn: {e}"))?;
            net::dial(a,ntx,self.outbound.clone(),self.net.clone());}
        if let Some(a)=self.cfg.rpc{crate::rpc::serve(a,self.tx.clone(),self.light.clone(),self.cfg.limits.rpc.clone())?;}
        let (t,pending)=(self.tx.clone(),self.tick_pending.clone());
        crate::threads::spawn("tick",crate::threads::IO_STACK,move||loop{std::thread::sleep(Duration::from_millis(50));
            if !pending.swap(true,std::sync::atomic::Ordering::SeqCst)&&t.send(Event::Tick).is_err(){break}}).map_err(|e|format!("spawn: {e}"))?;
        Ok(())
    }
    /// Runs until stop_at or channel closure.
    pub fn run(&mut self){self.run_controlled(|_| false)}
    /// The host can stop at event boundaries and publish local status. The process must exit
    /// after return: existing transport workers are process-scoped. No consensus rules change.
    pub fn run_controlled(&mut self, mut stop:impl FnMut(&mut Self)->bool){
        self.restart_production();
        while !self.stopped && !stop(self){
            // D-7: while the pool has a backlog, one event (if any is waiting) and one slice alternate. D-6: so do
            // one event and one release step while a dropped checkpoint's nodes are owed (the pool first).
            let ev=match self.deferred.pop_front(){Some(e)=>e,None=>if self.background_pending(){match self.rx.try_recv(){Ok(e)=>e,Err(mpsc::TryRecvError::Empty)=>{self.drain_light();self.background_step();continue}Err(_)=>break}}
                else{match self.rx.recv_timeout(Duration::from_millis(200)){Ok(e)=>e,Err(mpsc::RecvTimeoutError::Timeout)=>Event::Tick,Err(_)=>break}}};
            if matches!(ev,Event::Wake){self.light.wake.store(false,std::sync::atomic::Ordering::SeqCst);}
            self.drain_light();self.handle(ev);
            if self.background_pending(){self.background_step();}
        }
        // VA-SPEC §1a.6: a clean stop saves the address book. `pocold run` and the launcher stop cleanly on SIGINT and
        // SIGTERM (so `systemctl stop` is a clean stop); a killed node keeps its last save.
        self.disc_save();
        if let Some(p)=&self.prod{if let Some(m)=&p.miner{m.stop();}}
    }
    /// Local UI control, never exposed through public RPC. Validation and P2P continue.
    pub fn set_mining_key(&mut self,key:Option<Key>){
        if self.cfg.miner.as_ref().map(Key::address)==key.as_ref().map(Key::address){return}
        // Join the old worker before dropping its key; queued results no longer match production.
        if let Some(p)=self.prod.take(){if let Some(m)=p.miner{m.stop();drop(m);}}
        self.cfg.miner=key;
        self.restart_production();
    }
    /// Runs up to LIGHT_TURN waiting light requests, never nested inside a light request. A
    /// full turn may leave requests behind: the node wakes itself for them.
    pub fn drain_light(&mut self){
        if self.draining{return}
        self.draining=true;let mut ran=0;
        while ran<LIGHT_TURN{match self.light_rx.try_recv(){Ok(ev)=>{self.handle(ev);ran+=1}Err(_)=>break}}
        self.draining=false;
        if ran==LIGHT_TURN{self.light.wake(&self.tx);}
    }
    /// Between two members of an RPC batch (PR #27 review R2-1; network.md (I) keeps RPC from holding
    /// up validation, mining and sync): the light lane as before, then up to EVENT_TURN queued events
    /// that are not RPC requests (P2P messages and connections, miner results, ticks), so one batch no
    /// longer holds them for its whole length. A queued RPC request is set aside and runs next, in
    /// order, never inside the batch. Never nested.
    pub fn drain_between_members(&mut self){
        self.drain_light();
        if self.between{return}
        self.between=true;
        for _ in 0..EVENT_TURN{match self.rx.try_recv(){
            Ok(Event::Rpc(q))=>self.deferred.push_back(Event::Rpc(q)),
            Ok(Event::Wake)=>self.light.wake.store(false,std::sync::atomic::Ordering::SeqCst),
            Ok(ev)=>self.handle(ev),
            Err(_)=>break}}
        self.between=false;
    }
    /// The RPC response pools (rpc::answer reserves from them).
    pub fn resp_pools(&self)->crate::rpc::RespPools{self.resp.clone()}
    /// The light lane (rpc::serve pushes into it).
    pub fn light_lane(&self)->crate::rpc::LightLane{self.light.clone()}
    pub fn handle(&mut self,ev:Event){
        match ev{
            Event::Tick=>{self.tick_pending.store(false,std::sync::atomic::Ordering::SeqCst);self.tick()}
            Event::Wake=>{}
            Event::Miner(m)=>self.on_miner(m),
            Event::Rpc(q)=>{
                // A05/RPC work deadline: a request whose caller already got 504 is not run. Its
                // body and queue place are freed when `q` drops here.
                if std::time::Instant::now()>=q.deadline{crate::rpc::COUNTERS.expired.fetch_add(1,std::sync::atomic::Ordering::SeqCst);}
                else{let r=crate::rpc::answer(self,&q);let _=q.reply.send(r);}
            }
            // A feeler (VA-SPEC §1a.7) sends nothing and is never a peer; a connection its slot no longer waits for is
            // closed at once.
            Event::Net(NetEvent::Connected(id,peer))=>match self.disc_connected(id,&peer,self.disc_now()){
                discovery::Arrival::Peer=>{let hello=self.hello_to(&peer,self.hello());peer.send(&hello);self.peers.insert(id,peer);}
                discovery::Arrival::Feeler=>{}
                discovery::Arrival::Close=>peer.close()},
            Event::Net(NetEvent::Closed(id))=>{self.disc_closed(id,self.disc_now());self.peer_status.remove(&id);if self.session.as_ref().map_or(false,|s|s.peer==id){self.end_session(false);}
                if let Some(p)=self.peers.remove(&id){self.allow_redial(p.origin);}}
            Event::Net(NetEvent::Message(id,m))=>self.on_msg(id,m),
            Event::Net(NetEvent::DialFailed(t,a,e))=>self.disc_dial_failed(t,a,e,self.disc_now()),
            // Its Closed follows; the 0.4.0 handshake is refused by arity, as before (plan §6).
            Event::Net(NetEvent::Legacy(id))=>{self.stats.legacy_hellos+=1;
                let at=self.peers.get(&id).map(|p|p.addr.clone()).unwrap_or_default();self.log(&format!("peer {at} speaks the PoCol 0.4.0 handshake; closing"));}
        }
    }
    /// H2: a configured peer's dialer (`net::dial`) dials again once the address it dialed has left the outbound
    /// set, so an ended connection clears the set by that address (`Origin::Configured`), never by the address
    /// its socket reported: that is empty when `peer_addr` failed (a reset right after connect), and clearing by
    /// it would stall the dialer for good.
    /// A suppressed seed's address stays in the set, so its dialer waits (VA-SPEC §1a.8).
    fn allow_redial(&self,origin:net::Origin){if let net::Origin::Configured(a)=origin{if self.disc.suppressed(a){return}if let Ok(mut s)=self.outbound.lock(){s.remove(&a.to_string());}}}
    /// Φ = 2φ from the admitted local parameters (future-timestamp slack).
    fn capital_phi(&self)->u64{u64::from(self.chain.spec.local().capital_phi())}
    /// `hello` as sent to `p`: without listenPort and nonce when its connection cannot reach the listener
    /// (`Disc::reaches`, VA-SPEC §1a.1).
    fn hello_to(&self,p:&Peer,mut hello:Msg)->Msg{
        if !self.disc.reaches(p.local(),p.remote){if let Msg::Hello{listen_port,nonce,..}=&mut hello{(*listen_port,*nonce)=(0,0);}}hello
    }
    fn hello(&self)->Msg{let h=self.chain.head();Msg::Hello{chain_id:self.chain.spec.chain_id(),genesis:self.chain.spec.genesis_hash(),head:h.hash,height:h.height,work:h.total_work.to_be_bytes::<32>(),node:self.cfg.name.clone(),protocol:crate::identity::PROTOCOL_VERSION,compatibility:self.consensus.id,listen_port:self.disc.listen_port(),nonce:self.disc.wire_nonce()}}
    fn broadcast(&self,m:&Msg,except:Option<PeerId>){for (id,p) in &self.peers{if Some(*id)!=except&&self.peer_status.contains_key(id){p.send(m);}}}

    // ---------------- import ----------------
    /// Validates, journals (fsync) and inserts a block. Returns Ok(true) if new head.
    pub fn import(&mut self,b:Block,from:Option<PeerId>)->Result<bool,Reject>{self.import_with(b,from,false)}
    /// PR4 (I-5): takes an unsolicited deep-rebuild token, giving back one per REBUILD_REFILL_MS elapsed (at most
    /// REBUILD_BURST held); false when none is left. The clock is the wall clock: after it steps back (an NTP
    /// correction, a VM restore) the refill time is moved back to now, so a step back costs at most one refill
    /// interval, not its length (discovery's deadlines do the same, VA-SPEC §1a.6); a step forward gives back at
    /// most REBUILD_BURST.
    fn rebuild_token(&mut self)->bool{
        let now=self.rebuild_clock.unwrap_or_else(now_ms);
        if self.rebuild_refill_ms==0||now<self.rebuild_refill_ms{self.rebuild_refill_ms=now;}
        let gained=now.saturating_sub(self.rebuild_refill_ms)/REBUILD_REFILL_MS;
        if gained>0{self.rebuild_tokens=u32::try_from(gained).unwrap_or(u32::MAX).saturating_add(self.rebuild_tokens).min(REBUILD_BURST);self.rebuild_refill_ms=now;}
        if self.rebuild_tokens>0{self.rebuild_tokens-=1;true}else{false}
    }
    /// As `import`; `solicited` marks a block a sync session asked for (D-REORG-REDB re-fetch). An
    /// unsolicited block retention removed is not validated, journaled or requested again
    /// (chain::EVICTED_MAX, a churn guard); a solicited one is taken again (consensus.md §10: a branch
    /// that becomes heavier is fetched by sync) and, while non-canonical, protected from the side cap.
    /// `h`'s state made available as an import's rebuild makes it (the journal and history.redb supplying the
    /// blocks' bytes). For tests and measurements that build blocks on an arbitrary parent.
    pub fn ensure_state(&mut self,h:&[u8;32])->Result<(),Reject>{let src=Src{journal:&self.journal,history:&self.history,canon:&self.canon};self.chain.ensure_state_with(h,Some(&src))}
    /// A13 recheck of the continuous chain every 256 journal records (import_with). With eviction on (PR7 §6) the
    /// files are observed again and the bound counts them; with eviction off it projects from the record count (the
    /// behaviour before PR7, which stops the node once the journal outgrows the budget). At or above 15/16 of the
    /// budget a LOCAL CAPACITY warning is logged; past it the node stops (`stopped`), naming the override.
    fn capacity_recheck(&mut self)->Result<(),Reject>{
        self.cfg.limits.continuous_history=Some(self.journal.records);
        self.cfg.limits.disk=Some(crate::resources::DiskBytes::observe(&self.cfg.dir));
        let (cp,local)=(self.chain.spec.cp(),self.chain.spec.local());
        match crate::resources::check(cp,local.k_hist,local.side_max,&self.cfg.limits,self.cfg.peers.len()){
            Err(e)=>{let e=format!("{e}; raise --memory-budget-bytes or POCOL_MEMORY_BUDGET_BYTES");self.log(&format!("LOCAL CAPACITY: {e}"));self.stopped=true;Err(Reject::Local(e))}
            Ok(b)=>{
                let need=crate::resources::budget_required(b.total).unwrap_or(u128::MAX);let high=need.saturating_mul(16)>=u128::from(self.cfg.limits.mem_budget).saturating_mul(15);
                if high&&!self.capacity_warned{self.log(&format!("LOCAL CAPACITY warning: the A13 bound {} B needs {need} B with its 1/8 reserve, at least 15/16 of the node memory budget {} B; raise --memory-budget-bytes or POCOL_MEMORY_BUDGET_BYTES before it is reached",b.total,self.cfg.limits.mem_budget));}
                self.capacity_warned=high;Ok(())}
        }
    }
    pub fn import_with(&mut self,b:Block,from:Option<PeerId>,solicited:bool)->Result<bool,Reject>{
        let import_ms=imports::enabled(&self.chain.spec).then(||self.import_clock.unwrap_or_else(now_ms));
        if (self.chain.spec.cp().H_END==0||self.cfg.limits.first_imports) && self.journal.records/256>self.cfg.limits.continuous_history.unwrap_or(0)/256{self.capacity_recheck()?;}
        let hash=b.hash();
        let was_evicted=self.chain.was_evicted(&hash);
        if was_evicted&&!solicited{return Err(Reject::Evicted)}
        // A new block at or below head-R_keep is removed by the next retain() and cannot become canonical
        // (its work is at most the canonical block's at that height): refused before validation or
        // journaling, as local retention (governance.md:200), never as invalid.
        if !self.chain.blocks.contains_key(&hash)&&b.header.h.saturating_add(self.chain.r_keep)<=self.chain.head().height{return Err(Reject::Evicted)}
        let mut res=self.chain.validate(&b,Some(now_s()),self.capital_phi());
        let mut tries=0;
        let on_branch=solicited&&self.session.as_ref().is_some_and(|s|self.on_session_branch(s,&b.header.parent_hash));
        while let Err(Reject::NeedsRebuild(p))=&res{let p=*p;if tries==REBUILD_TRIES{break}
            // C-1 S4: a slimmed entry (the parent, or an ancestor the evidence walk reads) gets its header,
            // ParentResult and nonce ranges back from the journal and history.redb: two record reads, no state
            // work, at most REBUILD_TRIES per block (each one clears a slim flag), so neither budget below is
            // charged for it. A pruned parent still needs its state: it goes on, in the same try, to the deep
            // rebuild below, which is charged.
            if self.chain.get(&p).map_or(false,|e|e.slim){self.stats.unslims+=1;
                let src=Src{journal:&self.journal,history:&self.history,canon:&self.canon};
                if let Err(e)=self.chain.unslim(&p,&src){if let Reject::Local(w)=&e{self.fatal(w)}self.chain.reprune();return Err(e)}
                if !(p==b.header.parent_hash&&self.chain.get(&p).is_some_and(|e|e.pruned)){tries+=1;res=self.chain.validate(&b,Some(now_s()),self.capital_phi());continue}
            }
            // PR4 (I-5): deep rebuilds are rate-bounded. A solicited block on the session's branch draws on its
            // session's allowance (an honest adoption needs about one), any other one on the node's bucket (a side
            // block the session's peer relays included: it neither spends the allowance nor ends the session).
            // Exhaustion drops the block as a NON-VERDICT (Evicted): not marked invalid, not journaled, no penalty; a real branch
            // returns through a sync session (network.md D38). A session out of allowance is ended as not
            // reached, its peer's claim set aside until its next Hello. A solicited block outliving its session
            // (an orphan child taken after it ended) draws on the bucket.
            if on_branch{
                if self.session.as_ref().is_some_and(|s|s.rebuilds>=SESSION_REBUILD_MAX){self.log("sync session: deep-rebuild allowance reached; claim set aside");self.end_session(false);res=Err(Reject::Evicted);break}
                if let Some(s)=&mut self.session{s.rebuilds+=1;}
            }else if !self.rebuild_token(){self.log("deep-rebuild budget spent; an unsolicited block dropped (a real branch returns by sync)");res=Err(Reject::Evicted);break}
            tries+=1;self.log("TrieRebuild for a fork below the K_hist window");
            let src=Src{journal:&self.journal,history:&self.history,canon:&self.canon};
            if let Err(e)=self.chain.ensure_state_with(&p,Some(&src)){if let Reject::Local(w)=&e{self.fatal(w)}self.chain.reprune();return Err(e)}res=self.chain.validate(&b,Some(now_s()),self.capital_phi());}
        // PR4 (A13 MATERIALIZED): a block refused after its parent's state was rebuilt (or dropped at the budget
        // after an earlier rebuild of its retries) leaves none of it behind.
        if tries>0&&res.is_err(){self.chain.reprune();}
        let (mut entry,_)=match res{
            Ok(v)=>v,
            Err(Reject::UnknownParent)=>{
                let parent=b.header.parent_hash;
                self.add_orphan(parent,b);
                if let Some(id)=from{if let Some(p)=self.peers.get(&id){p.send(&Msg::GetBlock{hash:parent});}}
                return Err(Reject::UnknownParent);
            }
            Err(Reject::Future)=>{if self.future.len()<FUTURE_MAX{self.future.push(b);}return Err(Reject::Future)}
            Err(Reject::Duplicate)=>{if solicited&&!self.chain.is_canonical(&hash){self.protect(hash);}return Err(Reject::Duplicate)}
            // The node's own storage failed: not a verdict on the block, which is not marked invalid.
            Err(Reject::Local(why))=>self.fatal(&why),
            Err(Reject::Block(why))=>{self.chain.mark_invalid(hash,why.clone());self.reject(hash,&why);return Err(Reject::Block(why))}
            Err(e)=>{if let Reject::Envelope(w)=&e{self.reject(hash,w);}return Err(e)}
        };
        let mut first_import=None;
        if let Some(received_ms)=import_ms{
            first_import=Some(match imports::first(&self.canon,&self.journal,&hash){
                Ok(Some(original))=>original,
                Ok(None)=>{
                    // Only a fully valid block can cause these original-frame
                    // reads. Their time is not charged to the captured arrival.
                    let parent_received_ms=match self.first_import(&b.header.parent_hash){
                        Ok(Some(m))=>m.received_ms,
                        Ok(None)=>self.received.get(&b.header.parent_hash).copied().unwrap_or(received_ms),
                        Err(e)=>self.fatal(&e),
                    };
                    let tid=template_id(&b.header.ut_raw);
                    let template_ms=self.template_seen.get(&Self::seen_key(&b.header)).filter(|x|x.0==tid).map(|x|x.1);
                    let p=self.chain.get(&b.header.parent_hash).unwrap();match imports::classify(&self.chain.spec,p,&entry,received_ms,parent_received_ms,template_ms){
                    Ok(stamp)=>stamp,Err(e)=>self.fatal(&e)}},
                Err(e)=>self.fatal(&e),
            });
            entry.timely=first_import.unwrap().timely;
        }
        crate::crash_point("state-commit");
        // PR4: a validated signed block passed items 1, 2, 3 and 12 (20-byte proposer, a=0, h=p.h+1, sig by the
        // proposer, proposer=Draw(h,0)), every check observe_template makes, so its template is recorded without
        // reading the parent's state (observe_template's full_entry had materialized a diff-held parent in
        // place — an A13 MATERIALIZED excess).
        if entry.signed{self.record_template(&b.header);}self.observe_win(&b.header);
        // C-1 S1 (invariant I3): the receipts and transaction index are durable before the journal record.
        // A write that fails (a disk error, or a store marked damaged) stops the node as a failed journal
        // append does; the next open sets a damaged store aside and replay rebuilds it.
        if let Err(e)=self.history.put(&entry.hash,&entry.receipts,&entry.tx_hashes,&entry.ranges){self.log(&format!("FATAL history write: {}",e.0));std::process::exit(70);}
        crate::crash_point("history");
        let record=store::encode_record_with_import(&b.header.encoded,&b.body.encode(),&result_bytes(&entry.result),first_import);
        // D-REORG-REDB: record the journal frame offset so this block's body can be read back off-heap.
        match self.journal.append(&record){Ok(off)=>entry.body_off=Some(off),Err(e)=>{self.log(&format!("FATAL journal append: {e:?}"));std::process::exit(70);}}
        self.ckpt.roll=crate::checkpoint::roll(&self.ckpt.roll,&record);
        if import_ms.is_some(){
            crate::crash_point("import-index");
            let coverage=canon::ImportCoverage{binding:crate::checkpoint::Binding{records:self.journal.records,end:self.journal.end_offset(),hash:self.ckpt.roll},
                genesis_received_ms:self.genesis_received_ms};
            if let Err(e)=self.canon.note_import(Some((hash,entry.body_off.unwrap())),coverage){self.fatal(&e)}
        }
        crate::crash_point("journal");
        if solicited&&was_evicted{self.stats.refetched+=1;}
        self.received.insert(hash,first_import.map_or_else(now_ms,|m|m.received_ms));
        if self.received.len()>RECEIVED_MAX{let keep=self.chain.head().height.saturating_sub(8);let blocks=&self.chain.blocks;self.received.retain(|h,_|blocks.get(h).map_or(false,|e|e.height>=keep));}
        let old_head=self.chain.head;
        let new_head=self.chain.insert(entry);self.stats.imported+=1;
        // Only a block new here gets this far (a duplicate, an orphan or an invalid block returned above).
        if let Some(id)=from{self.credit(id,Useful::Block);}
        // A session's block is protected from the side cap while it is non-canonical (before retain()).
        if solicited&&!self.chain.is_canonical(&hash){self.protect(hash);}
        // A11: the abandoned blocks' journal offsets are taken before retain() may evict them; their
        // bodies are read back one block at a time when the pool is rebuilt.
        let detached=if new_head{self.detached_blocks(old_head)}else{vec![]};
        let fork_h=if new_head{self.chain.lca_height(&old_head,&self.chain.head).unwrap_or(0)}else{0};
        self.chain.retain_unsettled();
        // C-1 S5: the window moves with the head; what it evicts is written to the canonical index (a failure of the
        // node's own storage stops it, exit 70, and the next open rebuilds the index).
        if let Err(e)=self.chain.window(&Src{journal:&self.journal,history:&self.history,canon:&self.canon}){self.fatal(&format!("the window: {e:?}"))}
        if let Err(e)=flush_evicted(&mut self.chain,&self.canon){self.fatal(&e)}
        // D-REORG-REDB: the adopted blocks that were offloaded as non-canonical are made whole again
        // (after retain offloaded the abandoned branch, so the two are never resident together). C-1
        // S3a: before the side states are pruned (`settle`), so the fork point's state is reachable.
        if new_head{match self.chain.restore_canonical(fork_h,&Src{journal:&self.journal,history:&self.history,canon:&self.canon}){Ok(n)=>self.stats.restored+=n as u64,
            Err(e)=>{self.log(&format!("FATAL restore of the adopted branch: {e:?}"));std::process::exit(70);}}}
        self.chain.settle();
        self.checkpoint_if_due();
        self.broadcast(&Msg::Block{header:b.header.encoded.clone(),body:b.body.encode()},from);
        if new_head{
            if self.chain.get(&self.chain.head).and_then(|e|e.parent)!=Some(old_head){self.stats.reorgs+=1;}
            self.log(&format!("head h={} {} signed={} txs={}",self.chain.head().height,&hex(&hash)[..14],self.chain.head().signed,b.body.txs.len()));
            self.on_new_head(detached);
            // Status refresh (network.md:7): peers learn the new head and its cumulative work.
            let hello=self.hello();
            for (id,p) in &self.peers{if self.peer_status.contains_key(id){p.send(&self.hello_to(p,hello.clone()));}}
        }
        // Orphans waiting for this block.
        // A copy whose body does not match its header is only an envelope fault: ask the peers for
        // the real block under that hash (the junk copy may have taken its orphan slot).
        // Children that arrived first keep the solicited status of their parent's import (a session's
        // blocks arriving out of order stay protected and re-fetchable).
        if let Some(children)=self.take_orphans(&hash){for c in children{let h=c.hash();if matches!(self.import_with(c,None,solicited),Err(Reject::Envelope(_))){self.broadcast(&Msg::GetBlock{hash:h},None);}}}
        Ok(new_head)
    }
    /// Bounded orphan store (A05): per parent, parent keys and bytes; the oldest parent goes first.
    fn add_orphan(&mut self,parent:[u8;32],b:Block){
        let hash=b.hash();let size=b.header.encoded.len()+b.body.txs.iter().chain(b.body.evidence.iter()).map(|x|x.len()).sum::<usize>();
        // Duplicates are the same header AND body: a copy with a foreign body (an envelope fault once
        // the parent arrives) must not keep the real block out (review finding, A01-adjacent).
        if self.orphans.get(&parent).map_or(false,|l|l.len()>=ORPHANS_PER_PARENT||l.iter().any(|x|x.hash()==hash&&x.body==b.body)){return}
        if size>ORPHAN_BYTES_MAX{return}
        if !self.orphans.contains_key(&parent){self.orphan_order.push_back(parent);}
        self.orphans.entry(parent).or_default().push(b);self.orphan_bytes+=size;
        while self.orphan_order.len()>ORPHAN_PARENTS_MAX||self.orphan_bytes>ORPHAN_BYTES_MAX{
            let Some(old)=self.orphan_order.pop_front() else{break};
            if let Some(l)=self.orphans.remove(&old){self.orphan_bytes-=l.iter().map(orphan_size).sum::<usize>();}
        }
    }
    fn take_orphans(&mut self,parent:&[u8;32])->Option<Vec<Block>>{
        let l=self.orphans.remove(parent)?;self.orphan_order.retain(|k|k!=parent);self.orphan_bytes-=l.iter().map(orphan_size).sum::<usize>();Some(l)
    }
    pub fn orphan_stats(&self)->(usize,usize,usize){(self.orphans.len(),self.orphans.values().map(|l|l.len()).sum(),self.orphan_bytes)}
    fn reject(&mut self,hash:[u8;32],why:&str){self.rejects.push_back((hex(&hash),chain::truncate(why)));if self.rejects.len()>REJECTS_MAX{self.rejects.pop_front();}self.log(&format!("reject {}: {why}",&hex(&hash)[..14]));}
    fn on_new_head(&mut self,detached:Vec<u64>){
        self.rebuild_pool(detached);
        if let Some(s)=self.cfg.stop_at{if self.chain.head().height>=s{self.stopped=true;return}}
        self.restart_production();
    }

    // ---------------- pool ----------------
    fn next_block_ctx(&self)->NativeBlockContext{self.child_block_ctx(self.chain.head())}
    /// The execution context of a child of `h` built now (pool admission and template selection).
    fn child_block_ctx(&self,h:&Entry)->NativeBlockContext{
        let cp=self.chain.spec.cp();
        let floor=self.chain.spec.base_fee_floor(h.height.saturating_add(1));
        let base=if h.height==0{chain::from_fixed_lp4(&cp.baseFee0).max(floor)}else{pocol_lp4_execution::fees::next_base_fee(h.base_fee,h.gas_used,h.header.gas_limit,floor).unwrap_or(U256::MAX)};
        NativeBlockContext{height:U256::from(h.height+1),timestamp:U256::from(now_s().max(h.ts+1)),gas_limit:U256::from(cp.GAS_LIMIT),base_fee:base,prevrandao:B256::ZERO}
    }
    pub fn submit_tx(&mut self,raw:Vec<u8>,relay:bool)->Result<B256,String>{self.submit_tx_by(raw,relay,None)}
    /// `submit_tx` for an RPC request with a COMPUTE_DEADLINE `by` (network.md RpcGuard (D) step 4, (F);
    /// review R2-2): the execution polls the deadline every STOP_GAS gas and before every call, and a
    /// transaction whose admission ends past it is refused with SUBMIT_DEADLINE before anything is
    /// pooled or relayed, so the -32020 {deadline} answer never hides an accepted transaction.
    pub fn submit_tx_by(&mut self,raw:Vec<u8>,relay:bool,by:Option<Instant>)->Result<B256,String>{
        // D-7: a submission is executed after the backlog, in pool order. An RPC submission drives the
        // backlog until its compute deadline and is answered busy if the backlog is still not done (nothing
        // is pooled); an in-process caller (no deadline) settles it first.
        if self.pool_pending(){match by{
            Some(by)=>{while self.pool_pending()&&Instant::now()<by{self.pool_step_until(by);}if self.pool_pending(){return Err(SUBMIT_BUSY.into())}}
            None=>self.pool_settle()}}
        self.admit_tx(raw,if relay{Relay::Gossip}else{Relay::No},by,true)
    }
    /// Tests only: queues `raw` for revalidation, as a new head queues a transaction returned from an abandoned block
    /// or left in the previous pool (`PoolItem::Tx`, not relayed), and settles the backlog. Such a transaction is not
    /// a new submission: the overdraft rule, replacement and fee eviction do not apply to it (N4), so a transaction
    /// funded only by another pending one is admitted on the pending state, as an abandoned block's can be.
    #[doc(hidden)]
    pub fn requeue_for_revalidation(&mut self,raw:Vec<u8>){
        self.pool.backlog_bytes+=raw.len();self.pool.backlog.push_back(PoolItem::Tx(raw,Relay::No));self.pool_settle();
    }
    /// A transaction from peer `from`: admitted now, or queued behind the backlog (D-7) and relayed once
    /// admitted. A queued one is held within POOL_MAX_BYTES with the pool. Only one admitted now credits `from`
    /// (`credit`): a duplicate is answered Ok without entering the pool, so the pool must grow; a queued one earns
    /// nothing, since the backlog does not record its sender (that would change PoolItem's A13 cost).
    fn relay_tx(&mut self,raw:Vec<u8>,from:PeerId){
        if !self.pool_pending(){let n=self.pool.txs.len();let _=self.submit_tx(raw,true);if self.pool.txs.len()>n{self.credit(from,Useful::Tx);}return}
        if self.pool.bytes+self.pool.backlog_bytes+self.pool.parked_bytes+raw.len()>POOL_MAX_BYTES{return}
        self.pool.backlog_bytes+=raw.len();self.pool.backlog.push_back(PoolItem::Fresh(raw));
    }
    /// Whether a new head left pool revalidation to do (D-7).
    pub fn pool_pending(&self)->bool{!self.pool.backlog.is_empty()}
    /// D-6: a dropped checkpoint's nodes are still owed (store mode).
    pub fn release_pending(&self)->bool{self.state_store.as_ref().map_or(false,|s|s.release_pending())}
    fn background_pending(&self)->bool{self.pool_pending()||self.release_pending()}
    /// One background step, as `run` takes it between events: a pool slice (D-7) or a release step (D-6).
    /// With both pending they alternate (review 2026-10-27): a pool whose backlog does not empty between
    /// heads would otherwise leave the whole release to the next checkpoint's write, the stall the steps
    /// remove. An event still waits for at most one step of either kind.
    pub fn background_step(&mut self){
        let release=self.release_pending()&&(self.release_turn||!self.pool_pending());
        if release{self.release_step()}else if self.pool_pending(){self.pool_step()}
        self.release_turn=!release;
    }
    /// D-6: one release step (`RELEASE_SLICE` rows, one transaction). A step that fails stops the steps until
    /// the next checkpoint's write or open finishes what is owed; only a broken count marks the store damaged
    /// (`StateStore::release_step`).
    pub fn release_step(&mut self){
        let Some(st)=self.state_store.clone() else{return};
        let t0=Instant::now();
        match st.release_step(RELEASE_SLICE){Ok(n)=>{self.stats.released_rows+=n as u64;}
            Err(e) if st.io_failed()=>self.fatal(&format!("release step: {}",e.0)),
            Err(e)=>self.log(&format!("release step: {}",e.0))}
        let ms=t0.elapsed().as_millis() as u64;self.stats.release_steps+=1;self.stats.release_step_max_ms=self.stats.release_step_max_ms.max(ms);
    }
    /// Revalidates the whole backlog now (in-process callers and tests; `run` never does).
    pub fn pool_settle(&mut self){while self.pool_pending(){self.pool_step_until(Instant::now()+Duration::from_secs(3600));}}
    /// One slice of revalidation (D-7): at most `pool_slice`, checked between transactions.
    pub fn pool_step(&mut self){let by=Instant::now()+self.cfg.limits.pool_slice;self.pool_step_until(by);}
    fn pool_step_until(&mut self,by:Instant){
        let t0=Instant::now();let body_max=self.chain.spec.cp().BODY_MAX as usize;
        while let Some(item)=self.pool.backlog.pop_front(){
            match item{
                PoolItem::Block(off)=>match self.journal.read_at(off).map_err(|e|format!("{e:?}")).and_then(|(_,bb,_)|Body::decode(&bb,body_max).map_err(|e|format!("{e:?}"))){
                    // An abandoned body is held one at a time; its transactions go first, in block order, and
                    // each one admitted again is offered to the peers.
                    Ok(b)=>{for raw in b.txs.into_iter().rev(){self.pool.backlog_bytes+=raw.len();self.pool.backlog.push_front(PoolItem::Tx(raw,Relay::Offer));}}
                    Err(e)=>self.log(&format!("journal read of a detached block at {off}: {e}"))},
                PoolItem::Tx(raw,relay)=>{self.pool.backlog_bytes-=raw.len();
                    // A pool full by count refuses every further revalidation without a state change, so the
                    // rest of the revalidation backlog (and the abandoned bodies) is dropped (PR #27 review R5-6).
                    // N4: a queued P2P arrival (`Fresh`) is a new submission, which may still replace a pooled
                    // transaction or evict the lowest-priority one at the count bound, so it is kept and admitted.
                    if self.pool.txs.len()>=POOL_MAX_TXS{
                        self.pool.backlog.retain(|i|matches!(i,PoolItem::Fresh(_)));
                        self.pool.backlog_bytes=self.pool.backlog.iter().map(|i|match i{PoolItem::Fresh(r)=>r.len(),_=>0}).sum();
                        if self.pool.backlog.is_empty(){break}
                        continue}
                    let _=self.admit_tx(raw,relay,None,false);}
                // N4 review: only a P2P arrival queued behind the backlog (`relay_tx`) is a new submission; a parked or
                // previous-pool transaction re-queued at a head is revalidated (`fresh` false) whatever its relay.
                PoolItem::Fresh(raw)=>{self.pool.backlog_bytes-=raw.len();let _=self.admit_tx(raw,Relay::Gossip,None,true);}
            }
            if Instant::now()>=by{break}
        }
        let ms=t0.elapsed().as_millis() as u64;self.stats.pool_slices+=1;self.stats.pool_slice_max_ms=self.stats.pool_slice_max_ms.max(ms);
    }
    /// `fresh`: a new submission (RPC, or a peer's relay), which may replace a pooled or parked transaction of its
    /// sender and nonce (N4, `replace`) or evict the lowest-priority one at the count or byte bound (N4,
    /// `eviction_victim`), and must be covered by its sender's head-state balance with the sender's other pooled and
    /// parked transactions (N4 review, POOL_OVERDRAFT); a revalidation at a new head (the previous pool, the parked
    /// set, the abandoned blocks) does none of these.
    fn admit_tx(&mut self,raw:Vec<u8>,relay:Relay,by:Option<Instant>,fresh:bool)->Result<B256,String>{
        let cp=self.chain.spec.cp();
        if raw.len()>cp.TX_MAX as usize{return Err("-32014 oversized".into())}
        let env=wire::decode_for_profile(&raw,self.chain.spec.profile()).map_err(|e|format!("{} {e:?}",e.specified_pool_code().unwrap_or(-32000)))?;
        let hash=env.hash();
        if self.pool.txs.iter().any(|(h,_)|*h==hash){return Ok(hash)}
        // PR6 P2: a parked one is held already; nothing changes.
        if self.pool.parked.iter().any(|x|x.hash==hash){return Err(POOL_PARKED.into())}
        // D-7: refused before it is executed, with no state change; a new head recounts the sum.
        let (gas_limit,caller,nonce)=(env.tx_env().gas_limit,env.tx_env().caller,env.tx_env().nonce.unwrap_or(0));let fields=TxFields::of(&env.tx_env());
        if fresh{
            // N4 (V3-20), replace-by-fee: a new submission with a pooled or parked transaction's sender and nonce replaces
            // it, or is refused (`replace`); it is never committed to the pending state, where that nonce is used.
            if let Some(i)=self.pooled_at(caller,nonce){return self.replace(raw,hash,&env.tx_env(),fields,Slot::Pooled(i),relay,by)}
            if let Some(j)=self.pool.parked.iter().position(|x|x.sender==caller&&x.nonce==nonce){return self.replace(raw,hash,&env.tx_env(),fields,Slot::Parked(j),relay,by)}
        }
        // N4 review, the overdraft rule (EIP-1559 clients'): a new submission's sender's balance at the head covers it
        // and its pooled and parked ones, each at gas_limit x maxFeePerGas + value. A transaction funded only by
        // another pending one (whose effects are in the pending state) waits until that one is mined, so taking a
        // pooled transaction out before the next head (eviction, replacement) or ordering by fee never leaves another
        // sender's new submissions pooled but unaffordable in the next block. (N4 second review: a transaction a new
        // head revalidates, `fresh` false, keeps the pending-state check alone, so it may depend on pending funds; the
        // fee order ranks such a sender's uncovered transactions last, `fee_ranks`.) Checked where the transaction
        // would be pooled or parked, after the refusals that come first as before PR N4 (the count and byte bounds'
        // plain refusal, then the sender quota), and before the eviction scan. Two reads and a scan of the pool's
        // entries.
        let overdraft=|n:&Self|fresh&&fields.cost().is_none_or(|c|!n.covers(caller,c,None));
        // N4 (V3-20), lowest-fee eviction: at the count or byte bound, a new submission that would be executed into the
        // pending state (its sender has nothing parked and the gas bound has room; otherwise it is refused, as before)
        // evicts the pooled transaction of lowest priority at the next base fee (`eviction_victim`: the last one of a
        // sender with nothing parked, ranked as the fee order ranks it), when its own priority is strictly higher;
        // else it is refused (POOL_EVICT_UNDERPRICED). The cheap refusals come first (N4 review): a fee cap below the
        // next base fee, an effective tip of 0, a nonce other than the sender's pending one. The victim leaves only once
        // the newcomer has executed and is committed, so an invalid newcomer evicts nothing.
        // N4 second review: the gas bound (at most POOL_MAX_TXS x GAS_LIMIT, `Node::open`) bounds what an eviction
        // executes too: the victim's writes stay in the layer (`ghost_gas`), so the layer holds at most A13's term.
        let mut victim=None;
        let total=self.pool.bytes+self.pool.backlog_bytes+self.pool.parked_bytes+raw.len();
        let at_bound=total>POOL_MAX_BYTES||self.pool.txs.len()>=POOL_MAX_TXS;
        if at_bound{
            let executes=!self.pool.parked.iter().any(|x|x.sender==caller)&&self.pool.gas.saturating_add(gas_limit)<=self.pool.gas_max;
            if !fresh||!executes{return Err("-32016 pool full".into())}
        }
        // PR6 P1: the sender quota, before the pool's bound. A transaction returned from an abandoned block was in a
        // sealed block and is not counted (a reorganization must not drop a sender's second block of them). N4 second
        // review: before the eviction's refusals too, so the answer does not depend on the count or byte bound.
        let exempt=matches!(relay,Relay::Offer|Relay::Returned);
        if !exempt{
            // PR6 review: and its parked ones (a sender may park a run of its next nonces).
            let held=self.pool.meta.iter().filter(|m|m.sender==caller&&!m.exempt).fold(0u64,|n,m|n.saturating_add(m.gas));
            let held=self.pool.parked.iter().filter(|x|x.sender==caller&&!matches!(x.relay,Relay::Offer|Relay::Returned)).fold(held,|n,x|n.saturating_add(x.gas));
            if held.saturating_add(gas_limit)>self.pool.sender_gas_max{self.stats.sender_quota_refused+=1;return Err(POOL_FULL_SENDER.into())}
        }
        if at_bound{
            use pocol_lp4_execution::view::View;
            let base=self.next_block_ctx().base_fee;let tip=fields.effective_tip(base);
            if tip==0{self.stats.evict_refused+=1;return Err(format!("{POOL_EVICT_UNDERPRICED} (an effective tip of 0 at the next base fee {base} evicts nothing)"))}
            if self.pool_view().account(&caller).map_or(0,|a|a.info.nonce)!=nonce{return Err("-32016 pool full".into())}
            if overdraft(self){self.stats.overdraft_refused+=1;return Err(POOL_OVERDRAFT.into())}
            // Its own priority: the fee order ranks it at the lowest tip of its sender's pooled ones and its own.
            let own=tip.min(self.chain_priority(caller,base));
            match self.eviction_victim(caller,total.saturating_sub(POOL_MAX_BYTES),base){
                Some((i,low)) if own>low=>victim=Some(i),
                Some((_,low))=>{self.stats.evict_refused+=1;return Err(format!("{POOL_EVICT_UNDERPRICED} (the lowest evictable priority is {low} wei per gas; this one ranks at {own} wei per gas)"))}
                None=>{self.stats.evict_refused+=1;return Err(format!("{POOL_EVICT_UNDERPRICED} (no pooled transaction can be evicted for it)"))}}
        }
        // PR6 third review: so is a later transaction of a sender with parked ones, whatever its gas. Its nonce follows
        // theirs, which the pending state does not have, so executed there it was refused as nonce too high and
        // lost (at a head, one that was pooled behind a parked one); parked, it joins its sender's run.
        // N4 x D-V3-21 review: under v3 a newcomer that would evict also passes the pool's checks on the head state (the
        // next block's parent, `head_checks`), as the selection will run it. The pending state can hold writes that no
        // block will make (an evicted or replaced transaction's, which stay until the next head), so there a creation
        // can look done and its burn zero; the head lacks what the pooled transactions ahead of it write. It evicts
        // only when both pass: the head checks, and its own execution on the pending state below (with its creation
        // charge within its signed maximum), as `eviction_checks` runs them for a parked one. N4 x D-V3-21 re-review:
        // priced on those two states only; a parked transaction of another sender that a block takes first is not
        // modelled (its sender has nothing parked here, else it is parked). Run before its execution on the pending
        // state, so no result is held across it (A13: one execution at a time); nothing is committed. An RPC
        // submission runs both within its one deadline (`by`).
        if victim.is_some()&&self.chain.spec.v3_active(self.chain.head().height.saturating_add(1)){
            let (tx,ctx,room)=(env.tx_env(),self.next_block_ctx(),self.headroom(caller,None).unwrap_or_default());
            if let Err(e)=self.head_checks(&tx,&ctx,by,room){self.stats.evict_refused+=1;return Err(e)}
        }
        if self.pool.gas.saturating_add(gas_limit)>self.pool.gas_max||self.pool.parked.iter().any(|x|x.sender==caller){
            // PR6 second review: refused as the engine would refuse it before executing it (intrinsic gas and the
            // rest), never parked.
            let (tx,ctx)=(env.tx_env(),self.next_block_ctx());pre_execution(&tx,&ctx,self.chain.spec.profile().spec().chain_id)?;
            // N4 review: one the overdraft rule refuses is not held (the answer of any unaffordable one here).
            if overdraft(self){self.stats.overdraft_refused+=1;return Err(POOL_FULL_GAS.into())}
            return Err(self.park(raw,hash,&tx,relay,fresh,by))}
        if overdraft(self){self.stats.overdraft_refused+=1;return Err(POOL_OVERDRAFT.into())}
        let ctx=self.next_block_ctx();
        let base=self.pool.pending_base.clone();let view=pocol_lp4_execution::view::LayerView{base:&base,layer:&self.pool.pending};
        let r=match(by,self.pool.code_total){
            (Some(by),total)=>native::admit_for_profile(&view,total,env.tx_env(),&ctx,self.chain.spec.profile(),std::rc::Rc::new(move||Instant::now()>=by)),
            (None,Some(total))=>native::execute_on_validated(&view,total,env.tx_env(),&ctx,self.chain.spec.profile()),
            (None,None)=>native::execute_for_profile(&view,env.tx_env(),&ctx,self.chain.spec.profile())};
        let mut r=match r{Err(revm::primitives::EVMError::Custom(m)) if m==native::SIMULATION_STOPPED=>return Err(SUBMIT_DEADLINE.into()),r=>r.map_err(|e|format!("-32000 {e:?}"))?};
        // A precompile runs to its end between two polls: the deadline is checked again before the commit.
        if by.is_some_and(|by|Instant::now()>=by){return Err(SUBMIT_DEADLINE.into())}
        // Price on the actual pending state before checking logs: a paid fee has
        // one receipt annotation, and an unpaid fee is refused like a cap halt.
        let total=if self.chain.spec.v3_active(self.chain.head().height.saturating_add(1)){match self.pool.code_total{Some(t)=>t,None=>pocol_lp4_execution::code_caps::inventory(&view).map_err(|e|format!("-32000 {e}"))?}}else{0};
        let (slots,accounts,_)=self.pool.meter.live();
        // D-V3-21: refused, too, when the gas cost plus the burn would pass the signed max_fee*gas_limit.
        user_phase::charge_creation(&view,&mut r,caller,user_phase::SignedFee::of(&env.tx_env()),&ctx,self.chain.spec.profile(),(slots,accounts),total).map_err(|h|format!("-32000 creation charge would halt ({h:?})"))?;
        if by.is_some_and(|by|Instant::now()>=by){return Err(SUBMIT_DEADLINE.into())}
        // PR6 review: one whose own logs pass a block's log caps would halt in every block on this head. The honest
        // selection takes it only as such a halt (charged all its gas); the pool refuses it, as it refuses one past
        // the meter's caps, so it cannot hold the gas bound unpaid.
        if let Some(why)=own_logs_halt_for(self.chain.spec.cp(),self.chain.head().cum_log_bytes,r.result.logs(),self.chain.spec.v3_active(self.chain.head().height.saturating_add(1))){return Err(format!("-32000 logs would exceed a block cap ({why:?})"))}
        // A13: the effects must stay within the caps a block enforces; then only live entries are kept.
        self.pool.meter.admit(&view,&r.state).map_err(|h|format!("-32000 pending state would exceed a cap ({h:?})"))?;
        // The same commit as a block's USER phase (EIP-161 cleanup, normalization), so the meter
        // follows the pending state exactly (second review: touched empty accounts were kept).
        let touched:Vec<Address>=r.state.keys().copied().collect();
        let before=pocol_lp4_execution::code_caps::code_of(&view,&touched);
        // C-1 E3: committed into the layer (no early return between taking it and putting it back). The
        // code drop of a whole pending state is not needed: the layer holds only the code written here.
        // D-7: nor does an entry keep the copy of its code the transaction read (A13 store mode).
        let mut ov=pocol_lp4_execution::view::Overlay::from_layer(&base,std::mem::take(&mut self.pool.pending));
        user_phase::commit_tx_on(&mut ov,r.state,true);let after=pocol_lp4_execution::code_caps::code_of(&ov,&touched);self.pool.pending=ov.into_layer();
        self.pool.pending.drop_code_copies(&touched);
        // The running total follows the commit; any failure (unreachable for an engine result) falls back to full checks.
        self.pool.code_total=match(self.pool.code_total,before,after){
            (Some(t),Ok(b),Ok(a))=>t.checked_sub(b).and_then(|t|t.checked_add(a)),_=>None};
        // N4: the victim leaves now (the newcomer executed); its effects stay in the pending layer until the next head.
        if let Some(i)=victim{self.evict_pooled(i);}
        self.pool.bytes+=raw.len();self.pool.gas+=gas_limit;self.pool.txs.push((hash,raw.clone()));self.pool.meta.push(Meta{gas:gas_limit,sender:caller,exempt,repl:0});
        match relay{
            Relay::Gossip=>self.broadcast(&Msg::Tx{raw},None),
            Relay::Offer=>{let m=Msg::Tx{raw};for p in self.peers.values(){if p.offer(&m){self.stats.reinserted_offered+=1}else{self.stats.reinserted_unoffered+=1}}}
            Relay::No|Relay::Returned=>{}}
        Ok(hash)
    }
    /// PR6 P2: a transaction refused only by the pool's gas bound (PR6 third review: or whose sender has parked ones,
    /// `admit_tx`) is held for the next head when it is the sender's next transaction (on the pending state, or after the sender's parked ones: PR6 review, a sender
    /// may park a run of its next nonces, within its quota), its sender can pay it and its parked ones at their
    /// fee caps and values, and its fee cap covers the next base fee (two reads, no execution). The set is bounded
    /// (`parked_max`, POOL_MAX_BYTES shared with the pool and the backlog); when it is full, the parked
    /// transaction declaring the most gas (the latest of equals), with its sender's later parked ones, makes room
    /// for one of another sender declaring strictly less (PR6 second review: and the last entry of the longest run makes
    /// room, at equal declared gas, for its sender's first one). Refused before parking: what the engine refuses
    /// before execution (`pre_execution`, in `admit_tx`). A parked transaction is a template candidate
    /// (`template_selection`) and is relayed now (PR6 review): the drawn proposer can take it in the next block
    /// even while the gas bound is held; PR6 second review: only one that evicted nothing, at most `parked_max` per
    /// head, offered; another is relayed when admitted at a later head. Answer: POOL_PARKED, or POOL_FULL_GAS when it is not held.
    /// N4: `fresh` (a new submission) may also evict parked ones of lower priority (`evict_parked_by_fee`); a
    /// revalidation at a new head does not.
    /// N4 x D-V3-21 review: under v3, before it evicts anything (by either rule), it passes the pool's checks on the
    /// head state and on the pending state (`eviction_checks`: the engine, the creation charge within its signed
    /// max_fee*gas_limit, its own logs, the meter), once; if they fail nothing is evicted, the refusal counts in
    /// `evict_refused` and the answer is their refusal. N4 x D-V3-21 re-review: neither state holds a parked
    /// transaction's effects, so under v3 one behind its sender's parked ones (a run) evicts nothing (POOL_FULL_GAS,
    /// counted in `evict_refused`), whatever it would pay: its sender's earlier parked ones come first in every block.
    /// A revalidated one is held to the same rule (dropped, with its sender's later ones, as one finding no room is).
    /// A parked transaction is not executed otherwise, so one whose burn would pass its signed maximum can take free
    /// parked room (never evict), until the head that admits it refuses it.
    fn park(&mut self,raw:Vec<u8>,hash:B256,tx:&revm::primitives::TxEnv,relay:Relay,fresh:bool,by:Option<Instant>)->String{
        use pocol_lp4_execution::view::View;
        let view=pocol_lp4_execution::view::LayerView{base:&self.pool.pending_base,layer:&self.pool.pending};
        let acct=view.account(&tx.caller).map(|a|a.info).unwrap_or_default();
        let (mut next,mut spent,mut run)=(acct.nonce,Some(U256::ZERO),None);
        for x in self.pool.parked.iter().filter(|x|x.sender==tx.caller){next=x.nonce.saturating_add(1);spent=spent.and_then(|s|s.checked_add(x.cost));run=Some(x.run);}
        let cost=U256::from(tx.gas_limit).checked_mul(tx.gas_price).and_then(|c|c.checked_add(tx.value));
        let total=cost.and_then(|c|spent.and_then(|s|s.checked_add(c)));
        let base=self.next_block_ctx().base_fee;
        if tx.nonce!=Some(next)||total.is_none_or(|c|acct.balance<c)||tx.gas_price<base{return POOL_FULL_GAS.into()}
        let prio=TxFields::of(tx).effective_tip(base).min(self.chain_priority(tx.caller,base));
        // N4 x D-V3-21 review: the eviction checks are owed (under v3) until the first eviction, run with the balance
        // the checks above leave it: for a new submission what the overdraft rule leaves (as for a replacement), else
        // its pending-state balance less its sender's other parked ones. One behind its sender's parked ones evicts
        // nothing (re-review: no state the pool holds has their effects).
        let mut owed=self.chain.spec.v3_active(self.chain.head().height.saturating_add(1));
        let room=if fresh{self.headroom(tx.caller,None).unwrap_or_default()}else{spent.map_or(U256::ZERO,|s|acct.balance.saturating_sub(s))};
        let ctx=self.next_block_ctx();
        macro_rules! gate{()=>{if std::mem::take(&mut owed){
            if run.is_some(){self.stats.evict_refused+=1;return POOL_FULL_GAS.into()}
            if let Err(e)=self.eviction_checks(tx,&ctx,by,room){self.stats.evict_refused+=1;return e}}}}
        // The fee eviction is planned first when the checks are owed, so they run only when it would evict; its plan
        // is dropped before they execute (the scratch is not held across an execution).
        macro_rules! by_fee{()=>{{
            if fresh&&(!owed||self.evict_parked_by_fee(tx.caller,prio,base,raw.len(),false)){gate!();if self.evict_parked_by_fee(tx.caller,prio,base,raw.len(),true){continue}}
            return POOL_FULL_GAS.into()}}}
        let mut evicted=false;
        while self.pool.parked.len()>=self.pool.parked_max||self.pool.bytes+self.pool.backlog_bytes+self.pool.parked_bytes+raw.len()>POOL_MAX_BYTES{
            evicted=true;
            let Some(i)=(0..self.pool.parked.len()).max_by_key(|&i|(self.pool.parked[i].gas,self.pool.parked[i].seq)) else{return POOL_FULL_GAS.into()};
            let (s,k)=(self.pool.parked[i].sender,self.pool.parked[i].nonce);
            if self.pool.parked[i].gas>tx.gas_limit&&s!=tx.caller{
                gate!();
                // Its sender's later ones would follow a nonce gap: they go with it.
                let (mut freed,mut n)=(0usize,0u64);
                self.pool.parked.retain(|x|if x.sender==s&&x.nonce>=k{freed+=x.raw.len();n+=1;false}else{true});
                self.pool.parked_bytes-=freed;self.stats.parked_evicted+=n;continue
            }
            // PR6 second review: a run's later entries hold room only while it is spare. A newcomer that would be its
            // sender's first parked one evicts, at equal declared gas, the last entry of the longest run (the latest
            // of equals), so one sender's run of cheap transactions cannot set the eviction floor for many slots.
            // N4 (V3-20): when neither rule makes room, a fresh submission takes the room from parked transactions of
            // other senders of lower priority (`evict_parked_by_fee`), when they free all of it; else nothing is evicted.
            if run.is_some(){by_fee!()}
            let p=&self.pool.parked;let mut runs:HashMap<Address,(usize,usize)>=HashMap::new();
            for (j,x) in p.iter().enumerate(){let e=runs.entry(x.sender).or_insert((0,j));e.0+=1;if x.nonce>p[e.1].nonce{e.1=j;}}
            let Some(j)=runs.values().filter(|(m,j)|*m>=2&&p[*j].gas>=tx.gas_limit).max_by_key(|(m,j)|(*m,p[*j].seq)).map(|(_,j)|*j) else{by_fee!()};
            drop(runs);gate!();
            let x=self.pool.parked.remove(j);self.pool.parked_bytes-=x.raw.len();self.stats.parked_evicted+=1;
        }
        self.pool.parked_bytes+=raw.len();self.pool.park_seq+=1;let seq=self.pool.park_seq;
        // PR6 second review: relayed now only when it evicted nothing and fewer than `parked_max` were relayed when
        // parked since the last head, and offered (a peer with half its queue in use does not get it and is kept):
        // eviction lets a sender park again at the same nonce, so relaying every parked one let the relays per head
        // grow without bound and unpaid. Any other one keeps its relay and is relayed when admitted at a later head.
        let relay=if relay==Relay::Gossip&&!evicted&&self.pool.parked_relays<self.pool.parked_max{
            let m=Msg::Tx{raw:raw.clone()};for p in self.peers.values(){let _=p.offer(&m);}
            self.pool.parked_relays+=1;self.stats.parked_relayed+=1;Relay::No}else{relay};
        self.pool.parked.push(Parked{gas:tx.gas_limit,nonce:next,seq,run:run.unwrap_or((tx.gas_limit,seq)),ahead:self.pool.txs.len() as u64,sender:tx.caller,hash,cost:cost.unwrap_or_default(),relay,raw,repl:0});
        POOL_PARKED.into()
    }
    /// N4: the pooled transaction of `sender` with `nonce` (its nonce read from its bytes, `tx_fields`).
    fn pooled_at(&self,sender:Address,nonce:u64)->Option<usize>{
        self.pool.meta.iter().enumerate().filter(|(_,m)|m.sender==sender).map(|(i,_)|i).find(|&i|tx_fields(&self.pool.txs[i].1).is_some_and(|f|f.nonce==nonce))
    }
    /// N4 review: the summed cost (gas_limit x maxFeePerGas + value) of `sender`'s pooled and parked transactions,
    /// leaving out `skip` (the entry a replacement takes); None past U256. N4 second review: at each one's current
    /// declared gas, read from its bytes (an entry's charged gas, `Meta::gas`, can be more after a replacement).
    fn committed(&self,sender:Address,skip:Option<Slot>)->Option<U256>{
        let mut sum=U256::ZERO;
        for (i,m) in self.pool.meta.iter().enumerate(){
            if m.sender!=sender||matches!(skip,Some(Slot::Pooled(k)) if k==i){continue}
            sum=sum.checked_add(tx_fields(&self.pool.txs[i].1)?.cost()?)?;
        }
        for (j,x) in self.pool.parked.iter().enumerate(){if x.sender==sender&&!matches!(skip,Some(Slot::Parked(k)) if k==j){sum=sum.checked_add(x.cost)?;}}
        Some(sum)
    }
    /// N4 review, the overdraft rule: whether `sender`'s balance at the head covers `cost` with its pooled and parked
    /// transactions (`committed`, without `skip`).
    fn covers(&self,sender:Address,cost:U256,skip:Option<Slot>)->bool{self.headroom(sender,skip).is_some_and(|h|h>=cost)}
    /// What the overdraft rule leaves `sender` for one more transaction: its head-state balance less `committed`.
    fn headroom(&self,sender:Address,skip:Option<Slot>)->Option<U256>{
        use pocol_lp4_execution::view::View;
        let head=self.pool.pending_base.account(&sender).map(|a|a.info.balance).unwrap_or_default();
        head.checked_sub(self.committed(sender,skip)?)
    }
    /// N4: the priority the fee order gives a transaction `sender` adds after its pooled and parked ones: the lowest
    /// effective tip at `base` among them (u64::MAX when it holds none), capped by the newcomer's own tip by the caller.
    /// Read only for a new submission the overdraft rule has passed, so its sender's head balance covers them all and
    /// the fee order's coverage rule (`fee_ranks`) leaves this rank as it is.
    fn chain_priority(&self,sender:Address,base:U256)->u64{
        let pooled=self.pool.meta.iter().enumerate().filter(|(_,m)|m.sender==sender).map(|(i,_)|tx_fields(&self.pool.txs[i].1).map_or(0,|f|f.effective_tip(base)));
        let parked=self.pool.parked.iter().filter(|x|x.sender==sender).map(|x|tx_fields(&x.raw).map_or(0,|f|f.effective_tip(base)));
        pooled.chain(parked).min().unwrap_or(u64::MAX)
    }
    /// N4 (V3-20): the eviction candidate at the pool's count or byte bound for a new submission of `caller` that
    /// needs `need` more bytes: among the pooled transactions that are the last of their sender (pool order is each
    /// sender's nonce order), of senders other than `caller` with nothing parked, holding at least `need` bytes, the
    /// one of lowest priority at `base`, with that priority. N4 review: a tail's priority is the one the fee order
    /// gives it (`fee_ranks`: the lowest effective tip along its sender's pooled transactions, 0 from the first its
    /// sender's head balance does not cover), not its own tip: a cheap first transaction holds a sender's later ones
    /// back whatever they pay. Ties take the latest. N4 second review: the scan is the fee order's (`fee_ranks`, A13
    /// FEE_ORDER_ENTRY_BYTES per candidate), so the eviction ranks exactly as the selection orders.
    fn eviction_victim(&self,caller:Address,need:usize,base:U256)->Option<(usize,u64)>{
        let (by,prio,_)=self.fee_ranks(base,u64::MAX);let np=self.pool.txs.len();
        let sender=|c:usize|if c<np{self.pool.meta[c].sender}else{self.pool.parked[c-np].sender};
        let mut best:Option<(u64,usize)>=None;
        for k in 0..by.len(){
            let i=by[k] as usize;
            // A sender's queue is its pooled ones then its parked ones: a pooled tail is last only with nothing parked.
            if i>=np||k+1<by.len()&&sender(by[k+1] as usize)==sender(i)||sender(i)==caller||self.pool.txs[i].1.len()<need{continue}
            let low=prio[i];
            if best.is_none_or(|(bt,bi)|low<bt||(low==bt&&i>bi)){best=Some((low,i));}
        }
        best.map(|(t,i)|(i,t))
    }
    /// N4: removes pooled transaction `i` (an eviction). Its effects stay in the pending layer until the next head,
    /// so its gas moves to `ghost_gas` (`gas` does not fall); the parked ones that arrived after it keep their order.
    /// N4 review: it was its sender's last transaction, so its nonce goes back to its sender in the pending state
    /// (the account keeps the balance the victim spent, which only makes its next checks stricter): the sender can
    /// submit it again, at a fee that outbids the pool's lowest, and its pending nonce (eth_getTransactionCount) does
    /// not count the evicted one. An account that would read as empty (EIP-161) keeps the nonce.
    fn evict_pooled(&mut self,i:usize){
        use pocol_lp4_execution::view::WorkState;
        let (_,raw)=self.pool.txs.remove(i);let m=self.pool.meta.remove(i);
        self.pool.bytes-=raw.len();self.pool.ghost_gas=self.pool.ghost_gas.saturating_add(m.gas);
        for x in &mut self.pool.parked{if x.ahead>i as u64{x.ahead-=1;}}
        if let Some(n)=tx_fields(&raw).map(|f|f.nonce){
            let mut ov=pocol_lp4_execution::view::Overlay::from_layer(&self.pool.pending_base,std::mem::take(&mut self.pool.pending));
            if let Some(info)=ov.info_mut(&m.sender){
                if info.nonce==n.saturating_add(1)&&(n>0||!info.balance.is_zero()||info.code_hash!=revm::primitives::KECCAK_EMPTY){info.nonce=n;}
            }
            self.pool.pending=ov.into_layer();
        }
        self.stats.pool_evicted+=1;
    }
    /// N4 (V3-20): makes room in the parked set for a transaction of `caller` of priority `prio` at `base` and `len`
    /// bytes (a free place when the set is full, and the bytes past POOL_MAX_BYTES). The candidates are the parked
    /// transactions of other senders whose priority, the fee order's (`fee_ranks`: the lowest effective tip among
    /// their sender's pooled ones and their run up to them, 0 from the first their sender's head balance does not
    /// cover), is below `prio`, taken lowest first and the latest of equals first (the parked set is in arrival
    /// order), so a run's later ones go before its earlier ones and what stays is still a run. N4 review: the room is
    /// planned in one pass first; only when the candidates free all of it are they evicted, else nothing is and the
    /// answer is false. Scratch: the fee order's (A13: FEE_ORDER_ENTRY_BYTES per candidate). N4 x D-V3-21 review:
    /// without `apply` it only plans (true when the candidates would free all of it), so `park` runs a newcomer's
    /// head checks only when it would evict.
    fn evict_parked_by_fee(&mut self,caller:Address,prio:u64,base:U256,len:usize,apply:bool)->bool{
        let (mut by,pr,_)=self.fee_ranks(base,u64::MAX);let np=self.pool.txs.len();
        let p=&self.pool.parked;let n=p.len();
        by.retain(|&c|{let c=c as usize;c>=np&&p[c-np].sender!=caller&&pr[c]<prio});
        by.sort_unstable_by_key(|&c|(pr[c as usize],std::cmp::Reverse(c)));drop(pr);
        let need_n=(n+1).saturating_sub(self.pool.parked_max);
        let need_b=(self.pool.bytes+self.pool.backlog_bytes+self.pool.parked_bytes+len).saturating_sub(POOL_MAX_BYTES);
        let (mut take,mut freed)=(0usize,0usize);
        while take<by.len()&&(take<need_n||freed<need_b){freed+=p[by[take] as usize-np].raw.len();take+=1;}
        if take<need_n||freed<need_b{return false}
        if !apply{return true}
        by.truncate(take);for c in by.iter_mut(){*c-=np as u32;}by.sort_unstable();
        let mut k=0u32;self.pool.parked.retain(|_|{let keep=by.binary_search(&k).is_err();k+=1;keep});
        self.pool.parked_bytes-=freed;self.stats.parked_fee_evicted+=take as u64;true
    }
    /// N4 (V3-20), replace-by-fee: `tx` has the sender and nonce of the pooled or parked entry `slot`. It replaces
    /// that entry in place (its position, so the arrival order, and a parked one's run and `ahead`, are kept) when:
    /// - both fee caps rise by at least POOL_PRICE_BUMP percent (`TxFields::bumps`), else POOL_REPLACE_UNDERPRICED;
    /// - fewer than POOL_REPLACE_SLOT_MAX replacements of this entry and fewer than POOL_REPLACE_SENDER_MAX of its
    ///   sender's entries were accepted since the last head, else POOL_REPLACE_LIMIT (N4 review: per entry and per
    ///   sender, so one sender cannot use up the others' replacements);
    /// - the engine's checks before execution pass (`pre_execution`) and its fee cap covers the next base fee;
    /// - the pool's byte bound, the sender quota (the replacement is a new transaction, never exempt) and, for a
    ///   pooled entry, the gas bound hold with it in the old one's place;
    /// - N4 review, the overdraft rule: the sender's head-state balance covers it with the sender's other pooled and
    ///   parked transactions (`covers`), so its later transactions stay affordable whatever it spends; a parked one's
    ///   pending-state check (`park`'s) holds too;
    /// - N4 review: a pooled one passes the pool's execution checks (`head_checks`: the engine, the creation
    ///   charge, its own logs, the meter) run on the head, as the selection runs it, without committing;
    /// - N4 x D-V3-21 re-review: under v3, a parked one that is its sender's first parked one (the only kind that can
    ///   have evicted, `park`) passes the checks an evicting one passes (`eviction_checks`, on the head and on the
    ///   pending state), so a transaction cannot evict and then be swapped for one that could not have. A later one
    ///   of a run evicted nothing (`park`) and is checked like a parked transaction (two reads).
    /// It is not committed to the pending state: the old one's effects stay in the pending layer until the next head,
    /// which re-executes the replacement in its place (`rebuild_pool`), and the selection executes it on the block's
    /// parent like any candidate. N4 second review: a pooled entry is charged (`Meta::gas`, in `gas` and the sender
    /// quota) the most gas it or any replacement of it declared since the last head, so a replacement declaring less
    /// frees nothing until the next head and raising it again charges nothing twice: replacements cannot take a
    /// sender past its quota of the gas bound. (A returned, quota-exempt entry's replacement is charged its own gas;
    /// the returned one's goes to `ghost_gas`, as its writes stay in the layer.) It is relayed while fewer than POOL_REPLACE_RELAY_MAX were since
    /// the last head; a later one is pooled here only (a parked one is relayed when admitted at the next head).
    fn replace(&mut self,raw:Vec<u8>,hash:B256,tx:&revm::primitives::TxEnv,fields:TxFields,slot:Slot,relay:Relay,by:Option<Instant>)->Result<B256,String>{
        use pocol_lp4_execution::view::View;
        let (old,repl,old_gas,old_len,old_exempt)=match slot{
            Slot::Pooled(i)=>{let m=self.pool.meta[i];(tx_fields(&self.pool.txs[i].1).unwrap_or_default(),m.repl,m.gas,self.pool.txs[i].1.len(),m.exempt)}
            Slot::Parked(j)=>{let x=&self.pool.parked[j];(tx_fields(&x.raw).unwrap_or_default(),x.repl,x.gas,x.raw.len(),matches!(x.relay,Relay::Offer|Relay::Returned))}};
        if !fields.bumps(&old){let (c,t)=TxFields::min_replacement(&old);self.stats.replace_refused+=1;
            return Err(format!("{POOL_REPLACE_UNDERPRICED} (both caps must rise by {POOL_PRICE_BUMP}%: maxFeePerGas >= {c} and maxPriorityFeePerGas >= {t} wei)"))}
        let by_sender=self.pool.meta.iter().filter(|m|m.sender==tx.caller).map(|m|u32::from(m.repl)).chain(self.pool.parked.iter().filter(|x|x.sender==tx.caller).map(|x|u32::from(x.repl))).sum::<u32>();
        if repl>=POOL_REPLACE_SLOT_MAX||by_sender>=POOL_REPLACE_SENDER_MAX{self.stats.replace_refused+=1;return Err(POOL_REPLACE_LIMIT.into())}
        let ctx=self.next_block_ctx();pre_execution(tx,&ctx,self.chain.spec.profile().spec().chain_id)?;
        if tx.gas_price<ctx.base_fee{return Err(format!("-32000 replacement maxFeePerGas {} is below the next base fee {}",tx.gas_price,ctx.base_fee))}
        if (self.pool.bytes+self.pool.backlog_bytes+self.pool.parked_bytes+raw.len()).saturating_sub(old_len)>POOL_MAX_BYTES{return Err("-32016 pool full".into())}
        let held=self.pool.meta.iter().filter(|m|m.sender==tx.caller&&!m.exempt).fold(0u64,|n,m|n.saturating_add(m.gas));
        let held=self.pool.parked.iter().filter(|x|x.sender==tx.caller&&!matches!(x.relay,Relay::Offer|Relay::Returned)).fold(held,|n,x|n.saturating_add(x.gas));
        // The gas the entry is charged with the replacement in place: a pooled one keeps its most since the last head.
        let charge=match slot{Slot::Pooled(_) if !old_exempt=>old_gas.max(tx.gas_limit),_=>tx.gas_limit};
        if held.saturating_sub(if old_exempt{0}else{old_gas}).saturating_add(charge)>self.pool.sender_gas_max{self.stats.sender_quota_refused+=1;return Err(POOL_FULL_SENDER.into())}
        let cost=fields.cost().ok_or(POOL_OVERDRAFT)?;
        let Some(room)=self.headroom(tx.caller,Some(slot)).filter(|h|*h>=cost) else{self.stats.overdraft_refused+=1;return Err(POOL_OVERDRAFT.into())};
        let relayed=relay==Relay::Gossip&&self.pool.replace_relays<POOL_REPLACE_RELAY_MAX;
        match slot{
            Slot::Pooled(i)=>{
                let gas=if charge>old_gas{let g=self.pool.gas.saturating_add(charge-old_gas);if g>self.pool.gas_max{return Err(POOL_FULL_GAS.into())}Some(g)}else{None};
                self.head_checks(tx,&ctx,by,room)?;
                // `charge` is below `old_gas` only for a returned entry: its writes stay in the layer (`ghost_gas`).
                match gas{Some(g)=>self.pool.gas=g,None=>self.pool.ghost_gas=self.pool.ghost_gas.saturating_add(old_gas-charge)}
                self.pool.bytes=self.pool.bytes+raw.len()-old_len;self.pool.txs[i]=(hash,raw.clone());
                let m=&mut self.pool.meta[i];m.gas=charge;m.exempt=false;m.repl=repl+1;
            }
            Slot::Parked(j)=>{
                let view=pocol_lp4_execution::view::LayerView{base:&self.pool.pending_base,layer:&self.pool.pending};
                let balance=view.account(&tx.caller).map(|a|a.info.balance).unwrap_or_default();
                let spent=self.pool.parked.iter().enumerate().filter(|(k,x)|x.sender==tx.caller&&*k!=j).try_fold(cost,|n,(_,x)|n.checked_add(x.cost));
                if spent.is_none_or(|c|balance<c){self.stats.overdraft_refused+=1;return Err(POOL_OVERDRAFT.into())}
                let first=!self.pool.parked.iter().any(|x|x.sender==tx.caller&&x.nonce<self.pool.parked[j].nonce);
                if first&&self.chain.spec.v3_active(self.chain.head().height.saturating_add(1)){self.eviction_checks(tx,&ctx,by,room)?;}
                self.pool.parked_bytes=self.pool.parked_bytes+raw.len()-old_len;
                let x=&mut self.pool.parked[j];x.gas=tx.gas_limit;x.hash=hash;x.cost=cost;x.raw=raw.clone();x.repl=repl+1;
                // Relayed now (below), so not again when admitted; one past the relay bound is relayed when admitted at
                // the next head (within the parked set's own bounds); a local one keeps no relay.
                x.relay=if relay==Relay::Gossip&&!relayed{Relay::Gossip}else{Relay::No};
            }
        }
        self.stats.pool_replaced+=1;
        if relayed{let m=Msg::Tx{raw};for p in self.peers.values(){let _=p.offer(&m);}self.pool.replace_relays+=1;self.stats.replace_relayed+=1;}
        else if relay==Relay::Gossip{self.stats.replace_unrelayed+=1;}
        Ok(hash)
    }
    /// N4 review: the execution checks a pooled replacement passes before it takes the old one's place. It runs as the
    /// selection runs a candidate, on the head state (the next block's parent; the pending state still holds the old
    /// one's effects), with its sender's nonce at its own and its balance at `room`, what the overdraft rule leaves
    /// it; then the pool's admission checks: the engine's, the creation charge and the meter's caps at the pending
    /// state's utilisation (as a newcomer's are priced) and its own logs. Nothing is committed (a scratch overlay).
    /// N4 x D-V3-21 review: also run, under v3, for a newcomer before it evicts a pooled transaction (`admit_tx`) or a
    /// parked one (`park`), with its balance at what the pool's checks leave it.
    /// The pending utilisation counts what the pooled transactions create, the old one's own creations included for a
    /// replacement (its effects stay in the pending state), so a replacement is priced as if one more of each unit it
    /// creates were taken: near a cap it costs more than a block would charge, and one creating the cap's last free
    /// unit is refused (AcctCap, SlotCap) until the next head (network.md, Replace-by-fee).
    /// N4 x D-V3-21 re-review: priced on the head state alone; the effects of the transactions a block takes before it
    /// (its sender's own earlier pooled and parked ones, and other senders') are not modelled, so a transaction that
    /// depends on them is priced as if they were not there (`eviction_checks` adds the pending state).
    fn head_checks(&self,tx:&revm::primitives::TxEnv,ctx:&NativeBlockContext,by:Option<Instant>,room:U256)->Result<(),String>{
        self.checks_on(&self.pool.pending_base,tx,ctx,by,room)
    }
    /// N4 x D-V3-21 re-review: the checks owed under v3 before a transaction evicts anything (a pooled one at the count
    /// or byte bound, `admit_tx`; a parked one, `park`) or replaces a parked one that may have evicted (`replace`):
    /// `head_checks` (an evicted or replaced transaction's writes, which no block makes, are not there), then the same
    /// checks on the pending state (the pooled transactions' effects, its sender's earlier ones and other senders', are
    /// there), with its sender's nonce at its own and its balance at `room` in both; both must pass. Two executions,
    /// one after the other, nothing held across them and nothing committed (A13: one at a time). Neither state holds a
    /// parked transaction's effects, so `park` lets no transaction behind its sender's parked ones evict; another
    /// sender's parked transaction that a block takes first is not modelled (network.md, Parked set). `admit_tx` runs
    /// the pending-state half as its own execution there.
    fn eviction_checks(&self,tx:&revm::primitives::TxEnv,ctx:&NativeBlockContext,by:Option<Instant>,room:U256)->Result<(),String>{
        self.head_checks(tx,ctx,by,room)?;
        let view=pocol_lp4_execution::view::LayerView{base:&self.pool.pending_base,layer:&self.pool.pending};
        self.checks_on(&view,tx,ctx,by,room)
    }
    /// The pool's execution checks of `tx` on `base` (`head_checks`, `eviction_checks`), on a scratch overlay.
    fn checks_on<B:pocol_lp4_execution::view::View>(&self,base:&B,tx:&revm::primitives::TxEnv,ctx:&NativeBlockContext,by:Option<Instant>,room:U256)->Result<(),String>{
        use pocol_lp4_execution::view::WorkState;
        let mut ov=pocol_lp4_execution::view::Overlay::new(base);
        let mut info=base.account(&tx.caller).map(|a|a.info).unwrap_or_default();
        info.nonce=tx.nonce.unwrap_or(0);info.balance=room;ov.insert_info(tx.caller,info);
        let profile=self.chain.spec.profile();
        let r=match(by,self.pool.code_total){
            (Some(by),total)=>native::admit_for_profile(&ov,total,tx.clone(),ctx,profile,std::rc::Rc::new(move||Instant::now()>=by)),
            (None,Some(total))=>native::execute_on_validated(&ov,total,tx.clone(),ctx,profile),
            (None,None)=>native::execute_for_profile(&ov,tx.clone(),ctx,profile)};
        let mut r=match r{Err(revm::primitives::EVMError::Custom(m)) if m==native::SIMULATION_STOPPED=>return Err(SUBMIT_DEADLINE.into()),r=>r.map_err(|e|format!("-32000 {e:?}"))?};
        if by.is_some_and(|by|Instant::now()>=by){return Err(SUBMIT_DEADLINE.into())}
        let v3=self.chain.spec.v3_active(self.chain.head().height.saturating_add(1));
        let total=if v3{match self.pool.code_total{Some(t)=>t,None=>pocol_lp4_execution::code_caps::inventory(&ov).map_err(|e|format!("-32000 {e}"))?}}else{0};
        let (slots,accounts,_)=self.pool.meter.live();
        // D-V3-21: as a new submission's (`admit_tx`), the replacement's own signed fee bounds its burn: one whose gas
        // cost plus the burn would pass its max_fee*gas_limit is refused here, before the old one leaves.
        user_phase::charge_creation(&ov,&mut r,tx.caller,user_phase::SignedFee::of(tx),ctx,profile,(slots,accounts),total).map_err(|h|format!("-32000 creation charge would halt ({h:?})"))?;
        if let Some(why)=own_logs_halt_for(self.chain.spec.cp(),self.chain.head().cum_log_bytes,r.result.logs(),v3){return Err(format!("-32000 logs would exceed a block cap ({why:?})"))}
        let mut meter=self.pool.meter;meter.admit(&ov,&r.state).map_err(|h|format!("-32000 pending state would exceed a cap ({h:?})"))?;
        Ok(())
    }
    /// The parked transactions, in the order the next head admits them (PR6 P2; tests and inspection): each
    /// sender's run by its first one's declared gas and arrival, in nonce order.
    pub fn pool_parked(&self)->Vec<Vec<u8>>{let mut v:Vec<&Parked>=self.pool.parked.iter().collect();v.sort_by_key(|x|(x.run,x.nonce));v.into_iter().map(|x|x.raw.clone()).collect()}
    /// Lowers the parked set's bound (tests reach a full set with a few senders); never above POOL_PARKED_MAX.
    pub fn set_pool_parked_max(&mut self,m:usize){self.pool.parked_max=m.min(POOL_PARKED_MAX);}
    /// Journal offsets of the blocks that left the canonical chain, from `old` back to the common
    /// ancestor with the new head, oldest first (A11). The walk always reaches the common ancestor
    /// (as `lca_height` does); the abandoned side is at most R_keep blocks, since a deeper fork is
    /// never adopted, and only its offsets are held here.
    fn detached_blocks(&self,old:[u8;32])->Vec<u64>{
        let (mut a,mut b)=(old,self.chain.head);let mut gone=Vec::new();
        while a!=b{
            let (Some(ea),Some(eb))=(self.chain.get(&a),self.chain.get(&b)) else{break};
            if ea.height>=eb.height{
                match ea.body_off{Some(off)=>gone.push(off),None=>self.log(&format!("detached block {} has no journal offset",&hex(&a)[..14]))}
                match ea.parent{Some(p)=>a=p,None=>break}
            }else{match eb.parent{Some(p)=>b=p,None=>break}}
        }
        gone.reverse();gone
    }
    fn rebuild_pool(&mut self,detached:Vec<u64>){
        // D-7: the pending state restarts at the new head and everything is queued for revalidation in
        // pool order, then one slice runs now. Transactions in abandoned blocks come first (consensus
        // §10), oldest block first and in block order, then the previous pool (its admitted transactions,
        // then its own backlog); each is re-executed on the new head (nonce, balance, fee), so one the
        // new branch includes is not taken again. One from an abandoned block that is admitted again is
        // offered to the peers (Relay::Offer); the previous pool's were relayed when first admitted.
        // PR6 P3, the order: (1) the abandoned blocks; (2) the previous pool up to each sender's first transaction
        // declaring more than GAS_LIMIT/2; (3) the parked transactions, least declared gas first (each sender's run by
        // its first one, in nonce order); (4) the previous
        // pool's deferred part (each sender's transactions from its first big one on, in pool order), then the
        // parked ones of those senders; (5) the previous backlog. Transactions a block cannot take many of are
        // re-admitted after the waiting small ones, so they cannot hold the gas bound against them across heads.
        // One refused for the gas bound here is parked again when eligible (`park`), not dropped.
        let old=std::mem::take(&mut self.pool.txs);let meta=std::mem::take(&mut self.pool.meta);let rest=std::mem::take(&mut self.pool.backlog);
        let mut parked=std::mem::take(&mut self.pool.parked);self.pool.backlog_bytes+=std::mem::take(&mut self.pool.parked_bytes);self.pool.parked_relays=0;
        parked.sort_by_key(|x|(x.run,x.nonce));
        // N4: the replacements' budget and the evicted or replaced gas restart with the pending layer.
        self.pool.bytes=0;self.pool.gas=0;self.pool.ghost_gas=0;self.pool.replace_relays=0;self.pool.pending=Default::default();self.pool.pending_base=chain::StateHandle::of(self.chain.head());
        let (meter,code)=pool_start(self.chain.head(),&self.chain.spec);self.pool.code_total=code;self.pool.meter=meter;
        let big=self.chain.spec.cp().GAS_LIMIT/2;
        let mut q:VecDeque<PoolItem>=detached.into_iter().map(PoolItem::Block).collect();
        let (mut later,mut deferred)=(HashSet::new(),Vec::new());
        for ((_,raw),Meta{sender,gas,exempt,..}) in old.into_iter().zip(meta){
            // N4 second review: its declared gas (a replaced entry's charge can be more).
            let gas=tx_fields(&raw).map_or(gas,|f|f.gas);
            self.pool.backlog_bytes+=raw.len();let item=PoolItem::Tx(raw,if exempt{Relay::Returned}else{Relay::No});
            if gas>big||later.contains(&sender){later.insert(sender);deferred.push(item);}else{q.push_back(item);}
        }
        let (after,first):(Vec<Parked>,Vec<Parked>)=parked.into_iter().partition(|x|later.contains(&x.sender));
        q.extend(first.into_iter().map(|x|PoolItem::Tx(x.raw,x.relay)));
        q.extend(deferred);q.extend(after.into_iter().map(|x|PoolItem::Tx(x.raw,x.relay)));
        q.extend(rest);self.pool.backlog=q;
        self.pool_step();
    }
    /// A signed template on `p` built from `sel` at `clock` as production builds it (`build_signed_template`), but
    /// neither recorded nor sent: for tests only. Broadcasting it with another template for the same parent and
    /// height is self-equivocation (F7). None without a mining key.
    #[doc(hidden)]
    pub fn build_template_unrecorded(&mut self,p:&Entry,sel:Selection,clock:u64)->Option<(Header,Body)>{self.build_signed_template(p,sel,clock)}
    /// Pool transactions that still execute in order (for a signed template on `p`), as selected now:
    /// `template_selection(p, now_s()).txs`.
    pub fn template_txs(&self,p:&Entry)->Vec<Vec<u8>>{self.template_selection(p,now_s()).txs}
    /// The execution context of a child of `p` exactly as `build_signed` at `clock` gives it (PR6 B1): its
    /// base fee, prevrandao, proposer timestamp and BLOCKHASH window. None when p has no draw context (the
    /// selection then uses `child_block_ctx`).
    fn template_ctx(&self,p:&Entry,clock:u64)->Option<(NativeBlockContext,Vec<(u64,B256)>)>{
        let ctx=self.chain.child_ctx(p,0).ok()?;let ts=ctx.window.proposer_timestamp(clock).ok()?;
        Some((NativeBlockContext{height:U256::from(p.height+1),timestamp:U256::from(ts),gas_limit:U256::from(self.chain.spec.cp().GAS_LIMIT),base_fee:ctx.base_fee,prevrandao:ctx.prevrandao},ctx.ancestors))
    }
    /// N4 (V3-20, node-local): the order the selection takes the pooled and parked candidates in, as candidate keys (a
    /// pooled one its index, a parked one the pool's length + its index). Each candidate's priority is the lowest
    /// effective tip at `base` (`TxFields::effective_tip`, min(maxPriorityFeePerGas, maxFeePerGas - base fee)) among it
    /// and its sender's earlier candidates (each sender's queue is its pooled ones in pool order, then its parked ones
    /// in arrival order: its nonce order); the highest priority comes first, ties by candidate key (the pooled ones in
    /// pool order, then the parked ones in arrival order; keys are distinct, so the order is total and deterministic).
    /// A sender's priorities do not rise along its queue and its keys do, so its candidates keep their nonce order;
    /// with equal tips this is the order before N4. N4 second review: a candidate its sender's head balance does not
    /// cover, with the sender's earlier ones (gas_limit x maxFeePerGas + value each), ranks 0 (`fee_ranks`), so it
    /// comes after its funders, in pool order. N4 review: the eviction ranks the same way (`eviction_victim`,
    /// `evict_parked_by_fee`). Bounded: a u32 and a u64 per candidate (at most POOL_MAX_TXS + POOL_PARKED_MAX, A13
    /// FEE_ORDER_ENTRY_BYTES, `resources::fee_order_bytes`), each allocated at its length once; O(n log n), sorted in place.
    pub fn fee_order(&self,base:U256)->Vec<u32>{self.fee_order_and_big(base,u64::MAX).0}
    /// `fee_order`, and the big-first slot's candidate (B6, PR6 third review) read from the same pass by sender
    /// (`fee_ranks`). This is the rule PR6 computed with a set of the senders seen, which this pass replaces (A13:
    /// `resources::selection_bytes` counts this scratch, at most 12 B per candidate, in that set's place).
    pub fn fee_order_and_big(&self,base:U256,half:u64)->(Vec<u32>,Option<usize>){
        let (mut idx,prio,big)=self.fee_ranks(base,half);
        idx.sort_unstable_by_key(|&c|(std::cmp::Reverse(prio[c as usize]),c));
        (idx,big)
    }
    /// N4: the fee order's pass by sender. The candidate keys sorted by (sender, key), so each sender's queue is a run
    /// in its nonce order; each candidate's priority (indexed by key, `fee_order`); and the big-first slot's candidate:
    /// the oldest candidate that is its sender's first (its first pooled one, or its first parked one when it has
    /// nothing pooled) and declares more than `half`. Pooled ones are in arrival order; a parked one arrived after the
    /// pooled ones below its `ahead` and before the others, and the parked set is in arrival order. N4 second review,
    /// coverage: along a sender's queue the costs (gas_limit x maxFeePerGas + value, its declared gas read from its
    /// bytes) are summed, and from the first candidate whose sum its sender's head-state balance does not cover, the
    /// priority is 0. A new submission is always covered (the overdraft rule); a transaction a new head revalidated
    /// (returned from an abandoned block, or left in the previous pool) may be funded by another sender's pending one,
    /// and on the block's parent it is not yet affordable: ranked last, it is tried after its funding (pool order:
    /// revalidation admits in block order), as the pool order took it before N4. One read of the head state per
    /// sender.
    fn fee_ranks(&self,base:U256,half:u64)->(Vec<u32>,Vec<u64>,Option<usize>){
        use pocol_lp4_execution::view::View;
        let np=self.pool.txs.len();let n=np+self.pool.parked.len();
        let sender=|c:usize|if c<np{self.pool.meta[c].sender}else{self.pool.parked[c-np].sender};
        let raw=|c:usize|if c<np{&self.pool.txs[c].1}else{&self.pool.parked[c-np].raw};
        let mut idx:Vec<u32>=(0..n as u32).collect();idx.sort_unstable_by_key(|&c|(sender(c as usize),c));
        let mut prio=vec![0u64;n];let mut low=u64::MAX;let (mut big_pooled,mut big_parked)=(None::<usize>,None::<usize>);
        let (mut balance,mut spent)=(U256::ZERO,Some(U256::ZERO));
        for k in 0..n{let c=idx[k] as usize;let f=tx_fields(raw(c));
            if k==0||sender(idx[k-1] as usize)!=sender(c){low=u64::MAX;spent=Some(U256::ZERO);
                balance=self.pool.pending_base.account(&sender(c)).map(|a|a.info.balance).unwrap_or_default();
                let gas=if c<np{f.map_or(self.pool.meta[c].gas,|f|f.gas)}else{self.pool.parked[c-np].gas};
                if gas>half{let b=if c<np{&mut big_pooled}else{&mut big_parked};if b.is_none_or(|b|c<b){*b=Some(c);}}}
            spent=spent.and_then(|s|f.and_then(|f|f.cost()).and_then(|x|s.checked_add(x)));
            low=if spent.is_none_or(|s|s>balance){0}else{low.min(f.map_or(0,|f|f.effective_tip(base)))};prio[c]=low;}
        let big=match(big_pooled,big_parked){(i,Some(c)) if i.is_none_or(|i|self.pool.parked[c-np].ahead as usize<=i)=>Some(c),(i,_)=>i};
        (idx,prio,big)
    }
    /// Pool transactions for a signed template on `p` built at `clock` (PR6). execution.md:83: a
    /// transaction that would pass TX_CAP or TXBYTES_CAP on p's branch (item 16, the accounting of
    /// `Chain::within_caps`) is skipped, and so are its sender's later ones (their nonces follow it).
    ///
    /// Gas (PR6 B2): each candidate is executed on p's state in the block's own context (B1) and counted at
    /// the gas it uses (post-refund, as `user_phase.rs:318`). It fits when its declared gas limit is at most
    /// GAS_LIMIT minus the gas used so far, the consensus rule (`user_phase.rs:288`); one that does not fit
    /// is skipped with its sender's later ones (kept in `rest` for the append round,
    /// `build_signed_template`), and the selection goes on. Without a checked parent state (`sim` None: an
    /// entry without its full state, tests only) declared gas is counted instead (`fallback`), valid by
    /// construction. A transaction whose logs or state effects would pass a block cap (B4) is skipped: it
    /// would halt in the block and be charged all its gas. One whose own logs pass a log cap halts in every
    /// block on p (`own_logs_halt`): it is included, as that halt, counted at its declared gas. Execution (PR6 second
    /// review): the selection executes at most `selection_exec_budget(GAS_LIMIT)` of gas, committed or left out
    /// (`Selection::executed`); a candidate whose declared gas does not fit what is left is kept, unexecuted, with
    /// those that do not fit by gas.
    pub fn template_selection(&self,p:&Entry,clock:u64)->Selection{
        let cp=self.chain.spec.cp();let mut sel=Selection::default();let mut used=0u64;let mut bytes=0usize;
        let (room_txs0,room_bytes0)=self.chain.caps_room(p);let (mut room_txs,mut room_bytes)=(room_txs0,room_bytes0);
        let mut skipped=HashSet::new();let mut gas_skipped=HashSet::new();let mut rest_bytes=0usize;
        let body_room=cp.BODY_MAX as usize-12;
        // PR #27 review R1-1: a skipped transaction (no room, or not executable) can be what a later
        // transaction of another sender depends on, for example the transfer that funds it. Each candidate
        // is therefore executed, in order, on a running copy of the parent state, and one that fails
        // validation there is skipped with the rest of its sender's transactions, so the selection stays
        // executable and an independent transaction is not starved. (The block's SYSTEM phases run before
        // its USER phase; a transaction depending on them may still be dropped by build_signed_template's
        // retry.)
        let profile=self.chain.spec.profile();
        // C-1 E3: on an overlay over p's state, not a copy of it.
        let handle=chain::StateHandle::of(p);
        let mut sim=if p.pruned||!p.full{None}else{summary_code_total(p,&self.chain.spec).map(|t|(pocol_lp4_execution::view::Overlay::new(&handle),t))};
        // B1: the block's context, and on the simulation the BLOCKHASH window the block's overlay gets
        // (`Job::run`). Pool admission keeps `next_block_ctx` (a validity pre-check that counts no gas).
        let ctx=match sim.as_mut().and_then(|(st,_)|self.template_ctx(p,clock).map(|(c,anc)|{pocol_lp4_execution::view::WorkState::set_block_hashes(st,&anc);c})){Some(c)=>c,None=>self.child_block_ctx(p)};
        // B4: the block caps a candidate's logs and state effects could reach (the pool's meter for
        // SLOT_CAP/ACCT_CAP/SYS_SLOT_MAX; logs exactly). Dirty keys and accounts are not modelled.
        let mut meter=sim.as_ref().map(|_|pool_start(p,&self.chain.spec).0);let (mut log_count,mut log_bytes)=(0u64,0u64);
        // D-7: candidates are the admitted transactions, then (PR6 review) the parked ones, then the backlog's (each is executed here on p's
        // state anyway); an abandoned block not yet expanded is not a candidate. A pooled one's sender and
        // declared gas come from the pool's meta, so one that is skipped is not decoded; B3: at most
        // BUILD_VISIT_MAX decodes (each recovers a signature).
        let mut decodes=0usize;
        // PR6 second review: the gas executed so far (`Selection::executed`), committed or left out, within
        // `selection_exec_budget`. A candidate is executed only when its declared gas fits what is left of it, so
        // one that B4 leaves out after running (it commits nothing, so `used` does not grow) cannot make the
        // selection run without bound; one that does not fit is kept for the append round, unexecuted.
        let budget=selection_exec_budget(cp.GAS_LIMIT);
        // B6: the oldest transaction declaring more than half a block, first in its sender's queue, is tried first
        // (it always fits an empty block), so a transaction that needs most of a block waits a bounded time behind
        // the small ones. If it fails here (for example it depends on another sender's transaction), the in-order
        // pass meets it again. PR6 third review: the pooled and the parked ones together. A parked one is its
        // sender's first when its sender has nothing pooled and it starts its run; it arrived after the pooled
        // transactions below its `ahead` and before the others. Without the parked ones a big transaction held by
        // the gas bound (parked when submitted, or again at each head behind the parked small ones, P3) was tried
        // only after every pooled one and never fit a block they used more than half of.
        // N4 (V3-20): each sender's first candidate is read from the fee order's pass by sender (`fee_order_and_big`),
        // which takes the place of the first-seen sender set (A13: its scratch is counted in that set's place).
        let (np,half)=(self.pool.txs.len(),cp.GAS_LIMIT/2);
        let (order,big)=self.fee_order_and_big(ctx.base_fee,half);
        // PR6 review: then the parked ones, in arrival order (each sender's run in nonce order): they were checked on
        // the pending state when parked (next nonce, balance, fee cap) and are executed here like any candidate, so
        // a valid transaction the gas bound holds out of the pool still reaches the next block. Candidate keys: a
        // pooled one its index, a parked one np + its index (the slot's key), the backlog none.
        // N4 (V3-20): the pooled and parked candidates are taken in fee order (`fee_order`): the highest effective tip
        // at the block's base fee first, each sender's in nonce order, ties in that candidate order (so equal tips
        // keep the order above).
        // N4 second review: a pooled one's declared gas is read from its bytes (its entry's charge can be more after a
        // replacement, `Meta::gas`).
        let declared=|c:usize|{let m=&self.pool.meta[c];(m.sender,tx_fields(&self.pool.txs[c].1).map_or(m.gas,|f|f.gas))};
        let ordered=order.into_iter().map(|c|{let c=c as usize;
            if c<np{(false,c,&self.pool.txs[c].1,Some(declared(c)))}else{let x=&self.pool.parked[c-np];(false,c,&x.raw,Some((x.sender,x.gas)))}});
        let backlog=self.pool.backlog.iter().filter_map(|i|match i{PoolItem::Tx(r,_)|PoolItem::Fresh(r)=>Some((false,usize::MAX,r,None)),PoolItem::Block(_)=>None});
        let slot_first=big.map(|i|if i<np{(true,i,&self.pool.txs[i].1,Some(declared(i)))}
            else{let x=&self.pool.parked[i-np];(true,i,&x.raw,Some((x.sender,x.gas)))});
        let mut big_in=false;
        for (slot,i,raw,known) in slot_first.into_iter().chain(ordered).chain(backlog){
            if !slot&&big_in&&big==Some(i){continue}
            // B3: nothing fits any more and nothing more can be kept for the append round.
            if room_txs==0||room_bytes<MIN_TX_BYTES||bytes+enc(MIN_TX_BYTES as usize)>body_room{break}
            let rest_full=rest_bytes>=cp.BODY_MAX as usize||sel.rest.len() as u64>=room_txs0;
            if (cp.GAS_LIMIT-used<TX_GAS_MIN||budget-sel.executed<TX_GAS_MIN)&&rest_full{break}
            let mut decode=||{if decodes>=BUILD_VISIT_MAX{return Err(())}decodes+=1;wire::decode_for_profile(raw,profile).map(|e|Some(e.tx_env())).or(Ok(None))};
            let (caller,gas_limit,mut env)=match known{Some((c,g))=>(c,g,None),None=>match decode(){Err(())=>break,Ok(None)=>continue,Ok(Some(t))=>(t.caller,t.gas_limit,Some(t))}};
            if skipped.contains(&caller){continue}
            // `rest` (B5): a candidate skipped only because it did not fit by gas, and its sender's later ones.
            // PR6 third review: only a pooled or parked one (checked when admitted or parked). A backlog one is only
            // decoded: junk declaring little gas (below its intrinsic gas, or from an unfunded key) went first in the
            // append round and failed each append build in turn, so the round added nothing and cost every build.
            let gas_skip=gas_skipped.contains(&caller)||gas_limit>cp.GAS_LIMIT-used||(sim.is_some()&&gas_limit>budget-sel.executed);
            if gas_skip&&!slot{
                gas_skipped.insert(caller);
                if !rest_full&&known.is_some(){rest_bytes+=raw.len();sel.rest.push(Cand{sender:caller,gas_limit,raw:raw.clone()});}
                continue}
            // Other reasons skip the sender (its later nonces follow it); the big-first slot (B6) marks nothing.
            let mut skip=|slot:bool|if !slot{skipped.insert(caller);};
            if gas_skip||raw.len() as u64>room_bytes||bytes+enc(raw.len())>body_room{skip(slot);continue}
            let gas=match &mut sim{
                Some((st,total))=>{
                    let tx=match env.take(){Some(t)=>t,None=>match decode(){Err(())=>break,Ok(None)=>{skip(slot);continue},Ok(Some(t))=>t}};
                    let (gas_price,priority,signed)=(tx.gas_price,tx.gas_priority_fee,user_phase::SignedFee::of(&tx));
                    let r=native::execute_on_validated(&*st,*total,tx,&ctx,profile);
                    if let Ok(r)=&r{sel.executed=sel.executed.saturating_add(executed_gas(&r.result));}
                    let r=match r{Ok(mut r)=>{
                        if let Some(m)=&meter{let (slots,accounts,_)=m.live();
                            // Repriced in this selection's order: a burn this order pushes past the signed
                            // max_fee*gas_limit (D-V3-21) is skipped like an unpaid one, never included as a halt.
                            if user_phase::charge_creation(&*st,&mut r,caller,signed,&ctx,profile,(slots,accounts),*total).is_err(){skip(slot);continue}}
                        Ok(r)
                    },Err(e)=>Err(e)};
                    match r{
                    // PR6 review: one whose own logs pass a log cap halts in every block on p (the pool refuses
                    // it, but its logs can follow the context, for example PREVRANDAO, which admission does not
                    // have). It is included as the block applies it, charged its declared gas as before PR6, so it
                    // cannot stay pooled unpaid. The halt writes no logs.
                    Ok(r) if own_logs_halt_for(cp,p.cum_log_bytes,r.result.logs(),self.chain.spec.v3_active(p.height.saturating_add(1))).is_some()=>{
                        let Ok((price,tip))=pocol_lp4_execution::fees::effective_fees(ctx.base_fee,gas_price,priority) else{skip(slot);continue};
                        if sim_halt(st,caller,gas_limit,price,tip).is_none(){skip(slot);continue}
                        gas_limit
                    }
                    Ok(r)=>{
                        // B4: a transaction that would halt on a block cap is left out.
                        let logs=r.result.logs();let (n,c)=(logs.len() as u64,user_phase::log_charge(logs));
                        let log_cap=cp.H_END!=0&&p.cum_log_bytes.saturating_add(log_bytes+c)>user_phase::caps::LOG_CAP;
                        let log_limits=user_phase::log_caps(self.chain.spec.v3_active(p.height.saturating_add(1)));
                        if log_count+n>log_limits.count||log_bytes+c>log_limits.bytes||log_cap{skip(slot);continue}
                        if let Some(m)=&mut meter{let mut m2=*m;if m2.admit(&*st,&r.state).is_err(){skip(slot);continue}*m=m2;}
                        log_count+=n;log_bytes+=c;let gas=r.result.gas_used();
                        let touched:Vec<Address>=r.state.keys().copied().collect();let before=pocol_lp4_execution::code_caps::code_of(&*st,&touched);
                        user_phase::commit_tx_on(st,r.state,true);
                        // An arithmetic failure (unreachable for an engine result) ends the simulation: declared
                        // gas is counted from here on.
                        match(before,pocol_lp4_execution::code_caps::code_of(&*st,&touched)){
                            (Ok(b),Ok(a))=>match total.checked_sub(b).and_then(|t|t.checked_add(a)){Some(t)=>*total=t,None=>{sim=None;sel.fallback=true}},_=>{sim=None;sel.fallback=true}}
                        gas
                    }
                    // Pre-execution validation (nonce, balance, fee): no execution work, only this sender.
                    Err(_)=>{skip(slot);continue}
                }},
                None=>{sel.fallback=true;gas_limit}
            };
            used+=gas;bytes+=enc(raw.len());room_txs-=1;room_bytes-=raw.len() as u64;sel.txs.push(raw.clone());
            if slot{big_in=true;}
        }
        debug_assert!(self.chain.within_caps(p,&sel.txs));
        sel.counted=used;sel
    }

    // ---------------- production (consensus §4) ----------------
    /// Starts producing on the current head, as `run` does first: for a node driven in process.
    pub fn start_production(&mut self){self.restart_production()}
    /// V3-02: the locked template's shares go to the late-share cache before production leaves it, and the cache is
    /// pruned to the current head's carrier window (`LateCache::prune`; v3 program set only).
    fn stash_late(&mut self){
        let h=self.chain.head().height+1;if !self.chain.late_shares(h){return}
        let (d,cap)=(u64::from(self.chain.spec.cp().D),SHARE_RELAY_PER_S_MAX*usize::from(self.chain.spec.cp().S_max));
        if let Some(Production{mode:Mode::Locked(l),shares,..})=&self.prod{
            let t=LateTemplate{tid:l.tid,height:l.header.h,nonce_max:l.ctx.assignment.nonce_max(),t_share:l.ctx.t_share,shares:shares.iter().copied().collect()};
            self.late_cache.stash(t,cap);
        }
        let chain=&self.chain;let head=chain.head();
        self.late_cache.prune(head.height,d,|j|chain.late_template(head,j));
    }
    fn restart_production(&mut self){
        self.stash_late();
        if let Some(p)=self.prod.take(){if let Some(m)=p.miner{m.stop();}}
        let head=self.chain.head().meta_clone();
        if head.is_final{self.log("chain is final; no production");return}
        let received_ms=match self.first_import(&head.hash){
            Ok(Some(m))=>m.received_ms,
            Ok(None)=>*self.received.get(&head.hash).unwrap_or(&now_ms()),Err(e)=>self.fatal(&e),
        };
        self.prod=Some(Production{parent:head.hash,height:head.height+1,attempt:0,mode:Mode::Idle,shares:BTreeSet::new(),miner:None,solved:false,received_ms});
        self.begin_attempt(0);
    }
    fn begin_attempt(&mut self,a:u32){
        let parent_hash=match &self.prod{Some(p)=>p.parent,None=>return};
        let Some(p)=self.full_entry(&parent_hash) else{return};
        self.stash_late();
        if let Some(pr)=&mut self.prod{pr.attempt=a;pr.shares.clear();pr.solved=false;if let Some(m)=pr.miner.take(){m.stop();}}
        if a==0{
            let ctx=match self.chain.child_ctx(&p,0){Ok(c)=>c,Err(e)=>{self.log(&format!("ctx error {e:?}"));return}};
            let me=self.cfg.miner.as_ref().map(|k|k.address());
            if me==Some(ctx.proposer){
                let hk=(p.hash,ctx.height);
                if self.signed_a0.contains_key(&hk){
                    // F7: we already signed an attempt-0 template for this (parent,height). A different
                    // signed template for the same pair is self-equivocation (type 1) and burns our bond,
                    // so never sign a second. Same process (a mining-key toggle): re-broadcast the cached
                    // template unchanged. After a restart (body not cached): decline to sign and fall
                    // through to wait for our own re-gossiped template, or a fallback — never a new sign.
                    if let Some((hdr,body))=self.signed_a0_mem.get(&hk).cloned(){
                        if let (Ok(h),Ok(b))=(decode_header(&hdr),Body::decode(&body,self.chain.spec.cp().BODY_MAX as usize)){
                            self.log(&format!("F7: re-broadcasting the template already signed for h={}",ctx.height));
                            self.mark_relayed(template_id(&h.ut_raw));
                            self.broadcast(&Msg::Template{shell:hdr,body},None);self.lock(h,b,false);return;
                        }
                    }
                    self.log(&format!("F7: already signed h={} before this restart; not signing a second template",ctx.height));
                }else{
                    let phi=self.capital_phi();
                    if now_s().saturating_add(phi)<ctx.window.lower(){if let Some(pr)=&mut self.prod{pr.mode=Mode::Propose{at_s:ctx.window.lower()-phi};}return;}
                    // B1: one clock for the selection and every build (one timestamp).
                    let clock=now_s();let sel=self.template_selection(&p,clock);
                    if sel.fallback{self.stats.template_gas_fallbacks+=1;}
                    if let Some(t)=self.build_signed_template(&p,sel,clock){
                        // Persist the signing record (fsync) BEFORE broadcasting, so a crash can never
                        // leave us free to sign a different template for this (parent,height).
                        self.record_signed_a0(p.hash,ctx.height,template_id(&t.0.ut_raw),t.0.encoded.clone(),t.1.encode());
                        self.stats.signed_templates+=1;self.mark_relayed(template_id(&t.0.ut_raw));
                        let shell=t.0.encoded.clone();self.broadcast(&Msg::Template{shell,body:t.1.encode()},None);
                        self.lock(t.0,t.1,false);return;
                    }
                }
            }
            let received=self.prod.as_ref().unwrap().received_ms;
            let deadline=ctx.window.deadline_ms(received,u64::from(self.chain.spec.local().delta_rel),self.chain.spec.cp().dFbWait).unwrap_or(0);
            if let Some(pr)=&mut self.prod{pr.mode=Mode::Wait{deadline_ms:deadline};}
            // V3-05 telemetry: this node waits in this slot for Draw(h,0)'s template (b_j, L(0), T_j).
            if deadline>0{self.latency.slot(parent_hash,ctx.proposer.0.0,received,u128::from(ctx.window.lower())*1000,deadline);}
            // A signed template may already have arrived for this parent.
            if let Some((h,b,arr,from))=self.templates.remove(&parent_hash){self.note_template_arrival(&h,arr);if arr<=deadline{self.try_lock_signed(h,b,from);}}
        }else{
            self.lock_fallback(&p,a);
        }
    }
    /// The signed template for `p` from a selection, built at `clock` (PR6 B5). At most TEMPLATE_BUILDS_MAX
    /// builds, all at one clock (one timestamp); only the returned one leaves the process (F7: the caller records
    /// and broadcasts it).
    /// - A USER transaction the build reports by index is dropped with its sender's later ones, and the build is
    ///   tried again (as before PR6).
    /// - A gas failure (`is_user_gas`: the simulation counted less gas than the block used, for example because
    ///   the block's SYSTEM phase ran first) carries no index: the transactions fall back to the longest prefix
    ///   whose declared gas fits GAS_LIMIT, valid by construction, and the rest goes back to the candidates. The
    ///   template is never emptied while such a prefix is known.
    /// - Append round (one): the candidates left out for gas (`Selection::rest`) are added within the room the
    ///   real build left, counting their declared gas, smallest first, the head of each sender's queue at a
    ///   time; the block is built again only if one fits. The simulation cannot inflate declared gas, so a
    ///   transaction that burns gas only in the simulation cannot leave the block under-filled. PR6 third review:
    ///   an appended transaction the build reports by index is dropped with every appended one after it, and the
    ///   block is built once more (its prefix executed), so the round costs at most two builds. Under v3
    ///   (D-V3-21 review) an appended one that halts in a successful append build is dropped the same way, so
    ///   the round costs at most three builds (a halt found in the build after an index failure).
    /// The last good build is kept when a later one fails.
    fn build_signed_template(&mut self,p:&Entry,sel:Selection,clock:u64)->Option<(Header,Body)>{
        self.cfg.miner.as_ref()?;
        let cp=self.chain.spec.cp();let (gas_limit,body_max)=(cp.GAS_LIMIT,cp.BODY_MAX as usize);
        let mut evidence=self.includable_evidence(p);
        let effects=match self.chain.evidence_effects(p,&Body{txs:vec![],evidence:evidence.clone()},p.height+1){Ok(e)=>e,Err(_)=>vec![]};
        let mut effects=if effects.len()==evidence.len(){effects}else{evidence.clear();vec![]};
        // Evidence alone over BODY_MAX (not reachable with E_max·EVID_BYTES_MAX < BODY_MAX): built without it.
        if (Body{txs:vec![],evidence:evidence.clone()}).encode().len()>body_max{evidence.clear();effects.clear();}
        let Selection{mut txs,mut rest,..}=sel;
        let sender=|chain:&Chain,raw:&Vec<u8>|wire::decode_for_profile(raw,chain.spec.profile()).ok().map(|e|(e.tx_env().caller,e.tx_env().gas_limit));
        let mut last_good:Option<(Header,Body)>=None;let mut base:Option<usize>=None;let t0=Instant::now();let mut builds=0;
        let v3=self.chain.spec.v3_active(p.height.saturating_add(1));
        while builds<TEMPLATE_BUILDS_MAX{
            builds+=1;
            // Evidence shares BODY_MAX with the transactions: drop transactions until the encoded body fits.
            let mut body=Body{txs:txs.clone(),evidence:evidence.clone()};
            while body.encode().len()>body_max&&!body.txs.is_empty(){body.txs.pop();}txs=body.txs.clone();
            if base.is_some_and(|n|txs.len()<=n){break}
            let key=self.cfg.miner.as_ref()?;
            match self.chain.build_signed(p,key,clock,body.clone(),effects.clone()){
                Ok((h,out,_))=>{
                    if let Some(n)=base{
                        // D-V3-21 review (v3 only, node-local): an appended transaction that halts in this build
                        // (an A03 cap halt charged its declared gas, for example LogCount behind the selected
                        // ones' logs, or a CreationFee) is dropped with every appended one after it, as one the
                        // build reports by index, and the block is built once more (its prefix executed, so that
                        // build has no halt from n on). The selection checks only its own order; this keeps the
                        // honest append round from including a halt it could leave out. It stays pooled.
                        if v3{let halts=match &out{chain::Executed::Mem(o)=>&o.halts,chain::Executed::Stored{run,..}=>&run.halts};
                            if let Some(i)=halts.iter().map(|(i,_)|*i).filter(|i|*i>=n&&*i<txs.len()).min(){
                                self.log(&format!("signed template: appended transaction {i} would halt ({:?}); dropped",halts.iter().find(|x|x.0==i).map(|x|x.1)));
                                txs.truncate(i);continue}}
                        self.stats.template_appended+=(body.txs.len()-n) as u64;last_good=Some((h,body));break}
                    // The append round, within the room this build left (exact: its own gas used).
                    let (room_txs,room_bytes)=self.chain.caps_room(p);let used_bytes:u64=body.txs.iter().map(|t|t.len() as u64).sum();
                    let add=append_round(&rest,gas_limit-h.gas_used,room_txs.saturating_sub(body.txs.len() as u64),room_bytes.saturating_sub(used_bytes),body_max.saturating_sub(body.encode().len()+8));
                    base=Some(body.txs.len());last_good=Some((h,body));
                    if add.is_empty(){break}
                    txs.extend(add.into_iter().map(|i|rest[i].raw.clone()));rest.clear();
                }
                Err(Reject::Local(why))=>self.fatal(&why),
                Err(Reject::Block(why))=>{
                    self.log(&format!("signed template failed: {why}"));
                    if is_user_gas(&why){
                        // A failure of the append build keeps the last good one.
                        if base.is_some(){break}
                        let mut sum=0u64;let k=txs.iter().take_while(|t|{sum=sum.saturating_add(sender(&self.chain,t).map_or(u64::MAX,|(_,g)|g));sum<=gas_limit}).count();
                        let tail=txs.split_off(if k==txs.len(){0}else{k});
                        let mut back:Vec<Cand>=tail.into_iter().filter_map(|raw|sender(&self.chain,&raw).map(|(s,g)|Cand{sender:s,gas_limit:g,raw})).collect();
                        // A13 (resources::selection_bytes): the tail is at most the selected transactions (GAS_LIMIT/
                        // TX_GAS_MIN entries, BODY_MAX bytes), counted beside the candidates' own bound.
                        back.append(&mut rest);rest=back;self.stats.template_gas_recoveries+=1;continue
                    }
                    // Drop the offending user transaction (USER phase reports its index) with its sender's later
                    // ones, and retry.
                    let idx=why.split("User(").nth(1).and_then(|s|s.split(|c:char|!c.is_ascii_digit()).find(|x|!x.is_empty())).and_then(|n|n.parse::<usize>().ok());
                    match idx{
                        // PR6 third review: in the append build, the appended ones from the failing one on are dropped
                        // (the transactions before it executed, so the next build succeeds): the append round costs at
                        // most two builds more (three under v3, with the halt drop above), whatever its candidates are.
                        Some(i) if base.is_some_and(|n|i>=n)&&i<txs.len()=>txs.truncate(i),
                        Some(i) if i<txs.len()=>{let s=sender(&self.chain,&txs[i]).map(|x|x.0);let chain=&self.chain;let mut j=0;txs.retain(|t|{j+=1;j-1<i||(j-1>i&&(s.is_none()||sender(chain,t).map(|x|x.0)!=s))});}
                        _ if base.is_some()=>break,
                        _=>{if txs.is_empty(){break}txs.clear();}}
                }
                Err(_)=>break,
            }
        }
        self.stats.template_builds+=builds as u64;
        if builds>1{self.log(&format!("signed template: {builds} builds in {} ms",t0.elapsed().as_millis()));}
        last_good
    }
    /// F7: record that we signed `tid` for (parent,h), persist it durably BEFORE the caller broadcasts,
    /// and cache the encoded template so a same-process re-attempt re-broadcasts it unchanged. Pruned to
    /// the reorg window (a pair below it can never become the production parent again) and hard-capped.
    fn record_signed_a0(&mut self,parent:[u8;32],h:u64,tid:[u8;32],header_enc:Vec<u8>,body_enc:Vec<u8>){
        self.signed_a0.insert((parent,h),tid);
        self.signed_a0_mem.insert((parent,h),(header_enc,body_enc));
        let cutoff=h.saturating_sub(self.chain.r_keep.max(64));
        self.signed_a0.retain(|(_,hh),_|*hh>=cutoff);
        self.signed_a0_mem.retain(|(_,hh),_|*hh>=cutoff);
        while self.signed_a0.len()>SIGNED_A0_MAX{
            let Some(k)=self.signed_a0.keys().min_by_key(|(_,hh)|*hh).copied() else{break};
            self.signed_a0.remove(&k);self.signed_a0_mem.remove(&k);
        }
        self.persist_signed_a0();
    }
    /// F7: atomically rewrite `<dir>/signed-a0` (temp file, fsync, rename, directory fsync), the same
    /// durability `retention_rkeep` uses. A write error is logged, not fatal: the in-memory guard still
    /// blocks a second signature this run; only cross-restart protection would be weakened.
    fn persist_signed_a0(&self){
        use std::io::Write;
        let dir=&self.cfg.dir;let path=dir.join("signed-a0");let tmp=dir.join("signed-a0.tmp");
        let mut body=String::new();
        for ((p,h),t) in &self.signed_a0{body.push_str(&hex(&p[..]));body.push(' ');body.push_str(&h.to_string());body.push(' ');body.push_str(&hex(&t[..]));body.push('\n');}
        let r=(||->std::io::Result<()>{
            {let mut f=std::fs::File::create(&tmp)?;f.write_all(body.as_bytes())?;f.sync_all()?;}
            std::fs::rename(&tmp,&path)?;if cfg!(unix){std::fs::File::open(dir)?.sync_all()?;}Ok(())
        })();
        if let Err(e)=r{self.log(&format!("F7: could not persist the signing record: {e}"));}
    }
    fn lock_fallback(&mut self,p:&Entry,a:u32){
        match self.chain.build_fallback(p,a){
            Ok((h,_,_))=>{self.stats.fallback_templates+=1;self.lock(h,Body::default(),true)}
            Err(Reject::Local(why))=>self.fatal(&why),
            Err(e)=>{self.log(&format!("fallback build failed a={a}: {e:?}"));if let Some(pr)=&mut self.prod{pr.mode=Mode::Idle;}}
        }
    }
    fn lock(&mut self,header:Header,body:Body,fallback:bool){
        let parent=match &self.prod{Some(p)=>p.parent,None=>return};
        let Some(p)=self.full_entry(&parent) else{return};
        let ctx=match self.chain.child_ctx(&p,header.a){Ok(c)=>c,Err(_)=>return};
        let tid=template_id(&header.ut_raw);
        if !fallback{self.note_template_seen(&header);}
        self.log(&format!("lock h={} a={} {} tid={}",header.h,header.a,if fallback{"FB"}else{"PROP"},&hex(&tid)[..12]));
        // V3-02: a template locked again leaves the late-share cache before stash_late prunes it, so its cached shares
        // cannot be dropped by that prune.
        let relocked=self.late_cache.remove(&tid);
        if !matches!(&self.prod,Some(Production{mode:Mode::Locked(l),..}) if l.tid==tid){self.stash_late();}
        // The previous miner is stopped and joined before the next starts: one miner thread at a time.
        if let Some(pr)=&mut self.prod{
            pr.miner=None;
            // Shares and `solved` belong to one TemplateID. A switch within an attempt (a locked template to
            // the sticky fallback after a type-1 equivocation) must not carry the abandoned template's shares
            // into the new header: they do not meet its share target, and its block would be invalid (item 9).
            if !matches!(&pr.mode,Mode::Locked(l) if l.tid==tid){pr.shares.clear();pr.solved=false;
                // V3-02: a template locked again (the head back on its parent, or the F7 attempt-0 copy) is mined
                // again: its cached shares come back to the production's, and its later shares take the locked path.
                if let Some(t)=relocked{pr.shares.extend(t.shares);}}
            else if let Some(t)=relocked{self.late_cache.templates.push_back(t);}
        }
        // A miner that cannot be spawned (the task limit reached) leaves this node idle for the template, not stopped.
        let miner=self.cfg.miner.as_ref().and_then(|k|chain::assignment_range(&ctx,k.address())).and_then(|(s,e)|
            miner::start(tid,s,e,ctx.target,ctx.t_share,usize::from(self.chain.spec.cp().S_max),self.miner_tx.clone()));
        if let Some(pr)=&mut self.prod{pr.mode=Mode::Locked(Box::new(Locked{header,body,tid,ctx,fallback}));pr.miner=miner;}
    }
    /// Validates a received signed template for the current parent: signature,
    /// proposer = Draw(h,0), a=0, window, and UT equal to the template rebuilt
    /// from apply(parent, body) (1b independence). Locks on success.
    /// Locks a signed template from Draw(h,0) for the production parent (consensus.md §4(a)). One that
    /// does not apply here (another parent or attempt, a missing or foreign signature, a timestamp
    /// outside the window or in the future) is ignored. One that the drawn proposer signed for this
    /// parent but whose block would be invalid (item 16, its evidence, or a rebuild that differs) is "as if
    /// absent, and the peer is punished" (consensus.md:101, review R1-5): its sender's connection is
    /// closed, a local penalty; a check this node cannot complete is no verdict (`template_error`).
    /// A template that locks is then relayed (`relay_template`); the checks that
    /// punish are deterministic on the parent state, and a node relays only a template that passed them,
    /// so an honest relay is never the cause of a punished template: the sender is still the peer that
    /// made it fail.
    fn try_lock_signed(&mut self,shell:Header,body:Body,from:PeerId)->bool{
        let parent=match &self.prod{Some(p)=>p.parent,None=>return false};
        if shell.parent_hash!=parent||shell.a!=0||shell.proposer.len()!=20{return false}
        let Some(p)=self.full_entry(&parent) else{return false};
        let ctx=match self.chain.child_ctx(&p,0){Ok(c)=>c,Err(_)=>return false};
        let tid=template_id(&shell.ut_raw);
        let ok_sig=matches!(hashes::recover_address(&hashes::sig_msg(&tid),&shell.sig),Ok(a) if a[..]==shell.proposer[..]);
        if !ok_sig||shell.proposer[..]!=ctx.proposer[..]||!ctx.window.contains(shell.ts)||crate::timing::is_future(shell.ts,now_s(),self.capital_phi()){return false}
        // Its block would fail item 2 (h = parent height + 1): as if absent, and the peer is punished, on every chain. This
        // is node-local: on spec 1 such a template used to be locked and mined although its block could never be valid.
        // Under V3-02 it is also checked before the evidence, whose rules are relative to a child of p (kind 3: the window
        // [max(1,h-D), h-1]); judged at a false height, a kind-3 item made the node stop.
        if shell.h!=p.height+1{return self.punish_template(from,"its height is not its parent's plus one (item 2)")}
        // consensus.md:101, §4(a): a template whose block would fail item 16 is as if absent (no lock).
        if !self.chain.within_caps(&p,&body.txs){return self.punish_template(from,"over a cap (item 16)")}
        let effects=match self.chain.evidence_effects(&p,&body,shell.h){Ok(e)=>e,Err(e)=>return self.template_error(from,"invalid evidence",e)};
        let rebuilt=self.chain.rebuild_signed(&p,&ctx,shell.ts,Address::from_slice(&shell.proposer),&body,effects);
        match rebuilt{
            Ok(ut) if ut==shell.ut_raw=>{
                let m=Msg::Template{shell:shell.encoded.clone(),body:body.encode()};
                self.latency.locked(&parent,now_ms());
                self.lock(shell,body,false);self.credit(from,Useful::Template);self.disc_useful(from,false,self.disc_now());self.relay_template(tid,&m,from);true}
            Ok(_)=>self.punish_template(from,"its rebuild differs"),
            Err(e)=>self.template_error(from,"its rebuild fails",e),
        }
    }
    /// V3-05 telemetry: the first arrival of Draw(h,0)'s signed template for a slot this node waits in
    /// (`latency::Tracker::slot`), whatever happens to it next (it may be late, or not lock). Only a template whose
    /// signature recovers to the slot's drawn proposer counts, so no other peer can write a sample; one signature
    /// recovery per such template until the slot's first one arrived. Telemetry only: no decision reads it.
    fn note_template_arrival(&mut self,h:&Header,arrival:u128){
        if h.a!=0||h.sig.len()!=65{return}
        let Some(drawn)=self.latency.waiting_for(&h.parent_hash) else{return};
        if h.proposer[..]!=drawn[..]{return}
        if !matches!(hashes::recover_address(&hashes::sig_msg(&template_id(&h.ut_raw)),&h.sig),Ok(a) if a[..]==drawn[..]){return}
        self.latency.arrival(&h.parent_hash,arrival,h.ts);
    }
    /// pocol_templateLatency (and pocol_status `templateLatency`): the V3-05 counters, with the local timing T_j
    /// derives from.
    pub fn template_latency_json(&self)->serde_json::Value{
        let l=self.chain.spec.local();
        self.latency.json(serde_json::json!({"delta":l.delta,"phi":l.phi,"tExec":l.t_exec,"deltaRel":l.delta_rel,"gamma":l.gamma,"dFbWait":self.chain.spec.cp().dFbWait}))
    }
    /// An error checking a template from Draw(h,0). Only a verdict on its block (`Reject::Block`) punishes
    /// the sender. The node's own storage failing (`Reject::Local`) stops it, as on every production path,
    /// and a state it does not hold now (`NeedsRebuild`) is no verdict either: no lock and no penalty. So a
    /// local fault never closes an honest proposer or an honest relay.
    fn template_error(&mut self,from:PeerId,what:&str,e:Reject)->bool{
        match e{
            Reject::Block(why)=>self.punish_template(from,&format!("{what}: {why}")),
            Reject::Local(why)=>self.fatal(&why),
            e=>{self.log(&format!("signed template not checked here ({what}: {e:?}): no lock, no penalty"));false}
        }
    }
    /// Records a TemplateID as sent; false when it already was. Bounded: past RELAYED_MAX the set is
    /// cleared (an old TemplateID can no longer lock anywhere, so relaying it again would be harmless).
    fn mark_relayed(&mut self,tid:[u8;32])->bool{
        if self.relayed.len()>=RELAYED_MAX{self.relayed.clear();}
        self.relayed.insert(tid)
    }
    /// Gossip (network.md:13): a signed template that locked here goes to every other peer, once per
    /// TemplateID, never back to its sender. Without it a miner reached only through another node never
    /// sees the drawn proposer's template and mines an empty fallback instead (review DEC-04).
    fn relay_template(&mut self,tid:[u8;32],m:&Msg,from:PeerId){
        if !self.mark_relayed(tid){return}
        self.broadcast(m,Some(from));self.stats.relayed_templates+=1;
    }
    /// Records that peer `id` delivered something of kind `u` first (inbound eviction protects the latest deliverers of
    /// each kind, net::Net::establish), and counts it (`Stats::credits`). A peer already gone earns nothing.
    fn credit(&mut self,id:PeerId,u:Useful){if let Some(p)=self.peers.get(&id){p.credit(u);self.stats.credits[u as usize]+=1;}}
    fn punish_template(&mut self,from:PeerId,why:&str)->bool{
        self.stats.template_penalties+=1;self.log(&format!("invalid template from Draw(h,0) ({why}): closing its peer"));
        self.disc_closed_by_us(from);if let Some(p)=self.peers.get(&from){p.close();}
        false
    }
    // ---------------- sync sessions (network.md D38; D-REORG-REDB re-fetch) ----------------
    /// Locator (network.md:8, <= 32 hashes, highest first): the head and the 9 blocks below it, then
    /// doubling steps, the canonical block at depth R_keep (so a peer whose chain forks within R_keep
    /// always answers from a point within the window) and genesis.
    pub fn locator(&self,tip:Option<[u8;32]>)->Vec<[u8;32]>{
        let mut out:Vec<[u8;32]>=tip.into_iter().collect();
        // C-1 S5: a height below the window is read from the canonical index (O(log H) of them); one that cannot
        // be read is left out and logged (the index is then marked damaged, and the node's next eviction stops it).
        out.extend(Self::locator_heights(self.chain.head().height,self.chain.r_keep).into_iter().filter_map(|h|self.canon_hash(h).unwrap_or_else(|e|{self.log(&format!("locator height {h}: {e}"));None})));
        out.truncate(net::LOCATOR_MAX);out
    }
    /// The canonical heights of a locator built at head height `head`, highest first.
    fn locator_heights(head:u64,r_keep:u64)->Vec<u64>{
        let mut hs=Vec::new();let (mut h,mut step)=(head,1u64);
        while hs.len()<net::LOCATOR_MAX-3{hs.push(h);if h==0{break}if hs.len()>=10{step=step.saturating_mul(2);}h=h.saturating_sub(step);}
        hs.push(head.saturating_sub(r_keep));hs.push(0);hs.sort_unstable_by(|a,b|b.cmp(a));hs.dedup();hs
    }
    /// PR4 (I-5): whether a block on `parent` from the session's peer is on the session's branch: its parent is the
    /// last block the session delivered, a block it delivered that is not canonical (protected), or a canonical block
    /// of the locator the round asked from, where the peer's answer starts (network.md D38). Only these draw on the
    /// session's deep-rebuild allowance.
    fn on_session_branch(&self,s:&Session,parent:&[u8;32])->bool{
        s.tip==Some(*parent)||self.chain.protect.contains(parent)||
            (self.chain.is_canonical(parent)&&self.chain.get(parent).is_some_and(|e|Self::locator_heights(s.loc_head,self.chain.r_keep).contains(&e.height)))
    }
    fn protect(&mut self,hash:[u8;32]){if self.chain.protect.len()<self.chain.protect_max(){self.chain.protect.insert(hash);}}
    /// Whether a peer's announced Status calls for a session: its head is not stored here and its
    /// work is at least ours (an equal-work head may still win D133's tie-break at the fork point).
    fn wants(&self,st:&PeerStatus)->bool{!st.stale&&!self.chain.blocks.contains_key(&st.head)&&st.work>=self.chain.head().total_work&&st.head!=self.chain.head}
    fn start_session(&mut self,peer:PeerId,now:u128){
        let Some(p)=self.peers.get(&peer) else{return};
        p.send(&Msg::GetFrom{locator:self.locator(None),count:SYNC_COUNT});
        self.session=Some(Session{peer,tip:None,asked_ms:now,last_ms:now,got:0,got_round:0,rounds:1,best_h:0,rebuilds:0,loc_head:self.chain.head().height});self.sync_ask_ms=now;
        self.stats.sync_sessions+=1;self.stats.sync_rounds+=1;
    }
    /// Ends the session; its blocks lose their protection (adopted ones are canonical by now). A session
    /// that did not reach the peer's head sets that claim aside until the peer's next Hello.
    fn end_session(&mut self,reached:bool){
        if let Some(s)=self.session.take(){if !reached{if let Some(st)=self.peer_status.get_mut(&s.peer){st.stale=true;}}}
        self.chain.protect.clear();
    }
    /// Session progress on every tick: done when the peer's announced head is stored or our work exceeds
    /// its claim; the next round (locator from the last delivered block) once a round has gone quiet;
    /// given up when the peer stays silent or the protected branch reaches its bound. Without a session,
    /// peers announcing at least our work with an unknown head are asked in turn (round-robin, so a peer
    /// that lies in its Status gets only its share), at most one start per SESSION_GAP_MS.
    fn sync_tick(&mut self,now:u128){
        if let Some(s)=self.session.clone(){
            let claim=self.peer_status.get(&s.peer).cloned();
            let reached=claim.as_ref().map_or(true,|c|self.chain.blocks.contains_key(&c.head)||self.chain.head().total_work>c.work);
            if !self.peers.contains_key(&s.peer)||reached{self.end_session(reached);return}
            if self.chain.protect.len()>=self.chain.protect_max(){self.log("sync session: protected branch bound reached");self.end_session(false);return}
            if now.saturating_sub(s.last_ms)>=SESSION_STALL_MS{self.log("sync session: peer silent; claim set aside");self.end_session(false);return}
            if s.got_round>0&&now.saturating_sub(s.last_ms)>=ROUND_IDLE_MS{
                let loc=self.locator(s.tip);
                if let Some(p)=self.peers.get(&s.peer){p.send(&Msg::GetFrom{locator:loc,count:SYNC_COUNT});}
                let loc_head=self.chain.head().height;
                if let Some(x)=&mut self.session{x.got_round=0;x.asked_ms=now;x.last_ms=now;x.rounds+=1;x.loc_head=loc_head;}self.stats.sync_rounds+=1;
            }
            return
        }
        if now.saturating_sub(self.sync_ask_ms)<SESSION_GAP_MS{return}
        let mut ahead:Vec<PeerId>=self.peer_status.iter().filter(|(_,st)|self.wants(st)).map(|(i,_)|*i).collect();ahead.sort();
        if ahead.is_empty(){return}
        let id=ahead[self.sync_turn%ahead.len()];self.sync_turn=self.sync_turn.wrapping_add(1);
        self.start_session(id,now);
    }
    fn tick(&mut self){
        self.sync_tick(now_ms());self.disc_tick(self.disc_now());
        // Retry future blocks.
        if !self.future.is_empty(){let f=std::mem::take(&mut self.future);for b in f{let h=b.hash();if self.import(b,None).is_ok()&&self.own_future.remove(&h){self.stats.mined+=1;}}
            let waiting:HashSet<[u8;32]>=self.future.iter().map(|b|b.hash()).collect();self.own_future.retain(|h|waiting.contains(h));}
        let now=now_ms();
        let Some(pr)=&self.prod else{return};
        let parent=pr.parent;let a=pr.attempt;
        match &pr.mode{
            Mode::Propose{at_s} if now_s()>=*at_s=>{self.begin_attempt(0);}
            Mode::Wait{deadline_ms} if now>*deadline_ms=>{
                // FB is sticky for (parent, h, 0).
                if let Some(p)=self.full_entry(&parent){self.lock_fallback(&p,0);}
            }
            Mode::Locked(_)|Mode::Idle|Mode::Wait{..}|Mode::Propose{..}=>{
                let p=self.chain.get(&parent).unwrap();
                if let Ok(w)=crate::timing::AttemptWindow::new(p.ts,a,self.chain.spec.cp().D_att){
                    if w.advance_due(now_s(),u64::from(self.chain.spec.local().gamma)){self.log(&format!("attempt {} expired; advancing",a));self.begin_attempt(a+1);}
                }
            }
        }
    }
    fn on_miner(&mut self,m:MinerEvent){
        if self.cfg.miner.is_none(){return}
        let Some(pr)=&mut self.prod else{return};
        let Mode::Locked(l)=&pr.mode else{return};
        match m{
            MinerEvent::Share{tid,nonce} if tid==l.tid=>{pr.shares.insert(nonce);self.stats.shares_seen+=1;self.broadcast(&Msg::Share{tid,nonce},None);}
            MinerEvent::Exhausted{tid,hashes} if tid==l.tid=>{self.stats.hashes=self.stats.hashes.saturating_add(hashes);pr.miner=None;self.log("range exhausted; idle until next template");}
            MinerEvent::Solution{tid,nonce} if tid==l.tid&&!pr.solved=>{
                pr.solved=true;
                let s_max=usize::from(self.chain.spec.cp().S_max);
                let shares:Vec<u64>=pr.shares.iter().copied().filter(|n|*n!=nonce).take(s_max).collect();
                let mut h=l.header.clone();h.nonce=nonce;h.shares=shares;reseal(&mut h);
                let key=self.cfg.miner.as_ref().unwrap();
                h.winner_sig=key.sign_hash(&B256::from(hashes::win_msg(&tid,nonce,&share_root(&h))));reseal(&mut h);
                let b=Block{header:h,body:l.body.clone()};
                let hash=b.hash();
                match self.import(b,None){Ok(_)=>{self.stats.mined+=1;}
                    Err(Reject::Future)=>{self.own_future.insert(hash);self.log("own block waits for its timestamp");}
                    Err(e)=>self.log(&format!("own block rejected: {e:?}"))}
            }
            _=>{}
        }
    }
    fn on_msg(&mut self,from:PeerId,m:Msg){
        // The feeler (VA-SPEC §1a.7) is not in `peers`: only its first Hello is read, then it closes.
        if self.disc.is_feeler(from){if matches!(m,Msg::Hello{..}){self.disc_feeler_hello(from,m,self.disc_now());}return}
        // Reader events may already be queued when an incompatible Hello closes a socket.
        // Neither those frames nor a later Hello may revive the removed connection.
        if !self.peers.contains_key(&from){return}
        if !matches!(&m,Msg::Hello{..})&&!self.peer_status.contains_key(&from){return}
        match m{
            Msg::Hello{chain_id,genesis,head,height,work,node,protocol,compatibility,listen_port,nonce}=>{
                // VA-SPEC §2, §1a.1: listenPort and nonce are read from a connection's first Hello only, and a first
                // Hello carrying this node's own nonce is a self-connection, closed before any Status is kept.
                let first=!self.peer_status.contains_key(&from);let now=self.disc_now();
                if first&&nonce!=0&&nonce==self.disc.wire_nonce(){self.disc_self_connection(from);return}
                // VA-SPEC §2: the one acceptance predicate (also net::Net::eligible's). Another network forgets a dialed
                // address; another wire range or consensus identity holds it.
                if let Err(why)=crate::identity::accepts(&self.consensus,chain_id,&genesis,protocol,&compatibility){
                    use crate::identity::Mismatch;
                    let (k,text)=match why{Mismatch::Network=>(0,"another network (chain id or genesis)".to_string()),
                        Mismatch::Wire(v)=>(1,format!("wire protocol {v} < {}",crate::identity::PROTOCOL_VERSION_MIN)),
                        Mismatch::Consensus=>(2,crate::identity::explain(&self.consensus,&compatibility))};
                    self.log(&format!("peer {node}: {text}; closing"));
                    self.stats.incompatible[k]+=1;self.stats.last_incompatible=format!("peer {node}: {text}").chars().take(INCOMPATIBLE_REASON_MAX).collect();
                    self.disc_incompatible(from,why==Mismatch::Network,now);
                    self.peer_status.remove(&from);
                    if self.session.as_ref().is_some_and(|s|s.peer==from){self.end_session(false);}
                    if let Some(p)=self.peers.remove(&from){p.close();self.allow_redial(p.origin);}
                    return;
                }
                // VA-SPEC §1a.2: what a first compatible Hello teaches, and the GetAddr of a connection this node
                // dialed, sent before a session's GetFrom so that its answer is not queued behind the blocks.
                if first{self.disc_first_hello(from,listen_port,nonce,now);}
                // Only a peer's first Hello may start a session at once; later Status updates wait for
                // sync_tick's round-robin, so a peer re-announcing a false claim cannot hold the session
                // slot. And only SESSION_GAP_MS after the last start (VA-SPEC §4): a peer that ends its session by
                // disconnecting gets no new one (and no new SESSION_REBUILD_MAX allowance) at its next first Hello
                // sooner than the round-robin would give it (sync_tick). A claim set aside stays aside until the
                // announced head changes.
                let prev=self.peer_status.get(&from);
                let stale=prev.map_or(false,|p|p.stale&&p.head==head);
                let st=PeerStatus{work:U256::from_be_bytes(work),head,height,stale};
                let start=first&&self.session.is_none()&&height>0&&self.wants(&st)&&now_ms().saturating_sub(self.sync_ask_ms)>=SESSION_GAP_MS;
                self.peer_status.insert(from,st);
                if start{self.start_session(from,now_ms());}
            }
            Msg::Block{header,body}=>{
                let Ok(h)=decode_header(&header) else{return};
                let Ok(b)=Body::decode(&body,self.chain.spec.cp().BODY_MAX as usize) else{return};
                let solicited=self.session.as_ref().map_or(false,|s|s.peer==from);
                let height=h.h;let b=Block{header:h,body:b};let hash=b.hash();
                let before=self.chain.head;let r=self.import_with(b,Some(from),solicited);let now=self.disc_now();
                // D25, D26 (VA-SPEC §1a.7): a peer's block that moved the head; first delivery (new, within 2 of the
                // head) makes the peer useful. A Duplicate earns nothing, also inside the peer's own sync session.
                if self.chain.head!=before{self.disc_head(from,now);}
                if discovery::first_delivery(r.is_ok(),height,self.chain.head().height){self.disc_useful(from,true,now);}
                // Progress: a block stored by this import, or a stored copy at a new height inside the
                // R_keep window (the honest common point is inside it). Anything else (junk, duplicates
                // below the frontier, blocks retention removed at once) does not refresh the session.
                if solicited&&self.chain.blocks.contains_key(&hash){
                    let window=height.saturating_add(self.chain.r_keep)>self.chain.head().height;
                    if let Some(s)=&mut self.session{
                        if matches!(r,Ok(_))||(height>s.best_h&&window){s.tip=Some(hash);s.best_h=s.best_h.max(height);s.got+=1;s.got_round+=1;s.last_ms=now_ms();}
                    }
                }
            }
            Msg::Template{shell,body}=>{
                let Ok(h)=decode_header(&shell) else{return};
                let Ok(b)=Body::decode(&body,self.chain.spec.cp().BODY_MAX as usize) else{return};
                let arrival=now_ms();
                self.note_template_arrival(&h,arrival);
                let equivocated=self.observe_template(&h);
                let waiting=matches!(&self.prod,Some(p) if p.parent==h.parent_hash&&p.attempt==0&&matches!(p.mode,Mode::Wait{deadline_ms} if arrival<=deadline_ms));
                if equivocated{
                    // Second template from the same proposer: evidence type 1, then sticky FB. A node already on
                    // that fallback keeps mining it: relayed copies of the other template arrive once per
                    // neighbour that locked it, and each would otherwise restart the miner from its range start.
                    if matches!(&self.prod,Some(p) if p.parent==h.parent_hash&&p.attempt==0&&!matches!(&p.mode,Mode::Locked(l) if l.fallback)){
                        let p=self.chain.get(&h.parent_hash).unwrap().clone();self.lock_fallback(&p,0);}
                }else if waiting{self.try_lock_signed(h,b,from);}
                else if !self.chain.blocks.contains_key(&h.parent_hash)||self.prod.as_ref().map(|p|p.parent!=h.parent_hash).unwrap_or(true){
                    // A copy identical to the buffered one (a relay of the same template) keeps the first receipt
                    // time, so the V3-05 arrival (and T_j's check at slot start) counts from the first copy.
                    let first=match self.templates.get(&h.parent_hash){Some((o,ob,t,_)) if o.encoded==h.encoded&&*ob==b=>*t,_=>arrival};
                    if self.templates.len()>TEMPLATES_MAX{self.templates.clear();}self.templates.insert(h.parent_hash,(h,b,first,from));
                }
            }
            Msg::Share{tid,nonce}=>{
                let relay_max=SHARE_RELAY_PER_S_MAX*usize::from(self.chain.spec.cp().S_max);
                // V3-02: a share of a cached earlier template (not the one locked here) is kept for late credit, and a
                // new one is relayed as a live share is: its credit then does not depend on which peers its finder
                // reaches. Within the cache's cap, so a template's shares are relayed at most SHARE_RELAY_PER_S_MAX·S_max
                // times over its life, locked and cached together. Only the v3 program set caches.
                let locked=matches!(&self.prod,Some(Production{mode:Mode::Locked(l),..}) if l.tid==tid);
                if !locked{
                    match self.late_cache.take(&tid,nonce,relay_max){
                        Some(true)=>{self.credit(from,Useful::Share);self.broadcast(&Msg::Share{tid,nonce},Some(from));self.stats.relayed_shares+=1;return}
                        Some(false)=>return,
                        None=>{}
                    }
                }
                let relay={let Some(pr)=&mut self.prod else{return};let Mode::Locked(l)=&pr.mode else{return};
                    if tid!=l.tid||(nonce as u128)>=l.ctx.assignment.nonce_max()||pr.shares.contains(&nonce){return}
                    if !le_target(&pow_hash(&tid,nonce),&l.ctx.t_share){return}
                    pr.shares.insert(nonce);pr.shares.len()<=relay_max};
                self.stats.shares_seen+=1;self.stats.peer_shares+=1;self.credit(from,Useful::Share);
                // Gossip (network.md:15): a valid new share for the locked template goes to every other
                // peer, never back to its sender; a duplicate or an invalid one was dropped above. Without it
                // a share reaches only its miner's neighbours, and a winner elsewhere cannot credit it.
                if relay{self.broadcast(&Msg::Share{tid,nonce},Some(from));self.stats.relayed_shares+=1;}
            }
            Msg::GetBlock{hash}=>{if let Some(m)=self.block_msg(&hash){if let Some(p)=self.peers.get(&from){p.send(&m);}}}
            Msg::GetFrom{locator,count}=>{
                let Some(p)=self.peers.get(&from) else{return};
                // The highest locator hash on our canonical chain (genesis always is); answers stop when
                // the peer's queue is half full (offer) and the peer asks again from its last block.
                let Some(start)=self.locator_start(&locator) else{return};
                self.serve_range(p,start,count.min(SYNC_COUNT));
            }
            Msg::GetRange{start,count}=>{
                let Some(p)=self.peers.get(&from) else{return};
                // Responses stop when the peer's queue is half full (offer); the peer asks again. Genesis is not served.
                if start==0{return}
                self.serve_range(p,start,count.min(512));
            }
            Msg::Tx{raw}=>self.relay_tx(raw,from),
            // Peer discovery (VA-SPEC §1a.2, §1a.5): answers to inbound peers, and what this node asked for.
            Msg::GetAddr=>self.disc_getaddr(from,self.disc_now()),
            Msg::Addr{addrs}=>self.disc_addr(from,addrs,self.disc_now()),
        }
    }
}
impl Node{
    /// VA-SPEC §2: the wire range and consensus identity this node peers on (pocol_status, pocol_getParams).
    pub fn identity_json(&self)->serde_json::Map<String,serde_json::Value>{
        use crate::identity as id;
        let j=serde_json::json!({"protocolVersionMin":id::PROTOCOL_VERSION_MIN,"consensusRulesVersion":id::CONSENSUS_RULES_VERSION,
            "maxForkVersion":pocol_lp4_execution::profile::MAX_FORK_VERSION,"consensusIdentity":self.consensus.hex(),"consensusPinned":id::PINNED});
        let serde_json::Value::Object(m)=j else{unreachable!()};m
    }
    /// VA-SPEC §2.5: what refused peers differed in (pocol_status and wallet status), so a lagging node is told why.
    pub fn incompatible_json(&self)->serde_json::Map<String,serde_json::Value>{
        let [network,wire,consensus]=self.stats.incompatible;let last=&self.stats.last_incompatible;
        let j=serde_json::json!({"incompatiblePeers":{"network":network,"wire":wire,"consensus":consensus},"legacyHellos":self.stats.legacy_hellos,
            "lastIncompatibleReason":if last.is_empty(){serde_json::Value::Null}else{serde_json::json!(last)}});
        let serde_json::Value::Object(m)=j else{unreachable!()};m
    }
    /// GetFrom's start (C-1 S5, PR7): one past the first locator hash that is a canonical block held in memory other
    /// than genesis (an honest locator is highest first, so a peer that is behind matches on its first hash, as
    /// before); else one past the first hash found below the window in the canonical index, in locator order; else
    /// one past genesis. Every hash not held in memory is looked up, one index read each, at most the wire's
    /// net::LOCATOR_MAX (32): a peer on its own fork leads its locator with blocks this node never held (the 10 dense
    /// hashes and the first doubling steps above its fork point), so a cap below the locator's length skipped the
    /// shared hashes after them and served a peer forked more than 15 blocks deep from genesis, which it holds
    /// already, and it never synced (review 2026-11, finding 1; a hash lookup in memory before C-1 S5).
    fn locator_start(&self,locator:&[[u8;32]])->Option<u64>{
        let g=*self.chain.canonical.genesis();
        if let Some(e)=locator.iter().filter(|h|**h!=g&&self.chain.is_canonical(h)).find_map(|h|self.chain.get(h)){return Some(e.height+1)}
        if self.chain.canonical.base()>1{
            for h in locator.iter().filter(|h|**h!=g&&!self.chain.blocks.contains_key(*h)).take(net::LOCATOR_MAX){
                match self.canon_height(h){Ok(Some(x))=>return Some(x+1),Ok(None)=>{},Err(e)=>{self.log(&format!("GetFrom locator: {e}"));return None}}
            }
        }
        locator.contains(&g).then_some(1)
    }
    /// The canonical blocks of heights start..start+count to peer `p`, until its queue is half full (`offer`) or a
    /// height is not held. Below the window from one index read (`canon.range_each`, a row read only when the one
    /// before it was offered) and the journal, each record checked to hash to its row's block; then the window's, as
    /// before. Only the index is blamed for what is the index's: a
    /// row naming no record, or a record that reads but is not the row's block, marks it damaged; a journal that
    /// cannot be read is logged and serving stops (as `block_msg`, `indexed_view` and `canonical_entry` do), since
    /// marking the index for it stopped the node at its next eviction and made the next open drop every checkpoint
    /// based above height 1 and replay from genesis (review 2026-11, findings 2 and 5).
    fn serve_range(&self,p:&Peer,start:u64,count:u64){
        let (end,base)=(start.saturating_add(count),self.chain.canonical.base());let mut hgt=start;
        if hgt<base{
            let mut stopped=false;
            let read=self.canon.range_each(hgt,end.min(base)-hgt,|h,r|{
                stopped=true;if h!=hgt{return false}
                let Some(o)=r.body_off else{self.log(&self.canon.mark_damaged(&format!("the row at height {h} names no journal record")));return false};
                let (hb,bb,_)=match self.journal.read_at(o){Ok(x)=>x,Err(e)=>{self.log(&format!("serving height {h}: journal read at {o}: {e:?}"));return false}};
                if decode_header(&hb).map(|x|block_hash(&x)).ok()!=Some(r.hash){self.log(&self.canon.mark_damaged(&format!("the row at height {h} does not name its journal record")));return false}
                if !p.offer(&Msg::Block{header:hb,body:bb}){return false}hgt+=1;stopped=false;true});
            if let Err(e)=read{self.log(&format!("serving height {hgt}: {e}"));return}
            if stopped||hgt<end.min(base){return}
        }
        for h in hgt..end{match self.chain.canonical_at(h).and_then(|e|self.block_msg(&e.hash)){Some(m)=>{if !p.offer(&m){break}}None=>break}}
    }
    /// The Block message for a stored block (genesis excluded); an offloaded non-canonical block is
    /// served from its journal record, byte-equal to what was imported (VA-SPEC §4).
    pub fn block_msg(&self,hash:&[u8;32])->Option<Msg>{
        let e=self.chain.get(hash).filter(|e|e.height>0)?;
        if !e.offloaded&&!e.slim{return Some(Msg::Block{header:e.header.encoded.clone(),body:e.body.encode()})}
        match self.journal.read_at(e.body_off?){Ok((hb,bb,_))=>Some(Msg::Block{header:hb,body:bb}),Err(err)=>{self.log(&format!("journal read for {}: {err:?}",&hex(hash)[..14]));None}}
    }
    /// The canonical hash at height `h` (C-1 S5): the window's or genesis's, else the canonical index's below the
    /// window. Ok(None) above the head. Err: the index could not be read (it is then marked damaged).
    pub fn canon_hash(&self,h:u64)->Result<Option<[u8;32]>,String>{
        if h==0||h>=self.chain.canonical.base(){return Ok(self.chain.canonical.get(h as usize).copied())}
        Ok(Some(self.indexed(h)?.hash))
    }
    /// The height of `hash` if it is canonical: held in memory (the window or genesis), else found in the canonical
    /// index below the window.
    pub fn canon_height(&self,hash:&[u8;32])->Result<Option<u64>,String>{
        if let Some(e)=self.chain.blocks.get(hash){return Ok(self.chain.is_canonical(hash).then_some(e.height))}
        let base=self.chain.canonical.base();if base<2{return Ok(None)}
        Ok(self.canon.height_of(hash)?.filter(|h|*h<base))
    }
    /// A block with its body, encoded header and (in memory always, below the window when asked) its receipts, by
    /// hash: one held in memory (any, as `block_view`), else a canonical one below the window. Never inserted into
    /// the chain. Err: its history could not be read (logged; the store is marked damaged and rebuilt at the next open).
    pub fn view_by_hash(&self,hash:&[u8;32],want:Want)->Result<Option<std::borrow::Cow<'_,Entry>>,String>{
        if let Some(e)=self.chain.get(hash){return self.view(e).map(Some).ok_or_else(||HISTORY_UNAVAILABLE.to_string())}
        match self.canon_height(hash)?{Some(h)=>self.indexed_view(h,want).map(|e|Some(std::borrow::Cow::Owned(e))),None=>Ok(None)}
    }
    /// `view_by_hash` of the canonical block at height `h` (Ok(None) above the head).
    pub fn canonical_view(&self,h:u64,want:Want)->Result<Option<std::borrow::Cow<'_,Entry>>,String>{
        if h==0||h>=self.chain.canonical.base(){return match self.chain.canonical_at(h){Some(e)=>self.view(e).map(Some).ok_or_else(||HISTORY_UNAVAILABLE.to_string()),None=>Ok(None)}}
        self.indexed_view(h,want).map(|e|Some(std::borrow::Cow::Owned(e)))
    }
    /// The canonical index's row at height `h` below the window: one must be there (a row missing marks the index damaged).
    fn indexed(&self,h:u64)->Result<crate::checkpoint::Rec,String>{self.canon.at(h)?.ok_or_else(||self.canon.mark_damaged(&format!("no row at height {h} below the window")))}
    /// A canonical block below the window as RPC reads it: one index read and one journal read (header, body and
    /// ParentResult), receipts from history.redb only when asked, no nonce ranges (`checkpoint::entry_of`, so the
    /// answer is the one the entry gave while it was held). The record must hash to the row's block.
    fn indexed_view(&self,h:u64,want:Want)->Result<Entry,String>{
        let r=self.indexed(h)?;let off=r.body_off.ok_or_else(||self.canon.mark_damaged(&format!("the row at height {h} names no journal record")))?;
        let payload=self.journal.payload_at(off).map_err(|e|{let m=format!("journal read at {off}: {e:?}");self.log(&m);m})?;
        let mut q=r.clone();q.flags&=!(crate::checkpoint::flags::OFFLOADED|crate::checkpoint::flags::SLIM);
        let why=|m:&str|format!("the row at height {h}: {m}");
        let (mut e,_)=crate::checkpoint::entry_of(&q,&payload,|_|Ok(()),||Ok(Some(Vec::new())),||Ok(Some(Vec::new())),self.chain.spec.cp().BODY_MAX as usize,false,&why)
            .map_err(|m|self.canon.mark_damaged(&m))?;
        if e.height!=h{return Err(self.canon.mark_damaged(&format!("the row at height {h} names a block at height {}",e.height)))}
        if want.receipts{e.receipts=match self.history.receipts(&e.hash){Ok(Some(x))=>x,
            Ok(None)=>{let m=self.history.mark_damaged("receipts missing for a journaled block").0;self.log(&m);return Err(m)}
            Err(x)=>{self.log(&format!("history read for {}: {}",&hex(&e.hash)[..14],x.0));return Err(x.0)}};}
        Ok(e)
    }
    /// The canonical index (C-1 S5; tests and inspection).
    pub fn canon(&self)->&canon::Index{&self.canon}
    /// A stored block for RPC (eth_getBlockByHash): as stored when resident; an offloaded one (a side block,
    /// or since C-1 S1 canonical history below the resident window) is materialized from its journal record
    /// (encoded header, body, tx hashes) and history.redb (receipts), so every answer is the one it had
    /// while resident. None if it is not stored, or if its record cannot be read (logged; a damaged
    /// history store is marked, imports then stop the node and the next open rebuilds the store).
    pub fn block_view(&self,hash:&[u8;32])->Option<std::borrow::Cow<'_,Entry>>{self.view(self.chain.get(hash)?)}
    /// `block_view` of an entry the caller already holds.
    pub fn view<'a>(&'a self,e:&'a Entry)->Option<std::borrow::Cow<'a,Entry>>{
        if !e.offloaded&&!e.slim{return Some(std::borrow::Cow::Borrowed(e))}
        let hash=e.hash;
        let (hb,bb,rb)=self.journal.read_at(e.body_off?).map_err(|err|self.log(&format!("journal read for {}: {err:?}",&hex(&hash)[..14]))).ok()?;
        let mut v=e.meta_clone();
        if e.offloaded{let body=Body::decode(&bb,self.chain.spec.cp().BODY_MAX as usize).ok()?;v.tx_hashes=body.txs.iter().map(|t|revm::primitives::keccak256(t)).collect();v.body=body;}
        // C-1 S4: a slimmed entry's header fields and ParentResult come back from the same record.
        if e.slim{v.header=decode_header(&hb).ok()?;v.result=chain::decode_result(&rb,&hash).ok()?;v.slim=false;}
        v.header.encoded=hb;
        if !e.offloaded{return Some(std::borrow::Cow::Owned(v))}
        v.offloaded=false;
        // Every journaled block's receipts were stored before its record (I3) or by replay: a missing or
        // damaged record means the store is damaged. No partial answer is given; the next open rebuilds it.
        match self.history.receipts(&hash){Ok(Some(r))=>v.receipts=r,
            Ok(None)=>{let e=self.history.mark_damaged("receipts missing for a journaled block");self.log(&format!("history read for {}: {}",&hex(&hash)[..14],e.0));return None}
            Err(err)=>{self.log(&format!("history read for {}: {}",&hex(&hash)[..14],err.0));return None}}
        Some(std::borrow::Cow::Owned(v))
    }
    /// C-1 S3a: a stored block with its full state (materialized through the diffs, or rebuilt), for
    /// production on it. None if it cannot be had.
    fn full_entry(&mut self,h:&[u8;32])->Option<Entry>{
        if let Err(e)=self.chain.ensure_state_with(h,Some(&Src{journal:&self.journal,history:&self.history,canon:&self.canon})){if let Reject::Local(w)=&e{self.fatal(w)}self.log(&format!("state for {}: {e:?}",&hex(h)[..14]));return None}
        self.chain.get(h).cloned()
    }
    /// C-1 S4: a stored block with its slimmed fields restored (header, ParentResult, nonce ranges) from
    /// the journal and history.redb; itself when not slimmed. For inspection and tests.
    pub fn restored_entry(&self,h:&[u8;32])->Option<Entry>{
        let e=self.chain.get(h)?;self.chain.restored(e,Some(&Src{journal:&self.journal,history:&self.history,canon:&self.canon})).ok().map(|x|x.into_owned())
    }
    /// TrieRebuild of a stored block's state through the journal (offloaded bodies read back).
    pub fn rebuild_state(&mut self,hash:&[u8;32])->Result<(),Reject>{self.chain.ensure_state_with(hash,Some(&Src{journal:&self.journal,history:&self.history,canon:&self.canon}))}
    /// C-1 S4: restores a slimmed entry's fields in place (header, ParentResult, nonce ranges) from the
    /// journal and history.redb, for a walk that reads the chain's own entries (the evidence walk).
    pub fn unslim(&mut self,hash:&[u8;32])->Result<(),Reject>{self.chain.unslim(hash,&Src{journal:&self.journal,history:&self.history,canon:&self.canon})}
    /// A stored block's committed body (resident or from the journal).
    pub fn body_of(&self,hash:&[u8;32])->Result<Body,Reject>{let e=self.chain.get(hash).ok_or(Reject::UnknownParent)?;self.chain.body_of(e,Some(&self.journal))}
    /// The pending state as a whole state (the layer merged over its base; tests and inspection).
    pub fn pool_state(&self)->InMemoryDB{let base=self.pool.pending_base.whole();pocol_lp4_execution::view::Overlay::from_layer(&base,self.pool.pending.clone()).merge()}
    /// The pending state read in place (RPC "pending").
    pub fn pool_view(&self)->pocol_lp4_execution::view::LayerView<'_,chain::StateHandle>{pocol_lp4_execution::view::LayerView{base:&self.pool.pending_base,layer:&self.pool.pending}}
    /// Accounts the pool's layer has written (A13).
    pub fn pool_written(&self)->usize{self.pool.pending.written()}
    /// The pool meter's (user slots, user accounts, system slots).
    pub fn pool_live(&self)->(u64,u64,u64){self.pool.meter.live()}
    /// The pending layer's account and slot entries (A13 tests).
    pub fn pool_layer_entries(&self)->(usize,usize){self.pool.pending.entries()}
    /// D-7: the pool's counters recomputed from what it holds (tests). `gas` and `bytes` are the sums over the
    /// admitted transactions and `backlog_bytes` over the backlog's; each admitted transaction is held once
    /// under its own hash; the bounds hold. The backlog may hold one abandoned body past POOL_MAX_BYTES while
    /// it is revalidated (`pool_step_until` expands one at a time).
    pub fn pool_audit(&self)->Result<(),String>{
        let profile=self.chain.spec.profile();let (mut gas,mut bytes,mut seen)=(0u64,0usize,HashSet::new());
        if self.pool.meta.len()!=self.pool.txs.len(){return Err(format!("{} meta entries for {} pooled transactions",self.pool.meta.len(),self.pool.txs.len()))}
        let mut held:HashMap<Address,u64>=HashMap::new();
        // N4: and each sender and nonce is held once (pooled or parked), within the replacement limits.
        let mut nonces=HashSet::new();
        if self.pool.replace_relays>POOL_REPLACE_RELAY_MAX{return Err(format!("{} replacements relayed since the last head, past POOL_REPLACE_RELAY_MAX",self.pool.replace_relays))}
        let mut repl:HashMap<Address,u32>=HashMap::new();
        for ((h,raw),m) in self.pool.txs.iter().zip(&self.pool.meta){
            let Meta{sender,gas:g,exempt,..}=m;
            let env=wire::decode_for_profile(raw,profile).map_err(|e|format!("a pooled transaction does not decode: {e:?}"))?;
            if env.hash()!=*h{return Err(format!("pooled under {h}, which is not its hash"))}
            if !seen.insert(*h){return Err(format!("{h} is pooled twice"))}
            // N4 second review: its charge is its declared gas, or after replacements at least it.
            let d=env.tx_env().gas_limit;
            if env.tx_env().caller!=*sender||*g<d||(m.repl==0&&*g!=d){return Err(format!("{h}: meta ({sender}, {g}, replaced {}) is not its sender and gas {d}",m.repl))}
            // N4: the fields the fee market reads from its bytes are the decoder's.
            let f=TxFields::of(&env.tx_env());if tx_fields(raw)!=Some(f){return Err(format!("{h}: its bytes read as {:?}, not its decoded {f:?}",tx_fields(raw)))}
            if m.repl>POOL_REPLACE_SLOT_MAX||!nonces.insert((*sender,f.nonce)){return Err(format!("{h}: replaced {} times, or its sender and nonce are pooled twice",m.repl))}
            *repl.entry(*sender).or_default()+=u32::from(m.repl);
            if !exempt{let s=held.entry(*sender).or_default();*s=s.checked_add(*g).ok_or("sender gas overflows")?;
                if *s>self.pool.sender_gas_max{return Err(format!("{sender} holds {s} declared gas, past the sender quota {}",self.pool.sender_gas_max))}}
            gas=gas.checked_add(*g).ok_or("pooled gas overflows")?;bytes+=raw.len();
        }
        let backlog:usize=self.pool.backlog.iter().map(|i|match i{PoolItem::Tx(r,_)|PoolItem::Fresh(r)=>r.len(),PoolItem::Block(_)=>0}).sum();
        // PR6 P2: the parked set is bounded, counted, per sender a run of consecutive nonces sharing its run key (in
        // arrival order), within the sender quota with its pooled ones, and holds nothing pooled.
        let parked:usize=self.pool.parked.iter().map(|x|x.raw.len()).sum();let mut runs:HashMap<Address,((u64,u64),u64,u64)>=HashMap::new();
        if self.pool.parked.len()>self.pool.parked_max||self.pool.parked_max>POOL_PARKED_MAX{return Err(format!("{} parked, past the bound {}",self.pool.parked.len(),self.pool.parked_max))}
        if parked!=self.pool.parked_bytes{return Err(format!("parked bytes {} differ from the contents: {parked}",self.pool.parked_bytes))}
        for x in &self.pool.parked{
            let g=if matches!(x.relay,Relay::Offer|Relay::Returned){0}else{x.gas};
            match runs.get_mut(&x.sender){
                None=>{runs.insert(x.sender,(x.run,x.nonce,g));}
                Some((run,last,h))=>{if *run!=x.run||x.nonce!=last.saturating_add(1){return Err(format!("{}'s parked transactions are not one run of consecutive nonces",x.sender))}*last=x.nonce;*h=h.saturating_add(g);}}
            if seen.contains(&x.hash){return Err(format!("{} is both pooled and parked",x.hash))}
            if x.ahead>self.pool.txs.len() as u64{return Err(format!("parked {} arrived behind {} pooled transactions, more than are pooled",x.hash,x.ahead))}
            let env=wire::decode_for_profile(&x.raw,profile).map_err(|e|format!("a parked transaction does not decode: {e:?}"))?;
            if (env.hash(),env.tx_env().caller,env.tx_env().gas_limit,env.tx_env().nonce)!=(x.hash,x.sender,x.gas,Some(x.nonce)){return Err(format!("parked {} is not its hash, sender, gas and nonce",x.hash))}
            if tx_fields(&x.raw)!=Some(TxFields::of(&env.tx_env()))||x.repl>POOL_REPLACE_SLOT_MAX||!nonces.insert((x.sender,x.nonce)){return Err(format!("parked {}: its bytes read other fields, replaced {} times, or its sender and nonce held twice",x.hash,x.repl))}
            *repl.entry(x.sender).or_default()+=u32::from(x.repl);
        }
        for (s,(_,_,g)) in &runs{
            let all=held.get(s).copied().unwrap_or(0).saturating_add(*g);
            if *g>0&&all>self.pool.sender_gas_max{return Err(format!("{s} holds {all} declared gas pooled and parked, past the sender quota {}",self.pool.sender_gas_max))}
        }
        if let Some((s,n))=repl.iter().find(|(_,n)|**n>POOL_REPLACE_SENDER_MAX){return Err(format!("{s}'s entries were replaced {n} times since the last head, past POOL_REPLACE_SENDER_MAX"))}
        // N4: `gas` is the pooled entries' charged gas plus `ghost_gas` (evicted, or a returned one replaced by less,
        // since the last head), within the bound in effect (at most POOL_MAX_TXS x GAS_LIMIT, A13's layer term).
        let gas=gas.checked_add(self.pool.ghost_gas).ok_or("pooled gas overflows")?;
        if (gas,bytes,backlog)!=(self.pool.gas,self.pool.bytes,self.pool.backlog_bytes){
            return Err(format!("counters gas {}, bytes {}, backlog bytes {} differ from the contents: {gas}, {bytes}, {backlog}",self.pool.gas,self.pool.bytes,self.pool.backlog_bytes))}
        if gas>self.pool.gas_max{return Err(format!("pooled gas {gas} is past the bound {}",self.pool.gas_max))}
        let term=(POOL_MAX_TXS as u64).saturating_mul(self.chain.spec.cp().GAS_LIMIT);
        if self.pool.gas_max>term{return Err(format!("the pool's gas bound {} is past POOL_MAX_TXS x GAS_LIMIT {term}, the pending layer A13 counts",self.pool.gas_max))}
        if self.pool.txs.len()>POOL_MAX_TXS{return Err(format!("{} transactions pooled, past POOL_MAX_TXS",self.pool.txs.len()))}
        if bytes>POOL_MAX_BYTES||bytes+backlog+parked>POOL_MAX_BYTES+self.chain.spec.cp().BODY_MAX as usize{return Err(format!("{bytes} pooled, {backlog} backlog and {parked} parked bytes, past POOL_MAX_BYTES"))}
        Ok(())
    }
    pub fn peer_count(&self)->usize{self.peers.len()}
    pub fn status_json(&self)->serde_json::Value{
        let h=self.chain.head();
        // A13 (PR7 §6.2): the bound over the files as they are now.
        let mut limits=self.cfg.limits.clone();let disk=Some(crate::resources::DiskBytes::observe(&self.cfg.dir));limits.disk=disk;
        let mode=match &self.prod{None=>"none".to_string(),Some(p)=>match &p.mode{Mode::Idle=>"idle".into(),Mode::Wait{..}=>format!("wait a={}",p.attempt),Mode::Propose{at_s}=>format!("propose at {at_s}"),
            Mode::Locked(l)=>format!("{} h={} a={} shares={} mining={}",if l.fallback{"FB"}else{"PROP"},p.height,p.attempt,p.shares.len(),p.miner.is_some())}};
        let mut status=serde_json::json!({"name":self.cfg.name,"chainId":self.chain.spec.chain_id(),"genesisHash":hex(&self.chain.spec.genesis_hash()),"head":{"height":h.height,"hash":hex(&h.hash),"ts":h.ts,"signed":h.signed,"final":h.is_final,"totalWork":format!("{:#x}",h.total_work)},
            "peers":self.peers.values().map(|p|p.addr.clone()).collect::<Vec<_>>(),"production":mode,"miner":self.cfg.miner.as_ref().map(|k|format!("{:#x}",k.address())),
            "stateStore":self.cfg.limits.state_store,"pool":self.pool.txs.len(),"poolGas":self.pool.gas,"poolGasMax":self.pool.gas_max,"poolSenderGasMax":self.pool.sender_gas_max,"poolParked":self.pool.parked.len(),"poolParkedMax":self.pool.parked_max,"journalRecords":self.journal_records(),"blocksKnown":self.chain.blocks.len(),"invalidKnown":self.chain.invalid.len(),
            "admission":crate::admission_json(self.chain.spec.admission()),
            "a13":crate::resources::derive(self.chain.spec.cp(),self.chain.spec.local().k_hist,self.chain.spec.local().side_max,&limits,self.cfg.peers.len()).map(|b|serde_json::json!({
                "boundBytes":b.total.to_string(),"budgetBytes":self.cfg.limits.mem_budget,"conditions":crate::resources::CONDITIONS.iter().map(|(id,d)|serde_json::json!({"id":id,"detail":d})).collect::<Vec<_>>()})),
            "stats":{"imported":self.stats.imported,"mined":self.stats.mined,"signedTemplates":self.stats.signed_templates,"fallbackTemplates":self.stats.fallback_templates,"sharesSeen":self.stats.shares_seen,"hashesInExhaustedRanges":self.stats.hashes,"reorgs":self.stats.reorgs,
                "syncSessions":self.stats.sync_sessions,"syncRounds":self.stats.sync_rounds,"refetched":self.stats.refetched,"restored":self.stats.restored,"replaySkipped":self.stats.replay_skipped,"templatePenalties":self.stats.template_penalties,"poolSlices":self.stats.pool_slices,"poolSliceMaxMs":self.stats.pool_slice_max_ms,"reinsertedOffered":self.stats.reinserted_offered,"reinsertedUnoffered":self.stats.reinserted_unoffered,"poolBacklog":self.pool.backlog.len(),"historyRefilled":self.stats.history_refilled,"historyRebuilt":self.history.rebuilt,"historyDamaged":self.history.is_damaged()},
            "open":serde_json::to_value(&self.stats.open).unwrap_or_default(),"retention":{"rKeep":self.chain.r_keep,"sideCap":self.chain.side_cap,"side":self.chain.side_counts().0,"protected":self.chain.protect.len(),"evicted":self.chain.evicted_total}});
        // VA-SPEC §2: the wire range and the consensus identity peers are checked against; the build digest
        // (compatibilitySha256) is informational install-integrity status.
        status["protocolVersion"]=serde_json::json!(crate::identity::PROTOCOL_VERSION);
        status["compatibilitySha256"]=serde_json::json!(crate::identity::COMPATIBILITY_HEX);
        for (k,v) in self.identity_json(){status[k]=v;}
        for (k,v) in self.incompatible_json(){status[k]=v;}
        status["compatiblePeers"]=serde_json::json!(self.peer_status.len());
        // V3-28 H1: computed from this build and the admission this node booted with (readiness.rs).
        crate::readiness::annotate(&mut status,&crate::readiness::of_admitted(self.chain.spec.admission()));
        // Gossip and inbound grouping (net::Limits::per_group_max).
        status["stats"]["relayedTemplates"]=serde_json::json!(self.stats.relayed_templates);
        // PR6: the builder (gas used) and the pool's sender quota and parking.
        status["stats"]["templateGasFallbacks"]=serde_json::json!(self.stats.template_gas_fallbacks);
        status["stats"]["templateGasRecoveries"]=serde_json::json!(self.stats.template_gas_recoveries);
        status["stats"]["templateAppended"]=serde_json::json!(self.stats.template_appended);
        status["stats"]["templateBuilds"]=serde_json::json!(self.stats.template_builds);
        status["stats"]["senderQuotaRefused"]=serde_json::json!(self.stats.sender_quota_refused);
        status["stats"]["parkedEvicted"]=serde_json::json!(self.stats.parked_evicted);status["stats"]["parkedRelayed"]=serde_json::json!(self.stats.parked_relayed);
        // N4 (V3-20): the fee market's pool (replacement and eviction).
        status["stats"]["poolReplaced"]=serde_json::json!(self.stats.pool_replaced);status["stats"]["replaceRefused"]=serde_json::json!(self.stats.replace_refused);
        status["stats"]["replaceRelayed"]=serde_json::json!(self.stats.replace_relayed);status["stats"]["replaceUnrelayed"]=serde_json::json!(self.stats.replace_unrelayed);
        status["stats"]["poolEvicted"]=serde_json::json!(self.stats.pool_evicted);status["stats"]["evictRefused"]=serde_json::json!(self.stats.evict_refused);
        status["stats"]["parkedFeeEvicted"]=serde_json::json!(self.stats.parked_fee_evicted);status["stats"]["overdraftRefused"]=serde_json::json!(self.stats.overdraft_refused);
        status["stats"]["relayedShares"]=serde_json::json!(self.stats.relayed_shares);
        status["stats"]["peerShares"]=serde_json::json!(self.stats.peer_shares);
        status["stats"]["inboundRefusedByGroup"]=serde_json::json!(self.net.counters.refused_group.load(std::sync::atomic::Ordering::SeqCst));
        // Inbound eviction (net::Net::establish): the established and physical bounds, the evictions and the newcomers
        // refused at their Hello because no peer could be evicted or the eviction budget was spent.
        {let c=&self.net.counters;let n=|a:&std::sync::atomic::AtomicUsize|serde_json::json!(a.load(std::sync::atomic::Ordering::SeqCst));
            status["stats"]["inboundEvicted"]=n(&c.evicted);status["stats"]["inboundEvictedInGroup"]=n(&c.evicted_in_group);
            status["stats"]["inboundRefusedFull"]=n(&c.refused_full);status["stats"]["inboundRefusedRate"]=n(&c.refused_rate);}
        // What peers delivered first (`Node::credit`), per kind.
        for (k,u) in ["creditsBlock","creditsTemplate","creditsTx","creditsShare"].into_iter().zip([Useful::Block,Useful::Template,Useful::Tx,Useful::Share]){
            status["stats"][k]=serde_json::json!(self.stats.credits[u as usize]);}
        status["inboundEstablished"]=serde_json::json!(self.net.established());
        status["inboundMax"]=serde_json::json!(self.net.limits.peers_max);
        // Peer discovery (VA-SPEC §1a): the live connections this node dialed, and discovery's state.
        status["outboundPeers"]=serde_json::json!(self.net.counters.outbound.load(std::sync::atomic::Ordering::SeqCst));
        status["discovery"]=self.disc_status();
        status["lifetime"]=if self.chain.spec.cp().H_END==0{serde_json::json!({"continuous":true,"closingHeight":null,"finalHeight":null,"closingTime":null,"finalTime":null})}else{serde_json::json!({"continuous":false,"closingHeight":self.chain.spec.cp().H_CLOSE,"finalHeight":self.chain.spec.cp().H_END})};
        // C-1 S5 (PR7 §4.4): the in-memory window and the canonical index below it, and the files the bound counts.
        let windowed=self.chain.evict_depth.filter(|e|*e<u64::MAX).is_some();let base=self.chain.canonical.base();
        status["canonicalBase"]=serde_json::json!(base);status["blocksIndexed"]=serde_json::json!(base.saturating_sub(1));
        status["anchors"]=serde_json::json!(self.chain.anchors().len());status["evictStalls"]=serde_json::json!(self.chain.evict_stalls);status["evictDepth"]=serde_json::json!(self.chain.evict_depth);
        status["diskObserved"]=serde_json::json!(disk.map(|d|serde_json::json!({"historyBytes":d.history,"stateBytes":d.state,"canonBytes":d.canon})));
        // What the continuous chain's bound grows with: the observed files with eviction on, the journal's records off.
        status["resourceDiskBytes"]=serde_json::json!((self.chain.spec.cp().H_END==0&&windowed).then(||disk.map(|d|d.history+d.state+d.canon)).flatten());
        status["unlimitedResourceProof"]=serde_json::json!(false);
        // V3-05: per-proposer template arrival and lock latency (network.md, "Template latency").
        status["templateLatency"]=self.template_latency_json();
        crate::network_display::annotate(status)
    }
    pub fn template_json(&self)->serde_json::Value{
        match &self.prod{Some(Production{mode:Mode::Locked(l),..})=>serde_json::json!({"templateId":hex(&l.tid),"header":hex(&l.header.encoded),"fallback":l.fallback,"nonceMax":l.ctx.assignment.nonce_max().to_string()}),_=>serde_json::Value::Null}
    }
    pub fn journal_records(&self)->u64{self.journal.records}
}
impl Node{
    fn st_bytes(h:&Header)->Vec<u8>{let mut p=h.ut_raw.clone();crate::rlp::encode_bytes(&h.sig,&mut p);let mut o=Vec::new();crate::rlp::encode_list_payload(&p,&mut o);o}
    /// Records a gossiped signed template; returns true when it equivocates (evidence type 1 created). It
    /// never rebuilds or materializes state (PR4, NODE-02).
    fn observe_template(&mut self,h:&Header)->bool{
        if h.proposer.len()!=20||h.sig.len()!=65||h.a!=0{return false}
        // F5/F6: a template counts only if its signer is the DRAWN proposer Draw(parent,0) at the drawn
        // height. The `proposer` field and the signing key are both chosen by the sender, so a signature
        // that merely recovers to the named `proposer` proves nothing — any peer could self-sign two
        // templates for a (parent,h) it picks and forge "equivocation" to force sticky fallback on every
        // node (F5). Verify the signature and the draw, after cheap parent/height filters, before recording
        // a template or flagging equivocation. Unknown parent / wrong height / any other signer: ignore (no
        // record, no evidence, no fallback).
        // NODE-02 (PR4): Draw(h,0) is read without making any state. full_entry rebuilt a pruned parent
        // from genesis (or materialized a diff-held one in place) for every gossiped template, before the
        // draw and signature checks. The drawn proposer for height h on the CANONICAL branch is the signer
        // that evidence_type1 (chain.rs:680-681) accepts for a type-1 pair at that height — it reads
        // ancestor_at(carrier,h).drawn, never tying the templates' parent to the carrier. So:
        //   - a template for a height already produced: the canonical block at h carries Draw(.,h) in
        //     `drawn`, kept through prune, slim and offload (chain.rs:749,213,230 never touch `drawn`), the
        //     same source evidence_type1 uses. This also records equivocation on a SIDE parent whose child's
        //     signer equals the canonically drawn proposer — valid, includable evidence an honest node keeps
        //     (PR4 plan §7 finding A). State cost: zero.
        //   - a template for the tip (h>head, no canonical block yet), or one on a SIDE parent signed by that
        //     parent's own Draw (includable on its branch if it wins): only a parent whose state is already
        //     resident is read (child_ctx on it; ensure_state_with would return at once).
        //   - anything else: dropped (no record, no evidence, no penalty).
        // Evidence is an incentive item, not a validity rule; the authenticated half (an imported block's
        // template) is recorded by import_with.
        let Some(pe)=self.chain.get(&h.parent_hash) else{return false};
        if h.h!=pe.height+1{return false}
        // Only a height whose type-1 evidence can still be included (evidence_type1's "ev1 too old": h+E_win >=
        // the inclusion height, at least head+1). Every older canonical template is public, so without this a peer
        // could replay them at one signature recovery each and churn seen_templates (review: historic flood).
        if h.h.saturating_add(u64::from(self.chain.spec.cp().E_win))<self.chain.head().height+1{return false}
        // The signature is cheap; it runs before the draw (PR4 plan §7 finding E2: a junk self-signed shell no
        // longer pays the membership+draw of child_ctx). Either order returns false for a bad signature.
        let tid=template_id(&h.ut_raw);
        if !matches!(hashes::recover_address(&hashes::sig_msg(&tid),&h.sig),Ok(a) if a[..]==h.proposer[..]){return false}
        // The canonically drawn proposer, or Draw(parent,0) of a parent whose state is resident: the tip, or a SIDE
        // parent at a height already produced (a pair by Draw(S,0) is includable on S's branch if it wins, and the
        // nodes that build there may not see it again). A canonical parent's Draw is the canonical block's `drawn`.
        let canon=self.chain.canonical_at(h.h).map(|c|c.drawn);
        let drawn=canon.is_some_and(|d|h.proposer[..]==d[..])||
            ((canon.is_none()||!self.chain.is_canonical(&h.parent_hash))&&pe.full&&!pe.pruned&&!pe.slim
                &&self.chain.child_ctx(pe,0).is_ok_and(|c|h.proposer[..]==c.proposer[..]));
        drawn&&self.record_template(h)
    }
    /// V3-04: the first time this node LOCKED a signed template, for classifying its solved block (`imports::classify`).
    /// `lock` calls it for a signed template only: its own, or one `try_lock_signed` validated as minable here (the
    /// drawn signer, the window, item 16 and its body rebuilding to its UT) by T_j. A template merely received, say a
    /// signed shell whose body does not rebuild, never counts: nobody else can mine it, so its block is a withheld one.
    /// Only on a profile that can activate v3: a spec-1 node keeps the map empty. One TemplateID per slot (parent and
    /// signer, height), the earliest time kept; a second TemplateID for a slot, or one seen_templates already
    /// contradicts, is an equivocation and poisons the slot (`template_equivocated`).
    fn note_template_seen(&mut self,h:&Header){
        if !self.cfg.limits.first_imports{return}
        let key=Self::seen_key(h);let tid=template_id(&h.ut_raw);let ms=self.import_clock.unwrap_or_else(now_ms);
        match self.template_seen.get_mut(&key){
            Some(x) if x.0==tid=>x.1=x.1.min(ms),
            Some(x)=>x.0=NO_TEMPLATE,
            None=>{let other=self.seen_templates.get(&key).is_some_and(|x|x.0!=h.ut_raw);self.insert_template_seen(key,if other{NO_TEMPLATE}else{tid},ms)}
        }
    }
    /// V3-04: the signer of slot `key` equivocated (type-1 evidence). No template of the slot is timely by the template
    /// rule, so the equivocator's privately released block is judged on its own arrival, as without the map.
    fn template_equivocated(&mut self,key:([u8;32],u64)){
        if !self.cfg.limits.first_imports{return}
        match self.template_seen.get_mut(&key){Some(x)=>x.0=NO_TEMPLATE,None=>self.insert_template_seen(key,NO_TEMPLATE,0)}
    }
    /// Bounded at TEMPLATE_SEEN_MAX slots. A full map first forgets the heights below the head (blocks there can only
    /// be side blocks of a chosen head's parent), then the heights above the tip (head+1, the height this node
    /// locks at). Each slot is one per (parent, signer, height) and a lock needs this node's production on that
    /// parent, so only valid blocks at the head and the tip, with PoW, can fill it; still full, a new slot is not
    /// recorded and its block falls back to the block-arrival rule: never more timely than without the map.
    fn insert_template_seen(&mut self,key:([u8;32],u64),tid:[u8;32],ms:u128){
        if self.template_seen.len()>=TEMPLATE_SEEN_MAX{
            let head=self.chain.head().height;self.template_seen.retain(|k,_|k.1>=head);
            if self.template_seen.len()>=TEMPLATE_SEEN_MAX{self.template_seen.retain(|k,_|k.1<=head+1);}
        }
        if self.template_seen.len()<TEMPLATE_SEEN_MAX{self.template_seen.insert(key,(tid,ms));}
    }
    /// The seen_templates key: (parent, signer) folded into one digest, and the height. Each of the two signers a
    /// (parent, h) can take (the canonically drawn proposer and Draw(parent,0)) has its own slot, so one's template
    /// never hides the other's pair; the key keeps its size (A13 `seen`).
    fn seen_key(h:&Header)->([u8;32],u64){let mut p=h.parent_hash.to_vec();p.extend_from_slice(&h.proposer);(hashes::keccak256(&p),h.h)}
    /// Records a template signed by Draw(h,0); true when it equivocates (type-1 evidence created). The
    /// caller has checked the shape (20-byte proposer, 65-byte sig, a=0), the signature and the draw.
    fn record_template(&mut self,h:&Header)->bool{
        let key=Self::seen_key(h);let st=Self::st_bytes(h);
        match self.seen_templates.get(&key){
            None=>{if self.seen_templates.len()>SEEN_MAX{self.evict_seen_templates();}self.seen_templates.insert(key,(h.ut_raw.clone(),st,h.proposer.clone()));false}
            Some((ut,first,prop)) if *ut!=h.ut_raw&&*prop==h.proposer=>{
                let mut p=Vec::new();crate::rlp::encode_bytes(&[1],&mut p);p.extend(first.clone());p.extend(st);let mut ev=Vec::new();crate::rlp::encode_list_payload(&p,&mut ev);
                if !self.evidence_pool.contains(&ev)&&self.evidence_pool.len()<EVIDENCE_POOL_MAX{self.log(&format!("equivocation (type 1) by proposer of h={}",h.h));self.evidence_pool.push(ev);}
                self.template_equivocated(key);true}
            _=>false,
        }
    }
    /// Makes room in a full seen_templates (more than SEEN_MAX keys) without forgetting the live tip: first the keys
    /// whose evidence can no longer be included (height + E_win < head + 1), then, if it is still full, every key
    /// but the tip's (height > head: the templates a sticky fallback acts on). The map is cleared whole only if the
    /// tip keys alone fill it. Clearing it whole on every overflow let a flood of templates hide a live
    /// equivocation (T1, flood, T2 was recorded as a first template).
    fn evict_seen_templates(&mut self){
        let head=self.chain.head().height;let lo=(head+1).saturating_sub(u64::from(self.chain.spec.cp().E_win));
        self.seen_templates.retain(|k,_|k.1>=lo);
        if self.seen_templates.len()>SEEN_MAX{self.seen_templates.retain(|k,_|k.1>head);}
        if self.seen_templates.len()>SEEN_MAX{self.seen_templates.clear();}
    }
    /// Records winner signatures per (TemplateID, nonce); a second share root creates evidence type 2.
    fn observe_win(&mut self,h:&Header){
        let tid=template_id(&h.ut_raw);let root=share_root(h);
        match self.seen_wins.get(&(tid,h.nonce)){
            None=>{if self.seen_wins.len()>SEEN_MAX{self.seen_wins.clear();}self.seen_wins.insert((tid,h.nonce),(root,h.winner_sig));}
            Some((r,sig)) if *r!=root=>{
                let mut p=Vec::new();crate::rlp::encode_bytes(&[2],&mut p);crate::rlp::encode_bytes(&tid,&mut p);crate::rlp::encode_bytes(&h.nonce.to_be_bytes(),&mut p);
                crate::rlp::encode_bytes(r,&mut p);crate::rlp::encode_bytes(sig,&mut p);crate::rlp::encode_bytes(&root,&mut p);crate::rlp::encode_bytes(&h.winner_sig,&mut p);
                let mut ev=Vec::new();crate::rlp::encode_list_payload(&p,&mut ev);
                if !self.evidence_pool.contains(&ev)&&self.evidence_pool.len()<EVIDENCE_POOL_MAX{self.log(&format!("equivocation (type 2) on h={}",h.h));self.evidence_pool.push(ev);}
            }
            _=>{}
        }
    }
    /// Evidence still valid on top of `p`, at most E_max, one per accused.
    fn includable_evidence(&mut self,p:&Entry)->Vec<Vec<u8>>{
        let e_max=usize::from(self.chain.spec.cp().E_max);let mut out:Vec<Vec<u8>>=Vec::new();let mut keep=Vec::new();
        for ev in std::mem::take(&mut self.evidence_pool){
            let one=Body{txs:vec![],evidence:vec![ev.clone()]};
            match self.chain.evidence_effects(p,&one,p.height+1){
                Ok(_)=>{let mut trial=out.clone();trial.push(ev.clone());
                    if out.len()<e_max&&self.chain.evidence_effects(p,&Body{txs:vec![],evidence:trial},p.height+1).is_ok(){out.push(ev.clone());}keep.push(ev);}
                Err(_)=>{} // already applied on this branch (accused tomb) or no longer provable: drop
            }
        }
        self.evidence_pool=keep;
        // V3-02 (v3 program set only): the shares this node saw for its ancestors' templates j in [max(1,h-D), h-1]
        // that the branch has not credited, as kind-3 items within E_max (`Chain::late_offer` applies every rule).
        let h=p.height+1;
        if self.chain.late_shares(h)&&!self.late_cache.is_empty(){
            let mut block=chain::LateBlock::default();
            for raw in &out{if let Ok((j,ns))=chain::late_parse(raw,u64::MAX){block.seen.entry(j).or_default().extend(ns);}}
            let lo=h.saturating_sub(u64::from(self.chain.spec.cp().D)).max(1);
            // Oldest first: j = h-D has no later carrier (settle(h-D) runs in this block), newer ones still do.
            for j in lo..h{
                if out.len()>=e_max{break}
                let Some(tid)=self.chain.late_template(p,j) else{continue};
                let Some(t)=self.late_cache.get(&tid) else{continue};
                if let Some(item)=self.chain.late_offer(p,j,&t.shares,&mut block){out.push(item);self.stats.late_items+=1;}
            }
        }
        out
    }
}

#[cfg(test)]
mod relay_tests{
    use super::*;
    use pocol_lp4_execution::{devnet::{self,DevnetConfig},txsign::Tx};
    fn pay(nonce:u64)->Vec<u8>{let alice=devnet::account_key("alice");
        Tx{chain_id:777910,nonce,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas:21000,to:Some(devnet::account_key("bob").address()),value:U256::from(1u64),data:Default::default()}.sign_1559(&alice)}
    /// D-7 re-insertion (mutant R5 of evidence/d7-bound/mutants-relay.log): a returned transaction goes to a peer
    /// through `Peer::offer`, never `send`. With the peer's queue empty it is queued. With the queue past half and
    /// not full, it is not, although gossip of it would still fit; the peer stays connected (a later frame is
    /// queued) and `reinserted_unoffered` counts the refusal.
    #[test] fn a_returned_transaction_is_not_queued_to_a_peer_past_half_its_queue(){
        let mut c=DevnetConfig::new(777910,1_700_000_000,&["a","b","c"]);c.hash_rate=1024;c.d_att=73;c.tau=40;c.c=8;
        let spec=Spec::new(devnet::build(&c).unwrap().admit(&crate::single_host_local()).unwrap().0);
        let dir=std::env::temp_dir().join(format!("pocold-relay-unit-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
        let cfg=Config{dir:dir.clone(),name:"relay".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits::default()};
        let mut n=Node::open(cfg,spec).unwrap();
        let (p,_other)=net::test_peer(7,4096);n.peers.insert(7,p.clone());
        // Room: the returned transaction is queued.
        n.admit_tx(pay(0),Relay::Offer,None,false).unwrap();
        let q0=p.queued_bytes();assert!(q0>0&&(n.stats.reinserted_offered,n.stats.reinserted_unoffered)==(1,0),"queued with room: {q0} bytes");
        // Past half of 4096 and below the whole queue.
        let filler=Msg::Tx{raw:vec![7u8;400]};while p.queued_bytes()<=2048{assert!(p.send(&filler));}
        let q1=p.queued_bytes();let f=Msg::Tx{raw:pay(1)}.encode().len()+4;assert!(q1+f<=4096,"gossip of it would still fit: {q1}+{f}");
        n.admit_tx(pay(1),Relay::Offer,None,false).unwrap();
        assert_eq!(p.queued_bytes(),q1,"not queued past half the queue");
        assert_eq!((n.stats.reinserted_offered,n.stats.reinserted_unoffered),(1,1));
        assert!(p.send(&Msg::Tx{raw:vec![1u8;8]}),"the peer is kept");
        assert_eq!(n.pool.txs.len(),2,"both are pooled");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// N4 review: freshness is explicit. A transaction re-queued for revalidation (`PoolItem::Tx`), whatever its relay
    /// (a parked one keeps `Relay::Gossip` when it was not relayed when parked), is not a new submission: with a pooled
    /// one's sender and nonce and higher fees it replaces nothing (executed on the pending state, its nonce is used).
    /// Only a P2P arrival queued behind the backlog (`PoolItem::Fresh`) may replace (or evict).
    /// Codex review of #32: a pool full by count drops the revalidation backlog, but not a P2P arrival queued behind it
    /// (`PoolItem::Fresh`), which may still evict the lowest-priority transaction (or replace one) at the count bound.
    /// FAIL-ON-ORIGINAL: `backlog.clear()` dropped the arrival with the revalidation entries.
    #[test] fn a_full_pool_keeps_a_queued_p2p_arrival_when_it_drops_the_revalidation_backlog(){
        let senders:Vec<String>=(0..16).map(|i|format!("s{i}")).collect();
        let mut c=DevnetConfig::new(777912,1_700_000_000,&["a","b","c"]);c.hash_rate=1024;c.d_att=73;c.tau=40;c.c=8;
        for l in senders.iter().map(|s|s.as_str()).chain(["zed","yan"]){c.accounts.push((l.into(),U256::from(10u128.pow(24))));}
        let spec=Spec::new(devnet::build(&c).unwrap().admit(&crate::single_host_local()).unwrap().0);
        let dir=std::env::temp_dir().join(format!("pocold-fullfresh-unit-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
        let limits=Limits{pool_gas_max:Some(POOL_MAX_TXS as u64*21000+10*21000),..Default::default()};
        let cfg=Config{dir:dir.clone(),name:"fullfresh".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits};
        let mut n=Node::open(cfg,spec).unwrap();
        let tx=|who:&str,nonce:u64,tip:u64|{let k=devnet::account_key(who);Tx{chain_id:777912,nonce,max_priority:U256::from(tip),max_fee:U256::from(100_000_000_000u64),gas:21000,to:Some(Address::with_last_byte(0xbb)),value:U256::from(1u64),data:Default::default()}.sign_1559(&k)};
        let per=POOL_MAX_TXS/senders.len();
        for s in &senders{for j in 0..per{n.submit_tx(tx(s,j as u64,2_000_000_000),false).unwrap();}}
        assert_eq!(n.pool.txs.len(),POOL_MAX_TXS);n.pool_audit().unwrap();
        // Two revalidation entries (refused at the count bound), then a P2P arrival that outbids the lowest rank.
        for raw in [tx("yan",0,2_000_000_000),tx("yan",1,2_000_000_000)]{n.pool.backlog_bytes+=raw.len();n.pool.backlog.push_back(PoolItem::Tx(raw,Relay::Gossip));}
        let fresh=tx("zed",0,3_000_000_000);n.pool.backlog_bytes+=fresh.len();n.pool.backlog.push_back(PoolItem::Fresh(fresh.clone()));
        n.pool_settle();n.pool_audit().unwrap();
        assert!(n.pool.txs.iter().any(|(_,r)|r==&fresh),"the queued P2P arrival evicted the lowest and was pooled");
        assert_eq!((n.pool.txs.len(),n.stats.pool_evicted,n.pool.backlog.len(),n.pool.backlog_bytes),(POOL_MAX_TXS,1,0,0));
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    #[test] fn only_a_queued_p2p_arrival_is_a_new_submission(){
        let mut c=DevnetConfig::new(777910,1_700_000_000,&["a","b","c"]);c.hash_rate=1024;c.d_att=73;c.tau=40;c.c=8;
        let spec=Spec::new(devnet::build(&c).unwrap().admit(&crate::single_host_local()).unwrap().0);
        let dir=std::env::temp_dir().join(format!("pocold-fresh-unit-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
        let cfg=Config{dir:dir.clone(),name:"fresh".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits::default()};
        let mut n=Node::open(cfg,spec).unwrap();
        let bump=|tip:u64|{let alice=devnet::account_key("alice");Tx{chain_id:777910,nonce:0,max_priority:U256::from(tip),max_fee:U256::from(200_000_000_000u64),gas:21000,to:Some(devnet::account_key("bob").address()),value:U256::from(1u64),data:Default::default()}.sign_1559(&alice)};
        let old=pay(0);n.submit_tx(old.clone(),false).unwrap();
        let (gossip,fresh)=(bump(2_000_000_000),bump(3_000_000_000));
        for (item,raw) in [(PoolItem::Tx(gossip.clone(),Relay::Gossip),&gossip),(PoolItem::Fresh(fresh.clone()),&fresh)]{
            n.pool.backlog_bytes+=raw.len();n.pool.backlog.push_back(item);n.pool_settle();n.pool_audit().unwrap();
            if raw==&gossip{assert_eq!((n.pool.txs[0].1.clone(),n.stats.pool_replaced),(old.clone(),0),"a revalidation replaces nothing")}
            else{assert_eq!((n.pool.txs[0].1.clone(),n.stats.pool_replaced),(fresh.clone(),1),"a queued P2P arrival replaces")}
        }
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod template_error_tests{
    //! Gossip review: only a verdict on a template (`Reject::Block`) punishes its sender. A check this node
    //! cannot complete (a state it does not hold, retention, ...) is no verdict, so an honest proposer or an
    //! honest relay is never closed for it. (`Reject::Local` stops the node, `fatal`: not testable in process.)
    use super::*;
    use pocol_lp4_execution::devnet::{self,DevnetConfig};
    #[test] fn only_a_block_verdict_punishes_the_sender_of_a_template(){
        let c=DevnetConfig::new(777916,1_700_000_000,&["a","b","c"]);
        let spec=Spec::new(devnet::build(&c).unwrap().admit(&crate::single_host_local()).unwrap().0);
        let dir=std::env::temp_dir().join(format!("pocold-tplerr-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
        let cfg=Config{dir:dir.clone(),name:"tplerr".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits::default()};
        let mut n=Node::open(cfg,spec).unwrap();
        for e in [Reject::NeedsRebuild([7;32]),Reject::Evicted,Reject::Envelope("sig".into()),Reject::UnknownParent,Reject::Future,Reject::Duplicate]{
            assert!(!n.template_error(1,"check",e.clone()),"{e:?}: no lock");
            assert_eq!(n.stats.template_penalties,0,"{e:?} is no verdict on the template: no penalty");
        }
        assert!(!n.template_error(1,"check",Reject::Block("15 evidence kind".into())),"a verdict: no lock");
        assert_eq!(n.stats.template_penalties,1,"a Block verdict punishes the sender");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod f5f6_tests{
    //! F5/F6: observe_template must treat a template as the drawn proposer's only when its signer IS
    //! Draw(parent,0). Encodes the FIXED behavior — a throwaway-key peer cannot forge "equivocation"
    //! to force sticky fallback, and a wrong-height template is ignored — while a real drawn-proposer
    //! equivocation is still detected. FAILS on the original observe_template (which flagged any
    //! self-signed pair). Synthetic keys only; no network.
    use super::*;
    use pocol_lp4_execution::devnet::{self,DevnetConfig};
    fn open_node()->(Node,std::path::PathBuf){open_node_at("f5f6")}
    /// Each test of this module opens its own directory: they run in parallel.
    fn open_node_at(tag:&str)->(Node,std::path::PathBuf){
        let c=DevnetConfig::new(777915,1_700_000_000,&["a","b","c"]);
        let spec=Spec::new(devnet::build(&c).unwrap().admit(&crate::single_host_local()).unwrap().0);
        let dir=std::env::temp_dir().join(format!("pocold-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
        let cfg=Config{dir:dir.clone(),name:"f5f6".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits::default()};
        (Node::open(cfg,spec).unwrap(),dir)
    }
    /// A self-signed template for (parent,h,a=0) by `key`; `salt` varies ut_raw so two differ.
    fn selfsigned(parent:[u8;32],h:u64,key:&Key,salt:u8)->Header{
        let mut hd=crate::types::blank_header();
        hd.parent_hash=parent;hd.h=h;hd.a=0;hd.ut_raw=vec![salt;8];
        hd.proposer=key.address().as_slice().to_vec();
        let tid=template_id(&hd.ut_raw);
        hd.sig=key.sign_hash(&B256::from(hashes::sig_msg(&tid))).to_vec();
        hd
    }
    #[test] fn observe_template_ignores_non_drawn_proposer(){
        let (mut n,dir)=open_node();
        let gh=n.chain.head().hash;assert_eq!(n.chain.head().height,0,"fresh node at genesis");
        // The real drawn proposer for (genesis,0) and its signing key.
        let par=n.full_entry(&gh).unwrap();
        let drawn=n.chain.child_ctx(&par,0).unwrap().proposer;drop(par);
        let drawn_key=["a","b","c"].into_iter().map(|l|devnet::miner_identity(l).key).find(|k|k.address()==drawn).expect("the draw is a genesis miner");
        // 1) A throwaway-key peer cannot forge equivocation -> no evidence, no sticky-fallback trigger.
        let atk=devnet::account_key("zzz-f5-attacker");
        assert!(!n.observe_template(&selfsigned(gh,1,&atk,1)),"first attacker template ignored");
        assert!(!n.observe_template(&selfsigned(gh,1,&atk,2)),"FAIL-ON-ORIGINAL: attacker forged equivocation (returned true)");
        assert!(n.evidence_pool.is_empty(),"FAIL-ON-ORIGINAL: a non-drawn signer produced type-1 evidence");
        // 2) A template at the wrong height is ignored even from the real proposer.
        assert!(!n.observe_template(&selfsigned(gh,2,&drawn_key,1)),"wrong height (h!=parent.height+1) ignored");
        // 3) Positive control: a genuine equivocation by the DRAWN proposer is still detected.
        assert!(!n.observe_template(&selfsigned(gh,1,&drawn_key,10)),"first genuine template recorded");
        assert!(n.observe_template(&selfsigned(gh,1,&drawn_key,11)),"a real drawn-proposer equivocation is still flagged");
        assert_eq!(n.evidence_pool.len(),1,"exactly one type-1 evidence, from the real proposer");
        eprintln!("F5/F6: attacker self-signed pair ignored (pool stayed empty); real proposer equivocation still detected.");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// V3-04 template first-seen times: none on a profile that cannot activate v3; on one that can, a template counts
    /// only once locked (a received shell never does), one TemplateID per slot with the earliest time, an equivocated
    /// slot poisoned however many templates its signer floods, and never more than TEMPLATE_SEEN_MAX slots.
    #[test] fn template_first_seen_times_are_v3_only_locked_one_per_slot_and_bounded(){
        let (mut n,dir)=open_node_at("template-seen");
        let gh=n.chain.head().hash;let par=n.full_entry(&gh).unwrap();let drawn=n.chain.child_ctx(&par,0).unwrap().proposer;drop(par);
        let drawn_key=["a","b","c"].into_iter().map(|l|devnet::miner_identity(l).key).find(|k|k.address()==drawn).unwrap();
        // A signed template with a distinct UT per salt (up to 2^16 of them).
        let signed=|parent:[u8;32],h:u64,key:&Key,salt:u16|{let mut t=selfsigned(parent,h,key,0);t.ut_raw=salt.to_be_bytes().repeat(4);
            t.sig=key.sign_hash(&B256::from(hashes::sig_msg(&template_id(&t.ut_raw)))).to_vec();t};
        let t=signed(gh,1,&drawn_key,20);let tid=template_id(&t.ut_raw);let slot=Node::seen_key(&t);
        assert!(!n.cfg.limits.first_imports,"a spec-1 profile");n.note_template_seen(&t);n.template_equivocated(slot);assert!(n.template_seen.is_empty(),"spec 1 keeps no map");
        n.cfg.limits.first_imports=true;
        // Gossip alone never counts: a drawn signer's shell may carry a body nobody can rebuild (a withheld template).
        n.import_clock=Some(3);n.observe_template(&t);assert!(n.template_seen.is_empty(),"received, not locked: not recorded");
        n.import_clock=Some(9);n.note_template_seen(&t);n.import_clock=Some(5);n.note_template_seen(&t);n.import_clock=Some(8);n.note_template_seen(&t);
        assert_eq!(n.template_seen.get(&slot),Some(&(tid,5)),"the earliest lock is kept");
        // A flood of distinct templates by the drawn signer for one (parent, h) is one slot, poisoned at the first pair.
        for salt in 21..21+TEMPLATE_SEEN_MAX as u16+44{n.observe_template(&signed(gh,1,&drawn_key,salt));}
        assert_eq!(n.template_seen.len(),1,"one slot however many templates");assert_eq!(n.template_seen[&slot].0,NO_TEMPLATE,"equivocation poisons the slot");
        n.note_template_seen(&t);assert_eq!(n.template_seen[&slot].0,NO_TEMPLATE,"a later lock of the slot stays poisoned");
        // A first lock that seen_templates already contradicts (the pair seen while this slot was not held) is poisoned.
        n.template_seen.clear();n.note_template_seen(&signed(gh,1,&drawn_key,21));assert_eq!(n.template_seen[&slot].0,NO_TEMPLATE);
        // A second TemplateID locked for a held slot poisons it as well.
        let other=signed([0x77;32],1,&drawn_key,1);let other_slot=Node::seen_key(&other);n.note_template_seen(&other);
        assert_eq!(n.template_seen[&other_slot].0,template_id(&other.ut_raw));
        n.note_template_seen(&signed([0x77;32],1,&drawn_key,2));assert_eq!(n.template_seen[&other_slot].0,NO_TEMPLATE);
        // The bound: a full map forgets the slots above the tip (head+1) first; full of head and tip slots, it records
        // nothing new and keeps the earlier entries.
        n.template_seen.clear();let parent=|i:u16|{let mut p=[0xab;32];p[..2].copy_from_slice(&i.to_be_bytes());p};
        for i in 0..TEMPLATE_SEEN_MAX as u16-1{n.note_template_seen(&signed(parent(i),7,&drawn_key,i));}
        n.note_template_seen(&t);assert_eq!(n.template_seen.len(),TEMPLATE_SEEN_MAX);
        let tip=signed([0xcd;32],1,&drawn_key,9);n.note_template_seen(&tip);
        assert_eq!(n.template_seen.len(),2,"the slots above the tip were forgotten");assert_eq!(n.template_seen[&Node::seen_key(&tip)].0,template_id(&tip.ut_raw));
        for i in 0..TEMPLATE_SEEN_MAX as u16-2{n.note_template_seen(&signed(parent(i),1,&drawn_key,i));}
        assert_eq!(n.template_seen.len(),TEMPLATE_SEEN_MAX);
        let late=signed([0xce;32],1,&drawn_key,10);n.note_template_seen(&late);
        assert_eq!(n.template_seen.len(),TEMPLATE_SEEN_MAX,"the bound holds");assert!(!n.template_seen.contains_key(&Node::seen_key(&late)),"a full map keeps the earlier entries");
        assert_eq!(n.template_seen[&Node::seen_key(&tip)].0,template_id(&tip.ut_raw));
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod f7_tests{
    //! F7: once this node signs an attempt-0 template for (parent,height) it must never sign a
    //! DIFFERENT one (self-equivocation, type 1, burns the bond). In-process (a mining-key toggle) it
    //! re-broadcasts the same template; after a restart (durable record loaded, body not cached) it
    //! declines to sign. The guard runs before begin_attempt's timing gate, so the test is
    //! deterministic (T1 is built with an explicit clock, as the consensus tests do). The original node
    //! has no such guard at all, so on a restart/toggle it would build and broadcast the differing
    //! template this test constructs as `h2` — the double-sign. Synthetic keys; no network.
    use super::*;
    use pocol_lp4_execution::devnet::{self,DevnetConfig};
    fn miner_node(dir:&std::path::Path)->(Node,Key,[u8;32]){
        let c=DevnetConfig::new(777917,1_700_000_000,&["a","b","c"]);
        let spec=Spec::new(devnet::build(&c).unwrap().admit(&crate::single_host_local()).unwrap().0);
        let cfg=Config{dir:dir.to_path_buf(),name:"f7".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits::default()};
        let mut n=Node::open(cfg,spec).unwrap();
        let gh=n.chain.head().hash;
        let par=n.full_entry(&gh).unwrap();let drawn=n.chain.child_ctx(&par,0).unwrap().proposer;drop(par);
        // Key is not Clone; re-derive the deterministic devnet key of the drawn miner's label each time.
        let label=["a","b","c"].into_iter().find(|l|devnet::miner_identity(l).key.address()==drawn).expect("the draw is a genesis miner");
        n.cfg.miner=Some(devnet::miner_identity(label).key); // this node is the drawn proposer (no restart_production)
        (n,devnet::miner_identity(label).key,gh)
    }
    fn set_prod(n:&mut Node,parent:[u8;32]){
        n.prod=Some(Production{parent,height:1,attempt:0,mode:Mode::Idle,shares:BTreeSet::new(),miner:None,solved:false,received_ms:now_ms()});
    }
    fn locked(n:&Node)->Option<([u8;32],bool)>{match &n.prod{Some(Production{mode:Mode::Locked(l),..})=>Some((l.tid,l.fallback)),_=>None}}
    fn stop_miner(n:&mut Node){if let Some(p)=n.prod.as_mut(){if let Some(m)=p.miner.take(){m.stop();}}}

    #[test] fn never_signs_a_second_template_for_the_same_parent_height(){
        let dir=std::env::temp_dir().join(format!("pocold-f7-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);std::fs::create_dir_all(&dir).unwrap();
        let (mut n,key,gh)=miner_node(&dir);
        // Sign T1 for (genesis,1) exactly as the node does, and record it (persisted + cached).
        let par=n.full_entry(&gh).unwrap();let ctx=n.chain.child_ctx(&par,0).unwrap();
        let (h1,_,_)=n.chain.build_signed(&par,&key,ctx.window.lower(),Body::default(),vec![]).unwrap();
        let tid1=template_id(&h1.ut_raw);
        n.record_signed_a0(gh,1,tid1,h1.encoded.clone(),Body::default().encode());
        assert_eq!(n.signed_a0.get(&(gh,1)),Some(&tid1),"T1 recorded");
        assert!(dir.join("signed-a0").exists(),"the signing record is persisted durably");
        // A second template for the SAME (parent,height) is a different template id — this is exactly
        // the signature the original (guardless) node would build and broadcast on a restart/toggle.
        let (h2,_,_)=n.chain.build_signed(&par,&key,ctx.window.lower()+1,Body::default(),vec![]).unwrap();
        assert_ne!(template_id(&h2.ut_raw),tid1,"a re-build for (parent,h) differs from T1 (the original's double-sign)");
        drop(par);
        // In-process re-attempt (mining-key toggle): the guard re-broadcasts T1, never a new template.
        set_prod(&mut n,gh);n.begin_attempt(0);
        assert_eq!(locked(&n),Some((tid1,false)),"re-attempt re-locked T1 as the signed template, not a new one");
        assert_eq!(n.signed_a0.len(),1,"still exactly T1 — no second signature");
        stop_miner(&mut n);drop(n);
        // Restart: reopen on the same data dir (durable record loads; the cached body is gone).
        let (mut n2,_k,gh2)=miner_node(&dir);assert_eq!(gh2,gh);
        assert_eq!(n2.signed_a0.get(&(gh,1)),Some(&tid1),"the signing record survived the restart");
        set_prod(&mut n2,gh);n2.begin_attempt(0);
        assert!(locked(&n2).is_none(),"after a restart it declines to sign (waits), never locks a NEW signed template");
        assert_eq!(n2.signed_a0.len(),1,"no second signature was recorded after the restart");
        assert_eq!(n2.signed_a0.get(&(gh,1)),Some(&tid1),"still only T1");
        eprintln!("F7: a recorded (parent,h) is never re-signed — re-broadcast in-process, declined after restart.");
        stop_miner(&mut n2);drop(n2);let _=std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod late_cache_tests{
    //! V3-02 node-local (review E3): a template the node locks again is mined again, so a peer's share of it takes the
    //! locked path (into the production's shares, relayed) and never stops in the late-share cache; the cached shares
    //! come back to the production on that lock. A share of a template that is only cached is kept and relayed once.
    //! (The handler and the cache do not depend on the program set; a v3 node fills the cache, `stash_late`.)
    use super::*;
    use pocol_lp4_execution::devnet::{self,DevnetConfig};
    #[test] fn a_template_locked_again_takes_its_shares_back_and_its_peer_shares_are_relayed(){
        let c=DevnetConfig::new(777918,1_700_000_000,&["a","b","c"]);
        let spec=Spec::new(devnet::build(&c).unwrap().admit(&crate::single_host_local()).unwrap().0);
        let dir=std::env::temp_dir().join(format!("pocold-latecache-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
        let cfg=Config{dir:dir.clone(),name:"latecache".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits::default()};
        let mut n=Node::open(cfg,spec).unwrap();
        let (_p7,_o7)=net::test_peer(7,1<<16);let (p8,_o8)=net::test_peer(8,1<<16);n.peers.insert(7,_p7.clone());n.peers.insert(8,p8.clone());
        let gh=n.chain.head().hash;let par=n.full_entry(&gh).unwrap();
        for id in [7,8]{n.peer_status.insert(id,PeerStatus{work:U256::ZERO,head:gh,height:0,stale:false});}
        let (fb,_,ctx)=n.chain.build_fallback(&par,0).unwrap();let tid=template_id(&fb.ut_raw);drop(par);
        let nmax=ctx.assignment.nonce_max();let cap=SHARE_RELAY_PER_S_MAX*usize::from(n.chain.spec.cp().S_max);
        let on:Vec<u64>=(0..nmax as u64).filter(|x|le_target(&pow_hash(&tid,*x),&ctx.t_share)).take(4).collect();assert_eq!(on.len(),4);
        // The template was mined and left (stashed) with one share; the head is back on its parent and it is locked again.
        n.late_cache.stash(LateTemplate{tid,height:1,nonce_max:nmax,t_share:ctx.t_share,shares:vec![on[0]]},cap);
        n.prod=Some(Production{parent:gh,height:1,attempt:0,mode:Mode::Idle,shares:BTreeSet::new(),miner:None,solved:false,received_ms:now_ms()});
        n.lock(fb.clone(),Body::default(),true);
        let shares=|n:&Node|n.prod.as_ref().unwrap().shares.iter().copied().collect::<Vec<u64>>();
        assert!(n.late_cache.get(&tid).is_none(),"FAIL-ON-ORIGINAL: a template locked again leaves the cache");
        assert_eq!(shares(&n),vec![on[0]],"its cached share comes back to the production");
        // A peer's share of it: into the production's shares (a header share if this node wins) and relayed.
        let q=p8.queued_bytes();
        n.on_msg(7,Msg::Share{tid,nonce:on[1]});
        assert_eq!(shares(&n),vec![on[0],on[1]],"FAIL-ON-ORIGINAL: the locked template's peer share is not swallowed by the cache");
        assert!(p8.queued_bytes()>q,"relayed to the other peer");let q=p8.queued_bytes();
        // Production leaves it (another mode): a later share of the cached template is kept and relayed once.
        n.late_cache.stash(LateTemplate{tid,height:1,nonce_max:nmax,t_share:ctx.t_share,shares:shares(&n)},cap);
        n.prod.as_mut().unwrap().mode=Mode::Idle;
        n.on_msg(7,Msg::Share{tid,nonce:on[2]});
        assert_eq!(n.late_cache.get(&tid).unwrap().shares,vec![on[0],on[1],on[2]],"kept for late credit");
        assert!(p8.queued_bytes()>q,"FAIL-ON-ORIGINAL: a new late share is relayed");let q=p8.queued_bytes();
        n.on_msg(7,Msg::Share{tid,nonce:on[2]});
        assert_eq!(p8.queued_bytes(),q,"a duplicate is not relayed");
        let off=(0..nmax as u64).find(|x|!le_target(&pow_hash(&tid,*x),&ctx.t_share)).unwrap();
        n.on_msg(7,Msg::Share{tid,nonce:off});n.on_msg(7,Msg::Share{tid,nonce:nmax as u64});
        assert_eq!((p8.queued_bytes(),n.late_cache.get(&tid).unwrap().shares.len()),(q,3),"off target or past nMax: neither kept nor relayed");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
    /// Review E3 (blocking): a template the drawn proposer signed for the production parent but at a height other than
    /// the parent's plus one is a verdict on it (item 2): no lock, its sender punished, and its evidence never judged
    /// (kind-3 rules are relative to a child of the parent; judged at a false height one made the node stop).
    #[test] fn a_signed_template_at_a_false_height_is_punished_before_its_evidence(){
        let c=DevnetConfig::new(777919,1_700_000_000,&["a","b","c"]);
        let spec=Spec::new(devnet::build(&c).unwrap().admit(&crate::single_host_local()).unwrap().0);
        let dir=std::env::temp_dir().join(format!("pocold-latefalseh-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
        let cfg=Config{dir:dir.clone(),name:"latefalseh".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits::default()};
        let mut n=Node::open(cfg,spec).unwrap();
        let gh=n.chain.head().hash;let par=n.full_entry(&gh).unwrap();let ctx=n.chain.child_ctx(&par,0).unwrap();
        let label=["a","b","c"].into_iter().find(|l|devnet::miner_identity(l).key.address()==ctx.proposer).expect("the draw is a genesis miner");
        let key=devnet::miner_identity(label).key;
        let (mut hd,_,_)=n.chain.build_signed(&par,&key,ctx.window.lower(),Body::default(),vec![]).unwrap();drop(par);
        n.prod=Some(Production{parent:gh,height:1,attempt:0,mode:Mode::Idle,shares:BTreeSet::new(),miner:None,solved:false,received_ms:now_ms()});
        // (An empty body: on this spec-1 node a kind-3 item would be "15 evidence form" anyway; the v3 verdict on the
        // item itself is tests/v3_late_shares.rs late_shares_refused_on_a_real_branch. The original locked this.)
        hd.h=2;
        assert!(!n.try_lock_signed(hd.clone(),Body::default(),1),"FAIL-ON-ORIGINAL: no lock");
        assert_eq!(n.stats.template_penalties,1,"its sender is punished for the height (item 2)");
        assert!(matches!(n.prod.as_ref().unwrap().mode,Mode::Idle));
        // Positive control: the same template at its true height locks.
        hd.h=1;assert!(n.try_lock_signed(hd,Body::default(),1),"the true height locks");
        if let Some(p)=n.prod.as_mut(){if let Some(m)=p.miner.take(){m.stop();}}
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
}
