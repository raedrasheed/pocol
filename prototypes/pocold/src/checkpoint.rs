//! C-1 E4.4 (decision D-6, owner-approved: local state checkpoints; proposals/C1-STORAGE-DESIGN.md,
//! "E4.4 design"). A checkpoint lets `Node::open` resume from the chain as it was after a journal record
//! instead of re-executing every block from genesis. It is local derived data (invariant I1), never a
//! validity input: the journal stays the source of truth, nothing is deleted from it or from history.redb,
//! and a checkpoint that does not verify is released and open falls back to an older one or to genesis.
//!
//! A checkpoint is one sealed snapshot of the chain's metadata (every entry: what its journal record and
//! history.redb do not hold) together with a pin of every state version an entry holds, written in one
//! durable `state.redb` transaction (`StateStore::write_checkpoint`). It is bound to the journal prefix it
//! describes: the record count, the end offset and a rolling hash of those records' payloads.
//!
//! What open checks before it uses a checkpoint (`restore`): the seal; the journal binding; the retention
//! parameters it was written with; every entry against its journal record (the header must hash to the
//! entry's block hash, the parent must be restored, the total work must follow from it); every stored
//! version's root against the block's committed stateRoot; and every node of every stored version read, each
//! checked against its id, with each version's summary recomputed and compared with the sealed one. A version
//! whose parent's version was read is read through what differs from it (`NodeTrie::diff`, about one block's
//! writes); the others whole. Every state served afterwards is read through hash-checked nodes from a root the
//! consensus committed.
use std::collections::HashMap;
use std::sync::Arc;
use pocol_lp4_execution::{node_trie::{Id,TrieStore},state_summary::Summary};
use revm::primitives::{alloy_primitives::U512,keccak256,Address,B256,U256};
use crate::chain::{Chain,Entry,Pinned};
use crate::types::*;

/// Journal records between two checkpoints of a running or replaying node (local; `Limits::checkpoint_every`).
pub const EVERY:u64=1024;
/// Checkpoints kept (D-6: the newest and one before it, for a fallback).
pub const KEPT:usize=2;
/// C-1 S5 (PR7): format 2 describes the in-memory window (`canon_base` and the block below it) and the eviction and
/// anchor parameters. Format 1 (every entry of the chain) is still read: the one-time migration.
const MAGIC:&[u8;8]=b"POCKPT02";const MAGIC_V1:&[u8;8]=b"POCKPT01";

/// The retention parameters a snapshot was taken under. A checkpoint is used only under the same ones:
/// replay from it then keeps exactly what replay from genesis would.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Params{pub r_keep:u64,pub side_cap:u64,pub k_hist:u64,pub side_max:u64,pub history_window:Option<u64>,pub slim_depth:Option<u64>,pub keep_full:bool,
    /// C-1 S5: `Chain::evict_depth` and `Chain::anchor_step` (format 2; a format-1 snapshot reads None for both).
    pub evict_depth:Option<u64>,pub anchor_step:Option<u64>}
impl Params{
    pub fn of(c:&Chain)->Params{Params{r_keep:c.r_keep,side_cap:c.side_cap as u64,k_hist:c.k_hist,side_max:u64::from(c.spec.local().side_max),history_window:c.history_window,slim_depth:c.slim_depth,keep_full:c.keep_full_states,
        evict_depth:c.evict_depth,anchor_step:c.anchor_step}}
}
/// The journal prefix a snapshot describes: records, the offset just after them and their rolling hash.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Binding{pub records:u64,pub end:u64,pub hash:[u8;32]}
/// The rolling hash of a journal prefix, extended by one record's payload.
pub fn roll(prev:&[u8;32],payload:&[u8])->[u8;32]{let mut b=[0u8;64];b[..32].copy_from_slice(prev);b[32..].copy_from_slice(&keccak256(payload).0);keccak256(b).0}

const F_PRUNED:u16=1;const F_FULL:u16=2;const F_SLIM:u16=4;const F_OFFLOADED:u16=8;const F_FINAL:u16=16;const F_SUMMARY:u16=32;const F_STORED:u16=64;const F_WORLD:u16=128;const F_OFF:u16=256;const F_LATE:u16=512;

/// One entry as a snapshot keeps it.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Rec{pub hash:[u8;32],pub body_off:Option<u64>,pub import_seq:u64,pub total_work:U256,pub flags:u16,pub cum_txs:u64,pub cum_tx_bytes:u64,pub cum_log_bytes:u64,
    pub drawn:Address,pub summary:Option<Summary>,pub root:Option<Id>}
/// A decoded snapshot.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Snapshot{pub genesis:[u8;32],pub params:Params,pub binding:Binding,pub head:[u8;32],pub next_seq:u64,pub slim_cursor:u64,pub history_cursor:u64,pub evicted_total:u64,
    /// C-1 S5: the window's base (`Canon::base`: 0, or the lowest canonical height held other than genesis), the hash
    /// and total work of the canonical block just below it (in the node's canonical index; zero for base 0), and
    /// `Chain::anchor_cursor`. `v1`: read from a format-1 snapshot (base 0, every entry; params without the new fields).
    pub canon_base:u64,pub base_parent:[u8;32],pub base_parent_work:U256,pub anchor_cursor:u64,pub v1:bool,
    pub unslimmed:Vec<[u8;32]>,pub evicted_order:Vec<[u8;32]>,
    /// Written by replay: its protect set (blocks a later record builds on), which open recomputes from the
    /// journal and compares. None from a running node (its protect set is a sync session's).
    pub protect:Option<Vec<[u8;32]>>,pub entries:Vec<Rec>}

