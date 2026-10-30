//! C-1 E4.2 (proposals/C1-STORAGE-DESIGN.md, "E4 design"): `state.redb`, the node store of the state trie.
//!
//! It keeps the nodes of every pinned state version (lp4 `node_trie`): each node under its `Id` (the
//! Keccak-256 of its serialization) with the number of stored nodes and pins that name it, the runtime
//! code by hash (C-1 E5: a leaf keeps its key's preimage). A commit stores a version's new nodes and pins its
//! root in one redb transaction; releasing a pin deletes, in one transaction, the nodes nothing names
//! any more. The store is derived data (invariant I1): it can be rebuilt from the journal, and a store
//! that does not open or is found damaged is set aside and rebuilt, as `history.redb` is.
//!
//! D-6 (2026-10-27): a checkpoint's write drops the oldest checkpoint's pins without releasing its nodes in
//! the same transaction. It owes each of its roots one reference in the `release` table, and
//! `release_step` works that list off in bounded, durable transactions between the node's events. At the network's
//! state caps the release took 8-11 s on the consensus thread (evidence/memory-r14/scale-d30dc71). At most one
//! dropped checkpoint is owed at a time: the next checkpoint's write finishes what is left first, and open
//! finishes it before anything else.
//!
//! Reads are checked: a node whose bytes do not hash to its `Id`, or do not decode, marks the store
//! damaged and is never served. A bounded cache (`NODE_CACHE` bytes, counted in A13) sits in front of the
//! node table; redb's own page cache (`STATE_PAGE_CACHE`) is set explicitly.
use std::collections::{HashMap,VecDeque};
use std::panic::{catch_unwind,AssertUnwindSafe};
use std::path::{Path,PathBuf};
use std::sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}};
use pocol_lp4_execution::node_trie::{Batch,Id,Node,NodeStore};
use redb::{Database,Durability,ReadableTable,ReadableTableMetadata,TableDefinition};
use revm::primitives::{keccak256,Bytecode,Bytes,B256};

/// node id -> reference count (u32 big-endian) ++ node encoding.
const NODES:TableDefinition<&[u8;32],&[u8]>=TableDefinition::new("node");
/// code hash -> runtime bytes (kept for good: storage.md keeps code to the end).
const CODE:TableDefinition<&[u8;32],&[u8]>=TableDefinition::new("code");
/// The store's format ("format" -> FORMAT). C-1 E5: a leaf keeps its key's preimage (format 2). D-6
/// (2026-10-27): a node's count includes what it owes in the `release` table (format 3). A format-2 store
/// opens as format 3 and is marked so: it owes nothing, or (written by a build with the table) its counts
/// already include what it owes. A build of format 2 then sets it aside rather than miss what is owed. A
/// store of any other format is set aside and rebuilt, as a damaged one is (derived data, I1).
const META:TableDefinition<&str,u64>=TableDefinition::new("meta");
pub const FORMAT:u64=3;
/// root id -> pins.
const PINS:TableDefinition<&[u8;32],u32>=TableDefinition::new("pins");
/// C-1 E4.4 (D-6): checkpoint sequence -> the sealed chain snapshot (`checkpoint::encode`).
const CKPT:TableDefinition<u64,&[u8]>=TableDefinition::new("checkpoint");
/// Checkpoint sequence -> the roots it pins (32 bytes each, a root once per entry that holds it), so a
/// checkpoint is released without decoding its snapshot.
const CKPT_ROOTS:TableDefinition<u64,&[u8]>=TableDefinition::new("checkpoint_roots");
/// D-6 (2026-10-27): node id -> the references it still owes (released but not yet taken off its count): a
/// dropped checkpoint's roots, then the children of each node the work deletes. A node's stored count is the
/// stored nodes that name it, plus its pins, plus what it owes here.
const RELEASE:TableDefinition<&[u8;32],u32>=TableDefinition::new("release");

pub const FILE:&str="state.redb";
/// Written when the store was found damaged while running; the next open rebuilds it.
pub const DAMAGED:&str="state.redb.damaged";
/// redb's page cache for state.redb (A13).
pub const STATE_PAGE_CACHE:usize=64<<20;
/// The decoded-node cache in front of the node table (A13): resident bytes held (`CACHE_ENTRY` per node and
/// what its decoded node holds, `Node::resident_bytes`).
pub const NODE_CACHE:usize=256<<20;
/// C-1 E4.5: the cache's own share per node: its map slot (the id, the `Arc` and its size) and its place in
/// the eviction order, each at twice its size (a table at most half full grows by doubling).
pub const CACHE_ENTRY:usize=2*std::mem::size_of::<(Id,(Arc<Node>,usize))>()+2*std::mem::size_of::<Id>()+1;

#[derive(Debug)]
pub struct StateStoreError(pub String);
/// What marks a redb I/O failure in an error's text (`StateStoreError::is_io`).
const IO:&str="I/O failure: ";
fn err(e:impl Into<redb::Error>)->StateStoreError{let e:redb::Error=e.into();
    if disk_failed(&e){StateStoreError(format!("{IO}{e}"))}else{StateStoreError(e.to_string())}}
/// Whether a redb error is the disk failing. redb also reports a file that is not a database it can read (a wrong
/// magic number) as `Io(InvalidData)`, and a short file as `UnexpectedEof`: those are damage, not a failing disk
/// (2026-10-28: with them counted as I/O, an open of a damaged history.redb failed instead of rebuilding it).
pub(crate) fn disk_failed(e:&redb::Error)->bool{
    match e{redb::Error::Io(x)=>!matches!(x.kind(),std::io::ErrorKind::InvalidData|std::io::ErrorKind::UnexpectedEof),redb::Error::PreviousIo=>true,_=>false}}
