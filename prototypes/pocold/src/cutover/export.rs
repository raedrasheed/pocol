//! V3-33 re-genesis conduct (O-33=b, D-V3-33): the published data of the old chain. `pocold export-chain` writes the
//! canonical chain of a stopped node up to height H (normally the agreed S) and the state after block H, with a
//! manifest, so that anyone can check the re-genesis allocation against their own node: two nodes that hold the same
//! canonical chain write the same bytes, and `pocold verify-export` checks that an export is internally consistent
//! and of the given network. It does not by itself prove that block H is the old chain's real block H: that needs
//! block H's hash from a source other than the publisher (`--head-hash`, see `verify`; format:
//! deploy/mainnet/EARN-IN-CUTOVER-RUNBOOK.md, "Old-chain data export").
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
        Ok(part_json(name,self.bytes,self.records,&hex(&self.sha.finalize())))
    }
}
/// A part's manifest entry.
fn part_json(name:&str,bytes:u64,records:u64,sha256:&str)->Value{json!({"name":name,"bytes":bytes,"records":records,"sha256":sha256})}

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

/// The index of D (the reward delay) in the consensus parameters (profile.rs, schema order).
pub const CP_D:usize=11;
/// Consensus parameter `i` as `state` holds it: slot `i` of ChainCaps (0xc0c004), which genesis writes with every
/// parameter and nothing writes after it (the system programs read their parameters there). Bound to the state's
/// world-state root, so to the stateRoot of block H, unlike any value the manifest or facts.json states.
pub fn state_cp(state:&InMemoryDB,i:usize)->Result<U256,String>{
    let caps=state.accounts.get(&apply::caps()).filter(|a|a.info().is_some()).ok_or("the state has no ChainCaps account (0xc0c004)")?;
    Ok(caps.storage.get(&U256::from(i)).copied().unwrap_or_default())
}
/// D as the state gives it (consensus parameter 11 in ChainCaps): the D of the facts, never the manifest's.
pub fn state_d(state:&InMemoryDB)->Result<u64,String>{
    let v=state_cp(state,CP_D)?;
    if v>U256::from(u16::MAX){return Err(format!("the state's D (consensus parameter {CP_D}) is not a 16-bit value"))}
    Ok(v.to::<u64>())
}

/// The caller's own network files, which `verify` checks an export against: the chain id, the genesis hash and every
/// consensus parameter of the genesis spec (which the genesis hash commits), and the profile (consensus parameters and
/// fork schedule) that the headers' H-pre checks read (`h_pre`).
pub struct Network{pub chain_id:u64,pub genesis:[u8;32],pub cp:Vec<U256>,profile:pocol_lp4_execution::profile::ProfileIdentity}
impl Network{
    pub fn of(spec:&crate::chain::Spec)->Self{
        Self{chain_id:spec.chain_id(),genesis:spec.genesis_hash(),cp:spec.profile().spec().cp.iter().map(|v|U256::from_be_bytes(v.to_be_bytes())).collect(),
            profile:spec.profile().clone()}
    }
}

