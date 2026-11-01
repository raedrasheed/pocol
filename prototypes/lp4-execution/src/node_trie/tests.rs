use super::*;
use revm::primitives::Bytes;

struct Rng(u64);
impl Rng{fn next(&mut self)->u64{self.0^=self.0<<13;self.0^=self.0>>7;self.0^=self.0<<17;self.0}fn below(&mut self,n:u64)->u64{self.next()%n}}
fn codes()->Vec<Bytecode>{vec![Bytecode::default(),Bytecode::new_raw(Bytes::from_static(&[0x60,1,0x60,0,0x55])),Bytecode::new_raw(Bytes::from_static(&[0x00])),Bytecode::new_raw(Bytes::from_static(&[0x60,2,0x00]))]}
fn info(r:&mut Rng,codes:&[Bytecode])->AccountInfo{
    let code=codes[r.below(codes.len() as u64) as usize].clone();
    AccountInfo{balance:U256::from(r.below(1<<40)),nonce:r.below(5),code_hash:code.hash_slow(),code:Some(code)}
}
fn addr(r:&mut Rng,pool:u64)->Address{Address::from_word(B256::from(U256::from(r.below(pool))))}
fn random_state(r:&mut Rng,n:u64,codes:&[Bytecode])->InMemoryDB{
    let mut db=InMemoryDB::default();
    for _ in 0..n{let a=addr(r,4*n+8);db.insert_account_info(a,info(r,codes));for _ in 0..r.below(6){db.insert_account_storage(a,U256::from(r.below(50)),U256::from(r.below(4))).unwrap();}}
    db
}
/// One block's worth of changes, as `state_trie`'s test makes them: new and removed accounts, balance and
/// nonce changes, slots written, zeroed and cleared, an account marked not existing.
fn mutate(r:&mut Rng,db:&InMemoryDB,codes:&[Bytecode],pool:u64)->InMemoryDB{
    let mut d=db.clone();let addrs:Vec<Address>=d.accounts.keys().copied().collect();
    for _ in 0..1+r.below(20){
        let a=if addrs.is_empty()||r.below(4)==0{addr(r,pool)}else{addrs[r.below(addrs.len() as u64) as usize]};
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
/// Every read the executor makes of a state, compared between the whole state and the trie version.
fn same_reads(db:&InMemoryDB,t:&dyn View,pool:u64,ctx:&str){
    let ex=|a:&Address|db.accounts.get(a).filter(|x|!matches!(x.account_state,AccountState::NotExisting)).is_some();
    for i in 0..pool{
        let a=Address::from_word(B256::from(U256::from(i)));
        let (p,q)=(db.account(&a).filter(|x|x.exists()),t.account(&a));
        assert_eq!(p.as_ref().map(|x|(x.info.nonce,x.info.balance,x.info.code_hash)),q.as_ref().map(|x|(x.info.nonce,x.info.balance,x.info.code_hash)),"{ctx}: account {i}");
        if let Some(q)=&q{
            let code=|v:&dyn View,i:&AccountInfo|i.code.clone().or_else(||v.contract(&i.code_hash)).map(|c|c.original_bytes());
            assert_eq!(code(db,&p.as_ref().unwrap().info),code(t,&q.info),"{ctx}: code of {i}");
        }
        for k in 0..60u64{let k=U256::from(k);assert_eq!(if ex(&a){db.slot(&a,&k)}else{U256::ZERO},t.slot(&a,&k),"{ctx}: slot {i}/{k}");}
        let (mut s1,mut s2)=(Vec::new(),Vec::new());
        if ex(&a){db.each_slot(&a,&mut |k,v|{if *v!=U256::ZERO{s1.push((*k,*v))}});}
        t.each_slot(&a,&mut |k,v|s2.push((*k,*v)));s1.sort();s2.sort();assert_eq!(s1,s2,"{ctx}: slots of {i}");
    }
    let mut x1:Vec<Address>=db.accounts.iter().filter(|(_,x)|!matches!(x.account_state,AccountState::NotExisting)).map(|(a,_)|*a).collect();
    let mut x2=Vec::new();t.each_account(&mut |a,acc|{assert!(acc.exists());x2.push(*a)});x1.sort();x2.sort();assert_eq!(x1,x2,"{ctx}: accounts");
}

#[test]
fn node_encodings_round_trip(){
    let codes=codes();let mut r=Rng(3);let db=random_state(&mut r,300,&codes);
    let s=MemStore::default();let mut b=Batch::default();let _=NodeTrie::build(&db,&s,&mut b);
    assert!(b.nodes.len()>300,"positive control: {} nodes",b.nodes.len());
    for (id,n) in &b.nodes{let e=n.encode();assert_eq!(Node::decode(&e).as_ref(),Some(&**n));assert_eq!(&keccak256(&e).0,id);
        assert!(Node::decode(&e[..e.len()-1]).is_none());let mut longer=e.clone();longer.push(0);assert!(Node::decode(&longer).is_none());}
}

#[test]
fn trie_versions_in_a_store_give_world_roots_and_the_state_reads(){
    let codes=codes();let mut r=Rng(0x4e3);
    for case in 0..12u64{
        let n=[0,1,2,40,150,300][case as usize%6];let pool=4*n+8;
        let mut db=random_state(&mut r,n,&codes);
        let mut store=MemStore::default();let mut b=Batch::default();
        let mut t=NodeTrie::build(&db,&store,&mut b);
        assert_eq!(t.root(&store,&b),crate::genesis_builder::world_root(&db).unwrap(),"case {case} build");
        store.commit(&b,t.world);
        let mut pinned:std::collections::VecDeque<(Option<Id>,InMemoryDB)>=[(t.world,db.clone())].into();
        for step in 0..25{
            let next=mutate(&mut r,&db,&codes,pool);let touched=Touched::between(&db,&next);
            let mut b=Batch::default();let u=t.update(&store,&mut b,&next,&touched);
            let root=u.root(&store,&b);
            assert_eq!(root,crate::genesis_builder::world_root(&next).unwrap(),"case {case} step {step}");
            assert_eq!(root,crate::state_trie::StateTrie::build(&next).root());
            store.commit(&b,u.world);pinned.push_back((u.world,next.clone()));
            // A window of four versions stays pinned; older ones are released.
            if pinned.len()>4{let (old,_)=pinned.pop_front().unwrap();if let Some(o)=old{store.unpin(&o)}}
            store.recount().unwrap_or_else(|e|panic!("case {case} step {step}: {e}"));
            // Every pinned version still reads as its own state.
            for (w,st) in &pinned{same_reads(st,&TrieState{store:&store,trie:NodeTrie{world:*w},block_hashes:&st.block_hashes},pool,&format!("case {case} step {step}"));}
            t=u;db=next;
        }
        for (w,_) in pinned{if let Some(w)=w{store.unpin(&w)}}
        assert!(store.nodes.is_empty(),"case {case}: {} nodes left after every root is released",store.nodes.len());
    }
}

#[test]
fn an_update_from_an_overlay_view_equals_the_update_from_its_merge(){
    // The executor gives its write set and the overlay itself (C-1 E3); the update must read the child
    // through the view, not through a whole state.
    let codes=codes();let mut r=Rng(91);
    let parent=random_state(&mut r,120,&codes);
    let store=MemStore::default();let mut b0=Batch::default();let t=NodeTrie::build(&parent,&store,&mut b0);
    let mut store=store;store.commit(&b0,t.world);
    for case in 0..40{
        let mut ov=crate::view::Overlay::new(&parent);
        for _ in 0..1+r.below(6){
            let a=addr(&mut r,488);let mut acct=revm::primitives::Account{info:ov.account(&a).map(|x|x.info).unwrap_or_default(),storage:Default::default(),status:revm::primitives::AccountStatus::Touched};
            match r.below(4){0=>acct.status|=revm::primitives::AccountStatus::SelfDestructed,1=>{acct.info.balance+=U256::from(7);}
                _=>{for _ in 0..3{let k=U256::from(r.below(50));let o=ov.slot(&a,&k);acct.storage.insert(k,revm::primitives::EvmStorageSlot::new_changed(o,U256::from(r.below(3))));}}}
            crate::user_phase::commit_tx_on(&mut ov,[(a,acct)].into(),true);
        }
        let merged=ov.merge();let touched=ov.touched();
        let mut b1=Batch::default();let mut b2=Batch::default();
        let via_view=t.update(&store,&mut b1,&ov,&touched);let via_merge=t.update(&store,&mut b2,&merged,&touched);
        assert_eq!(via_view,via_merge,"case {case}");
        assert_eq!(via_view.root(&store,&b1),crate::genesis_builder::world_root(&merged).unwrap(),"case {case}");
    }
}

/// p1_trie's root of raw 32-byte keys holding nonzero integers (as `state_trie`'s raw test).
fn reference(rows:&std::collections::BTreeMap<Key,U256>)->B256{
    let entries:Vec<(Vec<u8>,Vec<u8>)>=rows.iter().map(|(k,v)|{let mut e=Vec::new();rlp_uint(v,&mut e);(k.to_vec(),e)}).collect();
    B256::from(crate::trie::root(&entries,entries.len()).unwrap())
}
/// Keys that share long prefixes, so extensions above branches split and merge at every depth (hashed
/// keys almost never do).
fn clustered_key(r:&mut Rng)->Key{
    let mut k=[0u8;32];let shared=r.below(65) as usize;
    for i in 0..64{let n=if i<shared{(i%3) as u8}else{r.below(16) as u8};if i%2==0{k[i/2]|=n<<4}else{k[i/2]|=n}}
    k
}
#[test]
fn raw_clustered_keys_build_and_update_give_p1_roots(){
    let mut r=Rng(0x9e3779b97f4a7c15);let store=MemStore::default();
    for round in 0..300{
        let mut model=std::collections::BTreeMap::new();let mut b=Batch::default();let mut t:Option<(Id,Arc<Node>)>=None;
        for _ in 0..1+r.below(12){
            let mut ch:Vec<(Key,Option<Val>)>=Vec::new();
            for _ in 0..1+r.below(if round%2==0{4}else{40}){
                let k=if !model.is_empty()&&r.below(3)==0{*model.keys().nth(r.below(model.len() as u64) as usize).unwrap()}else{clustered_key(&mut r)};
                if ch.iter().any(|(x,_)|*x==k){continue}
                let small=r.below(2)==0;let v=if r.below(3)==0{None}else{Some(U256::from(1+r.below(if small{3}else{u64::MAX})))};
                ch.push((k,v.map(|v|Val::Slot(v,U256::from(7u64)))));
            }
            ch.sort_by(|a,b|a.0.cmp(&b.0));
            for (k,v) in &ch{match v{Some(Val::Slot(v,_))=>{model.insert(*k,*v);}_=>{model.remove(k);}}}
            t=update(&store,&mut b,t,0,&ch);
            assert_eq!(root_of(t.as_ref().map(|x|&*x.1)),reference(&model),"round {round}");
        }
    }
}

// ---------------- C-1 E4.4 (2026-10-27): what differs between two versions ----------------
/// A store that records every node read through it.
struct Reads<'a>{s:&'a dyn NodeStore,ids:std::cell::RefCell<std::collections::HashSet<Id>>}
impl NodeStore for Reads<'_>{
    fn node(&self,id:&Id)->Option<Arc<Node>>{self.ids.borrow_mut().insert(*id);self.s.node(id)}
    fn code(&self,h:&B256)->Option<Bytecode>{self.s.code(h)}
}
/// Every node of the subtree at `root` (a storage trie hangs from its account's leaf).
fn reach(s:&dyn NodeStore,root:Option<Id>)->std::collections::HashSet<Id>{
    let mut seen=std::collections::HashSet::new();let mut st:Vec<Id>=root.into_iter().collect();
    while let Some(id)=st.pop(){if seen.insert(id){st.extend(s.node(&id).unwrap().children())}}seen
}
/// A `Touched` as a value: each account, whole (None) or with its distinct slots in order.
fn norm(t:&Touched)->std::collections::BTreeMap<Address,Option<Vec<U256>>>{t.iter().map(|(a,s)|(*a,s.map(|s|{let mut v=s.to_vec();v.sort();v.dedup();v}))).collect()}

