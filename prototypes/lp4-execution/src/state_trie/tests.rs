use super::*;
use revm::primitives::{AccountInfo,Bytecode,Bytes};

struct Rng(u64);
impl Rng{fn next(&mut self)->u64{self.0^=self.0<<13;self.0^=self.0>>7;self.0^=self.0<<17;self.0}fn below(&mut self,n:u64)->u64{self.next()%n}}

/// p1_trie's root of raw 32-byte keys holding nonzero integers: the reference for the generic trie.
fn reference(rows:&std::collections::BTreeMap<Key,U256>)->B256{
    let entries:Vec<(Vec<u8>,Vec<u8>)>=rows.iter().map(|(k,v)|{let mut e=Vec::new();rlp_uint(v,&mut e);(k.to_vec(),e)}).collect();
    B256::from(crate::trie::root(&entries,entries.len()).unwrap())
}
/// Keys drawn to share long prefixes, so that extensions above branches, and their splits and merges on
/// insertion and deletion, occur at every depth.
fn clustered_key(r:&mut Rng)->Key{
    let mut k=[0u8;32];let shared=r.below(65) as usize;
    for i in 0..64{let n=if i<shared{(i%3) as u8}else{r.below(16) as u8};if i%2==0{k[i/2]|=n<<4}else{k[i/2]|=n}}
    k
}

#[test]
fn raw_keys_build_and_update_give_p1_roots(){
    let mut r=Rng(0x9e3779b97f4a7c15);
    for round in 0..300{
        let mut model=std::collections::BTreeMap::new();let mut t:Option<Arc<Node<U256>>>=None;
        assert_eq!(root_of(&t),reference(&model));
        let steps=1+r.below(12);
        for _ in 0..steps{
            let mut ch:Vec<(Key,Option<U256>)>=Vec::new();
            for _ in 0..1+r.below(if round%2==0{4}else{40}){
                let k=if !model.is_empty()&&r.below(3)==0{*model.keys().nth(r.below(model.len() as u64) as usize).unwrap()}else{clustered_key(&mut r)};
                if ch.iter().any(|(x,_)|*x==k){continue}
                let small=r.below(2)==0;let v=if r.below(3)==0{None}else{Some(U256::from(1+r.below(if small{3}else{u64::MAX})))};
                ch.push((k,v));
            }
            ch.sort_by(|a,b|a.0.cmp(&b.0));
            for (k,v) in &ch{match v{Some(v)=>{model.insert(*k,*v);}None=>{model.remove(k);}}}
            t=update(t.as_ref(),0,&ch);
            assert_eq!(root_of(&t),reference(&model),"round {round}");
            let rows:Vec<(Key,U256)>=model.iter().map(|(k,v)|(*k,*v)).collect();
            assert_eq!(root_of(&build(0,&rows)),reference(&model));
            for (k,v) in &model{assert_eq!(get(t.as_deref().unwrap(),k),Some(v))}
        }
    }
}

#[test]
fn every_two_key_shape_matches(){
    // Two keys that agree on exactly n nibbles, for every n: the branch sits at n under an extension of n
    // nibbles; deleting either key leaves a single leaf at the root.
    for n in 0..64{
        let a=[0x11u8;32];let mut b=a;let i=n/2;if n%2==0{b[i]^=0x40}else{b[i]^=0x04}
        let mut m=std::collections::BTreeMap::new();m.insert(a,U256::from(7));m.insert(b,U256::from(9));
        let t=build(0,&m.iter().map(|(k,v)|(*k,*v)).collect::<Vec<_>>());assert_eq!(root_of(&t),reference(&m),"n {n}");
        let t2=update(t.as_ref(),0,&[(b,None)]);m.remove(&b);assert_eq!(root_of(&t2),reference(&m));
    }
}

