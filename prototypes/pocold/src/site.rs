//! LP8 web storage (M1 storage.md): ChunkFactory chunks (D27), the RLP site
//! manifest with strict validation (T16), path normalization (D96), Website
//! contract ABI, and a publisher/fetcher over loopback JSON-RPC.
use std::collections::BTreeMap;
use std::io::{Read,Write};
use std::net::TcpStream;
use std::time::{Duration,Instant};
use revm::primitives::{keccak256,Address,B256,U256};
use serde_json::{json,Value};
use crate::{rlp,types::{hex,unhex}};

pub const FACTORY:u64=0xC0C005;
pub const CHUNK_MAX:usize=24575;
pub const SITE_FILE_MAX:usize=1024*1024;
pub const SITE_MAX:usize=4*1024*1024;
pub const FILES_MAX:usize=256;
pub const MANIFEST_MAX:usize=65536;
pub fn factory()->Address{Address::from_word(U256::from(FACTORY).into())}

pub const MIME:[(u8,&str,&str);10]=[(1,"text/html","html"),(2,"text/css","css"),(3,"application/javascript","js"),(4,"application/json","json"),(5,"image/png","png"),
    (6,"image/jpeg","jpg"),(7,"image/webp","webp"),(8,"image/svg+xml","svg"),(9,"font/woff2","woff2"),(10,"text/plain","txt")];
pub fn mime_for_path(p:&str)->Option<u8>{let ext=p.rsplit('.').next()?.to_ascii_lowercase();let ext=if ext=="htm"{"html".into()}else if ext=="jpeg"{"jpg".into()}else if ext=="mjs"{"js".into()}else{ext};MIME.iter().find(|m|m.2==ext).map(|m|m.0)}
pub fn mime_name(id:u8)->Option<&'static str>{MIME.iter().find(|m|m.0==id).map(|m|m.1)}

/// path := ('/' seg)+, seg = [A-Za-z0-9._~-]+, not '.'/'..', <= 256 bytes.
pub fn valid_path(p:&str)->bool{
    p.len()<=256&&p.starts_with('/')&&p.len()>1&&p[1..].split('/').all(|s|!s.is_empty()&&s!="."&&s!=".."&&s.bytes().all(|c|c.is_ascii_alphanumeric()||b"._~-".contains(&c)))
}
/// Normalization used by the renderer and site_navigate: query/fragment ignored;
/// '%', '//', '.', '..' -> None (404); '' or '/' -> entry; '/x/' -> '/x/index.html'.
pub fn normalize(raw:&str)->Option<Option<String>>{
    let p=raw.split(['?','#']).next().unwrap_or("");
    if p.is_empty()||p=="/"{return Some(None)}
    if p.contains('%')||p.contains("//")||p.split('/').any(|s|s=="."||s==".."){return None}
    let p=if p.ends_with('/'){format!("{p}index.html")}else{p.to_string()};
    if !valid_path(&p){return None}
    Some(Some(p))
}

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct FileEntry{pub path:String,pub mime:u8,pub size:u32,pub content_hash:B256,pub chunks:Vec<(Address,u32)>}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Manifest{pub entry:u32,pub files:Vec<FileEntry>}

