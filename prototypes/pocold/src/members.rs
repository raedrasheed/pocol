//! FN-3 (C6, node-local storage; FRESH-NETWORK-DESIGN.md C6, development plan FN-3): each block's member set S_b, the
//! sorted ids its nonce assignment is computed from (`Assignment::from_ids`), kept as a record per block instead of the
//! block's whole nonce ranges (64 B per member and block). A record is the change from the parent's set (a delta of at
//! most DELTA_CAP ids) or, every SNAPSHOT_EVERY heights and whenever the change is larger, the whole set (a snapshot).
//! A stored block's owner(nonce) is recomputed on demand from its set, its parent's seal, its height, its attempt and
//! its target, behind small caches (`chain::Chain::assignment_of`).
//!
//! Nothing here is a validity input. A record is a pure function of (S_parent, S_b, h), so live import, replay and a
//! restore write the same bytes; a record or set that is missing, malformed or does not add up (its count and its
//! order-free digest) is the node's own storage failing (`Reject::Local`), never a verdict on a block. FN-3a kept the
//! nonce ranges beside the records and took every verdict from them (the reconstruction compared at each reader, the
//! shadow); FN-3b takes the verdicts from the reconstruction (`chain::CheckedOwners`) and removed the ranges, and the
//! oracle (off by default) compares it with a cold recomputation and with the capture below.
use std::collections::{BTreeMap,HashMap,VecDeque};
use std::sync::{Arc,Mutex};
use revm::primitives::{keccak256,Address,U256};
use crate::chain::{Entry,Reject};

