//! C-1 E4: the state trie as hash-addressed nodes in a store, and a state read from its root.
//!
//! The trie is E1's (`state_trie`): the same keys (Keccak-256 of the address or of the slot), the same
//! node encodings (`p1_trie::node`), the same roots. What differs is where the nodes are. A node here
//! names its children by an `Id`, the Keccak-256 of the child's own serialization, and carries each
//! child's reference at the depth it hangs at, so a branch is re-encoded without reading its unchanged
//! children. Nodes are immutable and identified by content: two versions that share a subtree share its
//! nodes, a write is idempotent, and a version is its root `Id`.
//!
//! `NodeStore` is where nodes, code and key preimages are read from. An update collects what it creates in
//! a `Batch`, read before the store, so a version can be computed before it is committed; `MemStore` is a
//! store in memory with the reference counts a disk store keeps (pocold's `state.redb` implements the
//! same trait). `TrieState` is a `view::View` of one version: what the executor reads of a state, read
//! from the trie.
use std::{collections::HashMap,sync::Arc};
use revm::{db::AccountState,primitives::{keccak256,AccountInfo,Address,Bytecode,B256,KECCAK_EMPTY,U256},InMemoryDB};
use crate::state_trie::{compact,diverge,hashed,hashed_slot,nib,rlp_bytes,rlp_list,rlp_uint,Key,Ref,Touched};
use crate::view::{Acc,View};

/// A node's identity: Keccak-256 of its serialization (`Node::encode`).
pub type Id=[u8;32];

/// What a leaf holds.
#[derive(Clone,Debug,PartialEq,Eq)]
pub enum Val{
    /// A storage slot's nonzero value, and the slot (the preimage of the leaf's key; C-1 E5: kept in the
    /// leaf, so it goes with the leaf's node and needs no table of its own).
    Slot(U256,U256),
    /// An account: its fields, its storage trie's root and that trie's top node (None: no storage), and
    /// its address (the preimage of the leaf's key).
    Acct{nonce:u64,balance:U256,code_hash:B256,storage_root:B256,storage:Option<Id>,address:Address},
}
impl Val{
    /// The bytes the leaf commits to in the trie (as `state_trie`'s values).
    fn commit(&self,out:&mut Vec<u8>){
        match self{
            Val::Slot(v,_)=>rlp_uint(v,out),
            Val::Acct{nonce,balance,code_hash,storage_root,..}=>{
                let mut p=Vec::with_capacity(110);rlp_uint(&U256::from(*nonce),&mut p);rlp_uint(balance,&mut p);
                rlp_bytes(storage_root.as_slice(),&mut p);rlp_bytes(code_hash.as_slice(),&mut p);out.extend(rlp_list(&p));
            }
        }
    }
}

/// A stored node. A branch keeps one key below it (`key`, for the extension above it), its children's
/// `Id`s with their references at depth `at + 1`, and its own reference at depth `at`.
#[derive(Clone,Debug,PartialEq,Eq)]
pub enum Node{
    Leaf{key:Key,val:Val},
    Branch{at:u8,mask:u16,key:Key,kids:Box<[(Id,Ref)]>,bref:Ref},
}
impl std::fmt::Debug for Ref{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{write!(f,"Ref({:02x?})",self.bytes())}}

