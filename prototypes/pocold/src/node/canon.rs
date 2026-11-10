//! C-1 S5 (PR7): the canonical history below the in-memory window, `canon.redb`. One row per canonical height
//! below `Chain::canonical.base()`: the entry's checkpoint record (`checkpoint::rec_bytes`, what the chain held
//! for it when it was evicted), from which the entry is built back exactly with its journal record and
//! history.redb (`checkpoint::entry_of`). Derived data (invariant I1): rows are written only for blocks the
//! node validated and journaled, never read for a verdict, and rebuilt by replay; a damaged or foreign file is
//! set aside at open, as history.redb is. Rows at heights >= base are not authoritative (a crash or a head that
//! fell back can leave them behind, and the next eviction overwrites them): readers use only h < base.
use std::path::{Path,PathBuf};
use std::panic::{catch_unwind,AssertUnwindSafe};
use std::sync::atomic::{AtomicBool,AtomicU64,Ordering};
use redb::{Database,Durability,ReadableTable,TableDefinition};
use revm::primitives::keccak256;
use crate::checkpoint::{self,Rec};

/// height -> keccak(height ++ record) ++ record (`checkpoint::rec_bytes`).
const BY_HEIGHT:TableDefinition<u64,&[u8]>=TableDefinition::new("by_height");
/// block hash -> height (checked against the row at that height by its reader).
const BY_HASH:TableDefinition<&[u8;32],u64>=TableDefinition::new("by_hash");
/// "genesis" (32 bytes) and "format" (u64, big-endian): a file of another chain or layout is set aside.
const META:TableDefinition<&str,&[u8]>=TableDefinition::new("meta");
pub const FILE:&str="canon.redb";
/// Written when the index was found damaged while running; the next open sets the file aside and rebuilds it.
pub const DAMAGED:&str="canon.redb.damaged";
/// The redb page cache (A13 counts it); it also holds redb's write buffer between two durable commits.
pub const CANON_CACHE:usize=4<<20;
const FORMAT:u64=1;
/// Puts between two durable commits: redb frees pages only on a durable commit (as history.redb's HISTORY_SYNC).
pub const SYNC_EVERY:u64=256;
const IO:&str="I/O failure: ";
fn rerr(e:impl Into<redb::Error>)->String{let e:redb::Error=e.into();if crate::state_store::disk_failed(&e){format!("{IO}{e}")}else{e.to_string()}}
fn seal(h:u64,rec:&[u8])->[u8;32]{let mut b=Vec::with_capacity(8+rec.len());b.extend_from_slice(&h.to_be_bytes());b.extend_from_slice(rec);keccak256(&b).0}

pub struct Index{db:Database,dir:PathBuf,pub rebuilt:bool,damaged:AtomicBool,puts:AtomicU64,
    /// Lookups made (one per `at`, `height_of` and `range` call): serving and RPC tests bound them.
    reads:AtomicU64}