/// `NodeTrie::diff` lists what `Touched::between` lists for the two whole states, `Summary::moved` through it gives
/// the second version's `scan_view`, and it reads every node the second version holds and the first does not. Pairs:
/// each version and the next, versions further apart in either direction, an unrelated state, the empty one.
#[test]
fn diff_lists_what_differs_and_reads_every_node_only_the_second_version_holds(){
    use crate::state_summary::Summary;
    let codes=codes();let mut r=Rng(0x51ab);let (mut pairs,mut big)=(0u64,vec![]);
    for case in 0..12u64{
        let n=[0,1,3,40,150,400][case as usize%6];let pool=4*n+8;
        let mut store=MemStore::default();let mut db=random_state(&mut r,n,&codes);
        let mut b=Batch::default();let t0=NodeTrie::build(&db,&store,&mut b);store.commit(&b,t0.world);
        let mut v=vec![(t0,db.clone())];
        for _ in 0..20{
            let next=mutate(&mut r,&db,&codes,pool);let touched=Touched::between(&db,&next);let mut b=Batch::default();
            let u=v.last().unwrap().0.update(&store,&mut b,&next,&touched);store.commit(&b,u.world);v.push((u,next.clone()));db=next;
        }
        let other=random_state(&mut r,n+5,&codes);let mut b=Batch::default();let o=NodeTrie::build(&other,&store,&mut b);store.commit(&b,o.world);
        let mut cases:Vec<(usize,usize,bool)>=(1..v.len()).map(|i|(i-1,i,true)).collect();
        for _ in 0..10{cases.push((r.below(v.len() as u64) as usize,r.below(v.len() as u64) as usize,false));}
        v.push((o,other));v.push((NodeTrie{world:None},InMemoryDB::default()));let (u,e)=(v.len()-2,v.len()-1);
        cases.extend([(0,u,false),(u,0,false),(e,0,false),(0,e,false),(e,e,false),(e,u,false)]);
        for (i,j,next) in cases{
            let ((fa,fd),(ta,td))=(&v[i],&v[j]);let ctx=format!("case {case}: version {i} -> {j}");
            let reads=Reads{s:&store,ids:Default::default()};let t=fa.diff(&reads,ta);
            assert_eq!(norm(&t),norm(&Touched::between(fd,td)),"{ctx}: the accounts and slots that differ");
            let (fv,tv)=(TrieState{store:&store,trie:*fa,block_hashes:&fd.block_hashes},TrieState{store:&store,trie:*ta,block_hashes:&td.block_hashes});
            assert_eq!(Summary::scan_view(&fv,24577).moved(&fv,&tv,&t),Summary::scan_view(&tv,24577),"{ctx}: the summary moved through the diff");
            let (from,to)=(reach(&store,fa.world),reach(&store,ta.world));let read=reads.ids.into_inner();
            assert!(to.difference(&from).all(|id|read.contains(id)),"{ctx}: a node only the second version holds was not read");
            if next&&n==400{big.push(read.len() as f64/to.len() as f64)}pairs+=1;
        }
    }
    // Positive control: between one version and the next of a 400-account state, the diff reads a small share of
    // the version (a whole read would be 1).
    big.sort_by(|a,b|a.partial_cmp(b).unwrap());let med=big[big.len()/2];
    println!("NODE-TRIE-DIFF pairs {pairs} nextOf400 {} medianShareRead {med:.3} maxShareRead {:.3}",big.len(),big[big.len()-1]);
    assert!(med<0.25,"the diff of consecutive versions reads {med} of the version");
}

