//! Durable block journal (LP4 P9; audit A04). A block is committed iff its complete record is on
//! disk and fsynced AND the commit file counts it. Restart replays and re-executes every committed
//! block in import order, so state, receipts and fork choice (importSeq) are rebuilt exactly.
//!
//! Format v2 (`POCOLJ2`, experimental local storage, not a protocol format):
//!   journal = MAGIC(8) ‖ binding(32) ‖ record*
//!   record  = len u32 ‖ lenCheck(4) = sha256("POCOL-REC-LEN" ‖ len)[..4] ‖ sha256(payload) ‖ payload ‖ 0x01
//!   commit  = two 64-byte slots {seq u64, count u64, end u64, sha256(tag ‖ binding ‖ seq ‖ count ‖ end)}
//!             at offsets 0 and 4096 (separate sectors, so one torn sector write cannot damage both);
//!             the valid slot with the larger seq gives the committed record count and end offset.
//!             A 128-byte commit file (both slots adjacent, the first v2 layout) is read and rewritten.
//! Classification of what follows the last verified record (LP2 P3/P4):
//!   * nothing → clean;
//!   * a strict prefix of a record (header incomplete, or header check valid and record incomplete), or an
//!     all-zero suffix no longer than one maximal record → an interrupted append (torn tail): recovered
//!     into a new journal holding the verified prefix, the original bytes kept as `blocks.journal.torn-n`;
//!   * anything else (bad header check, bad digest or marker in a complete record, length over the
//!     maximum) → `Corrupt`, nothing is modified, unless it lies entirely after the committed end:
//!     with every committed record verified and at most one maximal record of bytes after them, those
//!     bytes were never committed (appends commit before the next starts), so they are an interrupted
//!     append whatever they contain (review finding: a crash can persist a later page of an append
//!     before its header page). This needs both commit slots valid or never written: with a damaged
//!     slot the surviving one may predate the last commit;
//!   * any tail after a complete record beyond the commit count → `Corrupt` (two appends in flight);
//! Fewer verified records than the commit count (damaged or truncated committed data) → `Corrupt`.
//! A v1 journal (`POCOLJ1`) is migrated only when it parses completely with no tail; otherwise the
//! store refuses to open (`LegacyNeedsReview`) and nothing is modified.
use std::fs::{self,File,OpenOptions};
use std::io::{Read,Seek,SeekFrom,Write};
use std::path::{Path,PathBuf};
use sha2::{Digest,Sha256};

const MAGIC:&[u8;8]=b"POCOLJ2\n";
const MAGIC_V1:&[u8;8]=b"POCOLJ1\n";
pub const MAX_RECORD:usize=4*1024*1024;
const HDR:usize=8;            // len + lenCheck
pub const OVERHEAD:usize=HDR+32+1; // header + digest + marker (frame bytes beyond the payload)
const SLOT:usize=64;
/// Offset of the second commit slot (its own 4 KiB sector).
const SLOT_STRIDE:usize=4096;

#[derive(Debug,PartialEq,Eq)]
pub enum StoreError{Io(String),Locked,Binding,Corrupt(u64),TooLarge,MissingJournal,MissingCommit,CommitCorrupt,LegacyNeedsReview}
impl From<std::io::Error> for StoreError{fn from(e:std::io::Error)->Self{Self::Io(e.to_string())}}

pub struct Journal{file:File,commit:File,path:PathBuf,_lock:File,binding:[u8;32],seq:u64,end:u64,pub records:u64}
/// The journal opened: its records' frame offsets (each record verified when it was read), not their
/// payloads (2026-10-28): open held every payload while it replayed, so its transient grew with the journal
/// (condition `open-replay-transient`); a payload is now read back by its offset (`payload`).
pub struct Recovered{pub journal:Journal,pub offsets:Vec<u64>,pub torn_tail:bool,pub migrated_v1:bool}
impl Recovered{
    /// Record `i`'s payload, read from the file and verified again.
    pub fn payload(&self,i:usize)->Result<Vec<u8>,StoreError>{self.journal.payload_at(*self.offsets.get(i).ok_or(StoreError::Corrupt(i as u64))?)}
    /// Every record's payload (tests).
    pub fn payloads(&self)->Vec<Vec<u8>>{(0..self.offsets.len()).map(|i|self.payload(i).expect("a record read back")).collect()}
}