/// The header fields of block i-1 that H-pre reads for block i (the inputs of `Chain::child_pre`); genesis is its
/// timestamp g_ts at height 0 (`Chain::new`), whose base fee and gas no child reads (block 1 starts at baseFee0).
struct Parent{height:u64,ts:u64,base_fee:U256,gas_used:u64,gas_limit:u64}
impl Parent{
    fn genesis(cp:&pocol_lp4_execution::profile::ConsensusParams)->Self{Self{height:0,ts:cp.g_ts,base_fee:U256::ZERO,gas_used:0,gas_limit:0}}
    fn of(x:&crate::header::Header)->Self{Self{height:x.h,ts:x.ts,base_fee:types::from_fixed(&x.base_fee),gas_used:x.gas_used,gas_limit:x.gas_limit}}
}
/// H-pre items 1–9 and 11 of block `x` on its parent `p`, and the body bounds (1) and the fallback body (12), as
/// `Chain::validate` checks them before it needs the parent's state: from the headers and the genesis profile alone.
/// The window, ASERT target, nMax, T_share and base fee are computed as `Chain::child_pre` computes them (the profile
/// accessors are `Spec`'s). So a header that names any target, a nonce outside nMax, a timestamp outside its window,
/// another protocol version, an unrecoverable signature or another base fee is refused, and forging block H costs a
/// real proof of work at the ASERT target. Not checked (they need the parent's state, which only re-execution gives):
/// item 10 (the parent is not final), 12 (a signed template's proposer is Draw(h,0); a fallback's UT is Build_fb) and
/// 13–17 (the winner owns the nonce, evidence, caps, execution and the stateRoot).
fn h_pre(profile:&pocol_lp4_execution::profile::ProfileIdentity,p:&Parent,x:&crate::header::Header,body:&Body)->Result<(),String>{
    use crate::{asert,assignment::{self,NonceMode},hashes,timing::AttemptWindow};
    let cp=profile.cp();let h=x.h;let version=|h:u64|profile.forks().active_version(h);
    if x.encoded.len()>crate::header::HDR_MAX{return Err("1 header size".into())}
    if x.protocol_version!=version(h){return Err("netVersion: not the protocol version of this height".into())}
    if cp.H_END!=0&&h>cp.H_END{return Err("10a H_END".into())}
    let tid=types::template_id(&x.ut_raw);let signed=!x.proposer.is_empty();
    if signed{
        if x.proposer.len()!=20{return Err("1 proposer width".into())}
        if x.a!=0{return Err("3 signed requires a=0".into())}
        match hashes::recover_address(&hashes::sig_msg(&tid),&x.sig){Ok(a) if x.sig.len()==65&&a[..]==x.proposer[..]=>{},_=>return Err("3 template signature".into())}
    }else if !x.sig.is_empty(){return Err("3 signature without proposer".into())}
    let window=AttemptWindow::new(p.ts,x.a,cp.D_att).map_err(|e|format!("window {e:?}"))?;
    if !window.contains(x.ts){return Err("4 window".into())}
    if !signed&&x.ts!=window.fallback_timestamp(cp.dFbWait).map_err(|e|format!("4 fb {e:?}"))?{return Err("4 fallback timestamp".into())}
    let target_g=types::Fixed::from_be_slice(&cp.target_g.to_be_bytes()).ok_or("target_g")?;
    let target=asert::asert(&target_g,cp.g_ts as i128,cp.T_blk as i128,cp.tau as i128,p.ts as i128,p.height as i128).map(|o|o.target).ok_or("asert")?;
    if x.target!=target{return Err("5 target: not the ASERT target".into())}
    let mode=if crate::chain::full_nonce_rule(version(h),cp.nonceMode){NonceMode::Unbounded}else{NonceMode::Bounded};
    let nonce_max=assignment::nonce_max(&target,mode,cp.c).map_err(|e|format!("assignment {e:?}"))?;
    if u128::from(x.nonce)>=nonce_max{return Err("6 nonce>=nMax".into())}
    if !types::le_target(&types::pow_hash(&tid,x.nonce),&x.target){return Err("7 pow".into())}
    hashes::recover_address(&hashes::win_msg(&tid,x.nonce,&types::share_root(x)),&x.winner_sig).map_err(|_|"8 winnerSig")?;
    let t_share=types::to_fixed(types::from_fixed(&target).checked_mul(U256::from(cp.m)).unwrap_or(U256::MAX));
    if x.shares.len()>usize::from(cp.S_max){return Err("1 shareList size".into())}
    if x.shares.windows(2).any(|w|w[0]>=w[1]){return Err("9 shares not strictly ascending".into())}
    if x.shares.iter().any(|n|*n==x.nonce||u128::from(*n)>=nonce_max||!types::le_target(&types::pow_hash(&tid,*n),&t_share)){return Err("9 share".into())}
    if x.gas_limit!=cp.GAS_LIMIT{return Err("11 gasLimit".into())}
    let floor=if profile.v3_active(h){crate::chain::from_fixed_lp4(&cp.baseFeeMin)}else{U256::ZERO};
    let base_fee=if h==1{crate::chain::from_fixed_lp4(&cp.baseFee0).max(floor)}else{
        pocol_lp4_execution::fees::next_base_fee(p.base_fee,p.gas_used,p.gas_limit,floor).map_err(|e|format!("baseFee {e:?}"))?};
    if types::from_fixed(&x.base_fee)!=base_fee{return Err("11 baseFee".into())}
    if body.txs.iter().any(|t|t.len()>cp.TX_MAX as usize){return Err("1 TX_MAX".into())}
    if body.encode().len()>cp.BODY_MAX as usize{return Err("1 BODY_MAX".into())}
    if body.evidence.len()>usize::from(cp.E_max)||body.evidence.iter().any(|e|e.len()>cp.EVID_BYTES_MAX as usize){return Err("1 evidence bounds".into())}
    if !signed&&body.encode()!=types::EMPTY_BODY{return Err("12 fallback body".into())}
    Ok(())
}