/// Keys that share long prefixes, so that branches split and merge at every depth between the two versions: the
/// diff's changes are exactly the rows that differ, and it reads every node only the second trie holds.
#[test]
fn diff_of_raw_clustered_keys_is_exactly_the_rows_that_differ(){
    let mut r=Rng(0x2545f4914f6cdd1d);let store=MemStore::default();let mut reads_total=(0usize,0usize);
    for round in 0..300{
        let mut model=std::collections::BTreeMap::new();let mut b=Batch::default();let mut t:Option<(Id,Arc<Node>)>=None;
        for _ in 0..1+r.below(12){
            let (before,old)=(model.clone(),t.as_ref().map(|x|x.0));
            let mut ch:Vec<(Key,Option<Val>)>=Vec::new();
            for _ in 0..1+r.below(if round%2==0{4}else{40}){
                let k=if !model.is_empty()&&r.below(3)==0{*model.keys().nth(r.below(model.len() as u64) as usize).unwrap()}else{clustered_key(&mut r)};
                if ch.iter().any(|(x,_)|*x==k){continue}
                let v=if r.below(3)==0{None}else{Some(U256::from(1+r.below(3)))};
                ch.push((k,v.map(|v|Val::Slot(v,U256::from(7u64)))));
            }
            ch.sort_by(|a,b|a.0.cmp(&b.0));
            for (k,v) in &ch{match v{Some(Val::Slot(v,_))=>{model.insert(*k,*v);}_=>{model.remove(k);}}}
            t=update(&store,&mut b,t,0,&ch);let new=t.as_ref().map(|x|x.0);
            let over=Over{store:&store,batch:&b};let reads=Reads{s:&over,ids:Default::default()};
            let mut got=std::collections::BTreeMap::new();
            let val=|v:&Val|match v{Val::Slot(v,_)=>*v,_=>panic!("a slot trie")};
            diff_ids(&reads,old.as_ref(),new.as_ref(),&mut |c|{let (k,x,y)=match c{Change::Gone(k,v)=>(*k,Some(val(v)),None),Change::Came(k,v)=>(*k,None,Some(val(v))),
                Change::Moved(k,x,y)=>(*k,Some(val(x)),Some(val(y)))};assert!(got.insert(k,(x,y)).is_none(),"round {round}: a key reported twice");});
            let want:std::collections::BTreeMap<Key,(Option<U256>,Option<U256>)>=before.keys().chain(model.keys()).map(|k|(*k,(before.get(k).copied(),model.get(k).copied())))
                .filter(|(_,(x,y))|x!=y).collect();
            assert_eq!(got,want,"round {round}: the changes are the rows that differ");
            let (from,to)=(reach(&over,old),reach(&over,new));let read=reads.ids.into_inner();
            assert!(to.difference(&from).all(|id|read.contains(id)),"round {round}: a node only the new trie holds was not read");
            reads_total=(reads_total.0+read.len(),reads_total.1+to.len());
        }
    }
    println!("NODE-TRIE-DIFF-RAW nodesRead {} nodesOfTheNewTries {}",reads_total.0,reads_total.1);
}

