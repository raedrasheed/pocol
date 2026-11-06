//! Transaction and header signing shared by devnet accounts and independent key files.
//! RFC6979 deterministic, low-s. Private operations use the corrected RustCrypto stack;
//! public recovery retains its signature policy on the same corrected backend.
use k256::ecdsa::{SigningKey,VerifyingKey};
use revm::primitives::{keccak256,Address,B256,Bytes,U256};
use crate::rlp;

pub struct Key{sk:SigningKey}
impl Key{
    pub fn from_secret(secret:&[u8;32])->Result<Self,String>{SigningKey::from_slice(secret).map(|sk|Self{sk}).map_err(|e|e.to_string())}
    /// Deterministic disposable devnet key: secret = keccak256("pocol-devnet-key" || label).
    pub fn devnet(label:&str)->Self{
        let mut i=0u32;loop{
            let s=keccak256([b"pocol-devnet-key".as_slice(),label.as_bytes(),&i.to_be_bytes()].concat());
            if let Ok(k)=Self::from_secret(&s.0){return k}i+=1;
        }
    }
    pub fn secret(&self)->[u8;32]{self.sk.to_bytes().into()}
    pub fn address(&self)->Address{
        let p=self.sk.verifying_key().to_sec1_point(false);
        Address::from_slice(&keccak256(&p.as_bytes()[1..])[12..])
    }
    /// 65-byte r||s||v (v in {0,1}), low-s, over a 32-byte prehash.
    pub fn sign_hash(&self,hash:&B256)->[u8;65]{
        // k256 0.14 sets EcdsaCurve::NORMALIZE_S: normalization and recovery-id
        // adjustment happen inside its signing primitive without a secret-dependent branch.
        let (sig,rec)=self.sk.sign_prehash_recoverable(hash.as_slice());
        let mut out=[0u8;65];out[..64].copy_from_slice(&sig.to_bytes());out[64]=rec.to_byte();out
    }
}
pub fn address_of(vk:&VerifyingKey)->Address{
    let p=vk.to_sec1_point(false);Address::from_slice(&keccak256(&p.as_bytes()[1..])[12..])
}
/// Recovers the signer of a 65-byte r||s||v signature (v in {0,1}); rejects high-s.
pub fn recover(hash:&B256,sig:&[u8])->Option<Address>{
    if sig.len()!=65||sig[64]>1{return None}
    let s=k256::ecdsa::Signature::from_slice(&sig[..64]).ok()?;
    if s.normalize_s()!=s{return None}
    let rec=k256::ecdsa::RecoveryId::from_byte(sig[64])?;
    VerifyingKey::recover_from_prehash(hash.as_slice(),&s,rec).ok().map(|vk|address_of(&vk))
}

fn uint(v:U256,out:&mut Vec<u8>){let b=v.to_be_bytes::<32>();let f=b.iter().position(|x|*x!=0).unwrap_or(32);rlp::encode_bytes(&b[f..],out)}
fn list(payload:&[u8])->Vec<u8>{let mut o=Vec::new();rlp::encode_list_payload(payload,&mut o);o}

pub struct Tx{pub chain_id:u64,pub nonce:u64,pub max_priority:U256,pub max_fee:U256,pub gas:u64,pub to:Option<Address>,pub value:U256,pub data:Bytes}
impl Tx{
    fn fields(&self,p:&mut Vec<u8>){
        uint(U256::from(self.chain_id),p);uint(U256::from(self.nonce),p);uint(self.max_priority,p);uint(self.max_fee,p);uint(U256::from(self.gas),p);
        match self.to{Some(a)=>rlp::encode_bytes(a.as_slice(),p),None=>rlp::encode_bytes(&[],p)}
        uint(self.value,p);rlp::encode_bytes(&self.data,p);p.extend(list(&[]));
    }
    /// EIP-1559 envelope 0x02 || RLP([...fields, yParity, r, s]).
    pub fn sign_1559(&self,key:&Key)->Vec<u8>{
        let mut p=Vec::new();self.fields(&mut p);
        let hash=keccak256([&[2u8][..],&list(&p)].concat());let sig=key.sign_hash(&hash);
        uint(U256::from(sig[64]),&mut p);uint(U256::from_be_slice(&sig[..32]),&mut p);uint(U256::from_be_slice(&sig[32..64]),&mut p);
        [&[2u8][..],&list(&p)].concat()
    }
    /// EIP-155 legacy: gasPrice = max_fee.
    pub fn sign_legacy(&self,key:&Key)->Vec<u8>{
        let mut p=Vec::new();uint(U256::from(self.nonce),&mut p);uint(self.max_fee,&mut p);uint(U256::from(self.gas),&mut p);
        match self.to{Some(a)=>rlp::encode_bytes(a.as_slice(),&mut p),None=>rlp::encode_bytes(&[],&mut p)}
        uint(self.value,&mut p);rlp::encode_bytes(&self.data,&mut p);
        let mut q=p.clone();uint(U256::from(self.chain_id),&mut q);uint(U256::ZERO,&mut q);uint(U256::ZERO,&mut q);
        let sig=key.sign_hash(&keccak256(list(&q)));
        uint(U256::from(self.chain_id*2+35+u64::from(sig[64])),&mut p);uint(U256::from_be_slice(&sig[..32]),&mut p);uint(U256::from_be_slice(&sig[32..64]),&mut p);
        list(&p)
    }
}