/// A manifest or facts.json as the export writes it: pretty JSON and a newline. `verify` compares the bytes it reads
/// with this rendering of what it recomputed, so no field, key order or spacing is left to the publisher.
fn pretty(v:&Value)->Result<Vec<u8>,String>{let mut b=serde_json::to_vec_pretty(v).map_err(|e|e.to_string())?;b.push(b'\n');Ok(b)}
/// The manifest of an export with these values and part entries.
fn manifest_json(chain_id:u64,genesis:&[u8;32],height:u64,head:&[u8;32],state_root:&[u8;32],d:u64,parts:[Value;3])->Value{
    json!({"schema":SCHEMA,"chainId":chain_id,"genesisHash":hex(genesis),"height":height,"headHash":hex(head),"stateRoot":hex(state_root),"D":d,
        "parts":parts,"verify":"pocold verify-export --dir EXPORT_DIR [--network OLD_GENESIS_DIR]"})
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
    // D as `verify` derives it: from the state's ChainCaps, which must hold the network's own D.
    let sd=state_d(&old)?;
    if sd!=d{return Err(format!("the state at height {height} holds D = {sd} (consensus parameter {CP_D}), not the network's D = {d}"))}
    let mut fx=Part::create(&dir.join(PARTS[2]))?;
    fx.write(&pretty(&facts(&old,height,d)?)?)?;fx.records=1;
    let fx=fx.finish(PARTS[2])?;
    let mut manifest=manifest_json(chain_id,&genesis,height,hash,&state_root,d,[blocks,st,fx]);
    let mut m=Part::create(&dir.join(MANIFEST))?;m.write(&pretty(&manifest)?)?;
    // The manifest's own SHA-256, shown (not written): what `verify-export --manifest-sha256` compares.
    let digest=m.finish(MANIFEST)?["sha256"].clone();manifest["manifestSha256"]=digest;
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
/// The lines of a part, each at most LINE_MAX bytes, without its newline. A part must end with a newline. Returns the
/// line count and the byte count and SHA-256 of the very bytes the lines were read from (hashed in the same pass, so a
/// file that changes after its digest was checked is not verified with other bytes: `verify` compares the two).
fn lines(path:&Path,mut f:impl FnMut(u64,&[u8])->Result<(),String>)->Result<(u64,(u64,String)),String>{
    let mut r=BufReader::new(File::open(path).map_err(|e|format!("open {}: {e}",path.display()))?);let mut buf=Vec::new();let mut n=0u64;
    let mut sha=Sha256::new();let mut bytes=0u64;
    loop{
        buf.clear();let k=(&mut r).take(LINE_MAX+1).read_until(b'\n',&mut buf).map_err(|e|e.to_string())?;
        if k==0{return Ok((n,(bytes,hex(&sha.finalize()))))}
        sha.update(&buf);bytes+=k as u64;
        if buf.last()!=Some(&b'\n'){return Err(format!("{} line {}: longer than {LINE_MAX} bytes or not ended by a newline",path.display(),n+1))}
        buf.pop();f(n,&buf)?;n+=1;
    }
}
/// A 32-byte value given on the command line (a digest or a block hash): 64 hex digits, with or without `0x`, as
/// `sha256sum` or an RPC prints it. Anything else is refused, so a malformed pin never goes unchecked.
pub fn pin(v:&str,what:&str)->Result<[u8;32],String>{
    let t=v.trim();let t=t.strip_prefix("0x").or_else(||t.strip_prefix("0X")).unwrap_or(t);
    if t.len()!=64||!t.bytes().all(|b|b.is_ascii_hexdigit()){return Err(format!("{what} must be 64 hex digits (32 bytes), not {v:?}"))}
    let b=unhex(&format!("0x{}",t.to_ascii_lowercase()))?;b.try_into().map_err(|_|format!("{what}: not 32 bytes"))
}
fn field<'a>(v:&'a Value,k:&str,at:&str)->Result<&'a Value,String>{v.get(k).ok_or_else(||format!("{at}: no {k}"))}
fn hex32(v:&Value,k:&str,at:&str)->Result<[u8;32],String>{
    let b=unhex(field(v,k,at)?.as_str().ok_or_else(||format!("{at}: {k} is not text"))?)?;
    b.try_into().map_err(|_|format!("{at}: {k} is not 32 bytes"))
}

