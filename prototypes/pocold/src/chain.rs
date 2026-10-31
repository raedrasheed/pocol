//! Consensus state: block tree, H-pre/H-full validation (consensus.md), template
//! construction (signed / Build_fb), ParentResult (D142) and D133 fork choice.
use std::collections::{HashMap,BTreeMap};
use crate::{assignment::{self,Assignment,DrawWeights,Membership,NonceMode,ParentMember,ParentView,ShareCount},asert,hashes,header::Header,timing::AttemptWindow};
use pocol_lp4_execution::{admission::{Admitted,NodeLocal},apply::{self,BlockInput,BlockOutput,EvidenceEffect,Genesis,ParentResult,Proposer},fees,profile::ProfileIdentity,txsign::Key};
use revm::{InMemoryDB,primitives::{Address,B256,U256}};
use crate::types::*;

/// Longest stored rejection reason (A13 accounting).
pub const REASON_MAX:usize=256;
pub fn truncate(why:&str)->String{if why.len()<=REASON_MAX{return why.to_string()}let mut n=REASON_MAX;while !why.is_char_boundary(n){n-=1}why[..n].to_string()}
/// governance.md R_keep: a non-canonical block this far below the head is removed (A13).
pub const R_KEEP:u64=2048;
/// Unprotected non-canonical blocks kept as METADATA (local retention, not validity; consensus.md:29).
/// D-REORG-REDB (owner-approved): fork choice (`better`, `rebuild_canonical`, the LCA walk) reads only
/// metadata, and `retain` offloads every journaled non-canonical block's body, receipts, tx hashes and
/// encoded header (they are in the journal). R_keep entries: D118 keeps at most one non-canonical block
/// per height, so a whole window of competing blocks fits. Beyond it the lowest-work unprotected leaf is
/// evicted first; an evicted branch is re-fetched by a sync session from the fork point (locator) when a
/// peer serves a heavier chain (consensus.md §10: "fetched by sync"), and a session's blocks are
/// protected from this cap (`protect_max`). A13-DERIVATION §4 item 18.
pub const SIDE_BLOCKS_MAX:usize=R_KEEP as usize;
/// Hashes of side blocks removed by retention, remembered so that a copy received again is refused
/// before it is validated or journaled (a branch longer than SIDE_BLOCKS_MAX would otherwise be
/// re-fetched, re-executed and re-journaled every sync turn). FIFO, a cache only.
pub const EVICTED_MAX:usize=1024;
/// Bound of the invalid-block cache (A05). A forgotten entry is only re-validated.
pub const INVALID_MAX:usize=4096;
/// C-1 E1: the most diffs a parent's trie is derived through from the head's. A parent further away (a
/// deep side branch, an old rebuild root) has its root computed from the whole state, as before E1: the
/// derivation would cost more than that, and its distinct nodes are bounded by this many blocks' writes.
pub const TRIE_PATH_MAX:usize=64;

/// Immutable chain configuration: cfg = {chainId, genesisHash, ForkSchedule, CP, M_0}.
/// Constructible only from a ParamGate admission (A02): nothing boots an unadmitted profile.
pub struct Spec{admitted:Admitted}
impl Spec{
    pub fn new(admitted:Admitted)->Self{Self{admitted}}
    pub fn profile(&self)->&ProfileIdentity{self.admitted.profile()}
    pub fn genesis(&self)->&Genesis{self.admitted.genesis()}
    pub fn genesis_hash(&self)->[u8;32]{self.admitted.genesis_hash()}
    pub fn spec_bytes(&self)->&[u8]{self.admitted.spec_bytes()}
    /// The node-local timing admitted with the profile (Φ=2φ, Δ_rel, Γ).
    pub fn local(&self)->&NodeLocal{self.admitted.local()}
    pub fn admission(&self)->&Admitted{&self.admitted}
    pub fn cp(&self)->&pocol_lp4_execution::profile::ConsensusParams{self.profile().cp()}
    pub fn chain_id(&self)->u64{self.profile().spec().chain_id}
    pub fn target_g(&self)->Fixed{Fixed::from_be_slice(&self.profile().cp().target_g.to_be_bytes()).unwrap()}
    pub fn active_version(&self,h:u64)->u16{self.profile().forks().active_version(h)}
    /// The genesis CP remains immutable. Consensus v2 activates the complete u64
    /// nonce domain only for the block being built or validated, including replay.
    pub fn effective_nonce_mode(&self,h:u64)->NonceMode{
        if self.active_version(h)==2||self.cp().nonceMode==1{NonceMode::Unbounded}else{NonceMode::Bounded}
    }
}

#[derive(Clone,Debug)]
pub struct Entry{
    pub hash:[u8;32],
    pub header:Header,
    pub body:Body,
    pub height:u64,
    pub parent:Option<[u8;32]>,
    pub total_work:U256,
    pub import_seq:u64,
    pub signed:bool,
    pub ts:u64,
    pub target:Fixed,
    pub base_fee:U256,
    pub gas_used:u64,
    pub seal:[u8;32],
    /// C-1 E3: shared, so that a copy of the entry (production, a view, a rebuild's rolling entry) does
    /// not copy the state.
    pub state:std::sync::Arc<InMemoryDB>,
    pub is_final:bool,
    /// ParentResult describing THIS block, consumed by its children (D142).
    pub result:Option<ParentResult>,
    pub receipts:Vec<Vec<u8>>,
    pub tx_hashes:Vec<B256>,
    pub cum_txs:u64,
    pub cum_tx_bytes:u64,
    /// Charged log bytes up to and including this block (LOG_CAP, A03).
    pub cum_log_bytes:u64,
    /// R21: state dropped outside the K_hist window; rebuilt by re-execution on demand.
    pub pruned:bool,
    /// C-1 S3a: `state` holds the full state. A state that is not pruned but not full is materialized
    /// from a full one through the diffs (`Chain::state_of`): only the head, genesis and the side states
    /// within side_max are kept full.
    pub full:bool,
    /// C-1 S3a: this block's state as a diff from its parent's (set when it is validated or re-executed;
    /// kept for canonical blocks above the K_hist cut and for side blocks that keep a state).
    pub diff:Option<std::sync::Arc<crate::statediff::StateDiff>>,
    /// C-1 S4: the header's heap fields, the ParentResult and the nonce ranges were dropped (an old
    /// canonical block, deeper than `Chain::slim_depth`); `Chain::unslim` restores them exactly from the
    /// journal record and history.redb.
    pub slim:bool,
    /// Draw(h,0) for this block's height on its branch (evidence type 1 DrawIndex).
    pub drawn:Address,
    /// This block's nonce ranges (owner lookup for evidence without old state).
    pub ranges:Vec<(Address,u128,u128)>,
    /// D-REORG-REDB off-heap block store: the journal frame offset of this block's record, set by
    /// the node after `journal.append` (and recomputed on replay). `None` for a bare chain with no
    /// journal and for genesis (never journaled). Only a block with `body_off = Some` may be offloaded.
    pub body_off:Option<u64>,
    /// When true, `body`, `receipts`, `tx_hashes` and `header.encoded` have been dropped (empty) and
    /// are read on demand from the journal at `body_off`. State residence stays governed by `pruned`.
    pub offloaded:bool,
    /// C-1 E1: the trie of this block's state, for its children's incremental state roots. Only the head
    /// keeps one after `settle`; any other parent's trie is derived from the head's (`Chain::trie_for`).
    pub trie:Option<pocol_lp4_execution::state_trie::StateTrie>,
    /// C-1 E2: the summary of this block's state, for its children's checks (dropped when slimmed).
    pub summary:Option<Box<pocol_lp4_execution::state_summary::Summary>>,
    /// C-1 E4: this block's state as a version pinned in the node store (`Chain::trie_store`), read in place
    /// (`Chain::view`); `state` is then empty. The pin goes with the last copy of the entry.
    pub stored:Option<std::sync::Arc<Pinned>>,
}

/// C-1 E4: a state version pinned in the node store. The pin is released when the last holder goes: the
/// chain's entry and any copy of it (production, the pool, a rolling rebuild), as the `Arc` of a whole
/// state kept it alive before.
pub struct Pinned{pub trie:pocol_lp4_execution::node_trie::NodeTrie,pub hashes:HashMap<U256,B256>,store:std::sync::Arc<dyn pocol_lp4_execution::node_trie::TrieStore>}
impl Pinned{
    /// Commits the child's new nodes and pins its root.
    pub fn new(store:std::sync::Arc<dyn pocol_lp4_execution::node_trie::TrieStore>,c:StoredChild)->Result<Pinned,Reject>{
        store.commit(&c.batch,c.trie.world).map_err(|e|Reject::Local(format!("state store: {e}")))?;Ok(Pinned{trie:c.trie,hashes:c.hashes,store})
    }
    /// C-1 E4.4: a version already stored (a checkpoint's, restored at open) pinned once more for its entry.
    pub fn adopt(store:std::sync::Arc<dyn pocol_lp4_execution::node_trie::TrieStore>,root:pocol_lp4_execution::node_trie::Id,hashes:HashMap<U256,B256>)->Result<Pinned,Reject>{
        store.commit(&Default::default(),Some(root)).map_err(|e|Reject::Local(format!("state store: {e}")))?;
        Ok(Pinned{trie:pocol_lp4_execution::node_trie::NodeTrie{world:Some(root)},hashes,store})
    }
    pub fn view(&self)->pocol_lp4_execution::node_trie::TrieState<'_>{pocol_lp4_execution::node_trie::TrieState{store:&*self.store,trie:self.trie,block_hashes:&self.hashes}}
}
impl std::fmt::Debug for Pinned{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{write!(f,"Pinned({:?})",self.trie.world.map(B256::from))}}
impl Drop for Pinned{fn drop(&mut self){if let Some(r)=self.trie.world{if let Err(e)=self.store.unpin(&r){eprintln!("C-1 E4: releasing a state version failed: {e}")}}}}
/// C-1 E4: an entry's state as a value: its whole state or its pinned version (shared, not copied); a
/// `View`, so the pool keeps its layer over it and production simulates on an overlay over it.
#[derive(Clone)]
pub enum StateHandle{Mem(std::sync::Arc<InMemoryDB>),Stored(std::sync::Arc<Pinned>)}
impl StateHandle{
    pub fn of(e:&Entry)->Self{match &e.stored{Some(p)=>StateHandle::Stored(p.clone()),None=>StateHandle::Mem(e.state.clone())}}
    /// The state as a whole `InMemoryDB` (tests and inspection: a stored version is read whole).
    pub fn whole(&self)->InMemoryDB{match self{StateHandle::Mem(m)=>InMemoryDB::clone(m),StateHandle::Stored(p)=>{let mut w=whole_state(&p.view());w.block_hashes=p.hashes.clone();w}}}
}
macro_rules! handle_view{($s:ident,$m:ident,$e:expr)=>{match $s{StateHandle::Mem(m)=>{let $m:&dyn pocol_lp4_execution::view::View=&**m;$e}StateHandle::Stored(p)=>{let t=p.view();let $m:&dyn pocol_lp4_execution::view::View=&t;$e}}}}
impl pocol_lp4_execution::view::View for StateHandle{
    fn account(&self,a:&Address)->Option<pocol_lp4_execution::view::Acc>{handle_view!(self,v,v.account(a))}
    fn slot(&self,a:&Address,k:&U256)->U256{handle_view!(self,v,v.slot(a,k))}
    fn each_slot(&self,a:&Address,f:&mut dyn FnMut(&U256,&U256)){handle_view!(self,v,v.each_slot(a,f))}
    fn contract(&self,h:&B256)->Option<revm::primitives::Bytecode>{handle_view!(self,v,v.contract(h))}
    fn block_hash(&self,n:u64)->B256{handle_view!(self,v,v.block_hash(n))}
    fn each_account(&self,f:&mut dyn FnMut(&Address,pocol_lp4_execution::view::Acc)){handle_view!(self,v,v.each_account(f))}
}
/// A view read whole into an `InMemoryDB`: every existing account with its code and nonzero slots (the
/// BLOCKHASH window is the caller's to set).
pub fn whole_state(v:&dyn pocol_lp4_execution::view::View)->InMemoryDB{
    let mut db=InMemoryDB::default();let mut accts=Vec::new();v.each_account(&mut |a,acc|{if acc.exists(){accts.push((*a,acc.info))}});
    for (a,mut info) in accts{
        if info.code.is_none(){info.code=v.contract(&info.code_hash);}
        db.insert_account_info(a,info);let mut slots=Vec::new();v.each_slot(&a,&mut |k,x|{if *x!=U256::ZERO{slots.push((*k,*x))}});
        for (k,x) in slots{db.insert_account_storage(a,k,x).expect("in memory");}
    }
    db
}
/// C-1 E4: a child state as executing a block on a stored parent gives it, before it is stored: its trie
/// version, the batch with the version's new nodes, and its BLOCKHASH window.
pub struct StoredChild{pub trie:pocol_lp4_execution::node_trie::NodeTrie,pub batch:pocol_lp4_execution::node_trie::Batch,pub hashes:HashMap<U256,B256>}
/// A child entry's state fields (`Chain::child_state`).
pub struct ChildState{pub state:std::sync::Arc<InMemoryDB>,pub diff:Option<std::sync::Arc<crate::statediff::StateDiff>>,pub trie:Option<pocol_lp4_execution::state_trie::StateTrie>,
    pub summary:pocol_lp4_execution::state_summary::Summary,pub receipts:Vec<Vec<u8>>,pub stored:Option<std::sync::Arc<Pinned>>}
