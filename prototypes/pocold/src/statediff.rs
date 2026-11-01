//! C-1 S3a (proposals/C1-STORAGE-DESIGN.md "S3 refinement"): a block's state as a diff from its parent's.
//! A `StateDiff` holds, for every part of an `InMemoryDB` that differs between a parent state and its
//! child, the parent's value ("before") and the child's ("after"); absence is a value too (None). So
//! `apply_before` turns the child state back into the parent state exactly, and `apply_after` the
//! parent into the child, field by field: accounts (info with its cached code, account state),
//! each storage slot, contracts, block hashes and logs.
//!
//! Exactness is the invariant (I2): `states_equal` compares every field, including what
//! `AccountInfo`'s own PartialEq ignores (the cached code), and the tests check
//! apply_before(child, between(parent, child)) == parent and apply_after(parent, ..) == child.
use revm::db::{AccountState,DbAccount};
use revm::primitives::{AccountInfo,Address,Bytecode,Log,B256,U256};
use revm::InMemoryDB;

/// An account's fields other than its storage.
#[derive(Clone,Debug)]
pub struct AccountMeta{pub info:AccountInfo,pub state:AccountState}
#[derive(Clone,Debug,Default)]
pub struct AccountDiff{
    /// None: the account is absent on that side.
    pub before:Option<AccountMeta>,pub after:Option<AccountMeta>,
    /// Every slot whose presence or value differs: (slot, before, after).
    pub slots:Vec<(U256,Option<U256>,Option<U256>)>,
}
#[derive(Clone,Debug,Default)]
pub struct StateDiff{
    pub accounts:Vec<(Address,AccountDiff)>,
    pub contracts:Vec<(B256,Option<Bytecode>,Option<Bytecode>)>,
    pub block_hashes:Vec<(U256,Option<B256>,Option<B256>)>,
    /// The whole log list on each side, only when it differs.
    pub logs:Option<(Vec<Log>,Vec<Log>)>,
}

/// Same code: the shared buffer (a state copy shares it, S2) or equal bytes.
fn same_code(a:&Bytecode,b:&Bytecode)->bool{
    let (x,y)=(a.original_byte_slice(),b.original_byte_slice());
    (x.as_ptr()==y.as_ptr()&&x.len()==y.len()&&std::mem::discriminant(a)==std::mem::discriminant(b))||a==b
}
fn same_info(a:&AccountInfo,b:&AccountInfo)->bool{
    a==b&&match (&a.code,&b.code){(None,None)=>true,(Some(x),Some(y))=>same_code(x,y),_=>false}
}
fn same_meta(a:&DbAccount,b:&DbAccount)->bool{same_info(&a.info,&b.info)&&a.account_state==b.account_state}
fn meta(a:&DbAccount)->AccountMeta{AccountMeta{info:a.info.clone(),state:a.account_state.clone()}}

/// Every field of two states, exactly (the reference for the S3a tests).
pub fn states_equal(a:&InMemoryDB,b:&InMemoryDB)->bool{
    a.accounts.len()==b.accounts.len()&&a.accounts.iter().all(|(k,x)|b.accounts.get(k).map_or(false,|y|same_meta(x,y)&&x.storage==y.storage))
        &&a.contracts.len()==b.contracts.len()&&a.contracts.iter().all(|(k,x)|b.contracts.get(k).map_or(false,|y|same_code(x,y)))
        &&a.block_hashes==b.block_hashes&&a.logs==b.logs
}