fn sha(b:&[u8])->[u8;32]{Sha256::digest(b).into()}
fn len_check(len:u32)->[u8;4]{let mut h=Sha256::new();h.update(b"POCOL-REC-LEN");h.update(len.to_be_bytes());let d:[u8;32]=h.finalize().into();[d[0],d[1],d[2],d[3]]}
fn slot_check(binding:&[u8;32],seq:u64,count:u64,end:u64)->[u8;32]{
    let mut h=Sha256::new();h.update(b"POCOL-COMMIT");h.update(binding);h.update(seq.to_be_bytes());h.update(count.to_be_bytes());h.update(end.to_be_bytes());h.finalize().into()
}
fn slot_bytes(binding:&[u8;32],seq:u64,count:u64,end:u64)->[u8;SLOT]{
    let mut s=[0u8;SLOT];s[..8].copy_from_slice(&seq.to_be_bytes());s[8..16].copy_from_slice(&count.to_be_bytes());s[16..24].copy_from_slice(&end.to_be_bytes());
    s[24..56].copy_from_slice(&slot_check(binding,seq,count,end));s
}
/// The valid commit slot with the larger seq: (seq, count, end), and whether a slot is damaged
/// (neither valid nor never written, i.e. all zero).
fn read_commit(bytes:&[u8],binding:&[u8;32])->Option<((u64,u64,u64),bool)>{
    let second=match bytes.len(){n if n==SLOT_STRIDE+SLOT=>SLOT_STRIDE,n if n==2*SLOT=>SLOT,_=>return None};
    let parse=|s:&[u8]|{let u=|i:usize|u64::from_be_bytes(s[i..i+8].try_into().unwrap());let (seq,count,end)=(u(0),u(8),u(16));
        (s[24..56]==slot_check(binding,seq,count,end)[..]&&s[56..].iter().all(|b|*b==0)).then_some((seq,count,end))};
    let (s0,s1)=(&bytes[..SLOT],&bytes[second..second+SLOT]);
    let damaged=[s0,s1].iter().any(|s|parse(s).is_none()&&s.iter().any(|b|*b!=0));
    let best=match (parse(s0),parse(s1)){(Some(a),Some(b))=>Some(if a.0>=b.0{a}else{b}),(Some(a),None)|(None,Some(a))=>Some(a),_=>None};
    best.map(|b|(b,damaged))
}
pub fn encode_frame(payload:&[u8])->Vec<u8>{
    let mut rec=Vec::with_capacity(payload.len()+OVERHEAD);let len=payload.len() as u32;
    rec.extend(len.to_be_bytes());rec.extend(len_check(len));rec.extend(sha(payload));rec.extend(payload);rec.push(1);rec
}
enum Tail{Clean,Torn,Corrupt}
/// Parses v2 records of the file `f` (`total` bytes) from `pos`, reading one record at a time and handing
/// each verified payload to `visit` (its index and frame offset); returns (the records' frame offsets, end
/// of the verified prefix, tail classification). The checks and the classification are the ones the
/// whole-file parse made: at most one record's bytes are held.
fn parse_v2(f:&mut File,total:u64,mut pos:u64,visit:&mut dyn FnMut(usize,u64,&[u8]))->Result<(Vec<u64>,u64,Tail),StoreError>{
    let mut out=Vec::new();let mut buf=Vec::new();
    // The rest of the file is all zero and no longer than one maximal record (an interrupted append).
    let zero_fill=|f:&mut File,pos:u64|->Result<bool,StoreError>{let n=total-pos;if n>(MAX_RECORD+OVERHEAD) as u64{return Ok(false)}
        let mut r=vec![0u8;n as usize];f.seek(SeekFrom::Start(pos))?;f.read_exact(&mut r)?;Ok(r.iter().all(|b|*b==0))};
    f.seek(SeekFrom::Start(pos))?;let mut rd=std::io::BufReader::with_capacity(1<<20,&*f);
    loop{
        let rest=total-pos;
        if rest==0{return Ok((out,pos,Tail::Clean))}
        if rest<HDR as u64{return Ok((out,pos,Tail::Torn))}
        let mut hdr=[0u8;HDR];rd.read_exact(&mut hdr)?;
        let len=u32::from_be_bytes(hdr[..4].try_into().unwrap());
        if hdr[4..8]!=len_check(len)[..]{drop(rd);return Ok((out,pos,if zero_fill(f,pos)?{Tail::Torn}else{Tail::Corrupt}))}
        let len=len as usize;if len>MAX_RECORD{return Ok((out,pos,Tail::Corrupt))}
        if rest<(len+OVERHEAD) as u64{return Ok((out,pos,Tail::Torn))}
        buf.resize(32+len+1,0);rd.read_exact(&mut buf)?;
        let (digest,payload,marker)=(&buf[..32],&buf[32..32+len],buf[32+len]);
        if marker!=1||sha(payload)[..]!=digest[..]{return Ok((out,pos,Tail::Corrupt))}
        visit(out.len(),pos,payload);out.push(pos);pos+=(len+OVERHEAD) as u64;
    }
}
/// Copies the first `n` bytes of `src` to `dst` (fsynced).
fn copy_prefix(src:&Path,dst:&Path,n:u64)->Result<(),StoreError>{
    let mut r=File::open(src)?.take(n);let mut w=File::create(dst)?;std::io::copy(&mut r,&mut w)?;w.sync_all()?;Ok(())
}
/// Strict v1 parse (len ‖ digest ‖ payload ‖ marker): Some only when every byte belongs to a valid record.
fn parse_v1_strict(bytes:&[u8])->Option<Vec<Vec<u8>>>{
    let mut pos=40;let mut out=Vec::new();
    while pos<bytes.len(){
        if bytes.len()-pos<37{return None}
        let len=u32::from_be_bytes(bytes[pos..pos+4].try_into().unwrap()) as usize;if len>MAX_RECORD||pos+37+len>bytes.len(){return None}
        let digest=&bytes[pos+4..pos+36];let payload=&bytes[pos+36..pos+36+len];if bytes[pos+36+len]!=1||sha(payload)[..]!=digest[..]{return None}
        out.push(payload.to_vec());pos+=37+len;
    }
    Some(out)
}
/// A commit file holding one valid slot (`seq`, `count`, `end`) in the current layout.
fn commit_file(binding:&[u8;32],seq:u64,count:u64,end:u64)->Vec<u8>{
    let mut c=vec![0u8;SLOT_STRIDE+SLOT];let off=(seq%2) as usize*SLOT_STRIDE;c[off..off+SLOT].copy_from_slice(&slot_bytes(binding,seq,count,end));c
}
fn write_new(path:&Path,dir:&Path,bytes:&[u8])->Result<(),StoreError>{
    let tmp=path.with_extension("tmp");{let mut f=File::create(&tmp)?;f.write_all(bytes)?;f.sync_all()?;}
    fs::rename(&tmp,path)?;sync_dir(dir);Ok(())
}
fn backup_path(dir:&Path,stem:&str)->PathBuf{let n=(0..).find(|i|!dir.join(format!("{stem}-{i}")).exists()).unwrap();dir.join(format!("{stem}-{n}"))}
fn backup(dir:&Path,stem:&str,bytes:&[u8])->Result<(),StoreError>{let mut f=File::create(backup_path(dir,stem))?;f.write_all(bytes)?;f.sync_all()?;Ok(())}