struct W(Vec<u8>);
impl W{fn u(&mut self,x:u64){self.0.extend_from_slice(&x.to_be_bytes())}fn b(&mut self,x:&[u8]){self.0.extend_from_slice(x)}fn o(&mut self,x:Option<u64>){match x{Some(v)=>{self.0.push(1);self.u(v)}None=>self.0.push(0)}}
    fn hs(&mut self,v:&[[u8;32]]){self.u(v.len() as u64);for h in v{self.b(h)}}}
struct R<'a>{b:&'a [u8],i:usize}
impl R<'_>{
    fn take(&mut self,n:usize)->Result<&[u8],String>{if self.b.len()-self.i<n{return Err("snapshot truncated".into())}let s=&self.b[self.i..self.i+n];self.i+=n;Ok(s)}
    fn u(&mut self)->Result<u64,String>{Ok(u64::from_be_bytes(self.take(8)?.try_into().unwrap()))}
    fn h(&mut self)->Result<[u8;32],String>{Ok(self.take(32)?.try_into().unwrap())}
    fn o(&mut self)->Result<Option<u64>,String>{match self.take(1)?[0]{0=>Ok(None),1=>Ok(Some(self.u()?)),_=>Err("snapshot option tag".into())}}
    fn n(&mut self,each:usize)->Result<usize,String>{let n=self.u()?;if n>((self.b.len()-self.i)/each) as u64{return Err("snapshot count beyond its bytes".into())}Ok(n as usize)}
    fn hs(&mut self)->Result<Vec<[u8;32]>,String>{let n=self.n(32)?;(0..n).map(|_|self.h()).collect()}
}
fn put_summary(w:&mut W,s:&Summary){
    w.b(&s.runtime.to_be_bytes());w.b(&s.code_total.to_be_bytes());for x in [s.code_missing,s.code_mismatch,s.over_runtime,s.user_accounts,s.user_slots,s.sys_slots]{w.u(x)}
    w.b(&s.balance.to_be_bytes::<64>());w.0.push(u8::from(s.contracts_clean));
}
fn get_summary(r:&mut R<'_>)->Result<Summary,String>{
    let runtime=u32::from_be_bytes(r.take(4)?.try_into().unwrap());let code_total=u128::from_be_bytes(r.take(16)?.try_into().unwrap());
    let (code_missing,code_mismatch,over_runtime,user_accounts,user_slots,sys_slots)=(r.u()?,r.u()?,r.u()?,r.u()?,r.u()?,r.u()?);
    let balance=U512::from_be_bytes::<64>(r.take(64)?.try_into().unwrap());
    let contracts_clean=match r.take(1)?[0]{0=>false,1=>true,_=>return Err("snapshot bool".into())};
    Ok(Summary{runtime,code_total,code_missing,code_mismatch,over_runtime,user_accounts,user_slots,sys_slots,balance,contracts_clean})
}