fn u(v:u64,o:&mut Vec<u8>){let b=v.to_be_bytes();let f=b.iter().position(|x|*x!=0).unwrap_or(8);rlp::encode_bytes(&b[f..],o)}
fn list(p:&[u8])->Vec<u8>{let mut o=Vec::new();rlp::encode_list_payload(p,&mut o);o}
impl Manifest{
    /// RLP([1, entryIndex, files]); file = [path, mimeId, size, contentHash, [[addr20,len]...]].
    pub fn encode(&self)->Vec<u8>{
        let mut files=Vec::new();
        for f in &self.files{
            let mut c=Vec::new();for (a,l) in &f.chunks{let mut x=Vec::new();rlp::encode_bytes(a.as_slice(),&mut x);u(*l as u64,&mut x);c.extend(list(&x));}
            let mut p=Vec::new();rlp::encode_bytes(f.path.as_bytes(),&mut p);u(f.mime as u64,&mut p);u(f.size as u64,&mut p);rlp::encode_bytes(f.content_hash.as_slice(),&mut p);p.extend(list(&c));
            files.extend(list(&p));
        }
        let mut p=Vec::new();u(1,&mut p);u(self.entry as u64,&mut p);p.extend(list(&files));list(&p)
    }
    /// Strict decode + every T16 rule except chunk content (checked when fetched).
    pub fn decode(b:&[u8])->Result<Self,String>{
        if b.len()>MANIFEST_MAX{return Err("manifest > 64 KiB".into())}
        let it=rlp::decode(b).map_err(|e|format!("manifest RLP {}",e.code()))?;
        let top=it.list().ok_or("manifest shape")?;if top.len()!=3{return Err("manifest arity".into())}
        let int=|x:&rlp::Item<'_>,w:usize|->Result<u64,String>{let v=x.bytes().ok_or("int")?;if v.len()>w||v.first()==Some(&0){return Err("noncanonical int".into())}Ok(v.iter().fold(0u64,|a,c|(a<<8)|*c as u64))};
        if int(&top[0],1)?!=1{return Err("manifest version".into())}
        let entry=int(&top[1],4)? as u32;
        let mut files=Vec::new();let mut total=0usize;
        for f in top[2].list().ok_or("files")?{
            let x=f.list().ok_or("file")?;if x.len()!=5{return Err("file arity".into())}
            let path=String::from_utf8(x[0].bytes().ok_or("path")?.to_vec()).map_err(|_|"path utf8")?;
            if !path.is_ascii()||!valid_path(&path){return Err(format!("bad path {path:?}"))}
            let mime=int(&x[1],1)? as u8;if mime_name(mime).is_none(){return Err(format!("unknown mime {mime}"))}
            let size=int(&x[2],4)? as u32;
            let h=x[3].bytes().ok_or("hash")?;if h.len()!=32{return Err("hash len".into())}
            let mut chunks=Vec::new();let mut sum=0u64;
            for c in x[4].list().ok_or("chunks")?{let y=c.list().ok_or("chunk")?;if y.len()!=2{return Err("chunk arity".into())}
                let a=y[0].bytes().ok_or("chunk addr")?;if a.len()!=20{return Err("chunk addr len".into())}
                let l=int(&y[1],4)?;if l==0||l as usize>CHUNK_MAX{return Err("chunk len".into())}sum+=l;chunks.push((Address::from_slice(a),l as u32));}
            if sum!=size as u64||size as usize>SITE_FILE_MAX{return Err(format!("size mismatch for {path}"))}
            total+=size as usize;files.push(FileEntry{path,mime,size,content_hash:B256::from_slice(h),chunks});
        }
        if files.is_empty()||files.len()>FILES_MAX||total>SITE_MAX{return Err("file count/site size".into())}
        for w in files.windows(2){if w[0].path.as_bytes()>=w[1].path.as_bytes(){return Err("paths not strictly sorted (or duplicate)".into())}}
        if entry as usize>=files.len()||files[entry as usize].mime!=1{return Err("entry is not html".into())}
        let m=Self{entry,files};if m.encode()!=b{return Err("noncanonical manifest (trailing bytes)".into())}
        Ok(m)
    }
}
/// D27 chunk initcode and its CREATE2 address under the ChunkFactory.
pub fn chunk_initcode(data:&[u8])->Vec<u8>{let l=(1+data.len()) as u16;let mut v=vec![0x61];v.extend(l.to_be_bytes());v.extend([0x80,0x60,0x0a,0x5f,0x39,0x5f,0xf3,0x00]);v.extend(data);v}
pub fn chunk_address(data:&[u8])->Address{
    let salt=keccak256(data);let init=keccak256(chunk_initcode(data));
    let mut p=vec![0xff];p.extend(factory().as_slice());p.extend(salt.as_slice());p.extend(init.as_slice());Address::from_slice(&keccak256(&p)[12..])
}
/// Validates fetched chunk code: code[0]==0x00, length, and the factory address.
pub fn chunk_payload(addr:Address,code:&[u8],len:u32)->Result<Vec<u8>,String>{
    if code.first()!=Some(&0){return Err("code[0] != 0x00".into())}
    let data=&code[1..];if data.len()!=len as usize{return Err("chunk length mismatch".into())}
    if chunk_address(data)!=addr{return Err("chunk not produced by the ChunkFactory".into())}
    Ok(data.to_vec())
}

