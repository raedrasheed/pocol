//! C-1 E1 (C-2 item 4): the world-state root kept incrementally.
//!
//! `StateTrie` holds the hexary Merkle-Patricia trie of a state, the world trie with one storage trie per
//! account, as immutable nodes shared between versions. It gives the same root as
//! `genesis_builder::world_root`, the whole-state computation, which stays as the reference: every node
//! is encoded as `p1_trie::node` encodes the same rows, and a node's reference depends only on the keys
//! below it and the depth it hangs at. An update copies the paths of the changed keys, re-encodes only
//! those, and shares every other node with the version it started from.
//!
//! Keys are Keccak-256 of the address (world) or of the 32-byte slot (storage): 64 nibbles, so no key is
//! a prefix of another and a branch never holds a value. Extension nodes are not stored: a branch keeps
//! the nibble it splits on, and the extension above it is encoded from that and the depth it hangs at.
//!
//! The trie is only correct for the state it was built or updated from. Callers check a parent's trie
//! against the parent's committed stateRoot before they use it; `Touched::between` lists every account
//! and slot that differs between two states, so an update from it is complete by construction.
use std::{collections::HashMap,sync::Arc};
use revm::{db::{AccountState,DbAccount},primitives::{keccak256,Address,B256,U256},InMemoryDB};

pub(crate) type Key=[u8;32];
pub(crate) fn nib(k:&Key,i:usize)->u8{let b=k[i/2];if i%2==0{b>>4}else{b&15}}
/// First nibble index in [from, 64) where the keys differ (64 if none).
pub(crate) fn diverge(a:&Key,b:&Key,from:usize)->usize{(from..64).find(|i|nib(a,*i)!=nib(b,*i)).unwrap_or(64)}

pub(crate) fn rlp_bytes(b:&[u8],out:&mut Vec<u8>){crate::rlp::encode_bytes(b,out)}
pub(crate) fn rlp_list(payload:&[u8])->Vec<u8>{let mut out=Vec::with_capacity(payload.len()+3);crate::rlp::encode_list_payload(payload,&mut out);out}
pub(crate) fn rlp_uint(n:&U256,out:&mut Vec<u8>){let b=n.to_be_bytes::<32>();rlp_bytes(&b[b.iter().position(|v|*v!=0).unwrap_or(32)..],out)}
/// Hex-prefix encoding of key[from..to] (`p1_trie::compact`).
pub(crate) fn compact(k:&Key,from:usize,to:usize,leaf:bool)->Vec<u8>{
    let odd=(to-from)%2==1;let flag=if leaf{2}else{0};
    let mut out=vec![((flag+odd as u8)<<4)|if odd{nib(k,from)}else{0}];
    let mut i=from+odd as usize;while i<to{out.push(nib(k,i)<<4|nib(k,i+1));i+=2}
    out
}

/// A node's reference in its parent (`p1_trie::child`): its encoding when shorter than 32 bytes, else
/// RLP(Keccak(encoding)), 33 bytes.
#[derive(Clone,Copy,PartialEq,Eq)]
pub struct Ref{pub(crate) len:u8,pub(crate) b:[u8;33]}
impl Ref{
    pub(crate) fn of(enc:&[u8])->Ref{
        let mut b=[0u8;33];
        if enc.len()<32{b[..enc.len()].copy_from_slice(enc);Ref{len:enc.len() as u8,b}}
        else{b[0]=0xa0;b[1..].copy_from_slice(keccak256(enc).as_slice());Ref{len:33,b}}
    }
    pub(crate) fn bytes(&self)->&[u8]{&self.b[..usize::from(self.len)]}
    /// The root of a trie whose top node has this reference: the root is always hashed.
    pub(crate) fn root(&self)->B256{if self.len==33{B256::from_slice(&self.b[1..])}else{keccak256(self.bytes())}}
}

/// The bytes a leaf commits to (the value `p1_trie::root` takes for its key).
trait Value:Clone{fn encode(&self,out:&mut Vec<u8>);}
/// A storage slot: RLP(minimal integer); zero slots are not in the trie.
impl Value for U256{fn encode(&self,out:&mut Vec<u8>){rlp_uint(self,out)}}
/// A world leaf: the account fields, its storage trie and that trie's root.
#[derive(Clone)]
struct Acct{nonce:u64,balance:U256,code_hash:B256,storage:Option<Arc<Node<U256>>>,storage_root:B256}
impl Value for Acct{fn encode(&self,out:&mut Vec<u8>){
    let mut p=Vec::with_capacity(110);rlp_uint(&U256::from(self.nonce),&mut p);rlp_uint(&self.balance,&mut p);
    rlp_bytes(self.storage_root.as_slice(),&mut p);rlp_bytes(self.code_hash.as_slice(),&mut p);
    out.extend(rlp_list(&p));
}}