/// A member id as the assignment reads it.
pub type Id=[u8;20];
/// A block's member set: sorted ascending, distinct.
pub type Set=Arc<[Id]>;
/// A snapshot at every height h with h % SNAPSHOT_EVERY == 1 (h = 1, 257, 513, ...).
pub const SNAPSHOT_EVERY:u64=256;
/// A change of more ids than this is written as a snapshot.
pub const DELTA_CAP:usize=32;
/// The most delta steps a walk takes to its anchor (the cadence keeps it at most SNAPSHOT_EVERY - 1).
pub const WALK_MAX:u64=256;
/// The node's evidence pool bound the owner memo is sized for (node::EVIDENCE_POOL_MAX, asserted there).
pub const EVIDENCE_POOL_ASSUMED:usize=64;
/// The owner memo's hard cap.
pub const OWNERS_MAX:usize=4096;
/// The record format.
pub const REC_VERSION:u8=1;
/// The fixed part of an encoded record: ver, kind, parent, h, n, digest, n_add, n_rem.
pub const REC_FIXED:usize=1+1+32+8+2+32+2+2;

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct MemberRec{
    /// |S_b| (at most M_max, a u16).
    pub n:u16,
    /// The sum of keccak256(id) mod 2^256 over S_b: order-free, updated in O(|Δ|). A check of local data only.
    pub digest:[u8;32],
    pub step:MemberStep,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub enum MemberStep{
    /// S_b = S_parent + adds - rems: both sorted ascending, adds disjoint from S_parent, rems a subset of it.
    Delta{adds:Box<[Id]>,rems:Box<[Id]>},
    /// S_b is stored whole (history.redb `member_sets`, or a bare chain's `MemSets`).
    Snapshot,
}
/// What an entry holds of its member record while it is not offloaded (`chain::Entry::members`): the record and, for a
/// snapshot block built by `h_full`, its ids. A copy of the persisted record (the source holds it whatever the entry
/// holds), so it is dropped with the entry's other bytes when it is offloaded, and a reader then reads the source.
#[derive(Clone,Debug)]
pub struct EntryMembers{pub rec:MemberRec,pub ids:Option<Set>}
impl MemberRec{
    /// Genesis: the empty set (nothing reads it: a block at height 1 is a snapshot).
    pub fn genesis()->Self{MemberRec{n:0,digest:[0;32],step:MemberStep::Snapshot}}
    pub fn is_snapshot(&self)->bool{matches!(self.step,MemberStep::Snapshot)}
    /// The ids the record's delta changes (0 for a snapshot).
    pub fn delta_len(&self)->usize{match &self.step{MemberStep::Delta{adds,rems}=>adds.len()+rems.len(),_=>0}}
    /// The heap the record holds beyond its inline size.
    pub fn heap_bytes(&self)->usize{self.delta_len()*20}
}
/// Whether block h's record is a snapshot: at the cadence, or when its change exceeds DELTA_CAP.
pub fn is_snapshot(h:u64,delta_len:usize)->bool{h%SNAPSHOT_EVERY==1||delta_len>DELTA_CAP}
/// The most ids one block can change in S_b when the membership is every live record (`all_live_everywhere`): REG_OPS (register, deregister, removeInactive and
/// claimBond share it; registerReplacing costs two for its two changes), R_react (reactivate) and E_max (punish's
/// removeActive); the sweep and markClose do not touch S_b (PoColSystemV3.sol `checkBudgets`, `registerReplacing`,
/// `punish`). A larger change (a CP with delta_max > DELTA_CAP; on a CP whose membership is not every live record, an
/// activity-window flip) is written as a snapshot.
pub fn delta_max(cp:&pocol_lp4_execution::profile::ConsensusParams)->usize{usize::from(cp.REG_OPS)+usize::from(cp.R_react)+usize::from(cp.E_max)}
/// Whether this CP's membership is every live record at every height (`chain::Chain::membership`'s `all_live`, always
/// on): CP schema v2 and nonceMode 1, which every admitted CP has (admission refuses any other specVersion or nonceMode,
/// and a cap raise keeps both), so it holds for every `Spec::cp_max` A13 is computed at. It is false only for raw CP
/// values that no admission accepts (a CP schema v1 or nonceMode 0 encoding, such as the pre-phase-6a C6-row CP whose
/// A13 figures are recorded in tests/fn3_scale.rs); the A13 terms of those keep the activity-window bound.
pub fn all_live_everywhere(cp:&pocol_lp4_execution::profile::ConsensusParams)->bool{cp.schema_version()>=2&&cp.nonceMode==1}
/// The ids a delta of this CP's chain holds at most: min(DELTA_CAP, delta_max) when the membership is every live record
/// at every height (`all_live_everywhere`, every admitted CP); DELTA_CAP otherwise (a raw CP that no admission accepts,
/// whose activity window can flip any number of ids at once).
pub fn delta_bound(cp:&pocol_lp4_execution::profile::ConsensusParams)->usize{if all_live_everywhere(cp){DELTA_CAP.min(delta_max(cp))}else{DELTA_CAP}}

fn word(id:&Id)->U256{U256::from_be_bytes(keccak256(id).0)}
/// The digest of a set (M keccaks).
pub fn digest_of(ids:&[Id])->[u8;32]{ids.iter().fold(U256::ZERO,|a,id|a.wrapping_add(word(id))).to_be_bytes::<32>()}
/// `d` + Σ adds − Σ rems (O(|Δ|)).
pub fn digest_step(d:&[u8;32],adds:&[Id],rems:&[Id])->[u8;32]{
    let mut x=U256::from_be_bytes(*d);for id in adds{x=x.wrapping_add(word(id));}for id in rems{x=x.wrapping_sub(word(id));}x.to_be_bytes::<32>()
}
/// (adds, rems) taking the sorted set `a` to the sorted set `b`: one merge, O(|a| + |b|).
pub fn diff(a:&[Id],b:&[Id])->(Vec<Id>,Vec<Id>){
    let (mut i,mut j)=(0,0);let (mut adds,mut rems)=(Vec::new(),Vec::new());
    while i<a.len()||j<b.len(){
        match (a.get(i),b.get(j)){
            (Some(x),Some(y)) if x==y=>{i+=1;j+=1;}
            (Some(x),Some(y)) if x<y=>{rems.push(*x);i+=1;}
            (Some(_),Some(y))=>{adds.push(*y);j+=1;}
            (Some(x),None)=>{rems.push(*x);i+=1;}
            (None,Some(y))=>{adds.push(*y);j+=1;}
            (None,None)=>unreachable!(),
        }
    }
    (adds,rems)
}
/// Whether `ids` is strictly ascending (sorted and distinct).
pub fn ascending(ids:&[Id])->bool{ids.windows(2).all(|w|w[0]<w[1])}
/// The record of block h with set `s_b` over its parent's set `s_p` and digest `d_p`: a snapshot at the cadence or when
/// the change exceeds DELTA_CAP (the set is then returned to be kept), a delta otherwise. A function of (S_p, S_b, h).
pub fn record_for(h:u64,s_p:Option<(&[Id],&[u8;32])>,s_b:&Set)->(MemberRec,Option<Set>){
    let snap=||(MemberRec{n:s_b.len() as u16,digest:digest_of(s_b),step:MemberStep::Snapshot},Some(s_b.clone()));
    if h%SNAPSHOT_EVERY==1{return snap()}
    let Some((s_p,d_p))=s_p else{return snap()};
    let (adds,rems)=diff(s_p,s_b);
    if is_snapshot(h,adds.len()+rems.len()){return snap()}
    let digest=digest_step(d_p,&adds,&rems);
    (MemberRec{n:s_b.len() as u16,digest,step:MemberStep::Delta{adds:adds.into(),rems:rems.into()}},None)
}

/// The deltas `steps` (oldest first) applied to the sorted `base`: each removal must be in the set as it then is and
/// each addition must not, through an overlay (the latest state of each changed id) and one merge with `base` at the
/// end, exact whatever the order of the changes. O(Σ|Δ|·log + |base|).
pub fn apply(base:&[Id],steps:&[(&[Id],&[Id])])->Result<Vec<Id>,String>{
    let mut over:BTreeMap<Id,bool>=BTreeMap::new();
    let has=|over:&BTreeMap<Id,bool>,id:&Id|over.get(id).copied().unwrap_or_else(||base.binary_search(id).is_ok());
    for (i,(adds,rems)) in steps.iter().enumerate(){
        if !ascending(adds)||!ascending(rems){return Err(format!("delta {i} is not sorted"))}
        for id in rems.iter(){if !has(&over,id){return Err(format!("delta {i} removes an id the set does not hold"))}over.insert(*id,false);}
        for id in adds.iter(){if has(&over,id){return Err(format!("delta {i} adds an id the set holds"))}over.insert(*id,true);}
    }
    let mut out=Vec::with_capacity(base.len()+over.values().filter(|v|**v).count());
    let mut o=over.into_iter().peekable();
    for id in base{
        while let Some((k,v))=o.peek(){if k<id{if *v{out.push(*k)}o.next();}else{break}}
        match o.peek(){Some((k,v)) if k==id=>{if *v{out.push(*k)}o.next();}_=>out.push(*id)}
    }
    for (k,v) in o{if v{out.push(k)}}
    Ok(out)
}

/// The record bytes (big-endian): ver u8, kind u8 (0 delta, 1 snapshot), parent 32 B, h u64, n u16, digest 32 B, n_add
/// u16, n_rem u16, then the adds and the rems (20 B each, ascending).
pub fn encode_rec(parent:&[u8;32],h:u64,r:&MemberRec)->Vec<u8>{
    let (kind,adds,rems):(u8,&[Id],&[Id])=match &r.step{MemberStep::Delta{adds,rems}=>(0,adds,rems),MemberStep::Snapshot=>(1,&[],&[])};
    let mut v=Vec::with_capacity(REC_FIXED+20*(adds.len()+rems.len()));
    v.push(REC_VERSION);v.push(kind);v.extend_from_slice(parent);v.extend_from_slice(&h.to_be_bytes());v.extend_from_slice(&r.n.to_be_bytes());v.extend_from_slice(&r.digest);
    v.extend_from_slice(&(adds.len() as u16).to_be_bytes());v.extend_from_slice(&(rems.len() as u16).to_be_bytes());
    for id in adds.iter().chain(rems.iter()){v.extend_from_slice(id);}
    v
}
/// A record's (parent, h, record), with its shape checked: the version, the kind, the lengths, each list ascending,
/// the adds and rems disjoint, a delta of at most DELTA_CAP ids, and the snapshot cadence against h.
pub fn decode_rec(b:&[u8])->Result<([u8;32],u64,MemberRec),String>{
    if b.len()<REC_FIXED{return Err("member record truncated".into())}
    if b[0]!=REC_VERSION{return Err(format!("member record version {}",b[0]))}
    let kind=b[1];let parent:[u8;32]=b[2..34].try_into().unwrap();let h=u64::from_be_bytes(b[34..42].try_into().unwrap());
    let n=u16::from_be_bytes(b[42..44].try_into().unwrap());let digest:[u8;32]=b[44..76].try_into().unwrap();
    let na=u16::from_be_bytes(b[76..78].try_into().unwrap()) as usize;let nr=u16::from_be_bytes(b[78..80].try_into().unwrap()) as usize;
    if b.len()!=REC_FIXED+20*(na+nr){return Err("member record length".into())}
    let ids:Vec<Id>=b[REC_FIXED..].chunks(20).map(|c|c.try_into().unwrap()).collect();
    let (adds,rems)=ids.split_at(na);
    let step=match kind{
        1=>{if na+nr!=0{return Err("a member snapshot record with a delta".into())}MemberStep::Snapshot}
        0=>{
            if h%SNAPSHOT_EVERY==1{return Err(format!("a member delta at the snapshot height {h}"))}
            if na+nr>DELTA_CAP{return Err("a member delta beyond DELTA_CAP".into())}
            if !ascending(adds)||!ascending(rems){return Err("a member delta that is not sorted".into())}
            if adds.iter().any(|a|rems.binary_search(a).is_ok()){return Err("a member delta adds and removes one id".into())}
            MemberStep::Delta{adds:adds.into(),rems:rems.into()}
        }
        k=>return Err(format!("member record kind {k}")),
    };
    if h==0{return Err("a member record at height 0".into())}
    Ok((parent,h,MemberRec{n,digest,step}))
}
/// A snapshot's set bytes: the ids, 20 B each, ascending.
pub fn encode_set(ids:&[Id])->Vec<u8>{let mut v=Vec::with_capacity(20*ids.len());for id in ids{v.extend_from_slice(id);}v}
/// A snapshot's set from its bytes, checked ascending.
pub fn decode_set(b:&[u8])->Result<Set,String>{
    if b.len()%20!=0{return Err("member set length".into())}
    let ids:Vec<Id>=b.chunks(20).map(|c|c.try_into().unwrap()).collect();
    if !ascending(&ids){return Err("member set not ascending".into())}
    Ok(ids.into())
}

/// A small cache with move-to-front (a cache only: a miss recomputes). O(capacity) per lookup, for capacities of tens.
pub struct SmallLru<K,V>{cap:usize,v:VecDeque<(K,V)>}
impl<K:PartialEq,V:Clone> SmallLru<K,V>{
    pub fn new(cap:usize)->Self{SmallLru{cap:cap.max(1),v:VecDeque::new()}}
    pub fn get(&mut self,k:&K)->Option<V>{let i=self.v.iter().position(|e|e.0==*k)?;let e=self.v.remove(i)?;let x=e.1.clone();self.v.push_front(e);Some(x)}
    pub fn put(&mut self,k:K,x:V){if let Some(i)=self.v.iter().position(|e|e.0==k){self.v.remove(i);}self.v.push_front((k,x));while self.v.len()>self.cap{self.v.pop_back();}}
    pub fn len(&self)->usize{self.v.len()}
    pub fn is_empty(&self)->bool{self.v.is_empty()}
    pub fn cap(&self)->usize{self.cap}
    pub fn clear(&mut self){self.v.clear()}
}
/// A FIFO memo of a capacity in the thousands (hashed).
pub struct Memo<K,V>{cap:usize,map:HashMap<K,V>,order:VecDeque<K>}
impl<K:std::hash::Hash+Eq+Clone,V:Clone> Memo<K,V>{
    pub fn new(cap:usize)->Self{Memo{cap:cap.max(1),map:HashMap::new(),order:VecDeque::new()}}
    pub fn get(&self,k:&K)->Option<V>{self.map.get(k).cloned()}
    pub fn put(&mut self,k:K,x:V){if self.map.insert(k.clone(),x).is_none(){self.order.push_back(k);}while self.order.len()>self.cap{if let Some(o)=self.order.pop_front(){self.map.remove(&o);}}}
    pub fn len(&self)->usize{self.map.len()}
    pub fn is_empty(&self)->bool{self.map.is_empty()}
    pub fn cap(&self)->usize{self.cap}
    pub fn clear(&mut self){self.map.clear();self.order.clear();}
}

/// Where a chain reads the member records and snapshot sets of blocks it does not hold, and the sets of snapshots it
/// holds without their ids: history.redb for a node (`set_member_source`), else the chain's own `MemSets`.
pub trait MemberSets:Send+Sync{
    /// A block inserted into the chain (MemSets keeps its record; a store is written by the node instead).
    fn note(&self,_e:&Entry){}
    /// Blocks removed from the chain by retention (MemSets forgets them).
    fn forget(&self,_hashes:&[[u8;32]]){}
    /// Records below height `h` are no longer needed (MemSets drops them).
    fn prune_below(&self,_h:u64){}
    /// Block `hash`'s (parent, h, record); Ok(None): not held.
    fn record(&self,hash:&[u8;32])->Result<Option<([u8;32],u64,MemberRec)>,Reject>;
    /// Snapshot block `hash`'s set, checked against its record; Ok(None): not held.
    fn set(&self,hash:&[u8;32])->Result<Option<Set>,Reject>;
    /// The data read did not add up: a store is marked damaged (the next open rebuilds it, I1).
    fn damaged(&self,why:&str);
}
/// A bare chain's member records: every record and snapshot set `Chain::insert` gives it, by block hash, pruned with
/// the chain's eviction floor (`Chain::evict_below`) and its retention (`forget`).
#[derive(Default)]
pub struct MemSets{rows:Mutex<HashMap<[u8;32],([u8;32],u64,MemberRec,Option<Set>)>>,pub damaged:std::sync::atomic::AtomicU64}
impl MemSets{
    fn rows(&self)->std::sync::MutexGuard<'_,HashMap<[u8;32],([u8;32],u64,MemberRec,Option<Set>)>>{self.rows.lock().unwrap_or_else(|e|e.into_inner())}
    /// (records, snapshot sets) held.
    pub fn len(&self)->(usize,usize){let r=self.rows();(r.len(),r.values().filter(|x|x.3.is_some()).count())}
    /// The heap the rows hold (tests: the bare chain's term).
    pub fn heap_bytes(&self)->usize{self.rows().values().map(|x|112+x.2.heap_bytes()+x.3.as_ref().map_or(0,|s|20*s.len()+16)).sum()}
}
impl MemberSets for MemSets{
    fn note(&self,e:&Entry){
        let (Some(p),Some(m))=(e.parent,e.members.as_deref()) else{return};
        self.rows().insert(e.hash,(p,e.height,m.rec.clone(),m.ids.clone().filter(|_|m.rec.is_snapshot())));
    }
    fn forget(&self,hashes:&[[u8;32]]){if hashes.is_empty(){return}let mut r=self.rows();for h in hashes{r.remove(h);}}
    fn prune_below(&self,h:u64){let mut r=self.rows();if r.values().any(|x|x.1<h){r.retain(|_,x|x.1>=h);r.shrink_to_fit();}}
    fn record(&self,hash:&[u8;32])->Result<Option<([u8;32],u64,MemberRec)>,Reject>{Ok(self.rows().get(hash).map(|x|(x.0,x.1,x.2.clone())))}
    fn set(&self,hash:&[u8;32])->Result<Option<Set>,Reject>{Ok(self.rows().get(hash).and_then(|x|x.3.clone()))}
    fn damaged(&self,_why:&str){self.damaged.fetch_add(1,std::sync::atomic::Ordering::Relaxed);}
}

/// owner(nonce) of one stored block: recomputed from its member record (`chain::LazyOwners`, `chain::CheckedOwners`), or
/// from a slice of ranges (the test vectors). An Err is the node's own storage (`Reject::Local`), never a verdict.
pub trait NonceOwners{fn owner(&self,n:u64)->Result<Option<Address>,Reject>;}
/// Ranges (id, start, end exclusive), as the test vectors give them (and FN-3a's verdicts read the stored ones); never Err.
pub struct StoredRanges<'a>(pub &'a [(Address,u128,u128)]);
impl NonceOwners for StoredRanges<'_>{fn owner(&self,n:u64)->Result<Option<Address>,Reject>{Ok(crate::chain::range_owner(self.0,u128::from(n)))}}
impl NonceOwners for Vec<(Address,u128,u128)>{fn owner(&self,n:u64)->Result<Option<Address>,Reject>{StoredRanges(self).owner(n)}}

