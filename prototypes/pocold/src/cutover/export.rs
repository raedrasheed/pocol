//! V3-33 re-genesis conduct (O-33=b, D-V3-33): the published data of the old chain. `pocold export-chain` writes the
//! canonical chain of a stopped node up to height H (normally the agreed S) and the state after block H, with a
//! manifest, so that anyone can check the re-genesis allocation against their own node: two nodes that hold the same
//! canonical chain write the same bytes, and `pocold verify-export` checks an export without trusting whoever
//! published it (format: deploy/mainnet/EARN-IN-CUTOVER-RUNBOOK.md, "Old-chain data export").
//!
//! Node-local: it reads the node's journal, canonical index and state the way RPC and `pocold cutover` read them; it
//! changes no block, journal record or checkpoint. Bounded memory: the blocks are streamed one at a time to the part
//! files; what is held at once is one block and the state after block H (which the chain's state caps bound, as for
//! the cutover itself), whatever H is.
use crate::node::{Node,Want};
use crate::types::{self,Body,hex,unhex};
use pocol_lp4_execution::apply;
use revm::{InMemoryDB,primitives::{AccountInfo,Address,Bytecode,B256,U256,KECCAK_EMPTY}};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::fs::{self,File,OpenOptions};
use std::io::{BufRead,BufReader,BufWriter,Read,Write};
use std::path::Path;

/// The export's schema name (manifest.json `schema`).
pub const SCHEMA:&str="pocol-old-chain-export-v1";
/// The part files, in manifest order: every canonical block 0..=H, every account of the state after block H, and the
/// cutover facts (registry records, Escrow totals, unsettled pots) of that state.
pub const PARTS:[&str;3]=["blocks.jsonl","state.jsonl","facts.json"];
pub const MANIFEST:&str="manifest.json";
/// The longest line `verify` reads (a block's header and body in hex are below 2·BODY_MAX plus the header).
pub const LINE_MAX:u64=64<<20;
/// The largest manifest or facts file `verify` reads whole.
pub const SMALL_MAX:u64=16<<20;

/// A part file being written: its bytes go through SHA-256 as they are written.
struct Part{w:BufWriter<File>,sha:Sha256,bytes:u64,records:u64}
impl Part{
    fn create(path:&Path)->Result<Self,String>{
        let f=OpenOptions::new().write(true).create_new(true).open(path).map_err(|e|format!("create {}: {e}",path.display()))?;
        Ok(Self{w:BufWriter::new(f),sha:Sha256::new(),bytes:0,records:0})
    }
    fn write(&mut self,b:&[u8])->Result<(),String>{self.sha.update(b);self.bytes+=b.len() as u64;self.w.write_all(b).map_err(|e|e.to_string())}
    fn line(&mut self,s:&str)->Result<(),String>{self.write(s.as_bytes())?;self.write(b"\n")?;self.records+=1;Ok(())}
    fn finish(self,name:&str)->Result<Value,String>{
        let f=self.w.into_inner().map_err(|e|e.to_string())?;f.sync_all().map_err(|e|e.to_string())?;
        Ok(json!({"name":name,"bytes":self.bytes,"records":self.records,"sha256":hex(&self.sha.finalize())}))
    }
}