impl StateStoreError{
    /// redb could not read or write the file (2026-10-28): the node's own storage failed. Never a verdict on what
    /// the store holds; redb refuses the store until it is opened again (`StateStore::io_failed`).
    pub fn is_io(&self)->bool{self.0.starts_with(IO)}
}

/// Decoded nodes by id, bounded by their encodings' bytes; the oldest go first.
#[derive(Default)]
struct Cache{map:HashMap<Id,(Arc<Node>,usize)>,order:VecDeque<Id>,bytes:usize}
impl Cache{
    fn get(&self,id:&Id)->Option<Arc<Node>>{self.map.get(id).map(|x|x.0.clone())}
    fn put(&mut self,id:Id,n:Arc<Node>,len:usize,cap:usize){
        if self.map.contains_key(&id){return}
        self.map.insert(id,(n,len));self.order.push_back(id);self.bytes+=len;
        while self.bytes>cap{let Some(old)=self.order.pop_front() else{break};if let Some((_,l))=self.map.remove(&old){self.bytes-=l;}}
    }
    fn remove(&mut self,id:&Id){if let Some((_,l))=self.map.remove(id){self.bytes-=l;}}
}

pub struct StateStore{db:Database,path:PathBuf,dir:PathBuf,pub rebuilt:bool,damaged:AtomicBool,cache:Mutex<Cache>,cache_cap:usize,
    /// Set when the node closes (`close`): versions are no longer released one by one.
    closing:AtomicBool,
    /// D-6: the `release` table holds work (`release_pending`).
    owed:AtomicBool,
    /// An I/O failure was met since open (`io_failed`).
    io:AtomicBool,
    /// Replay (`set_deferred`): a version an entry stops holding is released inside the next block's commit
    /// transaction, after that block's own nodes are counted, instead of in a transaction of its own.
    deferring:AtomicBool,deferred:Mutex<Vec<Id>>,
    /// Nodes read from the table (cache misses) and nodes written, since open (measurements).
    pub reads:std::sync::atomic::AtomicU64,pub writes:std::sync::atomic::AtomicU64}

fn node_value(count:u32,enc:&[u8])->Vec<u8>{let mut v=Vec::with_capacity(4+enc.len());v.extend_from_slice(&count.to_be_bytes());v.extend_from_slice(enc);v}