/// The snapshot of a store-mode chain after the journal prefix `b`. Entries in height order, so a parent
/// comes before its children. An entry the snapshot cannot describe (a whole state, a diff or a trie kept
/// beside a stored version) is an error: no checkpoint is written.
pub fn snapshot_of(c:&Chain,b:&Binding)->Result<Snapshot,String>{
    if c.trie_store.is_none(){return Err("a checkpoint is of a store-mode chain".into())}
    let entries=sorted(c).into_iter().map(rec_of).collect::<Result<Vec<_>,_>>()?;let (canon_base,base_parent,base_parent_work)=window_base(c)?;
    Ok(Snapshot{genesis:c.spec.genesis_hash(),params:Params::of(c),binding:*b,head:c.head,next_seq:c.next_seq,slim_cursor:c.slim_cursor,history_cursor:c.history_cursor,
        evicted_total:c.evicted_total,canon_base,base_parent,base_parent_work,anchor_cursor:c.anchor_cursor,v1:false,
        unslimmed:c.unslimmed.clone(),evicted_order:c.evicted_order.iter().copied().collect(),protect:protect_of(c),entries})
}
/// The window's base and the canonical block below it, from the base's own entry: its parent hash, and its total
/// work less its own block's work (what that parent's entry held).
fn window_base(c:&Chain)->Result<(u64,[u8;32],U256),String>{
    let b=c.canonical.base();if b<2{return Ok((0,[0;32],U256::ZERO))}
    let e=c.canonical.get(b as usize).and_then(|h|c.blocks.get(h)).ok_or("the window's lowest canonical block is not held")?;
    Ok((b,e.parent.ok_or("the window's lowest block has no parent")?,e.total_work.checked_sub(crate::chain::work(&e.target)).ok_or("the window's lowest block's total work")?))
}
fn protect_of(c:&Chain)->Option<Vec<[u8;32]>>{c.replaying.then(||{let mut v:Vec<[u8;32]>=c.protect.iter().copied().collect();v.sort();v})}
/// C-1 S5: the chain's entries are the window's, the side blocks' and genesis's.
fn sorted(c:&Chain)->Vec<&Entry>{let mut es:Vec<&Entry>=c.blocks.values().map(|e|&**e).collect();es.sort_by_key(|e|(e.height,e.hash));es}
fn rec_of(e:&Entry)->Result<Rec,String>{
    if !e.state.accounts.is_empty()||e.diff.is_some()||e.trie.is_some(){return Err(format!("entry {} holds more than a stored version",B256::from(e.hash)))}
    Ok(rec_fields(e))
}
/// An entry's record without its state (no stored version, no root): what the node's canonical index keeps
/// for an entry evicted below the window (C-1 S5), which holds no state. Such an entry is slim, so its record
/// is the smallest one, `REC_MIN` bytes.
pub fn rec_of_meta(e:&Entry)->Rec{let mut r=rec_fields(e);r.flags&=!(F_STORED|F_WORLD);r.root=None;r}
fn rec_fields(e:&Entry)->Rec{
    let mut f=0u16;
    for (on,bit) in [(e.pruned,F_PRUNED),(e.full,F_FULL),(e.slim,F_SLIM),(e.offloaded,F_OFFLOADED),(e.is_final,F_FINAL),(e.summary.is_some(),F_SUMMARY),(e.stored.is_some(),F_STORED),
        (e.stored.as_ref().map_or(false,|s|s.trie.world.is_some()),F_WORLD),(e.body_off.is_some(),F_OFF),(!e.timely,F_LATE)]{if on{f|=bit}}
    Rec{hash:e.hash,body_off:e.body_off,import_seq:e.import_seq,total_work:e.total_work,flags:f,cum_txs:e.cum_txs,cum_tx_bytes:e.cum_tx_bytes,cum_log_bytes:e.cum_log_bytes,
        drawn:e.drawn,summary:e.summary.as_deref().copied(),root:e.stored.as_ref().and_then(|s|s.trie.world)}
}
/// Most bytes one entry takes in a snapshot (A13: a checkpoint being written holds the snapshot's bytes).
/// The smallest record `put_rec` writes: hash, flags, body offset, import sequence, total work, the three cumulative
/// counts and the drawn address (no summary, no root).
pub const REC_MIN:usize=32+2+8+8+32+8*3+20;
pub const REC_MAX:usize=REC_MIN+(4+16+8*6+64+1)+32;
fn put_rec(w:&mut W,e:&Rec){
    w.b(&e.hash);w.b(&e.flags.to_be_bytes());w.u(e.body_off.unwrap_or(0));w.u(e.import_seq);w.b(&e.total_work.to_be_bytes::<32>());
    w.u(e.cum_txs);w.u(e.cum_tx_bytes);w.u(e.cum_log_bytes);w.b(e.drawn.as_slice());
    if e.flags&F_SUMMARY!=0{if let Some(x)=&e.summary{put_summary(w,x)}}
    if e.flags&F_WORLD!=0{if let Some(r)=&e.root{w.b(r)}}
}
/// One record's bytes, as a snapshot writes it (the node's canonical index stores the same bytes).
pub fn rec_bytes(e:&Rec)->Vec<u8>{let mut w=W(Vec::with_capacity(REC_MAX));put_rec(&mut w,e);w.0}
/// A record read back from `rec_bytes`: every byte is used and none is missing.
pub fn rec_from(b:&[u8])->Result<Rec,String>{let mut r=R{b,i:0};let e=get_rec(&mut r)?;if r.i!=b.len(){return Err("record has trailing bytes".into())}Ok(e)}
fn get_rec(r:&mut R<'_>)->Result<Rec,String>{
    let hash=r.h()?;let flags=u16::from_be_bytes(r.take(2)?.try_into().unwrap());let off=r.u()?;let import_seq=r.u()?;
    let total_work=U256::from_be_bytes::<32>(r.take(32)?.try_into().unwrap());let (cum_txs,cum_tx_bytes,cum_log_bytes)=(r.u()?,r.u()?,r.u()?);
    let drawn=Address::from_slice(r.take(20)?);
    let summary=if flags&F_SUMMARY!=0{Some(get_summary(r)?)}else{None};
    let root=if flags&F_WORLD!=0{Some(r.h()?)}else{None};
    if flags&F_WORLD!=0&&flags&F_STORED==0{return Err("snapshot: a root without a stored version".into())}
    Ok(Rec{hash,body_off:(flags&F_OFF!=0).then_some(off),import_seq,total_work,flags,cum_txs,cum_tx_bytes,cum_log_bytes,drawn,summary,root})
}
/// The window fields of format 2: `canon_base`, `base_parent`, `base_parent_work` and `anchor_cursor`.
type Base=(u64,[u8;32],U256,u64);
fn put_head(w:&mut W,genesis:&[u8;32],p:&Params,b:&Binding,head:&[u8;32],next_seq:u64,slim_cursor:u64,history_cursor:u64,evicted_total:u64,base:&Base,unslimmed:&[[u8;32]],evicted:&[[u8;32]],protect:&Option<Vec<[u8;32]>>,n:usize){
    w.b(MAGIC);w.b(genesis);
    w.u(p.r_keep);w.u(p.side_cap);w.u(p.k_hist);w.u(p.side_max);w.o(p.history_window);w.o(p.slim_depth);w.0.push(u8::from(p.keep_full));w.o(p.evict_depth);w.o(p.anchor_step);
    w.u(b.records);w.u(b.end);w.b(&b.hash);
    w.b(head);w.u(next_seq);w.u(slim_cursor);w.u(history_cursor);w.u(evicted_total);
    w.u(base.0);w.b(&base.1);w.b(&base.2.to_be_bytes::<32>());w.u(base.3);
    w.hs(unslimmed);w.hs(evicted);match protect{Some(v)=>{w.0.push(1);w.hs(v)}None=>w.0.push(0)}w.u(n as u64);
}
/// A snapshot's bytes, sealed (Keccak-256 of everything before the seal).
pub fn encode_snapshot(s:&Snapshot)->Vec<u8>{
    let mut w=W(Vec::with_capacity(256+s.entries.len()*REC_MAX));
    put_head(&mut w,&s.genesis,&s.params,&s.binding,&s.head,s.next_seq,s.slim_cursor,s.history_cursor,s.evicted_total,&(s.canon_base,s.base_parent,s.base_parent_work,s.anchor_cursor),&s.unslimmed,&s.evicted_order,&s.protect,s.entries.len());
    for e in &s.entries{put_rec(&mut w,e)}
    let seal=keccak256(&w.0);w.b(&seal.0);w.0
}
/// The sealed snapshot of a store-mode chain and the roots it pins (one per entry that holds a stored
/// version): `encode_snapshot(snapshot_of(c, b))`, written one entry at a time (C-1 E4.5: what a
/// checkpoint being written holds is its bytes, an index of the entries and the roots).
pub fn encode(c:&Chain,b:&Binding)->Result<(Vec<u8>,Vec<Id>),String>{
    if c.trie_store.is_none(){return Err("a checkpoint is of a store-mode chain".into())}
    let es=sorted(c);let mut w=W(Vec::with_capacity(256+(c.unslimmed.len()+c.evicted_order.len())*32+es.len()*REC_MAX));let mut roots=Vec::new();
    let evicted:Vec<[u8;32]>=c.evicted_order.iter().copied().collect();let (b0,b1,b2)=window_base(c)?;
    put_head(&mut w,&c.spec.genesis_hash(),&Params::of(c),b,&c.head,c.next_seq,c.slim_cursor,c.history_cursor,c.evicted_total,&(b0,b1,b2,c.anchor_cursor),&c.unslimmed,&evicted,&protect_of(c),es.len());
    for e in es{let r=rec_of(e)?;if let Some(x)=r.root{roots.push(x)}put_rec(&mut w,&r);}
    let seal=keccak256(&w.0);w.b(&seal.0);Ok((w.0,roots))
}