/// What executing a block gives: a whole child state (`BlockOutput`), or a stored one (C-1 E4).
pub enum Executed{Mem(BlockOutput),Stored{run:Box<apply::BlockRun>,child:StoredChild,root:B256}}
impl Executed{
    pub fn state_root(&self)->B256{match self{Executed::Mem(o)=>o.state_root,Executed::Stored{root,..}=>*root}}
    pub fn transactions_root(&self)->B256{match self{Executed::Mem(o)=>o.transactions_root,Executed::Stored{run,..}=>run.transactions_root}}
    pub fn receipts_root(&self)->B256{match self{Executed::Mem(o)=>o.receipts_root,Executed::Stored{run,..}=>run.receipts_root}}
    pub fn evidence_root(&self)->B256{match self{Executed::Mem(o)=>o.evidence_root,Executed::Stored{run,..}=>run.evidence_root}}
    pub fn logs_bloom(&self)->&[u8;256]{match self{Executed::Mem(o)=>&o.logs_bloom,Executed::Stored{run,..}=>&run.logs_bloom}}
    pub fn gas_used(&self)->u64{match self{Executed::Mem(o)=>o.gas_used,Executed::Stored{run,..}=>run.gas_used}}
    pub fn is_final(&self)->bool{match self{Executed::Mem(o)=>o.is_final,Executed::Stored{run,..}=>run.is_final}}
    pub fn log_bytes(&self)->u64{match self{Executed::Mem(o)=>o.log_bytes,Executed::Stored{run,..}=>run.log_bytes}}
    pub fn receipts(&self)->&[Vec<u8>]{match self{Executed::Mem(o)=>&o.receipts,Executed::Stored{run,..}=>&run.receipts}}
    pub fn summary(&self)->pocol_lp4_execution::state_summary::Summary{match self{Executed::Mem(o)=>o.summary,Executed::Stored{run,..}=>run.summary}}
}
/// C-1 E4: a stored block's state read in place: whole in memory, materialized through the diffs, or a
/// version in the node store.
pub enum StateView<'a>{Mem(&'a InMemoryDB),Owned(Box<InMemoryDB>),Stored(pocol_lp4_execution::node_trie::TrieState<'a>)}
impl StateView<'_>{
    pub fn v(&self)->&dyn pocol_lp4_execution::view::View{match self{StateView::Mem(m)=>*m,StateView::Owned(m)=>&**m,StateView::Stored(t)=>t}}
}
impl pocol_lp4_execution::view::View for StateView<'_>{
    fn account(&self,a:&Address)->Option<pocol_lp4_execution::view::Acc>{self.v().account(a)}
    fn slot(&self,a:&Address,k:&U256)->U256{self.v().slot(a,k)}
    fn each_slot(&self,a:&Address,f:&mut dyn FnMut(&U256,&U256)){self.v().each_slot(a,f)}
    fn contract(&self,h:&B256)->Option<revm::primitives::Bytecode>{self.v().contract(h)}
    fn block_hash(&self,n:u64)->B256{self.v().block_hash(n)}
    fn each_account(&self,f:&mut dyn FnMut(&Address,pocol_lp4_execution::view::Acc)){self.v().each_account(f)}
}

/// Source of a block's bytes for the off-heap store: given a journal frame offset (an `Entry.body_off`),
/// returns (headerBytes, bodyBytes, parentResultBytes). Implemented by the node over `store::Journal`
/// (`read_at`); a bare chain passes `None` and never offloads, so it is never called there.
pub trait BlockBytes{fn record_at(&self,off:u64)->Result<(Vec<u8>,Vec<u8>,Vec<u8>),Reject>;
    /// C-1 S4: a block's stored nonce ranges (history.redb), for restoring a slimmed entry.
    fn ranges_of(&self,_hash:&[u8;32])->Result<Option<Vec<(Address,u128,u128)>>,Reject>{Ok(None)}}
/// C-1 S4: drops what a slimmed entry does not keep: the header's heap fields, the ParentResult and the
/// nonce ranges. Fork choice, contexts and evidence read only the rest at these depths.
pub(crate) fn slim(e:&mut Entry){let h=&mut e.header;h.logs_bloom=Vec::new();h.proposer=Vec::new();h.sig=Vec::new();h.shares=Vec::new();h.ut_raw=Vec::new();
    h.share_list_raw=Vec::new();h.encoded=Vec::new();e.result=None;e.ranges=Vec::new();e.summary=None;e.slim=true;}
/// The ParentResult of block `hash` from its journal record's result bytes (node::result_bytes).
pub fn decode_result(rb:&[u8],hash:&[u8;32])->Result<Option<ParentResult>,Reject>{
    let bad=||Reject::Block("result bytes".into());
    let it=crate::rlp::decode(rb).map_err(|_|bad())?;let l=it.list().ok_or_else(bad)?;
    if l.is_empty(){return Ok(None)}
    if l.len()!=2{return Err(bad())}
    let w=l[0].bytes().ok_or_else(bad)?;if w.len()!=20{return Err(bad())}
    let mut shares=Vec::new();
    for s in l[1].list().ok_or_else(bad)?{let q=s.list().ok_or_else(bad)?;if q.len()!=2{return Err(bad())}
        let n=q[0].bytes().ok_or_else(bad)?;let o=q[1].bytes().ok_or_else(bad)?;if n.len()!=8||o.len()!=20{return Err(bad())}
        shares.push((u64::from_be_bytes(n.try_into().unwrap()),Address::from_slice(o)));}
    Ok(Some(ParentResult{block_hash:B256::from(*hash),winner:Address::from_slice(w),shares}))
}
/// Drops a journaled block's bytes from memory (D-REORG-REDB): body, receipts, tx hashes and the
/// encoded header. Fork choice, child contexts and evidence ancestry read only the metadata kept.
pub(crate) fn offload(e:&mut Entry){e.body=Body::default();e.receipts=Vec::new();e.tx_hashes=Vec::new();e.header.encoded=Vec::new();e.offloaded=true;}
impl Entry{
    /// A copy without the executed state (an empty, pruned state), for rolling re-execution and views.
    pub fn meta_clone(&self)->Entry{
        Entry{hash:self.hash,header:self.header.clone(),body:self.body.clone(),height:self.height,parent:self.parent,total_work:self.total_work,import_seq:self.import_seq,
            signed:self.signed,ts:self.ts,target:self.target,base_fee:self.base_fee,gas_used:self.gas_used,seal:self.seal,state:empty_state(),is_final:self.is_final,
            result:self.result.clone(),receipts:self.receipts.clone(),tx_hashes:self.tx_hashes.clone(),cum_txs:self.cum_txs,cum_tx_bytes:self.cum_tx_bytes,
            cum_log_bytes:self.cum_log_bytes,pruned:true,full:false,diff:self.diff.clone(),slim:self.slim,drawn:self.drawn,ranges:self.ranges.clone(),body_off:self.body_off,offloaded:self.offloaded,trie:self.trie.clone(),summary:self.summary.clone(),stored:None}
    }
}

