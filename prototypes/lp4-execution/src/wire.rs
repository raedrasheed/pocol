//! Signed PoCol envelope codec, not pool/block admission or boot authorization.
//! Canonical RLP reuses the unchanged LP1 decoder. The explicit byte budget is a
//! local API allocation bound; profile binding/caps and actual block apply remain
//! separate obligations. Only legacy155/2930/1559 are supported (M1 execution.md).
use crate::rlp;
use revm::primitives::{keccak256,Address,AccessListItem,B256,Bytes,TxEnv,TxKind,U256};
use crate::CHAIN_ID;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum WireError { Budget, Rlp, Shape, Integer, Address, AccessList, ChainId,
    UnprotectedLegacy, UnsupportedType, Parity, Signature, HighS, FeeCap }
impl WireError {
    /// Only the explicitly specified M1 pool mapping; no invented mapping for
    /// other codec/state errors. Actual RPC and included-block tests remain P3.
    pub fn specified_pool_code(self)->Option<i32>{match self{
        Self::ChainId|Self::UnprotectedLegacy|Self::UnsupportedType=>Some(-32015),_=>None}}
}

#[derive(Debug,Clone)]
pub struct SignedEnvelope {
    chain_id:u64, kind:u8, raw:Bytes, signing_hash:B256, sender:Address, nonce:u64, gas_limit:u64,
    max_fee:U256, priority:Option<U256>, to:TxKind, value:U256, input:Bytes,
    access_list:Vec<AccessListItem>,
}
impl SignedEnvelope {
    pub fn sender(&self)->Address{self.sender}
    pub fn hash(&self)->B256{keccak256(&self.raw)}
    pub fn kind(&self)->u8{self.kind}
    pub fn raw(&self)->&[u8]{&self.raw}
    pub fn tx_env(&self)->TxEnv{TxEnv{caller:self.sender,gas_limit:self.gas_limit,
        gas_price:self.max_fee,gas_priority_fee:self.priority,transact_to:self.to,
        value:self.value,data:self.input.clone(),nonce:Some(self.nonce),
        chain_id:Some(self.chain_id),access_list:self.access_list.clone(),..Default::default()}}
    pub fn view(&self)->serde_json::Value{serde_json::json!({"kind":self.kind,
        "sender":format!("{:#x}",self.sender),"hash":format!("{}",self.hash()),
        "signingHash":format!("{}",self.signing_hash),"nonce":self.nonce,
        "gasLimit":self.gas_limit,"maxFee":self.max_fee.to_string(),
        "priority":self.priority.map(|v|v.to_string()),"value":self.value.to_string(),
        "to":self.to.to().map(|a|format!("{a:#x}")),"input":self.input,
        "accessList":self.access_list})}
}
fn bytes<'a>(item:&rlp::Item<'a>)->Result<&'a[u8],WireError>{item.bytes().ok_or(WireError::Shape)}
fn uint(item:&rlp::Item<'_>)->Result<U256,WireError>{let b=bytes(item)?;
    if b.len()>32 || b.first()==Some(&0){return Err(WireError::Integer)}
    Ok(U256::from_be_slice(b))}
