use super::*;
use revm::primitives::{AccountStatus,Bytes,EvmStorageSlot};

struct Rng(u64);
impl Rng{fn next(&mut self)->u64{self.0^=self.0<<13;self.0^=self.0>>7;self.0^=self.0<<17;self.0}fn below(&mut self,n:u64)->u64{self.next()%n}}
fn addr(r:&mut Rng)->Address{Address::from_word(B256::from(U256::from(r.below(30))))}
fn code(r:&mut Rng)->Option<Bytecode>{match r.below(4){0=>Some(Bytecode::new_raw(Bytes::from(vec![0x60,r.below(250) as u8]))),_=>None}}
/// A random revm commit: touched accounts with new info, created or self-destructed ones, changed and
/// unchanged slots.
fn changes(r:&mut Rng,v:&dyn View)->HashMap<Address,Account>{
    let mut m=HashMap::new();
    for _ in 0..1+r.below(4){
        let a=addr(r);let mut acct=Account{info:v.account(&a).map(|x|x.info.clone()).unwrap_or_default(),storage:Default::default(),status:AccountStatus::Touched};
        match r.below(6){
            0=>acct.status|=AccountStatus::SelfDestructed,
            1=>{acct.status|=AccountStatus::Created;acct.info.nonce=1;if let Some(c)=code(r){acct.info.code_hash=c.hash_slow();acct.info.code=Some(c);}}
            2=>{acct.info=AccountInfo::default();} // touched and empty: an EIP-161 removal by commit_tx
            _=>{acct.info.balance=U256::from(r.below(100));acct.info.nonce+=r.below(2);}
        }
        for _ in 0..r.below(4){let k=U256::from(r.below(12));let orig=v.slot(&a,&k);acct.storage.insert(k,EvmStorageSlot::new_changed(orig,U256::from(r.below(3))));}
        if r.below(5)==0{acct.status=AccountStatus::Loaded;} // not touched: skipped by commit
        m.insert(a,acct);
    }
    m
}
fn same_view(a:&dyn View,b:&dyn View,ctx:&str){
    for i in 0..30u64{
        let x=Address::from_word(B256::from(U256::from(i)));
        let (p,q)=(a.account(&x),b.account(&x));
        assert_eq!(p.map(|p|(p.exists(),p.info.nonce,p.info.balance,p.info.code_hash)),q.map(|q|(q.exists(),q.info.nonce,q.info.balance,q.info.code_hash)),"{ctx}: account {i}");
        for k in 0..12u64{assert_eq!(a.slot(&x,&U256::from(k)),b.slot(&x,&U256::from(k)),"{ctx}: slot {i}/{k}")}
        let (mut s1,mut s2)=(Vec::new(),Vec::new());a.each_slot(&x,&mut |k,v|{if *v!=U256::ZERO{s1.push((*k,*v))}});b.each_slot(&x,&mut |k,v|{if *v!=U256::ZERO{s2.push((*k,*v))}});
        s1.sort();s2.sort();assert_eq!(s1,s2,"{ctx}: nonzero slots of {i}");
    }
    for n in 0..5u64{assert_eq!(a.block_hash(n),b.block_hash(n),"{ctx}: block hash {n}")}
}
fn same_state(a:&InMemoryDB,b:&InMemoryDB,ctx:&str){
    assert_eq!(a.accounts.len(),b.accounts.len(),"{ctx}: accounts");
    for (k,v) in &b.accounts{let u=&a.accounts[k];assert_eq!((&u.info.nonce,&u.info.balance,&u.info.code_hash,&u.account_state,&u.storage),(&v.info.nonce,&v.info.balance,&v.info.code_hash,&v.account_state,&v.storage),"{ctx}: account {k}");}
    let (mut x,mut y):(Vec<_>,Vec<_>)=(a.contracts.keys().collect(),b.contracts.keys().collect());x.sort();y.sort();assert_eq!(x,y,"{ctx}: contracts");
    assert_eq!(a.block_hashes,b.block_hashes,"{ctx}: block hashes");
}