/// Everything a child of `p` at attempt `a` needs, derived only from p and its state.
pub struct ChildCtx{
    pub height:u64,
    pub attempt:u32,
    pub window:AttemptWindow,
    pub membership:Membership,
    pub assignment:Assignment,
    pub target:Fixed,
    pub t_share:Fixed,
    pub proposer:Address,
    pub base_fee:U256,
    pub prevrandao:B256,
    pub parent_result:Option<ParentResult>,
    pub ancestors:Vec<(u64,B256)>,
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub enum Reject{
    /// Depends only on signature bytes (envReject, D129): reject this copy, not the block.
    Envelope(String),
    /// Committed-content violation (blockReject): the blockHash and descendants are invalid.
    Block(String),
    UnknownParent,
    /// Parent state outside the K_hist window: TrieRebuild, then retry.
    NeedsRebuild([u8;32]),
    Future,
    Duplicate,
    /// Removed by local retention (SIDE_BLOCKS_MAX or R_keep) and not taken again (EVICTED_MAX), or a new
    /// block at or below head-R_keep, which retention would remove at once (it cannot be canonical or win
    /// D133 there; deeper rollback is the manual checkpoint of governance.md:200). Never a validity verdict.
    Evicted,
    /// 2026-10-28: the node's own storage failed (a state version's commit to state.redb, a journal read),
    /// never a validity verdict: the block is not marked invalid. The node stops (exit 70), as when a
    /// history.redb or journal write fails, and the next open recovers (I1). Before, a state.redb commit
    /// that failed was a `Block` rejection: the block and its descendants were refused until a restart.
    Local(String),
}
fn blk<T>(s:impl Into<String>)->Result<T,Reject>{Err(Reject::Block(s.into()))}

pub struct Chain{
    pub spec:Spec,
    /// Boxed (A13): a table keeps spare buckets, up to 24/7 per entry while it grows, and an entry inline
    /// would make each a whole Entry.
    pub blocks:HashMap<[u8;32],Box<Entry>>,
    pub head:[u8;32],
    pub canonical:Vec<[u8;32]>,
    pub(crate) next_seq:u64,
    /// R21 window: full states kept for heights > head-K_hist (and genesis).
    pub k_hist:u64,
    pub rebuilds:u64,
    /// C-1 S3a: states materialized from a full state through diffs (no re-execution).
    pub materializations:std::sync::atomic::AtomicU64,
    /// C-1 S3a off: every retained state stays full (the behaviour before S3a; a test reference).
    pub keep_full_states:bool,
    /// C-1 S4: canonical journaled blocks deeper than this below the head are slimmed (None: never).
    /// `slim_cursor`: every canonical height below it was slimmed once; `unslimmed`: entries restored
    /// since, slimmed again when they are deep enough.
    pub slim_depth:Option<u64>,pub slim_cursor:u64,pub unslimmed:Vec<[u8;32]>,
    /// `retain_unsettled`'s R21 prune, done once per height (it walked the whole canonical chain at every
    /// call: 41.6 s of a 234 s open at scale, evidence/r4/open-logs). Every canonical block at a height in
    /// 1..prune_cursor is pruned (no state, pin, full state or diff). What can break that lowers the cursor
    /// (`unprune`): a canonical change below it (`rebuild_canonical`), a rebuild (`ensure_state_with`) and a
    /// restore (`restore_canonical`) of the blocks they touch. `prune_holds` checks it.
    pub prune_cursor:u64,
    /// The first `retain_unsettled` of a chain (a new or restored one) looks for orphans although it removed
    /// nothing; later ones only after a removal (every removal's orphans are removed with it).
    orphan_check:bool,
    /// The non-canonical entries (height > 0), kept as entries and the canonical path change: `insert`, the
    /// suffix `rebuild_canonical` replaces, and every removal in `retain_unsettled`. A restore rebuilds it
    /// (`retention_reset`). Before (2026-10-28), each retain found them by walking every entry, and the
    /// entries are the whole chain: an import was O(H), growth to H_END O(H²) (evidence/r4 § 4). Debug
    /// builds check it against that walk.
    side:std::collections::HashSet<[u8;32]>,
    /// Entries given a trie since the last `settle` (only `insert` gives one; genesis has one from `new`):
    /// what `settle` clears, the head's excepted, instead of visiting every entry.
    tried:Vec<[u8;32]>,
    /// C-1 E1: state roots from the parent's trie (false: built from the whole state, a test reference),
    /// and how parents' tries were obtained: derived from the head's through the diffs, built from the
    /// whole state, or derived but refused because the root did not match the committed one.
    pub incremental_root:bool,pub trie_derived:std::sync::atomic::AtomicU64,pub trie_built:std::sync::atomic::AtomicU64,pub trie_refused:std::sync::atomic::AtomicU64,
    /// C-1 S1: when set (by a node whose receipts and transaction index live in `history.redb`), canonical
    /// journaled blocks more than this many blocks below the head are offloaded like side blocks; the
    /// last `history_window` canonical blocks stay resident. None for a bare chain: nothing canonical is
    /// offloaded. `history_cursor`: every canonical height below it has been offloaded already.
    pub history_window:Option<u64>,pub history_cursor:u64,
    /// Blocks `restore_canonical` re-executed (resident ones on its span included): the restore work.
    pub restore_reexecs:u64,
    /// Invalid committed contents (blockReject) never re-validated.
    pub invalid:HashMap<[u8;32],String>,
    /// Insertion order of `invalid`; a cache only (never a validity input), at most INVALID_MAX.
    invalid_order:std::collections::VecDeque<[u8;32]>,
    /// tid -> (proposer, parent, h) for equivocation detection (evidence type 1).
    pub seen_templates:BTreeMap<(u64,[u8;32]),Vec<(Vec<u8>,Vec<u8>)>>,
    /// Side blocks removed by retention (EVICTED_MAX, FIFO) and how many were removed in all.
    pub(crate) evicted:std::collections::HashSet<[u8;32]>,pub(crate) evicted_order:std::collections::VecDeque<[u8;32]>,pub evicted_total:u64,
    /// Local retention parameters (consensus.md:29, governance.md:15): defaults R_KEEP and SIDE_BLOCKS_MAX.
    /// Tests lower them to exercise the boundaries quickly; they never affect validity.
    pub r_keep:u64,pub side_cap:usize,
    /// Blocks the side cap must not evict: the live sync session's fetched prefix, or (on replay) blocks
    /// that later journal records build on. Still subject to the R_keep boundary.
    pub protect:std::collections::HashSet<[u8;32]>,
    /// Journal replay: the cap then bounds ALL non-canonical blocks by side_cap + protect_max (what live
    /// could hold), since replay protects only blocks live held (Node::open).
    pub replaying:bool,
    /// C-1 E4: the node store states are kept in (None: whole states in memory, as before E4).
    pub trie_store:Option<std::sync::Arc<dyn pocol_lp4_execution::node_trie::TrieStore>>,
}

/// C-1 E5: the state of an entry that holds none (pruned, stored, or a metadata copy): one shared empty
/// state, so an entry without a state allocates none (A13 counts it once, not per entry). Nothing writes
/// through an entry's `Arc` (a state is replaced, never changed in place).
pub fn empty_state()->std::sync::Arc<InMemoryDB>{static E:std::sync::OnceLock<std::sync::Arc<InMemoryDB>>=std::sync::OnceLock::new();E.get_or_init(Default::default).clone()}
pub fn u256_fixed(v:&Fixed)->U256{from_fixed(v)}
pub fn work(t:&Fixed)->U256{from_fixed(&crate::fixed::work(t))}

impl Chain{
    pub fn new(spec:Spec)->Self{
        let gh=spec.genesis_hash();let cp=spec.cp();
        let mut h=blank_header();h.chain_id=spec.chain_id();h.genesis_hash=gh;h.ts=cp.g_ts;h.target=spec.target_g();
        let genesis=Entry{hash:gh,header:h,body:Body::default(),height:0,parent:None,total_work:U256::ZERO,import_seq:0,signed:false,ts:cp.g_ts,
            target:spec.target_g(),base_fee:U256::ZERO,gas_used:0,seal:seal_genesis(&gh),state:std::sync::Arc::new(spec.genesis().state().clone()),is_final:false,result:None,
            receipts:vec![],tx_hashes:vec![],cum_txs:0,cum_tx_bytes:0,cum_log_bytes:0,pruned:false,full:true,diff:None,slim:false,drawn:Address::ZERO,ranges:vec![],body_off:None,offloaded:false,
            trie:Some(pocol_lp4_execution::state_trie::StateTrie::build(spec.genesis().state())),
            summary:Some(Box::new(pocol_lp4_execution::state_summary::Summary::scan(spec.genesis().state(),cp.B_code_max))),stored:None};
        let mut blocks=HashMap::new();blocks.insert(gh,Box::new(genesis));
        let k_hist=u64::from(spec.local().k_hist);
        Self{spec,blocks,head:gh,canonical:vec![gh],next_seq:1,k_hist,rebuilds:0,materializations:Default::default(),keep_full_states:false,slim_depth:None,slim_cursor:1,unslimmed:Vec::new(),prune_cursor:1,orphan_check:true,incremental_root:true,trie_derived:Default::default(),trie_built:Default::default(),trie_refused:Default::default(),history_window:None,history_cursor:1,restore_reexecs:0,invalid:HashMap::new(),invalid_order:Default::default(),seen_templates:BTreeMap::new(),evicted:Default::default(),evicted_order:Default::default(),evicted_total:0,
            r_keep:R_KEEP,side_cap:SIDE_BLOCKS_MAX,protect:Default::default(),replaying:false,trie_store:None,side:Default::default(),tried:vec![gh]}
    }
    pub fn head(&self)->&Entry{&self.blocks[&self.head]}
    /// C-1 E4: keeps states in `store` from now on, starting with genesis's (Node::open calls it on a chain
    /// that holds only genesis, before replay).
    pub fn enable_store(&mut self,store:std::sync::Arc<dyn pocol_lp4_execution::node_trie::TrieStore>)->Result<(),Reject>{
        assert_eq!(self.blocks.len(),1,"the store is enabled before any block is imported");
        let g=self.spec.genesis().state();let mut b=pocol_lp4_execution::node_trie::Batch::default();
        let t=pocol_lp4_execution::node_trie::NodeTrie::build(g,&*store,&mut b);
        let pin=Pinned::new(store.clone(),StoredChild{trie:t,batch:b,hashes:g.block_hashes.clone()})?;
        let e=self.blocks.get_mut(&self.head).unwrap();e.stored=Some(std::sync::Arc::new(pin));e.state=empty_state();e.trie=None;e.diff=None;
        self.trie_store=Some(store);Ok(())
    }
    /// A stored block's state read in place: its stored version, its whole state, or materialized through
    /// the diffs; None when it is pruned or cannot be reached.
    pub fn view<'a>(&'a self,e:&'a Entry)->Option<StateView<'a>>{
        if let Some(p)=&e.stored{return Some(StateView::Stored(p.view()))}
        if e.pruned||self.trie_store.is_some(){return None}
        if e.full{return Some(StateView::Mem(&e.state))}
        self.materialize(&e.hash).map(|s|StateView::Owned(Box::new(s)))
    }
    /// The state an execution reads for its parent `e` (a chain entry, a copy of one, or a rolling entry):
    /// its stored version, else its `state` as it is, which is what execution read before E4.
    fn view_or<'a>(&'a self,e:&'a Entry)->Result<StateView<'a>,Reject>{
        Ok(match &e.stored{Some(p)=>StateView::Stored(p.view()),None=>StateView::Mem(&e.state)})
    }
    /// A re-executed block's state put on its rolling entry: whole with its trie, or (C-1 E4) committed to
    /// the store and pinned.
    fn rolled(&self,mut next:Entry,out:Executed)->Result<Entry,Reject>{
        match out{
            Executed::Mem(o)=>{next.state=o.state.into();next.trie=Some(o.trie);next.summary=Some(Box::new(o.summary));}
            Executed::Stored{run,child,..}=>{
                let store=self.trie_store.clone().ok_or_else(||Reject::Block("a stored child without a store".into()))?;
                next.stored=Some(std::sync::Arc::new(Pinned::new(store,child)?));next.summary=Some(Box::new(run.summary));
            }
        }
        Ok(next)
    }
    pub fn get(&self,h:&[u8;32])->Option<&Entry>{self.blocks.get(h).map(|e|&**e)}
    pub fn canonical_at(&self,height:u64)->Option<&Entry>{self.canonical.get(height as usize).and_then(|h|self.get(h))}

    // ---------------- child context ----------------
    pub fn target_for_child(&self,p:&Entry)->Result<Fixed,Reject>{
        let cp=self.spec.cp();
        asert::asert(&self.spec.target_g(),cp.g_ts as i128,cp.T_blk as i128,cp.tau as i128,p.ts as i128,p.height as i128).map(|o|o.target).ok_or_else(||Reject::Block("asert".into()))
    }
    pub fn membership(&self,p:&Entry,h:u64)->Result<Membership,Reject>{
        let cp=self.spec.cp();
        let st=self.view_or(p)?;let st=st.v();
        let act:Vec<ParentMember>=apply::active_list(st).into_iter().map(|id|{let r=apply::miner_record(st,id);
            ParentMember{id:raw_addr(id),tomb:r.map(|r|r.tomb).unwrap_or(false),last_share:r.map(|r|r.last_share).unwrap_or(0),grace_start:r.map(|r|r.grace_start).unwrap_or(0)}}).collect();
        Membership::from_parent(&ParentView{height:p.height,seal:&p.seal,act:&act},h,cp.W_act,cp.M_max).map_err(|e|Reject::Block(format!("membership {e:?}")))
    }
    pub fn draw(&self,p:&Entry,members:&Membership,h:u64)->Result<Address,Reject>{
        let cp=self.spec.cp();let lo=h.saturating_sub(1).saturating_sub(u64::from(cp.W_w)).max(1);let mut hist=Vec::new();
        let st=self.view_or(p)?;let st=st.v();
        if h>=2{for j in lo..=h-2{for id in members.ids(){let c=apply::share_count(st,j,addr(id));if c>0{hist.push(ShareCount{height:j,id:*id,count:c});}}}}
        let w=DrawWeights::new(members,cp.W_w,&hist).map_err(|e|Reject::Block(format!("draw {e:?}")))?;
        w.draw().map(|(a,_)|addr(&a)).map_err(|e|Reject::Block(format!("draw {e:?}")))
    }
    pub fn ancestors(&self,p:&Entry)->Vec<(u64,B256)>{
        let mut out=Vec::new();let mut cur=Some(p);
        while let Some(e)=cur{ if p.height+1-e.height>256{break} out.push((e.height,B256::from(e.hash))); cur=e.parent.and_then(|x|self.get(&x)); }
        out
    }
    pub fn child_ctx(&self,p:&Entry,attempt:u32)->Result<ChildCtx,Reject>{
        let cp=self.spec.cp();let h=p.height+1;
        let window=AttemptWindow::new(p.ts,attempt,cp.D_att).map_err(|e|Reject::Block(format!("window {e:?}")))?;
        let membership=self.membership(p,h)?;
        let target=self.target_for_child(p)?;
        let mode=self.spec.effective_nonce_mode(h);
        let assignment=Assignment::new(&membership,attempt,&target,mode,cp.c).map_err(|e|Reject::Block(format!("assignment {e:?}")))?;
        let ts_share=from_fixed(&target).checked_mul(U256::from(cp.m)).unwrap_or(U256::MAX);
        let proposer=self.draw(p,&membership,h)?;
        let base_fee=if h==1{from_fixed_lp4(&cp.baseFee0)}else{fees::next_base_fee(p.base_fee,p.gas_used,p.header.gas_limit).map_err(|e|Reject::Block(format!("baseFee {e:?}")))?};
        Ok(ChildCtx{height:h,attempt,window,membership,assignment,target,t_share:to_fixed(ts_share),proposer,base_fee,prevrandao:prevrandao(&p.seal),
            parent_result:p.result.clone(),ancestors:self.ancestors(p)})
    }