/// A snapshot read back: its seal checked, then every field; trailing or missing bytes are an error.
pub fn decode(bytes:&[u8])->Result<Snapshot,String>{
    if bytes.len()<MAGIC.len()+32{return Err("snapshot too short".into())}
    let (body,seal)=bytes.split_at(bytes.len()-32);if keccak256(body).0[..]!=seal[..]{return Err("snapshot seal does not match".into())}
    let mut r=R{b:body,i:0};let v1=match r.take(8)?{m if m==MAGIC=>false,m if m==MAGIC_V1=>true,_=>return Err("snapshot magic".into())};
    let genesis=r.h()?;
    let mut params=Params{r_keep:r.u()?,side_cap:r.u()?,k_hist:r.u()?,side_max:r.u()?,history_window:r.o()?,slim_depth:r.o()?,keep_full:match r.take(1)?[0]{0=>false,1=>true,_=>return Err("snapshot bool".into())},
        evict_depth:None,anchor_step:None};
    if !v1{params.evict_depth=r.o()?;params.anchor_step=r.o()?;}
    let binding=Binding{records:r.u()?,end:r.u()?,hash:r.h()?};
    let (head,next_seq,slim_cursor,history_cursor,evicted_total)=(r.h()?,r.u()?,r.u()?,r.u()?,r.u()?);
    let (canon_base,base_parent,base_parent_work,anchor_cursor)=if v1{(0,[0;32],U256::ZERO,0)}else{(r.u()?,r.h()?,U256::from_be_bytes::<32>(r.take(32)?.try_into().unwrap()),r.u()?)};
    if canon_base==1{return Err("snapshot: a window based at height 1".into())}
    let (unslimmed,evicted_order)=(r.hs()?,r.hs()?);
    let protect=match r.take(1)?[0]{0=>None,1=>Some(r.hs()?),_=>return Err("snapshot option tag".into())};
    // 2026-10-28: the guard took 134 bytes a record (six u64 fields; put_rec writes five), so a snapshot of mostly
    // minimal entries (past about 100,000 slimmed ones) was refused and open rebuilt from genesis (evidence/r4/hend/).
    let n=r.n(REC_MIN)?;let mut entries=Vec::with_capacity(n);
    for _ in 0..n{entries.push(get_rec(&mut r)?);}
    if r.i!=body.len(){return Err("snapshot has trailing bytes".into())}
    Ok(Snapshot{genesis,params,binding,head,next_seq,slim_cursor,history_cursor,evicted_total,canon_base,base_parent,base_parent_work,anchor_cursor,v1,unslimmed,evicted_order,protect,entries})
}
/// The roots a snapshot pins, as `encode` lists them.
pub fn roots(s:&Snapshot)->Vec<Id>{s.entries.iter().filter_map(|e|e.root).collect()}