fn put_ref(r:&Ref,out:&mut Vec<u8>){out.push(r.len);out.extend_from_slice(r.bytes());}
fn get_ref(b:&[u8],i:&mut usize)->Option<Ref>{
    let len=*b.get(*i)? as usize;if len>33{return None}
    let bytes=b.get(*i+1..*i+1+len)?;let mut r=Ref{len:len as u8,b:[0;33]};r.b[..len].copy_from_slice(bytes);*i+=1+len;Some(r)
}
fn take<'a>(b:&'a [u8],i:&mut usize,n:usize)->Option<&'a [u8]>{let s=b.get(*i..*i+n)?;*i+=n;Some(s)}
impl Node{
    /// The serialization a store keeps and the `Id` is taken of.
    pub fn encode(&self)->Vec<u8>{
        let mut out=Vec::with_capacity(160);
        match self{
            Node::Leaf{key,val}=>{
                out.push(0);out.extend_from_slice(key);
                match val{
                    Val::Slot(v,slot)=>{out.push(0);out.extend_from_slice(&v.to_be_bytes::<32>());out.extend_from_slice(&slot.to_be_bytes::<32>());}
                    Val::Acct{nonce,balance,code_hash,storage_root,storage,address}=>{
                        out.push(1);out.extend_from_slice(&nonce.to_be_bytes());out.extend_from_slice(&balance.to_be_bytes::<32>());
                        out.extend_from_slice(code_hash.as_slice());out.extend_from_slice(storage_root.as_slice());
                        match storage{Some(id)=>{out.push(1);out.extend_from_slice(id);}None=>out.push(0)}
                        out.extend_from_slice(address.as_slice());
                    }
                }
            }
            Node::Branch{at,mask,key,kids,bref}=>{
                out.push(1);out.push(*at);out.extend_from_slice(&mask.to_be_bytes());out.extend_from_slice(key);put_ref(bref,&mut out);
                for (id,r) in kids.iter(){out.extend_from_slice(id);put_ref(r,&mut out);}
            }
        }
        out
    }
    /// The inverse of `encode` (None for bytes that are not a node's serialization).
    pub fn decode(b:&[u8])->Option<Node>{
        let mut i=0;let tag=*take(b,&mut i,1)?.first()?;
        let n=match tag{
            0=>{
                let key:Key=take(b,&mut i,32)?.try_into().ok()?;
                let val=match *take(b,&mut i,1)?.first()?{
                    0=>{let v=U256::from_be_slice(take(b,&mut i,32)?);Val::Slot(v,U256::from_be_slice(take(b,&mut i,32)?))}
                    1=>{
                        let nonce=u64::from_be_bytes(take(b,&mut i,8)?.try_into().ok()?);let balance=U256::from_be_slice(take(b,&mut i,32)?);
                        let code_hash=B256::from_slice(take(b,&mut i,32)?);let storage_root=B256::from_slice(take(b,&mut i,32)?);
                        let storage=match *take(b,&mut i,1)?.first()?{0=>None,1=>Some(take(b,&mut i,32)?.try_into().ok()?),_=>return None};
                        let address=Address::from_slice(take(b,&mut i,20)?);
                        Val::Acct{nonce,balance,code_hash,storage_root,storage,address}
                    }
                    _=>return None,
                };
                Node::Leaf{key,val}
            }
            1=>{
                let at=*take(b,&mut i,1)?.first()?;let mask=u16::from_be_bytes(take(b,&mut i,2)?.try_into().ok()?);
                let key:Key=take(b,&mut i,32)?.try_into().ok()?;let bref=get_ref(b,&mut i)?;
                let mut kids=Vec::with_capacity(mask.count_ones() as usize);
                for _ in 0..mask.count_ones(){let id:Id=take(b,&mut i,32)?.try_into().ok()?;kids.push((id,get_ref(b,&mut i)?));}
                Node::Branch{at,mask,key,kids:kids.into(),bref}
            }
            _=>return None,
        };
        (i==b.len()).then_some(n)
    }
    pub fn id(&self)->Id{keccak256(self.encode()).0}
    /// C-1 E4.5: the heap a decoded node holds behind its `Arc`: the Arc's two counts, the node inline and
    /// a branch's children (A13 counts the node cache by this, not by the encoding's length).
    pub fn resident_bytes(&self)->usize{
        2*std::mem::size_of::<usize>()+std::mem::size_of::<Node>()+match self{Node::Branch{kids,..}=>kids.len()*std::mem::size_of::<(Id,Ref)>(),Node::Leaf{..}=>0}
    }
    fn key(&self)->&Key{match self{Node::Leaf{key,..}|Node::Branch{key,..}=>key}}
    /// The children a store counts references to: a branch's kids, an account leaf's storage trie.
    pub fn children(&self)->Vec<Id>{
        match self{
            Node::Branch{kids,..}=>kids.iter().map(|(id,_)|*id).collect(),
            Node::Leaf{val:Val::Acct{storage:Some(s),..},..}=>vec![*s],
            Node::Leaf{..}=>vec![],
        }
    }
}
/// The node's reference when it hangs at depth d (`state_trie::node_ref`).
fn node_ref(n:&Node,d:usize)->Ref{
    match n{
        Node::Leaf{key,val}=>{let mut p=Vec::with_capacity(120);rlp_bytes(&compact(key,d,64,true),&mut p);let mut v=Vec::with_capacity(110);val.commit(&mut v);rlp_bytes(&v,&mut p);Ref::of(&rlp_list(&p))}
        Node::Branch{at,key,bref,..}=>{
            let at=usize::from(*at);if d==at{return *bref}
            let mut p=Vec::with_capacity(70);rlp_bytes(&compact(key,d,at,false),&mut p);p.extend_from_slice(bref.bytes());Ref::of(&rlp_list(&p))
        }
    }
}