enum Node<V>{
    Leaf{key:Key,value:V},
    /// All keys below share the nibbles before `at` and differ at `at`; `kids` holds one child per set bit
    /// of `mask`, in nibble order; `bref` is the reference of the branch node itself (depth `at`).
    Branch{at:u8,mask:u16,kids:Box<[Arc<Node<V>>]>,bref:Ref},
}
fn first_key<V>(mut n:&Node<V>)->&Key{loop{match n{Node::Leaf{key,..}=>return key,Node::Branch{kids,..}=>n=&kids[0]}}}
/// The node's reference when it hangs at depth d (its first nibble is key[d]).
fn node_ref<V:Value>(n:&Node<V>,d:usize)->Ref{
    match n{
        Node::Leaf{key,value}=>{let mut p=Vec::with_capacity(120);rlp_bytes(&compact(key,d,64,true),&mut p);let mut v=Vec::with_capacity(110);value.encode(&mut v);rlp_bytes(&v,&mut p);Ref::of(&rlp_list(&p))}
        Node::Branch{at,bref,..}=>{
            let at=usize::from(*at);if d==at{return *bref}
            let mut p=Vec::with_capacity(70);rlp_bytes(&compact(first_key(n),d,at,false),&mut p);p.extend_from_slice(bref.bytes());Ref::of(&rlp_list(&p))
        }
    }
}
fn branch<V:Value>(at:usize,kids:Vec<(u8,Arc<Node<V>>)>)->Node<V>{
    let mut p=Vec::with_capacity(16*33+1);let mut mask=0u16;let mut k=kids.iter().peekable();
    for nibble in 0..16u8{
        match k.peek(){Some((n,c)) if *n==nibble=>{p.extend_from_slice(node_ref(c,at+1).bytes());mask|=1<<nibble;k.next();}_=>p.push(0x80)}
    }
    p.push(0x80);
    Node::Branch{at:at as u8,mask,kids:kids.into_iter().map(|(_,c)|c).collect(),bref:Ref::of(&rlp_list(&p))}
}
/// The subtree of `rows` (sorted, distinct keys) hanging at depth d.
fn build<V:Value>(d:usize,rows:&[(Key,V)])->Option<Arc<Node<V>>>{
    match rows.len(){
        0=>None,
        1=>Some(Arc::new(Node::Leaf{key:rows[0].0,value:rows[0].1.clone()})),
        _=>{
            let at=diverge(&rows[0].0,&rows[rows.len()-1].0,d);let mut kids=Vec::new();let mut i=0;
            while i<rows.len(){let n=nib(&rows[i].0,at);let s=i;while i<rows.len()&&nib(&rows[i].0,at)==n{i+=1}kids.push((n,build(at+1,&rows[s..i]).expect("nonempty")));}
            Some(Arc::new(branch(at,kids)))
        }
    }
}
fn finish<V:Value>(at:usize,kids:Vec<(u8,Arc<Node<V>>)>)->Option<Arc<Node<V>>>{
    match kids.len(){0=>None,1=>kids.into_iter().next().map(|(_,c)|c),_=>Some(Arc::new(branch(at,kids)))}
}
/// Splits sorted changes into runs that share the nibble at position i.
fn groups<V>(ch:&[(Key,Option<V>)],i:usize)->Vec<(u8,&[(Key,Option<V>)])>{
    let mut out=Vec::new();let mut s=0;
    while s<ch.len(){let n=nib(&ch[s].0,i);let mut e=s;while e<ch.len()&&nib(&ch[e].0,i)==n{e+=1}out.push((n,&ch[s..e]));s=e;}
    out
}
/// The subtree hanging at depth d after `ch` (sorted, distinct keys; None deletes) is applied to `n`.
fn update<V:Value>(n:Option<&Arc<Node<V>>>,d:usize,ch:&[(Key,Option<V>)])->Option<Arc<Node<V>>>{
    if ch.is_empty(){return n.cloned()}
    let Some(node)=n else{
        let rows:Vec<(Key,V)>=ch.iter().filter_map(|(k,v)|v.clone().map(|v|(*k,v))).collect();return build(d,&rows);
    };
    match &**node{
        Node::Leaf{key,value}=>{
            let mut rows:Vec<(Key,V)>=ch.iter().filter_map(|(k,v)|v.clone().map(|v|(*k,v))).collect();
            if !ch.iter().any(|(k,_)|k==key){let at=rows.partition_point(|(k,_)|k<key);rows.insert(at,(*key,value.clone()));}
            build(d,&rows)
        }
        Node::Branch{at,mask,kids,..}=>{
            let at=usize::from(*at);let prefix=first_key(node);
            // An inserted key that leaves the branch's prefix before `at` splits the extension above it.
            let p=ch.iter().filter(|(_,v)|v.is_some()).map(|(k,_)|diverge(prefix,k,d)).min().unwrap_or(64).min(at);
            // A deletion that leaves the prefix before p names a key that is not in the trie. Dropping it
            // keeps the remaining keys sharing the prefix up to p, so their runs by nibble p are in order.
            let kept:Vec<(Key,Option<V>)>;
            let ch=if ch.iter().any(|(k,v)|v.is_none()&&diverge(prefix,k,d)<p){kept=ch.iter().filter(|(k,v)|v.is_some()||diverge(prefix,k,d)>=p).cloned().collect();&kept[..]}else{ch};
            if ch.is_empty(){return Some(node.clone())}
            if p<at{
                let own=nib(prefix,p);let mut out=Vec::new();let mut placed=false;
                for (n,g) in groups(ch,p){
                    if n==own{placed=true;if let Some(c)=update(Some(node),p+1,g){out.push((n,c))}}
                    else if let Some(c)=update(None,p+1,g){out.push((n,c))}
                }
                if !placed{let at_own=out.partition_point(|(n,_)|*n<own);out.insert(at_own,(own,node.clone()));}
                return finish(p,out);
            }
            let mut out:Vec<(u8,Arc<Node<V>>)>=Vec::with_capacity(16);let mut j=0;
            let present:Vec<u8>=(0..16u8).filter(|i|mask&(1<<i)!=0).collect();
            let gs=groups(ch,at);let mut g=gs.iter().peekable();
            for nibble in 0..16u8{
                let old=if present.get(j)==Some(&nibble){j+=1;Some(&kids[j-1])}else{None};
                let c=match g.peek(){Some((n,run)) if *n==nibble=>{let c=update(old,at+1,run);g.next();c}_=>old.cloned()};
                if let Some(c)=c{out.push((nibble,c))}
            }
            finish(at,out)
        }
    }
}
fn get<'a,V>(mut n:&'a Node<V>,k:&Key)->Option<&'a V>{
    loop{match n{
        Node::Leaf{key,value}=>return (key==k).then_some(value),
        Node::Branch{at,mask,kids,..}=>{let i=nib(k,usize::from(*at));if mask&(1<<i)==0{return None}n=&kids[(mask&((1u16<<i)-1)).count_ones() as usize]}
    }}
}
fn root_of<V:Value>(n:&Option<Arc<Node<V>>>)->B256{match n{None=>keccak256([0x80u8]),Some(n)=>node_ref(n,0).root()}}