/// One block line: `{"height":h,"hash":"0x..","header":"0x..","body":"0x.."}` (the encoded header and body exactly as
/// the block was imported); genesis is `{"height":0,"hash":"0x.."}` (its files are the network's own).
fn block_line(h:u64,hash:&[u8;32],header:&[u8],body:&[u8])->String{
    if h==0{return format!("{{\"height\":0,\"hash\":\"{}\"}}",hex(hash))}
    format!("{{\"height\":{h},\"hash\":\"{}\",\"header\":\"{}\",\"body\":\"{}\"}}",hex(hash),hex(header),hex(body))
}
/// One account line, the accounts in ascending address order: `{"address","nonce","balance" (decimal wei),"codeHash",
/// "code" (hex, "0x" without code),"storage":[[slot,value],..]}` with only nonzero slots, in ascending slot order, each
/// as 32-byte hex.
fn account_line(a:&Address,info:&AccountInfo,code:&[u8],slots:&[(U256,U256)])->String{
    let mut s=format!("{{\"address\":\"{a:#x}\",\"nonce\":{},\"balance\":\"{}\",\"codeHash\":\"{}\",\"code\":\"{}\",\"storage\":[",info.nonce,info.balance,hex(info.code_hash.as_slice()),hex(code));
    for (i,(k,v)) in slots.iter().enumerate(){if i>0{s.push(',');}s.push_str(&format!("[\"{}\",\"{}\"]",hex(&k.to_be_bytes::<32>()),hex(&v.to_be_bytes::<32>())));}
    s.push_str("]}");s
}
/// The registry records of a state, in id order, and the cutover facts `transform` reports for it with no new genesis
/// members (EARN-IN-SPEC.md section 7, C2–C4): the copied accounts, the HELD bonds the new genesis returns, the dropped
/// pots of the last D blocks and the Escrow totals (and, when the state runs the v3 programs, the carry into pot H + 1,
/// D-V3-15). Recomputed by `verify` from state.jsonl.
pub fn facts(state:&InMemoryDB,height:u64,d:u64)->Result<Value,String>{
    let mut ids=apply::record_list(state);ids.sort();
    let records:Vec<Value>=ids.iter().filter_map(|id|apply::miner_record(state,*id)).map(|r|json!({"id":format!("{:#x}",r.id),"status":r.status,"tomb":r.tomb,
        "bondState":r.bond_state,"bond":r.bond.to_string(),"reward":format!("{:#x}",r.reward),"lastShare":r.last_share,"registeredAt":r.registered_at,
        "exitHeight":r.exit_height,"dueHeight":r.due_height,"burnHeight":r.burn_height,"graceStart":r.grace_start})).collect();
    let (_,report)=super::transform(state,height,d,&[],U256::ZERO,super::runs_v3_programs(state))?;
    Ok(json!({"height":height,"D":d,"registry":records,"cutover":report}))
}

/// `pocold export-chain --network OLD_DIR --data OLD_DATA --to-height H --out NEW_DIR [NODE] [LOCAL]` (the node stopped:
/// its home lock is taken). Returns the manifest.
pub fn run(net:&Path,data:&Path,height:u64,out:&Path,limits:crate::node::Limits,local:&pocol_lp4_execution::admission::NodeLocal)->Result<Value,String>{
    if fs::symlink_metadata(out).is_ok(){return Err("--out must be a new directory; an export is never overwritten".into())}
    let spec=crate::load_spec_for(net,local,limits.state_store.then_some(data))?;
    let cfg=crate::node::Config{dir:data.to_path_buf(),name:"export".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits};
    let mut node=Node::open(cfg,spec)?;
    export(&mut node,height,out)
}

/// The export of `node`'s canonical chain up to `height` into the new directory `out`. The parts are written into
/// `<out>.partial` and renamed to `out` once complete and synced, so a directory named `out` is always a whole export.
pub fn export(node:&mut Node,height:u64,out:&Path)->Result<Value,String>{
    if fs::symlink_metadata(out).is_ok(){return Err("--out must be a new directory; an export is never overwritten".into())}
    if height==0{return Err("--to-height must be at least 1 (genesis is the network's own files)".into())}
    let head=node.chain.head().height;
    if height>head{return Err(format!("height {height} is above the head {head}"))}
    let base=node.chain.canonical.base();
    if height<base{return Err(format!("height {height} is below the in-memory window (it starts at height {base}); the state after it is not available here: export within the window, as the cutover cuts"))}
    let hash=node.chain.canonical_at(height).ok_or("no canonical block at that height")?.hash;
    let rounds=super::rebuild_at(node,&hash,height)?;
    let partial=out.with_file_name(format!("{}.partial",out.file_name().ok_or("--out names no directory")?.to_string_lossy()));
    if fs::symlink_metadata(&partial).is_ok(){return Err(format!("{} exists (an interrupted export?); remove it first",partial.display()))}
    fs::create_dir(&partial).map_err(|e|format!("create {}: {e}",partial.display()))?;
    let r=write_parts(node,height,&hash,&partial);
    let manifest=match r{Ok(m)=>m,Err(e)=>{let _=fs::remove_dir_all(&partial);return Err(e)}};
    fs::rename(&partial,out).map_err(|e|format!("rename {} to {}: {e}",partial.display(),out.display()))?;
    if let Some(parent)=out.parent().filter(|p|!p.as_os_str().is_empty()){
        #[cfg(unix)]
        File::open(parent).and_then(|f|f.sync_all()).map_err(|e|e.to_string())?;
        #[cfg(not(unix))]
        let _=parent;
    }
    let mut shown=manifest.clone();shown["rebuildRounds"]=json!(rounds);
    Ok(shown)
}

