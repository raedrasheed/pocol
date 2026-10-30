//! C-1 E3: the state the executor reads and writes, whole (`InMemoryDB`) or as an overlay over a parent.
//!
//! `View` is every read the executor makes of its state outside the engine, and `StateRef` serves the
//! engine through it. `InMemoryDB` answers each read exactly as the direct map reads did; `Overlay` holds
//! only what a block wrote on top of a borrowed parent and answers the rest from the parent, as the
//! parent's copy would. `WorkState` is every write the executor makes; for `InMemoryDB` each is the code
//! it replaces, for `Overlay` the same operation recorded on top of the parent. `Overlay::merge` gives
//! the state the copy would have reached, and `Overlay::touched` the accounts and slots it wrote.
use std::collections::{HashMap,HashSet};
use revm::{db::{AccountState,DbAccount},primitives::{keccak256,Account,AccountInfo,Address,Bytecode,B256,U256,KECCAK_EMPTY},InMemoryDB};

/// An account as a whole state holds it (C-1 E4: owned, so a state read from a store need not lend
/// references into its cache; code is shared, so a copy is cheap).
#[derive(Clone)]
pub struct Acc{pub info:AccountInfo,pub state:AccountState}
impl Acc{
    pub fn exists(&self)->bool{!matches!(self.state,AccountState::NotExisting)}
    /// `DbAccount::info`: the info of an existing account.
    pub fn info(&self)->Option<AccountInfo>{self.exists().then(||self.info.clone())}
}

/// The executor's reads of a state.
pub trait View{
    fn account(&self,a:&Address)->Option<Acc>;
    /// A slot's stored value (zero when the account or the slot is absent).
    fn slot(&self,a:&Address,k:&U256)->U256;
    /// Every stored slot of the account (stored zeros included), in no order.
    fn each_slot(&self,a:&Address,f:&mut dyn FnMut(&U256,&U256));
    fn contract(&self,h:&B256)->Option<Bytecode>;
    /// BLOCKHASH as the engine reads it (`CacheDB::block_hash_ref` over `EmptyDB`).
    fn block_hash(&self,n:u64)->B256;
    /// Every account the state holds, in no order.
    fn each_account(&self,f:&mut dyn FnMut(&Address,Acc));
}
fn empty_block_hash(n:u64)->B256{keccak256(n.to_string().as_bytes())}
impl View for InMemoryDB{
    fn account(&self,a:&Address)->Option<Acc>{self.accounts.get(a).map(|x|Acc{info:x.info.clone(),state:x.account_state.clone()})}
    fn slot(&self,a:&Address,k:&U256)->U256{self.accounts.get(a).and_then(|x|x.storage.get(k).copied()).unwrap_or_default()}
    fn each_slot(&self,a:&Address,f:&mut dyn FnMut(&U256,&U256)){if let Some(x)=self.accounts.get(a){for (k,v) in &x.storage{f(k,v)}}}
    fn contract(&self,h:&B256)->Option<Bytecode>{self.contracts.get(h).cloned()}
    fn block_hash(&self,n:u64)->B256{self.block_hashes.get(&U256::from(n)).copied().unwrap_or_else(||empty_block_hash(n))}
    fn each_account(&self,f:&mut dyn FnMut(&Address,Acc)){for (a,x) in &self.accounts{f(a,Acc{info:x.info.clone(),state:x.account_state.clone()})}}
}

/// A shared state reads as the state it holds (pocold keeps entry states in an Arc).
impl<T:View+?Sized> View for std::sync::Arc<T>{
    fn account(&self,a:&Address)->Option<Acc>{(**self).account(a)}
    fn slot(&self,a:&Address,k:&U256)->U256{(**self).slot(a,k)}
    fn each_slot(&self,a:&Address,f:&mut dyn FnMut(&U256,&U256)){(**self).each_slot(a,f)}
    fn contract(&self,h:&B256)->Option<Bytecode>{(**self).contract(h)}
    fn block_hash(&self,n:u64)->B256{(**self).block_hash(n)}
    fn each_account(&self,f:&mut dyn FnMut(&Address,Acc)){(**self).each_account(f)}
}