    // ---------------- execution ----------------
    fn execute(&self,p:&Entry,ctx:&ChildCtx,ts:u64,proposer:Proposer,body:&Body,effects:Vec<EvidenceEffect>)->Result<Executed,Reject>{
        let input=BlockInput{height:ctx.height,timestamp:ts,base_fee:ctx.base_fee,gas_limit:self.spec.cp().GAS_LIMIT,prevrandao:ctx.prevrandao,proposer,
            parent_result:ctx.parent_result.clone(),evidence:effects,evidence_raw:&body.evidence,transactions:&body.txs,ancestors:ctx.ancestors.clone(),cum_log_bytes:p.cum_log_bytes};
        // C-1 E4: a stored parent is read in place; the child's version is its trie updated through the
        // block's overlay, committed when the entry is made (`Pinned::new`).
        if let (Some(pin),Some(store))=(&p.stored,&self.trie_store){
            let parent=pin.view();
            let sum=match p.summary.as_deref(){Some(s)=>*s,None=>pocol_lp4_execution::state_summary::Summary::scan_view(&parent,self.spec.cp().B_code_max)};
            let mut run=apply::apply_block_on(&parent,&sum,self.spec.genesis(),self.spec.profile(),&input).map_err(|e|Reject::Block(format!("apply {e:?}")))?;
            let layer=std::mem::take(&mut run.layer);let child_view=pocol_lp4_execution::view::Overlay::from_layer(&parent,layer);
            let mut batch=pocol_lp4_execution::node_trie::Batch::default();
            let trie=pin.trie.update(&**store,&mut batch,&child_view,&run.writes);let root=trie.root(&**store,&batch);
            let hashes=input.ancestors.iter().map(|(n,h)|(U256::from(*n),*h)).collect();
            return Ok(Executed::Stored{run:Box::new(run),child:StoredChild{trie,batch,hashes},root})
        }
        let trie=if self.incremental_root{self.trie_for(p)}else{None};
        if self.incremental_root&&trie.is_none(){self.trie_built.fetch_add(1,std::sync::atomic::Ordering::Relaxed);}
        let summary=if self.incremental_root{p.summary.as_deref()}else{None};
        apply::apply_block_with(&p.state,trie.as_ref(),summary,self.spec.genesis(),self.spec.profile(),&input).map(Executed::Mem).map_err(|e|Reject::Block(format!("apply {e:?}")))
    }
    /// The committed state root of a stored block (genesis: the built genesis image's).
    fn committed_root(&self,e:&Entry)->[u8;32]{if e.height==0{self.spec.genesis().root().0}else{e.header.state_root}}
    /// C-1 E1: the trie of `p`'s state (`p.state` must be its full state): its own, or the head's updated
    /// through the diffs between the two blocks (at most TRIE_PATH_MAX of them). Either is used only when
    /// its root equals p's committed state root, so a trie that does not belong to the state is never used;
    /// None means the root is computed from the whole state, as before E1.
    fn trie_for(&self,p:&Entry)->Option<pocol_lp4_execution::state_trie::StateTrie>{
        let want=self.committed_root(p);
        if let Some(t)=&p.trie{if t.root().0==want{return Some(t.clone())}self.trie_refused.fetch_add(1,std::sync::atomic::Ordering::Relaxed);}
        let head=self.blocks.get(&self.head)?;let ht=head.trie.as_ref()?;if head.hash==p.hash{return None}
        let (down,up)=self.path_between(&head.hash,&p.hash)?;if down.len()+up.len()>TRIE_PATH_MAX{return None}
        let mut touched=pocol_lp4_execution::state_trie::Touched::default();
        for h in down.iter().chain(&up){self.blocks[h].diff.as_ref()?.touch(&mut touched);}
        let t=ht.update(&p.state,&touched);
        if t.root().0!=want{self.trie_refused.fetch_add(1,std::sync::atomic::Ordering::Relaxed);return None}
        self.trie_derived.fetch_add(1,std::sync::atomic::Ordering::Relaxed);Some(t)
    }
    fn fill_ut(&self,p:&Entry,ctx:&ChildCtx,ts:u64,out:&Executed,proposer:&[u8])->Header{
        let mut h=blank_header();h.chain_id=self.spec.chain_id();h.genesis_hash=self.spec.genesis_hash();h.parent_hash=p.hash;h.h=ctx.height;h.a=ctx.attempt;
        h.protocol_version=self.spec.active_version(ctx.height);h.ts=ts;h.target=ctx.target;h.state_root=out.state_root().0;h.tx_root=out.transactions_root().0;
        h.receipts_root=out.receipts_root().0;h.logs_bloom=out.logs_bloom().to_vec();h.gas_limit=self.spec.cp().GAS_LIMIT;h.gas_used=out.gas_used();
        h.base_fee=to_fixed(ctx.base_fee);h.evidence_root=out.evidence_root().0;h.proposer=proposer.to_vec();reseal(&mut h);h
    }
    /// Build_fb(p, a): deterministic fallback UT with empty body (consensus §4b).
    pub fn build_fallback(&self,p:&Entry,attempt:u32)->Result<(Header,Executed,ChildCtx),Reject>{
        let ctx=self.child_ctx(p,attempt)?;
        let ts=ctx.window.fallback_timestamp(self.spec.cp().dFbWait).map_err(|e|Reject::Block(format!("fb ts {e:?}")))?;
        let out=self.execute(p,&ctx,ts,Proposer::Fallback,&Body::default(),vec![])?;
        let h=self.fill_ut(p,&ctx,ts,&out,&[]);Ok((h,out,ctx))
    }
    /// Signed template at a=0 by Draw(h,0); ts=max(L(0),C).
    pub fn build_signed(&self,p:&Entry,key:&Key,clock:u64,body:Body,effects:Vec<EvidenceEffect>)->Result<(Header,Executed,ChildCtx),Reject>{
        let ctx=self.child_ctx(p,0)?;
        if key.address()!=ctx.proposer{return blk("not the drawn proposer")}
        let ts=ctx.window.proposer_timestamp(clock).map_err(|e|Reject::Block(format!("proposer ts {e:?}")))?;
        let out=self.execute(p,&ctx,ts,Proposer::Signed(key.address()),&body,effects)?;
        let mut h=self.fill_ut(p,&ctx,ts,&out,key.address().as_slice());
        let tid=template_id(&h.ut_raw);h.sig=key.sign_hash(&B256::from(hashes::sig_msg(&tid))).to_vec();reseal(&mut h);
        Ok((h,out,ctx))
    }

    /// UT bytes a valid signed template for (p, ts, proposer, body) must have.
    pub fn rebuild_signed(&self,p:&Entry,ctx:&ChildCtx,ts:u64,proposer:Address,body:&Body,effects:Vec<EvidenceEffect>)->Result<Vec<u8>,Reject>{
        let out=self.execute(p,ctx,ts,Proposer::Signed(proposer),body,effects)?;Ok(self.fill_ut(p,ctx,ts,&out,proposer.as_slice()).ut_raw)
    }

    // ---------------- validation ----------------
    /// H-pre items 1-9 (stateless with the parent header) and H-full 10-17.
    pub fn validate(&self,b:&Block,clock:Option<u64>,phi:u64)->Result<(Entry,ChildCtx),Reject>{
        let cp=self.spec.cp();let x=&b.header;let hash=b.hash();
        if self.blocks.contains_key(&hash){return Err(Reject::Duplicate)}
        if let Some(r)=self.invalid.get(&hash){return blk(format!("known invalid: {r}"))}
        // 1. sizes, 1b NetID.
        if x.encoded.len()>crate::header::HDR_MAX{return blk("1 header size")}
        if x.chain_id!=self.spec.chain_id(){return blk("netChain")}
        if x.genesis_hash!=self.spec.genesis_hash(){return blk("netGenesis")}
        if x.protocol_version!=self.spec.active_version(x.h){return blk("netVersion")}
        if !self.blocks.contains_key(&x.parent_hash){return Err(Reject::UnknownParent)}
        if let Some(r)=self.invalid.get(&x.parent_hash){return blk(format!("ancestorInvalid {r}"))}
        if self.blocks[&x.parent_hash].pruned||self.blocks[&x.parent_hash].slim{return Err(Reject::NeedsRebuild(x.parent_hash))}
        // C-1 S3a: a retained parent state that is not held full is materialized through the diffs.
        let materialized;
        let p=if self.blocks[&x.parent_hash].full{&self.blocks[&x.parent_hash]}else{
            materialized=self.full_entry(&x.parent_hash).ok_or(Reject::NeedsRebuild(x.parent_hash))?;
            self.materializations.fetch_add(1,std::sync::atomic::Ordering::Relaxed);&materialized};
        // 2. linkage and H_END.
        if x.h!=p.height+1{return blk("2 height")}
        if cp.H_END!=0&&x.h>cp.H_END{return blk("10a H_END")}
        if let Some(c)=clock{ if crate::timing::is_future(x.ts,c,phi){return Err(Reject::Future)} }
        // 3. template form. proposer and a are committed (blockReject); sig is not (envReject).
        let tid=template_id(&x.ut_raw);
        let signed=!x.proposer.is_empty();
        if signed{
            if x.proposer.len()!=20{return blk("1 proposer width")}
            if x.a!=0{return blk("3 signed requires a=0")}
            if x.sig.len()!=65{return Err(Reject::Envelope("3 signed template without signature".into()))}
            match hashes::recover_address(&hashes::sig_msg(&tid),&x.sig){Ok(a) if a[..]==x.proposer[..]=>{},_=>return Err(Reject::Envelope("3 template signature".into()))}
        }else if !x.sig.is_empty(){return Err(Reject::Envelope("3 signature without proposer".into()))}
        // 4. timing window and fallback timestamp.
        let ctx=self.child_ctx(p,x.a)?;
        if !ctx.window.contains(x.ts){return blk("4 window")}
        if !signed{let fb=ctx.window.fallback_timestamp(cp.dFbWait).map_err(|_|Reject::Block("4 fb".into()))?;if x.ts!=fb{return blk("4 fallback timestamp")}}
        // 5. ASERT target. 6/7 nonce and PoW.
        if x.target!=ctx.target{return blk("5 target")}
        if (x.nonce as u128)>=ctx.assignment.nonce_max(){return blk("6 nonce>=nMax")}
        if !le_target(&pow_hash(&tid,x.nonce),&x.target){return blk("7 pow")}
        // 8. winnerSig recoverable.
        let win=hashes::win_msg(&tid,x.nonce,&share_root(x));
        let winner=hashes::recover_address(&win,&x.winner_sig).map_err(|_|Reject::Envelope("8 winnerSig".into()))?;
        // 9. shares.
        if x.shares.len()>usize::from(cp.S_max){return blk("1 shareList size")}
        for w in x.shares.windows(2){if w[0]>=w[1]{return blk("9 shares not strictly ascending")}}
        for n in &x.shares{
            if *n==x.nonce||(*n as u128)>=ctx.assignment.nonce_max(){return blk("9 share range")}
            if !le_target(&pow_hash(&tid,*n),&ctx.t_share){return blk("9 share target")}
        }
        // 10. parent not final. 11. gas limit and base fee.
        if p.is_final{return blk("10 final parent")}
        if x.gas_limit!=cp.GAS_LIMIT{return blk("11 gasLimit")}
        if from_fixed(&x.base_fee)!=ctx.base_fee{return blk("11 baseFee")}
        // Body binding first (D129): the block hash commits the header only and the body through
        // tx_root/evidence_root. A body that does not match is another copy, not an invalid block:
        // envReject, never marked invalid. Body-content checks below run on the bound body only.
        // An uncomputable commitment (empty item) is also a copy fault: no header can commit to it (audit A01).
        let bound=crate::types::body_commitment(&b.body.txs)==Some(x.tx_root)&&crate::types::body_commitment(&b.body.evidence)==Some(x.evidence_root);
        if !bound{return Err(Reject::Envelope("15 body does not match the header roots".into()))}
        // 1 (body): sizes and evidence bounds before any evidence evaluation.
        if b.body.txs.iter().any(|t|t.len()>cp.TX_MAX as usize){return blk("1 TX_MAX")}
        // consensus.md: Body = [txs, evidence] <= BODY_MAX, the encoded body, exactly as Body::decode
        // checks it for blocks from peers (review finding: the raw item sum was checked here).
        if b.body.encode().len()>cp.BODY_MAX as usize{return blk("1 BODY_MAX")}
        if b.body.evidence.len()>usize::from(cp.E_max)||b.body.evidence.iter().any(|e|e.len()>cp.EVID_BYTES_MAX as usize){return blk("1 evidence bounds")}
        // 12. exclusive form. Fallback: body 0xc2c0c0 and UT byte-equal to Build_fb (needs apply).
        let mut fb_out=None;
        if signed{ if x.proposer[..]!=ctx.proposer[..]{return blk("12 proposer is not Draw(h,0)")} }
        else{
            if b.body.encode()!=EMPTY_BODY{return blk("12 fallback body")}
            let out=self.execute(p,&ctx,x.ts,Proposer::Fallback,&b.body,vec![])?;
            if self.fill_ut(p,&ctx,x.ts,&out,&[]).ut_raw!=x.ut_raw{return blk("12 fallback UT differs from Build_fb")}
            fb_out=Some(out);
        }
        // 13/14. ownership (winnerSig signer part is envReject).
        let owner=ctx.assignment.owner(x.nonce).ok_or_else(||Reject::Block("13 owner".into()))?;
        if owner!=winner{return Err(Reject::Envelope("13 winnerSig signer is not owner(nonce)".into()))}
        let mut shares=Vec::new();for n in &x.shares{shares.push((*n,addr(&ctx.assignment.owner(*n).ok_or_else(||Reject::Block("14 share owner".into()))?)));}
        // 15. evidence validity (roots were bound above). 16. cumulative caps.
        let effects=if signed{self.evidence_effects(p,&b.body,x.h)?}else{vec![]};
        let tx_bytes:u64=b.body.txs.iter().map(|t|t.len() as u64).sum();
        if !self.within_caps(p,&b.body.txs){return blk("16 caps")}
        // 17. apply.
        let out=match fb_out{Some(o)=>o,None=>self.execute(p,&ctx,x.ts,Proposer::Signed(Address::from_slice(&x.proposer)),&b.body,effects)?};
        if out.state_root().0!=x.state_root{return blk("17 stateRoot")}
        if out.receipts_root().0!=x.receipts_root||out.logs_bloom()[..]!=x.logs_bloom[..]||out.gas_used()!=x.gas_used{return blk("17 receipts/bloom/gasUsed")}
        let tx_hashes=b.body.txs.iter().map(|t|revm::primitives::keccak256(t)).collect();
        let (is_final,log_bytes)=(out.is_final(),out.log_bytes());
        let c=self.child_state(p,out)?;
        let entry=Entry{hash,header:x.clone(),body:b.body.clone(),height:x.h,parent:Some(p.hash),total_work:p.total_work.saturating_add(work(&x.target)),
            import_seq:0,signed,ts:x.ts,target:x.target,base_fee:ctx.base_fee,gas_used:x.gas_used,seal:seal(&tid,x.nonce),state:c.state,is_final,full:true,diff:c.diff,slim:false,
            result:Some(ParentResult{block_hash:B256::from(hash),winner:addr(&owner),shares}),receipts:c.receipts,tx_hashes,
            cum_txs:if self.spec.cp().H_END==0{b.body.txs.len() as u64}else{p.cum_txs+b.body.txs.len() as u64},cum_tx_bytes:if self.spec.cp().H_END==0{tx_bytes}else{p.cum_tx_bytes+tx_bytes},cum_log_bytes:if self.spec.cp().H_END==0{log_bytes}else{p.cum_log_bytes+log_bytes},pruned:false,drawn:ctx.proposer,
            ranges:ctx.assignment.ranges().iter().map(|r|(addr(&r.id()),r.start(),r.end_exclusive())).collect(),body_off:None,offloaded:false,trie:c.trie,summary:Some(Box::new(c.summary)),stored:c.stored};
        Ok((entry,ctx))
    }
    /// The state fields of a child entry from its execution: a whole state with its diff from the parent
    /// (C-1 S3a; from the write set when it ran on an overlay, C-1 E3) and its trie, or (C-1 E4) its
    /// stored version, committed and pinned here, before the block's journal record (I3).
    fn child_state(&self,p:&Entry,out:Executed)->Result<ChildState,Reject>{
        match out{
            Executed::Mem(o)=>{
                let diff=Some(std::sync::Arc::new(crate::statediff::StateDiff::of_output(&p.state,&o)));
                Ok(ChildState{state:std::sync::Arc::new(o.state),diff,trie:Some(o.trie),summary:o.summary,receipts:o.receipts,stored:None})
            }
            Executed::Stored{run,child,..}=>{
                let store=self.trie_store.clone().ok_or_else(||Reject::Block("a stored child without a store".into()))?;
                let pin=Pinned::new(store,child)?;
                Ok(ChildState{state:empty_state(),diff:None,trie:None,summary:run.summary,receipts:run.receipts,stored:Some(std::sync::Arc::new(pin))})
            }
        }
    }