// ---------------- A13 (C-1 E4.5): what a store keeps of versions ----------------
/// Bytes a store keeps: every row's 32-byte id, 4-byte count and serialization (as pocold's state_store
/// keeps them; counted here independently of `ROW`).
fn stored(s:&MemStore)->u128{s.nodes.values().map(|(n,_)|32+4+n.encode().len() as u128).sum()}
/// Every stored node within its kind's serialization bound, and every branch with two children or more.
fn kinds_within(s:&MemStore,ctx:&str){
    for (n,_) in s.nodes.values(){
        let e=n.encode().len() as u128;
        match &**n{
            Node::Branch{kids,..}=>{assert!(kids.len()>=2,"{ctx}: a branch with {} children",kids.len());assert!(e<=BRANCH_ENC+KID_ENC*kids.len() as u128,"{ctx}: branch {e}");}
            Node::Leaf{val:Val::Acct{..},..}=>assert!(e<=ACCT_LEAF_ENC,"{ctx}: account leaf {e}"),
            Node::Leaf{val:Val::Slot(..),..}=>assert!(e<=SLOT_LEAF_ENC,"{ctx}: slot leaf {e}"),
        }
    }
}
/// A batch's (branches, leaves, children of its branches).
fn batch_counts(b:&Batch)->(u128,u128,u128){
    let (mut br,mut lv,mut kids)=(0,0,0);
    for n in b.nodes.values(){match &**n{Node::Branch{kids:k,..}=>{br+=1;kids+=k.len() as u128}Node::Leaf{..}=>lv+=1}}
    (br,lv,kids)
}
/// A whole state's keys: (existing accounts, nonzero slots).
fn keys_of(db:&InMemoryDB)->(u128,u128){
    let live=db.accounts.values().filter(|x|!matches!(x.account_state,AccountState::NotExisting));
    live.fold((0,0),|(a,s),x|(a+1,s+x.storage.values().filter(|v|**v!=U256::ZERO).count() as u128))
}
/// The accounts and slots a write set changes, as `batch_nodes` counts them: an account listed whole brings
/// every nonzero slot it has in the child.
fn changed(t:&Touched,child:&InMemoryDB)->(u128,u128){
    t.iter().fold((0,0),|(a,s),(addr,slots)|(a+1,s+match slots{Some(k)=>k.len() as u128,None=>child.accounts.get(addr).map_or(0,|x|x.storage.values().filter(|v|**v!=U256::ZERO).count() as u128)}))
}
/// A window of consecutive versions holds at most the oldest whole and what each later block added, and at
/// most each whole: entries (root, the version's whole bound, the bound of what its block added).
fn window_within(store:&MemStore,w:&std::collections::VecDeque<(Option<Id>,u128,u128)>,ctx:&str){
    let union=w[0].1+w.iter().skip(1).map(|x|x.2).sum::<u128>();let each:u128=w.iter().map(|x|x.1).sum();
    assert!(stored(store)<=union.min(each),"{ctx}: {} stored, bound {}",stored(store),union.min(each));
}