/// blocks.jsonl: the canonical blocks 0..=`height` of `node`, one view at a time (resident, or read back from the
/// journal below the window), streamed to the new file `path`; block `height` must be `hash`. Returns the part's
/// manifest entry and block `height`'s stateRoot. What it holds at once is one block and the file's buffer, whatever
/// `height` is (tests/cutover.rs measures it).
pub fn write_blocks(node:&Node,height:u64,hash:&[u8;32],path:&Path)->Result<(Value,[u8;32]),String>{
    let mut blocks=Part::create(path)?;let mut state_root=[0u8;32];let mut prev=[0u8;32];
    let base=node.chain.canonical.base();
    for h in 0..=height{
        if h==0{let g=node.canon_hash(0)?.ok_or("no genesis")?;blocks.line(&block_line(0,&g,&[],&[]))?;prev=g;continue}
        // The block's bytes as imported: in the window from its entry or its journal record (`block_msg`, no receipts
        // read), below it from the canonical index and the journal (`canonical_view` without receipts).
        let (hash,hb,bb)=if h>=base{
            let e=node.chain.canonical_at(h).ok_or_else(||format!("no canonical block at height {h}"))?;
            match node.block_msg(&e.hash){Some(crate::net::Msg::Block{header,body})=>(e.hash,header,body),_=>return Err(format!("the block at height {h} cannot be read"))}
        }else{
            let e=node.canonical_view(h,Want::default())?.ok_or_else(||format!("no canonical block at height {h}"))?;
            (e.hash,e.header.encoded.clone(),e.body.encode())
        };
        let header=types::decode_header(&hb).map_err(|e|format!("the block at height {h}: {e}"))?;
        if header.h!=h||header.parent_hash!=prev||types::block_hash(&header)!=hash{return Err(format!("the canonical block at height {h} does not follow the one below it"))}
        blocks.line(&block_line(h,&hash,&hb,&bb))?;
        if h==height{state_root=header.state_root;}
        prev=hash;
    }
    if prev!=*hash{return Err("the canonical chain changed during the export".into())}
    Ok((blocks.finish(PARTS[0])?,state_root))
}