// ---------------- ABI ----------------
pub fn sel(sig:&str)->Vec<u8>{keccak256(sig.as_bytes())[..4].to_vec()}
fn word(v:U256)->[u8;32]{v.to_be_bytes()}
pub fn abi_bytes_call(sig:&str,data:&[u8])->Vec<u8>{let mut o=sel(sig);o.extend(word(U256::from(32)));o.extend(word(U256::from(data.len())));o.extend(data);o.resize(o.len()+(32-data.len()%32)%32,0);o}
pub fn abi_u256_call(sig:&str,v:u64)->Vec<u8>{let mut o=sel(sig);o.extend(word(U256::from(v)));o}
pub fn abi_create_version(hash:B256,len:u32,chunks:&[Address])->Vec<u8>{
    let mut o=sel("createVersion(bytes32,uint32,address[])");o.extend(hash.0);o.extend(word(U256::from(len)));o.extend(word(U256::from(96)));o.extend(word(U256::from(chunks.len())));
    for c in chunks{o.extend(c.into_word().0)}o
}
pub struct Current{pub version:u64,pub manifest_hash:B256,pub manifest_len:u32,pub published_block:u64,pub chunks:Vec<Address>}
/// ABI word i as an integer no wider than `max` (hostile contracts cannot make this panic).
fn abi_word(out:&[u8],i:usize,max:u64)->Result<u64,String>{
    let w=out.get(i*32..i*32+32).ok_or("abi: short output")?;let v=U256::from_be_slice(w);
    if v>U256::from(max){return Err("abi: word out of range".into())}Ok(v.to::<u64>())
}
/// Dynamic address[] (<= 3 entries) at the offset stored in head word `head`.
fn abi_addresses(out:&[u8],head:usize)->Result<Vec<Address>,String>{
    let off=abi_word(out,head,out.len() as u64)? as usize;if off%32!=0{return Err("abi: offset".into())}
    let n=abi_word(out,off/32,3)? as usize;
    (0..n).map(|i|{let w=out.get(off+32+32*i..off+64+32*i).ok_or("abi: short array")?;if w[..12].iter().any(|b|*b!=0){return Err("abi: address".into())}Ok(Address::from_slice(&w[12..]))}).collect()
}
pub fn decode_current(out:&[u8])->Result<Current,String>{
    if out.len()<32*6{return Err("current() output".into())}
    Ok(Current{version:abi_word(out,0,u64::MAX)?,manifest_hash:B256::from_slice(&out[32..64]),manifest_len:abi_word(out,2,u32::MAX as u64)? as u32,
        published_block:abi_word(out,3,u64::MAX)?,chunks:abi_addresses(out,4)?})
}
/// version(v) -> (manifestHash, manifestLen, status, publishedBlock, chunks).
pub fn decode_version(out:&[u8])->Result<(B256,u32,u8,Vec<Address>),String>{
    if out.len()<32*6{return Err("version() output".into())}
    Ok((B256::from_slice(&out[0..32]),abi_word(out,1,u32::MAX as u64)? as u32,abi_word(out,2,255)? as u8,abi_addresses(out,4)?))
}