/// `pocold verify-export --dir EXPORT [--network OLD_GENESIS_DIR] [--manifest-sha256 HEX] [--head-hash HEX]`: checks
/// that an export is internally consistent and, with `network`, of that network. What it recomputes and reports is
/// bound to the blocks' headers and to the stateRoot of block H; no value the manifest or facts.json states is used on
/// trust:
/// - manifest: the schema, the three parts, each part's byte count and SHA-256 (first, so a damaged copy fails at
///   once; each part is hashed again in the pass that reads it, and the two must agree);
/// - blocks.jsonl: heights 0..=H in order, genesis the manifest's; each header decodes, hashes to its line's hash,
///   meets its PoW target, names the genesis, the chain id, its height and the block below it as parent, and
///   commits the body on its line (tx_root and evidence_root); block H is the manifest's headHash with its stateRoot;
///   every line is the bytes `export-chain` writes for that block. With `network`, every header also passes H-pre
///   on the network's genesis profile (`h_pre`): the target is the ASERT target, so a header cannot name its own;
/// - state.jsonl: accounts in strictly ascending address order, each line the bytes `export-chain` writes for it,
///   whose world-state root is that stateRoot;
/// - D: consensus parameter 11 in the state's ChainCaps (`state_d`), under that stateRoot. The manifest's D must be
///   it: the dropped pots of facts.json are recomputed with it, never with the manifest's value;
/// - facts.json: the bytes of the registry records and cutover facts recomputed from that state and D;
/// - manifest, last: its bytes are the manifest those checked values give (chain id, genesis, height, headHash,
///   stateRoot, D, and each part's bytes, records and SHA-256), so no field of it is unchecked.
///
/// What this does not prove by itself: that block H is the old chain's real block H. Every value above is bound to
/// block H's header, and blocks are not re-executed, so a publisher who mines one block at the ASERT target can put
/// any state under a header of its own. `head_hash` closes that: the hash of block H obtained independently of the
/// publisher (the verifier's own synced node, `eth_getBlockByNumber` at H), which the manifest's headHash must be.
/// Only then is the summary's `verified` true; otherwise it is "selfConsistent". With `network` (the caller's own
/// network files) the manifest must name its chain id and genesis hash, and the state's ChainCaps must hold every
/// consensus parameter of its genesis spec. With `manifest_sha256` (the digest published with the export)
/// manifest.json must have it; the manifest commits every part, so that pins the whole export to what was published,
/// not to the chain. Without `network` the headers are checked against their own targets only and the genesis hash
/// is the publisher's word. A node that syncs the old chain re-executes every block, and its own export of the same
/// height must then be byte-equal. Returns a summary that says which of these were checked.
pub fn verify(dir:&Path,network:Option<&Network>,manifest_sha256:Option<&str>,head_hash:Option<&str>)->Result<Value,String>{
    let want_sha=manifest_sha256.map(|v|pin(v,"--manifest-sha256")).transpose()?;let want_head=head_hash.map(|v|pin(v,"--head-hash")).transpose()?;
    let mbytes=read_small(&dir.join(MANIFEST))?;
    let msha=hex(&Sha256::digest(&mbytes));
    if let Some(want)=want_sha{if hex(&want)!=msha{return Err(format!("manifest.json: its SHA-256 is {msha}, not the given {}",hex(&want)))}}
    let m:Value=serde_json::from_slice(&mbytes).map_err(|_|"manifest.json is not JSON")?;
    if m["schema"]!=SCHEMA{return Err(format!("manifest.json: schema is not {SCHEMA}"))}
    let height=field(&m,"height","manifest")?.as_u64().ok_or("manifest: height")?;let stated_d=field(&m,"D","manifest")?.as_u64().ok_or("manifest: D")?;
    let chain_id=field(&m,"chainId","manifest")?.as_u64().ok_or("manifest: chainId")?;
    let (g,head,root)=(hex32(&m,"genesisHash","manifest")?,hex32(&m,"headHash","manifest")?,hex32(&m,"stateRoot","manifest")?);
    if let Some(n)=network{if n.chain_id!=chain_id||n.genesis!=g{return Err("the export is of another network than the given network files".into())}}
    if let Some(want)=want_head{if want!=head{return Err(format!("manifest: headHash is {}, not the given --head-hash {}",hex(&head),hex(&want)))}}
    let parts=field(&m,"parts","manifest")?.as_array().ok_or("manifest: parts")?;
    if parts.len()!=PARTS.len()||parts.iter().zip(PARTS).any(|(p,n)|p["name"]!=n){return Err(format!("manifest: the parts must be {PARTS:?}"))}
    let mut sizes=Vec::new();
    for p in parts{
        let name=p["name"].as_str().unwrap_or_default();let (n,sha)=sha256_file(&dir.join(name))?;
        if p["bytes"].as_u64()!=Some(n)||p["sha256"].as_str()!=Some(sha.as_str()){return Err(format!("{name}: its bytes do not match the manifest"))}
        sizes.push((n,sha));
    }
    // Blocks.
    let mut prev=[0u8;32];let mut last=None;let mut txs=0u64;let mut parent=network.map(|n|Parent::genesis(n.profile.cp()));
    let (blocks,read)=lines(&dir.join(PARTS[0]),|i,line|{
        let at=format!("blocks.jsonl line {}",i+1);let v:Value=serde_json::from_slice(line).map_err(|_|format!("{at}: not JSON"))?;
        if field(&v,"height",&at)?.as_u64()!=Some(i){return Err(format!("{at}: height is not {i}"))}
        let hash=hex32(&v,"hash",&at)?;
        if i==0{
            if hash!=g{return Err(format!("{at}: genesis is not the manifest's"))}
            if line!=block_line(0,&hash,&[],&[]).as_bytes(){return Err(format!("{at}: not the line the export writes for genesis"))}
            prev=hash;return Ok(())
        }
        let hb=unhex(field(&v,"header",&at)?.as_str().ok_or_else(||format!("{at}: header"))?)?;let bb=unhex(field(&v,"body",&at)?.as_str().ok_or_else(||format!("{at}: body"))?)?;
        let h=types::decode_header(&hb).map_err(|e|format!("{at}: header: {e}"))?;
        if types::block_hash(&h)!=hash{return Err(format!("{at}: the header does not hash to the line's hash"))}
        if !types::le_target(&types::pow_hash(&types::template_id(&h.ut_raw),h.nonce),&h.target){return Err(format!("{at}: the header does not meet its target"))}
        if h.h!=i||h.parent_hash!=prev||h.genesis_hash!=g||h.chain_id!=chain_id{return Err(format!("{at}: the header is not the child of the line before it on this network"))}
        let body=Body::decode(&bb,bb.len()).map_err(|e|format!("{at}: body: {e}"))?;
        if body.encode()!=bb||types::body_commitment(&body.txs)!=Some(h.tx_root)||types::body_commitment(&body.evidence)!=Some(h.evidence_root){return Err(format!("{at}: the body is not the one the header commits"))}
        // With the network's profile, H-pre on the parent's header: the ASERT target, nMax, the window and the rest.
        if let (Some(n),Some(p))=(network,parent.as_ref()){
            h_pre(&n.profile,p,&h,&body).map_err(|e|format!("{at}: the header fails H-pre on the given network ({e})"))?;
            parent=Some(Parent::of(&h));
        }
        // No other field, key order, spacing or hex case: the line is the one the export writes for this block.
        if line!=block_line(i,&hash,&hb,&bb).as_bytes(){return Err(format!("{at}: not the line the export writes for this block"))}
        txs+=body.txs.len() as u64;prev=hash;last=Some((i,hash,h.state_root));Ok(())
    })?;
    match last{Some((h,x,r)) if h==height&&x==head&&r==root=>{},_=>return Err("blocks.jsonl does not end at the manifest's height, headHash and stateRoot".into())}
    if blocks!=height+1{return Err("blocks.jsonl: not one line per height".into())}
    let reread=|k:usize,got:&(u64,String)|if *got==sizes[k]{Ok(())}else{Err(format!("{}: it changed while it was verified (its bytes are not the ones whose digest was checked)",PARTS[k]))};
    reread(0,&read)?;
    // State.
    let mut db=InMemoryDB::default();let mut before:Option<Address>=None;
    let (accounts,read)=lines(&dir.join(PARTS[1]),|i,line|{
        let at=format!("state.jsonl line {}",i+1);let v:Value=serde_json::from_slice(line).map_err(|_|format!("{at}: not JSON"))?;
        let a:Address=field(&v,"address",&at)?.as_str().and_then(|s|s.parse().ok()).ok_or_else(||format!("{at}: address"))?;
        if before.is_some_and(|b|b>=a){return Err(format!("{at}: addresses are not in strictly ascending order"))}before=Some(a);
        let nonce=field(&v,"nonce",&at)?.as_u64().ok_or_else(||format!("{at}: nonce"))?;
        let balance:U256=field(&v,"balance",&at)?.as_str().filter(|s|!s.is_empty()&&s.bytes().all(|b|b.is_ascii_digit())).and_then(|s|s.parse().ok()).ok_or_else(||format!("{at}: balance"))?;
        let code_hash=B256::from(hex32(&v,"codeHash",&at)?);
        let raw=unhex(field(&v,"code",&at)?.as_str().ok_or_else(||format!("{at}: code"))?)?;
        let code=if raw.is_empty(){if code_hash!=KECCAK_EMPTY{return Err(format!("{at}: code is missing"))}None}else{
            let c=Bytecode::new_raw(raw.clone().into());if c.hash_slow()!=code_hash{return Err(format!("{at}: code does not hash to codeHash"))}Some(c)};
        let info=AccountInfo{balance,nonce,code_hash,code};
        let mut slots:Vec<(U256,U256)>=Vec::new();
        for s in field(&v,"storage",&at)?.as_array().ok_or_else(||format!("{at}: storage"))?{
            let pair=s.as_array().filter(|p|p.len()==2).ok_or_else(||format!("{at}: storage entry"))?;
            let w=|x:&Value|->Result<U256,String>{let b:[u8;32]=unhex(x.as_str().unwrap_or("-"))?.try_into().map_err(|_|format!("{at}: storage word"))?;Ok(U256::from_be_bytes(b))};
            let (k,val)=(w(&pair[0])?,w(&pair[1])?);
            if val.is_zero()||slots.last().is_some_and(|(l,_)|*l>=k){return Err(format!("{at}: storage must hold nonzero slots in ascending order"))}
            slots.push((k,val));
        }
        // No other field, key order, spacing, address or hex case: the line is the one the export writes for it.
        if line!=account_line(&a,&info,&raw,&slots).as_bytes(){return Err(format!("{at}: not the line the export writes for this account"))}
        db.insert_account_info(a,info);
        for (k,val) in slots{db.insert_account_storage(a,k,val).map_err(|_|format!("{at}: storage"))?;}
        Ok(())
    })?;
    reread(1,&read)?;
    let got=apply::state_root(&db).map_err(|e|format!("state root: {e:?}"))?;
    if got.0!=root{return Err(format!("state.jsonl: its world-state root {} is not the stateRoot {}",hex(&got.0),hex(&root)))}
    // D and the other consensus parameters: from the state's ChainCaps, under the stateRoot.
    let d=state_d(&db)?;
    if stated_d!=d{return Err(format!("manifest: D is {stated_d}, but the state's ChainCaps holds D = {d} (consensus parameter {CP_D}, under the stateRoot)"))}
    if let Some(n)=network{
        for (i,v) in n.cp.iter().enumerate(){if state_cp(&db,i)?!=*v{return Err(format!("state.jsonl: consensus parameter {i} in the state's ChainCaps is not the given network's"))}}
    }
    // Facts, byte for byte as the export writes them for this state and D.
    let fbytes=read_small(&dir.join(PARTS[2]))?;
    reread(2,&(fbytes.len() as u64,hex(&Sha256::digest(&fbytes))))?;
    let f=facts(&db,height,d)?;
    if fbytes!=pretty(&f)?{return Err("facts.json is not what the state gives".into())}
    // The manifest, byte for byte as the export writes it for these checked values.
    let records=[blocks,accounts,1];
    let entries:Vec<Value>=PARTS.iter().zip(&sizes).zip(records).map(|((name,(n,sha)),r)|part_json(name,*n,r,sha)).collect();
    let entries:[Value;3]=entries.try_into().map_err(|_|"manifest: parts")?;
    if mbytes!=pretty(&manifest_json(chain_id,&g,height,&head,&root,d,entries))?{
        return Err("manifest.json is not the manifest of this export (a field, a part's record count, or its form differs)".into())
    }
    // `verified` is true only when block H is pinned to a hash the publisher did not supply (see above).
    let verdict=if want_head.is_some(){json!(true)}else{json!("selfConsistent")};
    Ok(json!({"verified":verdict,"chainId":chain_id,"genesisHash":hex(&g),"height":height,"headHash":hex(&head),"stateRoot":hex(&root),"D":d,"blocks":blocks,"transactions":txs,"accounts":accounts,
        "allocTotal":f["cutover"]["allocTotal"],"bondsCreditedTotal":f["cutover"]["bondsCreditedTotal"],"manifestSha256":msha,"network":network.is_some(),
        "headerChecks":if network.is_some(){"H-pre (ASERT target, nMax, window, version, signatures, base fee)"}else{"own target only"},
        "manifestSha256Checked":want_sha.is_some(),"headHashPinned":want_head.is_some()}))
}