/// Nodes and heap bytes of one trie version (shared nodes counted once per version).
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub struct TrieStats{pub leaves:u64,pub branches:u64,pub kid_slots:u64,pub heap_bytes:u64}
/// Heap bytes of a node held by an `Arc`: the two reference counts and the node.
pub fn node_bytes(world:bool)->u64{(2*std::mem::size_of::<usize>()+if world{std::mem::size_of::<Node<Acct>>()}else{std::mem::size_of::<Node<U256>>()}) as u64}
fn stats<V>(n:&Node<V>,world:bool,s:&mut TrieStats,sub:&dyn Fn(&V,&mut TrieStats)){
    s.heap_bytes+=node_bytes(world);
    match n{Node::Leaf{value,..}=>{s.leaves+=1;sub(value,s)}
        Node::Branch{kids,..}=>{s.branches+=1;s.kid_slots+=kids.len() as u64;s.heap_bytes+=(kids.len()*std::mem::size_of::<usize>()) as u64;for k in kids.iter(){stats(k,world,s,sub)}}}
}

pub(crate) fn hashed(a:&Address)->Key{keccak256(a.as_slice()).0}
pub(crate) fn hashed_slot(s:&U256)->Key{keccak256(s.to_be_bytes::<32>()).0}
fn exists(a:&DbAccount)->bool{!matches!(a.account_state,AccountState::NotExisting)}
fn storage_rows(a:&DbAccount)->Vec<(Key,U256)>{
    let mut rows:Vec<(Key,U256)>=a.storage.iter().filter(|(_,v)|**v!=U256::ZERO).map(|(k,v)|(hashed_slot(k),*v)).collect();rows.sort_unstable_by(|x,y|x.0.cmp(&y.0));rows
}
fn acct(a:&DbAccount,storage:Option<Arc<Node<U256>>>)->Acct{
    let storage_root=root_of(&storage);Acct{nonce:a.info.nonce,balance:a.info.balance,code_hash:a.info.code_hash,storage,storage_root}
}