/// The executor's writes to a state.
pub trait WorkState:View{
    /// revm's `DatabaseCommit::commit` of a transaction's or a call's state.
    fn commit(&mut self,changes:HashMap<Address,Account>);
    /// A removed account (EIP-161 or self-destruct): not existing, no storage, default info.
    fn remove(&mut self,a:Address);
    /// `user_phase::normalize` of the touched accounts.
    fn normalize(&mut self,touched:&[Address]);
    /// `CacheDB::insert_account_info`.
    fn insert_info(&mut self,a:Address,info:AccountInfo);
    /// The info of an account the state holds, to change in place.
    fn info_mut(&mut self,a:&Address)->Option<&mut AccountInfo>;
    /// The BLOCKHASH window replaced (`block_hashes` cleared, then these inserted).
    fn set_block_hashes(&mut self,hashes:&[(u64,B256)]);
    fn as_view(&self)->&dyn View;
}
impl WorkState for InMemoryDB{
    fn commit(&mut self,changes:HashMap<Address,Account>){revm::DatabaseCommit::commit(self,changes)}
    fn remove(&mut self,a:Address){let x=self.accounts.entry(a).or_default();x.account_state=AccountState::NotExisting;x.storage.clear();x.info=Default::default();}
    fn normalize(&mut self,touched:&[Address]){crate::user_phase::normalize(self,touched)}
    fn insert_info(&mut self,a:Address,info:AccountInfo){self.insert_account_info(a,info)}
    fn info_mut(&mut self,a:&Address)->Option<&mut AccountInfo>{self.accounts.get_mut(a).map(|x|&mut x.info)}
    fn set_block_hashes(&mut self,hashes:&[(u64,B256)]){self.block_hashes.clear();for (n,h) in hashes{self.block_hashes.insert(U256::from(*n),*h);}}
    fn as_view(&self)->&dyn View{self}
}