/// Opens (or creates) the journal in `dir`, holding an exclusive OS lock on `dir/LOCK` for the
/// process lifetime (second writer => Locked).
pub fn open(dir:&Path,binding:&[u8;32])->Result<Recovered,StoreError>{open_with(dir,binding,&mut |_,_,_|{})}
/// `open`, handing each verified record's payload to `visit` (its index and frame offset) as it is read,
/// before the tail is classified: a caller keeps what it needs of each record, and nothing if open fails.
pub fn open_with(dir:&Path,binding:&[u8;32],visit:&mut dyn FnMut(usize,u64,&[u8]))->Result<Recovered,StoreError>{
    fs::create_dir_all(dir)?;
    let lock=OpenOptions::new().create(true).truncate(false).write(true).open(dir.join("LOCK"))?;
    match lock.try_lock(){Ok(())=>{},Err(std::fs::TryLockError::WouldBlock)=>return Err(StoreError::Locked),Err(std::fs::TryLockError::Error(e))=>return Err(e.into())}
    let path=dir.join("blocks.journal");let cpath=dir.join("blocks.commit");
    let _=fs::remove_file(dir.join("blocks.tmp"));let _=fs::remove_file(dir.join("blocks.journal.tmp"));
    if !path.exists(){
        // Never silently restart from genesis when recovery artifacts or a commit file exist.
        let artifacts=fs::read_dir(dir)?.filter_map(|e|e.ok()).any(|e|{let n=e.file_name().to_string_lossy().to_string();n.starts_with("blocks.journal.torn-")||n.starts_with("blocks.journal.v1-")});
        if artifacts||cpath.exists(){return Err(StoreError::MissingJournal)}
        let mut head=Vec::with_capacity(40);head.extend(MAGIC);head.extend(binding);
        write_new(&cpath,dir,&commit_file(binding,0,0,40))?;
        write_new(&path,dir,&head)?;
    }
    let mut head=[0u8;40];let head_len={let mut f=File::open(&path)?;let mut n=0;while n<40{let k=f.read(&mut head[n..])?;if k==0{break}n+=k;}n};
    if head_len<40||&head[8..40]!=binding{return Err(StoreError::Binding)}
    let mut migrated=false;
    if &head[..8]==MAGIC_V1{
        let mut bytes=Vec::new();File::open(&path)?.read_to_end(&mut bytes)?;
        // Legacy journal: unauthenticated lengths, so only a complete, fully verified parse is trusted.
        let payloads=parse_v1_strict(&bytes).ok_or(StoreError::LegacyNeedsReview)?;
        backup(dir,"blocks.journal.v1",&bytes)?;
        let mut v2=Vec::new();v2.extend(MAGIC);v2.extend(binding);for p in &payloads{v2.extend(encode_frame(p));}
        write_new(&cpath,dir,&commit_file(binding,1,payloads.len() as u64,v2.len() as u64))?;
        write_new(&path,dir,&v2)?;migrated=true;
    }else if &head[..8]!=MAGIC{return Err(StoreError::Binding)}
    let mut cbytes=Vec::new();match File::open(&cpath){Ok(mut f)=>{f.read_to_end(&mut cbytes)?;},Err(_)=>return Err(StoreError::MissingCommit)}
    let ((seq,count,cend),slot_damaged)=read_commit(&cbytes,binding).ok_or(StoreError::CommitCorrupt)?;
    let (total,(payloads,pos,tail))={let mut f=File::open(&path)?;let total=f.metadata()?.len();(total,parse_v2(&mut f,total,40,visit)?)};
    // Bytes that cannot be a record but lie after every committed record were never committed. Not
    // when a commit slot is damaged: the surviving older slot may predate the last commit (second
    // review: a double fault would otherwise drop a committed record).
    let uncommitted=!slot_damaged&&payloads.len() as u64==count&&pos==cend&&total-pos<=(MAX_RECORD+OVERHEAD) as u64;
    let tail=if matches!(tail,Tail::Corrupt)&&uncommitted{Tail::Torn}else{tail};
    if matches!(tail,Tail::Corrupt){return Err(StoreError::Corrupt(payloads.len() as u64))}
    // Committed data must all be present and verified; extra complete records were fsynced before a crash
    // that preceded their commit-file update and are kept (they are complete and verified).
    // A crash between the journal fsync and the commit update leaves at most one such record.
    if (payloads.len() as u64)<count||pos<cend||(payloads.len() as u64)>count+1{return Err(StoreError::Corrupt(payloads.len() as u64))}
    let torn=matches!(tail,Tail::Torn);
    // A tail after a complete uncommitted record would be a second append in flight; appends commit
    // before the next begins, so that is damage, not a crash.
    if torn&&payloads.len() as u64==count+1{return Err(StoreError::Corrupt(payloads.len() as u64))}
    if torn{
        // Recovery: preserve the original bytes (fsynced copy), write the verified prefix to a new
        // file (fsynced), atomically replace the journal, fsync the directory.
        copy_prefix(&path,&backup_path(dir,"blocks.journal.torn"),total)?;
        let tmp=path.with_extension("tmp");copy_prefix(&path,&tmp,pos)?;fs::rename(&tmp,&path)?;sync_dir(dir);
    }
    // The first v2 layout kept both slots in one sector: rewrite it in the current layout.
    if cbytes.len()==2*SLOT{write_new(&cpath,dir,&commit_file(binding,seq,count,cend))?;}
    let commit=OpenOptions::new().read(true).write(true).open(&cpath)?;
    let file=OpenOptions::new().append(true).open(&path)?;
    let mut j=Journal{file,commit,path,_lock:lock,binding:*binding,seq,end:pos,records:payloads.len() as u64};
    if j.records>count{j.write_commit()?;}
    Ok(Recovered{journal:j,offsets:payloads,torn_tail:torn,migrated_v1:migrated})
}
fn sync_dir(dir:&Path){if let Ok(d)=File::open(dir){let _=d.sync_all();}}