/// What `restore` reads besides the snapshot: a journal record's payload by its frame offset (only records
/// of the bound prefix), and history.redb's receipts and nonce ranges.
/// What marks a restore error that is the node's own storage failing, not the checkpoint (2026-10-28).
pub const LOCAL:&str="local storage failure: ";
pub trait Source{
    /// Ok(None): no record of the bound prefix there (a refusal). Err: the journal could not be read, or a record
    /// open verified no longer verifies (a local failure, never a refusal: `LOCAL`).
    fn payload_at(&self,off:u64)->Result<Option<Vec<u8>>,String>;
    fn receipts(&self,hash:&[u8;32])->Result<Option<Vec<Vec<u8>>>,String>;
    fn ranges(&self,hash:&[u8;32])->Result<Option<Vec<(Address,u128,u128)>>,String>;
    /// Whether history.redb holds the block's receipts and nonce ranges.
    fn has(&self,hash:&[u8;32])->Result<bool,String>;
    /// C-1 S5: the hash and total work of the canonical block at height `h` in the node's canonical index (None: no
    /// row there). Err: the index could not be read (`LOCAL` when its file failed, a local failure), or a row failed
    /// its check (a refusal).
    fn canonical(&self,h:u64)->Result<Option<([u8;32],U256)>,String>;
}
/// C-1 S5: the node's canonical index below the window, as a restore reads it (the node implements it; a `+` file
/// names no node module). Err: one that begins with `LOCAL` is the index's file failing (an I/O error: the node's
/// own storage, on which open stops and drops nothing); any other, a row missing or failing its check, refuses the
/// snapshot.
pub trait CanonIndex{fn at(&self,h:u64)->Result<Option<Rec>,String>;}
/// An index that holds no row (a source without one; tests).
pub struct NoIndex;
impl CanonIndex for NoIndex{fn at(&self,_:u64)->Result<Option<Rec>,String>{Ok(None)}}