// ---------------- RPC client ----------------
pub struct Rpc{pub addr:String}
impl Rpc{
    pub fn call(&self,method:&str,params:Value)->Result<Value,String>{
        let body=json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string();
        let mut s=TcpStream::connect(&self.addr).map_err(|e|format!("connect {}: {e}",self.addr))?;s.set_read_timeout(Some(Duration::from_secs(30))).ok();
        write!(s,"POST / HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",self.addr,body.len()).map_err(|e|e.to_string())?;
        let mut r=Vec::new();s.read_to_end(&mut r).map_err(|e|e.to_string())?;
        let i=r.windows(4).position(|w|w==b"\r\n\r\n").ok_or("http")?;let v:Value=serde_json::from_slice(&r[i+4..]).map_err(|e|format!("json {e}"))?;
        if let Some(e)=v.get("error"){return Err(format!("{method}: {e}"))}Ok(v["result"].clone())
    }
    pub fn code(&self,a:Address)->Result<Vec<u8>,String>{unhex(self.call("eth_getCode",json!([format!("{a:#x}"),"latest"]))?.as_str().unwrap_or("0x"))}
    pub fn eth_call(&self,to:Address,data:&[u8])->Result<Vec<u8>,String>{unhex(self.call("eth_call",json!([{"to":format!("{to:#x}"),"data":hex(data)},"latest"]))?.as_str().unwrap_or("0x"))}
    pub fn nonce(&self,a:Address)->Result<u64,String>{let v=self.call("eth_getTransactionCount",json!([format!("{a:#x}"),"pending"]))?;u64::from_str_radix(v.as_str().unwrap_or("0x0").trim_start_matches("0x"),16).map_err(|e|e.to_string())}
    pub fn send(&self,raw:&[u8])->Result<B256,String>{self.call("eth_sendRawTransaction",json!([hex(raw)]))?.as_str().ok_or("hash".to_string())?.parse().map_err(|_|"hash".to_string())}
    pub fn wait_receipt(&self,h:B256,timeout:Duration)->Result<Value,String>{
        let t=Instant::now();loop{let r=self.call("eth_getTransactionReceipt",json!([format!("{h}")]))?;if !r.is_null(){return Ok(r)}if t.elapsed()>timeout{return Err(format!("receipt timeout {h}"))}std::thread::sleep(Duration::from_millis(250));}
    }
}