impl Journal{
    fn write_commit(&mut self)->Result<(),StoreError>{
        self.seq+=1;let off=(self.seq%2) as u64*SLOT_STRIDE as u64;
        self.commit.seek(SeekFrom::Start(off))?;self.commit.write_all(&slot_bytes(&self.binding,self.seq,self.records,self.end))?;self.commit.sync_data()?;Ok(())
    }
    /// Appends one record, fsyncs it, then records it in the commit file (fsynced) before returning.
    /// Returns the record's frame offset (the start of its `len` field), so the caller can index it
    /// by block hash for a later `read_at` (D-REORG-REDB off-heap block store).
    pub fn append(&mut self,payload:&[u8])->Result<u64,StoreError>{
        if payload.len()>MAX_RECORD{return Err(StoreError::TooLarge)}
        let rec=encode_frame(payload);
        let off=self.end;
        self.file.write_all(&rec)?;self.file.sync_data()?;self.records+=1;self.end+=rec.len() as u64;
        self.write_commit()?;Ok(off)
    }
    /// Reads the record at `offset` (a value returned by `append`, or recomputed on replay), verifying
    /// its length check, payload digest and marker exactly as replay does, then decodes it into
    /// (headerBytes, bodyBytes, parentResultBytes). A failed check is a hard error, never an empty
    /// body: an off-heap body is thus never silently wrong (D-REORG-REDB acceptance test 6).
    pub fn read_at(&self,offset:u64)->Result<(Vec<u8>,Vec<u8>,Vec<u8>),StoreError>{decode_record(&self.payload_at(offset)?).map_err(StoreError::Io)}
    /// The payload of the record at `offset`, verified as `read_at` verifies it.
    pub fn payload_at(&self,offset:u64)->Result<Vec<u8>,StoreError>{
        #[cfg(feature="io-fault")] crate::io_fault::read(&self.path)?;
        let mut f=File::open(&self.path)?;
        f.seek(SeekFrom::Start(offset))?;
        let mut hdr=[0u8;HDR];f.read_exact(&mut hdr)?;
        let len=u32::from_be_bytes(hdr[..4].try_into().unwrap());
        if hdr[4..8]!=len_check(len)[..]{return Err(StoreError::Corrupt(offset))}
        let len=len as usize;if len>MAX_RECORD{return Err(StoreError::Corrupt(offset))}
        let mut rest=vec![0u8;32+len+1];f.read_exact(&mut rest)?;
        let (digest,payload,marker)=(&rest[..32],&rest[32..32+len],rest[32+len]);
        if marker!=1||sha(payload)[..]!=digest[..]{return Err(StoreError::Corrupt(offset))}
        rest.truncate(32+len);rest.drain(..32);Ok(rest)
    }
    /// The frame offset of the record that a fresh `append` of `payload_len` bytes would occupy, i.e.
    /// the current end; used on replay to index each payload by hash as it is read in order.
    pub fn end_offset(&self)->u64{self.end}
    pub fn path(&self)->&Path{&self.path}
}

/// payload = RLP([headerBytes, bodyBytes, parentResultBytes]); parentResult is
/// re-derived on replay and must match (parentResultMismatch halts import).
pub fn encode_record(header:&[u8],body:&[u8],result:&[u8])->Vec<u8>{
    let mut p=Vec::new();for x in [header,body,result]{crate::rlp::encode_bytes(x,&mut p)}
    let mut o=Vec::new();crate::rlp::encode_list_payload(&p,&mut o);o
}
pub fn decode_record(b:&[u8])->Result<(Vec<u8>,Vec<u8>,Vec<u8>),String>{
    let it=crate::rlp::decode(b).map_err(|e|e.code().to_string())?;let l=it.list().ok_or("record")?;if l.len()!=3{return Err("record fields".into())}
    let f=|i:usize|l[i].bytes().map(|x|x.to_vec()).ok_or_else(||"record field".to_string());Ok((f(0)?,f(1)?,f(2)?))
}