/// A13 (C-1 E4.5): the largest serialization of each node kind (`Node::encode`): an account leaf (its
/// storage trie's top included), a slot leaf, a branch without its children, and one child of a branch.
pub const ACCT_LEAF_ENC:u128=1+32+1+8+32+32+32+1+32+20;
pub const SLOT_LEAF_ENC:u128=1+32+1+32+32;
pub const BRANCH_ENC:u128=1+1+2+32+1+33;
pub const KID_ENC:u128=32+1+33;
/// What a stored row adds to a node's serialization: its 32-byte id and its 4-byte reference count
/// (pocold `state_store`).
pub const ROW:u128=32+4;
/// Branches on the path from a trie's top to a leaf, at most: one at each of the 64 nibbles of a key.
pub const PATH_BRANCHES:u128=64;
/// A13: the most bytes a store keeps for one version of a state of `accounts` accounts and `slots` nonzero
/// slots, every key and value. A leaf per account and per slot; at most one branch per leaf (every branch
/// has two children or more: `build` and `finish` make no other); at most one child per node, since each
/// node but the world trie's top hangs from one branch (a storage trie's top hangs from its account's leaf,
/// which ACCT_LEAF_ENC counts). A node that occurs more than once is stored once.
pub fn version_bytes(accounts:u128,slots:u128)->Option<u128>{
    let keys=accounts.checked_add(slots)?;
    [accounts.checked_mul(ACCT_LEAF_ENC+ROW)?,slots.checked_mul(SLOT_LEAF_ENC+ROW)?,keys.checked_mul(BRANCH_ENC+ROW)?,keys.checked_mul(2*KID_ENC)?].iter().try_fold(0u128,|a,b|a.checked_add(*b))
}
/// A13: the most nodes an update puts in its batch, (branches, leaves, children of those branches), when it
/// changes `accounts` accounts and `slots` slots and its child version holds at most `child_accounts`
/// accounts and `child_slots` slots. Every node an update puts in the batch is a node of the child: the top
/// of the subtree it returns, or below one. Branches: PATH_BRANCHES on each changed key's path, and at most
/// one per leaf of the child. Leaves: each changed key's, and the leaf it displaces (`update` rebuilds a
/// leaf's place with the old leaf in it, unchanged), and at most the child's. Children: at most 16 a branch,
/// and at most one per node of the child.
pub fn batch_nodes(accounts:u128,slots:u128,child_accounts:u128,child_slots:u128)->Option<(u128,u128,u128)>{
    let (keys,child)=(accounts.checked_add(slots)?,child_accounts.checked_add(child_slots)?);
    let branches=keys.checked_mul(PATH_BRANCHES)?.min(child);
    Some((branches,keys.checked_mul(2)?.min(child),branches.checked_mul(16)?.min(child.checked_mul(2)?)))
}
/// A13: the most bytes a block's version adds to a store that holds its parent's: the batch's nodes that are
/// not stored yet, which are among its branches and the changed keys' leaves (a displaced leaf is the
/// parent's, unchanged), with their children. Arguments as `batch_nodes`.
pub fn block_bytes(accounts:u128,slots:u128,child_accounts:u128,child_slots:u128)->Option<u128>{
    let (branches,_,kids)=batch_nodes(accounts,slots,child_accounts,child_slots)?;
    [accounts.min(child_accounts).checked_mul(ACCT_LEAF_ENC+ROW)?,slots.min(child_slots).checked_mul(SLOT_LEAF_ENC+ROW)?,branches.checked_mul(BRANCH_ENC+ROW)?,kids.checked_mul(KID_ENC)?].iter().try_fold(0u128,|a,b|a.checked_add(*b))
}

/// Where nodes and code are read from. (C-1 E5: a leaf keeps its key's preimage.)
pub trait NodeStore{
    fn node(&self,id:&Id)->Option<Arc<Node>>;
    fn code(&self,h:&B256)->Option<Bytecode>;
}

/// What an update creates, before it is committed: nodes, and the code and preimages they need.
#[derive(Default,Clone)]
pub struct Batch{pub nodes:HashMap<Id,Arc<Node>>,pub code:HashMap<B256,Bytecode>}
/// A store read through a batch.
pub struct Over<'a>{pub store:&'a dyn NodeStore,pub batch:&'a Batch}
impl NodeStore for Over<'_>{
    fn node(&self,id:&Id)->Option<Arc<Node>>{self.batch.nodes.get(id).cloned().or_else(||self.store.node(id))}
    fn code(&self,h:&B256)->Option<Bytecode>{self.batch.code.get(h).cloned().or_else(||self.store.code(h))}
}

fn load(s:&dyn NodeStore,b:&Batch,id:&Id)->Arc<Node>{
    b.nodes.get(id).cloned().or_else(||s.node(id)).unwrap_or_else(||panic!("C-1 E4: trie node {} is missing from the store",B256::from(*id)))
}
fn put(b:&mut Batch,n:Node)->(Id,Arc<Node>){let n=Arc::new(n);let id=n.id();b.nodes.entry(id).or_insert_with(||n.clone());(id,n)}

/// A child in a branch being rebuilt: kept as it was (its reference at the branch's depth is known), or
/// new or moved (its node is at hand).
enum Kid{Kept(Id,Ref),Node(Id,Arc<Node>)}
fn branch(b:&mut Batch,at:usize,kids:Vec<(u8,Kid)>)->(Id,Arc<Node>){
    let mut p=Vec::with_capacity(16*33+1);let mut mask=0u16;let mut out=Vec::with_capacity(kids.len());let mut key=None;
    let mut k=kids.into_iter().peekable();
    for nibble in 0..16u8{
        match k.peek(){Some((n,_)) if *n==nibble=>{
            let (_,kid)=k.next().unwrap();
            let (id,r,first)=match kid{Kid::Kept(id,r)=>(id,r,None),Kid::Node(id,n)=>(id,node_ref(&n,at+1),Some(*n.key()))};
            if key.is_none(){key=first;}
            p.extend_from_slice(r.bytes());mask|=1<<nibble;out.push((id,r));
        }_=>p.push(0x80)}
    }
    p.push(0x80);
    // Callers give at least one child node (`finish` loads one when all are kept).
    let key=key.expect("a branch is built with at least one child node at hand");
    put(b,Node::Branch{at:at as u8,mask,key,kids:out.into(),bref:Ref::of(&rlp_list(&p))})
}