impl StateDiff{
    /// C-1 E1: adds the accounts and slots this diff changes to `t` (an account that appears or disappears
    /// is read whole), so that a trie can be moved across it.
    pub fn touch(&self,t:&mut pocol_lp4_execution::state_trie::Touched){
        let exists=|m:&Option<AccountMeta>|m.as_ref().map_or(false,|m|!matches!(m.state,AccountState::NotExisting));
        for (a,d) in &self.accounts{
            if exists(&d.before)!=exists(&d.after){t.whole_account(*a);continue}
            t.account(*a);for (k,_,_) in &d.slots{t.slot(*a,*k)}
        }
    }
    /// The diff that turns `parent` into `child`.
    pub fn between(parent:&InMemoryDB,child:&InMemoryDB)->Self{
        let mut d=StateDiff::default();
        for (addr,c) in &child.accounts{
            match parent.accounts.get(addr){
                None=>d.accounts.push((*addr,AccountDiff{before:None,after:Some(meta(c)),slots:c.storage.iter().map(|(k,v)|(*k,None,Some(*v))).collect()})),
                Some(p)=>{
                    let mut slots:Vec<_>=c.storage.iter().filter_map(|(k,v)|match p.storage.get(k){Some(pv) if pv==v=>None,pv=>Some((*k,pv.copied(),Some(*v)))}).collect();
                    slots.extend(p.storage.iter().filter(|(k,_)|!c.storage.contains_key(*k)).map(|(k,v)|(*k,Some(*v),None)));
                    let m=!same_meta(p,c);
                    if m||!slots.is_empty(){d.accounts.push((*addr,AccountDiff{before:Some(meta(p)),after:Some(meta(c)),slots}));}
                }
            }
        }
        for (addr,p) in &parent.accounts{if !child.accounts.contains_key(addr){
            d.accounts.push((*addr,AccountDiff{before:Some(meta(p)),after:None,slots:p.storage.iter().map(|(k,v)|(*k,Some(*v),None)).collect()}));}}
        for (h,c) in &child.contracts{match parent.contracts.get(h){Some(p) if same_code(p,c)=>{},p=>d.contracts.push((*h,p.cloned(),Some(c.clone())))}}
        for (h,p) in &parent.contracts{if !child.contracts.contains_key(h){d.contracts.push((*h,Some(p.clone()),None));}}
        for (n,c) in &child.block_hashes{match parent.block_hashes.get(n){Some(p) if p==c=>{},p=>d.block_hashes.push((*n,p.copied(),Some(*c)))}}
        for (n,p) in &parent.block_hashes{if !child.block_hashes.contains_key(n){d.block_hashes.push((*n,Some(*p),None));}}
        if parent.logs!=child.logs{d.logs=Some((parent.logs.clone(),child.logs.clone()));}
        d
    }
    /// C-1 E3: `between` for a child that differs from its parent only at the accounts in `written`
    /// (the block's write set): the same diff, from those accounts' entries (storage compared whole, so
    /// that a slot dropped by normalization is recorded too), and from the contracts, block hashes and
    /// logs, which are compared whole as before.
    pub fn between_written(parent:&InMemoryDB,child:&InMemoryDB,written:&pocol_lp4_execution::state_trie::Touched)->Self{
        let mut d=StateDiff::default();
        for (addr,_) in written.iter(){
            match (parent.accounts.get(addr),child.accounts.get(addr)){
                (None,None)=>{}
                (None,Some(c))=>d.accounts.push((*addr,AccountDiff{before:None,after:Some(meta(c)),slots:c.storage.iter().map(|(k,v)|(*k,None,Some(*v))).collect()})),
                (Some(p),None)=>d.accounts.push((*addr,AccountDiff{before:Some(meta(p)),after:None,slots:p.storage.iter().map(|(k,v)|(*k,Some(*v),None)).collect()})),
                (Some(p),Some(c))=>{
                    let mut slots:Vec<_>=c.storage.iter().filter_map(|(k,v)|match p.storage.get(k){Some(pv) if pv==v=>None,pv=>Some((*k,pv.copied(),Some(*v)))}).collect();
                    slots.extend(p.storage.iter().filter(|(k,_)|!c.storage.contains_key(*k)).map(|(k,v)|(*k,Some(*v),None)));
                    if !same_meta(p,c)||!slots.is_empty(){d.accounts.push((*addr,AccountDiff{before:Some(meta(p)),after:Some(meta(c)),slots}));}
                }
            }
        }
        for (h,c) in &child.contracts{match parent.contracts.get(h){Some(p) if same_code(p,c)=>{},p=>d.contracts.push((*h,p.cloned(),Some(c.clone())))}}
        for (h,p) in &parent.contracts{if !child.contracts.contains_key(h){d.contracts.push((*h,Some(p.clone()),None));}}
        for (n,c) in &child.block_hashes{match parent.block_hashes.get(n){Some(p) if p==c=>{},p=>d.block_hashes.push((*n,p.copied(),Some(*c)))}}
        for (n,p) in &parent.block_hashes{if !child.block_hashes.contains_key(n){d.block_hashes.push((*n,Some(*p),None));}}
        if parent.logs!=child.logs{d.logs=Some((parent.logs.clone(),child.logs.clone()));}
        d
    }
    /// The diff of a block's output from its parent: from the write set when there is one (checked against
    /// `between` under lp4's reference check, by applying both to the parent).
    pub fn of_output(parent:&InMemoryDB,out:&pocol_lp4_execution::apply::BlockOutput)->Self{
        let Some(w)=&out.writes else{return Self::between(parent,&out.state)};
        let d=Self::between_written(parent,&out.state,w);
        if pocol_lp4_execution::apply::root_check(){
            let mut s=parent.clone();d.apply_after(&mut s);assert!(states_equal(&s,&out.state),"C-1 E3: the diff from the write set does not give the child");
            let mut b=out.state.clone();d.apply_before(&mut b);assert!(states_equal(&b,parent),"C-1 E3: the diff from the write set does not give the parent back");
        }
        d
    }
    /// child -> parent.
    pub fn apply_before(&self,s:&mut InMemoryDB){self.apply(s,true)}
    /// parent -> child.
    pub fn apply_after(&self,s:&mut InMemoryDB){self.apply(s,false)}
    fn apply(&self,s:&mut InMemoryDB,back:bool){
        fn pick<T:Clone>(back:bool,b:&Option<T>,a:&Option<T>)->Option<T>{if back{b.clone()}else{a.clone()}}
        for (addr,d) in &self.accounts{
            match pick(back,&d.before,&d.after){
                None=>{s.accounts.remove(addr);}
                Some(m)=>{
                    // A side that is absent has no storage: start from an empty account.
                    let other=if back{&d.after}else{&d.before};
                    let a=s.accounts.entry(*addr).or_default();
                    if other.is_none(){a.storage.clear();}
                    a.info=m.info;a.account_state=m.state;
                    for (k,b,af) in &d.slots{match if back{b}else{af}{Some(v)=>{a.storage.insert(*k,*v);}None=>{a.storage.remove(k);}}}
                }
            }
        }
        for (h,b,a) in &self.contracts{match pick(back,b,a){Some(c)=>{s.contracts.insert(*h,c);}None=>{s.contracts.remove(h);}}}
        for (n,b,a) in &self.block_hashes{match pick(back,b,a){Some(x)=>{s.block_hashes.insert(*n,x);}None=>{s.block_hashes.remove(n);}}}
        if let Some((b,a))=&self.logs{s.logs=if back{b.clone()}else{a.clone()};}
    }
    /// Changed accounts, changed slots (for accounting and status).
    pub fn size(&self)->(usize,usize){(self.accounts.len(),self.accounts.iter().map(|x|x.1.slots.len()).sum())}
    /// Heap bytes held, by its representation (each element's inline size; code buffers are shared
    /// with the states, S2, and counted by the code term).
    pub fn heap_bytes(&self)->usize{
        use std::mem::size_of;
        size_of::<(Address,AccountDiff)>()*self.accounts.capacity()
            +self.accounts.iter().map(|x|x.1.slots.capacity()*size_of::<(U256,Option<U256>,Option<U256>)>()).sum::<usize>()
            +self.contracts.capacity()*size_of::<(B256,Option<Bytecode>,Option<Bytecode>)>()
            +self.block_hashes.capacity()*size_of::<(U256,Option<B256>,Option<B256>)>()
            +self.logs.as_ref().map_or(0,|(b,a)|(b.len()+a.len())*size_of::<Log>())
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use revm::primitives::{Bytes,keccak256};
    /// A small deterministic generator (no external crate).
    struct G(u64);
    impl G{fn n(&mut self)->u64{self.0^=self.0<<13;self.0^=self.0>>7;self.0^=self.0<<17;self.0}fn pick(&mut self,k:u64)->u64{self.n()%k}}
    fn code(g:&mut G)->Bytecode{Bytecode::new_raw(Bytes::from(vec![0x60,(g.pick(250)) as u8,0x00]))}
    fn random_state(g:&mut G)->InMemoryDB{
        let mut s=InMemoryDB::default();
        for i in 0..40u64{let a=Address::with_last_byte(i as u8);let mut acc=DbAccount::default();acc.info.balance=U256::from(g.pick(5));acc.info.nonce=g.pick(3);
            if g.pick(4)==0{let c=code(g);acc.info.code_hash=keccak256(c.original_byte_slice());if g.pick(2)==0{acc.info.code=Some(c.clone());}s.contracts.insert(acc.info.code_hash,c);}
            acc.account_state=[AccountState::None,AccountState::Touched,AccountState::StorageCleared,AccountState::NotExisting][g.pick(4) as usize].clone();
            for _ in 0..g.pick(6){acc.storage.insert(U256::from(g.pick(10)),U256::from(g.pick(3)));}
            if g.pick(5)!=0{s.accounts.insert(a,acc);}}
        for n in 0..g.pick(5){s.block_hashes.insert(U256::from(n),B256::with_last_byte(g.pick(3) as u8));}
        s
    }
    /// A child: the parent with random edits (as one block's writes), sharing the parent's code buffers.
    fn mutate(p:&InMemoryDB,g:&mut G)->InMemoryDB{
        let mut c=p.clone();
        for _ in 0..g.pick(12){let a=Address::with_last_byte(g.pick(45) as u8);
            match g.pick(6){
                0=>{c.accounts.remove(&a);}
                1=>{let x=c.accounts.entry(a).or_default();x.storage.insert(U256::from(g.pick(10)),U256::from(g.pick(3)));}
                2=>{if let Some(x)=c.accounts.get_mut(&a){let k=U256::from(g.pick(10));x.storage.remove(&k);}}
                3=>{let x=c.accounts.entry(a).or_default();x.info.balance=U256::from(g.pick(5));x.account_state=AccountState::Touched;}
                4=>{if let Some(x)=c.accounts.get_mut(&a){x.info.code=None;x.storage.clear();x.account_state=AccountState::StorageCleared;}}
                _=>{let k=code(g);let h=keccak256(k.original_byte_slice());c.contracts.insert(h,k);}
            }}
        if g.pick(3)==0{let k=*c.contracts.keys().next().unwrap_or(&B256::ZERO);c.contracts.remove(&k);}
        c.block_hashes.clear();for n in 0..g.pick(5){c.block_hashes.insert(U256::from(n+1),B256::with_last_byte(g.pick(3) as u8));}
        c
    }
    #[test] fn a_diff_turns_each_state_into_the_other_exactly(){
        let mut g=G(0x9e3779b97f4a7c15);let mut changed=0;
        for _ in 0..2000{
            let p=random_state(&mut g);let c=mutate(&p,&mut g);let d=StateDiff::between(&p,&c);
            let mut back=c.clone();d.apply_before(&mut back);assert!(states_equal(&back,&p),"child -> parent");
            let mut fwd=p.clone();d.apply_after(&mut fwd);assert!(states_equal(&fwd,&c),"parent -> child");
            if !states_equal(&p,&c){changed+=1;assert!(d.size().0+d.contracts.len()+d.block_hashes.len()>0);}
            else{assert!(d.accounts.is_empty()&&d.contracts.is_empty());}
        }
        assert!(changed>1500,"positive control: most pairs differ ({changed})");
    }
    /// A chain of diffs composes: a state 8 blocks back is reached from the tip through the diffs.
    #[test] fn diffs_compose_along_a_chain(){
        let mut g=G(42);let mut states=vec![random_state(&mut g)];
        for _ in 0..8{let n=mutate(states.last().unwrap(),&mut g);states.push(n);}
        let diffs:Vec<StateDiff>=states.windows(2).map(|w|StateDiff::between(&w[0],&w[1])).collect();
        for k in 0..states.len(){let mut s=states.last().unwrap().clone();for d in diffs[k..].iter().rev(){d.apply_before(&mut s);}assert!(states_equal(&s,&states[k]),"back to {k}");}
        let mut s=states[0].clone();for d in &diffs{d.apply_after(&mut s);}assert!(states_equal(&s,states.last().unwrap()));
    }
    /// The comparator sees what AccountInfo's PartialEq ignores (the cached code).
    #[test] fn states_equal_sees_the_cached_code(){
        let mut g=G(7);let s=random_state(&mut g);let mut t=s.clone();assert!(states_equal(&s,&t));
        let a=*t.accounts.keys().next().unwrap();t.accounts.get_mut(&a).unwrap().info.code=Some(code(&mut g));
        assert!(!states_equal(&s,&t));let d=StateDiff::between(&s,&t);let mut b=t.clone();d.apply_before(&mut b);assert!(states_equal(&b,&s));
    }
}