/// What a block wrote on top of `base`, which it borrows and never changes.
///
/// The writes are a `Layer`:
/// - `top` holds an entry for every account written: its info and state as the parent's copy would have
///   them, and only the slots written;
/// - `cleared` are the accounts whose storage was cleared here (created, self-destructed or removed):
///   their unwritten slots read zero, not the parent's;
/// - the full-state code normalizes after every commit: it deletes a removed account's entry and drops
///   zero slots. Here a deleted account stays a tombstone in `deleted` (it reads as absent, and a later
///   write starts a fresh entry, as the copy's `or_default` does), and the accounts in `retain` have
///   their zero slots dropped by `merge`.
///
/// The executor normalizes every commit it makes on an overlay (`commit_tx_on(.., true)`); an
/// unnormalized commit sequence is not modelled. The BLOCKHASH window, once set, replaces the parent's.
///
/// C-1 E4: the base is any `View` (`B`, an `InMemoryDB` unless named): a state read from its root in the
/// store as well as a whole one. Only `merge` needs a whole base.
pub struct Overlay<'a,B:View+?Sized=InMemoryDB>{pub base:&'a B,l:Layer}
/// An overlay's writes without the parent it lies over, to keep between uses (the pool's pending state
/// over the head). It is only meaningful over the state it was written on.
#[derive(Clone,Default)]
pub struct Layer{top:InMemoryDB,cleared:HashSet<Address>,deleted:HashSet<Address>,retain:HashSet<Address>,hashes_set:bool}
/// A layer read over its base, without changing either (the pool's executions and checks).
pub struct LayerView<'a,B:View+?Sized=InMemoryDB>{pub base:&'a B,pub layer:&'a Layer}
impl Layer{
    /// Accounts written (A13: the layer's size is bounded by what wrote it).
    pub fn written(&self)->usize{self.top.accounts.len()}
    /// Account entries and slot entries held (A13, D-7: a created account keeps every slot it loaded).
    pub fn entries(&self)->(usize,usize){(self.top.accounts.len(),self.top.accounts.values().map(|x|x.storage.len()).sum())}
    pub fn is_empty(&self)->bool{self.top.accounts.is_empty()&&self.top.contracts.is_empty()&&!self.hashes_set}
    /// Entries that hold a copy of nonempty code (A13, D-7: none after `drop_code_copies`).
    pub fn code_copies(&self)->usize{self.top.accounts.values().filter(|x|x.info.code.as_ref().map_or(false,|c|!c.is_empty())).count()}
    /// D-7 (A13 store mode): the entries of `addresses` keep their code hash and not their code. A store-mode
    /// account brings a copy of its code from the store (`TrieState::info`), which an entry would hold for
    /// as long as the layer is kept (the pool's pending state). Every entry's code can be read again: an
    /// entry made from the base's account has the base's code; a commit or `insert_info` puts code the base
    /// lacks in the layer's own code (`contract_of`); and a base never loses code (a store keeps code for
    /// good, a whole state is not changed under the layer). `account` and `merge` read it back.
    pub fn drop_code_copies(&mut self,addresses:&[Address]){
        for a in addresses{if let Some(x)=self.top.accounts.get_mut(a){if x.info.code_hash!=KECCAK_EMPTY&&!x.info.code_hash.is_zero(){x.info.code=None;}}}
    }
    /// An entry's info as its account reads: with its code, from the layer's code or the base's when the
    /// entry holds the hash only (`drop_code_copies`).
    fn info(&self,base:&(impl View+?Sized),info:&AccountInfo)->AccountInfo{
        let mut i=info.clone();
        if i.code.is_none()&&i.code_hash!=KECCAK_EMPTY&&!i.code_hash.is_zero(){i.code=self.contract(base,&i.code_hash);}
        i
    }
    fn account(&self,base:&(impl View+?Sized),a:&Address)->Option<Acc>{
        match self.top.accounts.get(a){Some(x)=>(!self.deleted.contains(a)).then(||Acc{info:self.info(base,&x.info),state:x.account_state.clone()}),None=>base.account(a)}
    }
    fn slot(&self,base:&(impl View+?Sized),a:&Address,k:&U256)->U256{
        if let Some(x)=self.top.accounts.get(a){if let Some(v)=x.storage.get(k){return *v}if self.cleared.contains(a){return U256::ZERO}}
        base.slot(a,k)
    }
    fn each_slot(&self,base:&(impl View+?Sized),a:&Address,f:&mut dyn FnMut(&U256,&U256)){
        let top=self.top.accounts.get(a);
        if !self.cleared.contains(a){base.each_slot(a,&mut |k,v|{if top.map_or(true,|t|!t.storage.contains_key(k)){f(k,v)}})}
        if let Some(t)=top{for (k,v) in &t.storage{f(k,v)}}
    }
    fn contract(&self,base:&(impl View+?Sized),h:&B256)->Option<Bytecode>{self.top.contracts.get(h).cloned().or_else(||base.contract(h))}
    fn block_hash(&self,base:&(impl View+?Sized),n:u64)->B256{
        if self.hashes_set{self.top.block_hashes.get(&U256::from(n)).copied().unwrap_or_else(||empty_block_hash(n))}else{base.block_hash(n)}
    }
    fn each_account(&self,base:&(impl View+?Sized),f:&mut dyn FnMut(&Address,Acc)){
        base.each_account(&mut |a,x|{if !self.top.accounts.contains_key(a){f(a,x)}});
        for (a,x) in &self.top.accounts{if !self.deleted.contains(a){f(a,Acc{info:self.info(base,&x.info),state:x.account_state.clone()})}}
    }
}
macro_rules! layered_view{($t:ident)=>{impl<B:View+?Sized> View for $t<'_,B>{
    fn account(&self,a:&Address)->Option<Acc>{self.layer().account(self.base,a)}
    fn slot(&self,a:&Address,k:&U256)->U256{self.layer().slot(self.base,a,k)}
    fn each_slot(&self,a:&Address,f:&mut dyn FnMut(&U256,&U256)){self.layer().each_slot(self.base,a,f)}
    fn contract(&self,h:&B256)->Option<Bytecode>{self.layer().contract(self.base,h)}
    fn block_hash(&self,n:u64)->B256{self.layer().block_hash(self.base,n)}
    fn each_account(&self,f:&mut dyn FnMut(&Address,Acc)){self.layer().each_account(self.base,f)}
}}}
impl<B:View+?Sized> LayerView<'_,B>{fn layer(&self)->&Layer{self.layer}}
layered_view!(LayerView);
layered_view!(Overlay);
impl<'a,B:View+?Sized> Overlay<'a,B>{
    pub fn new(base:&'a B)->Self{Overlay{base,l:Layer::default()}}
    /// The overlay of `layer` over `base`, which must be the state the layer was written on.
    pub fn from_layer(base:&'a B,layer:Layer)->Self{Overlay{base,l:layer}}
    pub fn into_layer(self)->Layer{self.l}
    fn layer(&self)->&Layer{&self.l}
    /// The entry for `a` to write: created from the parent's info and state with no slots, or fresh
    /// (default, storage cleared) where the full-state code had deleted it.
    fn entry(&mut self,a:Address)->&mut DbAccount{
        let base=self.base;let l=&mut self.l;
        if l.deleted.remove(&a){let e=l.top.accounts.get_mut(&a).expect("a deleted account has a tombstone");*e=DbAccount::default();l.retain.remove(&a);return e}
        l.top.accounts.entry(a).or_insert_with(||base.account(&a).map(|x|DbAccount{info:x.info,account_state:x.state,storage:Default::default()}).unwrap_or_default())
    }
    /// The accounts and slots written (complete: every write goes through the layer). An account whose
    /// storage was cleared here, or that exists on one side only, is listed whole.
    pub fn touched(&self)->crate::state_trie::Touched{
        let mut t=crate::state_trie::Touched::default();
        for (a,x) in &self.l.top.accounts{
            let was=self.base.account(a).map_or(false,|y|y.exists());
            let is=!self.l.deleted.contains(a)&&!matches!(x.account_state,AccountState::NotExisting);
            if self.l.cleared.contains(a)||was!=is{t.whole_account(*a);continue}
            t.account(*a);for k in x.storage.keys(){t.slot(*a,*k)}
        }
        t
    }
    /// Accounts with an entry in the layer (A13: the overlay's size).
    pub fn written(&self)->usize{self.l.top.accounts.len()}
    /// CacheDB::insert_contract: the parent's map may hold the code already (an or_insert either way).
    fn contract_of(&mut self,info:&mut AccountInfo){
        if let Some(code)=&info.code{if !code.is_empty(){
            if info.code_hash==KECCAK_EMPTY{info.code_hash=code.hash_slow();}
            if self.base.contract(&info.code_hash).is_none(){self.l.top.contracts.entry(info.code_hash).or_insert_with(||code.clone());}
        }}
        if info.code_hash.is_zero(){info.code_hash=KECCAK_EMPTY;}
    }
}
impl Overlay<'_,InMemoryDB>{
    /// The state the parent's copy would have reached: the parent with every write applied and the
    /// normalization of the full-state code.
    pub fn merge(&self)->InMemoryDB{
        let mut full=self.base.clone();let l=&self.l;
        for (a,x) in &l.top.accounts{
            if l.deleted.contains(a){full.accounts.remove(a);continue}
            let info=l.info(self.base,&x.info);let e=full.accounts.entry(*a).or_default();
            e.info=info;e.account_state=x.account_state.clone();
            if l.cleared.contains(a){e.storage=x.storage.clone();}else{e.storage.extend(x.storage.iter().map(|(k,v)|(*k,*v)));}
            if l.retain.contains(a){e.storage.retain(|_,v|*v!=U256::ZERO);if e.storage.capacity()>2*e.storage.len()+8{e.storage.shrink_to_fit()}}
        }
        for (h,c) in &l.top.contracts{full.contracts.entry(*h).or_insert_with(||c.clone());}
        if l.hashes_set{full.block_hashes=l.top.block_hashes.clone();}
        if full.accounts.capacity()>2*full.accounts.len()+16{full.accounts.shrink_to_fit()}
        full
    }
}
impl<B:View+?Sized> WorkState for Overlay<'_,B>{
    fn commit(&mut self,changes:HashMap<Address,Account>){
        // CacheDB::commit on the parent's entries, recorded in the layer.
        for (address,mut account) in changes{
            if !account.is_touched(){continue}
            if account.is_selfdestructed(){
                let e=self.entry(address);e.storage.clear();e.account_state=AccountState::NotExisting;e.info=AccountInfo::default();self.l.cleared.insert(address);continue;
            }
            let created=account.is_created();
            self.contract_of(&mut account.info);
            let e=self.entry(address);e.info=account.info;
            e.account_state=if created{e.storage.clear();AccountState::StorageCleared}else if e.account_state.is_storage_cleared(){AccountState::StorageCleared}else{AccountState::Touched};
            // Slots only read are not recorded: the full-state commit writes their unchanged value (or a
            // zero that normalization drops), so the state is the same, and the layer stays the size of
            // what was written. A created account's storage starts cleared, so all its slots are kept.
            e.storage.extend(account.storage.into_iter().filter(|(_,v)|created||v.is_changed()).map(|(k,v)|(k,v.present_value())));
            if created{self.l.cleared.insert(address);}
        }
    }
    fn remove(&mut self,a:Address){let e=self.entry(a);e.account_state=AccountState::NotExisting;e.storage.clear();e.info=Default::default();self.l.cleared.insert(a);}
    fn normalize(&mut self,touched:&[Address]){
        // `user_phase::normalize`: a NotExisting entry is deleted, a live one keeps its nonzero slots.
        for a in touched{
            let Some(acc)=self.account(a) else{continue};
            if !acc.exists(){
                // Every account the executor normalizes was just committed, so it has an entry in the layer.
                if self.l.top.accounts.contains_key(a){self.l.deleted.insert(*a);self.l.cleared.insert(*a);}
            }else{self.l.retain.insert(*a);}
        }
    }
    fn insert_info(&mut self,a:Address,mut info:AccountInfo){self.contract_of(&mut info);self.entry(a).info=info;}
    fn info_mut(&mut self,a:&Address)->Option<&mut AccountInfo>{
        if self.account(a).is_some(){Some(&mut self.entry(*a).info)}else{None}
    }
    fn set_block_hashes(&mut self,hashes:&[(u64,B256)]){let l=&mut self.l;l.top.block_hashes.clear();for (n,h) in hashes{l.top.block_hashes.insert(U256::from(*n),*h);}l.hashes_set=true;}
    fn as_view(&self)->&dyn View{self}
}

#[cfg(test)] mod tests;