/// The subtree of `rows` (sorted, distinct keys) hanging at depth d.
fn build(b:&mut Batch,d:usize,rows:&[(Key,Val)])->Option<(Id,Arc<Node>)>{
    match rows.len(){
        0=>None,
        1=>Some(put(b,Node::Leaf{key:rows[0].0,val:rows[0].1.clone()})),
        _=>{
            let at=diverge(&rows[0].0,&rows[rows.len()-1].0,d);let mut kids=Vec::new();let mut i=0;
            while i<rows.len(){let n=nib(&rows[i].0,at);let s=i;while i<rows.len()&&nib(&rows[i].0,at)==n{i+=1}
                let (id,node)=build(b,at+1,&rows[s..i]).expect("nonempty");kids.push((n,Kid::Node(id,node)));}
            Some(branch(b,at,kids))
        }
    }
}
fn finish(s:&dyn NodeStore,b:&mut Batch,at:usize,kids:Vec<(u8,Kid)>)->Option<(Id,Arc<Node>)>{
    match kids.len(){
        0=>None,
        1=>Some(match kids.into_iter().next().unwrap().1{Kid::Node(id,n)=>(id,n),Kid::Kept(id,_)=>(id,load(s,b,&id))}),
        _=>{
            // The branch's key is any key below it: from a child node at hand, or a kept child's.
            let has_node=kids.iter().any(|(_,k)|matches!(k,Kid::Node(..)));
            let mut kids=kids;
            if !has_node{let (n,k)=kids.remove(0);let Kid::Kept(id,_)=k else{unreachable!()};let node=load(s,b,&id);kids.insert(0,(n,Kid::Node(id,node)));}
            Some(branch(b,at,kids))
        }
    }
}
fn groups<V>(ch:&[(Key,Option<V>)],i:usize)->Vec<(u8,&[(Key,Option<V>)])>{
    let mut out=Vec::new();let mut s=0;
    while s<ch.len(){let n=nib(&ch[s].0,i);let mut e=s;while e<ch.len()&&nib(&ch[e].0,i)==n{e+=1}out.push((n,&ch[s..e]));s=e;}
    out
}
/// `state_trie::update` over stored nodes: the subtree hanging at depth d after `ch` (sorted, distinct
/// keys; None deletes) is applied to the node `n`.
fn update(s:&dyn NodeStore,b:&mut Batch,n:Option<(Id,Arc<Node>)>,d:usize,ch:&[(Key,Option<Val>)])->Option<(Id,Arc<Node>)>{
    if ch.is_empty(){return n}
    let Some((nid,node))=n else{
        let rows:Vec<(Key,Val)>=ch.iter().filter_map(|(k,v)|v.clone().map(|v|(*k,v))).collect();return build(b,d,&rows);
    };
    match &*node{
        Node::Leaf{key,val}=>{
            let mut rows:Vec<(Key,Val)>=ch.iter().filter_map(|(k,v)|v.clone().map(|v|(*k,v))).collect();
            if !ch.iter().any(|(k,_)|k==key){let at=rows.partition_point(|(k,_)|k<key);rows.insert(at,(*key,val.clone()));}
            build(b,d,&rows)
        }
        Node::Branch{at,mask,key:prefix,kids,..}=>{
            let at=usize::from(*at);let prefix=*prefix;
            let p=ch.iter().filter(|(_,v)|v.is_some()).map(|(k,_)|diverge(&prefix,k,d)).min().unwrap_or(64).min(at);
            let kept:Vec<(Key,Option<Val>)>;
            let ch=if ch.iter().any(|(k,v)|v.is_none()&&diverge(&prefix,k,d)<p){kept=ch.iter().filter(|(k,v)|v.is_some()||diverge(&prefix,k,d)>=p).cloned().collect();&kept[..]}else{ch};
            if ch.is_empty(){return Some((nid,node.clone()))}
            if p<at{
                let own=nib(&prefix,p);let mut out=Vec::new();let mut placed=false;
                for (n,g) in groups(ch,p){
                    if n==own{placed=true;if let Some((id,c))=update(s,b,Some((nid,node.clone())),p+1,g){out.push((n,Kid::Node(id,c)))}}
                    else if let Some((id,c))=update(s,b,None,p+1,g){out.push((n,Kid::Node(id,c)))}
                }
                if !placed{let at_own=out.partition_point(|(n,_)|*n<own);out.insert(at_own,(own,Kid::Node(nid,node.clone())));}
                return finish(s,b,p,out);
            }
            let present:Vec<u8>=(0..16u8).filter(|i|mask&(1<<i)!=0).collect();
            let mut out:Vec<(u8,Kid)>=Vec::with_capacity(16);let mut j=0;
            let gs=groups(ch,at);let mut g=gs.iter().peekable();
            for nibble in 0..16u8{
                let old=if present.get(j)==Some(&nibble){j+=1;Some(kids[j-1])}else{None};
                match g.peek(){
                    Some((n,run)) if *n==nibble=>{
                        let oldn=old.map(|(id,_)|(id,load(s,b,&id)));
                        if let Some((id,c))=update(s,b,oldn,at+1,run){out.push((nibble,Kid::Node(id,c)))}
                        g.next();
                    }
                    _=>if let Some((id,r))=old{out.push((nibble,Kid::Kept(id,r)))},
                }
            }
            finish(s,b,at,out)
        }
    }
}
/// The leaf value under key k, from the subtree at `id`.
fn get(s:&dyn NodeStore,id:&Id,k:&Key)->Option<Val>{
    let mut n=s.node(id).unwrap_or_else(||panic!("C-1 E4: trie node {} is missing from the store",B256::from(*id)));
    loop{
        let next=match &*n{
            Node::Leaf{key,val}=>return (key==k).then(||val.clone()),
            Node::Branch{at,mask,kids,..}=>{let i=nib(k,usize::from(*at));if mask&(1<<i)==0{return None}kids[(mask&((1u16<<i)-1)).count_ones() as usize].0}
        };
        n=s.node(&next).unwrap_or_else(||panic!("C-1 E4: trie node {} is missing from the store",B256::from(next)));
    }
}
/// Every leaf of the subtree at `id`, in key order.
fn walk(s:&dyn NodeStore,id:&Id,f:&mut dyn FnMut(&Key,&Val)){
    let mut stack=vec![*id];
    while let Some(x)=stack.pop(){
        let n=s.node(&x).unwrap_or_else(||panic!("C-1 E4: trie node {} is missing from the store",B256::from(x)));
        match &*n{Node::Leaf{key,val}=>f(key,val),Node::Branch{kids,..}=>for (c,_) in kids.iter().rev(){stack.push(*c)}}
    }
}
/// A leaf that differs between two versions (`NodeTrie::diff`): only in the first, only in the second, or under the
/// same key in both with another value. (The key is read by the tests: the world and storage leaves carry their
/// preimages.)
#[cfg_attr(not(test),allow(dead_code))]
enum Change<'a>{Gone(&'a Key,&'a Val),Came(&'a Key,&'a Val),Moved(&'a Key,&'a Val,&'a Val)}
/// The key nibbles every key below a node shares: a branch's up to the nibble it branches at, a leaf's whole key.
fn shared(n:&Node)->usize{match n{Node::Leaf{..}=>64,Node::Branch{at,..}=>usize::from(*at)}}
fn kid(n:&Node,i:u8)->Option<Id>{match n{Node::Branch{mask,kids,..} if mask&(1<<i)!=0=>Some(kids[(mask&((1u16<<i)-1)).count_ones() as usize].0),_=>None}}
fn read(s:&dyn NodeStore,id:&Id)->Arc<Node>{s.node(id).unwrap_or_else(||panic!("C-1 E4: trie node {} is missing from the store",B256::from(*id)))}
/// Every leaf below `n`, each node read.
fn leaves(s:&dyn NodeStore,n:&Node,f:&mut dyn FnMut(&Key,&Val)){
    match n{Node::Leaf{key,val}=>f(key,val),Node::Branch{kids,..}=>for (c,_) in kids.iter(){leaves(s,&read(s,c),f)}}
}
/// The leaves that differ between the subtrees `a` and `b`, which hold the keys of one key range in two versions.
fn diff_ids(s:&dyn NodeStore,a:Option<&Id>,b:Option<&Id>,f:&mut dyn FnMut(Change)){
    match (a,b){
        (Some(x),Some(y)) if x==y=>{}
        (None,None)=>{}
        (Some(x),None)=>leaves(s,&read(s,x),&mut |k,v|f(Change::Gone(k,v))),
        (None,Some(y))=>leaves(s,&read(s,y),&mut |k,v|f(Change::Came(k,v))),
        (Some(x),Some(y))=>diff_nodes(s,&read(s,x),&read(s,y),f),
    }
}
/// As `diff_ids`, for two nodes at hand. All keys below `a` share its first `shared(a)` nibbles, and likewise for
/// `b`: if those prefixes part before either ends, no key is in both; if one is longer, the shorter node's other
/// children hold keys the longer one cannot.
fn diff_nodes(s:&dyn NodeStore,a:&Node,b:&Node,f:&mut dyn FnMut(Change)){
    let (la,lb)=(shared(a),shared(b));
    if diverge(a.key(),b.key(),0)<la.min(lb){leaves(s,a,&mut |k,v|f(Change::Gone(k,v)));leaves(s,b,&mut |k,v|f(Change::Came(k,v)));return}
    match (a,b){
        (Node::Leaf{key,val:x},Node::Leaf{val:y,..})=>if x!=y{f(Change::Moved(key,x,y))},
        _ if la==lb=>for i in 0..16u8{diff_ids(s,kid(a,i).as_ref(),kid(b,i).as_ref(),f)},
        _ if la<lb=>{let own=nib(b.key(),la);
            for i in 0..16u8{match (kid(a,i),i==own){(Some(x),true)=>diff_nodes(s,&read(s,&x),b,f),(None,true)=>leaves(s,b,&mut |k,v|f(Change::Came(k,v))),
                (Some(x),false)=>leaves(s,&read(s,&x),&mut |k,v|f(Change::Gone(k,v))),(None,false)=>{}}}}
        _=>{let own=nib(a.key(),lb);
            for i in 0..16u8{match (kid(b,i),i==own){(Some(y),true)=>diff_nodes(s,a,&read(s,&y),f),(None,true)=>leaves(s,a,&mut |k,v|f(Change::Gone(k,v))),
                (Some(y),false)=>leaves(s,&read(s,&y),&mut |k,v|f(Change::Came(k,v))),(None,false)=>{}}}}
    }
}
fn root_of(n:Option<&Node>)->B256{match n{None=>keccak256([0x80u8]),Some(n)=>node_ref(n,0).root()}}