#[test]
fn an_overlay_reads_and_merges_as_the_copy_it_replaces(){
    let mut r=Rng(0x0e3);
    for case in 0..3000{
        // A parent: a random normalized state (some accounts with storage, some not existing).
        let mut parent=InMemoryDB::default();
        for _ in 0..r.below(40){let c=changes(&mut r,&parent);crate::user_phase::commit_tx(&mut parent,c,r.below(2)==0);}
        parent.block_hashes.insert(U256::from(1),B256::from([1;32]));
        let (mut copy,mut over)=(parent.clone(),Overlay::new(&parent));
        for step in 0..1+r.below(12){
            match r.below(7){
                0=>{let a=addr(&mut r);let i=AccountInfo{balance:U256::from(r.below(9)),nonce:r.below(3),code_hash:KECCAK_EMPTY,code:code(&mut r)};
                    copy.insert_info(a,i.clone());over.insert_info(a,i);}
                1=>{let a=addr(&mut r);if let (Some(x),Some(y))=(copy.info_mut(&a),over.info_mut(&a)){x.balance+=U256::from(5);y.balance+=U256::from(5);}else{assert_eq!(copy.account(&a).is_some(),over.account(&a).is_some());}}
                2=>{let h=[(0,B256::from([7;32])),(3,B256::from([9;32]))];copy.set_block_hashes(&h);over.set_block_hashes(&h);}
                _=>{let c=changes(&mut r,&copy);crate::user_phase::commit_tx_on(&mut copy,c.clone(),true);crate::user_phase::commit_tx_on(&mut over,c,true);}
            }
            same_view(&copy,&over,&format!("case {case} step {step}"));
        }
        let merged=over.merge();same_state(&merged,&copy,&format!("case {case} merged"));
        // The write set names every account and slot that changed: an account the reference reads whole is
        // whole in it, and every changed slot of another account is listed (or its account is whole).
        let full=crate::state_trie::Touched::between(&parent,&copy);let t=over.touched();
        let written:HashMap<Address,Option<Vec<U256>>>=t.iter().map(|(a,s)|(*a,s.map(|v|v.to_vec()))).collect();
        for (a,slots) in full.iter(){
            let w=written.get(a).unwrap_or_else(||panic!("case {case}: {a} changed but not written"));
            match (slots,w){
                (_,None)=>{}
                (None,Some(_))=>panic!("case {case}: {a} changed whole but is written by slots"),
                (Some(ks),Some(ws))=>for k in ks{assert!(ws.contains(k),"case {case}: slot {k} of {a} changed but not written")},
            }
        }
    }
}

/// D-7 (A13 store mode): a layer whose entries keep their code hash and not their code reads, checks
/// code and merges as the layer that holds the copies (the pool's pending state, `drop_code_copies`).
#[test]
fn a_layer_without_its_code_copies_reads_and_merges_as_before(){
    fn code_of(v:&dyn View,a:&Address)->Option<(Option<Vec<u8>>,Result<u64,String>)>{v.account(a).map(|x|(x.info.code.as_ref().map(|c|c.original_bytes().to_vec()),crate::code_caps::code_len(v,&x.info)))}
    fn each(v:&dyn View)->Vec<(Address,Option<Vec<u8>>)>{let mut m=Vec::new();v.each_account(&mut |a,x|m.push((*a,x.info.code.map(|c|c.original_bytes().to_vec()))));m.sort();m}
    let mut r=Rng(0xc0de);let mut dropped=0;
    let all:Vec<Address>=(0..30u64).map(|i|Address::from_word(B256::from(U256::from(i)))).collect();
    for case in 0..2000{
        let mut parent=InMemoryDB::default();
        for _ in 0..r.below(40){let c=changes(&mut r,&parent);crate::user_phase::commit_tx(&mut parent,c,r.below(2)==0);}
        let mut over=Overlay::new(&parent);
        for _ in 0..1+r.below(12){
            if r.below(5)==0{let a=addr(&mut r);over.insert_info(a,AccountInfo{balance:U256::from(r.below(9)),nonce:r.below(3),code_hash:KECCAK_EMPTY,code:code(&mut r)});}
            else{let c=changes(&mut r,&over);crate::user_phase::commit_tx_on(&mut over,c,true);}
        }
        let held=over.into_layer();let mut shed=held.clone();
        dropped+=shed.code_copies();shed.drop_code_copies(&all);assert_eq!(shed.code_copies(),0,"case {case}: no entry keeps a copy");
        let (x,y)=(LayerView{base:&parent,layer:&held},LayerView{base:&parent,layer:&shed});
        same_view(&x,&y,&format!("case {case}"));
        for a in &all{assert_eq!(code_of(&x,a),code_of(&y,a),"case {case}: code of {a}");}
        assert_eq!(each(&x),each(&y),"case {case}: each_account");
        let (m1,m2)=(Overlay::from_layer(&parent,held).merge(),Overlay::from_layer(&parent,shed).merge());same_state(&m1,&m2,&format!("case {case} merged"));
        for (a,u) in &m1.accounts{assert_eq!(u.info.code.as_ref().map(|c|c.original_bytes()),m2.accounts[a].info.code.as_ref().map(|c|c.original_bytes()),"case {case}: merged code of {a}");}
    }
    assert!(dropped>1000,"the cases hold code copies to drop ({dropped})");
}