// ---------------- FN-3b: the oracle's default and its capture (tests) ----------------
/// Whether chains created from now on run the oracle (`chain::Chain::member_oracle`). Off: the oracle costs O(M) CPU per
/// import, re-execution and evidence read with no timing bound (A13-DERIVATION, CONDITIONS `cpu-and-timing`), so a node
/// never runs it; a test switches it on (`test_oracle`, or `Chain::set_member_oracle` for one chain).
static ORACLE_DEFAULT:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
/// The capture: each validated block's assignment (keccak of its nonce ranges, `ranges_digest`) by (the chain's spec id,
/// the block hash), as `Chain::h_full` validated it. None: not capturing.
static CAPTURE:Mutex<Option<HashMap<([u8;32],[u8;32]),[u8;32]>>>=Mutex::new(None);
fn capture_map()->std::sync::MutexGuard<'static,Option<HashMap<([u8;32],[u8;32]),[u8;32]>>>{CAPTURE.lock().unwrap_or_else(|e|e.into_inner())}
/// The oracle's default for chains created from now on (`Chain::new`).
pub fn oracle_default()->bool{ORACLE_DEFAULT.load(std::sync::atomic::Ordering::SeqCst)}
/// Tests: switches the oracle on for every chain created from now on in this process (bare chains, and a node's chain at
/// its open, so its replay and its checkpoint restore run under it too), and starts the capture: from now on each block
/// a chain with the oracle on validates (`Chain::h_full`, and a re-execution, `Chain::reexec`) records its assignment,
/// the nonce ranges FN-3a stored on the entry, so that the cold sweep (`Chain::member_sweep`) can compare a
/// recomputation with what the block was validated under after the ranges are gone, across restarts and between the
/// chains of one test. Never switched off again: it only adds checks. The capture grows by one row a validated block,
/// a test instrument outside A13 (a node never captures: the oracle is off).
pub fn test_oracle(){ORACLE_DEFAULT.store(true,std::sync::atomic::Ordering::SeqCst);let mut c=capture_map();if c.is_none(){*c=Some(HashMap::new());}}
/// Whether the capture is on.
pub fn capturing()->bool{capture_map().is_some()}
/// Records `digest` as block `block`'s assignment on the chain with spec id `chain`: None when not capturing; Some(true)
/// when new or equal to what was captured before (a block's assignment is a function of the block, so every chain and
/// every re-execution must agree); Some(false) when it differs.
pub fn capture(chain:&[u8;32],block:&[u8;32],digest:[u8;32])->Option<bool>{
    let mut c=capture_map();let m=c.as_mut()?;Some(*m.entry((*chain,*block)).or_insert(digest)==digest)
}
/// The captured assignment of block `block` on the chain with spec id `chain`.
pub fn captured(chain:&[u8;32],block:&[u8;32])->Option<[u8;32]>{capture_map().as_ref()?.get(&(*chain,*block)).copied()}
/// keccak over an assignment's nonce ranges in order, each owner (20 B), start and end exclusive (16 B each, big-endian).
pub fn ranges_digest(a:&crate::assignment::Assignment)->[u8;32]{
    let mut b=Vec::with_capacity(52*a.ranges().len());
    for r in a.ranges(){b.extend_from_slice(&r.id());b.extend_from_slice(&r.start().to_be_bytes());b.extend_from_slice(&r.end_exclusive().to_be_bytes());}
    keccak256(&b).0
}