fn info(r:&mut Rng,codes:&[Bytecode])->AccountInfo{
    let code=codes[r.below(codes.len() as u64) as usize].clone();
    AccountInfo{balance:U256::from(r.below(1<<40)),nonce:r.below(5),code_hash:code.hash_slow(),code:Some(code)}
}
fn random_state(r:&mut Rng,n:u64,codes:&[Bytecode])->InMemoryDB{
    let mut db=InMemoryDB::default();
    for i in 0..n{
        let a=Address::from_word(B256::from(U256::from(r.next()^i)));db.insert_account_info(a,info(r,codes));
        for _ in 0..r.below(6){db.insert_account_storage(a,U256::from(r.below(50)),U256::from(r.below(4))).unwrap();}
    }
    db
}
/// One block's worth of random changes: new and removed accounts, balance and nonce changes, slots
/// written, zeroed and cleared, an account marked not existing.
fn mutate(r:&mut Rng,db:&InMemoryDB,codes:&[Bytecode])->InMemoryDB{
    let mut d=db.clone();let addrs:Vec<Address>=d.accounts.keys().copied().collect();
    for _ in 0..1+r.below(20){
        let a=if addrs.is_empty()||r.below(4)==0{Address::from_word(B256::from(U256::from(r.next())))}else{addrs[r.below(addrs.len() as u64) as usize]};
        match r.below(7){
            0=>{d.accounts.remove(&a);}
            1=>{if let Some(x)=d.accounts.get_mut(&a){x.account_state=AccountState::NotExisting}}
            2=>{if let Some(x)=d.accounts.get_mut(&a){x.storage.clear();x.account_state=AccountState::StorageCleared}}
            3=>d.insert_account_info(a,info(r,codes)),
            _=>{if !d.accounts.contains_key(&a){d.insert_account_info(a,info(r,codes))}
                for _ in 0..1+r.below(4){d.insert_account_storage(a,U256::from(r.below(60)),U256::from(r.below(3))).unwrap();}}
        }
    }
    d
}

#[test]
fn state_roots_equal_world_root_through_random_blocks(){
    let codes=[Bytecode::default(),Bytecode::new_raw(Bytes::from_static(&[0x60,1,0x60,0,0x55])),Bytecode::new_raw(Bytes::from_static(&[0x00]))];
    let mut r=Rng(0xdeadbeefcafef00d);
    for case in 0..20{
        let mut db=random_state(&mut r,[0,1,2,40,300][case%5],&codes);
        let mut t=StateTrie::build(&db);
        assert_eq!(t.root(),crate::genesis_builder::world_root(&db).unwrap(),"case {case} build");
        for step in 0..30{
            let next=mutate(&mut r,&db,&codes);let touched=Touched::between(&db,&next);
            let u=t.update(&next,&touched);
            assert_eq!(u.root(),crate::genesis_builder::world_root(&next).unwrap(),"case {case} step {step}");
            assert_eq!(u.root(),StateTrie::build(&next).root());
            // The old version is unchanged by the update (shared nodes are never written).
            assert_eq!(t.root(),crate::genesis_builder::world_root(&db).unwrap());
            t=u;db=next;
        }
        assert!(Touched::between(&db,&db).is_empty());
    }
}

#[test]
fn an_incomplete_touched_set_gives_another_root(){
    // The update trusts its touched set: the callers' completeness matters, and this is the control that
    // a missed slot is visible in the root (pocold checks a derived trie against the committed root).
    let codes=[Bytecode::default()];let mut r=Rng(7);
    let db=random_state(&mut r,50,&codes);let t=StateTrie::build(&db);
    let mut next=db.clone();let a=*next.accounts.keys().next().unwrap();next.insert_account_storage(a,U256::from(999),U256::from(5)).unwrap();
    let full=Touched::between(&db,&next);assert_eq!(full.len(),2);
    assert_eq!(t.update(&next,&full).root(),crate::genesis_builder::world_root(&next).unwrap());
    let mut partial=Touched::default();partial.account(a);
    assert_ne!(t.update(&next,&partial).root(),crate::genesis_builder::world_root(&next).unwrap());
    let mut whole=Touched::default();whole.whole_account(a);
    assert_eq!(t.update(&next,&whole).root(),crate::genesis_builder::world_root(&next).unwrap());
}

#[test]
fn stats_count_every_node(){
    let codes=[Bytecode::default()];let mut r=Rng(11);let db=random_state(&mut r,200,&codes);
    let s=StateTrie::build(&db).stats();
    let slots:u64=db.accounts.values().map(|a|a.storage.values().filter(|v|**v!=U256::ZERO).count() as u64).sum();
    assert_eq!(s.leaves,200+slots);
    assert!(s.branches<s.leaves&&s.kid_slots==s.leaves+s.branches-1-db.accounts.values().filter(|a|a.storage.values().any(|v|*v!=U256::ZERO)).count() as u64);
}

#[test]
fn a_code_hash_change_alone_is_touched(){
    // Not reachable under Cancun (a CREATE also sets the nonce), but the touched set must not depend on it.
    let mut db=InMemoryDB::default();let a=Address::from([3;20]);
    db.insert_account_info(a,AccountInfo{balance:U256::from(5),nonce:1,code_hash:B256::from([1;32]),code:None});
    let t=StateTrie::build(&db);let mut next=db.clone();next.accounts.get_mut(&a).unwrap().info.code_hash=B256::from([2;32]);
    let touched=Touched::between(&db,&next);assert_eq!(touched.len(),1);
    assert_eq!(t.update(&next,&touched).root(),crate::genesis_builder::world_root(&next).unwrap());
}