#[cfg(test)]
mod tests{
    use super::*;
    /// Review finding 3: a pin is 64 hex digits with or without 0x, in either case; anything else is refused.
    #[test] fn pins_are_32_byte_hex_or_refused(){
        let x="ab".repeat(32);
        for ok in [x.clone(),format!("0x{x}"),format!("0X{}",x.to_ascii_uppercase()),format!(" {x}\n")]{assert_eq!(pin(&ok,"--p").unwrap(),[0xab;32],"{ok:?}");}
        for bad in ["".to_string(),"0x".into(),"00".into(),x[..62].into(),format!("{x}00"),format!("{}zz",&x[..62]),format!("0x0x{}",&x[..60])]{
            assert!(pin(&bad,"--p").unwrap_err().contains("--p must be 64 hex digits"),"{bad:?}");
        }
    }
    /// Review finding 4: `lines` hashes the very bytes it hands out, in the same pass, so `verify` can compare them
    /// with the digest it checked first; the digest equals the file's.
    #[test] fn lines_digest_the_bytes_they_read(){
        let d=std::env::temp_dir().join(format!("pocol-export-lines-{}",std::process::id()));let _=fs::remove_dir_all(&d);fs::create_dir_all(&d).unwrap();
        let p=d.join("part");fs::write(&p,b"{\"a\":1}\n\n{\"b\":2}\n").unwrap();
        let mut seen=Vec::new();let (n,got)=lines(&p,|_,l|{seen.push(l.to_vec());Ok(())}).unwrap();
        assert_eq!((n,seen),(3,vec![b"{\"a\":1}".to_vec(),vec![],b"{\"b\":2}".to_vec()]));
        assert_eq!(got,sha256_file(&p).unwrap());
        fs::write(&p,b"{\"a\":1}").unwrap();assert!(lines(&p,|_,_|Ok(())).unwrap_err().contains("not ended by a newline"));
        let _=fs::remove_dir_all(&d);
    }
}