impl StateStore{
    /// Opens `dir/state.redb`. A file redb cannot open (an error or a panic inside redb), one that fails
    /// redb's full integrity check, or one marked damaged, is moved aside (`state.redb.aside`) and a new,
    /// empty store is created; `rebuilt` tells the caller, which refills it from the journal (I1).
    pub fn open(dir:&Path)->Result<Self,StateStoreError>{Self::open_with(dir,NODE_CACHE)}
    pub fn open_with(dir:&Path,cache_cap:usize)->Result<Self,StateStoreError>{
        let path=dir.join(FILE);let marker=dir.join(DAMAGED);
        let create=|p:&Path|->Result<Database,StateStoreError>{
            let mut db=catch_unwind(AssertUnwindSafe(||{let mut b=Database::builder();b.set_cache_size(STATE_PAGE_CACHE);
                #[cfg(feature="io-fault")] let db=crate::io_fault::create(&b,p);#[cfg(not(feature="io-fault"))] let db=b.create(p);db})).map_err(|_|StateStoreError("redb panicked at open".into()))?.map_err(err)?;
            // An I/O error is the disk failing, not the store: it is returned (review 2026-10-28, F-A), not set aside.
            match catch_unwind(AssertUnwindSafe(||db.check_integrity())){Ok(Ok(true))=>{}Ok(Err(e))=>{let e=err(e);if e.is_io(){return Err(e)}return Err(StateStoreError("integrity check failed".into()))}_=>return Err(StateStoreError("integrity check failed".into()))}
            catch_unwind(AssertUnwindSafe(||->Result<(),StateStoreError>{let w=db.begin_write().map_err(err)?;
                {let nodes=w.open_table(NODES).map_err(err)?;w.open_table(CODE).map_err(err)?;w.open_table(PINS).map_err(err)?;
                 w.open_table(CKPT).map_err(err)?;w.open_table(CKPT_ROOTS).map_err(err)?;w.open_table(RELEASE).map_err(err)?;let mut m=w.open_table(META).map_err(err)?;
                 let format=m.get("format").map_err(err)?.map(|v|v.value());
                 match format{Some(FORMAT)=>{}Some(2)=>{m.insert("format",FORMAT).map_err(err)?;}None if nodes.is_empty().map_err(err)?=>{m.insert("format",FORMAT).map_err(err)?;}
                     f=>return Err(StateStoreError(format!("state store format {f:?}, not {FORMAT}")))}}
                w.commit().map_err(err)})).map_err(|_|StateStoreError("redb panicked at open".into()))??;
            Ok(db)};
        let first=if marker.exists(){Err(StateStoreError("marked damaged".into()))}else{create(&path)};
        // A store that cannot be read because the disk fails is kept for the next open: the open fails (the node's own
        // storage failing, as e028068 makes every other I/O failure). Only a store found damaged is set aside.
        if let Err(e)=&first{if e.is_io(){return Err(StateStoreError(e.0.clone()))}}
        let (db,rebuilt)=match first{
            Ok(db)=>(db,false),
            Err(_)=>{let aside=dir.join(format!("{FILE}.aside"));let _=std::fs::remove_file(&aside);let _=std::fs::rename(&path,&aside);
                let db=create(&path)?;let _=std::fs::remove_file(&marker);(db,true)}
        };
        let owed={let r=db.begin_read().map_err(err)?;!r.open_table(RELEASE).map_err(err)?.is_empty().map_err(err)?};
        Ok(Self{db,path,dir:dir.to_path_buf(),rebuilt,damaged:AtomicBool::new(false),cache:Mutex::new(Cache::default()),cache_cap,closing:AtomicBool::new(false),owed:AtomicBool::new(owed),io:AtomicBool::new(false),deferring:AtomicBool::new(false),deferred:Mutex::new(Vec::new()),reads:Default::default(),writes:Default::default()})
    }
    pub fn path(&self)->&Path{&self.path}
    pub fn is_damaged(&self)->bool{self.damaged.load(Ordering::SeqCst)}
    /// Marks the store damaged: nothing more is served from it and the next open rebuilds it.
    pub fn mark_damaged(&self,why:&str)->StateStoreError{
        if !self.damaged.swap(true,Ordering::SeqCst){let _=std::fs::write(self.dir.join(DAMAGED),why.as_bytes());}
        StateStoreError(format!("state store damaged: {why}"))
    }
    fn guard<T>(&self,f:impl FnOnce()->Result<T,StateStoreError>)->Result<T,StateStoreError>{
        if self.is_damaged(){return Err(StateStoreError("state store damaged".into()))}
        let r=catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_|Err(self.mark_damaged("redb panicked")));
        if let Err(e)=&r{if e.is_io(){self.io.store(true,Ordering::SeqCst);}}
        r
    }
    /// An I/O failure was met since open: redb refuses the store until it is opened again, and what failed says
    /// nothing about what the store holds (2026-10-28, independent review F1–F4). The node stops; nothing is
    /// set aside, dropped or rebuilt for it.
    pub fn io_failed(&self)->bool{self.io.load(Ordering::SeqCst)}
    /// redb's full integrity check (every page checksum).
    pub fn verify(&mut self)->Result<bool,StateStoreError>{
        let db=&mut self.db;catch_unwind(AssertUnwindSafe(||db.check_integrity())).map_err(|_|StateStoreError("redb panicked in check_integrity".into()))?.map_err(err)
    }
    /// A node read from the table and checked: its bytes hash to its id and decode. A failure marks the
    /// store damaged. A damaged store serves nothing, not even a node its cache holds.
    pub fn try_node(&self,id:&Id)->Result<Option<Arc<Node>>,StateStoreError>{
        if self.is_damaged(){return Err(StateStoreError("state store damaged".into()))}
        if let Some(n)=self.cache.lock().unwrap().get(id){return Ok(Some(n))}
        let v=self.guard(||{let r=self.db.begin_read().map_err(err)?;let t=r.open_table(NODES).map_err(err)?;Ok(t.get(id).map_err(err)?.map(|v|v.value().to_vec()))})?;
        let Some(v)=v else{return Ok(None)};
        self.reads.fetch_add(1,Ordering::Relaxed);
        if v.len()<4||keccak256(&v[4..]).0!=*id{return Err(self.mark_damaged("a node's bytes do not hash to its id"))}
        let n=Node::decode(&v[4..]).ok_or_else(||self.mark_damaged("a node does not decode"))?;let n=Arc::new(n);
        let len=CACHE_ENTRY+n.resident_bytes();self.cache.lock().unwrap().put(*id,n.clone(),len,self.cache_cap);Ok(Some(n))
    }
    /// The reference count stored with a node (None: not stored).
    pub fn count(&self,id:&Id)->Result<Option<u32>,StateStoreError>{
        self.guard(||{let r=self.db.begin_read().map_err(err)?;let t=r.open_table(NODES).map_err(err)?;Ok(t.get(id).map_err(err)?.map(|v|u32::from_be_bytes(v.value()[..4].try_into().unwrap())))})
    }
    /// D-6: the owed references (`release`), by node.
    pub fn owed_refs(&self)->Result<Vec<(Id,u32)>,StateStoreError>{
        self.guard(||{let r=self.db.begin_read().map_err(err)?;let t=r.open_table(RELEASE).map_err(err)?;let mut out=Vec::new();
            for kv in t.iter().map_err(err)?{let (k,v)=kv.map_err(err)?;out.push((*k.value(),v.value()));}Ok(out)})
    }
    pub fn pins(&self)->Result<Vec<(Id,u32)>,StateStoreError>{
        self.guard(||{let r=self.db.begin_read().map_err(err)?;let t=r.open_table(PINS).map_err(err)?;let mut out=Vec::new();
            for kv in t.iter().map_err(err)?{let (k,v)=kv.map_err(err)?;out.push((*k.value(),v.value()));}Ok(out)})
    }
    /// C-1 E4.5: the bytes of every key and value stored, over all tables (file size over this is the
    /// file factor measured in evidence/c1-e4).
    pub fn stored_bytes(&self)->Result<u64,StateStoreError>{
        self.guard(||{let r=self.db.begin_read().map_err(err)?;let mut n=0u64;
            {let t=r.open_table(NODES).map_err(err)?;for kv in t.iter().map_err(err)?{let (_,v)=kv.map_err(err)?;n+=32+v.value().len() as u64;}}
            {let t=r.open_table(CODE).map_err(err)?;for kv in t.iter().map_err(err)?{let (_,v)=kv.map_err(err)?;n+=32+v.value().len() as u64;}}
            {let t=r.open_table(PINS).map_err(err)?;n+=t.len().map_err(err)?*(32+4);}
            {let t=r.open_table(CKPT).map_err(err)?;for kv in t.iter().map_err(err)?{let (_,v)=kv.map_err(err)?;n+=8+v.value().len() as u64;}}
            {let t=r.open_table(CKPT_ROOTS).map_err(err)?;for kv in t.iter().map_err(err)?{let (_,v)=kv.map_err(err)?;n+=8+v.value().len() as u64;}}
            {let t=r.open_table(RELEASE).map_err(err)?;n+=t.len().map_err(err)?*(32+4);}
            Ok(n)})
    }
    /// C-1 E5: redb's own account of the file (measurements): (allocated pages' bytes, fragmented bytes,
    /// metadata bytes), read in a write transaction that is dropped uncommitted.
    pub fn file_stats(&self)->Result<(u64,u64,u64),StateStoreError>{
        self.guard(||{let w=self.db.begin_write().map_err(err)?;let s=w.stats().map_err(err)?;let r=(s.allocated_pages()*s.page_size() as u64,s.fragmented_bytes(),s.metadata_bytes());w.abort().map_err(err)?;Ok(r)})
    }
    /// Resident bytes the cache holds (at most its bound).
    pub fn cache_bytes(&self)->usize{self.cache.lock().unwrap().bytes}
    pub fn node_count(&self)->Result<u64,StateStoreError>{self.guard(||{let r=self.db.begin_read().map_err(err)?;let t=r.open_table(NODES).map_err(err)?;t.len().map_err(err)})}
    /// Stores the nodes of `root` that the batch holds (every node reachable from it that is not stored
    /// yet, each counting one reference to each child), the batch's code, and pins the root
    /// once, in one transaction (`durable`: fsynced at commit).
    pub fn commit(&self,b:&Batch,root:Option<Id>,durable:bool)->Result<(),StateStoreError>{
        let late=std::mem::take(&mut *self.deferred.lock().unwrap());
        let mut contradiction=None;
        let r=self.guard(||{
            let mut w=self.db.begin_write().map_err(err)?;if !durable{w.set_durability(Durability::None);}
            {
                let mut c=w.open_table(CODE).map_err(err)?;for (h,code) in &b.code{if c.get(&h.0).map_err(err)?.is_none(){c.insert(&h.0,code.original_bytes().as_ref()).map_err(err)?;}}
                let mut p=w.open_table(PINS).map_err(err)?;let mut t=w.open_table(NODES).map_err(err)?;
                if let Some(root)=root{
                    let n=p.get(&root).map_err(err)?.map_or(0,|v|v.value());p.insert(&root,n+1).map_err(err)?;
                    let mut stack=vec![root];
                    while let Some(id)=stack.pop(){
                        let cur=t.get(&id).map_err(err)?.map(|v|v.value().to_vec());
                        match cur{
                            Some(v)=>{let n=u32::from_be_bytes(v[..4].try_into().unwrap());t.insert(&id,node_value(n+1,&v[4..]).as_slice()).map_err(err)?;}
                            None=>{
                                let node=b.nodes.get(&id).ok_or_else(||StateStoreError(format!("node {} is neither stored nor in the batch",B256::from(id))))?;
                                t.insert(&id,node_value(1,&node.encode()).as_slice()).map_err(err)?;self.writes.fetch_add(1,Ordering::Relaxed);
                                stack.extend(node.children());
                            }
                        }
                    }
                }
                // The versions released late (`set_deferred`), after this block's nodes were counted: a node both
                // hold keeps its row, as when the release ran first and the batch held the node again.
                // A release that contradicts the counts (a root not pinned, a node named but not stored) fails the
                // transaction (a release is never applied in part) and marks the store damaged below.
                for r in &late{release(&mut p,&mut t,r,&self.cache).map_err(|e|{if !e.is_io(){contradiction=Some(e.0.clone());}e})?;}
            }
            crate::crash_point("state-txn");crate::io_point(&self.path,"state-txn");
            w.commit().map_err(err)
        });
        // 2026-10-28 (review F5, F6): what waited is not lost to a failed transaction (it waits again); a release that
        // contradicts the counts marks the store damaged, so the next open rebuilds it.
        if r.is_err(){let mut d=self.deferred.lock().unwrap();let mut back=late;back.extend(d.drain(..));*d=back;}
        if let Some(m)=contradiction{return Err(self.mark_damaged(&format!("a late release: {m}")))}
        r
    }
    /// Releases one pin of `root` in one transaction: every node no pin or stored node names any more is
    /// deleted, with the references it held.
    pub fn unpin(&self,root:&Id,durable:bool)->Result<(),StateStoreError>{
        self.guard(||{
            let mut w=self.db.begin_write().map_err(err)?;if !durable{w.set_durability(Durability::None);}
            {let mut p=w.open_table(PINS).map_err(err)?;let mut t=w.open_table(NODES).map_err(err)?;release(&mut p,&mut t,root,&self.cache)?;}
            w.commit().map_err(err)
        })
    }
    /// C-1 E5: the node is closing. The versions its entries pin are left pinned rather than released one
    /// transaction each as the entries drop (non-durable transactions whose pages redb reuses only after a
    /// durable commit, so a close released every version into new pages: closing after a replay from genesis
    /// grew the file from 17 MB to 34 MB and from 68 MB to 270 MB, evidence/c1-e4/disk-close-ab-*.log). The next open releases them in one durable transaction
    /// (`reset_pins`, what it does after a crash), or starts the store empty when no checkpoint is kept.
    pub fn close(&self){self.closing.store(true,Ordering::SeqCst);}
    /// Replay (2026-10-28): every version released through `TrieStore::unpin` waits for the next block's
    /// commit and is released in that transaction (at the network's state caps a release in a transaction of
    /// its own took about 45 ms, 39 s of a 229 s open, evidence/r4/open). Turning it off releases what waits,
    /// in one transaction. A crash in between leaves those versions pinned, as a crash before any release
    /// does, and the next open resets them (`reset_pins`).
    pub fn set_deferred(&self,on:bool)->Result<(),StateStoreError>{
        self.deferring.store(on,Ordering::SeqCst);if on{return Ok(())}
        let late=std::mem::take(&mut *self.deferred.lock().unwrap());if late.is_empty(){return Ok(())}
        self.guard(||{let mut w=self.db.begin_write().map_err(err)?;w.set_durability(Durability::None);
            {let mut p=w.open_table(PINS).map_err(err)?;let mut t=w.open_table(NODES).map_err(err)?;for r in &late{release(&mut p,&mut t,r,&self.cache)?;}}
            w.commit().map_err(err)})
    }
    /// Versions waiting for a late release (`set_deferred`).
    pub fn deferred_releases(&self)->usize{self.deferred.lock().unwrap().len()}
    pub fn is_deferring(&self)->bool{self.deferring.load(Ordering::SeqCst)}
    /// C-1 E4.4: the stored checkpoints, oldest first: (sequence, snapshot bytes, the roots it pins).
    pub fn checkpoints(&self)->Result<Vec<(u64,Vec<u8>,Vec<Id>)>,StateStoreError>{
        self.guard(||{let r=self.db.begin_read().map_err(err)?;let c=r.open_table(CKPT).map_err(err)?;let x=r.open_table(CKPT_ROOTS).map_err(err)?;let mut out=Vec::new();
            for kv in c.iter().map_err(err)?{let (k,v)=kv.map_err(err)?;let seq=k.value();
                let roots=x.get(seq).map_err(err)?.ok_or_else(||StateStoreError(format!("checkpoint {seq} has no roots record")))?.value().to_vec();
                if roots.len()%32!=0{return Err(StateStoreError(format!("checkpoint {seq}: roots record of {} bytes",roots.len())))}
                out.push((seq,v.value().to_vec(),roots.chunks(32).map(|c|c.try_into().unwrap()).collect()));}
            Ok(out)})
    }
    /// C-1 E4.4: the pins the stored checkpoints hold.
    pub fn checkpoint_pins(&self)->Result<u64,StateStoreError>{
        self.guard(||{let r=self.db.begin_read().map_err(err)?;let x=r.open_table(CKPT_ROOTS).map_err(err)?;let mut n=0u64;
            for kv in x.iter().map_err(err)?{n+=(kv.map_err(err)?.1.value().len()/32) as u64;}Ok(n)})
    }
    /// C-1 E4.4: stores a checkpoint under the next sequence and pins each of its roots once (as a block
    /// commit pins its root), then drops the oldest checkpoints beyond `keep`, in one durable transaction: a
    /// crash leaves the checkpoints before it or after it, never a part of one. Every root must already be
    /// stored (each is an entry's pinned version). Returns the new sequence.
    ///
    /// D-6 (2026-10-27): a dropped checkpoint's pins are taken off at once, but its nodes are owed in the
    /// `release` table rather than released here (`release_step`). What an earlier write left owed is
    /// finished first, in this transaction, so the store never owes more than one dropped checkpoint. A
    /// count that contradicts what is owed marks the store damaged (`Fault`).
    pub fn write_checkpoint(&self,snapshot:&[u8],roots:&[Id],keep:usize)->Result<u64,StateStoreError>{
        let (mut broken,mut owing)=(None,false);
        let seq=self.guard(||{
            let w=self.db.begin_write().map_err(err)?;let seq;
            {
                let mut c=w.open_table(CKPT).map_err(err)?;let mut x=w.open_table(CKPT_ROOTS).map_err(err)?;
                let mut p=w.open_table(PINS).map_err(err)?;let mut t=w.open_table(NODES).map_err(err)?;let mut o=w.open_table(RELEASE).map_err(err)?;
                while worked(work(&mut t,&mut o,usize::MAX,&self.cache),&mut broken)?>0{}
                seq=c.last().map_err(err)?.map_or(0,|(k,_)|k.value()+1);
                let flat:Vec<u8>=roots.iter().flat_map(|r|r.iter().copied()).collect();
                c.insert(seq,snapshot).map_err(err)?;x.insert(seq,flat.as_slice()).map_err(err)?;
                for r in roots{pin_stored(&mut p,&mut t,r)?;}
                let mut seqs=Vec::new();for kv in c.iter().map_err(err)?{seqs.push(kv.map_err(err)?.0.value());}
                for old in seqs.iter().take(seqs.len().saturating_sub(keep.max(1))){owe_one(&mut c,&mut x,&mut p,&mut o,*old)?;}
                owing=!o.is_empty().map_err(err)?;
            }
            crate::crash_point("checkpoint-txn");crate::io_point(&self.path,"checkpoint-txn");
            w.commit().map_err(err)?;Ok(seq)
        });
        if let Some(m)=broken{return Err(self.mark_damaged(&format!("a checkpoint's write: {m}")))}
        // Set once the write is committed: a write that failed left the table, and the flag, as they were.
        let seq=seq?;self.owed.store(owing,Ordering::SeqCst);Ok(seq)
    }
    /// D-6: whether a dropped checkpoint's nodes are still owed (`release_step` has work).
    pub fn release_pending(&self)->bool{self.owed.load(Ordering::SeqCst)}
    /// D-6: works off at most `budget` rows of the `release` table in one durable transaction (a crash loses
    /// the step, never part of it, and the work is still owed). Each row takes what its node owes off the
    /// node's count and, when none is left, deletes the node and owes one reference to each of its children.
    /// Returns the rows worked off. A step that fails stops the steps (`release_pending` is false until the
    /// next checkpoint's write or open, which finish what is owed). A storage error (an I/O error, say) is
    /// not damage: on disk the store stays whole with its checkpoints (a failed write undoes the step; after a
    /// failed fsync it may be durable). redb then refuses the store until it is opened again, so the node's
    /// next state commit fails, and the node stops (`Reject::Local`) and recovers at its next open
    /// (2026-10-28). A count that contradicts what is owed marks the store damaged, and the next open rebuilds it.
    /// Fsynced, unlike an entry's release: measured at the network's state caps, steps left unsynced made
    /// each block's durable commit write their pages (imports during a release up to 905 ms, against 209 ms
    /// with each step fsynced; evidence/memory-r14/scale-d30dc71).
    pub fn release_step(&self,budget:usize)->Result<usize,StateStoreError>{
        let (mut broken,mut done)=(None,false);
        let n=self.guard(||{
            let w=self.db.begin_write().map_err(err)?;let n;
            {let mut t=w.open_table(NODES).map_err(err)?;let mut o=w.open_table(RELEASE).map_err(err)?;
             n=worked(work(&mut t,&mut o,budget.max(1),&self.cache),&mut broken)?;done=o.is_empty().map_err(err)?;}
            crate::crash_point("release-step");crate::io_point(&self.path,"release-step");
            w.commit().map_err(err)?;Ok(n)
        });
        match n{
            Ok(n)=>{if done{self.owed.store(false,Ordering::SeqCst);}Ok(n)}
            Err(e)=>{self.owed.store(false,Ordering::SeqCst);match broken{Some(m)=>Err(self.mark_damaged(&format!("a release step: {m}"))),None=>Err(e)}}
        }
    }
    /// D-6: works off everything owed, in one durable transaction (open, before it reads a checkpoint).
    /// Returns the rows worked off. A storage error leaves the work owed; a count that contradicts what is
    /// owed marks the store damaged.
    pub fn release_all(&self)->Result<u64,StateStoreError>{
        let mut broken=None;
        let n=self.guard(||{
            let w=self.db.begin_write().map_err(err)?;let mut n=0u64;
            {let mut t=w.open_table(NODES).map_err(err)?;let mut o=w.open_table(RELEASE).map_err(err)?;
             loop{let k=worked(work(&mut t,&mut o,usize::MAX,&self.cache),&mut broken)?;if k==0{break}n+=k as u64;}}
            crate::io_point(&self.path,"release-all");
            w.commit().map_err(err)?;Ok(n)
        });
        if let Some(m)=broken{return Err(self.mark_damaged(&format!("the release at open: {m}")))}
        let n=n?;self.owed.store(false,Ordering::SeqCst);Ok(n)
    }
    /// C-1 E4.4: releases one checkpoint (one that failed verification at open) and its pins, durably.
    pub fn drop_checkpoint(&self,seq:u64)->Result<(),StateStoreError>{
        self.guard(||{
            let w=self.db.begin_write().map_err(err)?;
            {let mut c=w.open_table(CKPT).map_err(err)?;let mut x=w.open_table(CKPT_ROOTS).map_err(err)?;let mut p=w.open_table(PINS).map_err(err)?;let mut t=w.open_table(NODES).map_err(err)?;
             drop_one(&mut c,&mut x,&mut p,&mut t,seq,&self.cache)?;}
            crate::io_point(&self.path,"drop-checkpoint");
            w.commit().map_err(err)
        })
    }
    /// C-1 E4.4: at open, before any version is pinned again: the pins become exactly the stored
    /// checkpoints' (a pin of a running node's entry, or of a block whose journal record a crash cut
    /// off, is released with every node only it held), in one durable transaction. A root pinned fewer
    /// times than the checkpoints name it means the counts are not what the commits made them: an error,
    /// and the caller rebuilds the store.
    pub fn reset_pins(&self)->Result<u64,StateStoreError>{
        let mut released=0u64;
        self.guard(||{
            let w=self.db.begin_write().map_err(err)?;
            {
                let x=w.open_table(CKPT_ROOTS).map_err(err)?;let mut want:HashMap<Id,u32>=HashMap::new();
                for kv in x.iter().map_err(err)?{let (_,v)=kv.map_err(err)?;let v=v.value();if v.len()%32!=0{return Err(StateStoreError("a checkpoint roots record is not whole roots".into()))}
                    for c in v.chunks(32){*want.entry(c.try_into().unwrap()).or_insert(0)+=1;}}
                let mut p=w.open_table(PINS).map_err(err)?;let mut t=w.open_table(NODES).map_err(err)?;
                let mut held=Vec::new();for kv in p.iter().map_err(err)?{let (k,v)=kv.map_err(err)?;held.push((*k.value(),v.value()));}
                for (r,n) in &want{let h=held.iter().find(|x|x.0==*r).map_or(0,|x|x.1);if h<*n{return Err(StateStoreError(format!("root {} pinned {h} times, checkpoints name it {n} times",B256::from(*r))))}}
                for (r,n) in held{for _ in 0..n-want.get(&r).copied().unwrap_or(0){release(&mut p,&mut t,&r,&self.cache)?;released+=1;}}
            }
            w.commit().map_err(err)
        })?;
        Ok(released)
    }
    /// Recounts every node's references from the pins and what is owed (`release`): equal to the stored
    /// counts, and no stored node that no pin or owed reference reaches (a test of the counts; it reads the
    /// whole table).
    pub fn recount(&self)->Result<(),String>{
        let pins=self.pins().map_err(|e|e.0)?;let owed=self.owed_refs().map_err(|e|e.0)?;let mut count:HashMap<Id,u32>=HashMap::new();let mut seen=std::collections::HashSet::new();
        for (r,c) in pins.iter().chain(&owed){*count.entry(*r).or_insert(0)+=*c;}
        let mut stack:Vec<Id>=pins.iter().chain(&owed).map(|x|x.0).collect();
        while let Some(id)=stack.pop(){
            if !seen.insert(id){continue}
            let n=self.try_node(&id).map_err(|e|e.0)?.ok_or_else(||format!("node {} is reachable but not stored",B256::from(id)))?;
            for c in n.children(){*count.entry(c).or_insert(0)+=1;stack.push(c);}
        }
        let stored=self.node_count().map_err(|e|e.0)?;
        if seen.len() as u64!=stored{return Err(format!("{stored} nodes stored, {} reachable",seen.len()))}
        for id in &seen{let c=self.count(id).map_err(|e|e.0)?;if c!=count.get(id).copied(){return Err(format!("node {}: stored count {c:?}, recounted {:?}",B256::from(*id),count.get(id)))}}
        Ok(())
    }
    fn read<T>(&self,f:impl FnOnce(&redb::ReadTransaction)->Result<T,StateStoreError>)->Option<T>{
        match self.guard(||{let r=self.db.begin_read().map_err(err)?;f(&r)}){Ok(v)=>Some(v),Err(e)=>panic!("C-1 E4: state store read failed: {}",e.0)}
    }
}
type Pins<'a>=redb::Table<'a,&'static [u8;32],u32>;type Nodes<'a>=redb::Table<'a,&'static [u8;32],&'static [u8]>;
/// One pin of `root` released within a write transaction: every node nothing names any more is deleted, and
/// taken out of the cache as it is (a transaction that fails then leaves a stored node uncached, which is
/// only a cache miss; review 2026-10-27: no list of the deleted nodes is held, which A13 does not count).
fn release(p:&mut Pins<'_>,t:&mut Nodes<'_>,root:&Id,cache:&Mutex<Cache>)->Result<(),StateStoreError>{
    let n=p.get(root).map_err(err)?.map(|v|v.value()).ok_or_else(||StateStoreError(format!("root {} is not pinned",B256::from(*root))))?;
    if n>1{p.insert(root,n-1).map_err(err)?;}else{p.remove(root).map_err(err)?;}
    let mut stack=vec![*root];
    while let Some(id)=stack.pop(){
        let v=t.get(&id).map_err(err)?.map(|v|v.value().to_vec()).ok_or_else(||StateStoreError(format!("node {} is named but not stored",B256::from(id))))?;
        let n=u32::from_be_bytes(v[..4].try_into().unwrap());
        if n>1{t.insert(&id,node_value(n-1,&v[4..]).as_slice()).map_err(err)?;continue}
        t.remove(&id).map_err(err)?;cache.lock().unwrap().remove(&id);
        let node=Node::decode(&v[4..]).ok_or_else(||StateStoreError("a node does not decode".into()))?;stack.extend(node.children());
    }
    Ok(())
}
/// One more pin of a stored root within a write transaction (its node counts one more reference).
fn pin_stored(p:&mut Pins<'_>,t:&mut Nodes<'_>,root:&Id)->Result<(),StateStoreError>{
    let v=t.get(root).map_err(err)?.map(|v|v.value().to_vec()).ok_or_else(||StateStoreError(format!("root {} is not stored",B256::from(*root))))?;
    let n=u32::from_be_bytes(v[..4].try_into().unwrap());t.insert(root,node_value(n+1,&v[4..]).as_slice()).map_err(err)?;
    let k=p.get(root).map_err(err)?.map_or(0,|v|v.value());p.insert(root,k+1).map_err(err)?;Ok(())
}
fn drop_one(c:&mut redb::Table<'_,u64,&'static [u8]>,x:&mut redb::Table<'_,u64,&'static [u8]>,p:&mut Pins<'_>,t:&mut Nodes<'_>,seq:u64,cache:&Mutex<Cache>)->Result<(),StateStoreError>{
    let roots=x.remove(seq).map_err(err)?.map(|v|v.value().to_vec()).unwrap_or_default();c.remove(seq).map_err(err)?;
    for r in roots.chunks(32){release(p,t,&r.try_into().unwrap(),cache)?;}Ok(())
}
type Owed<'a>=redb::Table<'a,&'static [u8;32],u32>;
fn owe(o:&mut Owed<'_>,id:&Id)->Result<(),StateStoreError>{let k=o.get(id).map_err(err)?.map_or(0,|v|v.value());o.insert(id,k+1).map_err(err)?;Ok(())}
/// D-6: drops checkpoint `seq` and takes each of its pins off, owing each root's node the reference its pin
/// counted (`release`), instead of releasing the nodes now.
fn owe_one(c:&mut redb::Table<'_,u64,&'static [u8]>,x:&mut redb::Table<'_,u64,&'static [u8]>,p:&mut Pins<'_>,o:&mut Owed<'_>,seq:u64)->Result<(),StateStoreError>{
    let roots=x.remove(seq).map_err(err)?.map(|v|v.value().to_vec()).unwrap_or_default();c.remove(seq).map_err(err)?;
    for r in roots.chunks(32){let r:Id=r.try_into().unwrap();
        let n=p.get(&r).map_err(err)?.map(|v|v.value()).ok_or_else(||StateStoreError(format!("root {} is not pinned",B256::from(r))))?;
        if n>1{p.insert(&r,n-1).map_err(err)?;}else{p.remove(&r).map_err(err)?;}owe(o,&r)?;}
    Ok(())
}
/// D-6: how `work` fails. `Storage`: redb could not read or write (an I/O error, say); the transaction is
/// lost and the work is still owed. `Broken`: what is owed contradicts the stored counts (a row for a node
/// not stored, more owed than counted, a node that does not decode), so the store is damaged.
enum Fault{Storage(StateStoreError),Broken(String)}
impl From<StateStoreError> for Fault{fn from(e:StateStoreError)->Self{Fault::Storage(e)}}
/// `work`'s result inside a transaction: a storage error as it is; a broken count also noted in `broken`,
/// for the caller to mark the store damaged once the transaction is gone.
fn worked(r:Result<usize,Fault>,broken:&mut Option<String>)->Result<usize,StateStoreError>{
    match r{Ok(n)=>Ok(n),Err(Fault::Storage(e))=>Err(e),Err(Fault::Broken(m))=>{*broken=Some(m.clone());Err(StateStoreError(m))}}
}
/// D-6: works off at most `budget` rows of the `release` table: each takes what its node owes off the
/// node's count; a node left with none is deleted (and taken out of the cache, as `release` does) and owes
/// one reference to each of its children. Returns the rows worked off.
fn work(t:&mut Nodes<'_>,o:&mut Owed<'_>,budget:usize,cache:&Mutex<Cache>)->Result<usize,Fault>{
    let mut done=0;
    while done<budget{
        let Some((id,k))=o.pop_first().map_err(err)?.map(|(a,b)|(*a.value(),b.value())) else{break};
        let v=t.get(&id).map_err(err)?.map(|v|v.value().to_vec()).ok_or_else(||Fault::Broken(format!("node {} is owed a release but not stored",B256::from(id))))?;
        let n=u32::from_be_bytes(v[..4].try_into().unwrap());
        if n<k{return Err(Fault::Broken(format!("node {} owes {k} references and counts {n}",B256::from(id))))}
        if n>k{t.insert(&id,node_value(n-k,&v[4..]).as_slice()).map_err(err)?;}
        else{t.remove(&id).map_err(err)?;cache.lock().unwrap().remove(&id);
            for c in Node::decode(&v[4..]).ok_or_else(||Fault::Broken("a node does not decode".into()))?.children(){owe(o,&c)?;}}
        done+=1;
    }
    Ok(done)
}
impl NodeStore for StateStore{
    /// A damaged store panics (the caller's state cannot be read: never served). A missing node is None,
    /// which every reader of a version treats as fatal (`node_trie` asks only for nodes a stored node or a
    /// pinned root names): the store is marked damaged as well, so the next open rebuilds it (C-1 E4.4).
    fn node(&self,id:&Id)->Option<Arc<Node>>{match self.try_node(id){Ok(Some(n))=>Some(n),Ok(None)=>{self.mark_damaged(&format!("node {} is named but not stored",B256::from(*id)));None}Err(e)=>panic!("C-1 E4: {}",e.0)}}
    /// Code that does not hash to its key marks the store damaged and is never served.
    fn code(&self,h:&B256)->Option<Bytecode>{
        let b=self.read(|r|{let t=r.open_table(CODE).map_err(err)?;Ok(t.get(&h.0).map_err(err)?.map(|v|v.value().to_vec()))}).flatten()?;
        if keccak256(&b)!=*h{panic!("C-1 E4: {}",self.mark_damaged("code does not hash to its key").0)}
        Some(Bytecode::new_raw(Bytes::from(b)))
    }
}
impl pocol_lp4_execution::node_trie::TrieStore for StateStore{
    /// Durable: a block's nodes are committed before its journal record (I3). (Replay too: 2026-10-28, its
    /// commits without an fsync left every page they freed unreusable until the next durable commit, and at
    /// the network's state caps state.redb grew from 1.3 GB to more than 5.6 GB in 90 s, evidence/r4.)
    fn commit(&self,b:&Batch,root:Option<Id>)->Result<(),String>{StateStore::commit(self,b,root,true).map_err(|e|e.0)}
    fn unpin(&self,root:&Id)->Result<(),String>{if self.closing.load(Ordering::SeqCst){return Ok(())}
        if self.deferring.load(Ordering::SeqCst){self.deferred.lock().unwrap().push(*root);return Ok(())}
        StateStore::unpin(self,root,false).map_err(|e|e.0)}
    /// The pins entries hold: every pin but the checkpoints' (C-1 E4.4).
    fn pinned(&self)->Result<u64,String>{let all:u64=self.pins().map_err(|e|e.0)?.iter().map(|x|u64::from(x.1)).sum();Ok(all-self.checkpoint_pins().map_err(|e|e.0)?)}
}