/// How `restore` read the stored versions: whole, or through what differs from the parent's version.
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub struct Read{pub whole:u64,pub by_diff:u64}
/// Restores a snapshot into `c`: a chain that holds only genesis, with its store enabled and the
/// retention parameters set as open sets them. Every check in the module comment is made; on any failure
/// `c` is left as it was (the versions pinned on the way are released) and the error says which check
/// failed. Returns how the stored versions were read.
pub fn restore(c:&mut Chain,s:&Snapshot,src:&dyn Source,store:&Arc<dyn TrieStore>)->Result<Read,String>{
    if s.genesis!=c.spec.genesis_hash(){return Err("snapshot of another genesis".into())}
    // A format-1 snapshot predates the eviction and anchor parameters: it is compared without them (the migration).
    let now=Params::of(c);let then=if s.v1{Params{evict_depth:now.evict_depth,anchor_step:now.anchor_step,..s.params}}else{s.params};
    if then!=now{return Err(format!("snapshot taken under other retention parameters: {:?}, now {:?}",s.params,now))}
    if s.v1&&s.canon_base!=0{return Err("a format-1 snapshot with a window base".into())}
    if c.blocks.len()!=1{return Err("restore into a chain that holds only genesis".into())}
    let cp=c.spec.cp();let (body_max,code_max)=(cp.BODY_MAX as usize,cp.B_code_max);let gh=c.spec.genesis_hash();
    // The entries are boxed as they are restored, as `Chain::blocks` keeps them: A13 counts each entry as a boxed
    // slot and one Entry (review 2026-10-27: an inline map here held about 2.5 KB more per entry at open, about
    // 320 MB at the network profile's H_END).
    let mut last=(0u64,[0u8;32]);let mut blocks:HashMap<[u8;32],Box<Entry>>=HashMap::new();
    // Each stored version's summary as read here (a slimmed entry's is not sealed), and how the versions were read.
    let (mut read_sums,mut read):(HashMap<[u8;32],Summary>,Read)=(HashMap::new(),Read::default());
    // The BLOCKHASH window a child of `p` executed with (`Chain::ancestors`).
    let ancestors=|blocks:&HashMap<[u8;32],Box<Entry>>,p:&Entry|{let mut out=HashMap::new();let mut cur=Some(p);
        while let Some(e)=cur{if p.height+1-e.height>256{break}out.insert(U256::from(e.height),B256::from(e.hash));cur=e.parent.and_then(|x|blocks.get(&x)).map(|b|&**b);}out};
    for (i,r) in s.entries.iter().enumerate(){
        let why=|m:&str|format!("entry {i} ({}): {m}",B256::from(r.hash));
        if r.flags&F_STORED!=0&&r.flags&F_WORLD==0{return Err(why("a stored empty state"))}
        let e=if i==0{
            if r.hash!=gh||r.body_off.is_some(){return Err(why("the first entry is not genesis"))}
            let mut g=(*c.blocks[&gh]).clone();
            if r.flags&F_LATE!=0{return Err(why("genesis cannot be late"))}
            if g.summary.as_deref()!=r.summary.as_ref(){return Err(why("genesis summary"))}
            // Genesis's version is the one `enable_store` just built from the admitted genesis state.
            if r.root!=g.stored.as_ref().and_then(|p|p.trie.world){if r.root.is_some(){return Err(why("genesis root"))}g.stored=None;}
            g
        }else{
            let off=r.body_off.ok_or_else(||why("no journal record"))?;
            // Every journaled block's receipts and ranges are in history.redb (written before its record,
            // I3; replay makes them durable before it checkpoints): one an old entry would read on demand
            // must be there too.
            if !src.has(&r.hash)?{return Err(why("not in history.redb"))}
            let payload=src.payload_at(off)?.ok_or_else(||why("its journal offset is not a record of the bound prefix"))?;
            let parent=|h:&crate::header::Header|->Result<(),String>{
                // C-1 S5: the window's lowest canonical block follows from the block below the window the snapshot names
                // (checked against the canonical index below); every other entry from its restored parent.
                let (ph,pw)=if s.canon_base>1&&h.h==s.canon_base&&h.parent_hash==s.base_parent{(s.canon_base-1,s.base_parent_work)}else{
                    let p=blocks.get(&h.parent_hash).ok_or_else(||why("parent not restored"))?;(p.height,p.total_work)};
                if h.h!=ph+1{return Err(why("height"))}
                if r.total_work!=pw.saturating_add(crate::chain::work(&h.target)){return Err(why("total work does not follow from the parent"))}Ok(())};
            let (mut e,_)=entry_of(r,&payload,parent,||src.receipts(&r.hash),||src.ranges(&r.hash),body_max,true,&why)?;
            if let Some(root)=r.root{
                let p=blocks.get(&e.header.parent_hash).map(|b|&**b).ok_or_else(||why("parent not restored"))?;
                // The version's state root, from its stored root node (checked against its id as it is read).
                let r=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||pocol_lp4_execution::node_trie::NodeTrie{world:Some(root)}.root(&**store,&Default::default())))
                    .map_err(|_|why("the stored root node cannot be read"))?;
                if r.0!=e.header.state_root{return Err(why("the stored version's state root is not the block's committed stateRoot"))}
                let hashes=ancestors(&blocks,p);
                e.stored=Some(Arc::new(Pinned::adopt(store.clone(),root,hashes).map_err(|x|why(&format!("pin: {x:?}")))?));
            }
            e
        };
        if (e.height,e.hash)<=last&&i>0{return Err(why("entries out of order"))}last=(e.height,e.hash);
        let mut e=e;e.pruned=r.flags&F_PRUNED!=0;e.full=r.flags&F_FULL!=0;e.import_seq=r.import_seq;e.cum_txs=r.cum_txs;e.cum_tx_bytes=r.cum_tx_bytes;e.cum_log_bytes=r.cum_log_bytes;e.drawn=r.drawn;e.is_final=r.flags&F_FINAL!=0;
        // Every node of every stored version is read: each is checked against its id as it is read, a node that
        // is missing marks the store damaged, and the summary must be the sealed one. A version whose parent's
        // version was read here is read through what differs from it (2026-10-27: reading each whole took about
        // 17 s a version at the network's state caps, evidence/memory-r14/scale-d30dc71): the nodes the diff does
        // not read are the parent's, read already, and `Summary::moved` carries its summary through the accounts and
        // slots that differ, as an import does. The diff (`NodeTrie::diff`'s `Touched`) holds every account and slot
        // that differs at once, so it is taken from the parent only: one block's write set, which A13 counts
        // (`batch_max`). From a farther ancestor it held every write between the two (review 2026-11, finding 3: an
        // anchor R_keep blocks below, or genesis below the lowest K_hist entry of a format-1 migration, every write of
        // the chain up to the state caps), which neither `open_transient` nor `migration_fits` counts. The oldest
        // version, and any whose parent's was not read (an anchor; the lowest entry above the K_hist cut unless its parent is one), is read whole,
        // streamed. A slimmed entry may still hold its version (a slim depth below K_hist): it keeps no summary, which
        // execution then computes from the version (`Chain::execute`); the version is still read.
        if let Some(pin)=&e.stored{
            let base=e.parent.and_then(|h|blocks.get(&h)).and_then(|p|Some((p.stored.clone()?,*read_sums.get(&p.hash)?)));
            let got=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||match &base{
                Some((p,sum))=>sum.moved(&p.view(),&pin.view(),&p.trie.diff(&**store,&pin.trie)),
                None=>Summary::scan_view(&pin.view(),code_max)})).map_err(|_|why("the stored version could not be read"))?;
            if base.is_some(){read.by_diff+=1}else{read.whole+=1}read_sums.insert(e.hash,got);
            match r.summary{Some(want) if got!=want=>return Err(why("the stored version's summary is not the sealed one")),None if r.flags&F_SLIM==0=>return Err(why("a stored version without its summary")),_=>{}}
        }
        blocks.insert(e.hash,Box::new(e));
    }
    if !blocks.contains_key(&s.head){return Err("the snapshot's head is not one of its entries".into())}
    if blocks.values().any(|e|e.import_seq>=s.next_seq&&e.height>0){return Err("an import sequence at or beyond next_seq".into())}
    let evicted:std::collections::HashSet<[u8;32]>=s.evicted_order.iter().copied().collect();
    if evicted.len()!=s.evicted_order.len(){return Err("evicted hashes repeat".into())}
    if s.unslimmed.iter().chain(&s.evicted_order).any(|h|*h==[0u8;32]){return Err("a zero hash".into())}
    // C-1 S5: the heights below the window are the canonical index's, which must hold the block the window follows from.
    if s.canon_base>1{
        if src.canonical(s.canon_base-1)?!=Some((s.base_parent,s.base_parent_work)){return Err("the canonical index does not reach the snapshot's window".into())}
        let low=blocks.values().filter(|e|e.height==s.canon_base&&e.parent==Some(s.base_parent)).count();
        if low!=1||blocks.values().any(|e|e.height>0&&e.height<s.canon_base){return Err("the snapshot's entries do not start at its window base".into())}
    }
    // Every entry but the window's lowest follows from a restored parent, so the head's path ends at genesis or there.
    c.blocks=blocks;c.head=s.head;c.canonical=crate::chain::Canon::at_base(gh,s.canon_base);c.rebuild_canonical();
    // C-1 S5: the anchors are the canonical entries at or below the K_hist cut that hold a stored version at a height
    // the anchor step divides (what `prune_window` leaves at a checkpoint); none holds one below the cut otherwise.
    let cut=c.head().height.saturating_sub(c.k_hist);
    c.anchors=c.anchor_step.map_or_else(Vec::new,|st|c.canonical.iter_from(c.canonical.base().max(1) as usize).filter_map(|h|c.blocks.get(h)).take_while(|e|e.height<=cut)
        .filter(|e|e.stored.is_some()&&e.height%st==0).map(|e|(e.height,e.hash)).collect());
    let top=c.anchors.last().map_or(0,|a|a.0);
    c.retention_reset();c.next_seq=s.next_seq;c.slim_cursor=s.slim_cursor;c.history_cursor=s.history_cursor;c.evicted_total=s.evicted_total;c.anchor_cursor=s.anchor_cursor.max(top);
    c.unslimmed=s.unslimmed.clone();c.evicted_order=s.evicted_order.iter().copied().collect();c.evicted=evicted;
    Ok(read)
}
/// The entry a record describes, from its journal record's payload (header, body and ParentResult bytes) and
/// history.redb's receipts and nonce ranges, asked only when the entry keeps them: the one way an entry is
/// built back from what a snapshot or the node's canonical index keeps (C-1 S5). The checks, in order: the
/// record decodes, the header hashes to the record's block, `parent` accepts the header (a restore checks the
/// parent it restored, its height and the total work), then the body, the ParentResult, the receipts and the
/// ranges. The entry is offloaded and slimmed as the record's flags say, except that `slim: false` keeps a
/// slimmed record whole (its ranges are then read: a rebuild re-executes it). Refusals are worded by `why`;
/// an error of `receipts` or `ranges` is passed on as it is (a local failure). Returns the entry and the
/// block's body, which an offloaded entry does not hold.
pub fn entry_of(r:&Rec,payload:&[u8],parent:impl FnOnce(&crate::header::Header)->Result<(),String>,receipts:impl FnOnce()->Result<Option<Vec<Vec<u8>>>,String>,
    ranges:impl FnOnce()->Result<Option<Vec<(Address,u128,u128)>>,String>,body_max:usize,slim:bool,why:&dyn Fn(&str)->String)->Result<(Entry,Body),String>{
    let record=crate::store::decode_record_ref(payload).map_err(|e|why(&format!("record: {e}")))?;
    let timely=record.first_import.map_or(true,|m|m.timely);
    if timely!=(r.flags&F_LATE==0){return Err(why("first-import flag differs from the journal"))}
    let header=decode_header(record.header).map_err(|e|why(&format!("header: {e}")))?;
    if block_hash(&header)!=r.hash{return Err(why("the journal header does not hash to the entry"))}
    parent(&header)?;
    let body=Body::decode(record.body,body_max).map_err(|e|why(&format!("body: {e}")))?;
    let result=crate::chain::decode_result(record.result,&r.hash).map_err(|e|why(&format!("result: {e:?}")))?;
    let (offloaded,slimmed)=(r.flags&F_OFFLOADED!=0,slim&&r.flags&F_SLIM!=0);
    let receipts=if offloaded{Vec::new()}else{receipts()?.ok_or_else(||why("receipts not in history.redb"))?};
    let ranges=if slimmed{Vec::new()}else{ranges()?.ok_or_else(||why("nonce ranges not in history.redb"))?};
    let tx_hashes=body.txs.iter().map(|t|keccak256(t)).collect();
    let mut e=Entry{hash:r.hash,height:header.h,parent:Some(header.parent_hash),total_work:r.total_work,import_seq:r.import_seq,signed:!header.proposer.is_empty(),timely,ts:header.ts,
        target:header.target,base_fee:from_fixed(&header.base_fee),gas_used:header.gas_used,seal:seal(&template_id(&header.ut_raw),header.nonce),state:crate::chain::empty_state(),
        is_final:r.flags&F_FINAL!=0,result,receipts,tx_hashes,cum_txs:r.cum_txs,cum_tx_bytes:r.cum_tx_bytes,cum_log_bytes:r.cum_log_bytes,pruned:r.flags&F_PRUNED!=0,full:r.flags&F_FULL!=0,diff:None,
        slim:false,drawn:r.drawn,ranges,body_off:r.body_off,offloaded:false,trie:None,summary:r.summary.map(Box::new),stored:None,header,body:if offloaded{Body::default()}else{body.clone()}};
    if offloaded{crate::chain::offload(&mut e);}
    if slimmed{crate::chain::slim(&mut e);}
    if e.summary.as_deref()!=r.summary.as_ref(){return Err(why("a slimmed entry with a summary"))}
    Ok((e,body))
}

