//! Experimental signed-submission router, NOT complete M7 RPC or boot service.
//! Reuses accepted strict JSON/C19 id semantics. Only eth_chainId and
//! eth_sendRawTransaction; no block/header submission or signing endpoints.
use std::sync::Mutex;
use revm::InMemoryDB;
use crate::{json::{self,JVal},pool::Pool,native::NativeBlockContext,profile::ProfileIdentity};
pub struct Router<'a>{pool:Mutex<Pool>,snapshot:&'a InMemoryDB,block:&'a NativeBlockContext}
fn error(id:Option<u32>,code:i64,message:&str)->String{serde_json::json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}}).to_string()}
impl<'a> Router<'a>{
 pub fn new(profile:ProfileIdentity,snapshot:&'a InMemoryDB,block:&'a NativeBlockContext,budget:usize)->Self{Self{pool:Mutex::new(Pool::with_budget(profile,budget)),snapshot,block}}
 pub fn pending_bytes(&self)->usize{self.pool.lock().unwrap().bytes()}
 pub fn handle(&self,body:&[u8])->String{
  if body.len()>1024*1024{return error(None,-32014,"limit")}
  let v=match json::parse(body){Ok(v)=>v,Err(_)=>return error(None,-32700,"parse")};
  let fields=match &v{JVal::Obj(f)=>f,_=>return error(None,-32600,"request")};
  let id=match v.get("id"){Some(JVal::Num(s))=>match json::integral_u32(s){Some(n)=>n,None=>return error(None,-32600,"request")},_=>return error(None,-32600,"request")};
  if v.get("jsonrpc")!=Some(&JVal::Str("2.0".into())) || fields.iter().any(|(k,_)|!matches!(k.as_str(),"jsonrpc"|"id"|"method"|"params")){return error(Some(id),-32600,"request")}
  let method=match v.get("method"){Some(JVal::Str(m))=>m.as_str(),_=>return error(Some(id),-32600,"request")};
  let mut pool=match self.pool.lock(){Ok(p)=>p,Err(_)=>return error(Some(id),-32603,"internal")};
  match method{
   "eth_chainId"=>{if !v.get("params").map(|p|matches!(p,JVal::Arr(a) if a.is_empty())).unwrap_or(true){return error(Some(id),-32602,"params")};serde_json::json!({"jsonrpc":"2.0","id":id,"result":format!("0x{:x}",pool.identity().spec().chain_id)}).to_string()},
   "eth_sendRawTransaction"=>{
    let text=match v.get("params"){Some(JVal::Arr(a)) if a.len()==1=>match &a[0]{JVal::Str(s)=>s,_=>return error(Some(id),-32602,"params")},_=>return error(Some(id),-32602,"params")};
    let hex=match text.strip_prefix("0x"){Some(h)=>h,None=>return error(Some(id),-32602,"params")};
    // Bound before allocating decoded bytes; malformed odd hex is still params.
    if hex.len()%2!=0{return error(Some(id),-32602,"params")}
    if hex.len()/2>pool.identity().cp().TX_MAX as usize{return error(Some(id),-32014,"limit")}
    let raw=match crate::hex::decode(hex){Some(v)=>v,None=>return error(Some(id),-32602,"params")};
    match pool.submit(&raw,self.snapshot,self.block){Ok(hash)=>serde_json::json!({"jsonrpc":"2.0","id":id,"result":format!("{hash}")}).to_string(),Err(e)=>error(Some(id),e.rpc_code(),"transaction rejected")}
   },_=>error(Some(id),-32601,"method")
  }
 }
}
