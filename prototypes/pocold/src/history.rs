//! C-1 S1 (proposals/C1-STORAGE-DESIGN.md): canonical history off the heap. `history.redb` holds what a
//! block's journal record does not: its receipts and the transaction index. Both are derived data
//! (invariant I1): they are written for every validated block before its journal record (I3), can
//! always be rebuilt by re-execution, and a damaged store is rebuilt by replay. An I/O failure stops open and preserves the store.
//! Bodies and encoded headers stay in the journal, which is never compacted (storage.md:70).
//! C-1 S4 adds a third table: each block's nonce ranges, so that an entry slimmed in memory is restored
//! exactly (its header and ParentResult come back from its journal record).
//!
//! The store is never trusted past what it can prove. Each receipts record carries keccak(block hash ++
//! record), a transaction index entry is checked by its reader against the block's transaction hashes,
//! and a panic inside redb (it asserts on some damaged files) is caught. Any of these marks the store
//! damaged: the reader gets an error and the next open sets the file aside and rebuilds it.
use std::path::{Path,PathBuf};
use std::panic::{catch_unwind,AssertUnwindSafe};
use std::sync::atomic::{AtomicBool,Ordering};
use redb::{Database,Durability,TableDefinition};
use revm::primitives::{keccak256,B256};

/// block hash -> keccak(block hash ++ record) ++ record; the record is the block's receipts, each
/// length-prefixed (u32 big-endian), in transaction order.
const RECEIPTS:TableDefinition<&[u8;32],&[u8]>=TableDefinition::new("receipts");
/// (tx hash ++ block hash) -> the transaction's index in that block (u32 big-endian). One key per block
/// that holds the transaction; a reader takes the canonical one.
const TXIDX:TableDefinition<&[u8;64],u32>=TableDefinition::new("txidx");
/// C-1 S4: block hash -> keccak(block hash ++ record) ++ record; the record is the block's nonce ranges
/// (Entry::ranges), each owner (20 bytes), start and end (16 bytes each, big-endian).
const RANGES:TableDefinition<&[u8;32],&[u8]>=TableDefinition::new("ranges");
/// A block's nonce ranges: (owner, start, end exclusive).
pub type Ranges=Vec<(revm::primitives::Address,u128,u128)>;
/// The redb page cache (A13: resources.rs counts it). Reads past it go to the file.
pub const HISTORY_CACHE:usize=16<<20;
pub const FILE:&str="history.redb";
/// Written when the store was found damaged while running; the next open rebuilds the store.
pub const DAMAGED:&str="history.redb.damaged";

pub struct History{db:Database,path:PathBuf,dir:PathBuf,pub rebuilt:bool,damaged:AtomicBool}
#[derive(Debug)]
pub struct HistoryError(pub String);
fn err(e:impl std::fmt::Display)->HistoryError{HistoryError(e.to_string())}
/// A redb error, marked when it is an I/O failure (as `state_store` marks them): open keeps the file then.
const IO:&str="I/O failure: ";
fn rerr(e:impl Into<redb::Error>)->HistoryError{let e:redb::Error=e.into();if crate::state_store::disk_failed(&e){HistoryError(format!("{IO}{e}"))}else{HistoryError(e.to_string())}}
impl HistoryError{pub fn is_io(&self)->bool{self.0.starts_with(IO)}}
fn seal(block:&[u8;32],rec:&[u8])->B256{let mut b=Vec::with_capacity(32+rec.len());b.extend_from_slice(block);b.extend_from_slice(rec);keccak256(&b)}