impl Index{
    /// Opens `dir/canon.redb` for the chain of `genesis`, with history.redb's policy (`History::open`): an I/O
    /// failure keeps the file and fails the open; a file redb cannot open, that fails its integrity check, that is
    /// marked damaged, or whose META names another genesis or format is moved aside (`canon.redb.aside`) and a new
    /// index is created (`rebuilt`: replay refills it, and no snapshot that needs it is used).
    pub fn open(dir:&Path,genesis:&[u8;32])->Result<Self,String>{
        let path=dir.join(FILE);let marker=dir.join(DAMAGED);
        let create=|p:&Path|->Result<Database,String>{
            let mut db=catch_unwind(AssertUnwindSafe(||{let mut b=Database::builder();b.set_cache_size(CANON_CACHE);b.create(p)}))
                .map_err(|_|"redb panicked at open".to_string())?.map_err(rerr)?;
            match catch_unwind(AssertUnwindSafe(||db.check_integrity())){Ok(Ok(true))=>{}Ok(Err(e))=>{let e=rerr(e);if e.starts_with(IO){return Err(e)}return Err("integrity check failed".into())}_=>return Err("integrity check failed".into())}
            catch_unwind(AssertUnwindSafe(||->Result<(),String>{
                let w=db.begin_write().map_err(rerr)?;
                {w.open_table(BY_HEIGHT).map_err(rerr)?;w.open_table(BY_HASH).map_err(rerr)?;let mut m=w.open_table(META).map_err(rerr)?;
                    let (g,f)=(m.get("genesis").map_err(rerr)?.map(|v|v.value().to_vec()),m.get("format").map_err(rerr)?.map(|v|v.value().to_vec()));
                    match (g,f){(None,None)=>{m.insert("genesis",&genesis[..]).map_err(rerr)?;m.insert("format",&FORMAT.to_be_bytes()[..]).map_err(rerr)?;}
                        (Some(g),Some(f)) if g==genesis[..]&&f==FORMAT.to_be_bytes()=>{}
                        _=>return Err("the index is of another genesis or format".into())}}
                w.commit().map_err(rerr)})).map_err(|_|"redb panicked at open".to_string())??;
            Ok(db)};
        let first=if marker.exists(){Err("marked damaged".to_string())}else{create(&path)};
        if let Err(e)=&first{if e.starts_with(IO){return Err(format!("canon: {e}"))}}
        let (db,rebuilt)=match first{
            Ok(db)=>(db,false),
            Err(_)=>{let aside=dir.join(format!("{FILE}.aside"));let _=std::fs::remove_file(&aside);let _=std::fs::rename(&path,&aside);
                let db=create(&path).map_err(|e|format!("canon: {e}"))?;let _=std::fs::remove_file(&marker);(db,true)}
        };
        Ok(Self{db,dir:dir.to_path_buf(),rebuilt,damaged:AtomicBool::new(false),puts:AtomicU64::new(0),reads:AtomicU64::new(0)})
    }
    pub fn is_damaged(&self)->bool{self.damaged.load(Ordering::SeqCst)}
    /// Marks the index damaged: its operations fail from now on (a `put` too, so the node's next eviction stops
    /// it) and the next open rebuilds it.
    pub fn mark_damaged(&self,why:&str)->String{
        if !self.damaged.swap(true,Ordering::SeqCst){let _=std::fs::write(self.dir.join(DAMAGED),why.as_bytes());}
        format!("canonical index damaged: {why}")
    }
    fn guard<T>(&self,f:impl FnOnce()->Result<T,String>)->Result<T,String>{
        if self.is_damaged(){return Err("canonical index damaged".into())}
        catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_|Err(self.mark_damaged("redb panicked")))
    }
    /// Writes the rows of evicted entries, (height, record), in one commit; durable every SYNC_EVERY puts and
    /// otherwise not (the journal holds every block already, so a crash only means replay writes them again).
    /// A row overwritten by another block's drops that block's hash key.
    pub fn put(&self,rows:&[(u64,Rec)])->Result<(),String>{
        if rows.is_empty(){return Ok(())}
        let n=self.puts.fetch_add(1,Ordering::SeqCst)+1;
        self.guard(||{
            let mut w=self.db.begin_write().map_err(rerr)?;if n%SYNC_EVERY!=0{w.set_durability(Durability::None);}
            {let mut t=w.open_table(BY_HEIGHT).map_err(rerr)?;let mut x=w.open_table(BY_HASH).map_err(rerr)?;
                for (h,r) in rows{
                    let b=checkpoint::rec_bytes(r);let mut v=seal(*h,&b).to_vec();v.extend_from_slice(&b);
                    let old=t.insert(*h,v.as_slice()).map_err(rerr)?.map(|o|o.value().to_vec());
                    if let Some(o)=old{if o.len()>=64&&o[32..64]!=r.hash[..]{let k:[u8;32]=o[32..64].try_into().unwrap();x.remove(&k).map_err(rerr)?;}}
                    x.insert(&r.hash,*h).map_err(rerr)?;
                }}
            w.commit().map_err(rerr)})
    }
    /// A durable commit: every earlier `put` is persisted (invariant I5: before each checkpoint).
    pub fn sync(&self)->Result<(),String>{self.guard(||{let w=self.db.begin_write().map_err(rerr)?;w.commit().map_err(rerr)})}
    fn row(&self,h:u64,v:&[u8])->Result<Rec,String>{
        if v.len()<32||v[..32]!=seal(h,&v[32..])[..]{return Err(self.mark_damaged(&format!("the row at height {h} fails its seal")))}
        checkpoint::rec_from(&v[32..]).map_err(|e|self.mark_damaged(&format!("the row at height {h}: {e}")))
    }
    /// The row at height `h` (None: no row there).
    pub fn at(&self,h:u64)->Result<Option<Rec>,String>{self.reads.fetch_add(1,Ordering::Relaxed);self.at_row(h)}
    fn at_row(&self,h:u64)->Result<Option<Rec>,String>{
        let v=self.guard(||{let r=self.db.begin_read().map_err(rerr)?;let t=r.open_table(BY_HEIGHT).map_err(rerr)?;Ok(t.get(h).map_err(rerr)?.map(|v|v.value().to_vec()))})?;
        v.map(|v|self.row(h,&v)).transpose()
    }
    /// The height whose row holds `hash`, or None (the hash key is checked against that row).
    pub fn height_of(&self,hash:&[u8;32])->Result<Option<u64>,String>{
        self.reads.fetch_add(1,Ordering::Relaxed);
        let h=self.guard(||{let r=self.db.begin_read().map_err(rerr)?;let t=r.open_table(BY_HASH).map_err(rerr)?;Ok(t.get(hash).map_err(rerr)?.map(|v|v.value()))})?;
        let Some(h)=h else{return Ok(None)};
        Ok(self.at_row(h)?.filter(|r|r.hash==*hash).map(|_|h))
    }
    /// The rows of heights start..start+n, in height order, in one read (GetRange/GetFrom below the window).
    pub fn range(&self,start:u64,n:u64)->Result<Vec<(u64,Rec)>,String>{
        self.reads.fetch_add(1,Ordering::Relaxed);
        let rows=self.guard(||{let r=self.db.begin_read().map_err(rerr)?;let t=r.open_table(BY_HEIGHT).map_err(rerr)?;
            let mut out=Vec::new();for kv in t.range(start..start.saturating_add(n)).map_err(rerr)?{let (k,v)=kv.map_err(rerr)?;out.push((k.value(),v.value().to_vec()));}Ok(out)})?;
        rows.into_iter().map(|(h,v)|Ok((h,self.row(h,&v)?))).collect()
    }
    pub fn reads(&self)->u64{self.reads.load(Ordering::Relaxed)}
    /// The file's size (0 when it cannot be read).
    pub fn file_bytes(&self)->u64{std::fs::metadata(self.dir.join(FILE)).map_or(0,|m|m.len())}
}
/// What a checkpoint restore reads (`checkpoint::Source::canonical`): the row, checked.
impl checkpoint::CanonIndex for Index{fn at(&self,h:u64)->Result<Option<Rec>,String>{Index::at(self,h)}}