// ---------------- publisher ----------------
pub struct Publisher<'a>{pub rpc:&'a Rpc,pub key:&'a pocol_lp4_execution::txsign::Key,pub chain_id:u64,pub log:bool}
impl Publisher<'_>{
    fn tx(&self,to:Option<Address>,data:Vec<u8>,gas:u64)->Result<Value,String>{
        let nonce=self.rpc.nonce(self.key.address())?;
        let raw=pocol_lp4_execution::txsign::Tx{chain_id:self.chain_id,nonce,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(200_000_000_000u64),gas,to,value:U256::ZERO,data:data.into()}.sign_1559(self.key);
        let h=self.rpc.send(&raw)?;let r=self.rpc.wait_receipt(h,Duration::from_secs(300))?;
        if r["status"]!="0x1"{return Err(format!("transaction failed: {r}"))}Ok(r)
    }
    /// Stores one chunk (idempotent: skipped when the CREATE2 address has code).
    pub fn store_chunk(&self,data:&[u8])->Result<Address,String>{
        let a=chunk_address(data);if !self.rpc.code(a)?.is_empty(){return Ok(a)}
        let gas=200_000+data.len() as u64*260;self.tx(Some(factory()),abi_bytes_call("store(bytes)",data),gas.min(7_900_000))?;
        if self.rpc.code(a)?.is_empty(){return Err("chunk not deployed".into())}Ok(a)
    }
    pub fn deploy_website(&self)->Result<Address,String>{
        let init=website_initcode();let r=self.tx(None,init,1_500_000)?;r["contractAddress"].as_str().ok_or("no contractAddress".to_string())?.parse().map_err(|_|"addr".to_string())
    }
    /// Publishes `files` (path -> bytes) as a new version and makes it current. Every rule (paths,
    /// MIME, sizes, entry, manifest T16 and limits) is checked on the predicted manifest before the
    /// first transaction, so a rejected site has no state effect.
    pub fn publish(&self,site:Option<Address>,files:&BTreeMap<String,Vec<u8>>,entry:&str)->Result<(Address,u64,Manifest),String>{
        if files.is_empty()||files.len()>FILES_MAX{return Err("1..256 files".into())}
        let predicted=|data:&[u8]|->Vec<(Address,u32)>{data.chunks(CHUNK_MAX).map(|c|(chunk_address(c),c.len() as u32)).collect()};
        let mut entries=Vec::new();let mut total=0usize;
        for (p,data) in files{
            if !valid_path(p){return Err(format!("invalid path {p}"))}let mime=mime_for_path(p).ok_or(format!("unknown MIME for {p}"))?;
            if data.len()>SITE_FILE_MAX{return Err(format!("{p}: size 0..1 MiB"))}total+=data.len();
            entries.push(FileEntry{path:p.clone(),mime,size:data.len() as u32,content_hash:keccak256(data),chunks:predicted(data)});
        }
        if total>SITE_MAX{return Err("site > 4 MiB".into())}
        let entry=entries.iter().position(|f|f.path==entry).ok_or("entry path not found")? as u32;
        let m=Manifest{entry,files:entries};let mb=m.encode();Manifest::decode(&mb)?;
        let mpred=predicted(&mb);if mpred.is_empty()||mpred.len()>3{return Err("manifest needs 1..3 chunks".into())}
        // Validated: now store chunks (idempotent CREATE2) and check each lands at its predicted address.
        for (p,data) in files{
            if self.log{eprintln!("storing {p} ({} bytes)",data.len());}
            for c in data.chunks(CHUNK_MAX){self.store_chunk(c)?;}
        }
        for c in mb.chunks(CHUNK_MAX){self.store_chunk(c)?;}
        let site=match site{Some(s)=>s,None=>self.deploy_website()?};
        let count_before=U256::from_be_slice(&self.rpc.eth_call(site,&sel("versionCount()"))?).to::<u64>();
        let mhash=keccak256(&mb);
        self.tx(Some(site),abi_create_version(mhash,mb.len() as u32,&mpred.iter().map(|c|c.0).collect::<Vec<_>>()),600_000)?;
        // Another publisher may have created a version concurrently: find the draft with this exact manifest.
        let count_after=U256::from_be_slice(&self.rpc.eth_call(site,&sel("versionCount()"))?).to::<u64>();
        let mut mine=None;
        for v in count_before..count_after{
            let (h,l,status,_)=decode_version(&self.rpc.eth_call(site,&abi_u256_call("version(uint256)",v))?)?;
            if h==mhash&&l==mb.len() as u32&&status==0{mine=Some(v);break}
        }
        let v=mine.ok_or("created version not found (concurrent publisher?)")?;
        self.tx(Some(site),abi_u256_call("publish(uint256)",v),200_000)?;
        self.tx(Some(site),abi_u256_call("setCurrent(uint256)",v),200_000)?;
        Ok((site,v,m))
    }
}
pub fn website_initcode()->Vec<u8>{
    let v:Value=serde_json::from_str(include_str!("../../lp4-execution/tests/data/website.json")).unwrap();unhex(v["initcode"].as_str().unwrap()).unwrap()
}

// ---------------- fetcher ----------------
/// Fetches and fully verifies the current version of `site`: manifest chunks,
/// manifest hash/length/T16 rules, every file chunk (factory/code/len) and
/// every file contentHash. Returns (manifest, path -> bytes).
pub fn fetch(rpc:&Rpc,site:Address)->Result<(Current,Manifest,BTreeMap<String,Vec<u8>>),String>{
    let cur=decode_current(&rpc.eth_call(site,&sel("current()"))?)?;
    let mut mb=Vec::new();let mut left=cur.manifest_len as usize;
    for a in &cur.chunks{let code=rpc.code(*a)?;let l=(code.len().saturating_sub(1)).min(left);let d=chunk_payload(*a,&code,l as u32)?;left-=d.len();mb.extend(d);}
    if mb.len()!=cur.manifest_len as usize||keccak256(&mb)!=cur.manifest_hash{return Err("manifest hash/length mismatch".into())}
    let m=Manifest::decode(&mb)?;let mut files=BTreeMap::new();
    for f in &m.files{
        let mut data=Vec::with_capacity(f.size as usize);
        for (a,l) in &f.chunks{data.extend(chunk_payload(*a,&rpc.code(*a)?,*l)?);}
        if keccak256(&data)!=f.content_hash{return Err(format!("content hash mismatch for {}",f.path))}
        files.insert(f.path.clone(),data);
    }
    Ok((cur,m,files))
}