/// One version of the state trie: its world trie's top node.
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub struct NodeTrie{pub world:Option<Id>}
fn exists(a:&revm::db::DbAccount)->bool{!matches!(a.account_state,AccountState::NotExisting)}
fn acct_val(address:&Address,info:&AccountInfo,storage:Option<(Id,Arc<Node>)>)->Val{
    let storage_root=root_of(storage.as_ref().map(|(_,n)|&**n));
    Val::Acct{nonce:info.nonce,balance:info.balance,code_hash:info.code_hash,storage_root,storage:storage.map(|(id,_)|id),address:*address}
}
/// The code a state's account brings to the batch.
fn note(b:&mut Batch,s:&dyn NodeStore,info:&AccountInfo,code:Option<Bytecode>){
    if info.code_hash!=KECCAK_EMPTY&&!info.code_hash.is_zero()&&s.code(&info.code_hash).is_none(){if let Some(c)=code{b.code.entry(info.code_hash).or_insert(c);}}
}
impl NodeTrie{
    /// The trie of a whole state (what `world_root` reads: existing accounts and their nonzero slots).
    pub fn build(db:&InMemoryDB,s:&dyn NodeStore,b:&mut Batch)->NodeTrie{
        let mut rows:Vec<(Key,Val)>=Vec::with_capacity(db.accounts.len());
        for (addr,a) in db.accounts.iter().filter(|(_,a)|exists(a)){
            let mut sr:Vec<(Key,Val)>=a.storage.iter().filter(|(_,v)|**v!=U256::ZERO).map(|(k,v)|(hashed_slot(k),Val::Slot(*v,*k))).collect();
            sr.sort_unstable_by(|x,y|x.0.cmp(&y.0));
            let storage=build(b,0,&sr);
            note(b,s,&a.info,a.info.code.clone().or_else(||db.contracts.get(&a.info.code_hash).cloned()));
            rows.push((hashed(addr),acct_val(addr,&a.info,storage)));
        }
        rows.sort_unstable_by(|x,y|x.0.cmp(&y.0));
        NodeTrie{world:build(b,0,&rows).map(|(id,_)|id)}
    }
    /// The state root.
    pub fn root(&self,s:&dyn NodeStore,b:&Batch)->B256{root_of(self.world.map(|id|load(s,b,&id)).as_deref())}
    /// The trie of `child`, given that only the `touched` accounts and slots differ from this version's
    /// state (the executor's write set, `view::Overlay::touched`).
    pub fn update(&self,s:&dyn NodeStore,b:&mut Batch,child:&dyn View,touched:&Touched)->NodeTrie{
        let mut ch:Vec<(Key,Option<Val>)>=Vec::new();
        for (addr,slots) in touched.iter(){
            let k=hashed(addr);
            let Some(a)=child.account(addr).filter(|a|a.exists()) else{ch.push((k,None));continue};
            let old=self.world.and_then(|w|get(&Over{store:s,batch:b},&w,&k));
            let old_storage=match &old{Some(Val::Acct{storage,..})=>*storage,_=>None};
            let storage=match (&old,slots){
                (Some(_),Some(slots))=>{
                    let mut sc:Vec<(Key,Option<Val>)>=slots.iter().map(|sl|{let h=hashed_slot(sl);let v=child.slot(addr,sl);(h,(v!=U256::ZERO).then_some(Val::Slot(v,*sl)))}).collect();
                    sc.sort_unstable_by(|x,y|x.0.cmp(&y.0));sc.dedup_by(|x,y|x.0==y.0);
                    let start=old_storage.map(|id|(id,load(s,b,&id)));update(s,b,start,0,&sc)
                }
                _=>{
                    let mut sr:Vec<(Key,Val)>=Vec::new();
                    child.each_slot(addr,&mut |sl,v|{if *v!=U256::ZERO{sr.push((hashed_slot(sl),Val::Slot(*v,*sl)))}});
                    sr.sort_unstable_by(|x,y|x.0.cmp(&y.0));sr.dedup_by(|x,y|x.0==y.0);build(b,0,&sr)
                }
            };
            let code=a.info.code.clone().or_else(||child.contract(&a.info.code_hash));
            note(b,s,&a.info,code);
            ch.push((k,Some(acct_val(addr,&a.info,storage))));
        }
        ch.sort_unstable_by(|x,y|x.0.cmp(&y.0));
        let start=self.world.map(|id|(id,load(s,b,&id)));
        NodeTrie{world:update(s,b,start,0,&ch).map(|(id,_)|id)}
    }
}