fn write_parts(node:&Node,height:u64,hash:&[u8;32],dir:&Path)->Result<Value,String>{
    let spec=&node.chain.spec;let (chain_id,genesis,d)=(spec.chain_id(),spec.genesis_hash(),u64::from(spec.cp().D));
    let (blocks,state_root)=write_blocks(node,height,hash,&dir.join(PARTS[0]))?;
    // The state after block H, whole (as the cutover reads it), with every account's code.
    let view=node.chain.view_of(hash).ok_or("the state at that height is not available")?;
    let mut old=crate::chain::whole_state(view.v());
    let hashes:Vec<_>=old.accounts.values().filter_map(|a|a.info()).filter(|i|i.code.is_none()&&i.code_hash!=KECCAK_EMPTY).map(|i|i.code_hash).collect();
    for h in hashes{if let Some(c)=view.v().contract(&h){old.contracts.insert(h,c);}}
    drop(view);
    let root=apply::state_root(&old).map_err(|e|format!("state root: {e:?}"))?;
    if root.0!=state_root{return Err(format!("the state read for height {height} does not have the block's stateRoot"))}
    let mut st=Part::create(&dir.join(PARTS[1]))?;
    let mut addrs:Vec<_>=old.accounts.keys().copied().collect();addrs.sort();
    for a in &addrs{
        let acct=&old.accounts[a];let Some(info)=acct.info() else{continue};
        let code=match &info.code{Some(c)=>c.original_bytes(),None if info.code_hash==KECCAK_EMPTY=>Default::default(),
            None=>old.contracts.get(&info.code_hash).map(|c|c.original_bytes()).ok_or_else(||format!("code of {a:#x} is not available"))?};
        let mut slots:Vec<_>=acct.storage.iter().filter(|(_,v)|!v.is_zero()).map(|(k,v)|(*k,*v)).collect();slots.sort();
        st.line(&account_line(a,&info,&code,&slots))?;
    }
    let st=st.finish(PARTS[1])?;
    let mut fx=Part::create(&dir.join(PARTS[2]))?;
    let f=facts(&old,height,d)?;fx.write(serde_json::to_string_pretty(&f).map_err(|e|e.to_string())?.as_bytes())?;fx.write(b"\n")?;fx.records=1;
    let fx=fx.finish(PARTS[2])?;
    let manifest=json!({"schema":SCHEMA,"chainId":chain_id,"genesisHash":hex(&genesis),"height":height,"headHash":hex(hash),"stateRoot":hex(&state_root),"D":d,
        "parts":[blocks,st,fx],
        "verify":"pocold verify-export --dir EXPORT_DIR [--network OLD_GENESIS_DIR]"});
    let mut m=Part::create(&dir.join(MANIFEST))?;m.write(serde_json::to_string_pretty(&manifest).map_err(|e|e.to_string())?.as_bytes())?;m.write(b"\n")?;m.finish(MANIFEST)?;
    Ok(manifest)
}

fn read_small(path:&Path)->Result<Vec<u8>,String>{
    let f=File::open(path).map_err(|e|format!("open {}: {e}",path.display()))?;
    let mut b=Vec::new();f.take(SMALL_MAX+1).read_to_end(&mut b).map_err(|e|e.to_string())?;
    if b.len() as u64>SMALL_MAX{return Err(format!("{} exceeds {} bytes",path.display(),SMALL_MAX))}
    Ok(b)
}
fn sha256_file(path:&Path)->Result<(u64,String),String>{
    let mut f=File::open(path).map_err(|e|format!("open {}: {e}",path.display()))?;let mut sha=Sha256::new();let mut buf=vec![0u8;1<<16];let mut n=0u64;
    loop{let k=f.read(&mut buf).map_err(|e|e.to_string())?;if k==0{break}sha.update(&buf[..k]);n+=k as u64;}
    Ok((n,hex(&sha.finalize())))
}
/// The lines of a part, each at most LINE_MAX bytes, without its newline. A part must end with a newline.
fn lines(path:&Path,mut f:impl FnMut(u64,&[u8])->Result<(),String>)->Result<u64,String>{
    let mut r=BufReader::new(File::open(path).map_err(|e|format!("open {}: {e}",path.display()))?);let mut buf=Vec::new();let mut n=0u64;
    loop{
        buf.clear();let k=(&mut r).take(LINE_MAX+1).read_until(b'\n',&mut buf).map_err(|e|e.to_string())?;
        if k==0{return Ok(n)}
        if buf.last()!=Some(&b'\n'){return Err(format!("{} line {}: longer than {LINE_MAX} bytes or not ended by a newline",path.display(),n+1))}
        buf.pop();f(n,&buf)?;n+=1;
    }
}
fn field<'a>(v:&'a Value,k:&str,at:&str)->Result<&'a Value,String>{v.get(k).ok_or_else(||format!("{at}: no {k}"))}
fn hex32(v:&Value,k:&str,at:&str)->Result<[u8;32],String>{
    let b=unhex(field(v,k,at)?.as_str().ok_or_else(||format!("{at}: {k} is not text"))?)?;
    b.try_into().map_err(|_|format!("{at}: {k} is not 32 bytes"))
}