/// A journal's records by frame offset (the prefix a snapshot is bound to) over history.redb.
/// `offsets`: the journal's record frame offsets in order (open's first pass); the bound prefix is the first
/// `records` of them. A slice searched by halving, not a map: the map was O(records) at every open and was
/// copied for each snapshot tried (PR7).
pub struct JournalSource<'a>{pub offsets:&'a [u64],pub journal:&'a crate::store::Journal,pub records:usize,pub history:&'a crate::history::History,
    /// C-1 S5: the node's canonical index (`NoIndex` for a source without one).
    pub canon:&'a dyn CanonIndex}
impl Source for JournalSource<'_>{
    /// Read back from the journal file and verified again (open no longer holds the payloads).
    fn payload_at(&self,off:u64)->Result<Option<Vec<u8>>,String>{
        if self.offsets.get(..self.records).map_or(true,|o|o.binary_search(&off).is_err()){return Ok(None)}
        // Every offset here was verified by open's first pass, so a record that no longer verifies is the local file
        // changing (review 2026-10-28, F-B): a local failure, never a refusal that would drop the checkpoint.
        match self.journal.payload_at(off){Ok(p)=>Ok(Some(p)),Err(e)=>Err(format!("{LOCAL}the journal at offset {off}: {e:?}"))}}
    // history.redb that cannot be read is a local failure (review F2), never a refusal of the checkpoint.
    fn receipts(&self,hash:&[u8;32])->Result<Option<Vec<Vec<u8>>>,String>{self.history.receipts(hash).map_err(|e|format!("{LOCAL}history: {}",e.0))}
    fn ranges(&self,hash:&[u8;32])->Result<Option<Vec<(Address,u128,u128)>>,String>{self.history.ranges(hash).map_err(|e|format!("{LOCAL}history: {}",e.0))}
    fn has(&self,hash:&[u8;32])->Result<bool,String>{self.history.has(hash).map_err(|e|format!("{LOCAL}history: {}",e.0))}
    /// A row that fails its check refuses the snapshot (the index marks itself damaged and the next open rebuilds it).
    /// An index whose file cannot be read (`LOCAL`, as the index words it) is the node's own storage failing, as the
    /// journal and history.redb here: it stays a local failure, so open stops and drops nothing. Before, it refused
    /// the snapshot, and one transient I/O error on canon.redb dropped every kept checkpoint in turn (review 2026-11,
    /// finding 4).
    fn canonical(&self,h:u64)->Result<Option<([u8;32],U256)>,String>{
        Ok(self.canon.at(h).map_err(|e|match e.strip_prefix(LOCAL){Some(m)=>format!("{LOCAL}the canonical index at height {h}: {m}"),None=>format!("the canonical index at height {h}: {e}")})?.map(|r|(r.hash,r.total_work)))}
}
/// The chain's persistent fields as a snapshot sees them (tests compare two chains with it).
pub fn fingerprint(c:&Chain)->Result<Snapshot,String>{snapshot_of(c,&Binding{records:0,end:0,hash:[0;32]})}
/// The flag bits of `Rec::flags` (tests read them).
pub mod flags{pub const PRUNED:u16=super::F_PRUNED;pub const FULL:u16=super::F_FULL;pub const SLIM:u16=super::F_SLIM;pub const OFFLOADED:u16=super::F_OFFLOADED;pub const STORED:u16=super::F_STORED;
    pub const SUMMARY:u16=super::F_SUMMARY;pub const WORLD:u16=super::F_WORLD;pub const OFF:u16=super::F_OFF;pub const LATE:u16=super::F_LATE;}