fn uint64(item:&rlp::Item<'_>)->Result<u64,WireError>{uint(item)?.try_into().map_err(|_|WireError::Integer)}
fn destination(item:&rlp::Item<'_>)->Result<TxKind,WireError>{let b=bytes(item)?;
    match b.len(){0=>Ok(TxKind::Create),20=>Ok(TxKind::Call(Address::from_slice(b))),_=>Err(WireError::Address)}}
fn accesses(item:&rlp::Item<'_>)->Result<Vec<AccessListItem>,WireError>{
    let mut result=Vec::new();
    for entry in item.list().ok_or(WireError::AccessList)?{
        let fields=entry.list().ok_or(WireError::AccessList)?;if fields.len()!=2{return Err(WireError::AccessList)}
        let address=bytes(&fields[0])?;if address.len()!=20{return Err(WireError::AccessList)}
        let mut keys=Vec::new();for key in fields[1].list().ok_or(WireError::AccessList)?{
            let key=bytes(key)?;if key.len()!=32{return Err(WireError::AccessList)}keys.push(B256::from_slice(key));}
        // EIP-2930 duplicates/order are preserved, not silently deduplicated.
        result.push(AccessListItem{address:Address::from_slice(address),storage_keys:keys});
    }Ok(result)
}
fn order()->U256{U256::from_be_slice(&[
    0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xfe,
    0xba,0xae,0xdc,0xe6,0xaf,0x48,0xa0,0x3b,0xbf,0xd2,0x5e,0x8c,0xd0,0x36,0x41,0x41])}

pub fn decode_signed(raw:&[u8],byte_budget:usize)->Result<SignedEnvelope,WireError>{decode_on_chain(raw,byte_budget,CHAIN_ID)}
/// LP3 schema/identity binding only, never complete ParamGate or boot authorization.
pub fn decode_for_profile(raw:&[u8],profile:&crate::profile::ProfileIdentity)->Result<SignedEnvelope,WireError>{
 decode_on_chain(raw,profile.cp().TX_MAX as usize,profile.spec().chain_id)
}
fn decode_on_chain(raw:&[u8],byte_budget:usize,chain_id:u64)->Result<SignedEnvelope,WireError>{
    if raw.len()>byte_budget{return Err(WireError::Budget)}
    let first=*raw.first().ok_or(WireError::Rlp)?;
    let (kind,encoded)=match first{0x01|0x02=>(first,&raw[1..]),0xc0..=0xff=>(0,raw),0x00..=0x7f=>return Err(WireError::UnsupportedType),_=>return Err(WireError::Rlp)};
    let decoded=rlp::decode_with_depth(encoded,4).map_err(|_|WireError::Rlp)?;
    let f=decoded.list().ok_or(WireError::Shape)?;
    let required=match kind{0=>9,1=>11,_=>12};if f.len()!=required{return Err(WireError::Shape)}
    let (nonce,gas_limit,max_fee,priority,to,value,input,access_list,parity,unsigned)=if kind==0{
        let v=uint(&f[6])?;if v==U256::from(27)||v==U256::from(28){return Err(WireError::UnprotectedLegacy)}
        if v<U256::from(35){return Err(WireError::Parity)}
        let expected=U256::from(chain_id)*U256::from(2)+U256::from(35);
        if v!=expected && v!=expected+U256::from(1){return Err(WireError::ChainId)}
        (uint64(&f[0])?,uint64(&f[2])?,uint(&f[1])?,None,destination(&f[3])?,uint(&f[4])?,Bytes::copy_from_slice(bytes(&f[5])?),Vec::new(),(v-expected).to::<u8>(),6)
    }else{
        if uint(&f[0])?!=U256::from(chain_id){return Err(WireError::ChainId)}
        let shift=usize::from(kind==2);let p=uint(&f[8+shift])?;if p>U256::from(1){return Err(WireError::Parity)}
        let max=uint(&f[2+shift])?;let priority=if kind==2{Some(uint(&f[2])?)}else{None};
        if priority.map(|p|p>max).unwrap_or(false){return Err(WireError::FeeCap)}
        (uint64(&f[1])?,uint64(&f[3+shift])?,max,priority,destination(&f[4+shift])?,uint(&f[5+shift])?,Bytes::copy_from_slice(bytes(&f[6+shift])?),accesses(&f[7+shift])?,p.to::<u8>(),8+shift)
    };
    let r=uint(&f[required-2])?;let s=uint(&f[required-1])?;let n=order();
    if r==U256::ZERO || r>=n || s==U256::ZERO || s>=n{return Err(WireError::Signature)}
    if s>n/U256::from(2){return Err(WireError::HighS)}
    let mut payload=Vec::new();for field in &f[..unsigned]{payload.extend_from_slice(field.raw)}
    if kind==0{let chain=U256::from(chain_id).to_be_bytes::<32>();let start=chain.iter().position(|x|*x!=0).unwrap();rlp::encode_bytes(&chain[start..],&mut payload);payload.extend([0x80,0x80]);}
    let mut preimage=Vec::new();if kind!=0{preimage.push(kind)}rlp::encode_list_payload(&payload,&mut preimage);
    let signing_hash=keccak256(preimage);let mut recovery=vec![0u8;128];recovery[..32].copy_from_slice(signing_hash.as_slice());recovery[63]=27+parity;recovery[64..96].copy_from_slice(&r.to_be_bytes::<32>());recovery[96..].copy_from_slice(&s.to_be_bytes::<32>());
    let recovered=revm::precompile::secp256k1::ec_recover_run(&Bytes::from(recovery),3000).map_err(|_|WireError::Signature)?;
    if recovered.bytes.len()!=32{return Err(WireError::Signature)}let sender=Address::from_word(B256::from_slice(&recovered.bytes));
    Ok(SignedEnvelope{chain_id,kind,raw:Bytes::copy_from_slice(raw),signing_hash,sender,nonce,gas_limit,max_fee,priority,to,value,input,access_list})
}