/// Hashed keys: random states and blocks of accounts with storage (accounts created, removed and cleared),
/// four consecutive versions pinned. Each version, each batch, each block's addition and the window stay
/// within the bounds A13 counts (version_bytes, batch_nodes, block_bytes).
#[test]
fn a_store_keeps_at_most_what_a13_counts_for_its_versions(){
    let codes=codes();let mut r=Rng(0xa13);let mut tight=0u128;
    for case in 0..12u64{
        let n=[1,2,40,150,300,600][case as usize%6];let pool=4*n+8;
        let mut db=random_state(&mut r,n,&codes);
        let mut store=MemStore::default();let mut b=Batch::default();let mut t=NodeTrie::build(&db,&store,&mut b);store.commit(&b,t.world);
        let (a,s)=keys_of(&db);let whole=version_bytes(a,s).unwrap();assert!(stored(&store)<=whole,"case {case}: one version");kinds_within(&store,&format!("case {case}"));
        let mut w:std::collections::VecDeque<(Option<Id>,u128,u128)>=[(t.world,whole,whole)].into();
        for step in 0..30{
            let ctx=format!("case {case} step {step}");
            let next=mutate(&mut r,&db,&codes,pool);let touched=Touched::between(&db,&next);let (ca,cs)=keys_of(&next);let (wa,ws)=changed(&touched,&next);
            let mut b=Batch::default();let u=t.update(&store,&mut b,&next,&touched);
            let (br,lv,kids)=batch_counts(&b);let (mb,ml,mk)=batch_nodes(wa,ws,ca,cs).unwrap();
            assert!(br<=mb&&lv<=ml&&kids<=mk,"{ctx}: batch ({br},{lv},{kids}) over ({mb},{ml},{mk})");
            let before=stored(&store);store.commit(&b,u.world);let added=stored(&store)-before;let bound=block_bytes(wa,ws,ca,cs).unwrap();
            assert!(added<=bound,"{ctx}: a block added {added}, bound {bound}");tight=tight.max(added*100/bound.max(1));
            kinds_within(&store,&ctx);
            w.push_back((u.world,version_bytes(ca,cs).unwrap(),bound));
            if w.len()>4{let (old,_,_)=w.pop_front().unwrap();if let Some(o)=old{store.unpin(&o)}}
            window_within(&store,&w,&ctx);
            t=u;db=next;
        }
    }
    assert!(tight>=10,"positive control: some block adds a sizeable share of its bound ({tight}%)");
}