    /// Item 16 (consensus.md:170): the transaction count and raw bytes (D49: len(rawTx)) of p's branch plus
    /// `txs` within TX_CAP and TXBYTES_CAP. The one definition validation, the builder and template locking use.
    pub fn within_caps(&self,p:&Entry,txs:&[Vec<u8>])->bool{
        let cp=self.spec.cp();let past_count=if cp.H_END==0{0}else{p.cum_txs};let past_bytes=if cp.H_END==0{0}else{p.cum_tx_bytes};let bytes=txs.iter().try_fold(past_bytes,|n,t|n.checked_add(t.len() as u64));
        past_count.checked_add(txs.len() as u64).map_or(false,|n|n<=cp.TX_CAP)&&bytes.map_or(false,|n|n<=cp.TXBYTES_CAP)
    }
    /// What item 16 leaves for a child of p: (transactions, raw bytes).
    pub fn caps_room(&self,p:&Entry)->(u64,u64){let cp=self.spec.cp();if cp.H_END==0{(cp.TX_CAP,cp.TXBYTES_CAP)}else{(cp.TX_CAP.saturating_sub(p.cum_txs),cp.TXBYTES_CAP.saturating_sub(p.cum_tx_bytes))}}

    /// Evidence (incentives.md "Evidence"). Type 1: RLP([1, ST1, ST2]); type 2:
    /// RLP([2, TemplateID, nonce, shareRoot1, sig1, shareRoot2, sig2]). Invalid
    /// evidence invalidates the carrying block.
    pub fn evidence_effects(&self,p:&Entry,body:&Body,h:u64)->Result<Vec<EvidenceEffect>,Reject>{
        let cp=self.spec.cp();let mut out=Vec::new();let mut accused_seen=Vec::new();
        for raw in &body.evidence{
            let it=crate::rlp::decode(raw).map_err(|_|Reject::Block("15 evidence rlp".into()))?;let f=it.list().ok_or_else(||Reject::Block("15 evidence".into()))?;
            let kind=f.first().and_then(|k|k.bytes()).ok_or_else(||Reject::Block("15 evidence kind".into()))?;
            let (accused,j)=match kind{
                [1] if f.len()==3=>self.evidence_type1(p,&f[1],&f[2],h)?,
                [2] if f.len()==7=>self.evidence_type2(p,&f,h)?,
                _=>return blk("15 evidence form"),
            };
            if accused_seen.contains(&accused){return blk("15 duplicate accused")}
            match apply::miner_record(self.view_or(p)?.v(),accused){Some(r) if !r.tomb=>{},_=>return blk("15 accused has no live record")}
            accused_seen.push(accused);
            out.push(match j{Some(j)=>EvidenceEffect::PunishAndInvalidate{accused,j},None=>EvidenceEffect::Punish{accused}});
        }
        let _=cp;Ok(out)
    }
    fn ancestor_at<'a>(&'a self,p:&'a Entry,height:u64)->Option<&'a Entry>{let mut cur=Some(p);while let Some(e)=cur{if e.height==height{return Some(e)}if e.height<height{return None}cur=e.parent.and_then(|x|self.get(&x));}None}
    fn evidence_type1(&self,p:&Entry,a:&crate::rlp::Item<'_>,b:&crate::rlp::Item<'_>,h_incl:u64)->Result<(Address,Option<u64>),Reject>{
        // ST = [UT, sig]; reuse the strict header codec by wrapping ST in a header shell.
        let parse=|it:&crate::rlp::Item<'_>|->Result<(Header,Vec<u8>),Reject>{
            let st=it.list().ok_or_else(||Reject::Block("ev1 st".into()))?;if st.len()!=2{return blk("ev1 st")}
            let sig=st[1].bytes().ok_or_else(||Reject::Block("ev1 sig".into()))?.to_vec();
            let mut full=crate::rlp::encode(it);crate::rlp::encode_bytes(&0u64.to_be_bytes(),&mut full);full.push(0xc0);crate::rlp::encode_bytes(&[1u8;65],&mut full);
            let mut enc=Vec::new();crate::rlp::encode_list_payload(&full,&mut enc);
            let hb=crate::header::decode(&enc).map_err(|_|Reject::Block("ev1 ut decode".into()))?;Ok((hb,sig))
        };
        let (u1,s1)=parse(a)?;let (u2,s2)=parse(b)?;
        if u1.ut_raw==u2.ut_raw{return blk("ev1 identical")}
        if u1.parent_hash!=u2.parent_hash||u1.h!=u2.h||u1.a!=0||u2.a!=0||u1.chain_id!=self.spec.chain_id()||u2.chain_id!=self.spec.chain_id()||u1.genesis_hash!=self.spec.genesis_hash()||u2.genesis_hash!=self.spec.genesis_hash(){return blk("ev1 context")}
        if u1.h+u64::from(self.spec.cp().E_win)<h_incl{return blk("ev1 too old")}
        let r1=hashes::recover_address(&hashes::sig_msg(&template_id(&u1.ut_raw)),&s1).map_err(|_|Reject::Block("ev1 sig1".into()))?;
        let r2=hashes::recover_address(&hashes::sig_msg(&template_id(&u2.ut_raw)),&s2).map_err(|_|Reject::Block("ev1 sig2".into()))?;
        if r1!=r2{return blk("ev1 signers differ")}
        // Both are signed templates (§3 form): proposer present and equal to the signer.
        if u1.proposer[..]!=r1[..]||u2.proposer[..]!=r1[..]{return blk("ev1 proposer is not the signer")}
        if u1.h>h_incl{return blk("ev1 future height")}
        // DrawIndex[h mod E_win]: the proposer drawn for height u1.h on the carrying branch.
        let drawn=if u1.h==p.height+1{let m=self.membership(p,u1.h)?;self.draw(p,&m,u1.h)?}
            else{self.ancestor_at(p,u1.h).ok_or_else(||Reject::Block("ev1 height not on branch".into()))?.drawn};
        if drawn[..]!=r1[..]{return blk("ev1 signer not drawn")}
        Ok((addr(&r1),None))
    }
    fn evidence_type2(&self,p:&Entry,f:&[crate::rlp::Item<'_>],h_incl:u64)->Result<(Address,Option<u64>),Reject>{
        let b32=|i:usize|->Result<[u8;32],Reject>{let v=f[i].bytes().ok_or_else(||Reject::Block("ev2".into()))?;if v.len()!=32{return blk("ev2 len")}let mut o=[0;32];o.copy_from_slice(v);Ok(o)};
        let tid=b32(1)?;let nb=f[2].bytes().ok_or_else(||Reject::Block("ev2 nonce".into()))?;if nb.len()!=8{return blk("ev2 nonce")}
        let mut n8=[0u8;8];n8.copy_from_slice(nb);let nonce=u64::from_be_bytes(n8);
        let (r1,r2)=(b32(3)?,b32(5)?);if r1==r2{return blk("ev2 equal roots")}
        let s1=f[4].bytes().ok_or_else(||Reject::Block("ev2".into()))?;let s2=f[6].bytes().ok_or_else(||Reject::Block("ev2".into()))?;
        let a1=hashes::recover_address(&hashes::win_msg(&tid,nonce,&r1),s1).map_err(|_|Reject::Block("ev2 sig1".into()))?;
        let a2=hashes::recover_address(&hashes::win_msg(&tid,nonce,&r2),s2).map_err(|_|Reject::Block("ev2 sig2".into()))?;
        if a1!=a2{return blk("ev2 signers")}
        // An ancestor j >= h-E_win with this TemplateID; accused = owner(nonce) there.
        let mut cur=Some(p);
        while let Some(e)=cur{
            if e.height==0||e.height+u64::from(self.spec.cp().E_win)<h_incl{break}
            // C-1 S4: an ancestor this walk needs is restored first (never inside the slim depth).
            if e.slim{return Err(Reject::NeedsRebuild(e.hash))}
            if template_id(&e.header.ut_raw)==tid{
                // owner(nonce) under that block's assignment (stored ranges, D142).
                let owner=e.ranges.iter().find(|(_,s0,e0)|*s0<=nonce as u128&&(nonce as u128)<*e0).map(|r|r.0).ok_or_else(||Reject::Block("ev2 nonce outside nMax".into()))?;
                if owner.0.0!=a1{return blk("ev2 signer not owner(nonce)")}
                return Ok((addr(&a1),Some(e.height)));
            }
            cur=e.parent.and_then(|x|self.get(&x));
        }
        blk("ev2 no ancestor with TemplateID")
    }

    // ---------------- import and D133 fork choice ----------------
    pub fn insert(&mut self,mut e:Entry)->bool{
        e.import_seq=self.next_seq;self.next_seq+=1;let hash=e.hash;if e.trie.is_some(){self.tried.push(hash);}self.blocks.insert(hash,Box::new(e));
        let better=self.better(&hash,&self.head.clone());
        if better{self.head=hash;self.rebuild_canonical();}else if !self.is_canonical(&hash){self.side.insert(hash);}
        better
    }
    pub fn mark_invalid(&mut self,hash:[u8;32],why:String){
        if self.invalid.insert(hash,truncate(&why)).is_none(){self.invalid_order.push_back(hash);}
        while self.invalid_order.len()>INVALID_MAX{if let Some(old)=self.invalid_order.pop_front(){self.invalid.remove(&old);}}
    }
    /// R21 retention: drop states of canonical blocks at height <= head-K_hist (genesis kept).
    /// A13 side-branch bounds (local retention, never validity; consensus.md:29, D118):
    /// - a non-canonical block `r_keep` or more below the head is removed with its descendants
    ///   (adoption is owed at depth <= R_keep, network.md:144; deeper is governance.md:200);
    /// - at most `side_cap` UNPROTECTED non-canonical blocks are kept: the lowest-work unprotected
    ///   leaf goes first, repeatedly (its parent may become a leaf), so the bound holds after every
    ///   call, a reorg included. `protect` (a sync session's fetched branch, or on replay the blocks
    ///   later journal records build on) is exempt from this cap only;
    /// - every journaled non-canonical block is offloaded to metadata (D-REORG-REDB): its body,
    ///   receipts, tx hashes and encoded header are dropped and read from the journal on demand.
    ///   Canonical blocks are offloaded only below the resident window of C-1 S1 (`history_window`), so
    ///   `restore_canonical` makes adopted ones whole only inside it;
    /// - at most `side_max` non-canonical blocks keep a state (unprotected and lowest-work first
    ///   are pruned, rebuilt on demand).
    pub fn retain(&mut self){self.retain_unsettled();self.settle();}
    /// `retain` without its last part (`settle`): a node that adopted a branch restores it
    /// (`restore_canonical`) in between, while the side states and diffs that lead to the fork point
    /// are still there (C-1 S3a).
    pub fn retain_unsettled(&mut self){
        let head_h=self.head().height;let cut=head_h.saturating_sub(self.k_hist);
        // R21 is the canonical trie window; non-canonical states are bounded by side_max below. (Pruning
        // a side block below the window made each block of a deep competing branch rebuild its parent
        // from genesis: quadratic work for an R_keep-deep adoption.)
        // Heights 1..=cut, each once (`prune_cursor`); what has a block there was pruned before the cut moved.
        // With the invariant holding (checked in debug builds), the old walk over 1..=cut changed nothing below it.
        debug_assert!(self.prune_holds(),"prune_cursor {}: a canonical block below it is not pruned",self.prune_cursor);
        while self.prune_cursor<=cut{
            if let Some(h)=self.canonical.get(self.prune_cursor as usize).copied(){if let Some(e)=self.blocks.get_mut(&h){if !e.pruned{e.state=empty_state();e.pruned=true;}e.stored=None;e.full=false;e.diff=None;}}
            self.prune_cursor+=1;
        }
        // The side blocks: nothing below makes a block canonical, and orphans and evicted blocks are side
        // blocks (a canonical block's parent is canonical), so what is left is this list less what was removed.
        debug_assert!(self.side==self.side_scan(),"the side set is the non-canonical entries");
        let side:Vec<[u8;32]>=self.side.iter().copied().collect();
        let r_keep=self.r_keep;
        let mut gone:Vec<[u8;32]>=side.iter().filter(|h|self.blocks[*h].height+r_keep<=head_h).copied().collect();
        for h in &gone{self.blocks.remove(h);self.side.remove(h);}
        // Descendants of a removed block go too (their parent is gone). Only a side block can lose its parent.
        if !gone.is_empty()||self.orphan_check{self.orphan_check=false;
            loop{let orphaned:Vec<[u8;32]>=self.side.iter().filter(|h|self.blocks[*h].parent.map_or(false,|p|!self.blocks.contains_key(&p))).copied().collect();
                if orphaned.is_empty(){break}for h in orphaned{self.blocks.remove(&h);self.side.remove(&h);gone.push(h);}}}
        debug_assert!(self.blocks.values().all(|e|e.parent.map_or(true,|p|self.blocks.contains_key(&p))),"an orphan was left");
        let side_now:Vec<[u8;32]>=side.into_iter().filter(|h|self.blocks.contains_key(h)).collect();
        let unprotected=side_now.iter().filter(|h|!self.protect.contains(*h)).count();
        // Live: unprotected <= side_cap (a session holds <= protect_max more). Replay: all <= their sum.
        let (count,limit)=if self.replaying{(side_now.len(),self.side_cap+self.protect_max())}else{(unprotected,self.side_cap)};
        if count>limit{
            // Children: a block with a (protected) child is never a leaf. A side block's children are side
            // blocks (a canonical block's parent is canonical), so the side blocks give every count used here.
            let mut children:HashMap<[u8;32],usize>=HashMap::new();
            for h in &side_now{if let Some(p)=self.blocks[h].parent{*children.entry(p).or_insert(0)+=1;}}
            let evictable:std::collections::HashSet<[u8;32]>=side_now.iter().filter(|h|!self.protect.contains(*h)).copied().collect();
            let key=|c:&Chain,h:&[u8;32]|{let e=&c.blocks[h];std::cmp::Reverse((e.total_work,e.import_seq,*h))};
            let mut leaves:std::collections::BinaryHeap<_>=evictable.iter().filter(|h|!children.contains_key(*h)).map(|h|key(self,h)).collect();
            let mut n=count;
            while n>limit{
                let Some(std::cmp::Reverse((_,_,h)))=leaves.pop() else{break};
                let parent=self.blocks.remove(&h).and_then(|e|e.parent);self.side.remove(&h);gone.push(h);n-=1;
                if let Some(p)=parent{if let Some(c)=children.get_mut(&p){*c-=1;if *c==0&&evictable.contains(&p){children.remove(&p);leaves.push(key(self,&p));}}}
            }
        }
        for h in gone{self.evicted_total+=1;if self.evicted.insert(h){self.evicted_order.push_back(h);}}
        while self.evicted_order.len()>EVICTED_MAX{if let Some(old)=self.evicted_order.pop_front(){self.evicted.remove(&old);}}
        // Live: protection is for non-canonical session blocks only (an adopted one no longer counts
        // toward protect_max). Replay keeps a canonical block protected while a later record builds on
        // it, in case a later replayed reorg makes it non-canonical again.
        let keep:Vec<[u8;32]>=self.protect.iter().filter(|h|self.blocks.contains_key(*h)&&(self.replaying||!self.is_canonical(h))).copied().collect();
        self.protect=keep.into_iter().collect();
        // Offload: the journal holds every byte dropped here (body_off is set only after a fsynced
        // append), so no crash can lose them; genesis is never journaled and never offloaded.
        let side_left:Vec<[u8;32]>=side_now.into_iter().filter(|h|self.blocks.contains_key(h)).collect();
        for h in &side_left{if let Some(e)=self.blocks.get_mut(h){if !e.offloaded&&e.body_off.is_some()&&e.height>0{offload(e);}}}
        // C-1 S4: canonical journaled entries deeper than slim_depth keep only what fork choice, contexts
        // and evidence read at that depth (restored on demand, `unslim`).
        if let Some(d)=self.slim_depth{
            let floor=head_h.saturating_sub(d);
            let again:Vec<[u8;32]>=std::mem::take(&mut self.unslimmed);
            for h in again{match self.blocks.get_mut(&h){Some(e) if e.height<floor&&!e.slim&&e.body_off.is_some()&&self.canonical.get(e.height as usize)==Some(&h)=>slim(e),Some(_)=>self.unslimmed.push(h),None=>{}}}
            while self.slim_cursor<floor{
                if let Some(h)=self.canonical.get(self.slim_cursor as usize).copied(){if let Some(e)=self.blocks.get_mut(&h){if !e.slim&&e.body_off.is_some(){slim(e);}}}
                self.slim_cursor+=1;
            }
        }
        // C-1 S1: canonical history below the resident window goes off the heap too; its receipts and
        // transaction index are in history.redb (written before the journal record, invariant I3).
        if let Some(w)=self.history_window{
            let below=head_h.saturating_sub(w)+1;
            while self.history_cursor<below{
                if let Some(h)=self.canonical.get(self.history_cursor as usize).copied(){if let Some(e)=self.blocks.get_mut(&h){if !e.offloaded&&e.body_off.is_some(){offload(e);}}}
                self.history_cursor+=1;
            }
        }
    }
    /// The end of `retain`: at most side_max non-canonical blocks keep a state (unprotected and lowest-work
    /// first are pruned); then the C-1 S3a shedding of full states.
    pub fn settle(&mut self){
        debug_assert!(self.side==self.side_scan(),"the side set is the non-canonical entries");
        let side_left:Vec<[u8;32]>=self.side.iter().copied().collect();
        let mut with_state:Vec<(bool,U256,u64,[u8;32])>=side_left.into_iter().filter(|h|!self.blocks[h].pruned).map(|h|{let e=&self.blocks[&h];(self.protect.contains(&h),e.total_work,e.import_seq,h)}).collect();
        let side_max=self.spec.local().side_max as usize;
        with_state.sort();let over=with_state.len().saturating_sub(side_max);
        let drop:Vec<[u8;32]>=with_state.iter().take(over).map(|x|x.3).collect();
        let keep:Vec<[u8;32]>=with_state.iter().skip(over).map(|x|x.3).collect();
        // The pruned side states' memory goes first; their diffs stay until the kept ones are full.
        for h in &drop{if let Some(e)=self.blocks.get_mut(h){e.state=empty_state();e.stored=None;e.pruned=true;e.full=false;}}
        // C-1 S3a: every side state kept within side_max is full (materialized while the diffs that lead
        // to it are still here), as all of them were before S3a: none becomes unreachable when its
        // neighbours are pruned, and one that is adopted again answers as it did.
        // C-1 E4: a stored side state is read in place; nothing is materialized.
        if !self.keep_full_states&&self.trie_store.is_none(){for h in &keep{if self.blocks.get(h).map_or(false,|e|!e.full&&!e.pruned){
            match self.materialize(h){Some(st)=>{let e=self.blocks.get_mut(h).unwrap();e.state=st.into();e.full=true;self.materializations.fetch_add(1,std::sync::atomic::Ordering::Relaxed);}
                None=>{self.blocks.get_mut(h).unwrap().pruned=true;}}}}}
        for h in &drop{if let Some(e)=self.blocks.get_mut(h){e.diff=None;}}
        self.shed_full_states();
        // C-1 E1: only the head keeps its trie; any other parent's is derived from it when needed.
        let head=self.head;let tried=std::mem::take(&mut self.tried);
        for h in tried{if h==head{self.tried.push(h)}else if let Some(e)=self.blocks.get_mut(&h){e.trie=None;}}
        debug_assert!(self.blocks.values().all(|e|e.hash==head||e.trie.is_none()),"only the head keeps its trie");
    }
    /// C-1 S3a: drops the full state of every canonical block above the K_hist cut but the head (its diff
    /// is kept) and makes the head's state full. After it the full states are the head, genesis and the
    /// side states within side_max. A state that is then unreachable from a full one is marked pruned;
    /// none should be (side states are full, and the canonical window is linked by its diffs), so this is
    /// a guard that keeps `pruned` truthful, never a source of answers.
    fn shed_full_states(&mut self){
        // C-1 E4: every retained state is a stored version, read in place: nothing to shed or materialize.
        if self.keep_full_states||self.trie_store.is_some(){return}
        debug_assert!(self.prune_holds(),"shed starts at prune_cursor: every canonical block below it is pruned");
        let head=self.head;
        // Canonical blocks below prune_cursor are pruned (`prune_holds`): none is full.
        let canon:Vec<[u8;32]>=self.canonical.iter().skip(self.prune_cursor.max(1) as usize).copied().filter(|h|*h!=head).collect();
        for h in canon{if let Some(e)=self.blocks.get_mut(&h){if e.full&&!e.pruned{
            if e.diff.is_some(){e.state=empty_state();e.full=false;}else{e.state=empty_state();e.full=false;e.pruned=true;}}}}
        if self.blocks.get(&head).map_or(false,|e|!e.full&&!e.pruned){if let Some(s)=self.materialize(&head){let e=self.blocks.get_mut(&head).unwrap();e.state=s.into();e.full=true;self.materializations.fetch_add(1,std::sync::atomic::Ordering::Relaxed);}}
        // A state held as a diff: a canonical block from prune_cursor up. No side block is one here: `settle` has
        // just pruned each side state it dropped and made each one it kept full.
        let open:Vec<[u8;32]>=self.canonical.iter().skip(self.prune_cursor.max(1) as usize).filter(|h|self.blocks.get(*h).map_or(false,|e|!e.pruned&&!e.full)).copied().collect();
        debug_assert!({let mut a=open.clone();a.sort();let mut b:Vec<[u8;32]>=self.blocks.values().filter(|e|!e.pruned&&!e.full).map(|e|e.hash).collect();b.sort();a==b},"the diff-held states are found");
        for h in open{if self.state_path(&h).is_none(){let side=!self.is_canonical(&h);let e=self.blocks.get_mut(&h).unwrap();e.pruned=true;if side{e.diff=None;}}}
    }
    /// The diffs that lead from the full state `from` to `to`: `down` (apply_before, in order: `from` back
    /// to the common ancestor) and `up` (apply_after, in reverse: the common ancestor up to `to`). None if
    /// a block on the way has no diff.
    fn path_between(&self,from:&[u8;32],to:&[u8;32])->Option<(Vec<[u8;32]>,Vec<[u8;32]>)>{
        let (mut a,mut b)=(*from,*to);let (mut down,mut up)=(vec![],vec![]);
        while a!=b{
            let (ea,eb)=(self.blocks.get(&a)?,self.blocks.get(&b)?);
            if ea.height>=eb.height{ea.diff.as_ref()?;down.push(a);a=ea.parent?;}else{eb.diff.as_ref()?;up.push(b);b=eb.parent?;}
        }
        Some((down,up))
    }
    /// The shortest way to `to` from a full state: (base, down, up).
    fn state_path(&self,to:&[u8;32])->Option<([u8;32],Vec<[u8;32]>,Vec<[u8;32]>)>{
        self.blocks.values().filter(|e|e.full&&!e.pruned).filter_map(|e|self.path_between(&e.hash,to).map(|(d,u)|(e.hash,d,u)))
            .min_by_key(|(h,d,u)|(d.len()+u.len(),*h))
    }
    /// A block's state built from a full state and the diffs (None if unreachable).
    pub fn materialize(&self,to:&[u8;32])->Option<InMemoryDB>{
        let (base,down,up)=self.state_path(to)?;
        let mut s=InMemoryDB::clone(&self.blocks[&base].state);
        for h in &down{self.blocks[h].diff.as_ref()?.apply_before(&mut s);}
        for h in up.iter().rev(){self.blocks[h].diff.as_ref()?.apply_after(&mut s);}
        Some(s)
    }
    /// A copy of a stored block with its full state (materialized when it is not held full). A pruned
    /// block is copied as it is (its state is empty).
    pub fn full_entry(&self,h:&[u8;32])->Option<Entry>{
        let e=self.get(h)?;if e.full||e.pruned{return Some(e.clone())}
        let mut x=e.meta_clone();x.pruned=false;x.state=self.materialize(h)?.into();x.full=true;Some(x)
    }
    /// C-1 E4: a stored block's state read in place (`view`), by hash.
    pub fn view_of(&self,h:&[u8;32])->Option<StateView<'_>>{let e=self.blocks.get(h)?;self.view(e)}
    /// A stored block's state: borrowed when full, materialized otherwise; None when pruned.
    pub fn state_of(&self,h:&[u8;32])->Option<std::borrow::Cow<'_,InMemoryDB>>{
        let e=self.blocks.get(h)?;if e.pruned{return None}
        if e.full{return Some(std::borrow::Cow::Borrowed(&e.state))}
        self.materialize(h).map(std::borrow::Cow::Owned)
    }
    /// Height of the lowest common ancestor of two stored blocks (metadata walk).
    pub fn lca_height(&self,a:&[u8;32],b:&[u8;32])->Option<u64>{
        let (mut x,mut y)=(*a,*b);
        loop{
            if x==y{return self.blocks.get(&x).map(|e|e.height)}
            let (ex,ey)=(self.blocks.get(&x)?,self.blocks.get(&y)?);
            if ex.height>=ey.height{x=ex.parent?}else{y=ey.parent?}
        }
    }
    /// Replay of a journal record whose block is already stored (D-REORG-REDB). Live import journals
    /// a block only when it is absent, so a second record means live had removed it by retention and
    /// imported it again: consensus.md:205 gives such a block a NEW importSeq, so replay does the same
    /// (keeping live and replayed D133 keys order-equal). The head stays the D133 maximum.
    pub fn reimport(&mut self,hash:&[u8;32],header:Header,body_off:Option<u64>){
        // Live builds the re-imported entry from that copy (its envelope bytes may differ: the block hash
        // does not commit sig/winnerSig) and serves it from that record, so replay keeps the same copy.
        let Some(e)=self.blocks.get_mut(hash) else{return};e.import_seq=self.next_seq;self.next_seq+=1;
        let resident=!e.offloaded;e.header=header;if !resident{e.header.encoded=Vec::new();}if body_off.is_some(){e.body_off=body_off;}
        if e.slim{e.slim=false;slim(e);}
        if self.is_canonical(hash){self.recompute_head();}
        else if self.better(hash,&self.head.clone()){self.head=*hash;self.rebuild_canonical();}
    }
    /// Head = the D133 maximum over every stored block (D133: independent of the scan order).
    fn recompute_head(&mut self){
        let all:Vec<[u8;32]>=self.blocks.keys().copied().collect();let mut best=self.canonical[0];
        for h in all{if self.better(&h,&best){best=h;}}
        self.head=best;self.rebuild_canonical();
    }
    /// Whether every canonical block holds its bytes (D-REORG-REDB invariant after an import or open).
    /// Every canonical block inside the resident window is resident (all of them for a bare chain).
    pub fn canonical_resident(&self)->bool{let head_h=self.head().height;let start=self.history_window.map_or(0,|w|head_h.saturating_sub(w)+1);
        self.canonical.iter().skip(start as usize).all(|h|!self.blocks[h].offloaded)}
    /// A block's committed body: resident, or read back from the journal when offloaded.
    pub fn body_of(&self,e:&Entry,src:Option<&dyn BlockBytes>)->Result<Body,Reject>{
        if !e.offloaded{return Ok(e.body.clone())}
        let off=e.body_off.ok_or_else(||Reject::Block("offloaded block without body_off".into()))?;
        let src=src.ok_or(Reject::NeedsRebuild(e.hash))?;let (_,bb,_)=src.record_at(off)?;
        Body::decode(&bb,self.spec.cp().BODY_MAX as usize).map_err(|_|Reject::Block("offloaded body decode".into()))
    }
    /// Re-executes the stored block `e` on its parent `p` (whose state must be resident) and checks the
    /// result against the committed header and ParentResult.
    fn reexec(&self,p:&Entry,e:&Entry,body:&Body)->Result<Executed,Reject>{
        let ctx=self.child_ctx(p,e.header.a)?;let effects=self.evidence_effects(p,body,e.height)?;
        let proposer=if e.signed{Proposer::Signed(Address::from_slice(&e.header.proposer))}else{Proposer::Fallback};
        let out=self.execute(p,&ctx,e.ts,proposer,body,effects)?;
        if out.state_root().0!=e.header.state_root{return blk("rebuild stateRoot mismatch")}
        // Re-derive ParentResult from the header and the rebuilt assignment (parentResultMismatch).
        let owner=|n:u64|ctx.assignment.owner(n).map(|a|addr(&a));
        let derived=ParentResult{block_hash:B256::from(e.hash),winner:owner(e.header.nonce).ok_or_else(||Reject::Block("rebuild owner".into()))?,
            shares:e.header.shares.iter().map(|n|owner(*n).map(|o|(*n,o))).collect::<Option<Vec<_>>>().ok_or_else(||Reject::Block("rebuild share owner".into()))?};
        if Some(&derived)!=e.result.as_ref(){return blk("parentResultMismatch")}
        Ok(out)
    }
    /// D-REORG-REDB restore: after an adoption (or at the end of replay) the canonical blocks above
    /// `from_h` that were offloaded while non-canonical are made whole again: header bytes and body from
    /// the journal, receipts by re-executing the adopted suffix in order from the fork point (a rolling
    /// state; state residence is unchanged), each checked against stateRoot, receiptsRoot, gasUsed and
    /// ParentResult. Canonical blocks are therefore always resident for RPC and range serving, exactly as
    /// before this change. Returns the number of blocks restored.
    pub fn restore_canonical(&mut self,from_h:u64,src:&dyn BlockBytes)->Result<usize,Reject>{
        let head_h=self.head().height;
        // C-1 S3a: a canonical block above the K_hist cut without a diff (it was pruned as a side block) is
        // re-executed too when a retained canonical state lies below it, so that its diff links the head to
        // that state. (After a fork below the cut no retained state lies below the adopted blocks.)
        // C-1 S1: below the resident window a canonical block stays offloaded (its receipts are stored).
        // C-1 E4: the diffs are whole-state mode's. A stored entry has none and needs none (its state is a stored
        // version, and a re-executed one is only rolled), so in store mode only offloaded blocks are re-executed:
        // otherwise every import that moved the head re-executed the old head and the new one.
        let (cut,resident)=(head_h.saturating_sub(self.k_hist),self.history_window.map_or(1,|w|head_h.saturating_sub(w)+1));
        let lowest_kept=(cut+1..=head_h).find(|h|self.canonical_at(*h).map_or(false,|e|!e.pruned));
        let off=|c:&Chain,h:u64|c.canonical_at(h).map_or(false,|e|(e.offloaded&&h>=resident)||(!c.keep_full_states&&c.trie_store.is_none()&&h>cut&&e.diff.is_none()&&lowest_kept.map_or(false,|k|k<h)));
        let window_start=resident.min(cut+1);
        let Some(first)=(from_h.max(1).max(window_start)..=head_h).find(|h|off(self,*h)) else{return Ok(0)};
        let last=(first..=head_h).rev().find(|h|off(self,*h)).unwrap_or(first);
        let ph=self.canonical[first as usize-1];let (was_pruned,was_full)=(self.blocks[&ph].pruned,self.blocks[&ph].full);self.unprune(first-1);
        self.ensure_state_with(&ph,Some(src))?;
        let mut p=self.blocks[&ph].meta_clone();p.state=self.blocks[&ph].state.clone();p.stored=self.blocks[&ph].stored.clone();
        if (was_pruned||!was_full)&&ph!=self.canonical[0]{let x=self.blocks.get_mut(&ph).unwrap();x.state=empty_state();x.stored=None;x.full=false;x.pruned=was_pruned;}
        let mut n=0;
        for hgt in first..=last{
            let hash=self.canonical[hgt as usize];let er=self.meta_restored(&hash,Some(src))?;let e=&er;
            let (hb,bb)=if e.offloaded{let o=e.body_off.ok_or_else(||Reject::Block("offloaded block without body_off".into()))?;let (hb,bb,_)=src.record_at(o)?;(Some(hb),Some(bb))}else{(None,None)};
            let body=match &bb{Some(bb)=>Body::decode(bb,self.spec.cp().BODY_MAX as usize).map_err(|_|Reject::Block("offloaded body decode".into()))?,None=>e.body.clone()};
            if let Some(hb)=&hb{if decode_header(hb).map(|h|block_hash(&h)).ok()!=Some(hash){return blk("restore: journal header does not hash to the block")}}
            let out=self.reexec(&p,e,&body)?;self.restore_reexecs+=1;
            if out.receipts_root().0!=e.header.receipts_root||out.gas_used()!=e.header.gas_used{return blk("restore: receipts mismatch")}
            let x=self.blocks.get_mut(&hash).unwrap();
            // C-1 S4: a slimmed block made whole here is restored first (the next block reads its result).
            if x.slim{x.header=er.header.clone();x.result=er.result.clone();x.ranges=er.ranges.clone();x.slim=false;self.unslimmed.push(hash);}
            let x=self.blocks.get_mut(&hash).unwrap();
            // C-1 S3a: a re-executed block keeps its diff (an adopted block pruned as a side block had none); C-1
            // E3: from the write set when it ran on an overlay.
            if let Executed::Mem(o)=&out{if x.diff.is_none(){x.diff=Some(std::sync::Arc::new(crate::statediff::StateDiff::of_output(&p.state,o)));}}
            if x.offloaded{x.tx_hashes=body.txs.iter().map(|t|revm::primitives::keccak256(t)).collect();x.receipts=out.receipts().to_vec();x.body=body;
                x.header.encoded=hb.unwrap_or_default();x.offloaded=false;n+=1;}
            if x.summary.is_none()&&!x.slim{x.summary=Some(Box::new(out.summary()));}
            // State residence is unchanged (as before E4): the re-executed state is the rolling one only.
            let next=x.meta_clone();p=self.rolled(next,out)?;
        }
        Ok(n)
    }
    /// Bound of `protect` (a sync session's fetched branch): network.md:27 (sync stage R: <= 4096 headers
    /// per branch), i.e. 2·R_keep at the default R_keep.
    pub fn protect_max(&self)->usize{(2*self.r_keep) as usize}
    /// Whether retention removed this block recently (it is then not taken again).
    pub fn was_evicted(&self,hash:&[u8;32])->bool{self.evicted.contains(hash)}
    /// Non-canonical blocks, and those holding a state (A13 accounting, status).
    pub fn side_counts(&self)->(usize,usize){(self.side.len(),self.side.iter().filter(|h|self.blocks.get(*h).map_or(false,|e|!e.pruned)).count())}
    /// C-1 S4: a slimmed entry's header (the journal record's header bytes), ParentResult (its result
    /// bytes) and nonce ranges (history.redb), each checked against the entry it restores.
    fn restored_fields(&self,e:&Entry,src:&dyn BlockBytes)->Result<(Header,Option<ParentResult>,Vec<(Address,u128,u128)>),Reject>{
        let off=e.body_off.ok_or_else(||Reject::Block("slim entry without body_off".into()))?;let (hb,_,rb)=src.record_at(off)?;
        let header=decode_header(&hb).map_err(|_|Reject::Block("slim header decode".into()))?;
        if block_hash(&header)!=e.hash{return blk("slim: journal header does not hash to the block")}
        let result=decode_result(&rb,&e.hash)?;
        let ranges=src.ranges_of(&e.hash)?.ok_or_else(||Reject::Block("slim: ranges unavailable".into()))?;
        Ok((header,result,ranges))
    }
    /// A copy of `e` with its slimmed fields restored (`e` itself when it is not slim).
    pub fn restored<'a>(&self,e:&'a Entry,src:Option<&dyn BlockBytes>)->Result<std::borrow::Cow<'a,Entry>,Reject>{
        if !e.slim{return Ok(std::borrow::Cow::Borrowed(e))}
        let (mut header,result,ranges)=self.restored_fields(e,src.ok_or_else(||Reject::Block("slim entry without a block source".into()))?)?;
        if e.offloaded{header.encoded=Vec::new();}
        let mut x=e.meta_clone();x.state=empty_state();x.pruned=e.pruned;x.full=false;x.header=header;x.result=result;x.ranges=ranges;x.slim=false;
        Ok(std::borrow::Cow::Owned(x))
    }
    /// A stored block's metadata copy (no state) with its slimmed fields restored.
    fn meta_restored(&self,h:&[u8;32],src:Option<&dyn BlockBytes>)->Result<Entry,Reject>{
        let e=&self.blocks[h];if !e.slim{return Ok(e.meta_clone())}
        Ok(self.restored(e,src)?.into_owned())
    }
    /// C-1 S4: restores a slimmed entry in place (it is slimmed again by `retain` once it is deep enough).
    pub fn unslim(&mut self,h:&[u8;32],src:&dyn BlockBytes)->Result<(),Reject>{
        let e=self.blocks.get(h).ok_or_else(||Reject::Block("unslim target".into()))?;if !e.slim{return Ok(())}
        let (mut header,result,ranges)=self.restored_fields(e,src)?;
        let x=self.blocks.get_mut(h).unwrap();if x.offloaded{header.encoded=Vec::new();}
        x.header=header;x.result=result;x.ranges=ranges;x.slim=false;self.unslimmed.push(*h);Ok(())
    }
    /// TrieRebuild: re-executes from the nearest retained ancestor and checks
    /// every stateRoot against the committed header.
    pub fn ensure_state(&mut self,hash:&[u8;32])->Result<(),Reject>{self.ensure_state_with(hash,None)}
    /// As `ensure_state`, but given a `BlockBytes` source so a block whose body was offloaded
    /// (D-REORG-REDB) is re-executed from its journal record. A bare chain (nothing offloaded) passes
    /// `None`. A pruned block reached on the rebuild path that is offloaded but has no source, or no
    /// `body_off`, is an error — never a silent empty body.
    pub fn ensure_state_with(&mut self,hash:&[u8;32],src:Option<&dyn BlockBytes>)->Result<(),Reject>{
        // C-1 S4: a slimmed target is restored first (the caller needs its fields, as a parent or ancestor).
        if self.blocks.get(hash).map_or(false,|e|e.slim){self.unslim(hash,src.ok_or_else(||Reject::Block("slim entry without a block source".into()))?)?;}
        let e=self.blocks.get(hash).ok_or_else(||Reject::Block("rebuild target".into()))?;
        if e.full&&!e.pruned{return Ok(())}
        // C-1 S3a: a state that is not pruned is built from a full state through the diffs.
        if !e.pruned&&self.trie_store.is_none(){if let Some(st)=self.materialize(hash){let x=self.blocks.get_mut(hash).unwrap();let xh=x.height;self.unprune(xh);let x=self.blocks.get_mut(hash).unwrap();x.state=st.into();x.full=true;self.materializations.fetch_add(1,std::sync::atomic::Ordering::Relaxed);return Ok(())}
            self.blocks.get_mut(hash).unwrap().pruned=true;}
        // TrieRebuild from the nearest ancestor that has a state (full, or materialized).
        let mut path=vec![];let mut cur=*hash;
        let (root_state,root_stored)=loop{
            let e=self.blocks.get(&cur).ok_or_else(||Reject::Block("rebuild root".into()))?;
            // C-1 E4: a stored root state is read in place (its pin is shared).
            if let Some(pin)=&e.stored{break (empty_state(),Some(pin.clone()))}
            // C-1 E3: a full root state is shared, not copied (the rolling re-execution only reads it).
            if !e.pruned&&self.trie_store.is_none(){if e.full{break (e.state.clone(),None)}if let Some(st)=self.materialize(&cur){self.materializations.fetch_add(1,std::sync::atomic::Ordering::Relaxed);break (std::sync::Arc::new(st),None)}}
            path.push(cur);cur=e.parent.ok_or_else(||Reject::Block("rebuild root".into()))?;
        };
        let root=self.meta_restored(&cur,src)?;let root=&root;
        // The blocks above the root get diffs and the target a state (`prune_cursor`).
        self.unprune(root.height+1);
        // A13: rolling re-execution. Only the block being extended holds a state; intermediate
        // states are dropped, so a rebuild of any depth keeps at most two snapshots alive.
        let mut p=root.meta_clone();p.state=root_state;p.stored=root_stored;
        for h in path.into_iter().rev(){
            // A slimmed block on the path is re-executed from its restored fields (not kept).
            let e=self.meta_restored(&h,src)?;
            // Off-heap body: fetched and decoded from the journal when this block was offloaded.
            let body=self.body_of(&e,src)?;
            let out=self.reexec(&p,&e,&body)?;
            // C-1 S3a: the re-executed block gets its diff back (pruned blocks had none); C-1 E3: from the write
            // set when it ran on an overlay.
            if let Executed::Mem(o)=&out{if self.blocks[&h].diff.is_none(){let d=std::sync::Arc::new(crate::statediff::StateDiff::of_output(&p.state,o));self.blocks.get_mut(&h).unwrap().diff=Some(d);}}
            if !self.blocks[&h].slim&&self.blocks[&h].summary.is_none(){self.blocks.get_mut(&h).unwrap().summary=Some(Box::new(out.summary()));}
            let mut next=self.rolled(e,out)?;next.pruned=false;p=next;self.rebuilds+=1;
        }
        let x=self.blocks.get_mut(hash).ok_or_else(||Reject::Block("rebuild target".into()))?;x.state=p.state;x.stored=p.stored;x.pruned=false;x.full=true;
        Ok(())
    }
    fn key(&self,h:&[u8;32])->(u8,u64,[u8;32]){let e=&self.blocks[h];(if e.signed{0}else{1},e.import_seq,e.hash)}
    /// t1 ≻ t2: greater W, or equal W and smaller key at the children of the LCA.
    pub fn better(&self,t1:&[u8;32],t2:&[u8;32])->bool{
        if t1==t2{return false}
        let (a,b)=(&self.blocks[t1],&self.blocks[t2]);
        if a.total_work!=b.total_work{return a.total_work>b.total_work}
        let (mut x,mut y)=(*t1,*t2);let (mut cx,mut cy)=(*t1,*t2);
        while self.blocks[&x].height>self.blocks[&y].height{cx=x;x=self.blocks[&x].parent.unwrap();}
        while self.blocks[&y].height>self.blocks[&x].height{cy=y;y=self.blocks[&y].parent.unwrap();}
        if x==y{ // one is an ancestor of the other with equal work: impossible since work>=1.
            return self.blocks[t1].height>self.blocks[t2].height;
        }
        while x!=y{cx=x;cy=y;x=self.blocks[&x].parent.unwrap();y=self.blocks[&y].parent.unwrap();}
        self.key(&cx)<self.key(&cy)
    }
    pub(crate) fn rebuild_canonical(&mut self){
        // From the head back to the first block already canonical at its height. A block fixes its ancestors, so
        // the chain below that block is unchanged, and only the new branch is walked (2026-10-28: the walk from
        // the head to genesis at every insert made an import O(H) and a replay from genesis O(H²); at H_END an
        // empty block's import took 165 ms, evidence/r4). The canonical vector is always a parent-linked path.
        let mut path=Vec::new();let mut cur=Some(self.head);
        let fork=loop{
            let Some(h)=cur else{break 0};
            let e=&self.blocks[&h];
            if self.canonical.get(e.height as usize)==Some(&h){break e.height+1}
            path.push(h);cur=e.parent;
        };
        path.reverse();
        for h in self.canonical.drain((fork as usize).min(self.canonical.len())..){if self.blocks.contains_key(&h){self.side.insert(h);}}
        for h in &path{self.side.remove(h);}
        self.canonical.extend(path);
        debug_assert!({let mut full=Vec::new();let mut c=Some(self.head);while let Some(h)=c{full.push(h);c=self.blocks[&h].parent;}full.reverse();full==self.canonical},"the canonical path is the head's");
        // The first height whose canonical block changed: the prune below it still holds.
        self.unprune(fork);
    }
    /// Canonical heights from `h` up may hold what the R21 prune drops (`prune_cursor`).
    pub(crate) fn unprune(&mut self,h:u64){self.prune_cursor=self.prune_cursor.min(h.max(1));}
    /// A chain whose entries were replaced (a checkpoint's): the next `retain_unsettled` prunes and looks
    /// for orphans from the start.
    pub(crate) fn retention_reset(&mut self){self.prune_cursor=1;self.orphan_check=true;self.side=self.side_scan();}
    /// The non-canonical entries by a walk over every entry (what `side` holds).
    fn side_scan(&self)->std::collections::HashSet<[u8;32]>{self.blocks.values().filter(|e|e.height>0&&!self.is_canonical(&e.hash)).map(|e|e.hash).collect()}
    /// `prune_cursor`'s invariant: every canonical block at a height in 1..prune_cursor is pruned.
    pub fn prune_holds(&self)->bool{
        (1..self.prune_cursor).all(|h|self.canonical.get(h as usize).and_then(|x|self.blocks.get(x)).map_or(true,|e|e.pruned&&e.stored.is_none()&&!e.full&&e.diff.is_none()))
    }
    /// Whether `hash` is on the current canonical chain.
    pub fn is_canonical(&self,hash:&[u8;32])->bool{self.blocks.get(hash).map(|e|self.canonical.get(e.height as usize)==Some(hash)).unwrap_or(false)}
}
pub fn from_fixed_lp4(v:&pocol_lp4_execution::fixed::U256)->U256{U256::from_be_bytes(v.to_be_bytes())}
pub fn assignment_range(ctx:&ChildCtx,id:Address)->Option<(u64,u64)>{
    ctx.assignment.ranges().iter().find(|r|r.id()[..]==id[..]).and_then(|r|r.inclusive_end().map(|end|(r.start() as u64,end)))
}
pub fn nonce_space()->u128{assignment::NONCE_SPACE}