impl NodeTrie{
    /// C-1 E4.4 (2026-10-27): the accounts and slots whose committed values differ between this version and `to`,
    /// as `Touched::between` lists them for two whole states: an account in one version only, whole; one in both
    /// with another leaf, with the slots whose leaves differ. It reads every node of `to` that this version does
    /// not hold at the same place, the storage of an account only `to` has included, each through the store
    /// (which checks it against its id); of this version, only where the two differ. Two nodes with the same `Id`
    /// are the same subtree, so a node of `to` it does not read is one this version holds, under the same keys.
    pub fn diff(&self,s:&dyn NodeStore,to:&NodeTrie)->Touched{
        let mut t=Touched::default();
        diff_ids(s,self.world.as_ref(),to.world.as_ref(),&mut |c|match c{
            Change::Gone(_,Val::Acct{address,..})=>t.whole_account(*address),
            Change::Came(_,Val::Acct{address,storage,..})=>{t.whole_account(*address);if let Some(st)=storage{walk(s,st,&mut |_,_|{})}}
            Change::Moved(_,Val::Acct{address,storage:x,..},Val::Acct{storage:y,..})=>{let a=*address;t.account(a);
                diff_ids(s,x.as_ref(),y.as_ref(),&mut |c|match c{Change::Gone(_,Val::Slot(_,k))|Change::Came(_,Val::Slot(_,k))|Change::Moved(_,_,Val::Slot(_,k))=>t.slot(a,*k),
                    _=>panic!("C-1 E4: an account leaf in a storage trie")})}
            _=>panic!("C-1 E4: a slot leaf in the world trie"),
        });
        t
    }
}