/// Raw keys that share long prefixes (paths of up to 64 branches, which hashed keys almost never make):
/// each batch, each block's addition and a window of three versions stay within the same bounds.
#[test]
fn deep_shared_prefixes_stay_within_what_a13_counts(){
    let mut r=Rng(0x51de);let mut deepest=0;
    for round in 0..300{
        let mut store=MemStore::default();let mut model=std::collections::BTreeMap::new();let mut t:Option<(Id,Arc<Node>)>=None;
        let mut w:std::collections::VecDeque<(Option<Id>,u128,u128)>=Default::default();
        for step in 0..1+r.below(12){
            let ctx=format!("round {round} step {step}");
            let mut ch:Vec<(Key,Option<Val>)>=Vec::new();
            for _ in 0..1+r.below(if round%2==0{4}else{40}){
                let k=if !model.is_empty()&&r.below(3)==0{*model.keys().nth(r.below(model.len() as u64) as usize).unwrap()}else{clustered_key(&mut r)};
                if ch.iter().any(|(x,_)|*x==k){continue}
                let v=if r.below(3)==0{None}else{Some(U256::from(1+r.below(u64::MAX)))};
                ch.push((k,v.map(|v|Val::Slot(v,U256::from(7u64)))));
            }
            ch.sort_by(|a,b|a.0.cmp(&b.0));
            for (k,v) in &ch{match v{Some(Val::Slot(v,_))=>{model.insert(*k,*v);}_=>{model.remove(k);}}}
            let mut b=Batch::default();let u=update(&store,&mut b,t.clone(),0,&ch);
            let (keys,child)=(ch.len() as u128,model.len() as u128);
            let (br,lv,kids)=batch_counts(&b);let (mb,ml,mk)=batch_nodes(0,keys,0,child).unwrap();
            assert!(br<=mb&&lv<=ml&&kids<=mk,"{ctx}: batch ({br},{lv},{kids}) over ({mb},{ml},{mk})");
            let before=stored(&store);store.commit(&b,u.as_ref().map(|x|x.0));let added=stored(&store)-before;
            assert!(added<=block_bytes(0,keys,0,child).unwrap(),"{ctx}: a block added {added}");
            kinds_within(&store,&ctx);
            let whole=version_bytes(0,child).unwrap();w.push_back((u.as_ref().map(|x|x.0),whole,if w.is_empty(){whole}else{block_bytes(0,keys,0,child).unwrap()}));
            if w.len()>3{let (old,_,_)=w.pop_front().unwrap();if let Some(o)=old{store.unpin(&o)}}
            window_within(&store,&w,&ctx);
            for (n,_) in store.nodes.values(){if let Node::Branch{at,..}=&**n{deepest=deepest.max(*at)}}
            t=u;
        }
    }
    assert!(deepest>=60,"positive control: branches near the last nibble ({deepest})");
}