/// `pocold verify-export --dir EXPORT [--network OLD_GENESIS_DIR]`: checks an export without trusting its publisher.
/// - manifest: the schema, the three parts, each part's byte count and SHA-256;
/// - blocks.jsonl: heights 0..=H in order, genesis the manifest's; each header decodes, hashes to its line's hash,
///   meets its own PoW target, names the genesis, its height and the block below it as parent, and commits the body
///   on its line (tx_root and evidence_root); block H is the manifest's headHash with its stateRoot;
/// - state.jsonl: accounts in strictly ascending address order whose world-state root is that stateRoot;
/// - facts.json: equal to the registry records and cutover facts recomputed from that state.
///
/// With `genesis` (the chain id and genesis hash of the caller's own network files) the manifest must name them.
/// A block's execution is not repeated here: a node that syncs the old chain does that, and its own export of the
/// same height must then be byte-equal. Returns a summary.
pub fn verify(dir:&Path,genesis:Option<(u64,[u8;32])>)->Result<Value,String>{
    let m:Value=serde_json::from_slice(&read_small(&dir.join(MANIFEST))?).map_err(|_|"manifest.json is not JSON")?;
    if m["schema"]!=SCHEMA{return Err(format!("manifest.json: schema is not {SCHEMA}"))}
    let height=field(&m,"height","manifest")?.as_u64().ok_or("manifest: height")?;let d=field(&m,"D","manifest")?.as_u64().ok_or("manifest: D")?;
    let chain_id=field(&m,"chainId","manifest")?.as_u64().ok_or("manifest: chainId")?;
    let (g,head,root)=(hex32(&m,"genesisHash","manifest")?,hex32(&m,"headHash","manifest")?,hex32(&m,"stateRoot","manifest")?);
    if let Some((c,h))=genesis{if c!=chain_id||h!=g{return Err("the export is of another network than the given network files".into())}}
    let parts=field(&m,"parts","manifest")?.as_array().ok_or("manifest: parts")?;
    if parts.len()!=PARTS.len()||parts.iter().zip(PARTS).any(|(p,n)|p["name"]!=n){return Err(format!("manifest: the parts must be {PARTS:?}"))}
    for p in parts{
        let name=p["name"].as_str().unwrap_or_default();let (n,sha)=sha256_file(&dir.join(name))?;
        if p["bytes"].as_u64()!=Some(n)||p["sha256"].as_str()!=Some(sha.as_str()){return Err(format!("{name}: its bytes do not match the manifest"))}
    }
    // Blocks.
    let mut prev=[0u8;32];let mut last=None;let mut txs=0u64;
    let blocks=lines(&dir.join(PARTS[0]),|i,line|{
        let at=format!("blocks.jsonl line {}",i+1);let v:Value=serde_json::from_slice(line).map_err(|_|format!("{at}: not JSON"))?;
        if field(&v,"height",&at)?.as_u64()!=Some(i){return Err(format!("{at}: height is not {i}"))}
        let hash=hex32(&v,"hash",&at)?;
        if i==0{if hash!=g{return Err(format!("{at}: genesis is not the manifest's"))}prev=hash;return Ok(())}
        let hb=unhex(field(&v,"header",&at)?.as_str().ok_or_else(||format!("{at}: header"))?)?;let bb=unhex(field(&v,"body",&at)?.as_str().ok_or_else(||format!("{at}: body"))?)?;
        let h=types::decode_header(&hb).map_err(|e|format!("{at}: header: {e}"))?;
        if types::block_hash(&h)!=hash{return Err(format!("{at}: the header does not hash to the line's hash"))}
        if !types::le_target(&types::pow_hash(&types::template_id(&h.ut_raw),h.nonce),&h.target){return Err(format!("{at}: the header does not meet its target"))}
        if h.h!=i||h.parent_hash!=prev||h.genesis_hash!=g||h.chain_id!=chain_id{return Err(format!("{at}: the header is not the child of the line before it on this network"))}
        let body=Body::decode(&bb,bb.len()).map_err(|e|format!("{at}: body: {e}"))?;
        if body.encode()!=bb||types::body_commitment(&body.txs)!=Some(h.tx_root)||types::body_commitment(&body.evidence)!=Some(h.evidence_root){return Err(format!("{at}: the body is not the one the header commits"))}
        txs+=body.txs.len() as u64;prev=hash;last=Some((i,hash,h.state_root));Ok(())
    })?;
    match last{Some((h,x,r)) if h==height&&x==head&&r==root=>{},_=>return Err("blocks.jsonl does not end at the manifest's height, headHash and stateRoot".into())}
    if blocks!=height+1{return Err("blocks.jsonl: not one line per height".into())}
    // State.
    let mut db=InMemoryDB::default();let mut before:Option<Address>=None;
    let accounts=lines(&dir.join(PARTS[1]),|i,line|{
        let at=format!("state.jsonl line {}",i+1);let v:Value=serde_json::from_slice(line).map_err(|_|format!("{at}: not JSON"))?;
        let a:Address=field(&v,"address",&at)?.as_str().and_then(|s|s.parse().ok()).ok_or_else(||format!("{at}: address"))?;
        if before.is_some_and(|b|b>=a){return Err(format!("{at}: addresses are not in strictly ascending order"))}before=Some(a);
        let nonce=field(&v,"nonce",&at)?.as_u64().ok_or_else(||format!("{at}: nonce"))?;
        let balance:U256=field(&v,"balance",&at)?.as_str().filter(|s|!s.is_empty()&&s.bytes().all(|b|b.is_ascii_digit())).and_then(|s|s.parse().ok()).ok_or_else(||format!("{at}: balance"))?;
        let code_hash=B256::from(hex32(&v,"codeHash",&at)?);
        let code=unhex(field(&v,"code",&at)?.as_str().ok_or_else(||format!("{at}: code"))?)?;
        let code=if code.is_empty(){if code_hash!=KECCAK_EMPTY{return Err(format!("{at}: code is missing"))}None}else{
            let c=Bytecode::new_raw(code.into());if c.hash_slow()!=code_hash{return Err(format!("{at}: code does not hash to codeHash"))}Some(c)};
        db.insert_account_info(a,AccountInfo{balance,nonce,code_hash,code});
        let mut last_slot:Option<U256>=None;
        for s in field(&v,"storage",&at)?.as_array().ok_or_else(||format!("{at}: storage"))?{
            let pair=s.as_array().filter(|p|p.len()==2).ok_or_else(||format!("{at}: storage entry"))?;
            let w=|x:&Value|->Result<U256,String>{let b:[u8;32]=unhex(x.as_str().unwrap_or("-"))?.try_into().map_err(|_|format!("{at}: storage word"))?;Ok(U256::from_be_bytes(b))};
            let (k,val)=(w(&pair[0])?,w(&pair[1])?);
            if val.is_zero()||last_slot.is_some_and(|l|l>=k){return Err(format!("{at}: storage must hold nonzero slots in ascending order"))}last_slot=Some(k);
            db.insert_account_storage(a,k,val).map_err(|_|format!("{at}: storage"))?;
        }
        Ok(())
    })?;
    let got=apply::state_root(&db).map_err(|e|format!("state root: {e:?}"))?;
    if got.0!=root{return Err(format!("state.jsonl: its world-state root {} is not the stateRoot {}",hex(&got.0),hex(&root)))}
    // Facts.
    let f:Value=serde_json::from_slice(&read_small(&dir.join(PARTS[2]))?).map_err(|_|"facts.json is not JSON")?;
    if f!=facts(&db,height,d)?{return Err("facts.json is not what the state gives".into())}
    Ok(json!({"verified":true,"chainId":chain_id,"genesisHash":hex(&g),"height":height,"headHash":hex(&head),"stateRoot":hex(&root),"blocks":blocks,"transactions":txs,"accounts":accounts,
        "allocTotal":f["cutover"]["allocTotal"],"bondsCreditedTotal":f["cutover"]["bondsCreditedTotal"]}))
}
