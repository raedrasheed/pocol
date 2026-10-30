//! Block/Body types and consensus hashes (consensus.md §1).
use crate::{header::{self,Header},hashes,rlp};
use revm::primitives::{Address,B256,U256};

pub type Fixed=crate::fixed::U256;
pub fn to_fixed(v:U256)->Fixed{Fixed::from_be_slice(&v.to_be_bytes::<32>()).unwrap()}
pub fn from_fixed(v:&Fixed)->U256{U256::from_be_bytes(v.to_be_bytes())}
pub fn addr(a:&[u8;20])->Address{Address::from(*a)}
pub fn raw_addr(a:Address)->[u8;20]{a.0.0}

/// Body = RLP([txs, evidence]); every element is an RLP byte string of the raw
/// envelope. The empty body is exactly 0xc2c0c0.
#[derive(Clone,Debug,PartialEq,Eq,Default)]
pub struct Body{pub txs:Vec<Vec<u8>>,pub evidence:Vec<Vec<u8>>}
impl Body{
    pub fn encode(&self)->Vec<u8>{
        let list=|items:&[Vec<u8>]|{let mut p=Vec::new();for i in items{rlp::encode_bytes(i,&mut p)}let mut o=Vec::new();rlp::encode_list_payload(&p,&mut o);o};
        let mut p=list(&self.txs);p.extend(list(&self.evidence));let mut o=Vec::new();rlp::encode_list_payload(&p,&mut o);o
    }
    pub fn decode(b:&[u8],max:usize)->Result<Self,String>{
        if b.len()>max{return Err("BODY_MAX".into())}
        let it=rlp::decode(b).map_err(|e|format!("body rlp {}",e.code()))?;
        let top=it.list().ok_or("body shape")?;if top.len()!=2{return Err("body fields".into())}
        let items=|x:&rlp::Item<'_>|->Result<Vec<Vec<u8>>,String>{x.list().ok_or("body list")?.iter().map(|e|e.bytes().map(|b|b.to_vec()).ok_or_else(||"body item".to_string())).collect()};
        let body=Self{txs:items(&top[0])?,evidence:items(&top[1])?};
        if body.encode()!=b{return Err("noncanonical body".into())}
        Ok(body)
    }
    pub fn is_empty(&self)->bool{self.txs.is_empty()&&self.evidence.is_empty()}
}
pub const EMPTY_BODY:[u8;3]=[0xc2,0xc0,0xc0];

#[derive(Clone,Debug)]
pub struct Block{pub header:Header,pub body:Body}
impl Block{
    pub fn template_id(&self)->[u8;32]{template_id(&self.header.ut_raw)}
    pub fn hash(&self)->[u8;32]{block_hash(&self.header)}
}
pub fn template_id(ut_raw:&[u8])->[u8;32]{hashes::sha256(ut_raw)}
pub fn share_root(h:&Header)->[u8;32]{hashes::share_root(&h.share_list_raw)}
pub fn block_hash(h:&Header)->[u8;32]{hashes::block_hash(&template_id(&h.ut_raw),h.nonce,&share_root(h))}
pub fn pow_hash(tid:&[u8;32],nonce:u64)->[u8;32]{hashes::sha256(&hashes::tid_nonce_preimage(tid,nonce))}
pub fn seal(tid:&[u8;32],nonce:u64)->[u8;32]{hashes::keccak256(&hashes::tid_nonce_preimage(tid,nonce))}
pub fn seal_genesis(genesis_hash:&[u8;32])->[u8;32]{hashes::keccak256(genesis_hash)}
pub fn prevrandao(seal_p:&[u8;32])->B256{let mut b=b"PoColRandao".to_vec();b.extend_from_slice(seal_p);B256::from(hashes::sha256(&b))}
pub fn le_target(hash:&[u8;32],target:&Fixed)->bool{Fixed::from_be_slice(hash).unwrap()<=*target}

/// Re-derives the exact raw spans (ut_raw, share_list_raw, encoded) after editing typed fields.
pub fn reseal(h:&mut Header){
    h.ut_raw=h.encode_ut();h.share_list_raw=h.encode_share_list();h.encoded=h.encode();
}
pub fn decode_header(b:&[u8])->Result<Header,String>{
    let h=header::decode(b).map_err(|e|format!("header {}",e.detail))?;
    if h.encode()!=b{return Err("noncanonical header".into())}
    Ok(h)
}
/// A header with empty signatures/nonce/shares for a given UT (template only).
pub fn blank_header()->Header{
    Header{chain_id:0,genesis_hash:[0;32],parent_hash:[0;32],h:0,a:0,protocol_version:1,ts:0,target:Fixed::ONE,state_root:[0;32],tx_root:[0;32],
        receipts_root:[0;32],logs_bloom:vec![0;256],gas_limit:0,gas_used:0,base_fee:Fixed::ZERO,evidence_root:[0;32],proposer:vec![],sig:vec![],
        nonce:0,shares:vec![],winner_sig:[0;65],ut_raw:vec![],share_list_raw:vec![],encoded:vec![]}
}
pub fn hex(b:&[u8])->String{let mut s=String::from("0x");for x in b{s.push_str(&format!("{x:02x}"))}s}
pub fn unhex(s:&str)->Result<Vec<u8>,String>{let s=s.strip_prefix("0x").unwrap_or(s);if s.len()%2!=0{return Err("odd hex".into())}
    (0..s.len()).step_by(2).map(|i|u8::from_str_radix(&s[i..i+2],16).map_err(|e|e.to_string())).collect()}
/// MPT root keyed by RLP(i) (txRoot, evidenceRoot).
/// Commitment of a received body list, or None when the list cannot be the committed list of any
/// header (an empty item has no trie encoding; an over-budget list has no root). Used before the
/// body is bound to its header, so a None is an envelope fault of that copy, never a block verdict.
pub fn body_commitment(items:&[Vec<u8>])->Option<[u8;32]>{
    if items.iter().any(|v|v.is_empty()){return None}
    pocol_lp4_execution::receipts::ordered_root(items,items.len().max(1)).ok().map(|r|r.0)
}
pub fn ordered_root(items:&[Vec<u8>])->Result<[u8;32],crate::chain::Reject>{
    if items.iter().any(|v|v.is_empty()){return Err(crate::chain::Reject::Block("15 empty body item".into()))}
    pocol_lp4_execution::receipts::ordered_root(items,items.len().max(1)).map(|r|r.0).map_err(|e|crate::chain::Reject::Block(format!("15 root {e:?}")))
}