/// The bounds' worst cases, reached exactly: a key whose path has a branch at every nibble (64 keys that
/// leave its prefix one nibble deeper each), so changing it rebuilds 64 branches; and a key put where a leaf
/// is, so the batch holds the new leaf and the displaced one.
#[test]
fn a_path_of_64_branches_and_a_displaced_leaf_reach_the_batch_bounds(){
    let set=|k:&mut Key,i:usize,n:u8|{if i%2==0{k[i/2]=(k[i/2]&0x0f)|(n<<4)}else{k[i/2]=(k[i/2]&0xf0)|n}};
    let target=[0x11u8;32];let mut ch:Vec<(Key,Option<Val>)>=vec![(target,Some(Val::Slot(U256::from(1),U256::ZERO)))];
    for i in 0..64{let mut k=target;set(&mut k,i,2);ch.push((k,Some(Val::Slot(U256::from(2),U256::ZERO))));}
    ch.sort_by(|a,b|a.0.cmp(&b.0));
    let mut store=MemStore::default();let mut b=Batch::default();let t=update(&store,&mut b,None,0,&ch);store.commit(&b,t.as_ref().map(|x|x.0));
    let mut b=Batch::default();let u=update(&store,&mut b,t,0,&[(target,Some(Val::Slot(U256::from(3),U256::ZERO)))]);
    let (br,lv,_)=batch_counts(&b);assert_eq!((br,lv),(64,1),"the whole path is rebuilt");
    assert_eq!(batch_nodes(0,1,0,65).unwrap().0,64,"and the bound is that path");
    store.commit(&b,u.as_ref().map(|x|x.0));
    // One leaf, and a key that shares all but its last nibble: a branch and both leaves.
    let mut store=MemStore::default();let mut b=Batch::default();let one=[(target,Some(Val::Slot(U256::from(1),U256::ZERO)))];
    let t=update(&store,&mut b,None,0,&one);store.commit(&b,t.as_ref().map(|x|x.0));
    let mut k=target;set(&mut k,63,5);let mut b=Batch::default();let _=update(&store,&mut b,t,0,&[(k,Some(Val::Slot(U256::from(9),U256::ZERO)))]);
    let (br,lv,kids)=batch_counts(&b);assert_eq!((br,lv,kids),(1,2,2),"a branch at the last nibble over the new and the displaced leaf");
    let (mb,ml,mk)=batch_nodes(0,1,0,2).unwrap();assert_eq!(ml,2,"the leaves' bound counts the displaced leaf");assert!(br<=mb&&kids<=mk);
}