// ---------------- FN-1b: what membership() reads of a parent state (node-local) ----------------
/// FN-1b: the fields of one MinerRecord that `chain::Chain::membership_one_pass` reads (`apply::miner_record`): tomb,
/// last_share and grace_start for the ParentMember, bond_state and whether the bond is below B_reg/4 for the accruing
/// mark. None where the record is absent (its word 0 is zero), as `miner_record` gives it.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct BaseRec{pub tomb:bool,pub bond_state:u8,pub last_share:u64,pub grace_start:u64,pub below_quarter:bool}
/// FN-1b (C6, node-local): everything `chain::Chain::membership_one_pass` reads of a state, read once: the ActList in
/// order and, for each distinct id on it, the record fields it reads (`BaseRec`), and the record's base slot
/// (`apply::record_slot`) of each. `chain::Chain::membership_from_base` computes A_h* from it with the same code the one pass
/// runs, so a base equal to the one read from the state gives the same membership, the same error and the same order.
/// A child's base is derived from its parent's (`derive`) by re-reading exactly what the block wrote: every registry
/// slot of the block's write set (a superset of the slots whose value differs, and in store mode the very slots its
/// stored version was updated with) that is the list's length, one of its elements, or one of the four words of the
/// record of an id on the parent's list. A cache value only (keyed by the state's identity, `chain::CtxCache`), never a
/// validity input; the one-pass reading stays the oracle (`Chain::member_oracle`).
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct MemberBase{
    /// The ActList in order: registry slot 1 is its length, keccak(6) + i its element i.
    pub list:Vec<Id>,
    /// Each distinct id of `list`: the record fields read (None: no record).
    recs:HashMap<Id,Option<BaseRec>>,
    /// The record base slot (`apply::record_slot`) of each id of `recs`.
    slots:HashMap<U256,Id>,
}
/// FN-1b: what a derivation read (the counters of `chain::MemberCounters`).
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub struct DeriveReads{pub list:u64,pub records:u64}
/// The ActList's element base, keccak256(6) (`apply::active_list`'s array_slot(6)).
fn act_base()->U256{static B:std::sync::OnceLock<U256>=std::sync::OnceLock::new();*B.get_or_init(||U256::from_be_bytes(keccak256(U256::from(6).to_be_bytes::<32>()).0))}
fn act_len(st:&dyn pocol_lp4_execution::view::View)->u64{st.slot(&pocol_lp4_execution::apply::registry(),&U256::from(1)).as_limbs()[0]}
fn act_at(st:&dyn pocol_lp4_execution::view::View,i:u64)->Id{
    Address::from_word(revm::primitives::B256::from(st.slot(&pocol_lp4_execution::apply::registry(),&(act_base()+U256::from(i))))).0.0
}
fn base_rec(st:&dyn pocol_lp4_execution::view::View,id:&Id,quarter:U256)->Option<BaseRec>{
    pocol_lp4_execution::apply::miner_record(st,Address::from(*id)).map(|r|BaseRec{tomb:r.tomb,bond_state:r.bond_state,last_share:r.last_share,grace_start:r.grace_start,below_quarter:r.bond<quarter})
}
impl MemberBase{
    /// The whole read: the ActList (`apply::active_list`) and each distinct id's record (`apply::miner_record`), what
    /// the one pass reads. `quarter` is B_reg/4.
    pub fn read(st:&dyn pocol_lp4_execution::view::View,quarter:U256)->MemberBase{
        let list:Vec<Id>=pocol_lp4_execution::apply::active_list(st).into_iter().map(|a|a.0.0).collect();
        let (mut recs,mut slots)=(HashMap::with_capacity(list.len()),HashMap::with_capacity(list.len()));
        for id in &list{if !recs.contains_key(id){recs.insert(*id,base_rec(st,id,quarter));slots.insert(pocol_lp4_execution::apply::record_slot(Address::from(*id)),*id);}}
        MemberBase{list,recs,slots}
    }
    /// The record fields of an id of the list.
    pub fn rec(&self,id:&Id)->Option<BaseRec>{self.recs.get(id).copied().flatten()}
    /// The ids whose record is held (the distinct ids of the list).
    pub fn held(&self)->usize{self.recs.len()}
    /// The base of a child state `st` from this one (its parent's), given the registry slots the block wrote
    /// (`written`: every slot whose value can differ between the two states). Re-read from `st`: the length when slot 1
    /// was written, each written element below the new length and every element past the old one; the record of each id
    /// one of whose four words was written, and of each id new to the list. An id that left the list is dropped. Every
    /// other value is the parent's, which the child holds unchanged.
    pub fn derive(&self,st:&dyn pocol_lp4_execution::view::View,written:&[U256],quarter:U256)->(MemberBase,DeriveReads){
        let (base,one)=(act_base(),U256::from(1));let mut reads=DeriveReads::default();
        let mut len_written=false;let mut at:Vec<u64>=Vec::new();let mut dirty:Vec<Id>=Vec::new();
        for s in written{
            if *s==one{len_written=true;}
            if *s>=base{let d=*s-base;if d<=U256::from(u64::MAX){at.push(d.as_limbs()[0]);}}
            for k in 0..4u64{if let Some(id)=self.slots.get(&s.wrapping_sub(U256::from(k))){dirty.push(*id);}}
        }
        let old=self.list.len();
        let n=if len_written{reads.list+=1;act_len(st)}else{old as u64};
        let keep=(n.min(old as u64)) as usize;
        let mut list=self.list[..keep].to_vec();let mut left:Vec<Id>=self.list[keep..].to_vec();
        for i in &at{if *i<keep as u64{let i=*i as usize;let x=act_at(st,i as u64);reads.list+=1;if x!=list[i]{left.push(list[i]);list[i]=x;}}}
        for i in keep as u64..n{list.push(act_at(st,i));reads.list+=1;}
        let (mut recs,mut slots)=(self.recs.clone(),self.slots.clone());
        left.sort_unstable();left.dedup();
        if !left.is_empty(){
            // An id that left one position may hold another (a swap-remove moves the last id): only one the new list does
            // not hold is dropped.
            let set:Option<std::collections::HashSet<Id>>=(left.len()>64).then(||list.iter().copied().collect());
            let on=|id:&Id|set.as_ref().map_or_else(||list.contains(id),|s|s.contains(id));
            for id in &left{if !on(id)&&recs.remove(id).is_some(){slots.remove(&pocol_lp4_execution::apply::record_slot(Address::from(*id)));}}
        }
        dirty.sort_unstable();dirty.dedup();
        for id in &dirty{if let Some(x)=recs.get_mut(id){*x=base_rec(st,id,quarter);reads.records+=1;}}
        for id in &list{if !recs.contains_key(id){recs.insert(*id,base_rec(st,id,quarter));slots.insert(pocol_lp4_execution::apply::record_slot(Address::from(*id)),*id);reads.records+=1;}}
        (MemberBase{list,recs,slots},reads)
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    struct Rng(u64);
    impl Rng{fn next(&mut self)->u64{self.0^=self.0<<13;self.0^=self.0>>7;self.0^=self.0<<17;self.0}}
    fn id(i:u64)->Id{let mut a=[0x22u8;20];a[12..].copy_from_slice(&i.to_be_bytes());a}
    fn set(r:&mut Rng,max:u64,p:u64)->Vec<Id>{(0..max).filter(|_|r.next()%100<p).map(id).collect()}
    #[test] fn diff_then_apply_gives_the_target_and_the_digest_steps(){
        let mut r=Rng(7);
        for _ in 0..300{
            let (a,b)=(set(&mut r,200,40),set(&mut r,200,40));let (adds,rems)=diff(&a,&b);
            assert!(ascending(&adds)&&ascending(&rems));
            assert_eq!(apply(&a,&[(&adds,&rems)]).unwrap(),b);
            assert_eq!(digest_step(&digest_of(&a),&adds,&rems),digest_of(&b));
        }
    }
    #[test] fn the_overlay_is_sequential_application(){
        // Chains of deltas, including an id added, removed and added again, against applying them one at a time.
        let mut r=Rng(11);
        for _ in 0..100{
            let mut sets=vec![set(&mut r,64,50)];for _ in 0..8{sets.push(set(&mut r,64,50));}
            let x=id(5);sets[2].retain(|i|*i!=x);sets[3].push(x);sets[3].sort();sets[3].dedup();sets[4].retain(|i|*i!=x);sets[5].push(x);sets[5].sort();sets[5].dedup();
            let deltas:Vec<(Vec<Id>,Vec<Id>)>=sets.windows(2).map(|w|diff(&w[0],&w[1])).collect();
            let steps:Vec<(&[Id],&[Id])>=deltas.iter().map(|(a,b)|(&a[..],&b[..])).collect();
            assert_eq!(apply(&sets[0],&steps).unwrap(),*sets.last().unwrap());
            let mut seq=sets[0].clone();for s in &steps{seq=apply(&seq,&[*s]).unwrap();}assert_eq!(seq,*sets.last().unwrap());
        }
    }
    #[test] fn a_delta_that_does_not_fit_the_set_is_refused(){
        let base=vec![id(1),id(2),id(3)];
        assert!(apply(&base,&[(&[id(2)],&[])]).is_err(),"adds an id the set holds");
        assert!(apply(&base,&[(&[],&[id(9)])]).is_err(),"removes an id the set does not hold");
        assert!(apply(&base,&[(&[],&[id(2)]),(&[],&[id(2)])]).is_err(),"removes twice");
        assert_eq!(apply(&base,&[(&[],&[id(2)]),(&[id(2)],&[])]).unwrap(),base);
        assert!(apply(&base,&[(&[id(9),id(8)],&[])]).is_err(),"unsorted");
    }
    #[test] fn the_snapshot_rule(){
        assert!(is_snapshot(1,0)&&is_snapshot(257,0)&&is_snapshot(513,0));
        assert!(!is_snapshot(256,0)&&!is_snapshot(2,DELTA_CAP)&&is_snapshot(2,DELTA_CAP+1));
        let s_p:Vec<Id>=(0..40).map(id).collect();let d=digest_of(&s_p);
        let mut s_b=s_p.clone();s_b.truncate(40-DELTA_CAP);let s_b:Set=s_b.into();
        let (r,ids)=record_for(256,Some((&s_p,&d)),&s_b);assert!(!r.is_snapshot()&&ids.is_none()&&r.delta_len()==DELTA_CAP);assert_eq!(r.digest,digest_of(&s_b));
        let s_c:Set=s_p[..40-DELTA_CAP-1].to_vec().into();
        let (r,ids)=record_for(256,Some((&s_p,&d)),&s_c);assert!(r.is_snapshot()&&ids.as_deref()==Some(&s_c[..]),"33 changes are a snapshot");
        let (r,_)=record_for(257,Some((&s_p,&d)),&s_b);assert!(r.is_snapshot(),"the cadence");
        let (r,_)=record_for(2,None,&s_b);assert!(r.is_snapshot(),"no parent set");
        assert_eq!(r.n as usize,s_b.len());assert_eq!(r.digest,digest_of(&s_b));
    }
    #[test] fn records_round_trip_and_bad_shapes_are_refused(){
        let p=[9u8;32];
        let d=MemberRec{n:7,digest:[3;32],step:MemberStep::Delta{adds:vec![id(1),id(4)].into(),rems:vec![id(2)].into()}};
        let b=encode_rec(&p,300,&d);assert_eq!(b.len(),REC_FIXED+60);assert_eq!(decode_rec(&b).unwrap(),(p,300,d.clone()));
        let s=MemberRec{n:2,digest:[4;32],step:MemberStep::Snapshot};assert_eq!(decode_rec(&encode_rec(&p,257,&s)).unwrap(),(p,257,s));
        assert!(decode_rec(&encode_rec(&p,257,&d)).is_err(),"a delta at the cadence");
        assert!(decode_rec(&encode_rec(&p,0,&MemberRec::genesis())).is_err(),"height 0");
        let mut x=b.clone();x[0]=2;assert!(decode_rec(&x).is_err());
        let mut x=b.clone();x[1]=7;assert!(decode_rec(&x).is_err());
        assert!(decode_rec(&b[..b.len()-1]).is_err());
        let u=MemberRec{n:7,digest:[3;32],step:MemberStep::Delta{adds:vec![id(4),id(1)].into(),rems:vec![].into()}};assert!(decode_rec(&encode_rec(&p,300,&u)).is_err(),"unsorted");
        let o=MemberRec{n:7,digest:[3;32],step:MemberStep::Delta{adds:vec![id(1)].into(),rems:vec![id(1)].into()}};assert!(decode_rec(&encode_rec(&p,300,&o)).is_err(),"overlap");
        let big=MemberRec{n:70,digest:[3;32],step:MemberStep::Delta{adds:(0..33).map(id).collect::<Vec<_>>().into(),rems:vec![].into()}};assert!(decode_rec(&encode_rec(&p,300,&big)).is_err(),"beyond DELTA_CAP");
        let ids:Vec<Id>=(0..5).map(id).collect();assert_eq!(&*decode_set(&encode_set(&ids)).unwrap(),&ids[..]);
        assert!(decode_set(&encode_set(&[id(2),id(1)])).is_err());assert!(decode_set(&[0u8;21]).is_err());
    }
    #[test] fn the_caches_keep_their_capacity(){
        let mut l=SmallLru::new(3);for i in 0..10u64{l.put(i,i);assert!(l.len()<=3);}
        assert_eq!(l.get(&9),Some(9));assert_eq!(l.get(&0),None);l.get(&7);l.put(10,10);assert_eq!(l.get(&8),None,"the least recently used goes");assert_eq!(l.get(&7),Some(7));
        let mut m=Memo::new(5);for i in 0..100u64{m.put(i,i);assert!(m.len()<=5);}assert_eq!(m.get(&99),Some(99));assert_eq!(m.get(&94),None);
    }
    #[test] fn the_slice_owner_is_range_owner(){
        let a=|i:u8|Address::from([i;20]);
        let r=vec![(a(1),0u128,10u128),(a(2),10,10),(a(3),10,25)];
        for n in 0..30u64{assert_eq!(StoredRanges(&r).owner(n).unwrap(),crate::chain::range_owner(&r,u128::from(n)));assert_eq!(r.owner(n).unwrap(),crate::chain::range_owner(&r,u128::from(n)));}
        assert_eq!(r.owner(10).unwrap(),Some(a(3)));assert_eq!(r.owner(25).unwrap(),None);
    }
}