impl History{
    /// Opens `dir/history.redb`. An I/O failure preserves the file and fails open. A damaged file
    /// that redb cannot open (torn, truncated or foreign: an error or a
    /// panic inside redb), one that fails redb's full integrity check (every page checksum: a flipped
    /// byte in an index key is caught only there), or one marked damaged, is moved aside (as
    /// `history.redb.aside`, replacing an older one) and a new store is created: replay refills it (I1).
    /// `rebuilt` tells the caller. The check reads the whole file: about 1–1.4 s per GiB measured
    /// (evidence/c1-s1/), against the replay from genesis that open does anyway.
    pub fn open(dir:&Path)->Result<Self,HistoryError>{
        let path=dir.join(FILE);let marker=dir.join(DAMAGED);
        let create=|p:&Path|->Result<Database,HistoryError>{
            let mut db=catch_unwind(AssertUnwindSafe(||{let mut b=Database::builder();b.set_cache_size(HISTORY_CACHE);
                #[cfg(feature="io-fault")] {crate::io_fault::create(&b,p)}
                #[cfg(not(feature="io-fault"))] {b.create(p)}})).map_err(|_|HistoryError("redb panicked at open".into()))?.map_err(rerr)?;
            // Ok(false): redb found damage and repaired what it could; the store is not trusted.
            match catch_unwind(AssertUnwindSafe(||db.check_integrity())){Ok(Ok(true))=>{}Ok(Err(e))=>{let e=rerr(e);if e.is_io(){return Err(e)}return Err(HistoryError("integrity check failed".into()))}_=>return Err(HistoryError("integrity check failed".into()))}
            crate::io_point(p,"history-open-tables");
            catch_unwind(AssertUnwindSafe(||->Result<(),HistoryError>{let w=db.begin_write().map_err(rerr)?;{w.open_table(RECEIPTS).map_err(rerr)?;w.open_table(TXIDX).map_err(rerr)?;w.open_table(RANGES).map_err(rerr)?;}w.commit().map_err(rerr)}))
                .map_err(|_|HistoryError("redb panicked at open".into()))??;
            Ok(db)};
        let first=if marker.exists(){Err(HistoryError("marked damaged".into()))}else{create(&path)};
        // An I/O failure keeps the file and fails the open (review 2026-10-28, F-A); only a damaged store is set aside.
        if let Err(e)=&first{if e.is_io(){return Err(HistoryError(e.0.clone()))}}
        let (db,rebuilt)=match first{
            Ok(db)=>(db,false),
            Err(_)=>{let aside=dir.join(format!("{FILE}.aside"));let _=std::fs::remove_file(&aside);let _=std::fs::rename(&path,&aside);
                let db=create(&path)?;let _=std::fs::remove_file(&marker);(db,true)}
        };
        Ok(Self{db,path,dir:dir.to_path_buf(),rebuilt,damaged:AtomicBool::new(false)})
    }
    pub fn path(&self)->&Path{&self.path}
    /// redb's full check: every page checksum and the allocator state (`Database::check_integrity`). True
    /// only for a clean store; a store redb had to repair, or could not, is false (or an error).
    pub fn verify(&mut self)->Result<bool,HistoryError>{
        let db=&mut self.db;catch_unwind(AssertUnwindSafe(||db.check_integrity())).map_err(|_|HistoryError("redb panicked in check_integrity".into()))?.map_err(err)
    }
    /// Whether the store was found damaged since it was opened (the next open rebuilds it).
    pub fn is_damaged(&self)->bool{self.damaged.load(Ordering::SeqCst)}
    /// Marks the store damaged: its operations fail from now on and the next open rebuilds it.
    pub fn mark_damaged(&self,why:&str)->HistoryError{
        if !self.damaged.swap(true,Ordering::SeqCst){let _=std::fs::write(self.dir.join(DAMAGED),why.as_bytes());}
        HistoryError(format!("history store damaged: {why}"))
    }
    /// Runs one store operation; a panic inside redb marks the store damaged.
    fn guard<T>(&self,f:impl FnOnce()->Result<T,HistoryError>)->Result<T,HistoryError>{
        if self.is_damaged(){return Err(HistoryError("history store damaged".into()))}
        catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_|Err(self.mark_damaged("redb panicked")))
    }
    /// Whether the block's receipts and ranges are stored (a store written before S4 has no ranges: the
    /// next put of the block adds them).
    pub fn has(&self,block:&[u8;32])->Result<bool,HistoryError>{
        self.guard(||{let r=self.db.begin_read().map_err(err)?;let t=r.open_table(RECEIPTS).map_err(err)?;let g=r.open_table(RANGES).map_err(err)?;
            Ok(t.get(block).map_err(err)?.is_some()&&g.get(block).map_err(err)?.is_some())})
    }
    /// Stores a validated block's receipts and transaction index in one durable commit (live import:
    /// invariant I3). Idempotent: a block already stored is left as it is (its receipts are a function of
    /// the block and its parent).
    pub fn put(&self,block:&[u8;32],receipts:&[Vec<u8>],tx_hashes:&[B256],ranges:&[(revm::primitives::Address,u128,u128)])->Result<bool,HistoryError>{self.put_with(block,receipts,tx_hashes,ranges,true)}
    /// As `put`; a non-durable commit (`durable` false) is persisted by the next durable one. Replay uses
    /// it: the journal records exist already, so a crash before `sync` only means replay refills again.
    /// Returns whether the block was written (false: already stored).
    pub fn put_with(&self,block:&[u8;32],receipts:&[Vec<u8>],tx_hashes:&[B256],ranges:&[(revm::primitives::Address,u128,u128)],durable:bool)->Result<bool,HistoryError>{
        if self.has(block)?{return Ok(false)}
        let mut rec=Vec::with_capacity(receipts.iter().map(|r|r.len()+4).sum());
        for r in receipts{rec.extend_from_slice(&(r.len() as u32).to_be_bytes());rec.extend_from_slice(r);}
        let mut val=seal(block,&rec).to_vec();val.extend_from_slice(&rec);
        let mut rg=Vec::with_capacity(ranges.len()*52);for (a,s0,e0) in ranges{rg.extend_from_slice(a.as_slice());rg.extend_from_slice(&s0.to_be_bytes());rg.extend_from_slice(&e0.to_be_bytes());}
        let mut rval=seal(block,&rg).to_vec();rval.extend_from_slice(&rg);
        self.guard(||{
            let mut w=self.db.begin_write().map_err(err)?;if !durable{w.set_durability(Durability::None);}
            {let mut t=w.open_table(RECEIPTS).map_err(err)?;t.insert(block,val.as_slice()).map_err(err)?;
             let mut x=w.open_table(TXIDX).map_err(err)?;
             for (i,h) in tx_hashes.iter().enumerate(){let mut k=[0u8;64];k[..32].copy_from_slice(h.as_slice());k[32..].copy_from_slice(block);x.insert(&k,i as u32).map_err(err)?;}
             let mut g=w.open_table(RANGES).map_err(err)?;g.insert(block,rval.as_slice()).map_err(err)?;}
            w.commit().map_err(err)?;Ok(true)})
    }
    /// A durable commit: every earlier non-durable `put_with` is persisted.
    pub fn sync(&self)->Result<(),HistoryError>{self.guard(||{let w=self.db.begin_write().map_err(err)?;w.commit().map_err(err)})}
    /// A block's receipts, or None if the block is not stored. A record that fails its seal or does not
    /// parse marks the store damaged.
    pub fn receipts(&self,block:&[u8;32])->Result<Option<Vec<Vec<u8>>>,HistoryError>{
        let v=self.guard(||{let r=self.db.begin_read().map_err(err)?;let t=r.open_table(RECEIPTS).map_err(err)?;Ok(t.get(block).map_err(err)?.map(|v|v.value().to_vec()))})?;
        let Some(v)=v else{return Ok(None)};
        if v.len()<32||v[..32]!=seal(block,&v[32..])[..]{return Err(self.mark_damaged("receipts record fails its seal"))}
        let b=&v[32..];let (mut out,mut i)=(Vec::new(),0usize);
        while i<b.len(){
            if i+4>b.len(){return Err(self.mark_damaged("receipts record truncated"))}
            let n=u32::from_be_bytes(b[i..i+4].try_into().unwrap()) as usize;i+=4;
            if i+n>b.len(){return Err(self.mark_damaged("receipts record truncated"))}
            out.push(b[i..i+n].to_vec());i+=n;
        }
        Ok(Some(out))
    }
    /// C-1 S4: a block's nonce ranges, or None if not stored. A record that fails its seal or does not
    /// parse marks the store damaged.
    pub fn ranges(&self,block:&[u8;32])->Result<Option<Ranges>,HistoryError>{
        let v=self.guard(||{let r=self.db.begin_read().map_err(err)?;let t=r.open_table(RANGES).map_err(err)?;Ok(t.get(block).map_err(err)?.map(|v|v.value().to_vec()))})?;
        let Some(v)=v else{return Ok(None)};
        if v.len()<32||v[..32]!=seal(block,&v[32..])[..]||(v.len()-32)%52!=0{return Err(self.mark_damaged("ranges record fails its seal"))}
        Ok(Some(v[32..].chunks(52).map(|c|(revm::primitives::Address::from_slice(&c[..20]),u128::from_be_bytes(c[20..36].try_into().unwrap()),u128::from_be_bytes(c[36..52].try_into().unwrap()))).collect()))
    }
    /// Every stored (block hash, index) that holds the transaction. The caller checks each against the
    /// block's transaction hashes (`mark_damaged` on a mismatch).
    pub fn tx_locations(&self,tx:&B256)->Result<Vec<([u8;32],usize)>,HistoryError>{
        self.guard(||{
            let r=self.db.begin_read().map_err(err)?;let t=r.open_table(TXIDX).map_err(err)?;
            let mut lo=[0u8;64];lo[..32].copy_from_slice(tx.as_slice());let mut hi=lo;hi[32..].fill(0xff);
            let mut out=Vec::new();
            for kv in t.range::<&[u8;64]>(&lo..=&hi).map_err(err)?{let (k,v)=kv.map_err(err)?;let mut b=[0u8;32];b.copy_from_slice(&k.value()[32..]);out.push((b,v.value() as usize));}
            Ok(out)})
    }
}