/// A13 (pocold `chain_versions_bytes`): what a block's version adds is at most a whole version, term by term,
/// for any counts the caps allow (each a u64, plus the system keys). So the union bound whole + (n−1)·block is
/// never above n·whole for any n a u32 K_hist gives, and the `min` there never picks n·whole: its mutant (P6)
/// is equivalent. This pins that premise; a change to `block_bytes` that broke it would fail here.
#[test]
fn a_blocks_addition_is_at_most_a_whole_version(){
    let edge=[0u128,1,2,63,64,65,1<<20,u128::from(u64::MAX),u128::from(u64::MAX)+(1<<32)];
    let pick=|r:&mut Rng|->u128{if r.below(3)==0{edge[r.below(edge.len() as u64) as usize]}else{u128::from(r.next())>>r.below(64)}};
    let (mut r,mut equal,n)=(Rng(0xb10c),0,u128::from(u32::MAX)+1);
    for _ in 0..200_000{
        let (wa,ws,ka,ks)=(pick(&mut r),pick(&mut r),pick(&mut r),pick(&mut r));
        let (Some(b),Some(v))=(block_bytes(wa,ws,ka,ks),version_bytes(ka,ks)) else{panic!("overflow at {wa} {ws} {ka} {ks}")};
        assert!(b<=v,"{wa} {ws} {ka} {ks}: {b} > {v}");
        assert!(v.checked_add((n-1).checked_mul(b).unwrap()).unwrap()<=n.checked_mul(v).unwrap());
        if b==v&&v>0{equal+=1}
    }
    assert!(equal>0,"positive control: a block that changes every key of the child reaches a whole version");
}
