//! Receipt/bloom/ordered-root primitives, not full block/apply acceptance.
//! Uses unchanged P1 trie/fixed/RLP source copies, byte-compared to originals.
use crate::{rlp,trie};
use revm::primitives::{keccak256,B256,Log};
#[derive(Debug,Clone,Copy,PartialEq,Eq)] pub enum ReceiptError{UnsupportedType,Budget,EmptyEnvelope,Trie}
pub fn bloom(logs:&[Log])->[u8;256]{
 let mut out=[0u8;256];for log in logs{
  for bytes in std::iter::once(log.address.as_slice()).chain(log.data.topics().iter().map(|t|t.as_slice())){
   let hash=keccak256(bytes);for i in [0usize,2,4]{let bit=(((hash[i] as usize)<<8)|hash[i+1] as usize)&2047;out[255-bit/8]|=1<<(bit%8);}
  }
 }out
}
fn bytes(data:&[u8])->Vec<u8>{let mut out=Vec::new();rlp::encode_bytes(data,&mut out);out}
fn list(parts:&[Vec<u8>])->Vec<u8>{let payload=parts.iter().flatten().copied().collect::<Vec<_>>();let mut out=Vec::new();rlp::encode_list_payload(&payload,&mut out);out}
pub fn encode(kind:u8,success:bool,cumulative_gas:u64,logs:&[Log])->Result<Vec<u8>,ReceiptError>{
 if kind>2{return Err(ReceiptError::UnsupportedType)}
 let receipt_bloom=bloom(logs);let gas=cumulative_gas.to_be_bytes();let start=gas.iter().position(|x|*x!=0).unwrap_or(8);
 let logs=logs.iter().map(|l|list(&[bytes(l.address.as_slice()),list(&l.data.topics().iter().map(|t|bytes(t.as_slice())).collect::<Vec<_>>()),bytes(&l.data.data)])).collect::<Vec<_>>();
 let encoded=list(&[bytes(if success{&[1]}else{&[]}),bytes(&gas[start..]),bytes(&receipt_bloom),list(&logs)]);
 let mut out=Vec::new();if kind!=0{out.push(kind)}out.extend(encoded);Ok(out)
}
pub fn ordered_root(values:&[Vec<u8>],entry_budget:usize)->Result<B256,ReceiptError>{
 if values.len()>entry_budget{return Err(ReceiptError::Budget)}
 if values.iter().any(|v|v.is_empty()){return Err(ReceiptError::EmptyEnvelope)}
 trie::ordered_root(values,entry_budget).map(B256::from).map_err(|_|ReceiptError::Trie)
}