/// A node store that keeps versions: commits a batch under a pinned root and releases pins (pocold's
/// `state.redb`, or `SharedMem` in memory).
pub trait TrieStore:NodeStore+Send+Sync{
    /// Stores the root's new nodes from the batch, with its code and preimages, and pins the root once.
    fn commit(&self,b:&Batch,root:Option<Id>)->Result<(),String>;
    /// Releases one pin of the root and every node only it held.
    fn unpin(&self,root:&Id)->Result<(),String>;
    /// The pins held, summed over roots (a leak check).
    fn pinned(&self)->Result<u64,String>;
}
/// `MemStore` behind a lock, as a `TrieStore` (tests, and a node without a data directory).
#[derive(Default)]
pub struct SharedMem(pub std::sync::Mutex<MemStore>);
impl NodeStore for SharedMem{
    fn node(&self,id:&Id)->Option<Arc<Node>>{self.0.lock().unwrap().node(id)}
    fn code(&self,h:&B256)->Option<Bytecode>{self.0.lock().unwrap().code(h)}
}
impl TrieStore for SharedMem{
    fn commit(&self,b:&Batch,root:Option<Id>)->Result<(),String>{self.0.lock().unwrap().commit(b,root);Ok(())}
    fn unpin(&self,root:&Id)->Result<(),String>{self.0.lock().unwrap().unpin(root);Ok(())}
    fn pinned(&self)->Result<u64,String>{Ok(self.0.lock().unwrap().pins.values().map(|c|u64::from(*c)).sum())}
}
/// A store in memory with reference counts: a node lives while a pinned root or a stored node names it.
#[derive(Default)]
pub struct MemStore{pub nodes:HashMap<Id,(Arc<Node>,u32)>,pub code:HashMap<B256,Bytecode>,pub pins:HashMap<Id,u32>}
impl NodeStore for MemStore{
    fn node(&self,id:&Id)->Option<Arc<Node>>{self.nodes.get(id).map(|x|x.0.clone())}
    fn code(&self,h:&B256)->Option<Bytecode>{self.code.get(h).cloned()}
}
impl MemStore{
    /// Stores the nodes of `root` that the batch holds (every node reachable from it that is not stored
    /// yet), with the code and preimages, and pins the root once.
    pub fn commit(&mut self,b:&Batch,root:Option<Id>){
        for (h,c) in &b.code{self.code.entry(*h).or_insert_with(||c.clone());}
        let Some(root)=root else{return};
        *self.pins.entry(root).or_insert(0)+=1;
        let mut stack=vec![root];
        while let Some(id)=stack.pop(){
            if let Some(e)=self.nodes.get_mut(&id){e.1+=1;continue}
            let n=b.nodes.get(&id).cloned().unwrap_or_else(||panic!("C-1 E4: node {} is neither stored nor in the batch",B256::from(id)));
            stack.extend(n.children());self.nodes.insert(id,(n,1));
        }
    }
    /// Releases one pin of `root`: a node no pinned root or stored node names any more is deleted, with
    /// the references it held.
    pub fn unpin(&mut self,root:&Id){
        match self.pins.get_mut(root){Some(c) if *c>1=>{*c-=1}Some(_)=>{self.pins.remove(root);}None=>panic!("C-1 E4: root {} is not pinned",B256::from(*root))}
        let mut stack=vec![*root];
        while let Some(id)=stack.pop(){
            let e=self.nodes.get_mut(&id).expect("a referenced node is stored");
            e.1-=1;if e.1==0{let (n,_)=self.nodes.remove(&id).unwrap();stack.extend(n.children());}
        }
    }
    /// Recounts every node's references from the pinned roots: equal to the stored counts, and no node
    /// stored that no root reaches (a test of the counts, not used by the store itself).
    pub fn recount(&self)->Result<(),String>{
        let mut count:HashMap<Id,u32>=HashMap::new();let mut seen=std::collections::HashSet::new();
        for (r,c) in &self.pins{*count.entry(*r).or_insert(0)+=*c;}
        let mut stack:Vec<Id>=self.pins.keys().copied().collect();
        while let Some(id)=stack.pop(){
            if !seen.insert(id){continue}
            let n=self.nodes.get(&id).ok_or_else(||format!("node {} is reachable but not stored",B256::from(id)))?;
            for c in n.0.children(){*count.entry(c).or_insert(0)+=1;stack.push(c);}
        }
        if seen.len()!=self.nodes.len(){return Err(format!("{} nodes stored, {} reachable",self.nodes.len(),seen.len()))}
        for (id,(_,c)) in &self.nodes{if count.get(id)!=Some(c){return Err(format!("node {}: stored count {c}, recounted {:?}",B256::from(*id),count.get(id)))}}
        Ok(())
    }
}