/// The accounts and slots whose committed values may differ from the trie's state. `None` for an
/// account means its whole storage is read again.
#[derive(Clone,Debug,Default)]
pub struct Touched{accounts:HashMap<Address,Option<Vec<U256>>>}
impl Touched{
    pub fn account(&mut self,a:Address){self.accounts.entry(a).or_insert_with(||Some(Vec::new()));}
    pub fn whole_account(&mut self,a:Address){self.accounts.insert(a,None);}
    pub fn slot(&mut self,a:Address,s:U256){if let Some(v)=self.accounts.entry(a).or_insert_with(||Some(Vec::new())){v.push(s)}}
    /// Every touched account with its touched slots (None: its whole storage).
    pub fn iter(&self)->impl Iterator<Item=(&Address,Option<&[U256]>)>{self.accounts.iter().map(|(a,s)|(a,s.as_deref()))}
    pub fn len(&self)->usize{self.accounts.values().map(|s|1+s.as_ref().map_or(0,|v|v.len())).sum()}
    pub fn is_empty(&self)->bool{self.accounts.is_empty()}
    /// Every account and slot whose committed value (existence, nonce, balance, code hash, nonzero
    /// storage) differs between `from` and `to`. An account that appears or disappears is read whole.
    pub fn between(from:&InMemoryDB,to:&InMemoryDB)->Touched{
        let mut t=Touched::default();
        for (a,new) in &to.accounts{
            match from.accounts.get(a){
                Some(old) if exists(old)==exists(new)=>{
                    if !exists(new){continue}
                    if old.info.nonce!=new.info.nonce||old.info.balance!=new.info.balance||old.info.code_hash!=new.info.code_hash{t.account(*a)}
                    for (k,v) in &new.storage{if old.storage.get(k).copied().unwrap_or_default()!=*v{t.slot(*a,*k)}}
                    for (k,v) in &old.storage{if *v!=U256::ZERO&&!new.storage.contains_key(k){t.slot(*a,*k)}}
                }
                Some(old) if !exists(old)&&!exists(new)=>{}
                None if !exists(new)=>{}
                _=>t.whole_account(*a),
            }
        }
        for (a,old) in &from.accounts{if exists(old)&&!to.accounts.contains_key(a){t.whole_account(*a)}}
        t
    }
}

/// The trie of one state; cloning shares every node.
#[derive(Clone,Default)]
pub struct StateTrie{world:Option<Arc<Node<Acct>>>}
impl StateTrie{
    /// From the whole state (what `world_root` reads: existing accounts and their nonzero slots).
    pub fn build(db:&InMemoryDB)->StateTrie{
        let mut rows:Vec<(Key,Acct)>=db.accounts.iter().filter(|(_,a)|exists(a)).map(|(addr,a)|(hashed(addr),acct(a,build(0,&storage_rows(a))))).collect();
        rows.sort_unstable_by(|x,y|x.0.cmp(&y.0));StateTrie{world:build(0,&rows)}
    }
    pub fn root(&self)->B256{root_of(&self.world)}
    /// The trie of `db`, given that only the `touched` accounts and slots differ from this trie's state.
    pub fn update(&self,db:&InMemoryDB,touched:&Touched)->StateTrie{
        let mut ch:Vec<(Key,Option<Acct>)>=Vec::with_capacity(touched.accounts.len());
        for (addr,slots) in &touched.accounts{
            let k=hashed(addr);
            let Some(a)=db.accounts.get(addr).filter(|a|exists(a)) else{ch.push((k,None));continue};
            let old=self.world.as_deref().and_then(|w|get(w,&k));
            let storage=match (old,slots){
                (Some(o),Some(slots))=>{
                    let mut sc:Vec<(Key,Option<U256>)>=slots.iter().map(|s|(hashed_slot(s),a.storage.get(s).copied().filter(|v|*v!=U256::ZERO))).collect();
                    sc.sort_unstable_by(|x,y|x.0.cmp(&y.0));sc.dedup_by(|x,y|x.0==y.0);update(o.storage.as_ref(),0,&sc)
                }
                _=>build(0,&storage_rows(a)),
            };
            ch.push((k,Some(acct(a,storage))));
        }
        ch.sort_unstable_by(|x,y|x.0.cmp(&y.0));
        StateTrie{world:update(self.world.as_ref(),0,&ch)}
    }
    /// Nodes and heap bytes of this version, storage tries included.
    pub fn stats(&self)->TrieStats{
        let mut s=TrieStats::default();
        if let Some(w)=&self.world{stats(w,true,&mut s,&|a:&Acct,s:&mut TrieStats|if let Some(st)=&a.storage{stats(st,false,s,&|_,_|{})})}
        s
    }
}
impl std::fmt::Debug for StateTrie{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{write!(f,"StateTrie({})",self.root())}}

#[cfg(test)] mod tests;