/// A state read from one trie version: what the executor reads (`view::View`), from the trie, the code
/// table and the preimages, with the version's BLOCKHASH window.
pub struct TrieState<'a>{pub store:&'a dyn NodeStore,pub trie:NodeTrie,pub block_hashes:&'a HashMap<U256,B256>}
impl TrieState<'_>{
    fn acct(&self,a:&Address)->Option<(u64,U256,B256,Option<Id>)>{
        let w=self.trie.world?;
        match get(self.store,&w,&hashed(a))?{Val::Acct{nonce,balance,code_hash,storage,..}=>Some((nonce,balance,code_hash,storage)),Val::Slot(..)=>None}
    }
    fn info(&self,nonce:u64,balance:U256,code_hash:B256)->AccountInfo{
        let code=if code_hash==KECCAK_EMPTY||code_hash.is_zero(){Some(Bytecode::default())}else{self.store.code(&code_hash)};
        AccountInfo{balance,nonce,code_hash,code}
    }
}
impl View for TrieState<'_>{
    fn account(&self,a:&Address)->Option<Acc>{
        self.acct(a).map(|(nonce,balance,code_hash,_)|Acc{info:self.info(nonce,balance,code_hash),state:AccountState::Touched})
    }
    fn slot(&self,a:&Address,k:&U256)->U256{
        let Some((_,_,_,Some(st)))=self.acct(a) else{return U256::ZERO};
        match get(self.store,&st,&hashed_slot(k)){Some(Val::Slot(v,_))=>v,_=>U256::ZERO}
    }
    fn each_slot(&self,a:&Address,f:&mut dyn FnMut(&U256,&U256)){
        let Some((_,_,_,Some(st)))=self.acct(a) else{return};
        walk(self.store,&st,&mut |_,v|{if let Val::Slot(v,slot)=v{f(slot,v)}});
    }
    fn contract(&self,h:&B256)->Option<Bytecode>{self.store.code(h)}
    fn block_hash(&self,n:u64)->B256{self.block_hashes.get(&U256::from(n)).copied().unwrap_or_else(||keccak256(n.to_string().as_bytes()))}
    fn each_account(&self,f:&mut dyn FnMut(&Address,Acc)){
        let Some(w)=self.trie.world else{return};
        walk(self.store,&w,&mut |_,v|{if let Val::Acct{nonce,balance,code_hash,address,..}=v{
            f(address,Acc{info:self.info(*nonce,*balance,*code_hash),state:AccountState::Touched})}});
    }
}

#[cfg(test)] mod tests;
